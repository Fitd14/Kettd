/**
 * 粘贴截图（缺陷 #15 / tasks.md 6.3）：剪贴板图片 → canvas 压缩 → data URL 插入正文。
 *
 * 设计（design.md D5，MVP）：
 * - 仅知识库条目启用：调用方用 allowImages gate（便签不启用，保持 500 字上限）。
 * - 不新增依赖：压缩全部用浏览器原生能力（createImageBitmap / Image + canvas）。
 * - 图片以 `data:image/webp|png;base64,...` 写进正文，存进 notes.json 的就是这段字符串，
 *   所以既要压得小（单图 ≤300KB），也要给整条笔记设图片总量上限（默认 2.5MB）。
 * - 超限/解码失败：先 preventDefault 接管本次粘贴，再在编辑器旁弹一条中文轻提示，不写入任何数据。
 *
 * 「零运行时 import」约束：src-react/test/image-paste.test.mjs 用 Node 内置类型剥离直接
 * import 本 .ts，测 extractImageFiles 的纯逻辑；@tiptap/pm/* 在 node_modules 里是 TS 再导出，
 * Node 拒绝剥离 node_modules 内的 TS。因此本文件只允许 `import type`，且模块顶层不得触碰
 * window/document（真正的 DOM 访问都放在函数内惰性执行）。
 */
import type { EditorView } from '@tiptap/pm/view'

/* ─── 参数（压缩/上限策略；改动请同步 tasks.md 与 design.md D5）──────────── */

/**
 * 单张图片上限：300KB，按 data URL 字符串长度计（base64 全 ASCII，1 字符 ≈ 1 字节，
 * 也就是写进 notes.json 的体积）。超限继续降质量/缩小，缩到长边 900 仍超则拒绝。
 */
export const MAX_IMAGE_DATA_URL_BYTES = 300 * 1024

/** 单条笔记正文内图片 data URL 总量上限：2.5MB（默认值，可由 opts.maxDocBytes 覆盖） */
export const DEFAULT_MAX_DOC_IMAGE_BYTES = 2.5 * 1024 * 1024

/**
 * 一次粘贴最多处理的图片张数：多图逐张压缩插入，取前 3 张、其余忽略并提示。
 * 上限很低是有意的——连续粘贴多张大图会长时间占用主线程，截图场景通常只有一张。
 */
export const MAX_IMAGES_PER_PASTE = 3

/** 压缩目标：长边上限（超过按比例缩小，不放大） */
const LONG_EDGE_MAX = 1600

/** 压缩下限：长边缩到 900 仍超上限就拒绝（再缩截图已不可读） */
const LONG_EDGE_MIN = 900

/** 每轮压缩仍超限后的缩放系数 */
const SCALE_STEP = 0.85

/** WebP 质量梯度：从高到低逐级尝试，第一档通常就够 */
const WEBP_QUALITIES = [0.82, 0.74, 0.66, 0.58, 0.5]

/**
 * 解码像素上限：约 8000×8000，远超任何截图；挡住极端大图（如全景照片）解码时的内存峰值。
 * 超过则按「图片太大」拒绝，不尝试压缩。
 */
const MAX_DECODE_PIXELS = 64_000_000

/** 提示条停留时长（毫秒），与 styles/prosemirror.css 里的 .kb-editor-toast 配套 */
const TOAST_MS = 2600

/* ─── 剪贴板提取（纯函数，单测覆盖）──────────────────────────────────── */

/** DataTransferItem 的最小结构（便于测试传伪对象） */
export interface ClipboardItemLike {
  readonly kind: string
  readonly type: string
  getAsFile(): File | null
}

/** DataTransfer 的最小结构（Browser 的 DataTransfer 结构上兼容） */
export interface ClipboardDataLike {
  readonly items?: ArrayLike<ClipboardItemLike> | null
  readonly files?: ArrayLike<File> | null
}

function isImageMime(mime: string | null | undefined): mime is string {
  return typeof mime === 'string' && mime.toLowerCase().startsWith('image/')
}

/**
 * 从剪贴板对象里提取图片文件（纯函数，无 DOM 依赖）。
 *
 * 顺序：先看 `items`（截图/复制的图片都以 kind==='file' 的条目出现）；
 * 只有 items 一张图都没拿到时才回退 `files`（文件拖放/部分系统只填 files），避免同一张图出现两次。
 */
export function extractImageFiles(clipboardData: ClipboardDataLike | null | undefined): File[] {
  if (!clipboardData) return []
  const result: File[] = []

  const items = clipboardData.items
  if (items && typeof items.length === 'number') {
    for (let i = 0; i < items.length; i += 1) {
      const item = items[i]
      if (!item || item.kind !== 'file') continue
      const file = item.getAsFile()
      if (!file) continue
      // 个别来源 item.type 为空、类型只挂在 File 上；两处都看
      if (isImageMime(file.type || item.type)) result.push(file)
    }
  }

  if (result.length === 0) {
    const files = clipboardData.files
    if (files && typeof files.length === 'number') {
      for (let i = 0; i < files.length; i += 1) {
        const file = files[i]
        if (file && isImageMime(file.type)) result.push(file)
      }
    }
  }

  return result
}

/* ─── 压缩：decoded 图片 → 长边 ≤1600 的 canvas → webp/png data URL ≤300KB ── */

interface DecodedImage {
  source: CanvasImageSource
  width: number
  height: number
  dispose(): void
}

type CompressFailure = 'decode' | 'encode' | 'too-large'

type CompressResult = { ok: true; dataUrl: string } | { ok: false; reason: CompressFailure }

function loadHtmlImage(url: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image()
    image.onload = () => resolve(image)
    image.onerror = () => reject(new Error('image decode failed'))
    image.src = url
  })
}

/** 解码图片文件；createImageBitmap 优先（无 object URL 泄漏、后台解码），失败再用 <img> 兜底 */
async function decodeImageFile(file: Blob): Promise<DecodedImage> {
  if (typeof createImageBitmap === 'function') {
    try {
      const bitmap = await createImageBitmap(file)
      return {
        source: bitmap,
        width: bitmap.width,
        height: bitmap.height,
        dispose: () => bitmap.close(),
      }
    } catch {
      // 交给 <img> 兜底：个别格式 createImageBitmap 不认但 <img> 能解
    }
  }

  const objectUrl = URL.createObjectURL(file)
  try {
    const image = await loadHtmlImage(objectUrl)
    return {
      source: image,
      width: image.naturalWidth,
      height: image.naturalHeight,
      dispose: () => URL.revokeObjectURL(objectUrl),
    }
  } catch (error) {
    URL.revokeObjectURL(objectUrl)
    throw error
  }
}

let webpSupportCache: boolean | null = null

/** WebView2（Chromium）都支持 webp；缓存探测结果，兼容未来 WebView 内核替换 */
function detectWebpSupport(): boolean {
  if (webpSupportCache !== null) return webpSupportCache
  try {
    const probe = document.createElement('canvas')
    probe.width = 1
    probe.height = 1
    webpSupportCache = probe.toDataURL('image/webp').startsWith('data:image/webp')
  } catch {
    webpSupportCache = false
  }
  return webpSupportCache
}

function isWithinLimit(dataUrl: string): boolean {
  return dataUrl.startsWith('data:image/') && dataUrl.length <= MAX_IMAGE_DATA_URL_BYTES
}

/**
 * 把已解码图片按指定的「长边像素」画进 canvas 并编码成 data URL。
 * 先按 WebP 质量梯度试（q 从高到低），不支持 WebP 或全部超限时再试 PNG（PNG 无质量参数）。
 * 返回 null 表示这个尺寸下怎么编码都超 300KB。
 */
function encodeAtLongEdge(
  source: CanvasImageSource,
  sourceWidth: number,
  sourceHeight: number,
  longEdge: number,
): string | null {
  const scale = longEdge / Math.max(sourceWidth, sourceHeight)
  const targetWidth = Math.max(1, Math.round(sourceWidth * scale))
  const targetHeight = Math.max(1, Math.round(sourceHeight * scale))

  const canvas = document.createElement('canvas')
  canvas.width = targetWidth
  canvas.height = targetHeight
  const context = canvas.getContext('2d')
  if (!context) return null
  context.imageSmoothingEnabled = true
  context.imageSmoothingQuality = 'high'
  // 不铺底色：png/webp 截图可能带透明区域，jpg 来源没有 alpha，都不受影响
  context.drawImage(source, 0, 0, targetWidth, targetHeight)

  if (detectWebpSupport()) {
    for (const quality of WEBP_QUALITIES) {
      const webp = canvas.toDataURL('image/webp', quality)
      if (isWithinLimit(webp)) return webp
    }
  }

  const png = canvas.toDataURL('image/png')
  return isWithinLimit(png) ? png : null
}

/**
 * 压缩一张图片文件。长边从 min(原长边, 1600) 起，逐级 ×0.85 缩，最低到 900；
 * 每个尺寸先降 WebP 质量，仍超 300KB 再缩尺寸；长边 900 仍超 → too-large。
 */
async function compressImageFile(file: File): Promise<CompressResult> {
  let decoded: DecodedImage
  try {
    decoded = await decodeImageFile(file)
  } catch {
    return { ok: false, reason: 'decode' }
  }

  try {
    const { source, width, height } = decoded
    if (width <= 0 || height <= 0) return { ok: false, reason: 'decode' }
    if (width * height > MAX_DECODE_PIXELS) return { ok: false, reason: 'too-large' }

    const startLongEdge = Math.min(Math.max(width, height), LONG_EDGE_MAX)
    const minLongEdge = Math.min(startLongEdge, LONG_EDGE_MIN)
    let longEdge = startLongEdge

    for (;;) {
      let dataUrl: string | null
      try {
        dataUrl = encodeAtLongEdge(source, width, height, longEdge)
      } catch {
        return { ok: false, reason: 'encode' }
      }
      if (dataUrl) return { ok: true, dataUrl }

      if (longEdge <= minLongEdge) return { ok: false, reason: 'too-large' }
      const next = Math.floor(longEdge * SCALE_STEP)
      longEdge = next < minLongEdge ? minLongEdge : next
    }
  } finally {
    decoded.dispose()
  }
}

/* ─── 文档统计 / 插入 / 提示 ─────────────────────────────────────────── */

/** 统计文档内所有 image 节点 src 字符串长度之和（即这条笔记图片占用的 JSON 体积） */
function measureDocImageBytes(view: EditorView): number {
  let total = 0
  view.state.doc.descendants((node) => {
    if (node.type.name !== 'image') return true
    const { src } = node.attrs
    if (typeof src === 'string') total += src.length
    return false
  })
  return total
}

function normalizeMaxDocBytes(value: number | undefined): number {
  if (typeof value !== 'number' || !Number.isFinite(value) || value <= 0) {
    return DEFAULT_MAX_DOC_IMAGE_BYTES
  }
  return value
}

function formatMegabytes(bytes: number): string {
  const megabytes = bytes / (1024 * 1024)
  return Number.isInteger(megabytes) ? String(megabytes) : megabytes.toFixed(1)
}

/** 在当前选区插入图片节点（replaceSelectionWith 用 inheritMarks=false，避免图片带上粗体等 mark） */
function insertImageNode(view: EditorView, src: string, alt: string): boolean {
  const { state } = view
  const imageType = state.schema.nodes.image
  if (!imageType) return false
  try {
    const node = imageType.create({ src, alt })
    const transaction = state.tr.replaceSelectionWith(node, false).scrollIntoView()
    view.dispatch(transaction)
    view.focus()
    return true
  } catch {
    // 例如在代码块（content 只允许 text）里粘贴：当前节点不允许 image，静默失败由调用方提示
    return false
  }
}

const toastTimers = new WeakMap<HTMLElement, ReturnType<typeof setTimeout>>()

/** 在编辑器容器内弹一条轻提示（同一容器复用同一个节点，2.6 秒后移除） */
function showEditorToast(view: EditorView, message: string): void {
  const parent = view.dom.parentElement
  if (!parent) return
  const existing = parent.querySelector<HTMLElement>(':scope > .kb-editor-toast')
  const toast = existing ?? document.createElement('div')
  if (!existing) {
    toast.className = 'kb-editor-toast'
    toast.setAttribute('role', 'status')
    parent.appendChild(toast)
  }
  toast.textContent = message

  const previous = toastTimers.get(toast)
  if (previous) clearTimeout(previous)
  const timer = setTimeout(() => {
    toastTimers.delete(toast)
    toast.remove()
  }, TOAST_MS)
  toastTimers.set(toast, timer)
}

/** handleImagePaste 的可选项 */
export interface HandleImagePasteOptions {
  /** 是否启用图片粘贴（知识库条目 true；便签 false，保持 500 字上限） */
  allowImages: boolean
  /** 单条笔记图片 data URL 总量上限（字节），缺省 2.5MB */
  maxDocBytes?: number
}

/** 把压缩失败原因翻译成用户可见的中文短句 */
function failureMessage(reason: CompressFailure): string {
  if (reason === 'decode') return '这张图片无法读取，未插入'
  if (reason === 'encode') return '这张图片处理失败，未插入'
  return '图片太大，压缩后仍超过 300KB，未插入'
}

/**
 * ProseMirror handlePaste 处理函数（在 editorProps.handlePaste 里调用）。
 *
 * 返回值语义：
 * - false：不接管（未启用图片粘贴 / 剪贴板里没有图片），交给编辑器默认处理；
 * - true：已接管本次粘贴（event.preventDefault），图片在异步压缩后逐张插入。
 */
export function handleImagePaste(
  view: EditorView,
  event: ClipboardEvent,
  opts: HandleImagePasteOptions,
): boolean {
  if (!opts.allowImages) return false

  const files = extractImageFiles(event.clipboardData)
  if (files.length === 0) return false

  // 命中图片：必须挡掉编辑器默认粘贴，否则剪贴板里的 HTML/文件名文本也会一起进来
  event.preventDefault()

  const queue = files.slice(0, MAX_IMAGES_PER_PASTE)
  if (files.length > queue.length) {
    showEditorToast(view, `一次最多粘贴 ${MAX_IMAGES_PER_PASTE} 张图片，其余已忽略`)
  }

  const maxDocBytes = normalizeMaxDocBytes(opts.maxDocBytes)

  // 逐张顺序处理：不并发解码多张大图，避免内存峰值与画布竞争；
  // 每张 await 后让出事件循环，压缩期间 UI 仍有响应。
  void (async () => {
    for (const file of queue) {
      if (view.isDestroyed) return

      const compressed = await compressImageFile(file)
      if (view.isDestroyed) return

      if (!compressed.ok) {
        showEditorToast(view, failureMessage(compressed.reason))
        continue
      }

      const usedBytes = measureDocImageBytes(view)
      if (usedBytes + compressed.dataUrl.length > maxDocBytes) {
        showEditorToast(view, `本条笔记图片总量已满（约 ${formatMegabytes(maxDocBytes)}MB），未插入`)
        return
      }

      if (!insertImageNode(view, compressed.dataUrl, '截图')) {
        showEditorToast(view, '当前位置无法插入图片，未插入')
        return
      }
    }
  })()

  return true
}
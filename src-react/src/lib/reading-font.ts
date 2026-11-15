/**
 * 阅读面字体（CCG: 用户要「字体切换 + 自己上传」）—— 只改 `--font-reading`，
 * 不碰 UI 黑体（--font-ui）与报头衬线（Kettd Serif）。
 *
 * 三态与 Rust 侧 `models::normalize_reading_font` 一一对应：
 * - ""            跟随正文，直接移除变量 → 落回 --font-reading: var(--font-ui)
 * - "system:<族>" 系统字体，键即 CSS 族名（与 READING_FONT_SYSTEMS 对齐）
 * - "file:<名>"   用户上传，字节在 <数据目录>/fonts/，经 read_reading_font 取回
 */
import { readReadingFont } from './api'

/** 与 src-tauri/src/models.rs 的 READING_FONT_SYSTEMS 一一对应，改一处必须改两处 */
export const READING_FONT_SYSTEMS = [
  { key: 'SimSun', label: '宋体' },
  { key: 'KaiTi', label: '楷体' },
  { key: 'FangSong', label: '仿宋' },
  { key: 'Microsoft YaHei', label: '微软雅黑' },
] as const

/** 上传字体统一注册到这个族名，切换时整族替换，避免同名文件互相覆盖 */
const UPLOAD_FAMILY = 'Kettd Reading'

let mountedFace: FontFace | null = null

function setReadingStack(stack: string) {
  document.documentElement.style.setProperty('--font-reading', stack)
}

function resetReadingStack() {
  document.documentElement.style.removeProperty('--font-reading')
}

/** base64 → 二进制：FontFace 收 ArrayBuffer/TypedArray，不收 base64 串 */
function base64ToBytes(b64: string): ArrayBuffer {
  const bin = atob(b64)
  const buf = new ArrayBuffer(bin.length)
  const out = new Uint8Array(buf)
  for (let i = 0; i < bin.length; i += 1) out[i] = bin.charCodeAt(i)
  return buf
}

function unmountUploadedFont() {
  if (mountedFace) document.fonts.delete(mountedFace)
  mountedFace = null
}

/** 注册上传字体（先删旧的再挂新的，document.fonts 里不留垃圾） */
export async function mountUploadedFont(name: string): Promise<string> {
  const r = await readReadingFont(name)
  if (r.err || !r.data) throw new Error(r.err ?? '字体文件读取失败')
  const face = new FontFace(UPLOAD_FAMILY, base64ToBytes(r.data))
  await face.load()
  unmountUploadedFont()
  document.fonts.add(face)
  mountedFace = face
  return UPLOAD_FAMILY
}

/**
 * 把设置值落到 CSS 变量。任何失败都退回系统字体——字体坏了不能让正文消失。
 */
export async function applyReadingFont(value: string | null | undefined): Promise<void> {
  const raw = (value ?? '').trim()
  if (!raw) {
    unmountUploadedFont()
    resetReadingStack()
    return
  }
  if (raw.startsWith('system:')) {
    unmountUploadedFont()
    setReadingStack(`"${raw.slice(7)}", var(--font-ui)`)
    return
  }
  if (raw.startsWith('file:')) {
    try {
      const family = await mountUploadedFont(raw.slice(5))
      setReadingStack(`"${family}", var(--font-ui)`)
    } catch (err) {
      console.warn('[reading-font] 上传字体加载失败，已退回正文默认', err)
      unmountUploadedFont()
      resetReadingStack()
    }
  }
}
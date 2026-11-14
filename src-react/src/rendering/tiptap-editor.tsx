import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { useEditor, useEditorState, EditorContent, type UseEditorOptions } from '@tiptap/react'
import { BubbleMenu } from '@tiptap/react/menus'
import StarterKit from '@tiptap/starter-kit'
import Placeholder from '@tiptap/extension-placeholder'
import CodeBlockLowlight from '@tiptap/extension-code-block-lowlight'
import { TableWithAlignment } from './table-alignment'
import { TableRow } from '@tiptap/extension-table-row'
import { TableCell } from '@tiptap/extension-table-cell'
import { TableHeader } from '@tiptap/extension-table-header'
import { TaskList } from '@tiptap/extension-task-list'
import { TaskItem } from '@tiptap/extension-task-item'
import { Markdown } from 'tiptap-markdown'
import { Extension, type ChainedCommands, type Editor } from '@tiptap/core'
import { Plugin, PluginKey } from '@tiptap/pm/state'
import Suggestion from '@tiptap/suggestion'
import { common, createLowlight } from 'lowlight'
import {
  Bold, Italic, Strikethrough, Heading1, Heading2, Heading3, List, ListOrdered,
  ListTodo, TextQuote, Code, Table as TableIcon, Link2, Minus, Pilcrow,
  Search, ChevronUp, ChevronDown, X,
} from 'lucide-react'
import { LinkDialog } from './link-dialog'
import { ImageExtension } from './image-extension'
import { handleImagePaste } from './paste-image'
import {
  FindExtension,
  clearFind as dispatchClearFind,
  getFindState,
  goToFindMatch as dispatchGotoFind,
  scrollFindCurrentIntoView,
  setFindTerm as dispatchFindTerm,
} from './editor-find'

const lowlight = createLowlight(common)

/** 安全提取 tiptap-markdown 的 getMarkdown()（editor.storage 类型不含扩展 storage） */
function getMarkdown(editor: ReturnType<typeof useEditor> | null): string {
  try {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    return ((editor as unknown as { storage: Record<string, any> }).storage.markdown.getMarkdown() as string) ?? ''
  } catch {
    return ''
  }
}

/* ─── slash 命令项 ─────────────────────────────────────────────── */

interface SlashItem {
  id: string
  title: string
  desc: string
  keywords: string
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  icon: any
  /** 在已聚焦的命令链上执行（"/query" 文本由 applySlashItem 先行删除） */
  command: (chain: ChainedCommands) => void
}

/** 工具栏按钮描述（行内格式 / 块级格式共用） */
interface ToolItem {
  icon: typeof Bold
  title: string
  active: boolean
  run: () => void
}

/** 标题下拉：正文（普通段落）+ H1–H4 */
const HEADING_OPTIONS = [
  { value: 'p', label: '正文' },
  { value: '1', label: 'H1' },
  { value: '2', label: 'H2' },
  { value: '3', label: 'H3' },
  { value: '4', label: 'H4' },
] as const

const SLASH_ITEMS: SlashItem[] = [
  { id: 'table', title: '表格', desc: '插入可编辑表格', keywords: 'table biaoge', icon: TableIcon,
    command: (c) => c.insertTable({ rows: 3, cols: 3, withHeaderRow: true }) },
  { id: 'ul', title: '无序列表', desc: '松散要点', keywords: 'list bullet wuxu', icon: List,
    command: (c) => c.toggleBulletList() },
  { id: 'ol', title: '有序列表', desc: '步骤与顺序', keywords: 'ordered youxu', icon: ListOrdered,
    command: (c) => c.toggleOrderedList() },
  { id: 'task', title: '任务清单', desc: '可勾选待办', keywords: 'task todo renwu', icon: ListTodo,
    command: (c) => c.toggleTaskList() },
  { id: 'quote', title: '引用', desc: '摘录与批注', keywords: 'quote blockquote yong', icon: TextQuote,
    command: (c) => c.toggleBlockquote() },
  { id: 'code', title: '代码块', desc: '语法高亮', keywords: 'code daima', icon: Code,
    command: (c) => c.toggleCodeBlock() },
  { id: 'h1', title: '标题 1', desc: '章节大标题', keywords: 'heading h1 biaoti', icon: Heading1,
    command: (c) => c.toggleHeading({ level: 1 }) },
  { id: 'h2', title: '标题 2', desc: '小节标题', keywords: 'heading h2 biaoti', icon: Heading2,
    command: (c) => c.toggleHeading({ level: 2 }) },
  { id: 'h3', title: '标题 3', desc: '子小节标题', keywords: 'heading h3 biaoti', icon: Heading3,
    command: (c) => c.toggleHeading({ level: 3 }) },
  { id: 'paragraph', title: '正文', desc: '普通段落', keywords: 'paragraph text zhengwen wenben', icon: Pilcrow,
    command: (c) => c.setParagraph() },
  { id: 'hr', title: '分割线', desc: '内容分节', keywords: 'hr rule fenge', icon: Minus,
    command: (c) => c.setHorizontalRule() },
]

/**
 * slash 项过滤：allowTables=false（便签窗）时先把「表格」项剔除，
 * / 菜单与 Suggestion 的 items 都不会再出现表格入口（扩展也没装，相关命令会 no-op）。
 */
const filterSlash = (query: string, allowTables = true) =>
  SLASH_ITEMS.filter(
    (it) =>
      (allowTables || it.id !== 'table') &&
      (it.title + it.keywords).toLowerCase().includes(query.toLowerCase()),
  )

/**
 * 执行斜杠命令：先删掉文档里的 "/query" 触发文本，再套用块级命令（同一事务，可一次撤销）。
 * 不删 range 就会把 "/" 留在列表/标题/表格前面（bug："/ 使用后还带一个 /"）。
 */
function applySlashItem(
  editor: Editor,
  range: { from: number; to: number } | null,
  item: SlashItem,
) {
  const chain = editor.chain().focus()
  if (range) chain.deleteRange(range)
  item.command(chain)
  chain.run()
}

/* ─── 派生状态 / 稳定常量 ─────────────────────────────────────── */

/**
 * 工具栏、选区气泡、表格气泡与标题下拉真正依赖的派生状态。
 * 用 useEditorState 精确订阅：只有这些位变化时才重渲染组件，
 * 普通键入（文本变化但格式/块级状态不变）不会触发整组件重渲染。
 */
interface EditorFlags {
  bold: boolean
  italic: boolean
  strike: boolean
  code: boolean
  bulletList: boolean
  orderedList: boolean
  taskList: boolean
  blockquote: boolean
  codeBlock: boolean
  table: boolean
  tableHeader: boolean
  link: boolean
  heading: boolean
  headingLevel: number
}

/** editor 尚不可用（SSR 首帧 / 已销毁）时的兜底值：常量引用保证 selector 结果稳定 */
const EMPTY_EDITOR_FLAGS: EditorFlags = {
  bold: false,
  italic: false,
  strike: false,
  code: false,
  bulletList: false,
  orderedList: false,
  taskList: false,
  blockquote: false,
  codeBlock: false,
  table: false,
  tableHeader: false,
  link: false,
  heading: false,
  headingLevel: 0,
}

/**
 * editorProps 必须是稳定引用：@tiptap/react 的 compareOptions 对除事件回调外的键做 `===` 比较，
 * 每次 render 新建对象会让它恒 false，从而每次 render 都走 editor.setOptions() →
 * view.setProps + view.updateState（ProseMirror 结构性更新，就是「编辑卡顿」的根因之一）。
 */
const EDITOR_PROPS = { attributes: { class: 'kb-editor-content' } }

/**
 * 气泡菜单插件选项：模块级常量。BubbleMenu 的 useEffect 依赖 options / shouldShow 的引用，
 * 每次父组件渲染都换新对象会额外 dispatch 一个 updateOptions 事务。
 */
const BUBBLE_OPTIONS = { placement: 'top' } as const

/** 行内选区气泡：非空选区且不在代码块 / 表格内（模块级函数，引用稳定） */
function shouldShowInlineBubble({
  editor: e,
  state,
}: {
  editor: Editor
  state: { selection: { empty: boolean } }
}) {
  if (state.selection.empty) return false
  return !e.isActive('codeBlock') && !e.isActive('table')
}

/** 表格操作气泡：光标在表格内且编辑器可交互 */
function shouldShowTableBubble({ editor: e }: { editor: Editor }) {
  return e.isFocused && e.isEditable && e.isActive('table')
}

/* ─── 字数上限（便签窗 500 字）────────────────────────────────── */

const CHAR_LIMIT_PLUGIN_KEY = new PluginKey('kbCharLimit')

/**
 * 字数上限插件（Extension 工厂）。
 *
 * 拦截点必须是 ProseMirror 插件的 spec.filterTransaction —— EditorState.applyTransaction
 * 会在应用前逐个调用它，返回 false 即丢弃该事务。注意 prosemirror-view 的 EditorProps
 * 并没有 filterTransaction 字段，写进 editorProps 既不生效也过不了类型检查。
 *
 * Tiptap 对被过滤事务的处理是安全的：Editor.dispatchTransaction 发现 rootTrWasApplied=false
 * 时直接 return（不 emit update），prosemirror-view 再经 docView.markDirty + updateState
 * 把 DOM 回滚到文档状态，因此超限字符不会进入文档（键入 / 粘贴都会被拦）。
 *
 * 只拦「增长」事务：删除 / 撤销 / 等长替换一律放行，超限内容可以靠删减降回上限内，
 * 也避免外部注入的历史超长内容被永久锁死。
 *
 * 字数口径：doc.textContent.length（UTF-16 码元，中文按 1 字计）。与后端便签校验存在
 * 已知宽严差：后端按「存储的 Markdown 字符数」校验，而 Markdown 标记语法（#、**、-、>
 * 等）在此口径里不占字符、存盘后却占，故前端正好 500 字仍可能被后端判超 —— 本插件只负责
 * 编辑期阻挡直观越界，不替代后端校验。
 */
function createCharLimitExtension(
  getMaxChars: () => number | undefined,
  isBypassed: () => boolean,
) {
  return Extension.create({
    name: 'kbCharLimit',
    addProseMirrorPlugins() {
      return [
        new Plugin({
          key: CHAR_LIMIT_PLUGIN_KEY,
          filterTransaction: (tr, state) => {
            const max = getMaxChars()
            if (max === undefined || !Number.isFinite(max)) return true
            if (isBypassed()) return true
            if (!tr.docChanged) return true
            const nextLength = tr.doc.textContent.length
            if (nextLength <= max) return true
            // 已超限：只放行不增加正文长度的事务（删除 / 撤销 / 等长替换）
            return nextLength <= state.doc.textContent.length
          },
        }),
      ]
    },
  })
}

/**
 * 字数计数（独立小组件）：只有它自己随输入重渲染，主编辑器仍走 useEditorState 的 flags
 * 精确订阅，不引入「每键整组件重渲染」。
 * 同时订阅 'update'（正常键入）与 'transaction'（setContent(emitUpdate:false) 这类只发事务、
 * 不发 update 的外部内容替换）；字数没变时不 setState。
 */
function EditorCharCount({ editor, maxChars }: { editor: Editor; maxChars: number }) {
  const [count, setCount] = useState(() => editor.state.doc.textContent.length)

  useEffect(() => {
    const sync = () => {
      const next = editor.state.doc.textContent.length
      setCount((prev) => (prev === next ? prev : next))
    }
    sync()
    editor.on('update', sync)
    editor.on('transaction', sync)
    return () => {
      editor.off('update', sync)
      editor.off('transaction', sync)
    }
  }, [editor])

  const over = count > maxChars
  return (
    <div
      className={'kb-editor-count' + (over ? ' is-over' : '')}
      title={over ? '已超出字数上限，请删减内容' : `正文 ${count}/${maxChars} 字`}
      aria-label={`正文 ${count} / ${maxChars} 字`}
    >
      {count}/{maxChars}
    </div>
  )
}

/* ─── 编辑器主体 ──────────────────────────────────────────────── */

interface Props {
  /** Markdown 初始内容（外部源——KB item bodyMd 或知识便签视口） */
  markdown: string
  /** 编辑防抖回调：传入序列化后的 Markdown 字符串 */
  onSave: (markdown: string) => void
  /** 本次防抖 burst 的第一次键入（保存状态指示：编辑中…） */
  onDirty?: () => void
  /** 编辑器占位文案 */
  placeholder?: string
  /** 可选 CSS class */
  className?: string
  /** 自动保存防抖毫秒（默认 800ms） */
  autoSaveMs?: number
  /**
   * 正文字数上限（便签窗 500）。不传 = 不限制（KB 现状）。
   * 口径：editor.state.doc.textContent.length（UTF-16 码元，中文按 1 字）。
   * 与后端便签校验存在已知宽严差：后端按「存储的 Markdown 字符数」校验，标记语法
   * （#、**、-、> 等）会额外占字符，所以此处放行的边界内容仍可能被后端拒绝。
   */
  maxChars?: number
  /**
   * 是否启用表格能力（默认 true = KB 现状）。false 时：不加载 Table 扩展族、隐藏工具栏
   * 网格选择器与表格气泡、屏蔽 Ctrl/Cmd+T、slash 菜单去掉「表格」项。
   * 期望在挂载期固定（便签窗 false / KB 默认 true）；运行期切换不保证已创建的扩展栈生效。
   */
  allowTables?: boolean
  /**
   * 是否允许图片（默认 false = 便签窗）。true 时（知识库条目）：
   * - 注册 Image 节点扩展（data URL 存正文，与 tiptap-markdown 的 `image` spec 双向序列化）
   * - 粘贴截图 → canvas 压缩（长边 ≤1600、单图 ≤300KB、单条笔记 ≤2.5MB）→ 插入图片节点
   * 期望在挂载期固定（切换会重建 extensions 与编辑器）。
   */
  allowImages?: boolean
}

/**
 * TipTap WYSIWYG Markdown 编辑器（editor-polish-spec：常驻即时渲染画布）。
 * - 表格：Table 扩展族 + 工具栏网格选择器 + Ctrl/Cmd+T（allowTables=false 时整族关闭）
 * - 能力参数：maxChars 字数上限（独立计数条 + filterTransaction 拦截）、allowTables 表格开关、
 *   allowImages 图片开关（Image 节点 + 粘贴截图压缩，仅知识库条目启用）
 * - 提示：slash 命令面板（@tiptap/suggestion）+ 选区气泡（B/I/行内码/删除线）
 * - 工具栏：lucide 图标分组（格式 | 插入），title 带 kbd
 * - 存储契约：bodyMd 唯一真相，tiptap-markdown 双向序列化
 * - 性能：extensions / useEditor options 只创建一次 + useEditorState 精确订阅（键入不整组件重渲染）
 * - 可靠性：800ms 防抖之外，失焦 / 隐藏 / 卸载 / 编辑器销毁时立即 flush（切条不丢字）
 */
export function TipTapEditor({
  markdown,
  onSave,
  onDirty,
  placeholder = '写点什么…（输入 / 唤起插入菜单）',
  className,
  autoSaveMs = 800,
  maxChars,
  allowTables = true,
  allowImages = false,
}: Props) {
  /**
   * 最新 props 的 ref 镜像：useEditor 的 options 只创建一次，
   * 回调与可变 props 必须经 ref 取最新值，否则 options 引用每次 render 都变。
   */
  const onSaveRef = useRef(onSave)
  const onDirtyRef = useRef(onDirty)
  const placeholderRef = useRef(placeholder)
  const autoSaveMsRef = useRef(autoSaveMs)
  onSaveRef.current = onSave
  onDirtyRef.current = onDirty
  placeholderRef.current = placeholder
  autoSaveMsRef.current = autoSaveMs

  /**
   * 能力开关 / 字数上限的 ref 镜像：扩展与插件只创建一次（options 保持稳定引用），
   * 读 ref 才能拿到最新 props（外部内容同步等路径也需要绕过拦截）。
   */
  const maxCharsRef = useRef(maxChars)
  const allowTablesRef = useRef(allowTables)
  /** allowImages 运行期镜像：handlePaste 每次执行时读最新值（schema 层由 extensions 挂载期决定） */
  const allowImagesRef = useRef(allowImages)
  /** 外部 markdown 同步（setContent）期间临时绕过字数拦截，避免外源内容被拦后视图与数据不一致 */
  const bypassCharLimitRef = useRef(false)
  maxCharsRef.current = maxChars
  allowTablesRef.current = allowTables
  allowImagesRef.current = allowImages

  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const lastSavedRef = useRef(markdown)
  const dirtyRef = useRef(false)
  /** 挂载时的初始 Markdown：只在创建 editor 那一帧取，外部变化由下方同步 effect 处理 */
  const initialMarkdownRef = useRef(markdown)
  /** 保存 / flush 时取编辑器实例（useEditor 首帧同步创建） */
  const editorRef = useRef<Editor | null>(null)

  /**
   * 立即把当前文档序列化落盘（800ms 防抖到期与所有 flush 共用）。
   * `force`：editor 正在销毁时（Editor.destroy() 先置 destroyed=true 再 emit('destroy')，
   * 此刻 storage/state 尚未清理）仍允许序列化，保证「销毁不丢字」。
   */
  const saveNow = (force = false) => {
    const ed = editorRef.current
    if (!ed) return
    if (!force && ed.isDestroyed) return
    try {
      // getMarkdown 内部已 try/catch：序列化异常返回 ''
      const md = getMarkdown(ed)
      // 序列化异常得到空串时不要覆盖已有内容（仅当编辑器确实为空才允许写空串）
      if (md === '' && !ed.isEmpty) return
      dirtyRef.current = false
      if (md === lastSavedRef.current) return
      lastSavedRef.current = md
      onSaveRef.current(md)
    } catch (err) {
      // flush 可能发生在失焦 / 隐藏 / 卸载过程中：保存异常绝不能中断这些流程
      console.error('[tiptap-editor] 保存失败', err)
    }
  }

  /** 丢弃防抖 timer（同一份内容不会被定时器重复保存）并立即保存未落盘内容 */
  const flushSave = (force = false) => {
    if (timerRef.current) {
      clearTimeout(timerRef.current)
      timerRef.current = null
    }
    if (!dirtyRef.current) return
    saveNow(force)
  }

  /** 键入后重排 800ms 防抖（语义不变：整篇 Markdown 序列化有成本，合并连续输入） */
  const scheduleSave = () => {
    if (!dirtyRef.current) {
      dirtyRef.current = true
      onDirtyRef.current?.()
    }
    if (timerRef.current) clearTimeout(timerRef.current)
    timerRef.current = setTimeout(() => {
      timerRef.current = null
      saveNow()
    }, autoSaveMsRef.current)
  }

  // ── slash 面板状态（bridge ref 供 Suggestion 回调读写，规避闭包过期）──
  const [slashOpen, setSlashOpen] = useState(false)
  const [slashIndex, setSlashIndex] = useState(0)
  const [slashPos, setSlashPos] = useState<{ left: number; top: number; maxHeight: number } | null>(null)
  /** 过滤结果既进 state（面板渲染）也进 ref（Suggestion 键盘导航同步读取） */
  const [slashFiltered, setSlashFiltered] = useState<SlashItem[]>(SLASH_ITEMS)
  const slashIndexRef = useRef(0)
  const slashFilteredRef = useRef<SlashItem[]>(SLASH_ITEMS)
  /** "/query" 在文档中的范围：选中命令后必须删除，否则会残留斜杠文本 */
  const slashRangeRef = useRef<{ from: number; to: number } | null>(null)
  /** Suggestion 提供的锚点读取器（随文档 / 滚动变化，每次重新取） */
  const slashRectRef = useRef<(() => DOMRect | null) | null>(null)
  const slashPanelRef = useRef<HTMLDivElement | null>(null)
  const wrapperRef = useRef<HTMLDivElement>(null)

  const slashBridge = useRef<{
    onStart?: (p: any) => void
    onUpdate?: (p: any) => void
    onKeyDown?: (p: any) => boolean
    onExit?: () => void
  }>({})

  /**
   * 面板定位：默认贴光标下沿，下方空间不足时翻到上方，并夹取进视口；
   * 超高时给 maxHeight 让面板内部滚动（旧实现只算 bottom+6，页面靠下就被裁掉）。
   */
  const placeSlash = () => {
    const anchor = slashRectRef.current?.() ?? wrapperRef.current?.getBoundingClientRect() ?? null
    if (!anchor) return
    const gap = 6
    const margin = 8
    const vw = window.innerWidth
    const vh = window.innerHeight
    const panel = slashPanelRef.current
    const width = panel?.offsetWidth || 230
    const contentHeight = panel?.offsetHeight || 0
    const spaceBelow = vh - anchor.bottom - gap - margin
    const spaceAbove = anchor.top - gap - margin
    const needed = Math.min(contentHeight || 240, 240)
    const preferAbove = spaceBelow < needed && spaceAbove > spaceBelow
    const maxHeight = Math.max(120, Math.min(320, preferAbove ? spaceAbove : spaceBelow))
    const height = Math.min(contentHeight || maxHeight, maxHeight)
    const top = preferAbove
      ? Math.max(margin, anchor.top - gap - height)
      : Math.min(anchor.bottom + gap, vh - margin - height)
    const left = Math.min(Math.max(margin, anchor.left), Math.max(margin, vw - width - margin))
    setSlashPos((prev) =>
      prev && prev.left === left && prev.top === top && prev.maxHeight === maxHeight
        ? prev
        : { left, top, maxHeight },
    )
  }

  /** 更新过滤结果（state 驱动面板渲染，替代旧的 bumpRender(ref 藏结果) 方案） */
  const applySlashQuery = (query: string) => {
    const next = filterSlash(query, allowTablesRef.current)
    slashFilteredRef.current = next
    setSlashFiltered(next)
  }

  const openSlash = (query: string) => {
    applySlashQuery(query)
    slashIndexRef.current = 0
    setSlashIndex(0)
    setSlashPos(null) // 先隐藏旧位置，等 placeSlash 重新定位，避免闪一下上次的坐标
    setSlashOpen(true)
    requestAnimationFrame(placeSlash)
  }

  const closeSlash = () => {
    setSlashOpen(false)
    setSlashPos(null)
    slashRangeRef.current = null
  }

  // 表格网格选择器
  const [gridOpen, setGridOpen] = useState(false)
  const [gridSize, setGridSize] = useState({ rows: 0, cols: 0 })

  // 居中链接弹窗（预填当前链接；无选中文本也无链接时禁用应用）
  const [linkOpen, setLinkOpen] = useState(false)
  const [linkInitial, setLinkInitial] = useState('')
  const [linkCanApply, setLinkCanApply] = useState(true)

  // slash 扩展：菜单渲染与键盘导航经 bridge 委托给 React 层
  const slashExtension = useMemo(
    () =>
      Extension.create({
        name: 'slashCommand',
        addOptions() {
          return {
            suggestion: {
              char: '/',
              // 读 ref 取最新 allowTables：扩展只创建一次；便签窗（false）里不出现表格项
              items: ({ query }: { query: string }) => filterSlash(query, allowTablesRef.current),
              command: ({
                editor,
                range,
                props: item,
              }: {
                editor: Editor
                range: { from: number; to: number }
                props: SlashItem
              }) => {
                applySlashItem(editor, range, item)
                closeSlash()
              },
              render: () => ({
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                onStart: (props: any) => slashBridge.current.onStart?.(props),
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                onUpdate: (props: any) => slashBridge.current.onUpdate?.(props),
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                onKeyDown: (props: any) => slashBridge.current.onKeyDown?.(props) ?? false,
                onExit: () => slashBridge.current.onExit?.(),
              }),
            },
          }
        },
        addProseMirrorPlugins() {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          return [Suggestion({ editor: this.editor, ...this.options.suggestion } as any)]
        },
      }),
    [],
  )

  /**
   * 字数上限插件只创建一次（上限 / 旁路开关经 ref 读最新值）。
   * KB 不传 maxChars 时过滤器恒定放行，行为与之前完全一致。
   */
  const charLimitExtension = useMemo(
    () => createCharLimitExtension(() => maxCharsRef.current, () => bypassCharLimitRef.current),
    [],
  )

  /**
   * 扩展数组只创建一次（Placeholder 文案经 ref 取最新值）。
   * @tiptap/react 的 compareOptions 对 extensions 逐个做 `===` 比较，
   * 每次 render 重建数组会让比较恒 false → 每次 render 都 setOptions。
   * allowTables=false（便签窗）时整族 Table 扩展不加载（期望挂载期固定）：
   * 命令 / 快捷键 / slash 项 / 工具栏 / 气泡全部按条件关闭，文档里不会出现表格节点；
   * 扩展缺失时 isActive('table') 也会安全返回 false（schema 里找不到类型），不会崩。
   */
  const extensions = useMemo(
    () => [
      StarterKit.configure({
        codeBlock: false, // 用 CodeBlockLowlight 替代
      }),
      CodeBlockLowlight.configure({ lowlight }),
      TaskList,
      TaskItem.configure({ nested: true }),
      // 图片能力可关（#15）：便签窗只需纯文本 + 轻量格式；未加载 image 节点则不会出现图片
      ...(allowImages ? [ImageExtension] : []),
      // 表格能力可关：便签窗只需纯文本 + 轻量格式（未加载则不可能出现表格节点）
      ...(allowTables
        ? [TableWithAlignment.configure({ resizable: false }), TableRow, TableHeader, TableCell]
        : []),
      // 文案随 props 变化但不重建扩展：Placeholder 支持函数形式，渲染时经 ref 取最新值
      Placeholder.configure({ placeholder: () => placeholderRef.current }),
      Markdown.configure({
        html: false,       // 序列化为 Markdown（非 HTML）
        linkify: true,
        breaks: true,
      }),
      slashExtension,
      // 条目内查找（Ctrl+F）：模块级单例扩展，引用稳定，不参与 useMemo 依赖
      FindExtension,
      // 正文字数上限（便签窗 500 字；KB 不传 maxChars → 恒定放行）
      charLimitExtension,
    ],
    [slashExtension, allowTables, allowImages, charLimitExtension],
  )

  /**
   * useEditor 的 options 只创建一次：除事件回调（compareOptions 直接忽略）外的每个键都引用稳定，
   * 后续 render 的比较结果恒 true，不再触发 editor.setOptions() → view.updateState()。
   */
  const editorOptions = useMemo<UseEditorOptions>(
    () => ({
      extensions,
      // 初始内容只在挂载时取一次（tiptap-markdown 接受 Markdown 作为 content）
      content: initialMarkdownRef.current,
      editorProps: {
        ...EDITOR_PROPS,
        // #15 粘贴截图：仅 allowImages（知识库条目）启用；便签默认 false → 直接交回默认粘贴。
        // allowImagesRef 是运行期值，schema 层的 Image 扩展则由上面的 extensions 按挂载期决定。
        handlePaste: (view, event) =>
          handleImagePaste(view, event, { allowImages: allowImagesRef.current }),
      },
      onUpdate: () => scheduleSave(),
      // 失焦 / editor 销毁时立刻落盘（供切条、窗口切换不丢字）
      onBlur: () => flushSave(),
      onDestroy: () => flushSave(true),
    }),
    [extensions],
  )

  const editor = useEditor(editorOptions)
  editorRef.current = editor

  /**
   * 工具栏 / 气泡 / 标题下拉的状态订阅：selector 每个事务执行一次（很便宜），
   * 但组件只在派生状态 deepEqual 变化时才重渲染 —— 普通键入不再整组件重渲染。
   * （旧实现是 editor.on('transaction', bumpRender) 每个事务强制整组件重渲染。）
   */
  const flags = useEditorState({
    editor,
    selector: (snapshot): EditorFlags => {
      const ed: Editor | null = snapshot.editor
      if (!ed || ed.isDestroyed) return EMPTY_EDITOR_FLAGS
      const heading = ed.isActive('heading')
      return {
        bold: ed.isActive('bold'),
        italic: ed.isActive('italic'),
        strike: ed.isActive('strike'),
        code: ed.isActive('code'),
        bulletList: ed.isActive('bulletList'),
        orderedList: ed.isActive('orderedList'),
        taskList: ed.isActive('taskList'),
        blockquote: ed.isActive('blockquote'),
        codeBlock: ed.isActive('codeBlock'),
        // allowTables=false 时 Table 扩展不存在：直接固定 false（不依赖 isActive 的缺失兜底）
        table: allowTables && ed.isActive('table'),
        tableHeader: allowTables && ed.isActive('tableHeader'),
        link: ed.isActive('link'),
        heading,
        headingLevel: heading ? Number(ed.getAttributes('heading').level) : 0,
      }
    },
  })

  // slash 面板回调（Suggestion render 委托至此）
  /** 选中命令：删掉 "/query" 再执行块级命令 */
  const runSlash = (item: SlashItem | undefined) => {
    if (!item) {
      closeSlash()
      return
    }
    applySlashItem(editor, slashRangeRef.current, item)
    closeSlash()
  }

  slashBridge.current.onStart = (props) => {
    slashRangeRef.current = props.range ?? null
    slashRectRef.current = props.clientRect ?? null
    openSlash(props.query ?? '')
  }
  slashBridge.current.onUpdate = (props) => {
    slashRangeRef.current = props.range ?? null
    slashRectRef.current = props.clientRect ?? null
    applySlashQuery(props.query ?? '')
    slashIndexRef.current = 0
    setSlashIndex(0)
    setSlashOpen(true)
    placeSlash()
  }
  slashBridge.current.onKeyDown = ({ event }) => {
    const list = slashFilteredRef.current
    if (!list.length) return false
    // 键盘移动后把高亮项滚进可视区（面板可能因限高而滚动）
    const scrollActiveIntoView = () => {
      requestAnimationFrame(() => {
        slashPanelRef.current
          ?.querySelector<HTMLElement>('.kb-slash-item.on')
          ?.scrollIntoView({ block: 'nearest' })
      })
    }
    if (event.key === 'ArrowDown') {
      slashIndexRef.current = (slashIndexRef.current + 1) % list.length
      setSlashIndex(slashIndexRef.current)
      scrollActiveIntoView()
      return true
    }
    if (event.key === 'ArrowUp') {
      slashIndexRef.current = (slashIndexRef.current - 1 + list.length) % list.length
      setSlashIndex(slashIndexRef.current)
      scrollActiveIntoView()
      return true
    }
    if (event.key === 'Enter') {
      runSlash(list[slashIndexRef.current])
      return true
    }
    if (event.key === 'Escape') {
      closeSlash()
      return true
    }
    return false
  }
  slashBridge.current.onExit = () => closeSlash()

  // 面板挂载 / 内容高度变化后重新定位（避免超出视口）
  useLayoutEffect(() => {
    if (!slashOpen) return
    placeSlash()
    const el = slashPanelRef.current
    if (!el || typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver(() => placeSlash())
    observer.observe(el)
    return () => observer.disconnect()
  }, [slashOpen])

  // 面板打开期间跟随页面滚动 / 窗口缩放
  useEffect(() => {
    if (!slashOpen) return
    const follow = () => placeSlash()
    window.addEventListener('scroll', follow, true)
    window.addEventListener('resize', follow)
    return () => {
      window.removeEventListener('scroll', follow, true)
      window.removeEventListener('resize', follow)
    }
  }, [slashOpen])

  /* ── 条目内查找（Ctrl+F，缺陷 #6）────────────────────────────────── */

  const [findOpen, setFindOpen] = useState(false)
  const [findTerm, setFindTerm] = useState('')
  const [findCount, setFindCount] = useState(0)
  const [findIndex, setFindIndex] = useState(0)
  const findInputRef = useRef<HTMLInputElement | null>(null)
  /** 键盘 / 失焦 / effect 等非渲染路径同步读取开关状态 */
  const findOpenRef = useRef(false)

  /**
   * 关闭查找栏：清插件高亮与计数；restoreFocus 时把焦点还给编辑器。
   * useCallback 让「外部 markdown 变化 → 关闭查找」effect 拿到稳定引用。
   */
  const closeFind = useCallback(
    (restoreFocus: boolean) => {
      if (!editor || !findOpenRef.current) return
      findOpenRef.current = false
      dispatchClearFind(editor)
      setFindOpen(false)
      setFindTerm('')
      setFindCount(0)
      setFindIndex(0)
      if (restoreFocus && !editor.isDestroyed) editor.commands.focus()
    },
    [editor],
  )

  // 打开后聚焦输入框并全选关键词（浏览器查找同款：再按 Ctrl+F 也是全选）
  useEffect(() => {
    if (!findOpen) return
    findInputRef.current?.focus()
    findInputRef.current?.select()
  }, [findOpen])

  /**
   * 查找栏打开期间：文档编辑后刷新计数与高亮。
   * 仅在查找打开时挂 editor.on('update')，关闭立刻 off —— 不恢复「每事务整组件重渲染」。
   */
  useEffect(() => {
    if (!findOpen || !editor) return
    const onEditorUpdate = () => {
      // 只刷新计数 / 当前序号：插件 state.apply 已按 term 重算命中与位置。
      // 这里不主动滚动 —— 用户可能正就地在编辑器里继续输入，滚动会抢走视口。
      const find = getFindState(editor)
      setFindCount(find.matches.length)
      setFindIndex(find.current)
    }
    editor.on('update', onEditorUpdate)
    return () => {
      editor.off('update', onEditorUpdate)
    }
  }, [findOpen, editor])

  // 卸载兜底：编辑器销毁前清掉插件装饰，避免高亮残留
  useEffect(() => {
    return () => {
      if (editor && !editor.isDestroyed) dispatchClearFind(editor)
    }
  }, [editor])

  // 外部 markdown 变化同步（知识便签 viewport：KB 原文变化 → 编辑器更新）
  useEffect(() => {
    if (!editor) return
    const currentMd = getMarkdown(editor)
    // 仅在外部值与编辑器当前值不同时更新（避免光标跳动）
    if (markdown !== undefined && markdown.trim() !== currentMd.trim()) {
      // 切条目 / 外部替换内容：先收起查找栏，避免旧关键词与旧高亮带到新内容
      if (findOpenRef.current) closeFind(false)
      // 外源内容可能超过 maxChars（历史便签 / 与后端口径差异）：临时绕过字数拦截，
      // 否则 setContent 被过滤掉、视图会停在与外源不一致的旧内容上（更不能丢内容）。
      bypassCharLimitRef.current = true
      try {
        editor.commands.setContent(markdown, { emitUpdate: false })
      } finally {
        bypassCharLimitRef.current = false
      }
      lastSavedRef.current = markdown
    }
  }, [editor, markdown, closeFind])

  /**
   * 切条 / 隐藏 / 失焦即时落盘：800ms 防抖未到期就离开时，立即把当前内容序列化保存，避免丢字。
   * - visibilitychange：窗口最小化 / 切到其它应用（WebView2 触发）
   * - blur：焦点离开窗口
   * - cleanup：组件卸载兜底（此处 flush 不调 setState，且 useEditor 的销毁是 1ms 后异步的，
   *   本 effect 声明在 useEditor 之后，cleanup 时 editor 仍然可用）
   */
  useEffect(() => {
    const flushOnHidden = () => {
      if (document.visibilityState === 'hidden') flushSave()
    }
    const flushOnWindowBlur = () => flushSave()
    document.addEventListener('visibilitychange', flushOnHidden)
    window.addEventListener('blur', flushOnWindowBlur)
    return () => {
      document.removeEventListener('visibilitychange', flushOnHidden)
      window.removeEventListener('blur', flushOnWindowBlur)
      flushSave()
    }
    // flushSave 只读写 ref（首个闭包即最新语义），无需重订阅
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  if (!editor) return null

  const chain = () => editor.chain().focus()

  // 当前标题级别（正文 / H1–H4）；H5/H6 仅可能来自外部 Markdown，下拉不覆盖
  const headingLevel = flags.headingLevel
  const headingValue = headingLevel >= 1 && headingLevel <= 4 ? String(headingLevel) : 'p'

  /* ── 居中链接弹窗 ── */
  const openLinkDialog = () => {
    const href = (editor.getAttributes('link').href as string | undefined) ?? ''
    setLinkInitial(href)
    setLinkCanApply(!editor.state.selection.empty || href !== '')
    setLinkOpen(true)
  }

  const closeLinkDialog = () => {
    setLinkOpen(false)
    // 关闭后把焦点与选区还给编辑器
    editor.chain().focus().run()
  }

  /** 返回 false 表示地址不合法（setLink 内置校验失败），弹窗保留并提示 */
  const applyLink = (value: string): boolean => {
    const url = value.trim()
    const cmd = editor.chain().focus()
    if (!url) {
      // 与原 prompt 语义一致：留空确定 = 移除链接
      cmd.extendMarkRange('link').unsetLink().run()
    } else if (!cmd.extendMarkRange('link').setLink({ href: url }).run()) {
      return false
    }
    setLinkOpen(false)
    return true
  }

  const removeLink = () => {
    editor.chain().focus().extendMarkRange('link').unsetLink().run()
    setLinkOpen(false)
  }

  const renderTool = (t: ToolItem) => {
    const Icon = t.icon
    return (
      <button
        key={t.title}
        type="button"
        className={'kb-editor-tool' + (t.active ? ' on' : '')}
        title={t.title}
        aria-label={t.title}
        aria-pressed={t.active}
        onMouseDown={(e) => e.preventDefault()}
        onClick={t.run}
      >
        <Icon size={15} strokeWidth={1.7} />
      </button>
    )
  }

  // 行内格式（H1/H2 按钮已并入「标题」下拉）
  const inlineTools: ToolItem[] = [
    { icon: Bold, title: '加粗 Ctrl+B', active: flags.bold, run: () => chain().toggleBold().run() },
    { icon: Italic, title: '斜体 Ctrl+I', active: flags.italic, run: () => chain().toggleItalic().run() },
    { icon: Strikethrough, title: '删除线', active: flags.strike, run: () => chain().toggleStrike().run() },
  ]

  const blockTools: ToolItem[] = [
    { icon: List, title: '无序列表', active: flags.bulletList, run: () => chain().toggleBulletList().run() },
    { icon: ListOrdered, title: '有序列表', active: flags.orderedList, run: () => chain().toggleOrderedList().run() },
    { icon: ListTodo, title: '任务清单', active: flags.taskList, run: () => chain().toggleTaskList().run() },
    { icon: TextQuote, title: '引用', active: flags.blockquote, run: () => chain().toggleBlockquote().run() },
    { icon: Code, title: '代码块', active: flags.codeBlock, run: () => chain().toggleCodeBlock().run() },
  ]

  const insertGrid = (rows: number, cols: number) => {
    if (rows > 0 && cols > 0) {
      chain().insertTable({ rows, cols, withHeaderRow: true }).run()
    }
    setGridOpen(false)
  }

  const gridCells = () => {
    const cells = []
    for (let r = 1; r <= 4; r++) {
      for (let c = 1; c <= 4; c++) {
        cells.push(
          <button
            key={`${r}-${c}`}
            type="button"
            className={'grid-cell' + (r <= gridSize.rows && c <= gridSize.cols ? ' on' : '')}
            onMouseEnter={() => setGridSize({ rows: r, cols: c })}
            onClick={() => insertGrid(r, c)}
          />,
        )
      }
    }
    return cells
  }

  /* ── 条目内查找（Ctrl+F）：React 侧只镜像插件状态，不缓存匹配位置 ── */

  /** 从插件读回计数（只在查找栏打开 / 打开动作中调用） */
  const syncFindCounters = () => {
    const find = getFindState(editor)
    setFindCount(find.matches.length)
    setFindIndex(find.current)
  }

  /** 关键词变化：插件内整体重算匹配，当前项回到第一处并滚动过去 */
  const searchFind = (value: string) => {
    setFindTerm(value)
    dispatchFindTerm(editor, value)
    syncFindCounters()
    scrollFindCurrentIntoView(editor)
  }

  /** 上一个 / 下一个：循环回绕（无匹配时不动） */
  const stepFind = (delta: number) => {
    const find = getFindState(editor)
    if (find.matches.length === 0) return
    const next = (find.current + delta + find.matches.length) % find.matches.length
    dispatchGotoFind(editor, next)
    setFindIndex(next)
    scrollFindCurrentIntoView(editor)
  }

  /** 打开查找栏：预填编辑器内单行选中文本（≤100 字）；已打开时只聚焦全选 */
  const openFind = () => {
    if (findOpenRef.current) {
      findInputRef.current?.focus()
      findInputRef.current?.select()
      return
    }
    // 与 slash 面板互斥：斜杠命令面板不应与查找栏同屏
    closeSlash()
    let seed = ''
    const sel = editor.state.selection
    if (!sel.empty && sel.$from.sameParent(sel.$to)) {
      seed = editor.state.doc.textBetween(sel.from, sel.to, ' ').trim().slice(0, 100)
    }
    findOpenRef.current = true
    setFindOpen(true)
    setFindTerm(seed)
    dispatchFindTerm(editor, seed)
    syncFindCounters()
    scrollFindCurrentIntoView(editor)
  }

  /** 焦点整体离开编辑器（relatedTarget 不在 wrapper 内）→ 关闭查找、清高亮，不抢焦点 */
  const onWrapperBlur = (e: React.FocusEvent<HTMLDivElement>) => {
    if (!findOpenRef.current) return
    const next = e.relatedTarget
    if (next instanceof Node && e.currentTarget.contains(next)) return
    closeFind(false)
  }

  // Ctrl/Cmd+F 条目内查找；Ctrl/Cmd+T 插入表格（表内 Tab 跳格由 Table 扩展原生支持）
  const onKeyDown = (e: React.KeyboardEvent) => {
    // 中文输入法组合态（isComposing / keyCode 229）不处理快捷键，也不触发查找导航
    if (e.nativeEvent.isComposing || e.nativeEvent.keyCode === 229) return
    // 链接弹窗打开时快捷键交给弹窗，避免 portal 事件冒泡后误插表格 / 误开查找
    if (linkOpen) return

    const mod = e.ctrlKey || e.metaKey
    const key = e.key.toLowerCase()

    // Ctrl/Cmd+F（不带 Shift）：打开条目内查找；已打开则聚焦并全选关键词。
    // 必须排除 Shift —— KbView 用 Ctrl+Shift+F 聚焦知识库搜索框。
    if (mod && !e.shiftKey && !e.altKey && key === 'f') {
      e.preventDefault()
      e.stopPropagation()
      openFind()
      return
    }

    // Esc：关闭查找栏、清除高亮，焦点回到编辑器（ProseMirror 默认不消费 Esc）
    if (findOpenRef.current && e.key === 'Escape') {
      e.preventDefault()
      e.stopPropagation()
      closeFind(true)
      return
    }

    // Enter / Shift+Enter：查找输入框内循环切换命中（输入法组合态已在上面放行）
    if (findOpenRef.current && e.key === 'Enter' && e.target === findInputRef.current) {
      e.preventDefault()
      stepFind(e.shiftKey ? -1 : 1)
      return
    }

    // 普通文本输入框（标题 / 标签 / 搜索框 / 查找框）内不抢键
    const target = e.target as HTMLElement
    if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) return

    // Ctrl/Cmd+T 插入表格：allowTables=false（便签窗）时完全不响应，也不吞按键
    if (allowTables && mod && key === 't') {
      e.preventDefault()
      chain().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run()
    }
  }

  return (
    <div className={`kb-editor ${className ?? ''}`} ref={wrapperRef} onKeyDown={onKeyDown} onBlur={onWrapperBlur}>
      <div className="kb-editor-toolbar" role="toolbar" aria-label="格式工具栏">
        {inlineTools.map(renderTool)}

        {/* 标题级别：显示当前激活级别，选「正文」回普通段落 */}
        <div className={'kb-editor-heading' + (flags.heading ? ' on' : '')}>
          <select
            className="kb-editor-heading-select"
            value={headingValue}
            title="标题级别"
            aria-label="标题级别"
            onChange={(e) => {
              const next = e.target.value
              if (next === 'p') {
                chain().setParagraph().run()
                return
              }
              chain().toggleHeading({ level: Number(next) as 1 | 2 | 3 | 4 }).run()
            }}
          >
            {HEADING_OPTIONS.map((opt) => (
              <option key={opt.value} value={opt.value}>{opt.label}</option>
            ))}
          </select>
        </div>

        {blockTools.map(renderTool)}

        <span className="kb-tool-sep" />

        {/* 表格：网格选择器（allowTables=false 时整块不渲染，便签窗没有任何表格入口） */}
        {allowTables && (
          <div className="kb-editor-gridwrap">
            <button
              type="button"
              className={'kb-editor-tool' + (flags.table ? ' on' : '')}
              title="表格 Ctrl+T"
              aria-label="插入表格"
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => setGridOpen((v) => !v)}
            >
              <TableIcon size={15} strokeWidth={1.7} />
            </button>
            {gridOpen && (
              <div className="kb-grid-picker" onMouseLeave={() => setGridSize({ rows: 0, cols: 0 })}>
                <div className="grid-cells">{gridCells()}</div>
                <div className="grid-hint">{gridSize.rows || '—'} 行 × {gridSize.cols || '—'} 列</div>
              </div>
            )}
          </div>
        )}

        <button
          type="button"
          className={'kb-editor-tool' + (flags.link ? ' on' : '')}
          title="链接 Ctrl+K"
          aria-label="链接"
          onMouseDown={(e) => e.preventDefault()}
          onClick={openLinkDialog}
        >
          <Link2 size={15} strokeWidth={1.7} />
        </button>

        <button
          type="button"
          className="kb-editor-tool"
          title="分割线"
          aria-label="分割线"
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => chain().setHorizontalRule().run()}
        >
          <Minus size={15} strokeWidth={1.7} />
        </button>
      </div>

      {/* 条目内查找栏（Ctrl+F）：sticky 吸顶、右侧对齐，不遮挡工具栏；z-index 低于 LinkDialog */}
      {findOpen && (
        <div className="kb-find" role="search" aria-label="条目内查找">
          <Search size={14} strokeWidth={1.7} className="kb-find-ico" aria-hidden="true" />
          <input
            ref={findInputRef}
            className="kb-find-input"
            type="text"
            value={findTerm}
            placeholder="查找…"
            aria-label="查找关键词"
            spellCheck={false}
            onChange={(e) => searchFind(e.target.value)}
          />
          <span className="kb-find-count" aria-live="polite">
            {findTerm ? (findCount > 0 ? `${findIndex + 1}/${findCount}` : '无结果') : '0/0'}
          </span>
          <button
            type="button"
            className="kb-find-btn"
            title="上一个（Shift+Enter）"
            aria-label="上一个匹配"
            disabled={findCount === 0}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => stepFind(-1)}
          >
            <ChevronUp size={14} strokeWidth={1.8} />
          </button>
          <button
            type="button"
            className="kb-find-btn"
            title="下一个（Enter）"
            aria-label="下一个匹配"
            disabled={findCount === 0}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => stepFind(1)}
          >
            <ChevronDown size={14} strokeWidth={1.8} />
          </button>
          <button
            type="button"
            className="kb-find-btn"
            title="关闭（Esc）"
            aria-label="关闭查找"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => closeFind(true)}
          >
            <X size={14} strokeWidth={1.8} />
          </button>
        </div>
      )}

      {/* 选区浮动工具栏（B/I/行内码/删除线）：shouldShow/options 为模块级稳定引用 */}
      <BubbleMenu
        editor={editor}
        options={BUBBLE_OPTIONS}
        shouldShow={shouldShowInlineBubble}
      >
        <div className="kb-bubble">
          <button type="button" className={flags.bold ? 'on' : ''} onMouseDown={(e) => e.preventDefault()} onClick={() => chain().toggleBold().run()}><Bold size={13} strokeWidth={1.7} /></button>
          <button type="button" className={flags.italic ? 'on' : ''} onMouseDown={(e) => e.preventDefault()} onClick={() => chain().toggleItalic().run()}><Italic size={13} strokeWidth={1.7} /></button>
          <button type="button" className={flags.code ? 'on' : ''} onMouseDown={(e) => e.preventDefault()} onClick={() => chain().toggleCode().run()}><Code size={13} strokeWidth={1.7} /></button>
          <button type="button" className={flags.strike ? 'on' : ''} onMouseDown={(e) => e.preventDefault()} onClick={() => chain().toggleStrike().run()}><Strikethrough size={13} strokeWidth={1.7} /></button>
        </div>
      </BubbleMenu>

      {/* 表格操作气泡：光标在表内时出现（行 / 列 / 表头 / 删表）；allowTables=false 不挂载 */}
      {allowTables && (
        <BubbleMenu
          editor={editor}
          options={BUBBLE_OPTIONS}
          shouldShow={shouldShowTableBubble}
        >
          <div className="kb-bubble kb-bubble-table">
            <button type="button" title="在下方插入行" onMouseDown={(e) => e.preventDefault()} onClick={() => chain().addRowAfter().run()}>+行</button>
            <button
              type="button"
              title={flags.tableHeader ? '表头行不可删除（删除会破坏 Markdown 表格结构）' : '删除当前行'}
              disabled={flags.tableHeader}
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => { if (!flags.tableHeader) chain().deleteRow().run() }}
            >−行</button>
            <span className="kb-bubble-sep" />
            <button type="button" title="在右侧插入列" onMouseDown={(e) => e.preventDefault()} onClick={() => chain().addColumnAfter().run()}>+列</button>
            <button type="button" title="删除当前列" onMouseDown={(e) => e.preventDefault()} onClick={() => chain().deleteColumn().run()}>−列</button>
            <span className="kb-bubble-sep" />
            <button type="button" className="danger" title="删除整张表格" onMouseDown={(e) => e.preventDefault()} onClick={() => chain().deleteTable().run()}>删表</button>
          </div>
        </BubbleMenu>
      )}

      <EditorContent editor={editor} />

      {/* 字数计数（便签窗传 maxChars）：独立订阅的小组件，主编辑器不因计数重渲染 */}
      {maxChars !== undefined && Number.isFinite(maxChars) && (
        <EditorCharCount editor={editor} maxChars={maxChars} />
      )}

      {/* 居中链接弹窗：portal 到 body，知识库与便签窄窗下都可见 */}
      {linkOpen && (
        <LinkDialog
          initialUrl={linkInitial}
          canApply={linkCanApply}
          onConfirm={applyLink}
          onRemove={removeLink}
          onCancel={closeLinkDialog}
        />
      )}

      {/* slash 命令面板：portal 到 body + fixed 定位，避免被编辑器容器裁切 */}
      {slashOpen && createPortal(
        <div
          ref={slashPanelRef}
          className="kb-slash"
          style={{
            position: 'fixed',
            left: slashPos?.left ?? 0,
            top: slashPos?.top ?? 0,
            maxHeight: slashPos?.maxHeight,
            visibility: slashPos ? 'visible' : 'hidden',
          }}
        >
          {slashFiltered.length === 0 && (
            <div className="kb-slash-empty">没有匹配的插入项</div>
          )}
          {slashFiltered.map((it, i) => {
            const Icon = it.icon
            return (
              <div
                key={it.id}
                className={'kb-slash-item' + (i === slashIndex ? ' on' : '')}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => runSlash(it)}
                onMouseEnter={() => { slashIndexRef.current = i; setSlashIndex(i) }}
              >
                <span className="kb-slash-ico"><Icon size={13} strokeWidth={1.7} /></span>
                <span className="kb-slash-tx"><b>{it.title}</b><i>{it.desc}</i></span>
              </div>
            )
          })}
        </div>,
        document.body,
      )}
    </div>
  )
}
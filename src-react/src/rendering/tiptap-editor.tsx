import { useEffect, useMemo, useRef, useState } from 'react'
import { useEditor, EditorContent } from '@tiptap/react'
import { BubbleMenu } from '@tiptap/react/menus'
import StarterKit from '@tiptap/starter-kit'
import Placeholder from '@tiptap/extension-placeholder'
import CodeBlockLowlight from '@tiptap/extension-code-block-lowlight'
import { Table } from '@tiptap/extension-table'
import { TableRow } from '@tiptap/extension-table-row'
import { TableCell } from '@tiptap/extension-table-cell'
import { TableHeader } from '@tiptap/extension-table-header'
import { TaskList } from '@tiptap/extension-task-list'
import { TaskItem } from '@tiptap/extension-task-item'
import { Markdown } from 'tiptap-markdown'
import { Extension } from '@tiptap/core'
import Suggestion from '@tiptap/suggestion'
import { common, createLowlight } from 'lowlight'
import {
  Bold, Italic, Strikethrough, Heading1, Heading2, List, ListOrdered,
  ListTodo, TextQuote, Code, Table as TableIcon, Link2, Minus,
} from 'lucide-react'

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
  command: (editor: NonNullable<ReturnType<typeof useEditor>>) => void
}

const SLASH_ITEMS: SlashItem[] = [
  { id: 'table', title: '表格', desc: '插入可编辑表格', keywords: 'table biaoge', icon: TableIcon,
    command: (e) => e.chain().focus().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run() },
  { id: 'ul', title: '无序列表', desc: '松散要点', keywords: 'list bullet wuxu', icon: List,
    command: (e) => e.chain().focus().toggleBulletList().run() },
  { id: 'ol', title: '有序列表', desc: '步骤与顺序', keywords: 'ordered youxu', icon: ListOrdered,
    command: (e) => e.chain().focus().toggleOrderedList().run() },
  { id: 'task', title: '任务清单', desc: '可勾选待办', keywords: 'task todo renwu', icon: ListTodo,
    command: (e) => e.chain().focus().toggleTaskList().run() },
  { id: 'quote', title: '引用', desc: '摘录与批注', keywords: 'quote blockquote yong', icon: TextQuote,
    command: (e) => e.chain().focus().toggleBlockquote().run() },
  { id: 'code', title: '代码块', desc: '语法高亮', keywords: 'code daima', icon: Code,
    command: (e) => e.chain().focus().toggleCodeBlock().run() },
  { id: 'h1', title: '标题 1', desc: '章节大标题', keywords: 'heading h1 biaoti', icon: Heading1,
    command: (e) => e.chain().focus().toggleHeading({ level: 1 }).run() },
  { id: 'h2', title: '标题 2', desc: '小节标题', keywords: 'heading h2 biaoti', icon: Heading2,
    command: (e) => e.chain().focus().toggleHeading({ level: 2 }).run() },
  { id: 'hr', title: '分割线', desc: '内容分节', keywords: 'hr rule fenge', icon: Minus,
    command: (e) => e.chain().focus().setHorizontalRule().run() },
]

const filterSlash = (query: string) =>
  SLASH_ITEMS.filter((it) => (it.title + it.keywords).toLowerCase().includes(query.toLowerCase()))

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
}

/**
 * TipTap WYSIWYG Markdown 编辑器（editor-polish-spec：常驻即时渲染画布）。
 * - 表格：Table 扩展族 + 工具栏网格选择器 + Ctrl/Cmd+T
 * - 提示：slash 命令面板（@tiptap/suggestion）+ 选区气泡（B/I/行内码/删除线）
 * - 工具栏：lucide 图标分组（格式 | 插入），title 带 kbd
 * - 存储契约：bodyMd 唯一真相，tiptap-markdown 双向序列化
 */
export function TipTapEditor({
  markdown,
  onSave,
  onDirty,
  placeholder = '写点什么…（输入 / 唤起插入菜单）',
  className,
  autoSaveMs = 800,
}: Props) {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const lastSavedRef = useRef(markdown)
  const dirtyRef = useRef(false)
  const [, bumpRender] = useState(0)

  // ── slash 面板状态（bridge ref 供 Suggestion 回调读写，规避闭包过期）──
  const [slashOpen, setSlashOpen] = useState(false)
  const [slashIndex, setSlashIndex] = useState(0)
  const [slashPos, setSlashPos] = useState<{ left: number; top: number } | null>(null)
  const slashIndexRef = useRef(0)
  const slashQueryRef = useRef('')
  const slashFilteredRef = useRef<SlashItem[]>(SLASH_ITEMS)
  const wrapperRef = useRef<HTMLDivElement>(null)

  const slashBridge = useRef<{
    onStart?: (p: any) => void
    onUpdate?: (p: any) => void
    onKeyDown?: (p: any) => boolean
    onExit?: () => void
  }>({})

  const openSlash = (query: string, pos: { left: number; top: number } | null) => {
    slashQueryRef.current = query
    slashFilteredRef.current = filterSlash(query)
    slashIndexRef.current = 0
    setSlashIndex(0)
    setSlashPos(pos)
    setSlashOpen(true)
  }
  const closeSlash = () => setSlashOpen(false)

  // 表格网格选择器
  const [gridOpen, setGridOpen] = useState(false)
  const [gridSize, setGridSize] = useState({ rows: 0, cols: 0 })

  // slash 扩展：菜单渲染与键盘导航经 bridge 委托给 React 层
  const slashExtension = useMemo(
    () =>
      Extension.create({
        name: 'slashCommand',
        addOptions() {
          return {
            suggestion: {
              char: '/',
              items: ({ query }: { query: string }) => filterSlash(query),
              command: ({
                editor,
                item,
              }: {
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                editor: any
                item: SlashItem
              }) => {
                item.command(editor)
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

  const editor = useEditor({
    extensions: [
      StarterKit.configure({
        codeBlock: false, // 用 CodeBlockLowlight 替代
      }),
      CodeBlockLowlight.configure({ lowlight }),
      TaskList,
      TaskItem.configure({ nested: true }),
      Table.configure({ resizable: false }),
      TableRow,
      TableHeader,
      TableCell,
      Placeholder.configure({ placeholder }),
      Markdown.configure({
        html: false,       // 序列化为 Markdown（非 HTML）
        linkify: true,
        breaks: true,
      }),
      slashExtension,
    ],
    content: markdown,     // tiptap-markdown 直接接受 Markdown 作为 content
    editorProps: {
      attributes: { class: 'kb-editor-content' },
    },
    onUpdate: () => {
      if (!dirtyRef.current) {
        dirtyRef.current = true
        onDirty?.()
      }
      if (timerRef.current) clearTimeout(timerRef.current)
      timerRef.current = setTimeout(() => {
        const md = getMarkdown(editor)
        if (md !== lastSavedRef.current) {
          lastSavedRef.current = md
          onSave(md)
          dirtyRef.current = false
        }
      }, autoSaveMs)
    },
  })

  // slash 面板回调（Suggestion render 委托至此）
  slashBridge.current.onStart = (props) => {
    const rect = props.clientRect?.()
    openSlash(props.query ?? '', rect ? { left: rect.left, top: rect.bottom + 6 } : null)
  }
  slashBridge.current.onUpdate = (props) => {
    const rect = props.clientRect?.()
    slashQueryRef.current = props.query ?? ''
    slashFilteredRef.current = filterSlash(props.query ?? '')
    slashIndexRef.current = 0
    setSlashIndex(0)
    setSlashPos(rect ? { left: rect.left, top: rect.bottom + 6 } : null)
    setSlashOpen(true)
  }
  slashBridge.current.onKeyDown = ({ event }) => {
    const list = slashFilteredRef.current
    if (!list.length) return false
    if (event.key === 'ArrowDown') {
      slashIndexRef.current = (slashIndexRef.current + 1) % list.length
      setSlashIndex(slashIndexRef.current)
      return true
    }
    if (event.key === 'ArrowUp') {
      slashIndexRef.current = (slashIndexRef.current - 1 + list.length) % list.length
      setSlashIndex(slashIndexRef.current)
      return true
    }
    if (event.key === 'Enter') {
      list[slashIndexRef.current]?.command(editor)
      closeSlash()
      return true
    }
    if (event.key === 'Escape') {
      closeSlash()
      return true
    }
    return false
  }
  slashBridge.current.onExit = () => closeSlash()

  // 外部 markdown 变化同步（知识便签 viewport：KB 原文变化 → 编辑器更新）
  useEffect(() => {
    if (!editor) return
    const currentMd = getMarkdown(editor)
    // 仅在外部值与编辑器当前值不同时更新（避免光标跳动）
    if (markdown !== undefined && markdown.trim() !== currentMd.trim()) {
      editor.commands.setContent(markdown, { emitUpdate: false })
      lastSavedRef.current = markdown
    }
  }, [editor, markdown])

  // 工具栏按钮随选区状态高亮：事务后强制重渲染
  useEffect(() => {
    if (!editor) return
    const rerender = () => bumpRender((k) => k + 1)
    editor.on('transaction', rerender)
    return () => { editor.off('transaction', rerender) }
  }, [editor])

  // 清理防抖 timer
  useEffect(() => {
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current)
    }
  }, [])

  if (!editor) return null

  const chain = () => editor.chain().focus()
  const tools: Array<{ icon: typeof Bold; title: string; active: boolean; run: () => void }> = [
    { icon: Bold, title: '加粗 Ctrl+B', active: editor.isActive('bold'), run: () => chain().toggleBold().run() },
    { icon: Italic, title: '斜体 Ctrl+I', active: editor.isActive('italic'), run: () => chain().toggleItalic().run() },
    { icon: Strikethrough, title: '删除线', active: editor.isActive('strike'), run: () => chain().toggleStrike().run() },
    { icon: Heading1, title: '标题 1', active: editor.isActive('heading', { level: 1 }), run: () => chain().toggleHeading({ level: 1 }).run() },
    { icon: Heading2, title: '标题 2', active: editor.isActive('heading', { level: 2 }), run: () => chain().toggleHeading({ level: 2 }).run() },
    { icon: List, title: '无序列表', active: editor.isActive('bulletList'), run: () => chain().toggleBulletList().run() },
    { icon: ListOrdered, title: '有序列表', active: editor.isActive('orderedList'), run: () => chain().toggleOrderedList().run() },
    { icon: ListTodo, title: '任务清单', active: editor.isActive('taskList'), run: () => chain().toggleTaskList().run() },
    { icon: TextQuote, title: '引用', active: editor.isActive('blockquote'), run: () => chain().toggleBlockquote().run() },
    { icon: Code, title: '代码块', active: editor.isActive('codeBlock'), run: () => chain().toggleCodeBlock().run() },
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

  // Ctrl/Cmd+T 插入表格（表内 Tab 跳格由 Table 扩展原生支持）
  const onKeyDown = (e: React.KeyboardEvent) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 't') {
      e.preventDefault()
      chain().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run()
    }
  }

  return (
    <div className={`kb-editor ${className ?? ''}`} ref={wrapperRef} onKeyDown={onKeyDown}>
      <div className="kb-editor-toolbar" role="toolbar" aria-label="格式工具栏">
        {tools.map((t) => {
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
        })}

        <span className="kb-tool-sep" />

        {/* 表格：网格选择器 */}
        <div className="kb-editor-gridwrap">
          <button
            type="button"
            className={'kb-editor-tool' + (editor.isActive('table') ? ' on' : '')}
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

        <button
          type="button"
          className={'kb-editor-tool' + (editor.isActive('link') ? ' on' : '')}
          title="链接 Ctrl+K"
          aria-label="链接"
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => {
            const url = window.prompt('链接地址（留空取消链接）')
            if (url === null) return
            if (url === '') { chain().unsetLink().run(); return }
            chain().setLink({ href: url }).run()
          }}
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

      {/* 选区浮动工具栏（B/I/行内码/删除线） */}
      <BubbleMenu
        options={{ placement: 'top' }}
        shouldShow={({ editor: e, state }: { editor: typeof editor; state: any }) => {
          const { empty } = state.selection
          if (empty) return false
          return !e.isActive('codeBlock') && !e.isActive('table')
        }}
      >
        <div className="kb-bubble">
          <button type="button" className={editor.isActive('bold') ? 'on' : ''} onMouseDown={(e) => e.preventDefault()} onClick={() => chain().toggleBold().run()}><Bold size={13} strokeWidth={1.7} /></button>
          <button type="button" className={editor.isActive('italic') ? 'on' : ''} onMouseDown={(e) => e.preventDefault()} onClick={() => chain().toggleItalic().run()}><Italic size={13} strokeWidth={1.7} /></button>
          <button type="button" className={editor.isActive('code') ? 'on' : ''} onMouseDown={(e) => e.preventDefault()} onClick={() => chain().toggleCode().run()}><Code size={13} strokeWidth={1.7} /></button>
          <button type="button" className={editor.isActive('strike') ? 'on' : ''} onMouseDown={(e) => e.preventDefault()} onClick={() => chain().toggleStrike().run()}><Strikethrough size={13} strokeWidth={1.7} /></button>
        </div>
      </BubbleMenu>

      <EditorContent editor={editor} />

      {/* slash 命令面板 */}
      {slashOpen && (
        <div className="kb-slash" style={slashPos ? { position: 'fixed', left: slashPos.left, top: slashPos.top } : undefined}>
          {slashFilteredRef.current.length === 0 && (
            <div className="kb-slash-empty">没有匹配的插入项</div>
          )}
          {slashFilteredRef.current.map((it, i) => {
            const Icon = it.icon
            return (
              <div
                key={it.id}
                className={'kb-slash-item' + (i === slashIndex ? ' on' : '')}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => { it.command(editor); closeSlash() }}
                onMouseEnter={() => { slashIndexRef.current = i; setSlashIndex(i) }}
              >
                <span className="kb-slash-ico"><Icon size={13} strokeWidth={1.7} /></span>
                <span className="kb-slash-tx"><b>{it.title}</b><i>{it.desc}</i></span>
              </div>
            )
          })}
        </div>
      )}
    </div>
  )
}

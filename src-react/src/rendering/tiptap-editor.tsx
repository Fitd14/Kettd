import { useEffect, useRef, useState } from 'react'
import { useEditor, EditorContent } from '@tiptap/react'
import StarterKit from '@tiptap/starter-kit'
import Placeholder from '@tiptap/extension-placeholder'
import CodeBlockLowlight from '@tiptap/extension-code-block-lowlight'
import { common, createLowlight } from 'lowlight'
import { Markdown } from 'tiptap-markdown'

const lowlight = createLowlight(common)

/** 安全提取 tiptap-markdown 的 getMarkdown()（editor.storage 类型不含扩展 storage） */
function getMarkdown(editor: ReturnType<typeof useEditor>): string {
  try {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    return (editor!.storage as any).markdown.getMarkdown() as string
  } catch {
    return ''
  }
}

interface Props {
  /** Markdown 初始内容（外部源——KB item bodyMd 或自由便签 content） */
  markdown: string
  /** 编辑防抖回调：传入序列化后的 Markdown 字符串 */
  onSave: (markdown: string) => void
  /** 编辑器占位文案 */
  placeholder?: string
  /** 可选 CSS class */
  className?: string
  /** 自动保存防抖毫秒（默认 800ms） */
  autoSaveMs?: number
}

/**
 * TipTap WYSIWYG Markdown 编辑器。
 *
 * - 输入规则：`# ` 标题、`- ` 列表、```` ``` ```` 代码块等照打即成型
 * - 存储格式：bodyMd（Markdown 字符串）为唯一真相；ProseMirror↔Markdown 双向由 tiptap-markdown 处理
 * - 自动保存：输入防抖后调用 onSave（debounce ms 可配）
 * - 外部同步：当 markdown prop 从外部变化时更新编辑器（知识便签 viewport 模式）
 *
 * 三面复用锚点：主窗 KB 视图编辑态 / 知识便签窄窗 / （自由便签渲染开关态复用同一内核）
 */
export function TipTapEditor({
  markdown,
  onSave,
  placeholder = '开始写...',
  className,
  autoSaveMs = 800,
}: Props) {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const lastSavedRef = useRef(markdown)

  const editor = useEditor({
    extensions: [
      StarterKit.configure({
        codeBlock: false, // 用 CodeBlockLowlight 替代
      }),
      CodeBlockLowlight.configure({ lowlight }),
      Placeholder.configure({ placeholder }),
      Markdown.configure({
        html: false,       // 序列化为 Markdown（非 HTML）
        linkify: true,
        breaks: true,
      }),
    ],
    content: markdown,     // tiptap-markdown 直接接受 Markdown 作为 content
    editorProps: {
      attributes: {
        class: 'kb-editor-content',
      },
    },
    onUpdate: () => {
      if (timerRef.current) clearTimeout(timerRef.current)
      timerRef.current = setTimeout(() => {
        const md = getMarkdown(editor)
        if (md !== lastSavedRef.current) {
          lastSavedRef.current = md
          onSave(md)
        }
      }, autoSaveMs)
    },
  })

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
  const [, bumpRender] = useState(0)
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
  const tools: Array<{ label: string; title: string; active: boolean; run: () => void }> = [
    { label: 'B', title: '加粗', active: editor.isActive('bold'), run: () => chain().toggleBold().run() },
    { label: 'I', title: '斜体', active: editor.isActive('italic'), run: () => chain().toggleItalic().run() },
    { label: 'H1', title: '一级标题', active: editor.isActive('heading', { level: 1 }), run: () => chain().toggleHeading({ level: 1 }).run() },
    { label: 'H2', title: '二级标题', active: editor.isActive('heading', { level: 2 }), run: () => chain().toggleHeading({ level: 2 }).run() },
    { label: '•', title: '无序列表', active: editor.isActive('bulletList'), run: () => chain().toggleBulletList().run() },
    { label: '1.', title: '有序列表', active: editor.isActive('orderedList'), run: () => chain().toggleOrderedList().run() },
    { label: '❝', title: '引用', active: editor.isActive('blockquote'), run: () => chain().toggleBlockquote().run() },
    { label: '</>', title: '代码块', active: editor.isActive('codeBlock'), run: () => chain().toggleCodeBlock().run() },
    { label: '✕', title: '清除格式', active: false, run: () => chain().unsetAllMarks().clearNodes().run() },
  ]

  return (
    <div className={`kb-editor ${className ?? ''}`}>
      <div className="kb-editor-toolbar" role="toolbar" aria-label="格式工具栏">
        {tools.map((t) => (
          <button
            key={t.label}
            type="button"
            className={`kb-editor-tool${t.active ? ' on' : ''}`}
            title={t.title}
            aria-label={t.title}
            aria-pressed={t.active}
            onMouseDown={(e) => e.preventDefault()} /* 防止抢走编辑器选区 */
            onClick={t.run}
          >
            {t.label}
          </button>
        ))}
      </div>
      <EditorContent editor={editor} />
    </div>
  )
}

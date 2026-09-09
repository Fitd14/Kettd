import { useMemo } from 'react'
import MarkdownIt from 'markdown-it'
import DOMPurify from 'dompurify'
import { getRendererForLang, highlightCode } from './renderer-registry'

interface Props {
  /** Markdown 源文本 */
  markdown: string
  /** 可选 CSS class */
  className?: string
}

const md = new MarkdownIt({
  html: false,    // 禁止原始 HTML（安全）
  linkify: true,  // 自动链接 URL
  breaks: true,   // 单换行→<br>
})

// 覆盖 fenced renderer：优先查 RendererRegistry，否则 highlight.js
md.renderer.rules.fence = (tokens, idx) => {
  const token = tokens[idx]
  const info = token.info.trim().split(/\s+/)[0]
  const code = token.content

  // 1. 尝试注册表扩展（mermaid / chart 等——后续版本注册）
  const extRenderer = getRendererForLang(info)
  if (extRenderer) {
    const html = extRenderer(code)
    return `<pre class="kb-code-block kb-code-ext" data-lang="${info}">${html}</pre>`
  }

  // 2. 回落：highlight.js 语法高亮
  const highlighted = highlightCode(code, info || undefined)
  return `<pre class="kb-code-block hljs" data-lang="${info || 'auto'}"><code>${highlighted}</code></pre>`
}

// 表格：给 th/td 加 class 以便 CSS 美化
const defaultTableOpen = md.renderer.rules.table_open!
md.renderer.rules.table_open = (tokens, idx, options, env, self) => {
  return '<table class="kb-table">\n'
}

/**
 * 阅读态 Markdown→React 渲染器。
 * - markdown-it 转 HTML
 * - DOMPurify 消毒（防 XSS；允许 data-lang 属性给代码块使用）
 * - 渲染内核复用：主窗 KB 视图 + 自由便签预览 + 知识便签阅读态
 */
export function MdStaticRenderer({ markdown, className }: Props) {
  const html = useMemo(() => {
    if (!markdown) return ''
    const raw = md.render(markdown)
    return DOMPurify.sanitize(raw, {
      ADD_ATTR: ['data-lang', 'class'],
    })
  }, [markdown])

  return (
    <div
      className={`kb-md-render ${className ?? ''}`}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  )
}

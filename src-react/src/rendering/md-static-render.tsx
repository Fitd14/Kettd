import { useMemo } from 'react'
import DOMPurify from 'dompurify'
import { md } from './markdown'

interface Props {
  /** Markdown 源文本 */
  markdown: string
  /** 可选 CSS class */
  className?: string
}

/**
 * 阅读态 Markdown→React 渲染器。
 * - markdown-it 转 HTML（实例与 renderer 覆盖在 `./markdown.ts`，与导出共用）
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
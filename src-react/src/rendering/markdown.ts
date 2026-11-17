/**
 * 共享 MarkdownIt 实例（单源）。
 *
 * 阅读视图（`md-static-render.tsx`）与导出块模型（`lib/docx-blocks.ts`）必须用同一份配置：
 * `html:false` / `breaks:true` 这类开关一旦两处各写一份就会漂移，导出结构与阅读视图就会对不上。
 * 因此实例只在这里创建一次，renderer 覆盖也只在这里做。
 */
import MarkdownIt from 'markdown-it'
import { getRendererForLang, highlightCode } from './renderer-registry'
import { resolveAssetUrl } from './asset-url'

export const md = new MarkdownIt({
  html: false, // 禁止原始 HTML（安全）
  linkify: true, // 自动链接 URL
  breaks: true, // 单换行→<br>
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

// 表格：统一 class（kb-table）以便 CSS 美化（见 prosemirror.css）
md.renderer.rules.table_open = () => '<table class="kb-table">\n'

// 图片（D2）：正文只存 assets/<sha>.<ext> 相对引用，渲染时解析成 convertFileSrc 地址；
// alt 沿用 markdown-it 默认口径（renderInlineAsText），data URL / 远程图原样不动。
md.renderer.rules.image = (tokens, idx, options, env, self) => {
  const token = tokens[idx]
  const src = token.attrGet('src')
  if (typeof src === 'string' && src) token.attrSet('src', resolveAssetUrl(src))
  token.attrSet('alt', self.renderInlineAsText(token.children ?? [], options, env))
  return self.renderToken(tokens, idx, options)
}
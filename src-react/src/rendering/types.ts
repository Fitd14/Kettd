/**
 * RendererRegistry 插件接口。
 * 每个扩展对应一种 fenced code block 的语言标记（如 'mermaid' / 'chart'）。
 * v1 落地核心渲染 + 代码高亮；mermaid 等重扩展后续版本注册。
 */
export interface MdRenderExtension {
  /** 唯一标识（如 'mermaid' / 'code-highlight'） */
  id: string
  /** 支持的语言标记列表（fenced code block 的 info string） */
  languages: string[]
  /** 阅读态静态渲染：接收 fenced block 代码文本，返回 HTML 字符串 */
  staticRender: (code: string) => string
  /**
   * WYSIWYG 编辑期节点视图（ProseMirror NodeViewSpec）。
   * v1 不实现；mermaid 扩展落地时补齐——编辑期内联渲染图、聚焦回落源码。
   * null 表示使用默认代码块样式。
   */
  editNodeView?: null
}

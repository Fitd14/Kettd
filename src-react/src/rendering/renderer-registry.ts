import type { MdRenderExtension } from './types'

const registry = new Map<string, MdRenderExtension>()

/** 注册渲染扩展（扩展的 languages 列表每个 lang 都可命中） */
export function registerMdExtension(ext: MdRenderExtension) {
  for (const lang of ext.languages) registry.set(lang, ext)
}

/** 根据 fenced code block 的语言标记查找静态渲染器；未注册返回 null */
export function getRendererForLang(lang: string): ((code: string) => string) | null {
  return registry.get(lang)?.staticRender ?? null
}

// ─── Core: highlight.js 语言注册 ────────────────────────────────────────────
import hljs from 'highlight.js/lib/core'
import javascript from 'highlight.js/lib/languages/javascript'
import rust from 'highlight.js/lib/languages/rust'
import typescript from 'highlight.js/lib/languages/typescript'
import python from 'highlight.js/lib/languages/python'
import bash from 'highlight.js/lib/languages/bash'
import json from 'highlight.js/lib/languages/json'
import css from 'highlight.js/lib/languages/css'
import xml from 'highlight.js/lib/languages/xml'
import markdown from 'highlight.js/lib/languages/markdown'
import yaml from 'highlight.js/lib/languages/yaml'

hljs.registerLanguage('javascript', javascript)
hljs.registerLanguage('js', javascript)
hljs.registerLanguage('rust', rust)
hljs.registerLanguage('rs', rust)
hljs.registerLanguage('typescript', typescript)
hljs.registerLanguage('ts', typescript)
hljs.registerLanguage('python', python)
hljs.registerLanguage('py', python)
hljs.registerLanguage('bash', bash)
hljs.registerLanguage('sh', bash)
hljs.registerLanguage('json', json)
hljs.registerLanguage('css', css)
hljs.registerLanguage('xml', xml)
hljs.registerLanguage('html', xml)
hljs.registerLanguage('markdown', markdown)
hljs.registerLanguage('md', markdown)
hljs.registerLanguage('yaml', yaml)

/**
 * highlight.js 代码高亮：优先用指定语言，否则自动检测。
 * 返回 HTML 字符串（已转义，可直接插入 DOM）。
 */
export function highlightCode(code: string, lang?: string): string {
  const normalised = lang?.toLowerCase().trim()
  if (normalised && hljs.getLanguage(normalised)) {
    return hljs.highlight(code, { language: normalised }).value
  }
  return hljs.highlightAuto(code).value
}

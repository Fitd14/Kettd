/**
 * 外观主题统一入口（kb-theme-library 5.2 / 5.3）。
 *
 * 主题变量只写一次，写到 `document.documentElement` 上（design.md D2 / D9）：外壳、侧栏、
 * 设置页、知识库阅读区、portal 出去的弹层都在这棵子树里，一处写入全体换装。
 * 主窗与三个独立窗口（快速捕获 / 便签 / 待办悬浮）各自挂载一次（D10）。
 */
import { readReadingTheme } from '@/lib/api'
import { applyTheme, clearTheme, parseTheme } from '@/lib/reading-theme'
import paperglowCss from '@/assets/themes/paperglow.css?raw'
import mdmdtCss from '@/assets/themes/mdmdt.css?raw'

/** 内置主题源码（id 与后端 `READING_THEME_BUILTINS` 对齐，一致性测试把关）。 */
export const BUILTIN_THEME_CSS: Record<string, string> = { paperglow: paperglowCss, mdmdt: mdmdtCss }

export type ParsedTheme = ReturnType<typeof parseTheme>

/** 解析结果缓存：同一次运行里换明暗不必重读文件；导入/删除主题后由 `clearThemeCache` 作废。 */
const cache = new Map<string, ParsedTheme>()

export function clearThemeCache(): void {
  cache.clear()
}

/** 主题设置值 → 解析结果；内置走打包资源、`file:` 走后端读取，读不到返回 null（回默认外观）。 */
export async function resolveReadingTheme(value: string): Promise<ParsedTheme | null> {
  if (!value) return null
  const cached = cache.get(value)
  if (cached) return cached
  let css = ''
  if (value.startsWith('builtin:')) {
    css = BUILTIN_THEME_CSS[value.slice('builtin:'.length)] ?? ''
  } else if (value.startsWith('file:')) {
    const result = await readReadingTheme(value.slice('file:'.length))
    if (result.err || !result.data) return null
    css = result.data
  } else {
    return null
  }
  if (!css) return null
  const parsed = parseTheme(css)
  if (!parsed.ok) return null
  cache.set(value, parsed)
  return parsed
}

/**
 * 把设置里的主题应用到当前窗口根节点。
 * 幂等：store-changed / reading-theme-changed / 明暗切换后都可以直接重放。
 */
export async function applyReadingTheme(settings: {
  readingTheme?: string
  readingFont?: string
}): Promise<void> {
  const root = document.documentElement
  const dark = root.classList.contains('dark')
  const theme = await resolveReadingTheme(settings.readingTheme ?? '')
  applyTheme(root, theme, { dark, userFont: settings.readingFont ?? '' })
}

/** 卸载（窗口销毁 / 主题清空）时只摘自己写过的变量，其余内联样式不碰。 */
export function clearReadingTheme(): void {
  clearTheme(document.documentElement)
}
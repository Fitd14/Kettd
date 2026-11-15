import { plainText } from './sticky-text.ts'

/**
 * 便签搜索匹配（OpenSpec sticky-shell-polish D1 / tasks 2.6）。
 * 命中范围 = 正文纯文本 + 知识来源标题；大小写不敏感；空白查询视为不过滤（全命中）。
 * kbTitle 传 null/undefined（无来源或条目已删除）时只搜正文。
 */
export function stickyMatches(
  query: string,
  content: string | null | undefined,
  kbTitle?: string | null,
): boolean {
  const needle = typeof query === 'string' ? query.trim().toLowerCase() : ''
  if (!needle) return true
  if (plainText(content).toLowerCase().includes(needle)) return true
  const title = typeof kbTitle === 'string' ? kbTitle.trim().toLowerCase() : ''
  return title.length > 0 && title.includes(needle)
}
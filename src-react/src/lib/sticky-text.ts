/**
 * 便签纯文本工具（OpenSpec sticky-shell-polish D1 / tasks 2.2）。
 * 卡片墙与设置页共用：剥掉 Markdown 标记、去空行，按字符截断。
 * 纯字符串处理，不碰时间与数据格式；入参防御 null/undefined（content 缺字段黑屏教训）。
 */

/** 默认摘要长度：120 字符（可按需传入更短值，例如缩小题条） */
export const DEFAULT_PREVIEW_MAX = 120

const IMAGE_RE = /!\[([^\]]*)\]\([^)]*\)/g
const LINK_RE = /\[([^\]]*)\]\([^)]*\)/g
const CODE_RE = /`{1,3}([^`]*)`{1,3}/g
const STRONG_RE = /(\*\*|__|~~)(.*?)\1/g
const WEAK_EMPHASIS_RE = /(\*|_)([^*_]+)\1/g
const HTML_TAG_RE = /<[^>]+>/g

/** 单行剥离：块级标记（标题/引用/列表/围栏/分隔线/表格线）+ 行内标记 */
function stripLine(line: string): string {
  return line
    .replace(/^\s{0,3}#{1,6}(?:\s+|$)/, '') // 标题 #
    .replace(/^\s{0,3}>\s?/, '') // 引用 >
    .replace(/^\s*(?:[-*+]|\d+[.)])\s+(?:\[[ xX]\]\s+)?/, '') // 列表符号 / 任务复选框
    .replace(/^\s*(?:```|~~~)[\w-]*\s*$/, '') // 代码围栏行
    .replace(/^\s*(?:-{3,}|\*{3,}|_{3,})\s*$/, '') // 分隔线
    .replace(/^\s*\|?[\s:|-]*-[\s:|-]*\|?\s*$/, '') // 表格分隔行
    .replace(IMAGE_RE, '$1') // 图片 → alt
    .replace(LINK_RE, '$1') // 链接 → 文字
    .replace(CODE_RE, '$1') // 行内码
    .replace(STRONG_RE, '$2') // 粗体 / 删除线
    .replace(WEAK_EMPHASIS_RE, '$2') // 斜体
    .replace(HTML_TAG_RE, '') // 内联 HTML
    .replace(/\s*\|\s*/g, ' ') // 表格列分隔
    .replace(/\s+/g, ' ')
    .trim()
}

/** 正文纯文本（不截断；搜索与摘要共用同一口径） */
export function plainText(content: string | null | undefined): string {
  const raw = typeof content === 'string' ? content : ''
  if (!raw.trim()) return ''
  return raw
    .replace(/\r\n?/g, '\n')
    .split('\n')
    .map(stripLine)
    .filter((line) => line.length > 0)
    .join(' ')
    .replace(/\s+/g, ' ')
    .trim()
}

/**
 * 便签摘要：剥 Markdown → 去空行 → 压单行 → 超长按字符截断（末尾补「…」）。
 * 默认 120 字；空内容返回 ''，由调用方兜底「（空）」。
 */
export function plainPreview(
  content: string | null | undefined,
  maxLen: number = DEFAULT_PREVIEW_MAX,
): string {
  const text = plainText(content)
  const limit = Number.isFinite(maxLen) && maxLen > 0 ? Math.floor(maxLen) : DEFAULT_PREVIEW_MAX
  return text.length > limit ? `${text.slice(0, limit)}…` : text
}
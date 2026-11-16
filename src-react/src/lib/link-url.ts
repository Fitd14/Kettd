/**
 * 外链地址校验（链接语法 / 外链打开共用一份口径）。
 * 只放行 http / https / mailto：`javascript:`、`file:`、`data:` 一律不认。
 */
const SAFE_PROTOCOLS = new Set(['http:', 'https:', 'mailto:'])

export function isSafeLink(href: string): boolean {
  const value = href.trim()
  if (!value) return false
  try {
    return SAFE_PROTOCOLS.has(new URL(value).protocol)
  } catch {
    return false
  }
}
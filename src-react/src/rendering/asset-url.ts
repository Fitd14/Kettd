/**
 * 知识库图片的运行时地址解析（kb-storage-backup D2）。
 *
 * 正文里只存相对引用 `assets/<sha256>.<ext>`（数据目录可迁移，绝对路径一挪就坏）；
 * 渲染时才拼上 Bootstrap.assetsDir，并交给 Tauri 的 convertFileSrc 转成 webview 可加载的地址。
 *
 * 约束：
 * - 只处理 `assets/` 前缀；data URL / http(s) / 其它相对路径原样返回；
 * - `__TAURI__` 惰性取（注入可能晚于模块求值，不许缓存全局引用）；
 * - 非 Tauri 环境（浏览器调试 / 单测）或桥缺失：原样返回，不抛错、不产生破图以外的副作用。
 */

const ASSETS_PREFIX = 'assets/'

/** Tauri v1（`__TAURI__.tauri`）与 v2（`__TAURI__.core`）的 convertFileSrc 都认 */
type TauriBridgeLike = {
  tauri?: { convertFileSrc?: (filePath: string) => string }
  core?: { convertFileSrc?: (filePath: string) => string }
}

/** 模块级配置：App 加载 bootstrap 后写入 assets 目录绝对路径；空串 = 未配置 */
let assetsDir = ''

/** 配置 assets 目录（App 启动、bootstrap 返回后调用一次） */
export function configureAssetsDir(dir: string | null | undefined): void {
  assetsDir = typeof dir === 'string' ? dir.trim() : ''
}

function lazyConvertFileSrc(): ((filePath: string) => string) | null {
  const bridge = (globalThis as { __TAURI__?: TauriBridgeLike }).__TAURI__
  const convert = bridge?.tauri?.convertFileSrc ?? bridge?.core?.convertFileSrc
  return typeof convert === 'function' ? convert : null
}

/**
 * 把正文里的 `assets/<sha>.<ext>` 解析成可渲染地址。
 * 其余一切输入（空串、data URL、绝对 URL、未配置、非 Tauri、桥调用失败）原样返回。
 */
export function resolveAssetUrl(src: string): string {
  if (!src.startsWith(ASSETS_PREFIX)) return src
  if (!assetsDir) return src
  const fileName = src.slice(ASSETS_PREFIX.length)
  if (!fileName) return src
  const convert = lazyConvertFileSrc()
  if (!convert) return src
  // Windows 目录用反斜杠拼接更稳妥；convertFileSrc 两种分隔符都能处理。
  const separator = assetsDir.includes('\\') ? '\\' : '/'
  const base = assetsDir.replace(/[\\/]+$/, '')
  try {
    return convert(`${base}${separator}${fileName}`)
  } catch {
    return src
  }
}
/**
 * 外链打开（kb-writing-experience D11）。
 *
 * 前端没有 `@tauri-apps/api` 依赖，tauri.conf.json 开了 `withGlobalTauri`，
 * 所以直接走 `window.__TAURI__.shell.open`；桥可能晚于模块求值注入，每次惰性取，不缓存。
 * 协议先过 `isSafeLink`：只放行 http / https / mailto。
 */
import { isSafeLink } from './link-url'

type ShellBridge = { shell?: { open?: (url: string) => Promise<unknown> } }

/** 在系统浏览器打开；桥不可用或协议不合法时静默不动（不弹窗、不导航 WebView） */
export function openExternal(href: string): void {
  if (!isSafeLink(href)) return
  const bridge = (globalThis as { __TAURI__?: ShellBridge }).__TAURI__
  const open = bridge?.shell?.open
  if (typeof open !== 'function') return
  void open(href).catch(() => {})
}
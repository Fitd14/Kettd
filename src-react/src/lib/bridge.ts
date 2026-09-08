/**
 * Tauri 桥接（M1 最小集；M2 扩展为全部命令的类型化封装）。
 * 契约真相源：src-tauri/docs/V2-API.md；延续 vanilla 时代 { data, err } 约定（M4 后 vanilla 已退役）：
 * err 为后端可直接展示的中文短句，永不 throw。
 * withGlobalTauri 开启（tauri.conf.json），走 window.__TAURI__，不引入 npm 依赖。
 */

type CallResult<T> = { data: T | null; err: string | null }

type TauriGlobal = {
  tauri?: { invoke?: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> }
  core?: { invoke?: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> }
  invoke?: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>
  event?: {
    listen?: (name: string, handler: (event: { payload?: unknown }) => void) => Promise<() => void>
  }
  listen?: (name: string, handler: (event: { payload?: unknown }) => void) => Promise<() => void>
}

const T: TauriGlobal = (globalThis as { __TAURI__?: TauriGlobal }).__TAURI__ ?? {}

const rawInvoke = T.tauri?.invoke ?? T.core?.invoke ?? T.invoke
const rawListen = T.event?.listen ?? T.listen

export async function call<T>(command: string, args?: Record<string, unknown>): Promise<CallResult<T>> {
  if (typeof rawInvoke !== 'function') return { data: null, err: '未连接桌面运行时，命令未执行' }
  try {
    const data = await rawInvoke(command, args ?? {})
    return { data: (data ?? null) as T | null, err: null }
  } catch (e) {
    const err = typeof e === 'string' ? e : ((e as { message?: string; payload?: string })?.message
      ?? (e as { payload?: string })?.payload) ?? String(e)
    return { data: null, err }
  }
}

/** 订阅后端事件；返回退订函数（桥不可用时返回空退订） */
export function onEvent(name: string, handler: (payload: unknown) => void): () => void {
  if (typeof rawListen !== 'function') return () => {}
  let off: (() => void) | null = null
  void rawListen(name, (event) => handler(event.payload)).then((unlisten) => {
    off = unlisten
  }).catch(() => {})
  return () => { try { off?.() } catch { /* 已退订则忽略 */ } }
}

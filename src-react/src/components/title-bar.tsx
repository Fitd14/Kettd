import { useEffect, useState } from 'react'
import { mainWindowBridge } from '@/lib/api'

/**
 * 自定义标题栏（方案B）：main 窗 decorations:false 后，栏由前端用设计令牌绘制，
 * 明暗随 .dark 类即时切换 —— 这是原生栏喂不进应用暖黑令牌的根因解。
 *
 * - 拖动 / 双击最大化：data-tauri-drag-region（tauri 核心注入，无需自写）
 * - ─ □ ⧉ ×：Tauri window API；最大化态图标随 onResized 同步
 *   （按钮和拖拽区双击两条路径都会触发 resize，单一来源覆盖）
 */
export function TitleBar() {
  const [maximized, setMaximized] = useState(false)

  useEffect(() => {
    const w = mainWindowBridge()
    if (!w) return
    let disposed = false
    let off: (() => void) | null = null
    const sync = () => {
      void w.isMaximized?.().then((m) => {
        if (!disposed) setMaximized(!!m)
      })
    }
    sync()
    void w.onResized?.(() => sync()).then((un) => {
      if (disposed) un?.()
      else off = un ?? null
    })
    return () => {
      disposed = true
      off?.()
    }
  }, [])

  return (
    <div className="titlebar">
      <div className="titlebar-drag" data-tauri-drag-region>
        Kettd
      </div>
      <div className="titlebar-actions">
        <button
          type="button"
          className="titlebar-btn"
          title="最小化"
          aria-label="最小化"
          onClick={() => { void mainWindowBridge()?.minimize?.() }}
        >
          ─
        </button>
        <button
          type="button"
          className="titlebar-btn"
          title={maximized ? '向下还原' : '最大化'}
          aria-label={maximized ? '向下还原' : '最大化'}
          onClick={() => { void mainWindowBridge()?.toggleMaximize?.() }}
        >
          {maximized ? '⧉' : '□'}
        </button>
        <button
          type="button"
          className="titlebar-btn close"
          title="关闭"
          aria-label="关闭"
          onClick={() => { void mainWindowBridge()?.close?.() }}
        >
          ×
        </button>
      </div>
    </div>
  )
}

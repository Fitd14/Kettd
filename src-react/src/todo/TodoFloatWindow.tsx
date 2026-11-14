import { useCallback, useEffect, useState } from 'react'
import { GripHorizontal, Minus, Pin, PinOff, X } from 'lucide-react'
import {
  applyTheme,
  getBootstrap,
  hideTodoFloat,
  onEvent,
  openMainWindow,
  setSettings,
  setTodoFloatMini,
  updateTask,
  type Bootstrap,
} from '@/lib/api'
import { todayList } from '../kernel/selectors.js'
import { localToday } from '../kernel/time.js'
import { PaperPattern } from '@/sticky/paper-patterns'

/**
 * 待办悬浮窗（todo-float · 拍板「纯轻量镜像」档）：
 * - 今天列表镜像 + 勾选完成，写操作归主窗/捕获条——待办与便签彻底分家后的待办专属悬浮窗
 * - 纸感与便签同一族（复用 sticky 的纸色令牌/胶条/折角，纸色/花纹跟随设置）
 * - 两态折叠：展开态（完整列表）/ 迷你态（40px 单行，`待办 · 今天 N 项` + 展开 + ✕）。
 *   形态由 `Bootstrap.todoFloatMini` 持久化（后端按档建窗高 40px / 常规高度）；展开态的 − 收成
 *   悬浮条，迷你条点「展开」恢复完整面板，切换后刷新 boot 同步档位。拖拽复用 sticky 迷你条的 grip 写法。
 * - 置顶 = 窗内 Pin 钮（settings.todoFloatPinned，默认开），✕ = 藏窗（托盘/热键呼回）
 */
export default function TodoFloatWindow() {
  const [boot, setBoot] = useState<Bootstrap | null>(null)
  // 形态切换先乐观渲染，避免命令往返期间 40px 窗里先出现一次完整列表（或反之）
  const [miniOverride, setMiniOverride] = useState<boolean | null>(null)

  const refresh = useCallback(async () => {
    const r = await getBootstrap()
    if (!r.err && r.data) {
      setBoot(r.data)
      applyTheme(r.data.settings.theme)
    }
  }, [])

  useEffect(() => {
    void refresh()
    const off = onEvent('store-changed', () => { void refresh() })
    const offTheme = onEvent('theme-changed', (payload: unknown) => applyTheme(payload as string))
    return () => { off(); offTheme() }
  }, [refresh])

  const today = localToday()
  const list = boot ? todayList(boot.tasks, today) : []
  const paper = boot?.settings.stickyPaper ?? 'warm'
  const pattern = boot?.settings.stickyPattern ?? 'none'
  const pinnedOn = boot?.settings.todoFloatPinned ?? true
  const doneCount = list.filter((t) => t.done).length
  // 首帧兜底：后端按 mini 档把窗高建到 40px，窗高本身就是形态的可靠信号（boot 尚未回来时防闪）
  const detectedMini = typeof window !== 'undefined' && window.innerHeight > 0 && window.innerHeight <= 48
  const mini = miniOverride ?? (boot?.todoFloatMini ?? detectedMini)

  /** 展开 ↔ 迷你：持久化到 runtime，切换后刷新 boot 同步档位；失败则回退到旧形态 */
  const setMini = useCallback(async (next: boolean) => {
    setMiniOverride(next)
    const r = await setTodoFloatMini(next)
    if (r.err) {
      setMiniOverride(null)
      return
    }
    await refresh()
    setMiniOverride(null)
  }, [refresh])

  const togglePin = useCallback(async () => {
    const r = await setSettings({ todoFloatPinned: !pinnedOn })
    if (!r.err) await refresh()
  }, [pinnedOn, refresh])

  const toggleDone = useCallback(async (id: string, next: boolean) => {
    await updateTask(id, { done: next })
    // store-changed 广播会触发 refresh，这里不重复拉
  }, [])

  const openMain = useCallback(() => {
    openMainWindow('/today')
  }, [])

  /* ---- 迷你态：40px 悬浮条（拖动区 + 摘要 + 展开 + ✕）---- */
  if (mini) {
    return (
      <div className={`sticky-note is-mini paper-${paper} todo-mini`}>
        <span className="sticky-mini-grip" data-tauri-drag-region title="按住拖动">
          <GripHorizontal size={14} aria-hidden />
        </span>
        <span
          className="sticky-mini-text todo-mini-text"
          data-tauri-drag-region
          title="按住拖动 · 今天待办"
        >
          待办 · 今天 {list.length} 项{doneCount > 0 ? ` · 做完 ${doneCount}` : ''}
        </span>
        <button
          className="sticky-btn todo-mini-btn"
          title="展开为完整列表"
          aria-label="展开待办悬浮窗"
          onClick={() => { void setMini(false) }}
        >
          展开
        </button>
        <button
          className="sticky-btn todo-mini-btn"
          title="隐藏（托盘或热键呼回）"
          aria-label="隐藏待办悬浮窗"
          onClick={() => { void hideTodoFloat() }}
        >
          <X size={12} aria-hidden />
        </button>
      </div>
    )
  }

  return (
    <div className={`sticky-note paper-${paper}`}>
      <PaperPattern pattern={pattern} />
      <div className="sticky-strip">
        <span className="sticky-head" data-tauri-drag-region title="按住拖动">
          今天要做的 · <b>{list.length}</b>{doneCount > 0 && ` · 做完 ${doneCount}`}
        </span>
        <button
          className={`sticky-btn pin${pinnedOn ? ' on' : ''}`}
          title={pinnedOn ? '取消置顶（沉到普通层）' : '置顶'}
          aria-label={pinnedOn ? '取消置顶' : '置顶'}
          aria-pressed={pinnedOn}
          onClick={() => { void togglePin() }}
        >
          {pinnedOn ? <Pin size={14} aria-hidden /> : <PinOff size={14} aria-hidden />}
        </button>
        <button
          className="sticky-btn"
          title="最小化为悬浮条"
          aria-label="最小化为悬浮条"
          onClick={() => { void setMini(true) }}
        >
          <Minus size={14} aria-hidden />
        </button>
        <button className="sticky-btn" title="隐藏（托盘或热键呼回）" aria-label="隐藏待办悬浮窗" onClick={() => { void hideTodoFloat() }}>
          <X size={14} aria-hidden />
        </button>
      </div>

      <div className="sticky-body" role={list.length > 0 ? 'list' : undefined} aria-label="今天待办">
        {list.length === 0 ? (
          <div className="sticky-empty">
            今天没有待办 ·<br />
            <kbd>Alt+Shift+A</kbd> 记一条
          </div>
        ) : (
          list.map((t) => (
            <div key={t.id} className="sticky-row" role="listitem">
              <input
                type="checkbox"
                className="tcheck"
                aria-label={`完成 ${t.title}`}
                checked={!!t.done}
                onMouseDown={(e) => e.preventDefault()}
                onChange={() => { void toggleDone(t.id, !t.done) }}
              />
              <button className={`sticky-title${t.done ? ' is-done' : ''}`} onClick={openMain}>
                {t.title}
                {(Number(t.carriedFrom) || 0) > 0 && (
                  <span className="sticky-carry">顺延 {t.carriedFrom} 天</span>
                )}
              </button>
            </div>
          ))
        )}
      </div>
      <i className="sticky-fold" aria-hidden />
    </div>
  )
}
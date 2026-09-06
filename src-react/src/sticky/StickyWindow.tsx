import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  applyTheme,
  getBootstrap,
  onEvent,
  setStickyPinned,
  updateTask,
  type Bootstrap,
  type Task,
} from '@/lib/api'
import { todaySections } from '../../../src/kernel/selectors.js'
import { localToday } from '../../../src/kernel/time.js'
import './sticky.css'

/**
 * 便签（便签规格 / sticky-note-component-spec · M3 React 落地）：
 * - 单一便签，顶部仅 固定/关闭 两键（AC2）；固定 = 只切置顶，位置始终可拖（AC3）
 * - 移出鼠标淡化 ≈38%、移入恢复（AC4），装饰性弱化不承担常态可读
 * - 内容 = 镜像「今天」（零维护）+ 用户钉上的单条（D2：镜像今天 + 可钉单条）
 * - 空态：今天没有待办 → 提示 Alt+Shift+A（AC 便签 §4）
 * - 录入不在便签内：完全走捕获条（AC5）
 */
export default function StickyWindow() {
  const [boot, setBoot] = useState<Bootstrap | null>(null)
  const [faded, setFaded] = useState(false)

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
  const sections = useMemo(
    () => (boot ? todaySections(boot.tasks, today) : null),
    [boot, today],
  )
  const mirror: Task[] = useMemo(
    () => (sections ? [...sections.due, ...sections.carried].slice(0, 7) : []),
    [sections],
  )
  const pinned: Task[] = useMemo(
    () =>
      (boot?.tasks ?? []).filter(
        (t) => t.stickyPinned && !t.done && !t.deletedAt && !mirror.some((m) => m.id === t.id),
      ),
    [boot, mirror],
  )

  const paper = boot?.settings.stickyPaper ?? 'warm'
  const pinnedOn = boot?.settings.stickyPinned ?? true

  const togglePin = useCallback(async () => {
    const r = await setStickyPinned(!pinnedOn)
    if (!r.err) await refresh()
  }, [pinnedOn, refresh])

  const toggleDone = useCallback(async (id: string, next: boolean) => {
    const r = await updateTask(id, { done: next })
    if (!r.err) await refresh()
  }, [refresh])

  const unpin = useCallback(async (id: string) => {
    const r = await updateTask(id, { stickyPinned: false })
    if (!r.err) await refresh()
  }, [refresh])

  const openMain = useCallback(() => {
    void import('@/lib/api').then((m) => m.openMainWindow())
    void import('@/lib/api').then((m) => m.emitNavigate('/today'))
  }, [])

  const close = useCallback(async () => {
    await import('@/lib/api').then((m) => m.hideFloat())
  }, [])

  const rows = (list: Task[], removable: boolean) =>
    list.map((t) => (
      <div key={t.id} className="sticky-row">
        <input
          type="checkbox"
          className="tcheck"
          aria-label={`完成 ${t.title}`}
          checked={!!t.done}
          onChange={() => { void toggleDone(t.id, !t.done) }}
        />
        <button className="sticky-title" onClick={openMain}>
          {t.title}
          {(Number(t.carriedFrom) || 0) > 0 && (
            <span className="sticky-carry">拖了 {t.carriedFrom} 天</span>
          )}
        </button>
        {removable && (
          <button className="sticky-unpin" aria-label="从便签取下" title="取下" onClick={() => { void unpin(t.id) }}>
            ×
          </button>
        )}
      </div>
    ))

  const doneToday = boot?.tasks.filter(
    (t) => t.done && (t.plannedDate === today || String(t.dueAt || '').startsWith(today)),
  ).length ?? 0
  const total = mirror.length + pinned.length

  return (
    <div
      className={`sticky-note paper-${paper}${faded ? ' faded' : ''}${pinnedOn ? ' is-pinned' : ''}`}
      onMouseLeave={() => setFaded(true)}
      onMouseEnter={() => setFaded(false)}
    >
      <div className="sticky-strip" data-tauri-drag-region title="按住拖动">
        <button
          className={`sticky-btn pin${pinnedOn ? ' on' : ''}`}
          title={pinnedOn ? '取消固定（沉到普通层）' : '固定（置顶）'}
          aria-label={pinnedOn ? '取消固定' : '固定置顶'}
          aria-pressed={pinnedOn}
          onClick={() => { void togglePin() }}
        >
          {pinnedOn ? '📌' : '📍'}
        </button>
        <span className="sticky-head" data-tauri-drag-region>
          今天要做的 · <b>{total}</b>{doneToday > 0 && ` · 做完 ${doneToday}`}
        </span>
        <button className="sticky-btn" title="关闭（收进托盘）" aria-label="关闭" onClick={() => { void close() }}>
          ✕
        </button>
      </div>

      <div className="sticky-body">
        {total === 0 ? (
          <div className="sticky-empty">
            今天没有待办 ·<br />
            <kbd>Alt+Shift+A</kbd> 记一条
          </div>
        ) : (
          <>
            {rows(mirror, false)}
            {pinned.length > 0 && (
              <>
                <div className="sticky-gap" />
                {rows(pinned, true)}
              </>
            )}
          </>
        )}
      </div>
      <i className="sticky-fold" aria-hidden />
    </div>
  )
}

import { useMemo, useState } from 'react'
import type { Bootstrap } from '@/lib/api'
import { addTask, deleteTask, undoDelete, updateTask } from '@/lib/api'
import { isDoneToday, todaySections } from '../../../src/kernel/selectors.js'
import { localToday } from '../../../src/kernel/time.js'
import { TaskRow } from '@/components/task-row'
import type { useUndoToast } from '@/components/undo-toast'

interface Props {
  boot: Bootstrap
  refresh: () => Promise<void>
  undo: ReturnType<typeof useUndoToast>
}

/**
 * 今天视图（PRD 6.3 / today-list-ui-spec）：
 * 三段式（今日到期 → 已拖到今天 → 逾期折叠行）+ 三处「今天」同源计数 + 完成庆祝空态。
 */
export function TodayView({ boot, refresh, undo }: Props) {
  const today = localToday()
  const [overdueOpen, setOverdueOpen] = useState(false)
  const [saveErr, setSaveErr] = useState<string | null>(null)
  const s = useMemo(() => todaySections(boot.tasks, today), [boot.tasks, today])
  const doneToday = useMemo(
    () => boot.tasks.filter((t) => isDoneToday(t, today)),
    [boot.tasks, today],
  )

  const toggle = async (id: string) => {
    const r = await updateTask(id, { done: !boot.tasks.find((t) => t.id === id)?.done })
    if (r.err) {
      setSaveErr(r.err)
      return
    }
    setSaveErr(null)
    await refresh()
  }

  const planToday = async (id: string) => {
    const r = await updateTask(id, { plannedDate: today })
    if (r.err) { setSaveErr(r.err); return }
    setSaveErr(null)
    await refresh()
  }

  const schedule = async (id: string, dueAt: string | null) => {
    const r = await updateTask(id, { dueAt: dueAt ? `${dueAt}T09:00` : null })
    if (r.err) { setSaveErr(r.err); return }
    setSaveErr(null)
    await refresh()
  }

  const drop = async (id: string) => {
    // 丢弃 = 软删 + 10s 撤销（PRD 6.3 AC3 / 6.6）
    const r = await deleteTask(id)
    if (r.err) { setSaveErr(r.err); return }
    const target = boot.tasks.find((t) => t.id === id)
    undo.arm(target?.title ?? id, async () => {
      const u = await undoDelete()
      if (!u.err) await refresh()
      return u
    })
    await refresh()
  }

  const pinSticky = async (id: string, pinned: boolean) => {
    const r = await updateTask(id, { stickyPinned: pinned })
    if (r.err) { setSaveErr(r.err); return }
    setSaveErr(null)
    await refresh()
  }

  const rescheduleOverdue = async (id: string) => {
    // 逾期处置：改截止 = 顺延到今天 18:00（明示字面值，本地语义）
    await schedule(id, today)
  }

  const quickAdd = async () => {
    // 兜底入口：热键不可用时点这里记一条（PRD 6.2 边缘降级）
    const title = window.prompt('记一条…')
    if (!title?.trim()) return
    const r = await addTask({ title: title.trim(), source: 'manual' })
    if (r.err) { setSaveErr(r.err); return }
    await refresh()
  }

  const totalOpen = s.due.length + s.carried.length
  const allClear = totalOpen === 0 && s.overdue.length === 0

  return (
    <div className="view">
      <div>
        <div className="view-title">今天</div>
        <div className="view-sub">
          {today} · 剩 {totalOpen} · 今天做完 {doneToday.length}
          {' · '}侧栏 / 便签 / 这里三处数字同源
        </div>
      </div>

      {saveErr && <div className="inline-err" role="alert">{saveErr}</div>}

      {allClear && totalOpen === 0 && (
        doneToday.length > 0 ? (
          <div className="celebrate" aria-live="polite">
            <div className="mark">✓</div>
            <div>今天清零 · 做完 {doneToday.length} 件</div>
          </div>
        ) : (
          <div className="empty">
            今天还没有挑出来的事项。
            <div style={{ marginTop: 8 }}>
              按 <kbd>Alt+Shift+A</kbd> 记一条，或在收件箱里把它拖进来。
              <button className="btn ghost xs" style={{ marginLeft: 8 }} onClick={() => { void quickAdd() }}>
                就地记一条
              </button>
            </div>
          </div>
        )
      )}

      {s.due.length > 0 && (
        <section className="section" aria-label="今日到期">
          <div className="section-head">
            <b>今日到期</b>
            <span>{s.due.length} 件</span>
          </div>
          {s.due.map((t) => (
            <TaskRow
              key={t.id}
              task={t}
              today={today}
              actions="today"
              onToggle={(id) => { void toggle(id) }}
              onSchedule={(id, due) => { void schedule(id, due) }}
              onDelete={(id) => { void drop(id) }}
              onPinSticky={(id, pinned) => { void pinSticky(id, pinned) }}
            />
          ))}
        </section>
      )}

      {s.carried.length > 0 && (
        <section className="section" aria-label="已拖到今天">
          <div className="section-head">
            <b>已拖到今天</b>
            <span>{s.carried.length} 件</span>
          </div>
          {s.carried.map((t) => (
            <TaskRow
              key={t.id}
              task={t}
              today={today}
              actions="today"
              onToggle={(id) => { void toggle(id) }}
              onSchedule={(id, due) => { void schedule(id, due) }}
              onDelete={(id) => { void drop(id) }}
              onPinSticky={(id, pinned) => { void pinSticky(id, pinned) }}
            />
          ))}
        </section>
      )}

      {s.overdue.length > 0 && (
        <details className="overdue" open={overdueOpen} onToggle={(e) => setOverdueOpen((e.target as HTMLDetailsElement).open)}>
          <summary>已逾期 {s.overdue.length} 项 · 展开逐条处置</summary>
          <div style={{ marginTop: 6 }}>
            {s.overdue.map((t) => (
              <div key={t.id}>
                <TaskRow
                  task={t}
                  today={today}
                  actions="today"
                  onToggle={(id) => { void toggle(id) }}
                  onDelete={(id) => { void drop(id) }}
                  onSchedule={(id) => { void rescheduleOverdue(id) }}
                />
                {overdueOpen && (
                  <div style={{ margin: '0 0 4px 30px' }}>
                    <button className="btn ghost xs" onClick={() => { void rescheduleOverdue(t.id) }}>改到今天 18:00 前</button>
                    <button className="btn ghost xs" onClick={() => { void planToday(t.id) }}>移今天</button>
                  </div>
                )}
              </div>
            ))}
          </div>
        </details>
      )}
    </div>
  )
}

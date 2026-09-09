import { useMemo, useRef, useState } from 'react'
import type { Bootstrap } from '@/lib/api'
import { addTask, deleteTask, reorderTasks, undoDelete, updateTask } from '@/lib/api'
import { isDoneToday, nextReminderTime, todayList, todaySections } from '../kernel/selectors.js'
import { localToday } from '../kernel/time.js'
import { TaskRow } from '@/components/task-row'
import { TaskDetail } from '@/components/task-detail'
import { useRowNav } from '@/lib/list-nav'
import type { useUndoToast } from '@/components/undo-toast'

interface Props {
  boot: Bootstrap
  refresh: () => Promise<void>
  undo: ReturnType<typeof useUndoToast>
}

/**
 * 今天视图（today-list-ui-spec）：
 * - 单一捕获入口（B.1 内联输入框）+ 单一主列表（B.2 到期/顺延合并，badge 区分）
 * - 执行驾驶舱（B.4）：下一条提醒行 + 「今天计划 N 项」口径
 * - 逾期琥珀折叠条（B.3）；拖拽/Alt+↑↓ 排序（便签规格 §12.2）；j/k 行间导航
 * - 三处「今天」同源计数 + 完成庆祝空态（B.6 保留项）
 */
export function TodayView({ boot, refresh, undo }: Props) {
  const today = localToday()
  const [overdueOpen, setOverdueOpen] = useState(false)
  const [saveErr, setSaveErr] = useState<string | null>(null)
  const [capture, setCapture] = useState('')
  const [detailId, setDetailId] = useState<string | null>(null)
  const listRef = useRef<HTMLDivElement>(null)
  const s = useMemo(() => todaySections(boot.tasks, today), [boot.tasks, today])
  const merged = useMemo(() => todayList(boot.tasks, today), [boot.tasks, today])
  const doneToday = useMemo(
    () => boot.tasks.filter((t) => isDoneToday(t, today)),
    [boot.tasks, today],
  )
  const next = useMemo(() => nextReminderTime(boot.tasks, boot.reminders), [boot.tasks, boot.reminders])

  const { containerProps } = useRowNav({
    containerRef: listRef,
    ids: merged.map((t) => t.id),
    onReorder: async (ids) => {
      const r = await reorderTasks(ids)
      if (r.err) setSaveErr(r.err)
      await refresh()
    },
  })

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

  const rescheduleOverdue = async (id: string) => {
    // 逾期处置：改截止 = 顺延到今天 18:00（明示字面值，本地语义）
    await schedule(id, today)
  }

  const captureCommit = async () => {
    // 单一捕获入口（B.1）：回车放进今天
    const title = capture.trim()
    if (!title) return
    const r = await addTask({ title, source: 'manual', plannedDate: today })
    if (r.err) { setSaveErr(r.err); return }
    setSaveErr(null)
    setCapture('')
    await refresh()
  }

  const totalOpen = s.due.length + s.carried.length
  const planTotal = totalOpen + doneToday.length
  const allClear = totalOpen === 0 && s.overdue.length === 0
  const detailTask = boot.tasks.find((t) => t.id === detailId) ?? null

  return (
    <div className="view">
      <div>
        <div className="view-title">今天</div>
        <div className="view-sub">
          {today} · 今天计划 {planTotal} 项 · 剩 {totalOpen} · 做完 {doneToday.length}
          {' · '}三处同源
        </div>
        <div className="next-remind" aria-live="polite">
          下一条提醒 · <b>{next ?? '—'}</b>
        </div>
      </div>

      <form className="capture-bar" onSubmit={(e) => { e.preventDefault(); void captureCommit() }}>
        <input
          className="input sm grow"
          placeholder="记一条，回车放进今天（或按 Alt+Shift+A）"
          aria-label="记一条放进今天"
          value={capture}
          onChange={(e) => setCapture(e.target.value)}
        />
        <button className="btn xs outline" type="submit" disabled={!capture.trim()}>记下</button>
      </form>

      {saveErr && <div className="inline-err" role="alert">{saveErr}</div>}

      {allClear && (
        doneToday.length > 0 ? (
          <div className="celebrate" aria-live="polite">
            <div className="mark">✓</div>
            <div>今天清零 · 做完 {doneToday.length} 件</div>
          </div>
        ) : (
          <div className="empty">今天还没有挑出来的事项。</div>
        )
      )}

      {merged.length > 0 && (
        <section className="section" aria-label="今天要做的">
          <div className="section-head">
            <b>今天要做的</b>
            <span>{merged.length} 件</span>
          </div>
          <div ref={listRef} role="list" aria-label="今天要做的" {...containerProps}>
            {merged.map((t, i) => (
              <TaskRow
                key={t.id}
                task={t}
                today={today}
                actions="today"
                rowIndex={i}
                dragEnabled
                onToggle={(id) => { void toggle(id) }}
                onSchedule={(id, due) => { void schedule(id, due) }}
                onDelete={(id) => { void drop(id) }}
                onOpen={setDetailId}
              />
            ))}
          </div>
          <div className="tiny text-muted" style={{ marginTop: 4 }}>
            拖行或 Alt+↑↓ 调整顺序 · j/k 移动 · Enter 看详情
          </div>
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
                  onOpen={setDetailId}
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

      <TaskDetail
        task={detailTask}
        today={today}
        onClose={() => setDetailId(null)}
        onDelete={(id) => { void drop(id) }}
      />
    </div>
  )
}

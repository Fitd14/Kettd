import { useMemo, useState } from 'react'
import type { Bootstrap } from '@/lib/api'
import { deleteReminder, snoozeReminder, toggleReminder, updateTask, validateReminderTime } from '@/lib/api'
import { addReminder } from '@/lib/api'
import { localToday } from '../../../src/kernel/time.js'
import { nextReminderTime } from '../../../src/kernel/selectors.js'

interface Props {
  boot: Bootstrap
  refresh: () => Promise<void>
}

/**
 * 计划视图（planned-settings-ui-spec 要点）：提醒管理 + 有安排的任务按日分组。
 * qa-3 语义：HH:MM / HH:MM 多时刻 = 循环；完整时刻 = 单次——展示时显式标注。
 */
export function PlannedView({ boot, refresh }: Props) {
  const today = localToday()
  const [err, setErr] = useState<string | null>(null)
  const [newTitle, setNewTitle] = useState('')
  const [newTime, setNewTime] = useState('')

  const scheduled = useMemo(
    () =>
      boot.tasks
        .filter((t) => !t.done && !t.deletedAt && (t.plannedDate || t.dueAt))
        .sort((a, b) =>
          String(a.plannedDate || a.dueAt).slice(0, 10)
            .localeCompare(String(b.plannedDate || b.dueAt).slice(0, 10)),
        ),
    [boot.tasks],
  )
  const next = nextReminderTime(boot.tasks, boot.reminders)

  const byDay = useMemo(() => {
    const map = new Map<string, typeof scheduled>()
    for (const t of scheduled) {
      const day = String(t.plannedDate || t.dueAt).slice(0, 10)
      const list = map.get(day) ?? []
      list.push(t)
      map.set(day, list)
    }
    return [...map.entries()].sort(([a], [b]) => a.localeCompare(b))
  }, [scheduled])

  const add = async () => {
    const v = validateReminderTime(newTime)
    if (v && v !== '__past__') { setErr(v); return }
    if (!newTitle.trim()) { setErr('提醒标题不能为空'); return }
    const r = await addReminder({
      title: newTitle.trim(),
      time: newTime.trim(),
      repeat: newTime.includes('/') ? 'daily' : 'none',
    })
    if (r.err) { setErr(r.err); return }
    setErr(null)
    setNewTitle('')
    setNewTime('')
    await refresh()
  }

  return (
    <div className="view">
      <div>
        <div className="view-title">计划</div>
        <div className="view-sub">
          下一条待响：{next ?? '—'} · 共 {boot.reminders.filter((r) => r.enabled && !r.completed).length} 条在跑
        </div>
      </div>

      {err && <div className="inline-err" role="alert">{err}</div>}

      <section className="section" aria-label="提醒">
        <div className="section-head"><b>提醒</b><span>到点弹系统通知</span></div>
        {boot.reminders.filter((r) => !r.completed).map((r) => (
          <div key={r.id} className="trow" data-id={r.id}>
            <input
              type="checkbox"
              className="tcheck"
              aria-label={`启用 ${r.title}`}
              checked={!!r.enabled}
              onChange={async () => {
                const res = await toggleReminder(r.id)
                if (res.err) setErr(res.err)
                await refresh()
              }}
            />
            <span className={`grow truncate sm ${r.enabled ? '' : 'line-through opacity-60'}`}>{r.title}</span>
            <span className="row-flex items-center gap-1.5 whitespace-nowrap">
              <code className="tmeta mono">{r.time}</code>
              <span className="badge">{r.time.includes('T') ? '单次' : '循环'}</span>
              {r.enabled && (
                <button
                  className="btn ghost xs"
                  onClick={async () => {
                    const res = await snoozeReminder(r.id, 10)
                    if (res.err) setErr(res.err)
                    await refresh()
                  }}
                >
                  稍后10分
                </button>
              )}
              <button
                className="btn ghost xs"
                onClick={async () => {
                  const res = await deleteReminder(r.id)
                  if (res.err) setErr(res.err)
                  await refresh()
                }}
              >
                删
              </button>
            </span>
          </div>
        ))}
        <div className="trow">
          <input
            className="input xs grow"
            placeholder="提醒内容…"
            value={newTitle}
            onChange={(e) => setNewTitle(e.target.value)}
          />
          <input
            className="input xs"
            type="time"
            value={newTime}
            onChange={(e) => setNewTime(e.target.value)}
          />
          <button className="btn xs outline" onClick={() => { void add() }}>加提醒</button>
        </div>
      </section>

      <section className="section" aria-label="已安排的任务">
        <div className="section-head"><b>已安排的任务</b><span>{scheduled.length} 件</span></div>
        {byDay.length === 0 && <div className="empty">还没有带日期的任务。在任务详情里排期，或用行尾「排期…」。</div>}
        {byDay.map(([day, list]) => (
          <div key={day} style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            <div className="section-head">
              <span>{day === today ? '今天' : day.slice(5)}</span>
              <span>{list.length} 件</span>
            </div>
            {list.map((t) => (
              <div key={t.id} className="trow">
                <span className="grow truncate sm">{t.title}</span>
                <span className="row-flex items-center gap-1.5 whitespace-nowrap">
                  {t.dueAt && <code className="tmeta mono">{String(t.dueAt).slice(11, 16) || String(t.dueAt).slice(0, 10)}</code>}
                  <button
                    className="btn ghost xs"
                    onClick={async () => {
                      const res = await updateTask(t.id, { plannedDate: today })
                      if (res.err) setErr(res.err)
                      await refresh()
                    }}
                  >
                    →今天
                  </button>
                </span>
              </div>
            ))}
          </div>
        ))}
      </section>
    </div>
  )
}

import { useMemo, useState } from 'react'
import type { Bootstrap, Reminder } from '@/lib/api'
import {
  addReminder,
  deleteReminder,
  snoozeReminder,
  toggleReminder,
  updateReminder,
  updateTask,
  validateReminderTime,
} from '@/lib/api'
import { localToday } from '../../../src/kernel/time.js'
import { nextReminderTime } from '../../../src/kernel/selectors.js'
import { TimeText } from '@/components/time-text'

interface Props {
  boot: Bootstrap
  refresh: () => Promise<void>
}

type Tab = 'plan' | 'remind'

/**
 * 计划视图（planned-settings-ui-spec §A）：
 * - 顶部「安排 / 提醒」Tabs，默认进安排（A.1）；全页命名统一（A.5），计数只出现一处
 * - 提醒行精简：默认 [启停] 标题 · 时间 · badge，hover 才出 编辑时间/稍后10分/删（A.2）
 * - 时间编辑用弹层（日期可选 + HH:MM 时刻输入），不再原生 type=time 堆叠（A.3）
 * - 手动加提醒默认折叠（A.4）
 */
export function PlannedView({ boot, refresh }: Props) {
  const today = localToday()
  const [tab, setTab] = useState<Tab>('plan')
  const [err, setErr] = useState<string | null>(null)
  const [newTitle, setNewTitle] = useState('')
  const [newTime, setNewTime] = useState('')
  const [editing, setEditing] = useState<string | null>(null)
  const [editDate, setEditDate] = useState('')
  const [editTime, setEditTime] = useState('')

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
  const activeReminders = useMemo(
    () => boot.reminders.filter((r) => !r.completed),
    [boot.reminders],
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

  const startEdit = (r: Reminder) => {
    setEditing(r.id)
    const time = String(r.time)
    if (/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(time)) {
      setEditDate(time.slice(0, 10))
      setEditTime(time.slice(11, 16))
    } else {
      setEditDate('')
      setEditTime(time)
    }
  }

  const commitEdit = async (r: Reminder) => {
    const time = editDate.trim()
      ? `${editDate.trim()}T${editTime.trim().split('/')[0]}`
      : editTime.trim()
    const v = validateReminderTime(time)
    if (v && v !== '__past__') { setErr(v); return }
    const res = await updateReminder(r.id, { time })
    if (res.err) { setErr(res.err); return }
    setErr(null)
    setEditing(null)
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

      <div className="tabs" role="tablist" aria-label="计划分区">
        <button
          className={`tab${tab === 'plan' ? ' on' : ''}`}
          role="tab"
          aria-selected={tab === 'plan'}
          onClick={() => setTab('plan')}
        >
          安排
        </button>
        <button
          className={`tab${tab === 'remind' ? ' on' : ''}`}
          role="tab"
          aria-selected={tab === 'remind'}
          onClick={() => setTab('remind')}
        >
          提醒
        </button>
      </div>

      {tab === 'remind' && (
        <section className="section" aria-label="提醒">
          <div className="section-head"><b>提醒</b><span>到点弹系统通知</span></div>
          {activeReminders.map((r) => (
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
                <span className="row-actions">
                  <button className="btn ghost xs" onClick={() => startEdit(r)}>编辑时间</button>
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
              </span>
              {editing === r.id && (
                <span className="schedule-pop rem-edit" aria-label={`编辑提醒时间：${r.title}`}>
                  <label className="tiny text-muted">
                    日期（留空 = 每天循环）
                    <input
                      type="date"
                      className="input xs"
                      value={editDate}
                      onChange={(e) => setEditDate(e.target.value)}
                    />
                  </label>
                  <TimeText
                    value={editTime}
                    allowMulti={!editDate.trim()}
                    ariaLabel="提醒时刻"
                    onCommit={(v) => { setEditTime(v); void commitEdit(r) }}
                  />
                  <button className="btn xs outline" onClick={() => { void commitEdit(r) }}>定</button>
                  <button className="btn ghost xs" onClick={() => setEditing(null)}>取消</button>
                </span>
              )}
            </div>
          ))}

          <details className="add-rem">
            <summary>＋ 手动加提醒</summary>
            <div className="trow">
              <input
                className="input xs grow"
                placeholder="提醒内容…"
                value={newTitle}
                onChange={(e) => setNewTitle(e.target.value)}
              />
              <TimeText
                value={newTime}
                allowMulti
                ariaLabel="新提醒时刻"
                onCommit={(v) => { setNewTime(v); void add() }}
              />
              <button className="btn xs outline" onClick={() => { void add() }}>加提醒</button>
            </div>
          </details>
        </section>
      )}

      {tab === 'plan' && (
        <section className="section" aria-label="安排">
          <div className="section-head"><b>安排</b><span>{scheduled.length} 件</span></div>
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
      )}
    </div>
  )
}

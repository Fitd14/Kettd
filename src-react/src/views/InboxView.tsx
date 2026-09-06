import { useMemo, useState } from 'react'
import type { Bootstrap } from '@/lib/api'
import { deleteTask, undoDelete, updateTask } from '@/lib/api'
import { inboxGroups } from '../../../src/kernel/selectors.js'
import { localToday } from '../../../src/kernel/time.js'
import { TaskRow } from '@/components/task-row'
import type { useUndoToast } from '@/components/undo-toast'

interface Props {
  boot: Bootstrap
  refresh: () => Promise<void>
  undo: ReturnType<typeof useUndoToast>
}

/** 收件箱 = 清空工作台（inbox-ui-spec）：读列表 + hover 动作 + 老化分组 + 批量栏 + 拖到今天。 */
export function InboxView({ boot, refresh, undo }: Props) {
  const today = localToday()
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [dragOver, setDragOver] = useState(false)
  const [saveErr, setSaveErr] = useState<string | null>(null)
  const groups = useMemo(() => inboxGroups(boot.tasks, today), [boot.tasks, today])

  const selectedIds = [...selected]

  const runFor = async (ids: string[], action: (id: string) => Promise<{ err: string | null }>) => {
    let firstErr: string | null = null
    for (const id of ids) {
      const r = await action(id)
      if (r.err && !firstErr) firstErr = r.err
    }
    setSaveErr(firstErr)
    setSelected(new Set())
    await refresh()
  }

  const planToday = (ids: string[]) => runFor(ids, (id) => updateTask(id, { plannedDate: today }))
  const schedule = (ids: string[]) => {
    const due = window.prompt('排期到哪天？（YYYY-MM-DD，留空取消）')
    if (!due?.trim()) return
    void runFor(ids, (id) => updateTask(id, { dueAt: `${due.trim()}T09:00` }))
  }
  const drop = (ids: string[]) => {
    void (async () => {
      let firstErr: string | null = null
      for (const id of ids) {
        const target = boot.tasks.find((t) => t.id === id)
        const r = await deleteTask(id)
        if (r.err && !firstErr) firstErr = r.err
        else if (!r.err) {
          undo.arm(target?.title ?? id, async () => {
            const u = await undoDelete()
            if (!u.err) await refresh()
            return u
          })
        }
      }
      setSaveErr(firstErr)
      setSelected(new Set())
      await refresh()
    })()
  }

  const toggleSelect = (id: string, checked: boolean) => {
    setSelected((prev) => {
      const next = new Set(prev)
      if (checked) next.add(id)
      else next.delete(id)
      return next
    })
  }

  const renderGroup = (title: string, entries: { task: Parameters<typeof TaskRow>[0]['task']; age: number }[], aged: boolean) => {
    if (!entries.length) return null
    return (
      <section className="section" aria-label={title}>
        <div className="section-head">
          <b>{title}</b>
          <span>{entries.length} 件</span>
        </div>
        {entries.map(({ task, age }) => (
          <div
            key={task.id}
            draggable
            onDragStart={(e) => e.dataTransfer.setData('text/kettd-task', task.id)}
          >
            <TaskRow
              task={aged ? { ...task, carriedFrom: age } : task}
              today={today}
              actions="inbox"
              selected={selected.has(task.id)}
              onSelect={toggleSelect}
              onToggle={(id) => { void runFor([id], (tid) => updateTask(tid, { done: true })) }}
              onPlanToday={(id) => { void planToday([id]) }}
              onSchedule={(id) => { void schedule([id]) }}
              onDelete={(id) => { void drop([id]) }}
            />
          </div>
        ))}
      </section>
    )
  }

  const isEmpty = !groups.fresh.length && !groups.week.length && !groups.older.length

  return (
    <div className="view">
      <div>
        <div className="view-title">收件箱</div>
        <div className="view-sub">还没安排的事 · 拖到右侧「今天」或用行尾动作处理 · 越久滞留越醒目（琥珀，非红）</div>
      </div>

      {saveErr && <div className="inline-err" role="alert">{saveErr}</div>}

      {selected.size > 0 && (
        <div className="batch-bar" role="toolbar" aria-label="批量处理">
          <span>已选 {selected.size}</span>
          <button className="btn xs outline" onClick={() => { void planToday(selectedIds) }}>加入今天</button>
          <button className="btn xs outline" onClick={() => { void schedule(selectedIds) }}>排期…</button>
          <button className="btn xs outline" onClick={() => { void drop(selectedIds) }}>删除</button>
          <span className="grow" />
          <button className="btn ghost xs" onClick={() => setSelected(new Set())}>取消</button>
        </div>
      )}

      {isEmpty && (
        <div className="empty">
          收件箱清空了 ✓ 新想法按 <kbd>Alt+Shift+A</kbd> 直接记，不带 #分类 !优先级 就是纯文本落这里。
        </div>
      )}

      <div style={{ display: 'flex', gap: 16, alignItems: 'flex-start' }}>
        <div style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: 16 }}>
          {renderGroup('今天进的', groups.fresh, false)}
          {renderGroup('本周', groups.week, false)}
          {renderGroup('更早 · 滞留 ≥5 天', groups.older, true)}
        </div>
        <div
          className={`drop-today${dragOver ? ' over' : ''}`}
          onDragOver={(e) => { e.preventDefault(); setDragOver(true) }}
          onDragLeave={() => setDragOver(false)}
          onDrop={(e) => {
            e.preventDefault()
            setDragOver(false)
            const id = e.dataTransfer.getData('text/kettd-task')
            if (id) { void planToday([id]) }
          }}
          aria-label="拖到今天"
        >
          拖到这里<br />= 加入今天
        </div>
      </div>
    </div>
  )
}

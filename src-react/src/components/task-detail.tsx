import { useEffect, useRef } from 'react'
import { X } from 'lucide-react'
import type { Task } from '@/lib/api'
import { formatDue, getCategoryColor, PRIORITY_LABEL } from '@/lib/api'

interface Props {
  task: Task | null
  today?: string
  onClose: () => void
  onDelete?: (id: string) => void
}

/**
 * 任务详情层（today-list-ui-spec §A.2「⋯(详情/删除)」+ Enter 展开详情的承载）。
 * 只读概览 + 删除动作；编辑仍在行内/hover 动作（保持「默认只读」行语言）。
 */
export function TaskDetail({ task, today, onClose, onDelete }: Props) {
  const closeRef = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    if (!task) return
    closeRef.current?.focus()
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [task, onClose])

  if (!task) return null
  return (
    <div className="detail-overlay" onClick={onClose}>
      <div
        className="detail-card"
        role="dialog"
        aria-modal="true"
        aria-label={`任务详情：${task.title}`}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="detail-head">
          <b className="grow truncate">{task.title}</b>
          <button ref={closeRef} className="btn ghost xs" onClick={onClose} aria-label="关闭详情">
            <X size={13} aria-hidden />
          </button>
        </div>

        <div className="detail-grid">
          <span>分类</span>
          <span><span className={`cat-chip ${getCategoryColor(task.category)}`}>{task.category}</span></span>
          <span>优先级</span>
          <span>{PRIORITY_LABEL[task.priority] ?? task.priority}</span>
          <span>截止</span>
          <span>{formatDue(task.dueAt, today).text}</span>
          {task.remindAt && (
            <>
              <span>提醒</span>
              <code className="tmeta mono">{task.remindAt}</code>
            </>
          )}
          <span>状态</span>
          <span>
            {task.done
              ? `已完成${task.doneAt ? ` · ${String(task.doneAt).slice(0, 16).replace('T', ' ')}` : ''}`
              : task.deletedAt
                ? '在回收站'
                : '进行中'}
          </span>
        </div>

        {task.note && <p className="detail-note">{task.note}</p>}

        {task.subtasks.length > 0 && (
          <ul className="detail-list" aria-label="子任务">
            {task.subtasks.map((st) => (
              <li key={st.id} className={st.done ? 'did' : ''}>{st.done ? '✓' : '·'} {st.title}</li>
            ))}
          </ul>
        )}
        {task.notes.length > 0 && (
          <ul className="detail-list" aria-label="备注">
            {task.notes.map((n) => (
              <li key={n.id}>{n.content}<span className="text-muted"> · {String(n.createdAt).slice(0, 10)}</span></li>
            ))}
          </ul>
        )}

        <div className="detail-meta text-muted">
          创建 {String(task.createdAt).slice(0, 16).replace('T', ' ')} · 更新 {String(task.updatedAt).slice(0, 16).replace('T', ' ')}
        </div>

        {onDelete && !task.deletedAt && (
          <div className="detail-actions">
            <button
              className="btn xs outline"
              onClick={() => { onDelete(task.id); onClose() }}
            >
              删除
            </button>
          </div>
        )}
      </div>
    </div>
  )
}

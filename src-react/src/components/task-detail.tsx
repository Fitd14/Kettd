import { useEffect, useRef, useState } from 'react'
import { X } from 'lucide-react'
import type { Task } from '@/lib/api'
import { formatDue, getCategoryColor, PRIORITY_LABEL, getKbItems, updateTask } from '@/lib/api'
import type { KbItem } from '@/lib/api'
import { KbAttachPicker } from '@/components/kb/kb-attach-picker'

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
  const [kbItems, setKbItems] = useState<Map<string, KbItem>>(new Map())
  const [showAttachPicker, setShowAttachPicker] = useState(false)

  const kbRefs: string[] = task?.kbRefs ?? []

  // 加载 KB 条目标题（用于展示）
  useEffect(() => {
    if (kbRefs.length === 0) return
    getKbItems().then((r) => {
      const items = r.data ?? []
      setKbItems(new Map(items.map((i) => [i.id, i])))
    })
  }, [kbRefs.join(',')])

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

        {/* KB 挂载资料区 */}
        <div className="detail-section">
          <span className="detail-label">挂载资料 {kbRefs.length > 0 && `(${kbRefs.length})`}</span>
          <div className="detail-kb-refs">
            {kbRefs.map((ref) => {
              const item = kbItems.get(ref)
              return (
                <span key={ref} className={`kb-ref-chip ${item ? '' : 'invalid'}`}>
                  📎 {item?.title ?? '已失效'}
                  {item && (
                    <button
                      className="btn xs ghost"
                      onClick={() => { window.location.hash = `#/kb?id=${ref}` }}
                      title="跳转到知识库"
                    >↗</button>
                  )}
                  <button
                    className="btn xs ghost"
                    onClick={async () => {
                      const next = kbRefs.filter((r) => r !== ref)
                      await updateTask(task.id, { kbRefs: next })
                    }}
                    title="卸载"
                  >×</button>
                </span>
              )
            })}
            <button
              className="btn xs ghost"
              onClick={() => setShowAttachPicker(true)}
            >
              + 挂载
            </button>
          </div>
        </div>

        {showAttachPicker && (
          <KbAttachPicker
            currentRefs={kbRefs}
            onAttach={async (ids) => { await updateTask(task.id, { kbRefs: ids }) }}
            onClose={() => setShowAttachPicker(false)}
          />
        )}

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

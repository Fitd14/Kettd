import { useState } from 'react'
import { addTask, trackEvent } from '@/lib/api'
import type { KbItem } from '@/lib/api'

interface Props {
  item: KbItem
  onClose: () => void
  onCreated: () => void
}

/**
 * 「条目→建待办」弹窗。
 * 预填标题=条目标题，分类默认「工作」。创建后自动挂 kbRefs（两步调用：addTask+updateTask）。
 */
export function KbNewTaskDialog({ item, onClose, onCreated }: Props) {
  const [title, setTitle] = useState(item.title)
  const [category, setCategory] = useState('工作')
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const handleSubmit = async () => {
    if (!title.trim()) return
    setSubmitting(true)
    setError(null)
    try {
      // add_task 经 apply_task_patch 天然接受 kbRefs（qa-6 复核：无需两步）
      const result = await addTask({ title: title.trim(), category, note: '', kbRefs: [item.id] })
      if (result.data?.id) {
        void trackEvent('kb_item_to_task', { itemId: item.id })
      }
      onCreated()
      onClose()
    } catch (e) {
      setError(String(e))
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="dialog" onClick={(e) => e.stopPropagation()}>
        <h3 style={{ margin: '0 0 0.75rem' }}>转为待办</h3>

        <label style={{ fontSize: '0.85em', color: 'var(--muted-foreground)' }}>标题</label>
        <input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          autoFocus
          style={{ width: '100%', marginBottom: '0.5rem' }}
        />

        <label style={{ fontSize: '0.85em', color: 'var(--muted-foreground)' }}>分类</label>
        <select
          value={category}
          onChange={(e) => setCategory(e.target.value)}
          style={{ width: '100%', marginBottom: '0.75rem' }}
        >
          <option>工作</option>
          <option>学习</option>
          <option>生活</option>
        </select>

        <p style={{ fontSize: '0.8em', color: 'var(--muted-foreground)', margin: '0 0 0.75rem' }}>
          创建后自动挂载到此知识条目
        </p>

        {error && (
          <p style={{ color: 'var(--danger, #e53e3e)', fontSize: '0.85em', margin: '0 0 0.5rem' }}>
            {error}
          </p>
        )}

        <div className="dialog-actions" style={{ display: 'flex', gap: '0.5rem', justifyContent: 'flex-end' }}>
          <button className="btn ghost" onClick={onClose}>取消</button>
          <button className="btn primary" disabled={submitting || !title.trim()} onClick={handleSubmit}>
            {submitting ? '创建中...' : '创建并挂载'}
          </button>
        </div>
      </div>
    </div>
  )
}

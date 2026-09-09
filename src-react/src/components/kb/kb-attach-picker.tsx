import { useState, useEffect, useMemo } from 'react'
import { getKbItems } from '@/lib/api'
import type { KbItem } from '@/lib/api'

interface Props {
  currentRefs: string[]
  onAttach: (ids: string[]) => void
  onClose: () => void
}

/**
 * 任务→KB 资料挂载搜索弹层。
 * 从 KB 全量条目中搜索选择，提交后整体替换任务的 kbRefs。
 */
export function KbAttachPicker({ currentRefs, onAttach, onClose }: Props) {
  const [all, setAll] = useState<KbItem[]>([])
  const [query, setQuery] = useState('')
  const [selected, setSelected] = useState<Set<string>>(() => new Set(currentRefs))

  useEffect(() => {
    getKbItems().then((r) => setAll(r.data ?? []))
  }, [])

  const filtered = useMemo(() => {
    const q = query.toLowerCase()
    if (!q) return all
    return all.filter(
      (i) =>
        i.title.toLowerCase().includes(q) ||
        i.tags.some((t) => t.toLowerCase().includes(q)),
    )
  }, [all, query])

  const toggle = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="dialog" onClick={(e) => e.stopPropagation()} style={{ maxWidth: 400 }}>
        <h3 style={{ margin: '0 0 0.5rem' }}>挂载资料</h3>
        <input
          type="search"
          placeholder="搜索知识库..."
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          autoFocus
          style={{ width: '100%', marginBottom: '0.5rem' }}
        />
        <div className="kb-attach-list" style={{ maxHeight: 300, overflow: 'auto' }}>
          {filtered.length === 0 && (
            <p style={{ color: 'var(--muted-foreground)', textAlign: 'center', padding: '1rem' }}>
              {query ? '没找到' : '知识库为空'}
            </p>
          )}
          {filtered.map((item) => (
            <label
              key={item.id}
              className="kb-attach-item"
              style={{
                display: 'flex',
                gap: '0.5em',
                padding: '0.35em 0.25em',
                cursor: 'pointer',
                borderRadius: 4,
              }}
            >
              <input
                type="checkbox"
                checked={selected.has(item.id)}
                onChange={() => toggle(item.id)}
              />
              <span style={{ flex: 1 }}>{item.title}</span>
              {item.tags?.slice(0, 2).map((t) => (
                <span key={t} className="kb-tag-chip">{t}</span>
              ))}
            </label>
          ))}
        </div>
        <div className="dialog-actions" style={{ display: 'flex', gap: '0.5rem', justifyContent: 'flex-end', marginTop: '0.75rem' }}>
          <button className="btn ghost" onClick={onClose}>取消</button>
          <button className="btn primary" onClick={() => { onAttach(Array.from(selected)); onClose() }}>
            挂载 ({selected.size})
          </button>
        </div>
      </div>
    </div>
  )
}

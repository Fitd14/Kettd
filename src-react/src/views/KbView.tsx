import { useState, useEffect, useCallback, useMemo } from 'react'
import { getKbItems, searchKb } from '@/lib/api'
import type { KbItem } from '@/lib/api'
import './kb.css'

interface Props {
  boot?: unknown
}

/**
 * 知识库视图（#/kb，第六视图）。
 *
 * 布局：搜索框 → 左清单（标题/标签/时间） → 右详情（阅读态 ⇄ WYSIWYG 编辑态）。
 * 搜索策略：前端持全量 items（≤5k 内存安全），searchKb 返回排序 ID 列表后重排。
 */
export function KbView(_props: Props) {
  const [items, setItems] = useState<KbItem[]>([])
  const [query, setQuery] = useState('')
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const result = await getKbItems()
      const all = result.data ?? []
      if (query.trim()) {
        const rankedResult = await searchKb(query.trim())
        const ranked = rankedResult.data ?? []
        // 补充未命中的条目（防御性）
        const rankedIds = new Set(ranked.map((i) => i.id))
        const rest = all.filter((i) => !rankedIds.has(i.id))
        setItems([...ranked, ...rest])
      } else {
        // 无查询：按更新时间倒序
        setItems([...all].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt)))
      }
    } catch (e) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }, [query])

  useEffect(() => { void refresh() }, [refresh])

  const selected = useMemo(
    () => items.find((i) => i.id === selectedId) ?? null,
    [items, selectedId],
  )

  return (
    <div className="kb-view">
      {/* 搜索栏 */}
      <div className="kb-search-bar">
        <input
          type="search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="搜索知识库..."
          aria-label="搜索知识库"
        />
        {query && (
          <span className="kb-result-count">{items.length} 条结果</span>
        )}
      </div>

      {/* 标签筛选（Task 4 填充） */}
      {/* <KbTagFilter ... /> */}

      {/* 主布局 */}
      <div className="kb-layout">
        {/* 左清单 */}
        <div className="kb-list" role="listbox" aria-label="知识条目">
          {loading && <div className="kb-empty">加载中...</div>}
          {!loading && error && (
            <div className="kb-empty">
              <span className="empty-icon">⚠️</span>
              <span>{error}</span>
            </div>
          )}
          {!loading && !error && items.length === 0 && (
            <div className="kb-empty">
              <span className="empty-icon">📚</span>
              <span>{query ? '没找到' : '知识库空空如也'}</span>
            </div>
          )}
          {items.map((item) => (
            <div
              key={item.id}
              className={`kb-item-row${selectedId === item.id ? ' active' : ''}`}
              role="option"
              aria-selected={selectedId === item.id}
              onClick={() => setSelectedId(item.id)}
            >
              <span className="item-title">{item.title}</span>
              <span className="item-meta">
                {item.tags?.slice(0, 2).map((t) => (
                  <span key={t} className="kb-tag-chip">{t}</span>
                ))}
                <span>{relativeTime(item.updatedAt)}</span>
              </span>
            </div>
          ))}
        </div>

        {/* 右详情面板（Task 5 填充 WYSIWYG 编辑） */}
        <div className="kb-detail">
          {!selected && !loading && items.length > 0 && (
            <div className="kb-empty">选择一条知识查看详情</div>
          )}
          {selected && (
            <div>
              <div className="kb-detail-toolbar">
                <h3 className="kb-detail-title">{selected.title}</h3>
              </div>
              <p style={{ color: 'var(--muted-foreground)', fontSize: '0.9em' }}>
                {selected.bodyMd ? `${selected.bodyMd.length} 字` : '空白'}
              </p>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

function relativeTime(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime()
  const mins = Math.floor(diff / 60000)
  if (mins < 60) return `${mins}分钟前`
  const hrs = Math.floor(mins / 60)
  if (hrs < 24) return `${hrs}小时前`
  const days = Math.floor(hrs / 24)
  return `${days}天前`
}

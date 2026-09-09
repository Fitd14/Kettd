import { useState, useEffect, useCallback, useMemo } from 'react'
import { getKbItems, searchKb, addKbItem, deleteKbItem } from '@/lib/api'
import type { KbItem } from '@/lib/api'
import { KbSearchBar } from '@/components/kb/kb-search-bar'
import { KbTagFilter } from '@/components/kb/kb-tag-filter'
import { KbItemRow } from '@/components/kb/kb-item-row'
import { KbEmptyState } from '@/components/kb/kb-empty-state'
import './kb.css'

interface Props {
  boot?: unknown
}

/**
 * 知识库视图（#/kb，第六视图）。
 *
 * 布局：搜索框 + 标签筛选 → 左清单 → 右详情（阅读态 ⇄ WYSIWYG 编辑态）。
 * 搜索策略：前端持全量 items（≤5k 内存安全），searchKb 返回排序 items 后重排。
 * 标签筛选：前端对全量 items 做 client-side 过滤（不发新命令）。
 */
export function KbView(_props: Props) {
  const [items, setItems] = useState<KbItem[]>([])
  const [query, setQuery] = useState('')
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [activeTag, setActiveTag] = useState<string | null>(null)
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
        const rankedIds = new Set(ranked.map((i) => i.id))
        const rest = all.filter((i) => !rankedIds.has(i.id))
        setItems([...ranked, ...rest])
      } else {
        setItems([...all].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt)))
      }
    } catch (e) {
      setError(String(e))
    } finally {
      setLoading(false)
    }
  }, [query])

  useEffect(() => { void refresh() }, [refresh])

  // 标签筛选（client-side）
  const filteredItems = useMemo(() => {
    if (!activeTag) return items
    return items.filter((i) => i.tags.includes(activeTag))
  }, [items, activeTag])

  // 全局唯一标签列表
  const allTags = useMemo(() => {
    const tagSet = new Set<string>()
    for (const item of items) {
      for (const t of item.tags) tagSet.add(t)
    }
    return [...tagSet].sort()
  }, [items])

  const selected = useMemo(
    () => items.find((i) => i.id === selectedId) ?? null,
    [items, selectedId],
  )

  // 新建条目
  const handleCreate = useCallback(async () => {
    const title = `新建知识 ${new Date().toLocaleDateString('zh-CN')}`
    const result = await addKbItem({ title, bodyMd: '', tags: [] })
    if (result.data) {
      await refresh()
      setSelectedId(result.data.id)
    }
  }, [refresh])

  // 删除条目
  const handleDelete = useCallback(async (id: string) => {
    await deleteKbItem(id)
    if (selectedId === id) setSelectedId(null)
    await refresh()
  }, [selectedId, refresh])

  // 更新条目（来自右详情面板——Task 5 接入）
  // const handleUpdate = useCallback(async (id: string, patch: { title?: string; bodyMd?: string; tags?: string[] }) => {
  //   await updateKbItem(id, patch)
  //   await refresh()
  // }, [refresh])

  return (
    <div className="kb-view">
      <KbSearchBar
        query={query}
        onChange={setQuery}
        resultCount={filteredItems.length}
      />

      <KbTagFilter
        tags={allTags}
        activeTag={activeTag}
        onSelect={setActiveTag}
      />

      <div className="kb-layout">
        {/* 左清单 */}
        <div className="kb-list" role="listbox" aria-label="知识条目">
          {loading && <KbEmptyState variant="empty" />}
          {!loading && error && <KbEmptyState variant="error" errorMessage={error} />}
          {!loading && !error && filteredItems.length === 0 && (
            <KbEmptyState
              variant={query || activeTag ? 'no-results' : 'empty'}
              onCreate={handleCreate}
            />
          )}
          {filteredItems.map((item) => (
            <KbItemRow
              key={item.id}
              item={item}
              isActive={selectedId === item.id}
              onSelect={setSelectedId}
            />
          ))}
        </div>

        {/* 右详情面板 */}
        <div className="kb-detail">
          {!selected && !loading && filteredItems.length > 0 && (
            <div className="kb-empty">选择一条知识查看详情</div>
          )}
          {selected && (
            <div>
              <div className="kb-detail-toolbar">
                <h3 className="kb-detail-title">{selected.title}</h3>
                <span style={{ flex: 1 }} />
                <button className="btn xs primary" onClick={handleCreate}>+ 新建</button>
                <button
                  className="btn xs danger"
                  onClick={() => handleDelete(selected.id)}
                >
                  🗑 删除
                </button>
              </div>
              <p style={{ color: 'var(--muted-foreground)', fontSize: '0.9em' }}>
                {selected.bodyMd ? `${selected.bodyMd.length} 字 · ` : '空白 · '}
                标签: {selected.tags?.join(', ') || '无'}
              </p>
              {/* Task 5: TipTap WYSIWYG editor + MdStaticRenderer read/edit toggle */}
              <div className="kb-detail-body">
                <p style={{ fontSize: '0.85em', color: 'var(--muted-foreground)' }}>
                  WYSIWYG 编辑器将在 Task 5 接入
                </p>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

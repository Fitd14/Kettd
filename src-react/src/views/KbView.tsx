import { useState, useEffect, useCallback, useMemo } from 'react'
import { getKbItems, searchKb, addKbItem, createSticky, trackEvent, searchKbHybrid } from '@/lib/api'
import type { KbItem } from '@/lib/api'
import { KbSearchBar } from '@/components/kb/kb-search-bar'
import { KbTagFilter } from '@/components/kb/kb-tag-filter'
import { KbItemRow } from '@/components/kb/kb-item-row'
import { KbEmptyState } from '@/components/kb/kb-empty-state'
import { KbItemDetail } from '@/components/kb/kb-item-detail'
import { KbNewTaskDialog } from '@/components/kb/kb-new-task-dialog'
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
  const [createTaskItem, setCreateTaskItem] = useState<KbItem | null>(null)
  const [aiHits, setAiHits] = useState<Set<string>>(new Set())

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    setAiHits(new Set())
    try {
      const result = await getKbItems()
      const all = result.data ?? []
      if (query.trim()) {
        // 尝试混合检索（AI 徽标）
        const hybridResult = await searchKbHybrid(query.trim())
        if (hybridResult.data && hybridResult.data.length > 0) {
          // 混合结果带 source 标记
          const idMap = new Map(all.map((i) => [i.id, i]))
          const ranked = hybridResult.data
            .map((r) => idMap.get(r.id))
            .filter((i): i is KbItem => !!i)
          const rankedIds = new Set(ranked.map((i) => i.id))
          const rest = all.filter((i) => !rankedIds.has(i.id))
          setItems([...ranked, ...rest])
          // 标记 AI 命中
          const aiIds = new Set(hybridResult.data.filter((r) => r.source === 'ai').map((r) => r.id))
          setAiHits(aiIds)
        } else {
          // 回落纯本地
          const rankedResult = await searchKb(query.trim())
          const ranked = rankedResult.data ?? []
          const rankedIds = new Set(ranked.map((i) => i.id))
          const rest = all.filter((i) => !rankedIds.has(i.id))
          setItems([...ranked, ...rest])
        }
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

  // Deep Link：#/kb?id={itemId} → 自动选中该条目
  useEffect(() => {
    const hash = window.location.hash
    const idMatch = hash.match(/[?&]id=([^&]+)/)
    if (idMatch) {
      const targetId = decodeURIComponent(idMatch[1])
      // 等 items 加载后选中
      const trySelect = () => {
        const found = items.find((i) => i.id === targetId)
        if (found) setSelectedId(targetId)
      }
      trySelect()
      // items 可能还没加载，延迟重试
      const timer = setTimeout(trySelect, 500)
      return () => clearTimeout(timer)
    }
  }, [items])

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
              isAiHit={aiHits.has(item.id)}
            />
          ))}
        </div>

        {/* 右详情面板 */}
        <div className="kb-detail">
          {!selected && !loading && filteredItems.length > 0 && (
            <div className="kb-empty">选择一条知识查看详情</div>
          )}
          {selected && (
            <KbItemDetail
              item={selected}
              onDeleted={(id) => { if (selectedId === id) setSelectedId(null); void refresh() }}
              onRefresh={refresh}
              onCreateTask={(item) => setCreateTaskItem(item)}
              onPin={async (item) => {
                const result = await createSticky({ kbRef: item.id })
                if (!result.err) {
                  void trackEvent('kb_pin_desktop', { itemId: item.id })
                }
              }}
            />
          )}
        </div>
      </div>

      {createTaskItem && (
        <KbNewTaskDialog
          item={createTaskItem}
          onClose={() => setCreateTaskItem(null)}
          onCreated={refresh}
        />
      )}
    </div>
  )
}

import { useState, useEffect, useCallback, useMemo, useRef } from 'react'
import { getKbItems, searchKb, addKbItem, createSticky, trackEvent, searchKbHybrid } from '@/lib/api'
import type { KbItem, HybridResult } from '@/lib/api'
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
  // 左侧清单折叠（真机反馈：太宽影响编辑）；localStorage 持久化
  const [listCollapsed, setListCollapsed] = useState(
    () => localStorage.getItem('kb.listCollapsed') === '1',
  )

  const toggleList = useCallback(() => {
    setListCollapsed((prev) => {
      localStorage.setItem('kb.listCollapsed', prev ? '0' : '1')
      return !prev
    })
  }, [])

  // 渐进增强检索（kb-qa-fixes qa-1 拍板）：
  // 第一跳 键入即本地渲染（零延迟）→ 第二跳 停 400ms 后 hybrid 合并 + AI 徽标。
  // 同词 60s 缓存不重复计费；失败后 60s 冷却；请求序号防竞态（只认最新）。
  const hybridCacheRef = useRef(new Map<string, { ts: number; data: HybridResult[] }>())
  const failTsRef = useRef(0)
  const seqRef = useRef(0)
  const itemsRef = useRef<KbItem[]>([])
  itemsRef.current = items

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    setAiHits(new Set())
    try {
      const result = await getKbItems()
      const all = result.data ?? []
      const q = query.trim()
      if (q) {
        // 第一跳：本地扫，立即渲染
        const rankedResult = await searchKb(q)
        const ranked = rankedResult.data ?? []
        const rankedIds = new Set(ranked.map((i) => i.id))
        const rest = all.filter((i) => !rankedIds.has(i.id))
        setItems([...ranked, ...rest])
        void trackEvent('kb_search', { hits: ranked.length })
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

  // 第二跳：停 400ms 后语义召回，到达合并（渐进增强）
  useEffect(() => {
    const q = query.trim()
    if (!q) {
      seqRef.current += 1 // 使在途请求失效
      return
    }
    const seq = ++seqRef.current
    const timer = setTimeout(async () => {
      const now = Date.now()
      if (now - failTsRef.current < 60_000) return // 失败冷却期
      const cached = hybridCacheRef.current.get(q)
      if (cached && now - cached.ts < 60_000) {
        applyHybrid(cached.data)
        return
      }
      try {
        const r = await searchKbHybrid(q)
        if (seq !== seqRef.current) return // 过期响应丢弃
        const data = r.data ?? []
        hybridCacheRef.current.set(q, { ts: Date.now(), data })
        applyHybrid(data)
      } catch {
        if (seq === seqRef.current) failTsRef.current = Date.now()
      }
    }, 400)
    return () => clearTimeout(timer)

    function applyHybrid(data: HybridResult[]) {
      const aiIds = new Set(data.filter((r) => r.source === 'ai').map((r) => r.id))
      setAiHits(aiIds)
      if (aiIds.size === 0) return
      // 用混合顺序重排（hybrid 返回已按合并分排序）
      const idMap = new Map(itemsRef.current.map((i) => [i.id, i]))
      const ranked = data.map((r) => idMap.get(r.id)).filter((i): i is KbItem => !!i)
      const rankedIds = new Set(ranked.map((i) => i.id))
      const rest = itemsRef.current.filter((i) => !rankedIds.has(i.id))
      setItems([...ranked, ...rest])
    }
  }, [query])

  // Deep Link：#/kb?id={itemId} → 自动选中该条目。
  // 监听 hashchange：同页内从任务详情切不同 id 也能响应（route 不变但查询串变了）。
  const pendingDeepRef = useRef<string | null>(null)
  useEffect(() => {
    const applyDeepLink = () => {
      const idMatch = window.location.hash.match(/[?&]id=([^&]+)/)
      if (!idMatch) return
      const targetId = decodeURIComponent(idMatch[1])
      // items 可能尚未加载完成：记下 pending，items 到位后由下方 effect 兜底选中
      pendingDeepRef.current = targetId
      if (itemsRef.current.some((i) => i.id === targetId)) {
        setSelectedId(targetId)
        pendingDeepRef.current = null
      }
    }
    applyDeepLink()
    window.addEventListener('hashchange', applyDeepLink)
    return () => window.removeEventListener('hashchange', applyDeepLink)
  }, [])

  // items 变化后重试未完成的 Deep Link 选中
  useEffect(() => {
    const target = pendingDeepRef.current
    if (!target) return
    if (items.some((i) => i.id === target)) {
      setSelectedId(target)
      pendingDeepRef.current = null
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
      void trackEvent('kb_create', {})
      await refresh()
      setSelectedId(result.data.id)
    }
  }, [refresh])

  return (
    <div className="kb-view">
      <div className="kb-topbar">
        <KbSearchBar
          query={query}
          onChange={setQuery}
          resultCount={filteredItems.length}
        />
        <button className="btn xs primary kb-new-btn" onClick={handleCreate} title="新建知识条目">
          + 新建
        </button>
      </div>

      {!listCollapsed && (
        <KbTagFilter
          tags={allTags}
          activeTag={activeTag}
          onSelect={setActiveTag}
        />
      )}

      <div className={`kb-layout${listCollapsed ? ' is-collapsed' : ''}`}>
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
          {/* 折叠清单切换（常驻，未选中条目也可用） */}
          <button
            className="kb-list-toggle"
            onClick={toggleList}
            title={listCollapsed ? '展开列表' : '收起列表'}
            aria-label={listCollapsed ? '展开列表' : '收起列表'}
          >
            {listCollapsed ? '⟩' : '⟨'}
          </button>
          {listCollapsed && (
            <button className="kb-list-float-open" onClick={toggleList} title="展开列表" aria-label="展开列表">
              ⟩ 列表
            </button>
          )}
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

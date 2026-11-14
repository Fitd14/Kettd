import { useState, useEffect, useCallback, useMemo, useRef } from 'react'
import { AlertTriangle, FolderOpen } from 'lucide-react'
import {
  getKbItems,
  searchKb,
  addKbItem,
  createSticky,
  trackEvent,
  listFolders,
  createFolder,
  renameFolder,
  deleteFolder,
  openDataFolder,
} from '@/lib/api'
import type { Bootstrap, KbItem, KbFolder, KbPayload } from '@/lib/api'
import { KbSearchBar } from '@/components/kb/kb-search-bar'
import { KbTagFilter } from '@/components/kb/kb-tag-filter'
import { KbItemRow } from '@/components/kb/kb-item-row'
import { KbEmptyState } from '@/components/kb/kb-empty-state'
import { KbItemDetail } from '@/components/kb/kb-item-detail'
import { KbNewTaskDialog } from '@/components/kb/kb-new-task-dialog'
import { KbFolderTree } from '@/components/kb/kb-folder-tree'
import type { FolderSelection, KbFolderCounts } from '@/components/kb/kb-folder-tree'
import { KbFolderDialog } from '@/components/kb/kb-folder-dialog'
import './kb.css'

interface Props {
  boot?: Bootstrap
}

/** 新建 / 重命名共用弹窗的上下文（父级：create=目标父夹；rename=目标 folder） */
type FolderDialogState =
  | { mode: 'create'; parentId: string | null }
  | { mode: 'rename'; id: string; name: string }

/**
 * 知识库视图（#/kb，第六视图）。
 *
 * 布局：搜索框 + 标签筛选 → 左清单（文件夹树 + 条目）→ 右详情（阅读态 ⇄ WYSIWYG 编辑态）。
 * 搜索策略：前端持全量 items（≤5k 内存安全），searchKb 返回排序 items 后重排。
 * 标签筛选：前端对全量 items 做 client-side 过滤（不发新命令）。
 * 文件夹筛选（任务 #4）：左栏树选中「全部 / 未分类 / 具体文件夹」，同样 client-side 过滤；
 * 损坏错误态：getKbItems/listFolders 返回 err 时显示错误横幅，保留旧数据，绝不渲染假空态。
 */
export function KbView({ boot }: Props) {
  const [items, setItems] = useState<KbItem[]>([])
  const [query, setQuery] = useState('')
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [activeTag, setActiveTag] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  /** 条目加载错误（含知识库损坏）：保留旧列表，只出错误横幅（#4 受损文件错误态） */
  const [loadError, setLoadError] = useState<string | null>(null)
  const [createTaskItem, setCreateTaskItem] = useState<KbItem | null>(null)
  const [actionErr, setActionErr] = useState<string | null>(null)
  // 左侧清单折叠（真机反馈：太宽影响编辑）；localStorage 持久化
  const [listCollapsed, setListCollapsed] = useState(
    () => localStorage.getItem('kb.listCollapsed') === '1',
  )

  // ── 文件夹树（任务 #4）─────────────────────────────────────────────────
  const [folders, setFolders] = useState<KbFolder[]>([])
  const [foldersReady, setFoldersReady] = useState(false)
  const [folderLoadErr, setFolderLoadErr] = useState<string | null>(null)
  const [folderSelection, setFolderSelection] = useState<FolderSelection>({ kind: 'all' })
  const [folderDialog, setFolderDialog] = useState<FolderDialogState | null>(null)

  const toggleList = useCallback(() => {
    setListCollapsed((prev) => {
      localStorage.setItem('kb.listCollapsed', prev ? '0' : '1')
      return !prev
    })
  }, [])

  /** 折叠态点击条目：选中并展开回完整列表（#13 app-shell：点击标题展开） */
  const handleSelectItem = useCallback((id: string) => {
    setSelectedId(id)
    setListCollapsed((prev) => {
      if (!prev) return prev
      localStorage.setItem('kb.listCollapsed', '0')
      return false
    })
  }, [])

  /** 保存成功后局部回填单条（#1：不再全量 refresh，不闪空态、不重排列表） */
  const handleSaved = useCallback((saved: KbItem) => {
    setItems((prev) => prev.map((it) => (it.id === saved.id ? saved : it)))
  }, [])

  const itemsRef = useRef<KbItem[]>([])
  itemsRef.current = items

  const refresh = useCallback(async () => {
    setLoading(true)
    try {
      const result = await getKbItems()
      // 加载失败 / 知识库损坏：显示错误横幅并保留旧列表，绝不降级成「空空如也」假空态
      if (result.err) {
        setLoadError(result.err)
        return
      }
      const all = result.data ?? []
      const q = query.trim()
      if (q) {
        // 本地全文扫，立即渲染
        const rankedResult = await searchKb(q)
        const ranked = rankedResult.data ?? []
        const rankedIds = new Set(ranked.map((i) => i.id))
        const rest = all.filter((i) => !rankedIds.has(i.id))
        setItems([...ranked, ...rest])
        void trackEvent('kb_search', { hits: ranked.length })
      } else {
        setItems([...all].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt)))
      }
      setLoadError(null)
    } catch (e) {
      setLoadError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }, [query])

  const refreshFolders = useCallback(async () => {
    try {
      const result = await listFolders()
      if (result.err) {
        setFolderLoadErr(result.err)
        return
      }
      setFolders(result.data ?? [])
      setFolderLoadErr(null)
    } catch (e) {
      setFolderLoadErr(e instanceof Error ? e.message : String(e))
    } finally {
      setFoldersReady(true)
    }
  }, [])

  useEffect(() => { void refresh() }, [refresh])
  useEffect(() => { void refreshFolders() }, [refreshFolders])

  // 当前选中的文件夹已不存在（被本窗/别处删除）→ 回退「全部知识」。
  // foldersReady 之前不校正（首载 folders 还是空数组，避免把合法选择误伤成 all）。
  useEffect(() => {
    if (!foldersReady || folderSelection.kind !== 'folder') return
    if (!folders.some((f) => f.id === folderSelection.id)) {
      setFolderSelection({ kind: 'all' })
    }
  }, [folders, foldersReady, folderSelection])

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

  // #6 Ctrl/Cmd+Shift+F：KbView 挂载期间聚焦知识库搜索框（全库搜索键盘入口）。
  // 捕获阶段监听 window：先于编辑器 / WebView2 默认按键处理，preventDefault 后可完全接管该组合键；
  // 只认 ctrl/meta+shift+F，不碰普通输入；isComposing / keyCode 229 时直接放行（输入法组合态）。
  const viewRef = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.isComposing || e.keyCode === 229) return // 输入法组合态不拦截
      if (!(e.ctrlKey || e.metaKey) || !e.shiftKey || e.altKey) return
      if (e.key.toLowerCase() !== 'f' && e.code !== 'KeyF') return
      const root = viewRef.current
      if (!root) return
      // 仅限本视图内（含无焦点的 body）；焦点在侧栏/其他视图或模态弹窗内时不抢焦点
      const active = document.activeElement
      if (active && active !== document.body && !root.contains(active)) return
      // spec（organization/检索快捷键）：普通文本输入框内 MUST NOT 误触发，交还浏览器/原生行为；
      // 编辑器正文是 contenteditable（不属于 input/textarea），仍允许用快捷键跳到全库搜索。
      if (active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement) return
      if (root.querySelector('.dialog-overlay')) return
      const input = root.querySelector<HTMLInputElement>('.kb-search-bar input')
      if (!input) return
      e.preventDefault()
      e.stopPropagation()
      // 属性上完全接管该组合键：避免并行改动的编辑器 Ctrl+F 处理器把 Ctrl+Shift+F 也当作条目内查找
      e.stopImmediatePropagation()
      if (document.activeElement === input) {
        input.select() // 已聚焦：保持聚焦并全选，便于直接替换检索词
      } else {
        input.focus()
      }
    }
    window.addEventListener('keydown', onKeyDown, true)
    return () => window.removeEventListener('keydown', onKeyDown, true)
  }, [])

  // 筛选（client-side）：标签 → 文件夹（未分类 = folderId 为空/指向已删文件夹；具体文件夹 = 精确匹配）
  const knownFolderIds = useMemo(() => new Set(folders.map((f) => f.id)), [folders])
  const filteredItems = useMemo(() => {
    let list = items
    if (activeTag) list = list.filter((i) => i.tags.includes(activeTag))
    if (folderSelection.kind === 'unfiled') {
      // 备份恢复后 folderId 可能悬空（指向已删除文件夹）→ 按未分类兜底展示，避免条目在树中消失
      list = list.filter((i) => !(i.folderId ?? null) || !knownFolderIds.has(i.folderId as string))
    } else if (folderSelection.kind === 'folder') {
      const folderId = folderSelection.id
      list = list.filter((i) => (i.folderId ?? null) === folderId)
    }
    return list
  }, [items, activeTag, folderSelection, knownFolderIds])

  // #5 选择校正（spec: 默认选中可见条目，D7）：items 加载完成 / filteredItems 变化后执行一次。
  // 优先级：deep link 待兑现 > 当前选中仍在可见列表（保持用户选择）> 可见列表第一项。
  // loading 期间不校正（列表可能还在换血，且 deep link 依赖 items 就绪）；filteredItems 为空时
  // 清空选中，让详情区走空态。只在值真的变化时 setState，避免 effect 自触发循环。
  useEffect(() => {
    if (loading) return

    // 1) deep link 尚未兑现：数据就绪后补选（必须优先于默认第一项，避免互相覆盖）
    const pending = pendingDeepRef.current
    if (pending) {
      if (items.some((i) => i.id === pending)) {
        pendingDeepRef.current = null
        setSelectedId(pending)
        return
      }
      // 目标不在数据中（条目已删除 / 非法 id）→ 放弃 pending，交给下面的默认兜底
      pendingDeepRef.current = null
    }

    // 2) 无可见条目 → 清空选中，详情区显示空态
    if (filteredItems.length === 0) {
      if (selectedId !== null) setSelectedId(null)
      return
    }

    // 3) 当前选中仍在可见列表 → 保持不变（不要每次刷新都重置用户的选择）
    if (selectedId !== null && filteredItems.some((i) => i.id === selectedId)) return

    // 4) 无选中 / 选中项被查询或标签/文件夹筛选掉 → 选中可见第一项
    setSelectedId(filteredItems[0].id)
  }, [loading, items, filteredItems, selectedId])

  // 全局唯一标签列表
  const allTags = useMemo(() => {
    const tagSet = new Set<string>()
    for (const item of items) {
      for (const t of item.tags) tagSet.add(t)
    }
    return [...tagSet].sort()
  }, [items])

  // 文件夹树计数：全部 / 未分类 / 各文件夹直属条目数（精确计数，不含子文件夹；
  // 悬空 folderId（指向已删文件夹）计入未分类，与筛选口径一致）
  const folderCounts = useMemo<KbFolderCounts>(() => {
    let unfiled = 0
    const byFolder = new Map<string, number>()
    for (const item of items) {
      const folderId = item.folderId ?? null
      if (!folderId || !knownFolderIds.has(folderId)) {
        unfiled += 1
      } else {
        byFolder.set(folderId, (byFolder.get(folderId) ?? 0) + 1)
      }
    }
    return { all: items.length, unfiled, byFolder }
  }, [items, knownFolderIds])

  const selected = useMemo(
    () => items.find((i) => i.id === selectedId) ?? null,
    [items, selectedId],
  )

  // 条目被任务引用数（boot 无 stickies，知识便签引用不计；删除确认提示用 qa-8）
  const refCountOf = useCallback((id: string): number => {
    if (!boot) return 0
    return boot.tasks.filter((t) => !t.deletedAt && t.kbRefs?.includes(id)).length
  }, [boot])

  // 新建条目：继承当前文件夹筛选（全部视图不传 → 后端默认；未分类传 ""；文件夹传 id）
  const handleCreate = useCallback(async () => {
    const title = `新建知识 ${new Date().toLocaleDateString('zh-CN')}`
    const payload: KbPayload = { title, bodyMd: '', tags: [] }
    if (folderSelection.kind === 'unfiled') payload.folderId = ''
    else if (folderSelection.kind === 'folder') payload.folderId = folderSelection.id
    const result = await addKbItem(payload)
    if (result.err) {
      setActionErr(result.err)
      return
    }
    if (result.data) {
      void trackEvent('kb_create', {})
      setActionErr(null)
      await refresh()
      setSelectedId(result.data.id)
    }
  }, [refresh, folderSelection])

  // #14 打开本地数据目录（失败局部错误提示）
  const handleOpenDataFolder = useCallback(async () => {
    const result = await openDataFolder()
    if (result.err) {
      setActionErr(result.err)
      return
    }
    setActionErr(null)
  }, [])

  // ── 文件夹树操作 ──────────────────────────────────────────────────────
  const handleCreateFolder = useCallback(() => {
    const parentId = folderSelection.kind === 'folder' ? folderSelection.id : null
    setFolderDialog({ mode: 'create', parentId })
  }, [folderSelection])

  const handleRenameFolder = useCallback((folder: KbFolder) => {
    setFolderDialog({ mode: 'rename', id: folder.id, name: folder.name })
  }, [])

  /** 弹窗提交：成功返回 null（父级关窗），失败返回后端文案（弹窗内联显示） */
  const handleFolderDialogSubmit = useCallback(
    async (name: string): Promise<string | null> => {
      if (!folderDialog) return '弹窗状态异常，请重试'
      const result = folderDialog.mode === 'create'
        ? await createFolder(name, folderDialog.parentId)
        : await renameFolder(folderDialog.id, name)
      if (result.err) return result.err
      setFolderDialog(null)
      setActionErr(null)
      await refreshFolders()
      return null
    },
    [folderDialog, refreshFolders],
  )

  /** 删除文件夹（tree 内已做两步确认）；失败把后端文案放进局部错误区 */
  const handleDeleteFolder = useCallback(async (folder: KbFolder) => {
    const result = await deleteFolder(folder.id)
    if (result.err) {
      setActionErr(result.err)
      return
    }
    setActionErr(null)
    if (folderSelection.kind === 'folder' && folderSelection.id === folder.id) {
      setFolderSelection({ kind: 'all' })
    }
    // 后端可能把原直属条目改判为未分类或级联删除 → 条目也重拉一次，避免残留旧归属
    await Promise.all([refreshFolders(), refresh()])
  }, [folderSelection, refreshFolders, refresh])

  return (
    <div className="kb-view" ref={viewRef}>
      <div className="kb-topbar">
        <KbSearchBar
          query={query}
          onChange={setQuery}
          resultCount={filteredItems.length}
        />
        <button
          className="btn xs ghost kb-open-dir-btn"
          onClick={() => void handleOpenDataFolder()}
          title="打开本地数据目录"
        >
          <FolderOpen size={13} strokeWidth={1.7} /> 打开数据目录
        </button>
        <button className="btn xs primary kb-new-btn" onClick={handleCreate} title="新建知识条目">
          + 新建
        </button>
      </div>

      {actionErr && (
        <div className="inline-err" role="alert">{actionErr}</div>
      )}

      {!listCollapsed && (
        <KbTagFilter
          tags={allTags}
          activeTag={activeTag}
          onSelect={setActiveTag}
        />
      )}

      <div className={`kb-layout${listCollapsed ? ' is-collapsed' : ''}`}>
        {/* 左清单：文件夹树 + 条目列表（各自滚动；折叠态 CSS 隐藏文件夹树） */}
        <div className="kb-list">
          {!listCollapsed && (
            <KbFolderTree
              folders={folders}
              selection={folderSelection}
              counts={folderCounts}
              loadError={folderLoadErr}
              onSelect={setFolderSelection}
              onCreate={handleCreateFolder}
              onRename={handleRenameFolder}
              onDelete={handleDeleteFolder}
            />
          )}
          {/* 加载失败 / 知识库损坏：错误横幅 + 重试；绝不渲染「空空如也」假空态。
              放在 listbox 之外（横幅不是选项），折叠态也始终可见。 */}
          {loadError && (
            <div className="kb-error-banner" role="alert">
              <AlertTriangle size={13} className="kb-error-ico" aria-hidden="true" />
              <span className="kb-error-text">{loadError}</span>
              <button
                type="button"
                className="kb-error-retry"
                onClick={() => void refresh()}
                title="重新加载知识库"
              >
                重试
              </button>
            </div>
          )}
          <div className="kb-item-list" role="listbox" aria-label="知识条目">
            {/* 加载中轻提示：绝对定位、不占位，不清空/顶掉旧列表（#1 保存闪烁） */}
            {loading && (
              <div className="kb-loading-hint" role="status" aria-live="polite">加载中…</div>
            )}
            {/* 空态只在确实没有任何条目时出现；有筛选无命中才给「没找到」 */}
            {!loading && !loadError && filteredItems.length === 0 && (
              <KbEmptyState
                variant={items.length === 0 && !query && !activeTag ? 'empty' : 'no-results'}
                onCreate={handleCreate}
              />
            )}
            {filteredItems.map((item) => (
              <KbItemRow
                key={item.id}
                item={item}
                isActive={selectedId === item.id}
                onSelect={handleSelectItem}
              />
            ))}
          </div>
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
          {!selected && !loading && filteredItems.length > 0 && (
            <div className="kb-empty">选择一条知识查看详情</div>
          )}
          {selected && (
            <KbItemDetail
              key={selected.id}
              item={selected}
              refCount={refCountOf(selected.id)}
              folders={folders}
              onDeleted={(id) => { if (selectedId === id) setSelectedId(null); void refresh() }}
              onSaved={handleSaved}
              onCreateTask={(item) => setCreateTaskItem(item)}
              onPin={async (item) => {
                const result = await createSticky({ kbRef: item.id })
                if (result.err) {
                  setActionErr(result.err)
                  return
                }
                setActionErr(null)
                void trackEvent('kb_pin_desktop', { itemId: item.id })
              }}
            />
          )}
        </div>
      </div>

      {folderDialog && (
        <KbFolderDialog
          title={folderDialog.mode === 'create' ? '新建文件夹' : '重命名文件夹'}
          initialName={folderDialog.mode === 'rename' ? folderDialog.name : ''}
          submitLabel={folderDialog.mode === 'create' ? '创建' : '保存'}
          onSubmit={handleFolderDialogSubmit}
          onCancel={() => setFolderDialog(null)}
        />
      )}

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
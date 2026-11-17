import { useState, useEffect, useCallback, useMemo, useRef } from 'react'
import {
  AlertTriangle,
  ChevronsDownUp,
  ChevronsUpDown,
  ClipboardPaste,
  Copy,
  FilePlus,
  FileText,
  FolderInput,
  FolderPlus,
  PencilLine,
  Pin,
  PinOff,
  Scissors,
  TextSelect,
  Trash2,
} from 'lucide-react'
import {
  getKbItems,
  searchKb,
  addKbItem,
  createSticky,
  deleteKbItem,
  trackEvent,
  listFolders,
  createFolder,
  updateFolder,
  updateKbItem,
  deleteFolder,
  openDataFolder,
  onEvent,
} from '@/lib/api'
import type { Bootstrap, KbItem, KbFolder, KbPayload } from '@/lib/api'
import { KbSearchBar } from '@/components/kb/kb-search-bar'
import { KbTagFilter } from '@/components/kb/kb-tag-filter'
import { KbEmptyState } from '@/components/kb/kb-empty-state'
import { KbItemDetail } from '@/components/kb/kb-item-detail'
import { KbFolderDialog } from '@/components/kb/kb-folder-dialog'
import { KbFileTree, KbFlatResults } from '@/components/kb/kb-tree'
import { KbContextMenu } from '@/components/kb/kb-context-menu'
import { Button } from '@/components/ui/button'
import type { KbMenuItem } from '@/components/kb/kb-context-menu'
import {
  applyFolderDropLocally,
  applyItemDropLocally,
  buildKbTree,
  collectPinnedNodes,
  flattenFolderOptions,
  flattenResults,
  folderSubtreeIds,
  localKbMatch,
  resolveFolderPath,
  UNFILED_EXPAND_KEY,
} from '@/components/kb/kb-tree-utils'
import type {
  FolderSelection,
  KbDragPayload,
  KbDropPlan,
  KbFlatResult,
  KbPinnedNode,
} from '@/components/kb/kb-tree-utils'
import './kb.css'

interface Props {
  boot?: Bootstrap
}

/** 新建 / 重命名共用弹窗的上下文（父级：create=目标父夹；rename=目标 folder） */
type FolderDialogState =
  | { mode: 'create'; parentId: string | null }
  | { mode: 'rename'; id: string; name: string }

/** searchKb 命中排序的本地快照：ids=null 表示请求中/失败（用本地检索兜底） */
interface SearchRank {
  query: string
  ids: string[] | null
}

/**
 * 右键菜单状态（tasks 2.4）：只记「在哪儿右键了谁」，菜单项在渲染时按最新
 * items/folders 快照构建——避免拿到被保存、移动、刷新替换下来的旧对象。
 */
type KbMenuState =
  | { kind: 'folder'; x: number; y: number; folderId: string }
  | { kind: 'item'; x: number; y: number; itemId: string }
  | { kind: 'blank'; x: number; y: number }
  | { kind: 'input'; x: number; y: number; field: HTMLInputElement | HTMLTextAreaElement }

/** 展开态持久化 key（Set<string> 的 JSON 数组） */
const EXPANDED_KEY = 'kb.tree.expanded'

/** 删除两步确认的回退窗口（与树内行内删除一致：3s） */
const DELETE_CONFIRM_MS = 3000

function readExpanded(): Set<string> {
  try {
    const raw = localStorage.getItem(EXPANDED_KEY)
    if (!raw) return new Set()
    const parsed: unknown = JSON.parse(raw)
    if (!Array.isArray(parsed)) return new Set()
    return new Set(parsed.filter((id): id is string => typeof id === 'string'))
  } catch {
    return new Set()
  }
}

// ─── 输入框编辑菜单（tasks 2.4）的纯辅助函数 ─────────────────────────────

type EditableField = HTMLInputElement | HTMLTextAreaElement

function fieldHasSelection(field: EditableField): boolean {
  const start = field.selectionStart ?? 0
  const end = field.selectionEnd ?? start
  return end > start
}

function fieldSelectedText(field: EditableField): string {
  const start = field.selectionStart ?? 0
  const end = field.selectionEnd ?? start
  return field.value.slice(start, end)
}

/**
 * 往受控 input/textarea 的光标处插入文本：React 受控输入框必须走原生 value setter
 * 再派发 input 事件，直接改 field.value 会被下一次渲染抹掉、也不触发 onChange。
 */
function replaceFieldRange(field: EditableField, text: string) {
  const start = field.selectionStart ?? field.value.length
  const end = field.selectionEnd ?? start
  const next = field.value.slice(0, start) + text + field.value.slice(end)
  const proto = field instanceof HTMLTextAreaElement
    ? HTMLTextAreaElement.prototype
    : HTMLInputElement.prototype
  const setter = Object.getOwnPropertyDescriptor(proto, 'value')?.set
  if (setter) setter.call(field, next)
  else field.value = next
  field.dispatchEvent(new Event('input', { bubbles: true }))
  const caret = start + text.length
  field.setSelectionRange(caret, caret)
}

function tryExecCommand(command: 'cut' | 'copy' | 'paste'): boolean {
  try {
    return document.execCommand(command)
  } catch {
    return false
  }
}

/**
 * 知识库视图（#/kb，第六视图）。
 *
 * 布局（S1 单树）：搜索框 + 标签筛选 → 左栏一棵文件树（固定「全部知识 / 未分类」+ 文件夹/条目叶子）
 * → 右详情（阅读态 ⇄ WYSIWYG 编辑态）。
 * 树在前端由 folders + items 合成（buildKbTree，O(n log n)），只渲染展开分支；展开态落 localStorage。
 * 过滤态（tasks 2.3）：搜索词或标签非空时左栏切换成扁平结果（标题 + 路径面包屑），只显示命中项；
 * 清空后恢复树与展开态。搜索走 search_kb 取命中顺序，失败/未回时用 localKbMatch 兜底。
 * 右键菜单（tasks 2.4）：树只上报命中对象与坐标，菜单本体（portal + 视口 clamp + 键盘导航）
 * 由 KbContextMenu 统一渲染；文件夹/条目/空白/输入框四类菜单的语义动作都在本视图构建。
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
  const [actionErr, setActionErr] = useState<string | null>(null)
  // ── 正文按需取数（kb-storage-backup 2.6）────────────────────────────────
  // 列表 IPC 只回元数据（bodyMd 恒为空串），正文在选中条目后才按 id 读取：
  // bodyReadyIds 记录「正文已就绪」的条目（含正文本来就是空串的），用来区分
  //「还没取正文」与「正文为空」——未就绪前不挂载编辑器，避免空正文回灌覆盖用户输入。
  const [bodyReadyIds, setBodyReadyIds] = useState<Set<string>>(() => new Set())
  /** 正文取数代次：每次 refresh（含 store-changed）自增 → 已取正文缓存失效，当前选中项重取 */
  const [bodyEpoch, setBodyEpoch] = useState(0)
  const bodyEpochRef = useRef(0)
  bodyEpochRef.current = bodyEpoch
  /** 本轮代次内已取到正文 / 在途的 id（快速切条不重复请求同一 id） */
  const fetchedBodyRef = useRef<{ epoch: number; ids: Set<string> }>({ epoch: -1, ids: new Set() })
  const inflightBodyRef = useRef<Map<string, number>>(new Map())
  /** 正文取数失败文案：只影响详情占位（可重试），不清列表、不渲染假空正文 */
  const [bodyLoadError, setBodyLoadError] = useState<string | null>(null)
  /** 详情面板容器：store-changed 时若焦点在面板内则跳过刷新，防覆盖正在输入的内容 */
  const detailPaneRef = useRef<HTMLDivElement>(null)
  /** 待聚焦标题的条目 id：详情要等正文就绪才挂载，提前发焦点信号会在挂载时被吞掉 */
  const pendingTitleFocusRef = useRef<string | null>(null)
  // 左侧清单折叠（真机反馈：太宽影响编辑）；localStorage 持久化
  const [listCollapsed, setListCollapsed] = useState(
    () => localStorage.getItem('kb.listCollapsed') === '1',
  )

  // ── 文件夹树（任务 #4 / S1 单树）────────────────────────────────────────
  const [folders, setFolders] = useState<KbFolder[]>([])
  const [foldersReady, setFoldersReady] = useState(false)
  const [folderLoadErr, setFolderLoadErr] = useState<string | null>(null)
  const [folderSelection, setFolderSelection] = useState<FolderSelection>({ kind: 'all' })
  const [folderDialog, setFolderDialog] = useState<FolderDialogState | null>(null)
  /** 展开的文件夹 id 集合（含虚拟行 key UNFILED_EXPAND_KEY）；持久化到 localStorage */
  const [expanded, setExpanded] = useState<Set<string>>(readExpanded)
  const expandedRef = useRef(expanded)
  expandedRef.current = expanded
  const expandedInitRef = useRef(false)

  // ── 右键菜单（tasks 2.4）────────────────────────────────────────────────
  const [menu, setMenu] = useState<KbMenuState | null>(null)
  /** 删除两步确认：菜单打开期间点第一次「删除」→ 文案变「确认删除」，3s 内再点才执行 */
  const [deleteConfirm, setDeleteConfirm] = useState(false)
  /** 输入框「粘贴」被浏览器拒绝后置灰并可在 hint 里提示 Ctrl+V */
  const [pasteDenied, setPasteDenied] = useState(false)
  const deleteTimerRef = useRef<number | null>(null)
  /** 新建即命名（tasks 2.6）：自增信号请求详情面板把焦点放进标题并全选 */
  const [titleFocusSignal, setTitleFocusSignal] = useState(0)

  const clearDeleteTimer = useCallback(() => {
    if (deleteTimerRef.current !== null) {
      window.clearTimeout(deleteTimerRef.current)
      deleteTimerRef.current = null
    }
  }, [])

  const closeMenu = useCallback(() => {
    clearDeleteTimer()
    setMenu(null)
    setDeleteConfirm(false)
    setPasteDenied(false)
  }, [clearDeleteTimer])

  const openMenu = useCallback((next: KbMenuState) => {
    clearDeleteTimer()
    setDeleteConfirm(false)
    setPasteDenied(false)
    setMenu(next)
  }, [clearDeleteTimer])

  // 卸载时清掉删除确认计时器，避免 setState on unmounted
  useEffect(() => () => clearDeleteTimer(), [clearDeleteTimer])

  /** 删除第一次点击：只改文案并起 3s 回退计时，不碰后端 */
  const startDeleteConfirm = useCallback(() => {
    clearDeleteTimer()
    setDeleteConfirm(true)
    deleteTimerRef.current = window.setTimeout(() => {
      deleteTimerRef.current = null
      setDeleteConfirm(false)
    }, DELETE_CONFIRM_MS)
  }, [clearDeleteTimer])

  /**
   * 请求把焦点放进详情面板标题并全选（tasks 2.6）。
   * 放到下一帧发信号：等「新建 → refresh → setSelectedId」或换条目这一轮提交完、
   * KbItemDetail 挂载并收到新的 focusTitleSignal 后才聚焦，避免被同帧渲染覆盖。
   */
  const requestTitleFocus = useCallback(() => {
    requestAnimationFrame(() => setTitleFocusSignal((n) => n + 1))
  }, [])

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

  /** 标记某条目的正文已就绪（含正文本来就是空串的情况） */
  const markBodyReady = useCallback((id: string) => {
    setBodyReadyIds((prev) => {
      if (prev.has(id)) return prev
      const next = new Set(prev)
      next.add(id)
      return next
    })
  }, [])

  /** 保存成功后局部回填单条（#1：不再全量 refresh，不闪空态、不重排列表） */
  const handleSaved = useCallback((saved: KbItem) => {
    setItems((prev) => prev.map((it) => (it.id === saved.id ? saved : it)))
    // add/update 的回包是完整条目（含正文）：直接视为正文已就绪，省一次按 id 取正文
    markBodyReady(saved.id)
  }, [markBodyReady])

  const itemsRef = useRef<KbItem[]>([])
  itemsRef.current = items
  const foldersRef = useRef<KbFolder[]>([])
  foldersRef.current = folders

  // ── 数据加载（不再依赖搜索词：搜索命中顺序由独立 effect 维护）──────────
  // silent=true：后台刷新（store-changed），不翻加载态、不闪「加载中…」
  const refresh = useCallback(async (silent = false) => {
    if (!silent) setLoading(true)
    try {
      const result = await getKbItems()
      // 加载失败 / 知识库损坏：显示错误横幅并保留旧列表，绝不降级成「空空如也」假空态
      if (result.err) {
        setLoadError(result.err)
        return
      }
      const all = result.data ?? []
      // 列表只回元数据（正文为空串）：保留已取到的正文，避免选中项正文被清空后
      // 编辑器把空正文当外部更新回灌，覆盖用户正在输入的内容
      setItems((prev) => {
        const prevById = new Map(prev.map((it) => [it.id, it]))
        const merged = all.map((meta) => {
          const old = prevById.get(meta.id)
          return old ? { ...meta, bodyMd: old.bodyMd } : meta
        })
        merged.sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))
        return merged
      })
      setLoadError(null)
      // 正文缓存失效：当前选中项由正文取数 effect 按新代次重取
      setBodyEpoch((n) => n + 1)
    } catch (e) {
      setLoadError(e instanceof Error ? e.message : String(e))
    } finally {
      if (!silent) setLoading(false)
    }
  }, [])

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

  // ── 正文按需取数（task 2.6）：列表只有元数据，选中条目后按 id 拉完整正文 ──────────
  // 代次（bodyEpoch）变化 = 缓存失效：同一 epoch 内同一 id 只请求一次，快速切条不重复拉；
  // 期间若又发生刷新，丢弃旧代次回包，交给新一轮 effect 重取，避免旧正文覆盖新数据。
  useEffect(() => {
    if (!selectedId) return
    // 列表里还没有这条（列表还在加载 / 被过滤重建）：等 refresh 的代次变化再触发
    if (!itemsRef.current.some((it) => it.id === selectedId)) return
    const epoch = bodyEpoch
    if (fetchedBodyRef.current.epoch !== epoch) {
      fetchedBodyRef.current = { epoch, ids: new Set() }
    }
    const fetched = fetchedBodyRef.current.ids
    if (fetched.has(selectedId)) return
    if (inflightBodyRef.current.get(selectedId) === epoch) return
    inflightBodyRef.current.set(selectedId, epoch)
    setBodyLoadError(null)
    void (async () => {
      try {
        const result = await getKbItems([selectedId])
        // 期间刷新过：本回包属于旧代次，丢弃（新一轮 effect 会用新代次重取）
        if (bodyEpochRef.current !== epoch) return
        if (result.err) {
          setBodyLoadError(result.err)
          return
        }
        const full = (result.data ?? []).find((it) => it.id === selectedId)
        // 不存在的 id：保持元数据（列表刷新后选中会被校正清空）
        if (!full) return
        // 只按 id 合并完整条目，不改 selectedId：快速切条时回包不抢当前选中
        setItems((prev) => prev.map((it) => (it.id === full.id ? full : it)))
        markBodyReady(full.id)
        fetched.add(full.id)
      } catch (e) {
        if (bodyEpochRef.current === epoch) {
          setBodyLoadError(e instanceof Error ? e.message : String(e))
        }
      } finally {
        if (inflightBodyRef.current.get(selectedId) === epoch) inflightBodyRef.current.delete(selectedId)
      }
    })()
  }, [selectedId, bodyEpoch, markBodyReady])

  // ── store-changed：静默重拉元数据并让正文缓存失效（其它窗口编辑 / 导入 / 自身保存）──
  // 只关心知识库（id='kb'）与整体数据（id='data'）事件；焦点在详情面板内时跳过，
  // 防外部刷新回灌覆盖正在输入的内容（与知识便签窗同策略，AGENTS 2.6）。
  useEffect(() => {
    const off = onEvent('store-changed', (payload: unknown) => {
      const id = (payload as { id?: unknown } | null)?.id
      if (id !== 'kb' && id !== 'data') return
      const active = document.activeElement
      if (active instanceof Node && detailPaneRef.current?.contains(active)) return
      void refresh(true)
    })
    return off
  }, [refresh])

  // 详情要等正文就绪才挂载：新建即命名 / 右键重命名的焦点请求先登记，面板挂载后再发信号；
  // 选中切走后丢弃过期请求，避免之后切回该条目时突然抢焦点。
  useEffect(() => {
    const pending = pendingTitleFocusRef.current
    if (pending === null) return
    if (pending !== selectedId) {
      pendingTitleFocusRef.current = null
      return
    }
    if (!bodyReadyIds.has(pending)) return
    pendingTitleFocusRef.current = null
    requestTitleFocus()
  }, [selectedId, bodyReadyIds, requestTitleFocus])

  // ── 搜索命中顺序（tasks 2.3）：独立于全量刷新，失败/未回时扁平结果用本地兜底 ──
  const trimmedQuery = query.trim()
  const [searchRank, setSearchRank] = useState<SearchRank | null>(null)

  useEffect(() => {
    if (!trimmedQuery) {
      setSearchRank(null)
      return
    }
    // 先置 pending：本地检索立即出结果，search_kb 回来后再按相关度顺序替换，避免过滤态空窗
    setSearchRank({ query: trimmedQuery, ids: null })
    let cancelled = false
    void (async () => {
      try {
        const result = await searchKb(trimmedQuery)
        if (cancelled) return
        if (result.err) {
          setSearchRank({ query: trimmedQuery, ids: null })
          return
        }
        // D5：search_kb 已收窄为「标题 + 祖先文件夹名」并直接返回 id 数组（按相关度排序）
        const ids = result.data ?? []
        setSearchRank({ query: trimmedQuery, ids })
        void trackEvent('kb_search', { hits: ids.length })
      } catch {
        if (!cancelled) setSearchRank({ query: trimmedQuery, ids: null })
      }
    })()
    return () => { cancelled = true }
  }, [trimmedQuery])

  // ── 展开态（tasks 2.2）：Set<string> 存 localStorage('kb.tree.expanded') ──
  const persistExpanded = useCallback((next: Set<string>) => {
    expandedRef.current = next
    setExpanded(next)
    try {
      localStorage.setItem(EXPANDED_KEY, JSON.stringify([...next]))
    } catch {
      // localStorage 不可用（隐私模式等）：仅保留内存态
    }
  }, [])

  const toggleExpanded = useCallback((id: string) => {
    const next = new Set(expandedRef.current)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    persistExpanded(next)
  }, [persistExpanded])

  /** 展开全部文件夹（含未分类虚拟行）——空白菜单与「展开全部」共用 */
  const expandAll = useCallback(() => {
    const next = new Set(folders.map((folder) => folder.id))
    next.add(UNFILED_EXPAND_KEY)
    persistExpanded(next)
  }, [folders, persistExpanded])

  // 首次拿到文件夹且本地没有展开记录：默认全展开 + 未分类展开，让首屏能看到全部内容
  useEffect(() => {
    if (!foldersReady || expandedInitRef.current) return
    expandedInitRef.current = true
    try {
      if (localStorage.getItem(EXPANDED_KEY) !== null) return
    } catch {
      return
    }
    const next = new Set(folders.map((folder) => folder.id))
    next.add(UNFILED_EXPAND_KEY)
    persistExpanded(next)
  }, [foldersReady, folders, persistExpanded])

  // 选中条目自动展开祖先文件夹（deep link / 新建 / 默认选中都能看到条目）；未分类则展开虚拟行
  useEffect(() => {
    if (!selectedId) return
    const item = itemsRef.current.find((i) => i.id === selectedId)
    if (!item) return
    const path = resolveFolderPath(folders, item.folderId ?? null)
    const additions = path.map((folder) => folder.id)
    if (path.length === 0) additions.push(UNFILED_EXPAND_KEY)
    if (additions.every((id) => expandedRef.current.has(id))) return
    const next = new Set(expandedRef.current)
    for (const id of additions) next.add(id)
    persistExpanded(next)
  }, [selectedId, items, folders, persistExpanded])

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

  // 输入框编辑菜单（tasks 2.4）：capture 阶段拦 window contextmenu。
  // 只接管 .kb-view 内、且不属于 contenteditable / ProseMirror 的 input/textarea；
  // 正文编辑器（isContentEditable）直接 return，交给编辑器自己的菜单。
  useEffect(() => {
    const onContextMenu = (event: MouseEvent) => {
      const root = viewRef.current
      if (!root) return
      const target = event.target
      if (!(target instanceof HTMLInputElement) && !(target instanceof HTMLTextAreaElement)) return
      if (!root.contains(target)) return
      // 可编辑区域内的输入框（如编辑器内嵌控件）不接管，避免和编辑器菜单打架
      if (target.isContentEditable || target.closest('.ProseMirror')) return
      let ancestor: HTMLElement | null = target.parentElement
      while (ancestor) {
        if (ancestor.isContentEditable) return
        ancestor = ancestor.parentElement
      }
      event.preventDefault()
      openMenu({ kind: 'input', x: event.clientX, y: event.clientY, field: target })
    }
    window.addEventListener('contextmenu', onContextMenu, true)
    return () => window.removeEventListener('contextmenu', onContextMenu, true)
  }, [openMenu])

  // 过滤态下树被扁平结果替代：文件夹/空白菜单不可用（结构操作隐藏），开着也顺手关掉
  const isFiltering = trimmedQuery !== '' || activeTag !== null
  useEffect(() => {
    if (!isFiltering || !menu) return
    if (menu.kind === 'folder' || menu.kind === 'blank') closeMenu()
  }, [isFiltering, menu, closeMenu])

  // ── 合成树 + 过滤态扁平结果（tasks 2.1 / 2.3）──────────────────────────
  const tree = useMemo(() => buildKbTree(folders, items), [folders, items])

  /** 置顶快捷区（tasks 3.4）：文件夹 + 条目统一按 pinned/order 排序，路径提示由原树解析 */
  const pinnedNodes = useMemo<KbPinnedNode[]>(
    () => collectPinnedNodes(folders, items),
    [folders, items],
  )

  const flatResults = useMemo<KbFlatResult[]>(() => {
    if (!isFiltering) return []
    // 基础集合：搜索按 search_kb 命中顺序；无搜索词时按现有列表序（updatedAt desc）走标签过滤
    const base: KbItem[] = []
    if (trimmedQuery) {
      const rank = searchRank && searchRank.query === trimmedQuery ? searchRank : null
      if (rank?.ids) {
        // D5 真正过滤：后端命中集合即结果集合（顺序保留相关度 rank），不再掺入本地命中
        const byId = new Map(items.map((item) => [item.id, item]))
        for (const id of rank.ids) {
          const item = byId.get(id)
          if (item) base.push(item)
        }
      } else {
        // 请求中 / 后端不可用：本地兜底，口径与后端一致（标题 + 祖先文件夹名），绝不显示假空态
        for (const item of items) {
          if (localKbMatch(item, trimmedQuery, folders)) base.push(item)
        }
      }
    } else {
      base.push(...items)
    }
    const tagFiltered = activeTag
      ? base.filter((item) => item.tags.includes(activeTag))
      : base
    return flattenResults(tagFiltered, folders, () => true)
  }, [isFiltering, trimmedQuery, searchRank, items, folders, activeTag])

  /** 当前可见条目（过滤态 = 扁平结果；树态 = 全部条目，树自身承载层级导航） */
  const visibleItems = useMemo(
    () => (isFiltering ? flatResults.map((result) => result.item) : items),
    [isFiltering, flatResults, items],
  )

  // #5 选择校正（spec: 默认选中可见条目，D7）：items 加载完成 / visibleItems 变化后执行一次。
  // 优先级：deep link 待兑现 > 当前选中仍在可见列表（保持用户选择）> 可见列表第一项。
  // loading 期间不校正（列表可能还在换血，且 deep link 依赖 items 就绪）；visibleItems 为空时
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
    if (visibleItems.length === 0) {
      if (selectedId !== null) setSelectedId(null)
      return
    }

    // 3) 当前选中仍在可见列表 → 保持不变（不要每次刷新都重置用户的选择）
    if (selectedId !== null && visibleItems.some((item) => item.id === selectedId)) return

    // 4) 无选中 / 选中项被搜索或标签筛掉 → 选中可见第一项
    setSelectedId(visibleItems[0].id)
  }, [loading, items, visibleItems, selectedId])

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

  /** 选中条目的正文是否已就绪：就绪才挂载详情编辑器，未就绪显示占位（防空正文回灌） */
  const selectedBodyReady = useMemo(
    () => (selected ? bodyReadyIds.has(selected.id) : false),
    [selected, bodyReadyIds],
  )

  // 条目被任务引用数（boot 无 stickies，知识便签引用不计；删除确认提示用 qa-8）
  const refCountOf = useCallback((id: string): number => {
    if (!boot) return 0
    return boot.tasks.filter((t) => !t.deletedAt && t.kbRefs?.includes(id)).length
  }, [boot])

  // ── 条目 / 文件夹操作（右键菜单与树内按钮共用）──────────────────────────

  /**
   * 新建条目。folderIdOverride：文件夹菜单传该夹 id；空白菜单/顶栏按当前 context。
   * 成功后 refresh + 选中新条目 + 请求标题焦点（tasks 2.6 新建即命名）。
   */
  const createItem = useCallback(async (folderIdOverride?: string) => {
    const title = `新建知识 ${new Date().toLocaleDateString('zh-CN')}`
    const payload: KbPayload = { title, bodyMd: '', tags: [] }
    if (folderIdOverride !== undefined) payload.folderId = folderIdOverride
    else if (folderSelection.kind === 'unfiled') payload.folderId = ''
    else if (folderSelection.kind === 'folder') payload.folderId = folderSelection.id
    const result = await addKbItem(payload)
    if (result.err) {
      setActionErr(result.err)
      return
    }
    if (!result.data) return
    const created = result.data
    void trackEvent('kb_create', {})
    setActionErr(null)
    // add 回包是完整条目（正文为空串也算已知）：直接视为正文就绪，详情不必等一次按 id 取正文
    markBodyReady(created.id)
    await refresh()
    // 详情等正文就绪才挂载：登记待聚焦，面板挂载后由焦点信号把焦点放进标题（新建即命名）
    pendingTitleFocusRef.current = created.id
    setSelectedId(created.id)
  }, [refresh, folderSelection, markBodyReady])

  const handleCreate = useCallback(() => { void createItem() }, [createItem])

  /** 右键「打开 / 编辑」「重命名」：选中条目并把焦点放进标题（tasks 2.4 / 2.6） */
  const openItemForEdit = useCallback((item: KbItem) => {
    closeMenu()
    handleSelectItem(item.id)
    if (selectedId === item.id && bodyReadyIds.has(item.id)) {
      // 已在详情面板里（正文就绪）：直接发焦点信号
      requestTitleFocus()
      return
    }
    // 详情等正文就绪才挂载：先登记待聚焦，面板挂载后再发信号，避免信号提前被吞
    pendingTitleFocusRef.current = item.id
  }, [closeMenu, handleSelectItem, requestTitleFocus, selectedId, bodyReadyIds])

  /** 条目置顶 / 取消置顶：updateKbItem 回包局部回填，失败展示后端中文文案 */
  const toggleItemPin = useCallback(async (item: KbItem) => {
    const result = await updateKbItem(item.id, { pinned: !item.pinned })
    if (result.err) {
      setActionErr(result.err)
      return
    }
    setActionErr(null)
    handleSaved(result.data ?? { ...item, pinned: !item.pinned })
  }, [handleSaved])

  /** 条目移动：folderId 传 "" 表示移到未分类（后端约定） */
  const moveItem = useCallback(async (item: KbItem, folderId: string) => {
    const result = await updateKbItem(item.id, { folderId })
    if (result.err) {
      setActionErr(result.err)
      return
    }
    setActionErr(null)
    handleSaved(result.data ?? { ...item, folderId: folderId === '' ? null : folderId })
  }, [handleSaved])

  /** 条目删除：成功后移除并清选中；失败展示后端文案 */
  const deleteItem = useCallback(async (item: KbItem) => {
    const result = await deleteKbItem(item.id)
    if (result.err) {
      setActionErr(result.err)
      return
    }
    setActionErr(null)
    setItems((prev) => prev.filter((it) => it.id !== item.id))
    setSelectedId((prev) => (prev === item.id ? null : prev))
  }, [])

  /** 文件夹置顶 / 取消置顶：updateFolder 回包局部回填（树排序由 pinned 驱动） */
  const toggleFolderPin = useCallback(async (folder: KbFolder) => {
    const result = await updateFolder(folder.id, { pinned: !folder.pinned })
    if (result.err) {
      setActionErr(result.err)
      return
    }
    setActionErr(null)
    setFolders((prev) => prev.map((f) => (
      f.id === folder.id ? (result.data ?? { ...f, pinned: !folder.pinned }) : f
    )))
  }, [])

  /** 文件夹移动：parentId 传 "" 表示移到根目录；成功后重拉文件夹并保持/展开目标父级 */
  const moveFolder = useCallback(async (folder: KbFolder, parentId: string) => {
    const result = await updateFolder(folder.id, { parentId })
    if (result.err) {
      setActionErr(result.err)
      return
    }
    setActionErr(null)
    await refreshFolders()
    // 目标父级可见性：把新父级与它的祖先链展开，否则移动后看起来「消失了」
    if (parentId !== '') {
      const next = new Set(expandedRef.current)
      next.add(parentId)
      for (const ancestor of resolveFolderPath(folders, parentId)) next.add(ancestor.id)
      persistExpanded(next)
    }
  }, [refreshFolders, folders, persistExpanded])

  // ── S2 树拖拽落库（tasks 3.2 / 3.3，design D4/D5）───────────────────────
  /**
   * 树内拖放：先乐观回填本地 items/folders（树立即反映落点），再调后端。
   * order 语义 =「移除被拖节点后的目标同级索引」（后端 clamp + dense 重排 0..n-1）；
   * inside / 空区追加 不带 order（跨父 = 追加末尾，同父 = 保持原位）。
   * 失败恢复整组快照并展示后端中文文案；成功不整库重拉，仅文件夹结构变化时重拉 folders。
   */
  const handleDropNode = useCallback(async (dragged: KbDragPayload, plan: KbDropPlan) => {
    const itemsSnapshot = itemsRef.current
    const foldersSnapshot = foldersRef.current
    const order = plan.type === 'reorder' ? plan.index : undefined

    if (dragged.kind === 'item') {
      setItems((prev) => applyItemDropLocally(prev, dragged.id, plan.parentId, order))
    } else {
      setFolders((prev) => applyFolderDropLocally(prev, dragged.id, plan.parentId, order))
    }
    setActionErr(null)

    const result = dragged.kind === 'item'
      ? await updateKbItem(dragged.id, {
        folderId: plan.parentId ?? '',
        ...(order !== undefined ? { order } : {}),
      })
      : await updateFolder(dragged.id, {
        parentId: plan.parentId ?? '',
        ...(order !== undefined ? { order } : {}),
      })

    if (result.err) {
      // 回滚整组快照：后端可能已对旧/新同级做过 dense 重排，逐条打补丁对不齐
      setItems(itemsSnapshot)
      setFolders(foldersSnapshot)
      setActionErr(result.err)
      return
    }

    if (dragged.kind === 'item') {
      // 上方三元已按 kind 发起对应命令，回包类型跟着 kind 收窄（联合类型手动收口）
      const saved = result.data as KbItem | null
      if (saved) setItems((prev) => prev.map((item) => (item.id === saved.id ? saved : item)))
    } else {
      const saved = result.data as KbFolder | null
      if (saved) setFolders((prev) => prev.map((folder) => (folder.id === saved.id ? saved : folder)))
      // 文件夹移动/排序会影响其它同级（dense 重排）与树结构 → 重拉一次文件夹列表
      await refreshFolders()
    }

    // 落点可见性：展开目标父级与祖先链，避免拖进去后「消失」；选择与滚动位置不动
    const next = new Set(expandedRef.current)
    if (plan.parentId) {
      next.add(plan.parentId)
      for (const ancestor of resolveFolderPath(foldersRef.current, plan.parentId)) next.add(ancestor.id)
    } else if (dragged.kind === 'item') {
      next.add(UNFILED_EXPAND_KEY)
    }
    persistExpanded(next)
  }, [persistExpanded, refreshFolders])

  /** 窗口级防外部文件拖入导航（tasks 3.2）：WebView2 默认会打开被拖入的文件；只拦 Files，不影响内部拖拽 */
  useEffect(() => {
    const isExternalFile = (event: DragEvent): boolean =>
      Array.from(event.dataTransfer?.types ?? []).includes('Files')
    const onDragOver = (event: DragEvent) => {
      if (isExternalFile(event)) event.preventDefault()
    }
    const onDrop = (event: DragEvent) => {
      if (isExternalFile(event)) event.preventDefault()
    }
    window.addEventListener('dragenter', onDragOver)
    window.addEventListener('dragover', onDragOver)
    window.addEventListener('drop', onDrop)
    return () => {
      window.removeEventListener('dragenter', onDragOver)
      window.removeEventListener('dragover', onDragOver)
      window.removeEventListener('drop', onDrop)
    }
  }, [])

  /** 置顶快捷区点击 = 定位（tasks 3.4）：展开祖先 + 选中 + 滚动到树内原位置；折叠态自动展开列表 */
  const locateNode = useCallback((target: { kind: 'folder' | 'item'; id: string }) => {
    setListCollapsed((prev) => {
      if (!prev) return prev
      localStorage.setItem('kb.listCollapsed', '0')
      return false
    })
    const next = new Set(expandedRef.current)
    if (target.kind === 'item') {
      setSelectedId(target.id)
      const item = itemsRef.current.find((candidate) => candidate.id === target.id)
      const path = resolveFolderPath(foldersRef.current, item?.folderId ?? null)
      if (path.length === 0) next.add(UNFILED_EXPAND_KEY)
      for (const folder of path) next.add(folder.id)
    } else {
      setFolderSelection({ kind: 'folder', id: target.id })
      // 文件夹自身也展开：点击快捷区 = 进入该文件夹
      next.add(target.id)
      for (const folder of resolveFolderPath(foldersRef.current, target.id)) next.add(folder.id)
    }
    persistExpanded(next)
    // 等展开后的行挂载再滚动（两帧：React 提交 + 浏览器布局）
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        const key = `${target.kind}:${CSS.escape(target.id)}`
        viewRef.current
          ?.querySelector<HTMLElement>(`.kb-tree-scroll [data-kb-key="${key}"]`)
          ?.scrollIntoView({ block: 'nearest' })
      })
    })
  }, [persistExpanded])

  // ── 右键菜单项构建（每帧按最新 items/folders 快照生成）──────────────────

  const buildInputMenuItems = useCallback((field: EditableField): KbMenuItem[] => {
    const hasRange = fieldHasSelection(field)
    const locked = field.readOnly || field.disabled

    const cut = async () => {
      field.focus()
      if (fieldHasSelection(field) && tryExecCommand('cut')) {
        closeMenu()
        return
      }
      // 退路：复制选区文本，再把选区替换为空（受控输入框走原生 setter + input 事件）
      const text = fieldSelectedText(field)
      if (!text) {
        closeMenu()
        return
      }
      try {
        await navigator.clipboard.writeText(text)
      } catch {
        closeMenu()
        return
      }
      replaceFieldRange(field, '')
      closeMenu()
    }

    const copy = async () => {
      field.focus()
      if (tryExecCommand('copy')) {
        closeMenu()
        return
      }
      const text = fieldSelectedText(field)
      try {
        await navigator.clipboard.writeText(text)
      } catch {
        // 剪贴板写权限也没有：关菜单，让用户用 Ctrl+C（不静默改内容）
      }
      closeMenu()
    }

    const paste = async () => {
      field.focus()
      if (tryExecCommand('paste')) {
        closeMenu()
        return
      }
      try {
        const text = await navigator.clipboard.readText()
        replaceFieldRange(field, text)
        closeMenu()
      } catch {
        // 浏览器未授权读取剪贴板：菜单保持打开，粘贴项置灰并在 hint 里提示 Ctrl+V
        setPasteDenied(true)
      }
    }

    return [
      {
        label: '剪切',
        icon: <Scissors size={13} strokeWidth={1.7} />,
        disabled: !hasRange || locked,
        onSelect: () => { void cut() },
      },
      {
        label: '复制',
        icon: <Copy size={13} strokeWidth={1.7} />,
        disabled: !hasRange,
        onSelect: () => { void copy() },
      },
      {
        label: '粘贴',
        icon: <ClipboardPaste size={13} strokeWidth={1.7} />,
        disabled: pasteDenied || locked,
        hint: pasteDenied
          ? '浏览器未授权读取剪贴板，请按 Ctrl+V'
          : '优先系统粘贴；不可用时尝试读取剪贴板',
        onSelect: () => { void paste() },
      },
      {
        label: '全选',
        icon: <TextSelect size={13} strokeWidth={1.7} />,
        onSelect: () => {
          field.focus()
          field.select()
          closeMenu()
        },
      },
    ]
  }, [closeMenu, pasteDenied])

  const buildMenuItems = (
    state: KbMenuState,
    folder: KbFolder | null,
    item: KbItem | null,
  ): KbMenuItem[] | null => {
    switch (state.kind) {
      case 'folder': {
        if (!folder) return null
        const blocked = folderSubtreeIds(folders, folder.id)
        const moveOptions = flattenFolderOptions(folders)
          .filter((option) => !blocked.has(option.id))
          .map<KbMenuItem>((option) => ({
            label: `${'\u3000'.repeat(option.depth)}${option.name}`,
            hint: option.pathLabel,
            onSelect: () => { closeMenu(); void moveFolder(folder, option.id) },
          }))
        const atRoot = folder.parentId === null
        return [
          {
            label: '新建文件夹',
            icon: <FolderPlus size={13} strokeWidth={1.7} />,
            onSelect: () => {
              closeMenu()
              setFolderDialog({ mode: 'create', parentId: folder.id })
            },
          },
          {
            label: '新建文件',
            icon: <FilePlus size={13} strokeWidth={1.7} />,
            onSelect: () => { closeMenu(); void createItem(folder.id) },
          },
          {
            label: '重命名',
            icon: <PencilLine size={13} strokeWidth={1.7} />,
            onSelect: () => {
              closeMenu()
              setFolderDialog({ mode: 'rename', id: folder.id, name: folder.name })
            },
          },
          {
            label: folder.pinned ? '取消置顶' : '置顶',
            icon: folder.pinned
              ? <PinOff size={13} strokeWidth={1.7} />
              : <Pin size={13} strokeWidth={1.7} />,
            onSelect: () => { closeMenu(); void toggleFolderPin(folder) },
          },
          {
            label: '移动到…',
            icon: <FolderInput size={13} strokeWidth={1.7} />,
            submenu: [
              {
                label: '根目录',
                disabled: atRoot,
                hint: atRoot ? '已位于根目录' : undefined,
                onSelect: () => { closeMenu(); void moveFolder(folder, '') },
              },
              ...moveOptions,
            ],
          },
          {
            label: deleteConfirm ? '确认删除' : '删除',
            icon: <Trash2 size={13} strokeWidth={1.7} />,
            danger: true,
            hint: deleteConfirm ? '再次点击确认删除（3 秒内有效）' : '删除文件夹，其中条目不会删除',
            onSelect: () => {
              if (!deleteConfirm) {
                startDeleteConfirm()
                return
              }
              closeMenu()
              void handleDeleteFolder(folder)
            },
          },
        ]
      }
      case 'item': {
        if (!item) return null
        const moveOptions = flattenFolderOptions(folders).map<KbMenuItem>((option) => ({
          label: `${'\u3000'.repeat(option.depth)}${option.name}`,
          hint: `${option.pathLabel}${item.folderId === option.id ? '（当前位置）' : ''}`,
          disabled: item.folderId === option.id,
          onSelect: () => { closeMenu(); void moveItem(item, option.id) },
        }))
        const unfiled = item.folderId === null
        return [
          {
            label: '打开 / 编辑标题',
            icon: <FileText size={13} strokeWidth={1.7} />,
            onSelect: () => openItemForEdit(item),
          },
          {
            label: '重命名',
            icon: <PencilLine size={13} strokeWidth={1.7} />,
            onSelect: () => openItemForEdit(item),
          },
          {
            label: item.pinned ? '取消置顶' : '置顶',
            icon: item.pinned
              ? <PinOff size={13} strokeWidth={1.7} />
              : <Pin size={13} strokeWidth={1.7} />,
            onSelect: () => { closeMenu(); void toggleItemPin(item) },
          },
          {
            label: '移动到…',
            icon: <FolderInput size={13} strokeWidth={1.7} />,
            submenu: [
              {
                label: '未分类',
                disabled: unfiled,
                hint: unfiled ? '已在未分类' : undefined,
                onSelect: () => { closeMenu(); void moveItem(item, '') },
              },
              ...moveOptions,
            ],
          },
          {
            label: deleteConfirm ? '确认删除' : '删除',
            icon: <Trash2 size={13} strokeWidth={1.7} />,
            danger: true,
            hint: deleteConfirm ? '再次点击确认删除（3 秒内有效）' : '删除后不可恢复',
            onSelect: () => {
              if (!deleteConfirm) {
                startDeleteConfirm()
                return
              }
              closeMenu()
              void deleteItem(item)
            },
          },
        ]
      }
      case 'blank':
        return [
          {
            label: '新建文件夹',
            icon: <FolderPlus size={13} strokeWidth={1.7} />,
            onSelect: () => {
              closeMenu()
              setFolderDialog({ mode: 'create', parentId: null })
            },
          },
          {
            label: '新建文件',
            icon: <FilePlus size={13} strokeWidth={1.7} />,
            onSelect: () => { closeMenu(); void createItem('') },
          },
          {
            label: '展开全部',
            icon: <ChevronsUpDown size={13} strokeWidth={1.7} />,
            onSelect: () => { closeMenu(); expandAll() },
          },
          {
            label: '收起全部',
            icon: <ChevronsDownUp size={13} strokeWidth={1.7} />,
            onSelect: () => { closeMenu(); persistExpanded(new Set()) },
          },
        ]
      case 'input':
        return buildInputMenuItems(state.field)
    }
    return null
  }

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
        : await updateFolder(folderDialog.id, { name })
      if (result.err) return result.err
      const createdParentId = folderDialog.mode === 'create' ? folderDialog.parentId : null
      setFolderDialog(null)
      setActionErr(null)
      await refreshFolders()
      // 在某个文件夹下新建：把该父级与祖先链展开，否则新文件夹藏起来看不到
      if (createdParentId) {
        const next = new Set(expandedRef.current)
        next.add(createdParentId)
        for (const ancestor of resolveFolderPath(folders, createdParentId)) next.add(ancestor.id)
        persistExpanded(next)
      }
      return null
    },
    [folderDialog, refreshFolders, folders, persistExpanded],
  )

  /** 删除文件夹（tree 内与右键菜单共用；菜单侧已做两步确认）；失败把后端文案放进局部错误区 */
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

  // 菜单项按当前快照解析目标：目标被删除时菜单直接不渲染（避免对旧对象操作）
  const menuFolder = menu?.kind === 'folder'
    ? folders.find((f) => f.id === menu.folderId) ?? null
    : null
  const menuItem = menu?.kind === 'item'
    ? items.find((i) => i.id === menu.itemId) ?? null
    : null
  const menuItems = menu ? buildMenuItems(menu, menuFolder, menuItem) : null

  return (
    <div className="view kb-view" ref={viewRef}>
      {actionErr && (
        <div className="inline-err" role="alert">{actionErr}</div>
      )}

      <div className={`kb-layout${listCollapsed ? ' is-collapsed' : ''}`}>
        {/* 左清单：树态一棵树 / 过滤态扁平结果（各自滚动；折叠态 CSS 压成 96px 窄轨） */}
        <div className="kb-list">
          {/* 搜索移入左列顶部：知识库区不再有横跨整栏的搜索带，纸面从窗口顶端起算 */}
          <KbSearchBar
            query={query}
            onChange={setQuery}
            resultCount={isFiltering ? flatResults.length : items.length}
          />
          {/* 标签筛选也留在左列：详情纸面才能从窗口顶端起算（kb-workspace-layout D1） */}
          {!listCollapsed && (
            <KbTagFilter
              tags={allTags}
              activeTag={activeTag}
              onSelect={setActiveTag}
            />
          )}
          {/* 加载失败 / 知识库损坏：错误横幅 + 重试；绝不渲染「空空如也」假空态。折叠态也始终可见。 */}
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
          {/* 加载中轻提示：绝对定位、不占位，不清空/顶掉旧列表（#1 保存闪烁） */}
          {loading && (
            <div className="kb-loading-hint" role="status" aria-live="polite">加载中…</div>
          )}

          {isFiltering ? (
            flatResults.length > 0 ? (
              <KbFlatResults
                results={flatResults}
                selectedItemId={selectedId}
                onSelect={handleSelectItem}
                onContextItem={(item, x, y) => openMenu({ kind: 'item', x, y, itemId: item.id })}
              />
            ) : loading ? null : (
              <div className="kb-item-list" role="listbox" aria-label="知识检索结果">
                {/* 有筛选但无命中：明确「没有匹配的笔记」，不冒充空库 */}
                <div className="kb-empty" role="status">
                  <span className="empty-icon">🔍</span>
                  <span>没有匹配的笔记</span>
                </div>
              </div>
            )
          ) : (
            <KbFileTree
              tree={tree}
              expanded={expanded}
              selectedItemId={selectedId}
              folderSelection={folderSelection}
              folderLoadError={folderLoadErr}
              pinnedNodes={pinnedNodes}
              emptyContent={
                !loading && !loadError && items.length === 0
                  ? <KbEmptyState variant="empty" onCreate={handleCreate} />
                  : null
              }
              onToggleExpand={toggleExpanded}
              onSelectItem={handleSelectItem}
              onSelectFolder={setFolderSelection}
              onCreateFolder={handleCreateFolder}
              onCreateItem={handleCreate}
              onOpenDataFolder={() => void handleOpenDataFolder()}
              onRenameFolder={handleRenameFolder}
              onDeleteFolder={handleDeleteFolder}
              onContextFolder={(folder, x, y) => openMenu({ kind: 'folder', x, y, folderId: folder.id })}
              onContextItem={(item, x, y) => openMenu({ kind: 'item', x, y, itemId: item.id })}
              onContextBlank={(x, y) => openMenu({ kind: 'blank', x, y })}
              onLocateNode={locateNode}
              onDropNode={handleDropNode}
            />
          )}
        </div>

        {/* 右详情面板 */}
        <div className="kb-detail" ref={detailPaneRef}>
          {/* 折叠清单切换（常驻，未选中条目也可用） */}
          <button
            className="kb-list-toggle"
            onClick={toggleList}
            title={listCollapsed ? '展开列表' : '收起列表'}
            aria-label={listCollapsed ? '展开列表' : '收起列表'}
          >
            {listCollapsed ? '⟩' : '⟨'}
          </button>
          {!selected && !loading && visibleItems.length > 0 && (
            <div className="kb-empty">选择一条知识查看详情</div>
          )}
          {/* 正文未就绪（按 id 拉取中）：先占位，不挂载编辑器——空正文回灌会覆盖用户输入 */}
          {selected && !selectedBodyReady && !bodyLoadError && (
            <div className="kb-loading-hint" role="status" aria-live="polite">加载正文…</div>
          )}
          {selected && !selectedBodyReady && bodyLoadError && (
            <div className="kb-empty">
              正文加载失败：{bodyLoadError}
              <Button
                variant="ghost"
                size="xs"
                onClick={() => { setBodyLoadError(null); setBodyEpoch((n) => n + 1) }}
              >
                重试
              </Button>
            </div>
          )}
          {selected && selectedBodyReady && (
            <KbItemDetail
              key={selected.id}
              item={selected}
              refCount={refCountOf(selected.id)}
              focusTitleSignal={titleFocusSignal}
              onDeleted={(id) => { if (selectedId === id) setSelectedId(null); void refresh() }}
              onSaved={handleSaved}
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

      {/* 右键菜单（tasks 2.4）：文件夹 / 条目 / 空白 / 输入框四类 */}
      {menu && menuItems && (
        <KbContextMenu
          x={menu.x}
          y={menu.y}
          items={menuItems}
          onClose={closeMenu}
          ariaLabel={
            menu.kind === 'folder'
              ? '文件夹操作'
              : menu.kind === 'item'
                ? '条目操作'
                : menu.kind === 'blank'
                  ? '新建与展开操作'
                  : '编辑操作'
          }
        />
      )}
    </div>
  )
}
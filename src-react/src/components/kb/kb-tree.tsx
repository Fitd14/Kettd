import { useCallback, useEffect, useRef, useState } from 'react'
import type {
  CSSProperties,
  DragEvent as ReactDragEvent,
  MouseEvent as ReactMouseEvent,
  ReactNode,
} from 'react'
import {
  ChevronDown,
  ChevronRight,
  FilePlus,
  FileText,
  Folder,
  FolderOpen,
  FolderPlus,
  Inbox,
  Layers,
  MoreHorizontal,
  Pencil,
  Pin,
  Trash2,
} from 'lucide-react'
import type { KbFolder, KbItem } from '@/lib/api'
import { KbContextMenu } from '@/components/kb/kb-context-menu'
import type { KbMenuItem } from '@/components/kb/kb-context-menu'
import {
  findKbFolderNode,
  KB_DND_MIME,
  planAppendDrop,
  planKbDrop,
  resolveDropTarget,
  UNFILED_EXPAND_KEY,
} from '@/components/kb/kb-tree-utils'
import type {
  FolderSelection,
  KbDragPayload,
  KbDropPlan,
  KbDropZone,
  KbFlatResult,
  KbPinnedNode,
  KbTree,
  KbTreeFolderNode,
} from '@/components/kb/kb-tree-utils'

/**
 * 左栏单棵树（S1 tasks 2.2 + S2 tasks 3.2–3.4 / design D1/D5/D6）：
 * - 固定两行「全部知识」「未分类」+ 可展开/折叠的文件夹树，条目是叶子行；
 * - 只渲染展开分支（关闭的分支整枝不挂载，条目上限 5000 时仍可控）；
 * - 展开态由父级 KbView 持有并落 localStorage，本组件只读 + 回调；
 * - 文件夹行沿用旧 hover 图标：新建（树头）、重命名、删除（两步确认）；
 * - 右键菜单只上报命中对象与视口坐标，菜单本体由 KbView 统一渲染（tasks 2.4）；
 * - S2 拖拽：HTML5 DnD + `text/kettd-kb`，落点区判定与落库计划都是纯函数（kb-tree-utils）；
 *   拖拽状态（dragPayloadRef / dropHover）只在本组件局部，避免拖拽每帧重渲染 KbView；
 * - S2 置顶：树头下固定「置顶」快捷区（只读副本，独立于展开态/折叠态），点击定位；
 *   原位置行保留并带图钉角标，右键置顶/取消置顶两处共用。
 */
export interface KbFileTreeProps {
  tree: KbTree
  expanded: ReadonlySet<string>
  selectedItemId: string | null
  folderSelection: FolderSelection
  /** 文件夹列表加载失败的后端文案（显示在树头下，不阻塞「全部/未分类」） */
  folderLoadError?: string | null
  /** 空库时放在固定行之后的引导内容（如 KbEmptyState） */
  emptyContent?: ReactNode
  /** 置顶快捷区（tasks 3.4）：已按 compareKbNodes 排序；空数组不渲染该区 */
  pinnedNodes?: KbPinnedNode[]
  /** 置顶行点击 = 定位（选中 + 展开祖先 + scrollIntoView 由 KbView 统一执行） */
  onLocateNode?: (target: { kind: 'folder' | 'item'; id: string }) => void
  /** 树内拖放：KbView 负责乐观更新 / 落库 / 失败回滚；未传则整树禁用拖拽 */
  onDropNode?: (dragged: KbDragPayload, plan: KbDropPlan) => void
  onToggleExpand: (id: string) => void
  onSelectItem: (id: string) => void
  onSelectFolder: (selection: FolderSelection) => void
  onCreateFolder: () => void
  /** 新建条目（树头「额外操作」菜单的第二项）：KbView 负责建条目并聚焦标题 */
  onCreateItem?: () => void
  /** 打开本地数据目录（树头第一个图标）；未传则不渲染该图标 */
  onOpenDataFolder?: () => void
  onRenameFolder: (folder: KbFolder) => void
  onDeleteFolder: (folder: KbFolder) => void | Promise<void>
  /** 文件夹行右键（tasks 2.4）：菜单由 KbView 持有，树内只上报 */
  onContextFolder?: (folder: KbFolder, x: number, y: number) => void
  /** 条目叶子行右键 */
  onContextItem?: (item: KbItem, x: number, y: number) => void
  /** 树空白区右键（新建文件夹 / 新建文件 / 展开收起全部） */
  onContextBlank?: (x: number, y: number) => void
}

const DELETE_CONFIRM_MS = 3000

/** 600ms 悬停展开的等待时间（design D5） */
const HOVER_EXPAND_MS = 600

/** 拖拽落点高亮状态：行（含区）或树底部空区（追加） */
type KbDropHover =
  | { kind: 'folder' | 'item'; id: string; zone: KbDropZone }
  | { kind: 'blank' }

/** 层级缩进走 CSS 变量：折叠态可在样式表里整体压平，无需覆盖行内 style */
function depthStyle(depth: number): CSSProperties {
  return { '--kb-depth': depth } as CSSProperties
}

export function KbFileTree({
  tree,
  expanded,
  selectedItemId,
  folderSelection,
  folderLoadError,
  emptyContent,
  pinnedNodes,
  onLocateNode,
  onDropNode,
  onToggleExpand,
  onSelectItem,
  onSelectFolder,
  onCreateFolder,
  onCreateItem,
  onOpenDataFolder,
  onRenameFolder,
  onDeleteFolder,
  onContextFolder,
  onContextItem,
  onContextBlank,
}: KbFileTreeProps) {
  const [confirmingId, setConfirmingId] = useState<string | null>(null)
  const confirmTimer = useRef<number | null>(null)
  // 树头「额外操作」菜单锚点：null = 关闭。菜单复用 KbContextMenu（portal + 视口 clamp）
  const [headMenu, setHeadMenu] = useState<{ x: number; y: number } | null>(null)
  const headMoreRef = useRef<HTMLButtonElement>(null)

  const clearConfirmTimer = useCallback(() => {
    if (confirmTimer.current !== null) {
      window.clearTimeout(confirmTimer.current)
      confirmTimer.current = null
    }
  }, [])

  useEffect(() => clearConfirmTimer, [clearConfirmTimer])

  /** 删除按钮：第一次进入确认态，第二次执行（2.4 右键菜单上线前沿用旧交互） */
  const handleDeleteClick = useCallback(
    (folder: KbFolder) => {
      if (confirmingId === folder.id) {
        clearConfirmTimer()
        setConfirmingId(null)
        void onDeleteFolder(folder)
        return
      }
      clearConfirmTimer()
      setConfirmingId(folder.id)
      confirmTimer.current = window.setTimeout(() => {
        confirmTimer.current = null
        setConfirmingId(null)
      }, DELETE_CONFIRM_MS)
    },
    [confirmingId, clearConfirmTimer, onDeleteFolder],
  )

  // ── S2 树内拖拽（tasks 3.2 / design D5）────────────────────────────────
  const dndEnabled = Boolean(onDropNode)
  const scrollRef = useRef<HTMLDivElement | null>(null)
  /** 正在拖的节点（dataTransfer 在 dragover 时读不到 setData 的值，只能用 ref 传递） */
  const dragPayloadRef = useRef<KbDragPayload | null>(null)
  /** 落点高亮：只让树自身重渲染，KbView 不参与（design D10） */
  const [dropHover, setDropHover] = useState<KbDropHover | null>(null)
  const hoverTimerRef = useRef<number | null>(null)
  const hoverFolderRef = useRef<string | null>(null)

  const clearHoverTimer = useCallback(() => {
    if (hoverTimerRef.current !== null) {
      window.clearTimeout(hoverTimerRef.current)
      hoverTimerRef.current = null
    }
    hoverFolderRef.current = null
  }, [])

  /** 600ms 悬停展开：只对「折叠且有子项」的文件夹，同一目标只保留一个计时器 */
  const scheduleHoverExpand = useCallback(
    (folderId: string) => {
      if (!dndEnabled || hoverFolderRef.current === folderId) return
      clearHoverTimer()
      hoverFolderRef.current = folderId
      hoverTimerRef.current = window.setTimeout(() => {
        hoverTimerRef.current = null
        if (hoverFolderRef.current !== folderId) return
        hoverFolderRef.current = null
        onToggleExpand(folderId)
      }, HOVER_EXPAND_MS)
    },
    [clearHoverTimer, dndEnabled, onToggleExpand],
  )

  const resetDragState = useCallback(() => {
    dragPayloadRef.current = null
    clearHoverTimer()
    setDropHover(null)
  }, [clearHoverTimer])

  // 卸载清计时器（与删除确认计时器同理，避免 setState on unmounted）
  useEffect(() => resetDragState, [resetDragState])

  // 拖拽可能在树外结束（Esc / 丢到窗口外 / 拖到置顶区）：窗口级兜底清理高亮
  useEffect(() => {
    window.addEventListener('dragend', resetDragState)
    window.addEventListener('drop', resetDragState)
    return () => {
      window.removeEventListener('dragend', resetDragState)
      window.removeEventListener('drop', resetDragState)
    }
  }, [resetDragState])

  const beginDrag = useCallback(
    (event: ReactDragEvent<HTMLElement>, kind: 'folder' | 'item', id: string) => {
      if (!dndEnabled) {
        event.preventDefault()
        return
      }
      dragPayloadRef.current = { kind, id }
      clearHoverTimer()
      event.dataTransfer.effectAllowed = 'move'
      try {
        event.dataTransfer.setData(KB_DND_MIME, JSON.stringify({ kind, id }))
      } catch {
        // WebView2 对自定义 MIME 的 setData 偶发抛错：内部 ref 仍可用，拖拽照常进行
      }
    },
    [clearHoverTimer, dndEnabled],
  )

  /** 从事件目标向上找「带 data-kb-row-* 的树行」；拖拽高亮只认树滚动区内的行 */
  const findTreeRow = (
    eventTarget: EventTarget | null,
  ): { kind: 'folder' | 'item'; id: string; element: HTMLElement } | null => {
    if (!(eventTarget instanceof Element)) return null
    const row = eventTarget.closest<HTMLElement>('[data-kb-row-id]')
    if (!row || !scrollRef.current || !scrollRef.current.contains(row)) return null
    const kind = row.dataset.kbRowKind
    const id = row.dataset.kbRowId
    if ((kind !== 'folder' && kind !== 'item') || !id) return null
    return { kind, id, element: row }
  }

  /**
   * 行矩形 + 指针 y → 高亮落点。
   * 条目行没有「移入」语义：中区按上/下半区折成 before/after（design D5 只给文件夹移入）。
   * 返回 null = 前端预判拒绝（自身/后代/无法放置），不显示高亮、drop 不生效。
   */
  const resolveRowHover = (
    row: { kind: 'folder' | 'item'; id: string; element: HTMLElement },
    clientY: number,
    dragged: KbDragPayload,
  ): KbDropHover | null => {
    const rect = row.element.getBoundingClientRect()
    let zone = resolveDropTarget({ top: rect.top, height: rect.height }, clientY)
    if (row.kind === 'item' && zone === 'inside') {
      zone = clientY < rect.top + rect.height / 2 ? 'before' : 'after'
    }
    const plan = planKbDrop(tree, dragged, { kind: row.kind, id: row.id }, zone)
    if (!plan) return null
    return { kind: row.kind, id: row.id, zone }
  }

  const handleTreeDragOver = (event: ReactDragEvent<HTMLDivElement>) => {
    const dragged = dragPayloadRef.current
    if (!dragged || !dndEnabled) return
    const types = event.dataTransfer?.types
    if (!types || !Array.from(types).includes(KB_DND_MIME)) return
    // 只有内部拖拽才 preventDefault：外部文件拖入由 KbView 的窗口级 Files 拦截处理
    event.preventDefault()
    event.dataTransfer.dropEffect = 'move'

    const row = findTreeRow(event.target)
    if (!row) {
      clearHoverTimer()
      // 空区 = 追加到当前选中文件夹（无选中 = 根/未分类）；计划不成立（如拖进自身子树）不亮落点
      const appendParentId = folderSelection.kind === 'folder' ? folderSelection.id : null
      const appendPlan = planAppendDrop(tree, dragged, appendParentId)
      setDropHover((prev) => {
        if (!appendPlan) return prev === null ? prev : null
        return prev?.kind === 'blank' ? prev : { kind: 'blank' }
      })
      return
    }
    const hover = resolveRowHover(row, event.clientY, dragged)
    if (!hover || hover.kind === 'blank') {
      clearHoverTimer()
      setDropHover((prev) => (prev === null ? prev : null))
      return
    }
    setDropHover((prev) => (
      prev && prev.kind === hover.kind && prev.id === hover.id && prev.zone === hover.zone
        ? prev
        : hover
    ))
    if (hover.kind === 'folder' && hover.zone === 'inside') {
      const node = findKbFolderNode(tree, hover.id)
      const hasChildren = Boolean(node && (node.folders.length > 0 || node.items.length > 0))
      if (hasChildren && !expanded.has(hover.id)) scheduleHoverExpand(hover.id)
      else clearHoverTimer()
    } else {
      clearHoverTimer()
    }
  }

  const handleTreeDragLeave = (event: ReactDragEvent<HTMLDivElement>) => {
    if (!dragPayloadRef.current) return
    // 在行与子元素之间移动也会触发 dragleave：relatedTarget 仍在滚动区就忽略
    const related = event.relatedTarget
    if (related instanceof Node && scrollRef.current?.contains(related)) return
    clearHoverTimer()
    setDropHover(null)
  }

  const handleTreeDrop = (event: ReactDragEvent<HTMLDivElement>) => {
    const dragged = dragPayloadRef.current
    if (!dragged || !dndEnabled) return
    const types = event.dataTransfer?.types
    if (!types || !Array.from(types).includes(KB_DND_MIME)) return
    event.preventDefault()
    event.stopPropagation()

    let plan: KbDropPlan | null
    const row = findTreeRow(event.target)
    if (row) {
      const rect = row.element.getBoundingClientRect()
      let zone = resolveDropTarget({ top: rect.top, height: rect.height }, event.clientY)
      if (row.kind === 'item' && zone === 'inside') {
        zone = event.clientY < rect.top + rect.height / 2 ? 'before' : 'after'
      }
      plan = planKbDrop(tree, dragged, { kind: row.kind, id: row.id }, zone)
    } else {
      // 空区追加：条目追加到当前选中文件夹（无选中 = 未分类），文件夹追加到该夹（无选中 = 根）
      const appendParentId = folderSelection.kind === 'folder' ? folderSelection.id : null
      plan = planAppendDrop(tree, dragged, appendParentId)
    }
    resetDragState()
    if (plan) onDropNode?.(dragged, plan)
  }

  /** 落点高亮 class：inside 整行高亮 / before-after 2px 插入线 */
  const dropClassOf = (kind: 'folder' | 'item', id: string): string => {
    if (!dropHover || dropHover.kind === 'blank') return ''
    if (dropHover.kind !== kind || dropHover.id !== id) return ''
    if (dropHover.zone === 'inside') return ' kb-tree-drop-inside'
    return dropHover.zone === 'before' ? ' kb-tree-drop-before' : ' kb-tree-drop-after'
  }

  const renderItem = (item: KbItem, depth: number) => {
    const active = item.id === selectedItemId
    return (
      <button
        key={item.id}
        type="button"
        role="treeitem"
        aria-selected={active}
        className={`kb-tree-item${active ? ' is-active' : ''}${dropClassOf('item', item.id)}`}
        style={depthStyle(depth)}
        data-kind="item"
        data-kb-key={`item:${item.id}`}
        data-kb-row-kind="item"
        data-kb-row-id={item.id}
        draggable={dndEnabled}
        onDragStart={(event) => beginDrag(event, 'item', item.id)}
        onDragEnd={resetDragState}
        onClick={() => onSelectItem(item.id)}
        onContextMenu={(event) => {
          // 右键不触发 onClick；没有上报回调时保留浏览器原生菜单
          if (!onContextItem) return
          event.preventDefault()
          event.stopPropagation()
          onContextItem(item, event.clientX, event.clientY)
        }}
        title={item.title}
      >
        <FileText size={12} className="kb-folder-ico" aria-hidden="true" />
        <span className="item-title">{item.title}</span>
        {item.pinned && (
          <Pin size={10} className="kb-pin-badge" aria-label="已置顶" aria-hidden="false" />
        )}
      </button>
    )
  }

  const renderFolder = (node: KbTreeFolderNode, depth: number): ReactNode => {
    const hasChildren = node.folders.length > 0 || node.items.length > 0
    const isExpanded = expanded.has(node.id)
    const active = folderSelection.kind === 'folder' && folderSelection.id === node.id
    const confirming = confirmingId === node.id
    return (
      <div key={node.id} className="kb-tree-branch">
        <div
          className={`kb-folder-row-wrap${active ? ' is-active' : ''}${confirming ? ' is-confirming' : ''}${dropClassOf('folder', node.id)}`}
          style={depthStyle(depth)}
          data-kind="folder"
          data-kb-key={`folder:${node.id}`}
          data-kb-row-kind="folder"
          data-kb-row-id={node.id}
          onContextMenu={(event) => {
            if (!onContextFolder) return
            event.preventDefault()
            event.stopPropagation()
            onContextFolder(node.folder, event.clientX, event.clientY)
          }}
        >
          {hasChildren ? (
            <button
              type="button"
              className="kb-folder-caret"
              onClick={() => onToggleExpand(node.id)}
              title={isExpanded ? '收起子项' : '展开子项'}
              aria-label={isExpanded ? '收起子项' : '展开子项'}
              aria-expanded={isExpanded}
            >
              {isExpanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
            </button>
          ) : (
            <span className="kb-folder-caret is-empty" aria-hidden="true" />
          )}
          <button
            type="button"
            role="treeitem"
            aria-expanded={hasChildren ? isExpanded : undefined}
            aria-selected={active}
            className={`kb-folder-row${active ? ' is-active' : ''}`}
            draggable={dndEnabled}
            onDragStart={(event) => beginDrag(event, 'folder', node.id)}
            onDragEnd={resetDragState}
            onClick={() => {
              onSelectFolder({ kind: 'folder', id: node.id })
              if (hasChildren) onToggleExpand(node.id)
            }}
            title={node.name}
          >
            {isExpanded && hasChildren ? (
              <FolderOpen size={12} className="kb-folder-ico" aria-hidden="true" />
            ) : (
              <Folder size={12} className="kb-folder-ico" aria-hidden="true" />
            )}
            <span className="kb-folder-name">{node.name}</span>
            {node.pinned && (
              <Pin size={10} className="kb-pin-badge" aria-label="已置顶" aria-hidden="false" />
            )}
            <span className="kb-folder-count">{node.items.length}</span>
          </button>
          <span className="kb-folder-actions">
            {confirming ? (
              <button
                type="button"
                className="kb-folder-action is-danger"
                title="确认删除文件夹"
                onClick={() => handleDeleteClick(node.folder)}
              >
                确认删除
              </button>
            ) : (
              <>
                <button
                  type="button"
                  className="kb-folder-action"
                  title="重命名文件夹"
                  aria-label="重命名文件夹"
                  onClick={() => onRenameFolder(node.folder)}
                >
                  <Pencil size={11} />
                </button>
                <button
                  type="button"
                  className="kb-folder-action"
                  title="删除文件夹"
                  aria-label="删除文件夹"
                  onClick={() => handleDeleteClick(node.folder)}
                >
                  <Trash2 size={11} />
                </button>
              </>
            )}
          </span>
        </div>
        {isExpanded && hasChildren && (
          <>
            {node.folders.map((child) => renderFolder(child, depth + 1))}
            {node.items.map((item) => renderItem(item, depth + 1))}
          </>
        )}
      </div>
    )
  }

  const hasUnfiled = tree.unfiled.length > 0
  const unfiledExpanded = expanded.has(UNFILED_EXPAND_KEY)
  const pinned = pinnedNodes ?? []

  /**
   * 空白区右键：只在真正的留白上弹菜单。
   * 行/按钮/输入框等交互元素有各自的原生或自定义行为，不当作空白（避免抢掉按钮上的右键）。
   */
  const handleBlankContextMenu = (event: ReactMouseEvent<HTMLDivElement>) => {
    if (!onContextBlank) return
    const target = event.target
    const interactive = target instanceof Element && target.closest(
      '[data-kind="folder"], [data-kind="item"], button, input, textarea, a, select, [role="button"]',
    )
    if (interactive) return
    event.preventDefault()
    onContextBlank(event.clientX, event.clientY)
  }

  // 树头「额外操作」菜单：只有新建两项。菜单项不自行关闭，关闭由子树统一处理
  const headMenuItems: KbMenuItem[] = [
    {
      label: '新建文件夹',
      icon: <FolderPlus size={13} strokeWidth={1.7} />,
      onSelect: () => { setHeadMenu(null); onCreateFolder() },
    },
    {
      label: '新建文件',
      icon: <FilePlus size={13} strokeWidth={1.7} />,
      onSelect: () => { setHeadMenu(null); onCreateItem?.() },
    },
  ]

  return (
    <div className="kb-tree">
      <div className="kb-tree-head">
        <span className="kb-tree-title">目录</span>
        <div className="kb-tree-head-actions">
          {onOpenDataFolder && (
            <button
              type="button"
              className="kb-folder-add"
              title="打开本地数据目录"
              aria-label="打开本地数据目录"
              onClick={onOpenDataFolder}
            >
              <FolderOpen size={13} />
            </button>
          )}
          <button
            ref={headMoreRef}
            type="button"
            className="kb-folder-add"
            title="更多操作：新建文件夹 / 新建文件"
            aria-label="更多操作"
            aria-haspopup="menu"
            aria-expanded={headMenu !== null}
            onClick={() => {
              const rect = headMoreRef.current?.getBoundingClientRect()
              if (!rect) return
              setHeadMenu({ x: rect.left, y: rect.bottom + 4 })
            }}
          >
            <MoreHorizontal size={13} />
          </button>
        </div>
      </div>

      {headMenu && (
        <KbContextMenu
          x={headMenu.x}
          y={headMenu.y}
          ariaLabel="新建操作"
          onClose={() => setHeadMenu(null)}
          items={headMenuItems}
        />
      )}

      {folderLoadError && (
        <div className="kb-folder-error" role="alert">
          {folderLoadError}
        </div>
      )}

      {/* S2 tasks 3.4：置顶快捷区。独立于展开态与折叠态，只读副本、不参与拖拽/落点；
          空置顶区不渲染。右键复用原位置的文件夹/条目菜单（置顶/取消置顶两处都可用）。 */}
      {pinned.length > 0 && (
        <div className="kb-pin-section" role="group" aria-label="置顶快捷区">
          <div className="kb-pin-title">
            <Pin size={11} aria-hidden="true" />
            <span>置顶</span>
          </div>
          {pinned.map((node) => (
            <button
              key={`${node.kind}:${node.id}`}
              type="button"
              className="kb-pin-row"
              title={`${node.kind === 'folder' ? '文件夹' : '条目'}：${node.name}\n路径：${node.pathLabel}`}
              aria-label={`置顶${node.kind === 'folder' ? '文件夹' : '条目'} ${node.name}，路径 ${node.pathLabel}`}
              onClick={() => onLocateNode?.({ kind: node.kind, id: node.id })}
              onContextMenu={(event) => {
                event.preventDefault()
                event.stopPropagation()
                if (node.kind === 'folder') {
                  if (node.folder && onContextFolder) {
                    onContextFolder(node.folder, event.clientX, event.clientY)
                  }
                  return
                }
                if (node.item && onContextItem) {
                  onContextItem(node.item, event.clientX, event.clientY)
                }
              }}
            >
              {node.kind === 'folder' ? (
                <Folder size={11} className="kb-pin-ico" aria-hidden="true" />
              ) : (
                <FileText size={11} className="kb-pin-ico" aria-hidden="true" />
              )}
              <span className="kb-pin-name">{node.name}</span>
              <Pin size={10} className="kb-pin-star" aria-hidden="true" />
            </button>
          ))}
        </div>
      )}

      <div
        ref={scrollRef}
        className={`kb-tree-scroll${dropHover?.kind === 'blank' ? ' is-drop-append' : ''}`}
        role="tree"
        aria-label="知识库文件树"
        data-kind="blank"
        onContextMenu={handleBlankContextMenu}
        onDragOver={handleTreeDragOver}
        onDragLeave={handleTreeDragLeave}
        onDrop={handleTreeDrop}
        onDragEnd={resetDragState}
      >
        <div className="kb-folder-fixed">
          <button
            type="button"
            className={`kb-folder-row${folderSelection.kind === 'all' ? ' is-active' : ''}`}
            onClick={() => onSelectFolder({ kind: 'all' })}
            aria-pressed={folderSelection.kind === 'all'}
          >
            <Layers size={12} className="kb-folder-ico" aria-hidden="true" />
            <span className="kb-folder-name">全部知识</span>
            <span className="kb-folder-count">{tree.totalItems}</span>
          </button>

          <div className="kb-folder-row-wrap" style={depthStyle(0)}>
            {hasUnfiled ? (
              <button
                type="button"
                className="kb-folder-caret"
                onClick={() => onToggleExpand(UNFILED_EXPAND_KEY)}
                title={unfiledExpanded ? '收起未分类' : '展开未分类'}
                aria-label={unfiledExpanded ? '收起未分类' : '展开未分类'}
                aria-expanded={unfiledExpanded}
              >
                {unfiledExpanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
              </button>
            ) : (
              <span className="kb-folder-caret is-empty" aria-hidden="true" />
            )}
            <button
              type="button"
              role="treeitem"
              aria-expanded={hasUnfiled ? unfiledExpanded : undefined}
              aria-selected={folderSelection.kind === 'unfiled'}
              className={`kb-folder-row${folderSelection.kind === 'unfiled' ? ' is-active' : ''}`}
              onClick={() => {
                onSelectFolder({ kind: 'unfiled' })
                if (hasUnfiled) onToggleExpand(UNFILED_EXPAND_KEY)
              }}
              title="未分类"
            >
              <Inbox size={12} className="kb-folder-ico" aria-hidden="true" />
              <span className="kb-folder-name">未分类</span>
              <span className="kb-folder-count">{tree.unfiled.length}</span>
            </button>
          </div>

          {hasUnfiled && unfiledExpanded && tree.unfiled.map((item) => renderItem(item, 1))}
        </div>

        {tree.roots.map((node) => renderFolder(node, 0))}

        {tree.roots.length === 0 && !emptyContent && (
          <div className="kb-folder-empty">暂无文件夹</div>
        )}

        {emptyContent}
      </div>
    </div>
  )
}

/**
 * 过滤态扁平结果列表（tasks 2.3 / design D2）：
 * 搜索词或标签生效时替代整棵树，每行 = 标题 + 所在文件夹路径面包屑，只显示命中项。
 */
export interface KbFlatResultsProps {
  results: KbFlatResult[]
  selectedItemId: string | null
  onSelect: (id: string) => void
  /** 过滤态仍保留条目右键（移动/置顶/删除），tasks 2.4 */
  onContextItem?: (item: KbItem, x: number, y: number) => void
}

export function KbFlatResults({ results, selectedItemId, onSelect, onContextItem }: KbFlatResultsProps) {
  return (
    <div className="kb-item-list kb-flat-list" role="listbox" aria-label="知识检索结果">
      {results.map(({ item, title, pathLabel }) => {
        const active = item.id === selectedItemId
        return (
          <button
            key={item.id}
            type="button"
            role="option"
            aria-selected={active}
            className={`kb-item-row kb-flat-row${active ? ' active' : ''}`}
            onClick={() => onSelect(item.id)}
            onContextMenu={(event) => {
              if (!onContextItem) return
              event.preventDefault()
              event.stopPropagation()
              onContextItem(item, event.clientX, event.clientY)
            }}
            data-kind="flat-item"
            title={`${pathLabel} / ${title}`}
          >
            <span className="item-title">{title}</span>
            <span className="item-meta kb-flat-path" title={pathLabel}>
              {pathLabel}
            </span>
          </button>
        )
      })}
    </div>
  )
}
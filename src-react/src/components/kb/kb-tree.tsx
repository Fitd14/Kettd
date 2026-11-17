import { useCallback, useEffect, useRef, useState } from 'react'
import type {
  CSSProperties,
  MouseEvent as ReactMouseEvent,
  PointerEvent as ReactPointerEvent,
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
import { Button } from '@/components/ui/button'
import { KbContextMenu } from '@/components/kb/kb-context-menu'
import type { KbMenuItem } from '@/components/kb/kb-context-menu'
import {
  computeAutoScrollSpeed,
  findKbFolderNode,
  planAppendDrop,
  planKbDrop,
  resolveBlankDropParent,
  resolveDropTarget,
  shouldStartDrag,
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
 * - S3 拖拽：Pointer Events（阈值 + 自动滚动 + 悬停展开 + 幽灵标签），落点区判定与
 *   落库计划仍是 kb-tree-utils 的纯函数；拖拽态（pointerRef / dropHover）只在本组件局部，
 *   避免拖拽每帧重渲染 KbView；
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

/** 一次指针交互：active=false 时只是按下候选，越阈值后才成为拖拽 */
interface KbPointerDrag {
  payload: KbDragPayload
  /** 幽灵标签文案（按下那一行的可见文本） */
  label: string
  pointerId: number
  pointerType: string
  button: number
  startX: number
  startY: number
  startedAt: number
  active: boolean
}

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

  // ── S3 树内拖拽（tasks 1.2–1.6 / design D2/D3/D4/D6）──────────────────
  // HTML5 DnD 在 WebView2 下被窗口级拖放接管吞掉，这里改用 Pointer Events：
  // 阈值判定 → 置 dragging 态 → elementFromPoint 求落点 → pointerup 提交计划。
  const dndEnabled = Boolean(onDropNode)
  const rootRef = useRef<HTMLDivElement | null>(null)
  const scrollRef = useRef<HTMLDivElement | null>(null)
  const ghostRef = useRef<HTMLDivElement | null>(null)
  /** 按下候选 / 拖拽态共用一份；active=false 表示还没越过阈值（此时仍算点击） */
  const pointerRef = useRef<KbPointerDrag | null>(null)
  const lastPointRef = useRef<{ x: number; y: number } | null>(null)
  const rafRef = useRef<number | null>(null)
  /** 越阈值后吞掉这次 click，避免「拖完顺手选中/展开」 */
  const suppressClickRef = useRef(false)
  const [dragging, setDragging] = useState<{ payload: KbDragPayload; label: string } | null>(null)
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

  /** 自动滚动：拖拽期间常驻 rAF，只有指针停在边缘带里才真的动（design D6） */
  const stopAutoScroll = useCallback(() => {
    if (rafRef.current !== null) {
      window.cancelAnimationFrame(rafRef.current)
      rafRef.current = null
    }
  }, [])

  // 卸载清计时器与 rAF（避免 setState on unmounted）
  useEffect(() => () => {
    clearHoverTimer()
    stopAutoScroll()
  }, [clearHoverTimer, stopAutoScroll])

  /**
   * 指针位置 → 落点高亮 + 落库计划（design D3/D4/D8）：
   * - 命中树行：按行矩形切成 before/inside/after，条目行的 inside 折成上/下半；
   * - 命中「未分类」：条目 → 追加到未分类，文件夹 → 拒绝；
   * - 命中「全部知识」：一律拒绝（design D8）；
   * - 命中树内空白：追加到当前选中文件夹；选中「全部知识」时无明确归属，拒绝。
   */
  const resolveHoverAt = useCallback(
    (x: number, y: number, dragged: KbDragPayload): { hover: KbDropHover; plan: KbDropPlan } | null => {
      const scroll = scrollRef.current
      const element = document.elementFromPoint(x, y)
      if (!element || !scroll || !scroll.contains(element)) return null

      if (element.closest('[data-kb-fixed="all"]')) return null
      if (element.closest('[data-kb-fixed="unfiled"]')) {
        if (dragged.kind === 'folder') return null
        const plan = planAppendDrop(tree, dragged, null)
        return plan ? { hover: { kind: 'blank' }, plan } : null
      }

      const row = element.closest<HTMLElement>('[data-kb-row-id]')
      const kind = row?.dataset.kbRowKind
      const id = row?.dataset.kbRowId
      if (row && (kind === 'folder' || kind === 'item') && id) {
        const rect = row.getBoundingClientRect()
        let zone = resolveDropTarget({ top: rect.top, height: rect.height }, y)
        if (kind === 'item' && zone === 'inside') {
          zone = y < rect.top + rect.height / 2 ? 'before' : 'after'
        }
        const plan = planKbDrop(tree, dragged, { kind, id }, zone)
        return plan ? { hover: { kind, id, zone }, plan } : null
      }

      if (folderSelection.kind === 'all') return null
      const appendParentId = resolveBlankDropParent(folderSelection, dragged)
      if (appendParentId === undefined) return null
      const plan = planAppendDrop(tree, dragged, appendParentId)
      return plan ? { hover: { kind: 'blank' }, plan } : null
    },
    [folderSelection, tree],
  )

  /** 落点高亮只在真正变化时 setState；悬停展开与高亮同一处推进（design D7） */
  const applyHover = useCallback(
    (resolved: { hover: KbDropHover; plan: KbDropPlan } | null) => {
      setDropHover((prev) => {
        const next = resolved ? resolved.hover : null
        if (prev === null || next === null) return prev === next ? prev : next
        if (prev.kind === 'blank' || next.kind === 'blank') {
          return prev.kind === next.kind ? prev : next
        }
        return prev.kind === next.kind && prev.id === next.id && prev.zone === next.zone ? prev : next
      })
      if (resolved && resolved.hover.kind === 'folder' && resolved.hover.zone === 'inside') {
        const node = findKbFolderNode(tree, resolved.hover.id)
        const hasChildren = Boolean(node && (node.folders.length > 0 || node.items.length > 0))
        if (hasChildren && !expanded.has(resolved.hover.id)) scheduleHoverExpand(resolved.hover.id)
        else clearHoverTimer()
      } else {
        clearHoverTimer()
      }
    },
    [clearHoverTimer, expanded, scheduleHoverExpand, tree],
  )

  const startAutoScroll = useCallback(() => {
    if (rafRef.current !== null) return
    const tick = () => {
      rafRef.current = null
      const pending = pointerRef.current
      const scroll = scrollRef.current
      const point = lastPointRef.current
      if (!pending?.active || !scroll || !point) return
      const rect = scroll.getBoundingClientRect()
      const speed = computeAutoScrollSpeed(point.y, { top: rect.top, height: rect.height })
      if (speed !== 0) {
        const before = scroll.scrollTop
        scroll.scrollTop = before + speed
        // 滚动后指针下方的行变了，落点要立刻重算
        if (scroll.scrollTop !== before) {
          applyHover(resolveHoverAt(point.x, point.y, pending.payload))
        }
      }
      rafRef.current = window.requestAnimationFrame(tick)
    }
    rafRef.current = window.requestAnimationFrame(tick)
  }, [applyHover, resolveHoverAt])

  /** 结束一次指针交互：commit=true 时按最终指针位置提交计划（design D9） */
  const finishPointerDrag = useCallback(
    (commit: boolean) => {
      const pending = pointerRef.current
      if (!pending) return
      pointerRef.current = null
      stopAutoScroll()
      clearHoverTimer()
      if (!pending.active) {
        setDropHover(null)
        return
      }
      // 拖过一次之后的 click 不是选择意图
      suppressClickRef.current = true
      const scroll = scrollRef.current
      try {
        if (scroll?.hasPointerCapture(pending.pointerId)) {
          scroll.releasePointerCapture(pending.pointerId)
        }
      } catch {
        // 指针已被系统回收：忽略
      }
      const point = lastPointRef.current
      setDragging(null)
      setDropHover(null)
      if (!commit || !point) return
      const resolved = resolveHoverAt(point.x, point.y, pending.payload)
      if (resolved) onDropNode?.(pending.payload, resolved.plan)
    },
    [clearHoverTimer, onDropNode, resolveHoverAt, stopAutoScroll],
  )

  /** 行上 pointerdown：只登记候选，不阻止默认行为（否则丢 click、丢右键） */
  const handleRowPointerDown = useCallback(
    (event: ReactPointerEvent<HTMLElement>, kind: 'folder' | 'item', id: string) => {
      if (!dndEnabled || event.button !== 0) return
      suppressClickRef.current = false
      pointerRef.current = {
        payload: { kind, id },
        label: (event.currentTarget.textContent ?? '').trim(),
        pointerId: event.pointerId,
        pointerType: event.pointerType,
        button: event.button,
        startX: event.clientX,
        startY: event.clientY,
        startedAt: performance.now(),
        active: false,
      }
      lastPointRef.current = { x: event.clientX, y: event.clientY }
    },
    [dndEnabled],
  )

  /** 越阈值后才吞 click；没拖过就照常选择 */
  const consumeDragClick = useCallback(() => {
    if (!suppressClickRef.current) return false
    suppressClickRef.current = false
    return true
  }, [])

  // 指针移动 / 抬起 / 取消 / Esc / 失焦：全部挂在 window 捕获阶段，
  // 拖到行外、树外、甚至窗口边缘时状态也能收口（design D2/D10）。
  useEffect(() => {
    if (!dndEnabled) return
    const onPointerMove = (event: PointerEvent) => {
      const pending = pointerRef.current
      if (!pending || event.pointerId !== pending.pointerId) return
      lastPointRef.current = { x: event.clientX, y: event.clientY }

      if (!pending.active) {
        const start = shouldStartDrag({
          kind: pending.payload.kind,
          button: pending.button,
          pointerType: pending.pointerType,
          heldMs: performance.now() - pending.startedAt,
          from: { x: pending.startX, y: pending.startY },
          to: { x: event.clientX, y: event.clientY },
        })
        if (!start) return
        pending.active = true
        setDragging({ payload: pending.payload, label: pending.label })
        try {
          scrollRef.current?.setPointerCapture(pending.pointerId)
        } catch {
          // 捕获失败不影响 window 上的监听
        }
        document.getSelection()?.removeAllRanges()
        startAutoScroll()
      }

      event.preventDefault()
      const ghost = ghostRef.current
      if (ghost) {
        ghost.style.transform = `translate3d(${event.clientX + 14}px, ${event.clientY + 12}px, 0)`
      }
      applyHover(resolveHoverAt(event.clientX, event.clientY, pending.payload))
    }
    const onPointerUp = (event: PointerEvent) => {
      if (pointerRef.current?.pointerId !== event.pointerId) return
      finishPointerDrag(true)
    }
    const onPointerCancel = (event: PointerEvent) => {
      if (pointerRef.current?.pointerId !== event.pointerId) return
      finishPointerDrag(false)
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      if (!pointerRef.current?.active) return
      event.preventDefault()
      event.stopPropagation()
      finishPointerDrag(false)
    }
    const onBlur = () => finishPointerDrag(false)

    window.addEventListener('pointermove', onPointerMove, true)
    window.addEventListener('pointerup', onPointerUp, true)
    window.addEventListener('pointercancel', onPointerCancel, true)
    window.addEventListener('keydown', onKeyDown, true)
    window.addEventListener('blur', onBlur)
    return () => {
      window.removeEventListener('pointermove', onPointerMove, true)
      window.removeEventListener('pointerup', onPointerUp, true)
      window.removeEventListener('pointercancel', onPointerCancel, true)
      window.removeEventListener('keydown', onKeyDown, true)
      window.removeEventListener('blur', onBlur)
      stopAutoScroll()
    }
  }, [applyHover, dndEnabled, finishPointerDrag, resolveHoverAt, startAutoScroll, stopAutoScroll])

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
        className={`kb-tree-item${active ? ' is-active' : ''}${dragging?.payload.kind === 'item' && dragging.payload.id === item.id ? ' is-dragging' : ''}${dropClassOf('item', item.id)}`}
        style={depthStyle(depth)}
        data-kind="item"
        data-kb-key={`item:${item.id}`}
        data-kb-row-kind="item"
        data-kb-row-id={item.id}
        onPointerDown={(event) => handleRowPointerDown(event, 'item', item.id)}
        onClick={() => {
          if (consumeDragClick()) return
          onSelectItem(item.id)
        }}
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
            className={`kb-folder-row${active ? ' is-active' : ''}${dragging?.payload.kind === 'folder' && dragging.payload.id === node.id ? ' is-dragging' : ''}`}
            onPointerDown={(event) => handleRowPointerDown(event, 'folder', node.id)}
            onClick={() => {
              if (consumeDragClick()) return
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
              <Button
                variant="danger"
                size="xs"
                title="确认删除文件夹"
                onClick={() => handleDeleteClick(node.folder)}
              >
                确认删除
              </Button>
            ) : (
              <>
                <Button
                  size="icon-xs"
                  title="重命名文件夹"
                  aria-label="重命名文件夹"
                  onClick={() => onRenameFolder(node.folder)}
                >
                  <Pencil size={11} />
                </Button>
                <Button
                  size="icon-xs"
                  title="删除文件夹"
                  aria-label="删除文件夹"
                  onClick={() => handleDeleteClick(node.folder)}
                >
                  <Trash2 size={11} />
                </Button>
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
    <div ref={rootRef} className={`kb-tree${dragging ? ' is-drag-active' : ''}`}>
      {dragging && (
        <div
          ref={(el) => {
            ghostRef.current = el
            const point = lastPointRef.current
            if (el && point) {
              el.style.transform = `translate3d(${point.x + 14}px, ${point.y + 12}px, 0)`
            }
          }}
          className="kb-drag-ghost"
          aria-hidden="true"
        >
          {dragging.payload.kind === 'folder' ? (
            <Folder size={12} className="kb-folder-ico" />
          ) : (
            <FileText size={12} className="kb-folder-ico" />
          )}
          <span className="kb-drag-ghost-name">{dragging.label}</span>
        </div>
      )}
      <div className="kb-tree-head">
        <span className="kb-tree-title">目录</span>
        <div className="kb-tree-head-actions">
          {onOpenDataFolder && (
            <Button
              size="icon-xs"
              title="打开本地数据目录"
              aria-label="打开本地数据目录"
              onClick={onOpenDataFolder}
            >
              <FolderOpen size={13} />
            </Button>
          )}
          <Button
            ref={headMoreRef}
            size="icon-xs"
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
          </Button>
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
      >
        <div className="kb-folder-fixed">
          <button
            type="button"
            className={`kb-folder-row${folderSelection.kind === 'all' ? ' is-active' : ''}`}
            onClick={() => onSelectFolder({ kind: 'all' })}
            aria-pressed={folderSelection.kind === 'all'}
            data-kb-fixed="all"
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
              data-kb-fixed="unfiled"
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
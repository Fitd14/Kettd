import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react'
import type { KeyboardEvent as ReactKeyboardEvent, ReactNode } from 'react'
import { createPortal } from 'react-dom'
import { ChevronRight } from 'lucide-react'

/**
 * 知识库右键菜单（S1 后半 tasks 2.4 / design D3）。
 *
 * - 自建轻量组件（不引第三方菜单库）：`createPortal` 到 `document.body` + `position: fixed`，
 *   避免被左栏 `overflow: hidden` 裁剪；z-index 120，高于详情弹窗（100）；
 * - 打开后量宽高并 clamp 进视口（四边 8px）：根菜单底部溢出上翻，子菜单右侧溢出改左侧；
 * - 关闭：Esc / Tab / 菜单外 mousedown / window blur / 任意滚动容器 scroll / resize；
 * - 键盘：↑↓ 循环、→ 进子菜单、← 返回父级（根层再 ← 关闭）、Enter/Space 执行、Tab 关闭；
 * - 菜单不自行关闭命中项（`onSelect` 由父级决定）：删除的「第一次确认、3s 回退」需要菜单保持打开。
 */
export interface KbMenuItem {
  label: string
  icon?: ReactNode
  disabled?: boolean
  danger?: boolean
  /** 禁用原因：显示在 title 上，说明为什么不可点（如「已在当前文件夹」） */
  hint?: string
  /** 二级菜单（如「移动到…」的文件夹列表） */
  submenu?: KbMenuItem[]
  /** 点击 / 回车 / 空格执行（菜单不自动关闭，由父级决定） */
  onSelect?: () => void
}

export interface KbContextMenuProps {
  x: number
  y: number
  items: KbMenuItem[]
  onClose: () => void
  ariaLabel?: string
}

/** 视口安全边距（design D3：四边 8px） */
const VIEW_MARGIN = 8
/** 子菜单相对父行的垂直偏移：子菜单首项与父行大致对齐 */
const SUBMENU_Y_OFFSET = 6
/** 子菜单与父级的水平搭接：避免中间出现无法跨越的缝（鼠标一移出父项子菜单就关） */
const SUBMENU_X_OVERLAP = 2

interface PanelAnchor {
  x: number
  y: number
  /** 右侧放不下时改为左侧弹出用的参考点（父行左缘） */
  flipX?: number
}

interface PanelPosition {
  left: number
  top: number
  ready: boolean
}

function clamp(value: number, min: number, max: number): number {
  if (max < min) return min
  return Math.min(Math.max(value, min), max)
}

/** 同一层面板内的菜单项按钮（排除更深层子菜单已渲染的项） */
function panelItems(panel: HTMLElement): HTMLButtonElement[] {
  return Array.from(panel.querySelectorAll<HTMLButtonElement>('button[role="menuitem"]')).filter(
    (button) => button.closest('[data-kb-menu-level]') === panel,
  )
}

function isDisabled(button: HTMLButtonElement): boolean {
  return button.getAttribute('aria-disabled') === 'true'
}

/** 面板 DOM 上的路径编码：根面板是空串，子菜单是「父索引.本索引」 */
function panelPathOf(panel: HTMLElement): number[] {
  const raw = panel.dataset.kbMenuPath ?? ''
  if (!raw) return []
  return raw.split('.').map((part) => Number(part))
}

function panelKey(path: number[]): string {
  return path.join('.')
}

interface MenuPanelProps {
  items: KbMenuItem[]
  path: number[]
  anchor: PanelAnchor
  isRoot?: boolean
  ariaLabel?: string
  openPath: number[]
  anchors: Record<string, PanelAnchor>
  onOpenSubmenu: (parentPath: number[], index: number, trigger: HTMLElement) => void
  onHover: (path: number[]) => void
  onSelectItem: (item: KbMenuItem) => void
}

function MenuPanel({
  items,
  path,
  anchor,
  isRoot = false,
  ariaLabel,
  openPath,
  anchors,
  onOpenSubmenu,
  onHover,
  onSelectItem,
}: MenuPanelProps) {
  const panelRef = useRef<HTMLDivElement>(null)
  const [position, setPosition] = useState<PanelPosition>({ left: anchor.x, top: anchor.y, ready: false })

  /** 量宽高 → clamp 进视口；根菜单底部溢出上翻，子菜单右侧溢出左侧弹出 */
  const reposition = useCallback(() => {
    const el = panelRef.current
    if (!el) return
    const rect = el.getBoundingClientRect()
    const viewportW = window.innerWidth
    const viewportH = window.innerHeight
    const maxLeft = Math.max(VIEW_MARGIN, viewportW - rect.width - VIEW_MARGIN)
    const maxTop = Math.max(VIEW_MARGIN, viewportH - rect.height - VIEW_MARGIN)

    let left: number
    let top: number
    if (isRoot) {
      left = clamp(anchor.x, VIEW_MARGIN, maxLeft)
      top = anchor.y
      if (top + rect.height > viewportH - VIEW_MARGIN) top = anchor.y - rect.height
    } else {
      left = anchor.x
      if (left + rect.width > viewportW - VIEW_MARGIN) {
        left = (anchor.flipX ?? anchor.x) - rect.width
      }
      top = clamp(anchor.y, VIEW_MARGIN, maxTop)
    }
    left = clamp(left, VIEW_MARGIN, maxLeft)
    top = clamp(top, VIEW_MARGIN, maxTop)

    setPosition((prev) =>
      prev.ready && prev.left === left && prev.top === top ? prev : { left, top, ready: true },
    )
  }, [anchor.x, anchor.y, anchor.flipX, isRoot])

  // 每次渲染后量一次（字体/缩放/子菜单展开引起的尺寸变化都能纠偏）；同值 bail 不触发循环
  useLayoutEffect(() => {
    reposition()
  })

  // rAF 再兜一次：首帧 WebView2 的字体度量可能晚于 layout effect 生效
  useEffect(() => {
    const id = window.requestAnimationFrame(reposition)
    return () => window.cancelAnimationFrame(id)
  }, [reposition])

  return (
    <div
      ref={panelRef}
      role="menu"
      aria-label={ariaLabel}
      className="kb-context-menu"
      data-kb-menu-level={path.length}
      data-kb-menu-path={panelKey(path)}
      style={{
        left: `${position.left}px`,
        top: `${position.top}px`,
        visibility: position.ready ? 'visible' : 'hidden',
      }}
    >
      {items.map((item, index) => {
        const hasSubmenu = Array.isArray(item.submenu) && item.submenu.length > 0
        const submenuKey = panelKey([...path, index])
        const submenuOpen = hasSubmenu && openPath[path.length] === index
        const submenuAnchor = anchors[submenuKey]
        const disabled = item.disabled === true
        return (
          <div className="kb-context-menu-slot" key={`${index}:${item.label}`}>
            <button
              type="button"
              role="menuitem"
              tabIndex={-1}
              className={`kb-context-menu-item${item.danger ? ' is-danger' : ''}${
                disabled ? ' is-disabled' : ''
              }${submenuOpen ? ' is-open' : ''}`}
              aria-disabled={disabled ? true : undefined}
              aria-haspopup={hasSubmenu ? 'menu' : undefined}
              aria-expanded={hasSubmenu ? submenuOpen : undefined}
              data-menu-index={index}
              data-has-submenu={hasSubmenu ? '1' : undefined}
              title={item.hint}
              // 保住打开菜单前的焦点/选区（输入框编辑菜单要拿它做剪切/复制/粘贴）
              onMouseDown={(event) => event.preventDefault()}
              onClick={(event) => {
                if (disabled) return
                if (hasSubmenu) {
                  onOpenSubmenu(path, index, event.currentTarget)
                  return
                }
                onSelectItem(item)
              }}
              onMouseEnter={(event) => {
                if (disabled) return
                if (hasSubmenu) {
                  onOpenSubmenu(path, index, event.currentTarget)
                  return
                }
                // 悬停普通项：收起更深子菜单（菜单保持当前层）
                if (openPath.length > path.length) onHover(path)
              }}
            >
              {item.icon && (
                <span className="kb-context-menu-ico" aria-hidden="true">
                  {item.icon}
                </span>
              )}
              <span className="kb-context-menu-label">{item.label}</span>
              {hasSubmenu && (
                <ChevronRight size={12} className="kb-context-menu-arrow" aria-hidden="true" />
              )}
            </button>
            {hasSubmenu && submenuOpen && submenuAnchor && (
              <MenuPanel
                items={item.submenu ?? []}
                path={[...path, index]}
                anchor={submenuAnchor}
                ariaLabel={item.label}
                openPath={openPath}
                anchors={anchors}
                onOpenSubmenu={onOpenSubmenu}
                onHover={onHover}
                onSelectItem={onSelectItem}
              />
            )}
          </div>
        )
      })}
    </div>
  )
}

export function KbContextMenu({ x, y, items, onClose, ariaLabel = '知识库菜单' }: KbContextMenuProps) {
  const rootRef = useRef<HTMLDivElement>(null)
  const [openPath, setOpenPath] = useState<number[]>([])
  const [anchors, setAnchors] = useState<Record<string, PanelAnchor>>({})

  const openSubmenu = useCallback((parentPath: number[], index: number, trigger: HTMLElement) => {
    const rect = trigger.getBoundingClientRect()
    const key = panelKey([...parentPath, index])
    setAnchors((prev) => ({
      ...prev,
      [key]: {
        x: rect.right - SUBMENU_X_OVERLAP,
        y: rect.top - SUBMENU_Y_OFFSET,
        flipX: rect.left + SUBMENU_X_OVERLAP,
      },
    }))
    setOpenPath([...parentPath, index])
  }, [])

  /** 悬停更浅层时收起更深子菜单（路径相等则不动，避免子菜单自己被收起） */
  const collapseDeeperThan = useCallback((path: number[]) => {
    setOpenPath((prev) => (prev.length > path.length ? path : prev))
  }, [])

  const selectItem = useCallback((item: KbMenuItem) => {
    item.onSelect?.()
  }, [])

  // 外部交互关闭：mousedown / scroll 用 capture 才能先于内部处理器；blur / resize 兜底
  useEffect(() => {
    const closeIfOutside = (event: Event) => {
      const root = rootRef.current
      if (!root) return
      if (event.target instanceof Node && root.contains(event.target)) return
      onClose()
    }
    const onScroll = (event: Event) => {
      const root = rootRef.current
      if (!root) return
      // 菜单自身滚动（超长菜单 max-height）不关闭
      if (event.target instanceof Node && root.contains(event.target)) return
      onClose()
    }
    const onBlur = () => onClose()
    const onResize = () => onClose()
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.isComposing) return
      if (event.key === 'Escape') {
        event.preventDefault()
        event.stopPropagation()
        onClose()
        return
      }
      if (event.key === 'Tab') {
        // 焦点留在页面里但菜单已不可用：按 Tab 直接关闭，避免焦点漂到菜单外
        event.preventDefault()
        onClose()
      }
    }
    document.addEventListener('mousedown', closeIfOutside, true)
    document.addEventListener('scroll', onScroll, true)
    document.addEventListener('keydown', onKeyDown, true)
    window.addEventListener('blur', onBlur)
    window.addEventListener('resize', onResize)
    return () => {
      document.removeEventListener('mousedown', closeIfOutside, true)
      document.removeEventListener('scroll', onScroll, true)
      document.removeEventListener('keydown', onKeyDown, true)
      window.removeEventListener('blur', onBlur)
      window.removeEventListener('resize', onResize)
    }
  }, [onClose])

  // 打开后聚焦第一个可用项（tasks 2.4：键盘可达的前提）
  useEffect(() => {
    const root = rootRef.current
    if (!root) return
    const first = Array.from(root.querySelectorAll<HTMLButtonElement>('button[role="menuitem"]')).find(
      (button) => !isDisabled(button),
    )
    first?.focus()
  }, [])

  const focusFirstIn = useCallback((path: number[]) => {
    const root = rootRef.current
    if (!root) return
    const panel = root.querySelector<HTMLElement>(`[data-kb-menu-path="${panelKey(path)}"]`)
    if (!panel) return
    panelItems(panel).find((button) => !isDisabled(button))?.focus()
  }, [])

  const handleKeyDown = useCallback(
    (event: ReactKeyboardEvent<HTMLDivElement>) => {
      if (event.nativeEvent.isComposing) return
      const active = document.activeElement
      if (!(active instanceof HTMLButtonElement)) return
      const panel = active.closest<HTMLElement>('[data-kb-menu-level]')
      if (!panel || !rootRef.current?.contains(panel)) return
      const buttons = panelItems(panel)
      const enabled = buttons.filter((button) => !isDisabled(button))
      const index = buttons.indexOf(active)
      if (index < 0) return
      const path = panelPathOf(panel)

      const move = (step: number) => {
        event.preventDefault()
        if (enabled.length === 0) return
        const current = enabled.indexOf(active)
        const base = current < 0 ? (step > 0 ? -1 : 0) : current
        enabled[(base + step + enabled.length) % enabled.length].focus()
      }

      switch (event.key) {
        case 'ArrowDown':
          move(1)
          return
        case 'ArrowUp':
          move(-1)
          return
        case 'ArrowRight': {
          event.preventDefault()
          if (isDisabled(active) || !active.hasAttribute('data-has-submenu')) return
          const childPath = [...path, index]
          openSubmenu(path, index, active)
          // 子菜单可能已打开（重复按 →）：等 React 提交后再把焦点送进第一项
          window.requestAnimationFrame(() => focusFirstIn(childPath))
          return
        }
        case 'ArrowLeft': {
          event.preventDefault()
          if (path.length === 0) {
            onClose()
            return
          }
          const parentPath = path.slice(0, -1)
          const parentPanel = rootRef.current?.querySelector<HTMLElement>(
            `[data-kb-menu-path="${panelKey(parentPath)}"]`,
          )
          const parentIndex = path[path.length - 1]
          const trigger = parentPanel
            ? panelItems(parentPanel).find(
                (button) => Number(button.dataset.menuIndex) === parentIndex,
              )
            : undefined
          setOpenPath(parentPath)
          trigger?.focus()
          return
        }
        case 'Enter':
        case ' ': {
          event.preventDefault()
          if (isDisabled(active)) return
          if (active.hasAttribute('data-has-submenu')) {
            const childPath = [...path, index]
            openSubmenu(path, index, active)
            window.requestAnimationFrame(() => focusFirstIn(childPath))
            return
          }
          active.click()
          return
        }
        default:
          return
      }
    },
    [focusFirstIn, onClose, openSubmenu],
  )

  return createPortal(
    <div
      ref={rootRef}
      className="kb-context-menu-root"
      onKeyDown={handleKeyDown}
      onContextMenu={(event) => event.preventDefault()}
    >
      <MenuPanel
        items={items}
        path={[]}
        anchor={{ x, y }}
        isRoot
        ariaLabel={ariaLabel}
        openPath={openPath}
        anchors={anchors}
        onOpenSubmenu={openSubmenu}
        onHover={collapseDeeperThan}
        onSelectItem={selectItem}
      />
    </div>,
    document.body,
  )
}
import type { ReactNode } from 'react'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'

/**
 * 知识库右键菜单（S1 后半 tasks 2.4；openspec radix-component-layer 4.2 / D10）。
 *
 * 调用方只给坐标与菜单项：`x` / `y` 是菜单左上角，`onClose` 由父级收起。
 * 弹层、视口翻转、方向键与子菜单交给 radix DropdownMenu——本组件用 1×1 隐形锚点
 * 把浮层钉在坐标上，用 `asChild` 复用这套定位，不自建 portal / 量宽高 / 键盘循环。
 *
 * 两个语义保持原样：
 * - 命中项不自动关闭（`onSelect` 里 `preventDefault`），删除的「第一次确认、3s 回退」靠父级
 *   维护菜单项文本，菜单要一直开着；
 * - 打开前编辑器里的选区不被 mousedown 清掉，关闭时焦点也不抢回锚点。
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

function menuIcon(item: KbMenuItem) {
  if (!item.icon) return null
  return (
    <span className="kb-context-menu-ico" aria-hidden="true">
      {item.icon}
    </span>
  )
}

function renderItems(items: KbMenuItem[]): ReactNode {
  return items.map((item, index) => {
    const key = `${index}:${item.label}`
    const hasSubmenu = Array.isArray(item.submenu) && item.submenu.length > 0

    if (hasSubmenu) {
      return (
        <DropdownMenuSub key={key}>
          <DropdownMenuSubTrigger
            className="kb-context-menu-item"
            disabled={item.disabled}
            title={item.hint}
          >
            {menuIcon(item)}
            <span className="kb-context-menu-label">{item.label}</span>
          </DropdownMenuSubTrigger>
          <DropdownMenuSubContent className="kb-context-menu kb-context-menu-sub">
            {renderItems(item.submenu ?? [])}
          </DropdownMenuSubContent>
        </DropdownMenuSub>
      )
    }

    return (
      <DropdownMenuItem
        key={key}
        className="kb-context-menu-item"
        variant={item.danger ? 'danger' : 'default'}
        disabled={item.disabled}
        title={item.hint}
        onSelect={(event) => {
          // 菜单保持打开，关不关由父级按动作决定（删除二次确认需要它开着）
          event.preventDefault()
          item.onSelect?.()
        }}
      >
        {menuIcon(item)}
        <span className="kb-context-menu-label">{item.label}</span>
      </DropdownMenuItem>
    )
  })
}

export function KbContextMenu({ x, y, items, onClose, ariaLabel = '知识库菜单' }: KbContextMenuProps) {
  return (
    <DropdownMenu
      open
      modal={false}
      onOpenChange={(open) => {
        if (!open) onClose()
      }}
    >
      <DropdownMenuTrigger asChild>
        <span
          aria-hidden
          data-kb-menu-anchor
          tabIndex={-1}
          style={{ position: 'fixed', left: x, top: y, width: 1, height: 1, pointerEvents: 'none' }}
        />
      </DropdownMenuTrigger>
      <DropdownMenuContent
        className="kb-context-menu"
        aria-label={ariaLabel}
        align="start"
        side="bottom"
        sideOffset={-1}
        // 关闭时不要抢回锚点（锚点是隐形 span），页面焦点留在原处
        onCloseAutoFocus={(event) => event.preventDefault()}
      >
        {renderItems(items)}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
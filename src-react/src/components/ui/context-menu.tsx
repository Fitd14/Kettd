import * as React from "react"
import { ContextMenu as RadixContextMenu } from "radix-ui"

import { cn } from "@/lib/utils"

const ContextMenu = RadixContextMenu.Root
const ContextMenuTrigger = RadixContextMenu.Trigger
const ContextMenuGroup = RadixContextMenu.Group
const ContextMenuLabel = RadixContextMenu.Label
const ContextMenuSub = RadixContextMenu.Sub
const ContextMenuRadioGroup = RadixContextMenu.RadioGroup

export type ContextMenuItemProps = React.ComponentProps<typeof RadixContextMenu.Item> & {
  variant?: "default" | "danger"
}

function ContextMenuContent({
  className,
  ...props
}: React.ComponentProps<typeof RadixContextMenu.Content>) {
  return (
    <RadixContextMenu.Portal>
      <RadixContextMenu.Content
        data-slot="context-menu-content"
        className={cn("dropdown-menu", className)}
        {...props}
      />
    </RadixContextMenu.Portal>
  )
}

function ContextMenuItem({ className, variant = "default", ...props }: ContextMenuItemProps) {
  return (
    <RadixContextMenu.Item
      data-slot="context-menu-item"
      data-variant={variant}
      className={cn("dropdown-menu-item", className)}
      {...props}
    />
  )
}

function ContextMenuSubTrigger({
  className,
  ...props
}: React.ComponentProps<typeof RadixContextMenu.SubTrigger>) {
  return (
    <RadixContextMenu.SubTrigger
      data-slot="context-menu-sub-trigger"
      className={cn("dropdown-menu-item dropdown-menu-sub-trigger", className)}
      {...props}
    />
  )
}

function ContextMenuSubContent({
  className,
  ...props
}: React.ComponentProps<typeof RadixContextMenu.SubContent>) {
  return (
    <RadixContextMenu.Portal>
      <RadixContextMenu.SubContent
        data-slot="context-menu-sub-content"
        className={cn("dropdown-menu dropdown-menu-sub", className)}
        {...props}
      />
    </RadixContextMenu.Portal>
  )
}

function ContextMenuSeparator({
  className,
  ...props
}: React.ComponentProps<typeof RadixContextMenu.Separator>) {
  return (
    <RadixContextMenu.Separator
      data-slot="context-menu-separator"
      className={cn("dropdown-menu-separator", className)}
      {...props}
    />
  )
}

/**
 * 右键菜单唯一出口（openspec/radix-component-layer 1.6）。
 *
 * 与 `dropdown-menu` 共用 `.dropdown-menu*` 皮肤；radix 负责点击坐标定位、视口翻转、
 * 子菜单与全套键盘路径。调用方用 `ContextMenu.Trigger` 包住右键区域。
 */
export {
  ContextMenu,
  ContextMenuTrigger,
  ContextMenuGroup,
  ContextMenuLabel,
  ContextMenuSub,
  ContextMenuRadioGroup,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSubTrigger,
  ContextMenuSubContent,
  ContextMenuSeparator,
}
import * as React from "react"
import { DropdownMenu as RadixDropdownMenu } from "radix-ui"

import { cn } from "@/lib/utils"

const DropdownMenu = RadixDropdownMenu.Root
const DropdownMenuTrigger = RadixDropdownMenu.Trigger
const DropdownMenuGroup = RadixDropdownMenu.Group
const DropdownMenuLabel = RadixDropdownMenu.Label
const DropdownMenuSub = RadixDropdownMenu.Sub

export type DropdownMenuItemProps = React.ComponentProps<typeof RadixDropdownMenu.Item> & {
  variant?: "default" | "danger"
}

function DropdownMenuContent({
  className,
  sideOffset = 4,
  ...props
}: React.ComponentProps<typeof RadixDropdownMenu.Content>) {
  return (
    <RadixDropdownMenu.Portal>
      <RadixDropdownMenu.Content
        data-slot="dropdown-menu-content"
        sideOffset={sideOffset}
        className={cn("dropdown-menu", className)}
        {...props}
      />
    </RadixDropdownMenu.Portal>
  )
}

function DropdownMenuItem({ className, variant = "default", ...props }: DropdownMenuItemProps) {
  return (
    <RadixDropdownMenu.Item
      data-slot="dropdown-menu-item"
      data-variant={variant}
      className={cn("dropdown-menu-item", className)}
      {...props}
    />
  )
}

function DropdownMenuSeparator({
  className,
  ...props
}: React.ComponentProps<typeof RadixDropdownMenu.Separator>) {
  return (
    <RadixDropdownMenu.Separator
      data-slot="dropdown-menu-separator"
      className={cn("dropdown-menu-separator", className)}
      {...props}
    />
  )
}

function DropdownMenuSubTrigger({
  className,
  ...props
}: React.ComponentProps<typeof RadixDropdownMenu.SubTrigger>) {
  return (
    <RadixDropdownMenu.SubTrigger
      data-slot="dropdown-menu-sub-trigger"
      className={cn("dropdown-menu-item dropdown-menu-sub-trigger", className)}
      {...props}
    />
  )
}

function DropdownMenuSubContent({
  className,
  ...props
}: React.ComponentProps<typeof RadixDropdownMenu.SubContent>) {
  return (
    <RadixDropdownMenu.Portal>
      <RadixDropdownMenu.SubContent
        data-slot="dropdown-menu-sub-content"
        className={cn("dropdown-menu dropdown-menu-sub", className)}
        {...props}
      />
    </RadixDropdownMenu.Portal>
  )
}

/**
 * 下拉菜单唯一出口（openspec/radix-component-layer 1.6）。
 *
 * radix 负责 portal、定位翻转、↑↓ / Home / End / Enter / Esc 与 typeahead；
 * 皮肤走 `app.css` 的 `.dropdown-menu*`，触发器用 `asChild` 复用现有按钮原语。
 */
export {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuGroup,
  DropdownMenuLabel,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubTrigger,
  DropdownMenuSubContent,
}
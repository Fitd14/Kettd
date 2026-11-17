import * as React from "react"
import { Popover as RadixPopover } from "radix-ui"

import { cn } from "@/lib/utils"

const Popover = RadixPopover.Root
const PopoverTrigger = RadixPopover.Trigger
const PopoverAnchor = RadixPopover.Anchor
const PopoverClose = RadixPopover.Close

export type PopoverContentProps = React.ComponentProps<typeof RadixPopover.Content> & {
  /** 不叠加 `.popover` 基础皮肤；自带皮肤的浮层（如链接编辑条）用 */
  bare?: boolean
}

function PopoverContent({
  className,
  bare = false,
  align = "end",
  side = "bottom",
  sideOffset = 6,
  ...props
}: PopoverContentProps) {
  return (
    <RadixPopover.Portal>
      <RadixPopover.Content
        data-slot="popover-content"
        align={align}
        side={side}
        sideOffset={sideOffset}
        className={cn(!bare && "popover", className)}
        {...props}
      />
    </RadixPopover.Portal>
  )
}

/**
 * 轻量浮层唯一出口（openspec/radix-component-layer 1.7）。
 *
 * radix 负责 portal、定位翻转、点外关闭、Esc 关闭与焦点归还；
 * 内容默认 `role="dialog"`，无标题的浮层用 `aria-label` 给可访问名。
 * 皮肤走 `app.css` 的 `.popover`，特殊外观由调用方 `className` 覆盖。
 */
export { Popover, PopoverTrigger, PopoverAnchor, PopoverClose, PopoverContent }
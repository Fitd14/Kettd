import * as React from "react"
import { Dialog as RadixDialog } from "radix-ui"

import { cn } from "@/lib/utils"

const Dialog = RadixDialog.Root
const DialogTrigger = RadixDialog.Trigger
const DialogClose = RadixDialog.Close
const DialogTitle = RadixDialog.Title
const DialogDescription = RadixDialog.Description

export type DialogContentProps = React.ComponentProps<typeof RadixDialog.Content> & {
  /** 遮罩额外类名；面板默认由 `.dialog-overlay` / `.dialog-content` 承载皮肤 */
  overlayClassName?: string
}

/**
 * 模态弹窗唯一出口（openspec/radix-component-layer 1.5）。
 *
 * radix 负责 portal、遮罩、Esc、焦点圈定与焦点归还；皮肤走 `app.css` 的
 * `.dialog-overlay` / `.dialog-content`，调用方只补宽度等差异类。
 */
function DialogContent({ className, overlayClassName, children, ...props }: DialogContentProps) {
  return (
    <RadixDialog.Portal>
      <RadixDialog.Overlay
        data-slot="dialog-overlay"
        className={cn("dialog-overlay", overlayClassName)}
      />
      <RadixDialog.Content
        data-slot="dialog-content"
        className={cn("dialog-content", className)}
        {...props}
      >
        {children}
      </RadixDialog.Content>
    </RadixDialog.Portal>
  )
}

export { Dialog, DialogTrigger, DialogClose, DialogTitle, DialogDescription, DialogContent }
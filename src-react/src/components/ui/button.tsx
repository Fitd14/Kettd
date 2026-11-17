import * as React from "react"
import { Slot } from "radix-ui"

import { cn } from "@/lib/utils"

/** 变体：default / primary / ghost / soft / danger / bare，外加标题栏关闭键 window-close */
export type ButtonVariant =
  | "default"
  | "primary"
  | "ghost"
  | "soft"
  | "danger"
  | "bare"
  | "window-close"

/** 尺寸阶梯：xs 22 · sm 26 · md 30；图标钮同档；window / caret 是两个特例 */
export type ButtonSize =
  | "xs"
  | "sm"
  | "md"
  | "icon-xs"
  | "icon-sm"
  | "icon-md"
  | "window"
  | "caret"

export type ButtonProps = React.ComponentProps<"button"> & {
  variant?: ButtonVariant
  size?: ButtonSize
  asChild?: boolean
}

/**
 * 按钮唯一出口（openspec/radix-component-layer D9）。
 *
 * 外观全部由 `src-react/src/styles/primitives.css` 的 `.btn[data-variant]` / `.btn[data-size]`
 * 驱动，组件本身不拼 Tailwind 皮肤类：页面只给语义（变体 + 尺寸），
 * 像素、颜色、hover 与按下态集中在样式层，保证存量迁移后视觉零漂移。
 */
function Button({
  className,
  variant = "default",
  size = "sm",
  asChild = false,
  type,
  ...props
}: ButtonProps) {
  const Comp = asChild ? Slot.Root : "button"

  return (
    <Comp
      data-slot="button"
      data-variant={variant}
      data-size={size}
      className={cn("btn", className)}
      {...(asChild ? null : { type: type ?? "button" })}
      {...props}
    />
  )
}

export { Button }
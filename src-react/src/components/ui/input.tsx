import * as React from "react"

import { cn } from "@/lib/utils"

/** 尺寸沿用 `.input` 的既有三档：default 14px / sm 14px 宽内距 / xs 12.5px */
export type InputSize = "default" | "sm" | "xs"

export type InputProps = Omit<React.ComponentProps<"input">, "size"> & {
  size?: InputSize
  invalid?: boolean
}

/**
 * 输入框唯一出口（openspec/radix-component-layer 1.2）。
 *
 * 与 `Button` 同一套思路：只给语义（尺寸、是否无效），皮肤全部来自
 * `app.css` 的 `.input[data-size]` / `.input[aria-invalid]`，页面不再拼 `input xs` 这类字符串。
 */
function Input({ className, size = "default", invalid = false, ...props }: InputProps) {
  return (
    <input
      data-slot="input"
      data-size={size}
      aria-invalid={invalid || props["aria-invalid"] || undefined}
      className={cn("input", className)}
      {...props}
    />
  )
}

export { Input }
import * as React from "react"
import { Checkbox as RadixCheckbox } from "radix-ui"
import { Check } from "lucide-react"

import { cn } from "@/lib/utils"

export type CheckboxProps = React.ComponentProps<typeof RadixCheckbox.Root>

/**
 * 复选框唯一出口（openspec/radix-component-layer 1.4）。
 *
 * radix 负责 `aria-checked`、键盘（空格）与三态；外观走 `app.css` 的 `.checkbox`，
 * 勾选图标用 lucide 的 `Check`，尺寸与原生 `.tcheck` 同为 15×15。
 */
function Checkbox({ className, ...props }: CheckboxProps) {
  return (
    <RadixCheckbox.Root data-slot="checkbox" className={cn("checkbox", className)} {...props}>
      <RadixCheckbox.Indicator className="checkbox-indicator">
        <Check aria-hidden="true" />
      </RadixCheckbox.Indicator>
    </RadixCheckbox.Root>
  )
}

export { Checkbox }
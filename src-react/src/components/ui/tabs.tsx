import * as React from "react"
import { Tabs as RadixTabs } from "radix-ui"

import { cn } from "@/lib/utils"

const Tabs = RadixTabs.Root

function TabsList({ className, ...props }: React.ComponentProps<typeof RadixTabs.List>) {
  return <RadixTabs.List data-slot="tabs-list" className={cn("tabs", className)} {...props} />
}

function TabsTrigger({ className, ...props }: React.ComponentProps<typeof RadixTabs.Trigger>) {
  return <RadixTabs.Trigger data-slot="tabs-trigger" className={cn("tab", className)} {...props} />
}

function TabsContent({ className, ...props }: React.ComponentProps<typeof RadixTabs.Content>) {
  return <RadixTabs.Content data-slot="tabs-content" className={cn("tabs-content", className)} {...props} />
}

/**
 * 标签页唯一出口（openspec/radix-component-layer 1.8）。
 *
 * radix 负责 `tablist` / `tab` / `tabpanel` 语义、`←` / `→` / `Home` / `End` 与焦点跟随，
 * 未激活面板默认卸载（不留在 tab 序列里）；皮肤沿用 `.tabs` / `.tab`，
 * 激活态用 `data-state="active"` 表达，旧的 `.tab.on` 保留到批次 4 收口。
 */
export { Tabs, TabsList, TabsTrigger, TabsContent }
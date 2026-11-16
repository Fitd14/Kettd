# 20261116-1316-313c-原生控件迁移到-components-ui-封装: 原生控件迁移到 components/ui 封装

- **Tags**: 前端, radix, 组件封装, 迁移
- **Status**: doing · 2026-11-16 13:18:10 · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd
- **Created**: 2026-11-16 13:16:59 · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd

<!-- Header lines are maintained by the script, fixed order: Tags → Status → Created → Sealed.
     The `- **Tags**: …` line is inserted at the top by new --tags; omitted when there are no tags
     (category goes to the ledger and history only, never into this document).
     The `- **Sealed**: …` line is appended on close. -->

## Background

- 用户原话（2026-11-16）：组件优先基于 radix-ui 封装，不使用原生组件；前端页面优先采用内部已有封装；组件放 `src-react/src/components` 下，按表现形式可再分层。
- 规则已落地：[AGENTS.md §2.2](../../../AGENTS.md) 新增四条（组件单一入口 / radix 承载行为 / 组件分层 / 存量冻结），[README.md §目录结构](../../../README.md) 同步 components 那一行。
- 现状盘点：`components/ui/` 只有 `button.tsx`（radix `Slot` + cva，**全仓零引用**）与 `step-slider.tsx`（手写 role=slider）；页面侧 161 个原生 `<button>`、28 个 `<input>`、0 个 `<select>` / `<textarea>`，分布在 30 个 `.tsx`。
- 手写交互件：4 处 `role="dialog"`、2 处 `role="menu"`、2 处 `role="tab"`、2 处 `role="listbox"`、1 个手写 popover。

## To Do

按依赖排序，每批可独立交付：

1. **补原语**：`ui/input.tsx`、`ui/textarea.tsx`、`ui/dialog.tsx`、`ui/menu.tsx`（DropdownMenu + ContextMenu）、`ui/popover.tsx`、`ui/tabs.tsx`、`ui/checkbox.tsx`、`ui/switch.tsx`、`ui/tooltip.tsx`；评估 `ui/step-slider.tsx` 是否改用 radix `Slider`（现手写，`SettingsView` 阅读偏好在用）。
2. **Button 替换（161 处）**：先 `views/` 后业务组件，单文件提交；起点 `SettingsView.tsx`(36)、`tiptap-editor.tsx`(17)、`kb-tree.tsx`(12)、`PlannedView.tsx`(9)、`kb-item-detail.tsx`(9)。
3. **Input / Textarea 替换（28 处）**：`SettingsView.tsx`(8)、`task-row.tsx`(3)、`PlannedView.tsx`(3) 优先。
4. **弹层 / 菜单 / 浮层换 radix**：Dialog — `task-detail.tsx:50`、`kb-folder-dialog.tsx:114`、`SettingsView.tsx:165`、`link-inline-editor.tsx:84`；Menu — `task-row.tsx:156`、`kb-context-menu.tsx:157`；Popover — `typo-popover.tsx:76`。
5. **Tabs**：`ReviewView.tsx:121`、`PlannedView.tsx:122` / `:130`。

例外登记（规则里的「存量冻结」不覆盖，但迁移时单独处理）：

- Tiptap 工具条 `tiptap-editor.tsx:1311` 起，按钮依赖 `onMouseDown preventDefault` 保编辑器焦点；换 `ui/button` 时必须保留该事件，否则点一次失焦。
- `listbox` 两处（`KbView.tsx:1308`、`kb-tree.tsx:646`）：radix 无 Listbox 原语，选型（combobox / cmdk）定下来前不动。

## Next

先做第 1 批的第一件：新建 `src-react/src/components/ui/input.tsx`，照 `components/ui/button.tsx` 的 cva + `cn()` 写法（token 走 `--fs-*` / `hsl(var(--xxx))`），再补 `ui/dialog.tsx`（radix Dialog + 现有 `.dialog` 皮肤）；完成后跑 `npx tsc -b` 与 `node test/docs.test.mjs`。

## Process

- radix 已在依赖里，且是**统一包** `radix-ui@1.8.1`：`components/editor/heading-select.tsx:3` `import { Select } from 'radix-ui'`、`components/ui/button.tsx:3` `import { Slot } from 'radix-ui'`。AGENTS §2.7 禁的是 `@radix-ui/react-slot` 分包，两者不冲突 —— 规则因此写成「只用统一包」。
- radix 没有 Button / Input / Textarea 原语，`ui/` 封装内部仍渲染原生元素；「不使用原生组件」只能在页面层兑现，规则据此写成「原生元素只允许出现在 `ui/` 封装内部一次」。
- 存量计数用正则扫 `src-react/src/**/*.tsx` 的 `<button` / `<input` / `<select` / `<textarea`，共 30 个文件命中。
- 文档门禁：`node --test test/docs.test.mjs` 在本机沙箱下 spawn EPERM（Node test runner 需要管道），改用 `node test/docs.test.mjs` 直接执行，4/4 通过。

## Conclusion

- 规则已入库：AGENTS.md §2.2 四条 + README §目录结构一行；文档门禁 4 项全过。
- 迁移代码一行未动，按 To Do 五批推进；两条例外（Tiptap 工具条焦点、listbox 选型）已登记。

## Execution Log

<!-- Maintained by the script: appended by new / claim / set / release / log / close. Do not edit.
     Backfill with: progress log --id 20261116-1316-313c-原生控件迁移到-components-ui-封装 --note "...". -->
- 2026-11-16 13:16:59 · new · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd · created
- 2026-11-16 13:17:31 · claim · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd · claimed → doing
- 2026-11-16 13:18:10 · log · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd · 写入分批迁移清单（五批 + 两条例外：Tiptap 工具条焦点、listbox 选型）

# Proposal

## Why

前端交互控件散在页面里各自手写：161 个原生 `<button>`、28 个 `<input>`、4 处 `role="dialog"`、2 处 `role="menu"`、2 处 `role="tab"`、1 个手写 popover。焦点陷阱、Esc 关闭、键盘导航、层级 portal 这些行为被复制了十几份，改一处样式或修一次失焦要翻十几个文件；`components/ui/button.tsx` 早就按 radix + cva 建好，全仓却零引用 —— 底座在，纪律缺。

radix-ui 统一包已在依赖里（`radix-ui@1.8.1`），现在立规则并收口，成本最低；再拖下去，新页面会继续长原生控件。

## What Changes

- **规则入库**：`AGENTS.md` §2.2 新增四条（组件单一入口 / radix 承载行为 / 组件分层 / 存量冻结）；`README.md` §目录结构 中 components 一行改为三层描述。
- **补齐 `components/ui/` 原语**：`input`、`checkbox`（5 处已用）、`dialog`、`dropdown-menu` + `context-menu`、`popover`、`tabs`；`button` 按 D9 原型重做——3 档尺寸（22 / 26 / 30）× 5 个变体（`default` / `primary` / `ghost` / `soft` / `danger`，按下态用 `aria-pressed`），保留 `window`（44×32 无圆角）与 `caret`（18×20）两个特例，皮肤落 `.btn[data-variant]` / `[data-size]` 属性选择器；`step-slider` 改用 radix `Slider`。`textarea` / `switch` / `tooltip` 当前零使用点，按需再建，不预埋。
- **按钮视觉基线**：`design-review/button-system.html` 为高保真原型（4 组专用按钮 before/after、12 组专用类归并口径、「现类名 → variant/size」映射表）；2026-11-17 评审通过，`ui/button` 与 `app.css` 照它实现。
- **迁移存量**：161 处 `<button>`、28 处 `<input>` 换 `ui/` 封装；4 处手写 dialog、2 处 menu、1 处 popover、2 处 tab 换 radix 原语。
- **删除死代码**：`components/kb/kb-item-row.tsx` 零引用，随本次清理删除。
- **不引入新依赖**：只用已有的统一包 `radix-ui`，禁止 `@radix-ui/react-*` 分包。

## Capabilities

### New Capabilities

- `ui-components`: 前端交互控件的唯一出口 —— 控件只经 `components/ui/` 原语渲染、弹层与键盘行为由 radix-ui 承载、组件按表现形式分层、存量原生控件分批收口。

### Modified Capabilities

- 无。`visual-language` 管的是字号 / 颜色 / 材料语法，本变更不改它的任何要求；新原语继续遵它。

## Impact

- **新增**：`src-react/src/components/ui/` 下 9 个原语文件。
- **设计件**：`design-review/button-system.html`（按钮体系原型，评审基线）。
- **删除**：`src-react/src/components/kb/kb-item-row.tsx`（零引用死代码）。
- **改动**：`views/SettingsView.tsx`(44)、`views/PlannedView.tsx`(12)、`views/ReviewView.tsx`、`views/KbView.tsx`；`components/kb/kb-tree.tsx`(12)、`components/kb/kb-item-detail.tsx`(11)、`components/task-row.tsx`、`components/task-detail.tsx`、`components/kb/kb-folder-dialog.tsx`、`components/kb/kb-context-menu.tsx`、`components/kb/typo-popover.tsx`；`rendering/tiptap-editor.tsx`(18)、`rendering/link-inline-editor.tsx`。
- **文档**：`AGENTS.md` §2.2、`README.md` §目录结构。
- **依赖**：无新增；`radix-ui@1.8.1` 已在 `src-react/package.json`。
- **台账**：与 ledger 任务 `20261116-1316-313c-原生控件迁移到-components-ui-封装` 的五批计划对齐。
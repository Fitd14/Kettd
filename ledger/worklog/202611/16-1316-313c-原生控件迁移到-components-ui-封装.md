# 20261116-1316-313c-原生控件迁移到-components-ui-封装: 原生控件迁移到 components/ui 封装

- **Tags**: 前端, radix, 组件封装, 迁移
- **Status**: doing · 2026-11-17 02:12:58 · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd
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

> 以下是 2026-11-16 立项时的原始分批（历史记录，不再改）；实际执行结果与差异见 Conclusion 与 `openspec/changes/radix-component-layer/tasks.md`。

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

- 5.4：`AGENTS.md` §2.2 三条（例外清单 / primitives.css 位置 / 存量已收口）与 `README.md` 目录树加 `styles/` 一行，**等人工批准后提交**。
- 5.3：L4 冒烟五条（快速记录 / 今天计数 / 提醒触发 / 便签三态 / 知识库搜索），需要能起前端的真机。
- 5.2 复跑：在有正常桌面会话的机器上跑一次官方 `npm run build`，确认不靠沙箱绕行也能过。

## Process

- radix 已在依赖里，且是**统一包** `radix-ui@1.8.1`：`components/editor/heading-select.tsx:3` `import { Select } from 'radix-ui'`、`components/ui/button.tsx:3` `import { Slot } from 'radix-ui'`。AGENTS §2.7 禁的是 `@radix-ui/react-slot` 分包，两者不冲突 —— 规则因此写成「只用统一包」。
- radix 没有 Button / Input / Textarea 原语，`ui/` 封装内部仍渲染原生元素；「不使用原生组件」只能在页面层兑现，规则据此写成「原生元素只允许出现在 `ui/` 封装内部一次」。
- 存量计数用正则扫 `src-react/src/**/*.tsx` 的 `<button` / `<input` / `<select` / `<textarea`，共 30 个文件命中。
- 文档门禁：`node --test test/docs.test.mjs` 在本机沙箱下 spawn EPERM（Node test runner 需要管道），改用 `node test/docs.test.mjs` 直接执行，4/4 通过。
- 2026-11-17：改动量超出「快速迁移」，按用户要求单开分支 `feat/radix-component-layer`，并在 `openspec/changes/radix-component-layer/` 立变更（proposal / 9 条规格 / design D1–D18 / 5 组任务），`openspec validate --strict` 通过。
- 视觉基线：先出 `design-review/button-system.html` 原型（三档尺寸 22/26/30 × 五变体 + 数据表面），用户评审四项全过后才动代码。
- 关键结构发现：`app.css` 只有主窗加载，便签/待办/捕获三窗拿不到 `.btn` / `.checkbox` 底座 → 抽出 `styles/primitives.css` 由 `index.css` 引入（D15），三窗按钮与复选框此后才有原语皮肤。
- 映射缺口逐条补口径而非就地发明尺寸：D12（整行可点击行容器留原生）、D13（编辑器内皮肤输入走 `Input bare`）、D14（窄窗大纲抽屉留原生）、D16（深色气泡 `data-surface="bubble"`）、D17（chip 内嵌钮 `variant="bare"`）、D18（专用盒子钮：ghost + 不传 size + 域内选择器加重一层）。

## Conclusion

截至 2026-11-17，五批中四批已交付（分支 `feat/radix-component-layer`，14 个提交），按批对应：

| 批次 | 状态 | 结果 |
| --- | --- | --- |
| ① 补原语 | 完成 | 新建 `ui/input` `checkbox` `dialog` `dropdown-menu` `context-menu` `popover` `tabs`；重写 `ui/button`（三档 × 五变体 + bare）与 `ui/step-slider`（radix Slider）。按「零使用点不预埋」跳过 textarea / switch / tooltip |
| ② Button 161 → 0 | 完成 | 原生 `<button>` 161 → **22**，余量全是 D8 listbox 行（1）与 D12 整行/图元容器（21），不再迁 |
| ③ Input 28 → 0 | 完成 | 原生 `<input>` 28 → **4**：`ui/input.tsx` 自身 1 + 白名单 3（两个隐藏 file、一个 range） |
| ④ 弹层/菜单/浮层 | 完成 | 三处 dialog 换 radix（`task-detail`、`ImportPreviewDialog`、`kb-folder-dialog`）；右键菜单 429 → 152 行；`typo-popover` 与链接编辑条换 radix Popover；三处 tabs 换 radix Tabs |
| ⑤ 收口 | 完成 | 审计表落 `tasks.md` 5.1；零引用旧 CSS 四组删除；`tsc` 0、`oxlint` 31 warnings / 0 errors、`vite build` 通过（沙箱内需绕过 vite 的 `net use` 探测）、8 个测试文件 44 tests / 0 fail |

2026-11-17 收尾：用户在真机跑完 L4 五条并确认「没问题」，`AGENTS.md` §2.2（三类例外 / 原语皮肤位置 / 存量已收口）与 `README.md` 目录树经批准后提交（`176ac75`），`tasks.md` 全部复选框勾选，`openspec validate --strict` valid。

遗留：`npm run build` / `npm run check` 官方脚本仍待在有桌面会话的机器复跑（沙箱内 `node --test` 需管道 spawn、vite 会调 `net use`，均已用等价手段绕过，不是代码问题）。

## Execution Log

<!-- Maintained by the script: appended by new / claim / set / release / log / close. Do not edit.
     Backfill with: progress log --id 20261116-1316-313c-原生控件迁移到-components-ui-封装 --note "...". -->
- 2026-11-16 13:16:59 · new · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd · created
- 2026-11-16 13:17:31 · claim · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd · claimed → doing
- 2026-11-16 13:18:10 · log · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd · 写入分批迁移清单（五批 + 两条例外：Tiptap 工具条焦点、listbox 选型）
- 2026-11-17 01:48:21 · log · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd · 回写五批执行结论：②按钮 161→22、③输入 28→4、④弹层菜单标签页全部换 radix；遗留真机冒烟与文档待批
- 2026-11-17 02:12:58 · log · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd · 真机五条通过、文档批准并提交 176ac75；openspec 变更已归档，9 条规格同步进主规格

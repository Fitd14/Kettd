# Tasks

## 0. 视觉基线（评审门）

- [x] 0.1 评审按钮体系原型 `design-review/button-system.html`（D9）：3 档尺寸 22/26/30、6 变体、12 组专用类归并口径、`size="window"` 与 `size="caret"` 两个特例。2026-11-17 用户确认四项全过（尺寸上移 / primary 实心 / 便签换色 / 窗口保持 44×32），结论已回填 D9
- [x] 0.2 删除零引用死代码 `src-react/src/components/kb/kb-item-row.tsx`（`grep -r KbItemRow` 无 import；D8 原本留到后续，用户 2026-11-17 改为本轮删除），验证 `npx tsc -b` 通过
- [x] 0.3 把原型纳入版本库：只 `git add design-review/button-system.html`（同目录的 `capture.ps1` 等用户件不入库），已随 `4942230` 入库
- [x] 0.4 原语皮肤独立成 `src-react/src/styles/primitives.css`（D15）：`app.css` 迁出 `.btn` / `:focus-visible` / `.input` / `.dialog-*` / `.checkbox` / `.tabs` / `.dropdown-menu*` / `.popover`，由 `index.css` 引入，保证便签/悬浮/捕获三窗拿到原语底座；验证 `node scripts/css-parse.mjs` 13 张 0 失败、`tsc -b` exit 0

## 1. 原语与首个落点

每一项都是「建原语 + 立刻在真实页面用一次」，避免建出没人用的死代码。

- [x] 1.1 改造 `src-react/src/components/ui/button.tsx`：按 D9 原型实现 `variant`（default / primary / ghost / soft / danger / window-close）× `size`（xs / sm / md，另含 `icon-xs` / `icon-sm` / `icon-md` / `window` / `caret`），并在 `src-react/src/styles/app.css` 的 `.btn` 段后补 `.btn[data-variant="…"]` / `.btn[data-size="…"]` 选择器（含 `window`、`caret` 特例与 `[data-surface="sticky"]` 换色覆写）；保留 `asChild` 与 `data-slot`；在 `views/SettingsView.tsx` 用它替换 3 个按钮（`sm` 默认、`ghost xs`、`danger xs`）做试点。**已落地**：`npx tsc -b` 退出 0、oxlint 0 warning、`node scripts/css-parse.mjs` 通过；hover / 禁用态与原型的一致性未真机核对（沙箱无法截图），并入 5.3 冒烟一起看
- [x] 1.2 新建 `components/ui/input.tsx`（复用 `.input`，尺寸走 `.input[data-size]`、无效态走 `.input[aria-invalid="true"]`；`size` 与原生数字 `size` 冲突，已 `Omit` 掉原生声明），在 `views/SettingsView.tsx` 替换快速记录热键输入框一处试点。**已落地**：`npx tsc -b` 退出 0、oxlint 0 warning、CSS 解析通过；焦点环与边框色仍由 `.input:focus` 提供，文件内无裸 hex
- [ ] 1.3 `textarea` 零使用点（全仓 `<textarea` 计数 0），本轮不建原语；页面若出现，按 1.2 同一套 `.input` 规则补建并在此行注明
- [x] 1.4 新建 `components/ui/checkbox.tsx`（radix `Checkbox` + `Indicator`，lucide `Check`；15×15、勾选态 primary 实心，便签里换 `--sticky-accent`），**5 处原生 `type="checkbox"` 全部替换**：`views/PlannedView.tsx:144`、`components/task-row.tsx:99`/`:108`、`components/kb/kb-attach-picker.tsx:74`、`todo/TodoFloatWindow.tsx:156`。连带改动：`kb-attach-picker` 的 `<label>` 改 `<div onClick>`（radix 按钮在 label 里不会随文字点击切换，改由行点击兜底、按钮 `stopPropagation` 防双触发）；`sticky.css` 三处 `:has(.tcheck:checked)` 补 `.checkbox[data-state="checked"]` 分支，`app.css` 补 `.trow-done .checkbox` 的置灰态。**已落地**：`npx tsc -b` 退出 0、oxlint 0 error（仅 TodoFloatWindow 既有 effect 告警）、CSS 解析通过、`grep type="checkbox"` 计数 0
- [x] 1.5 新建 `components/ui/dialog.tsx`（radix `Dialog` + `Portal`，皮肤沿用现有遮罩与面板类），在 `components/kb/kb-folder-dialog.tsx:114` 替换，验证 `Esc` 与点遮罩都能关闭、关闭后焦点回到触发按钮、打开时背景不可 tab 进入。**已落地**：`ui/dialog.tsx` 导出 `Dialog` / `DialogTrigger` / `DialogClose` / `DialogTitle` / `DialogDescription` / `DialogContent`（`overlayClassName` 可补差异类；默认 `.dialog-overlay` + `.dialog-content` 皮肤写进 `app.css`，含 120/140ms 入场动画与 `prefers-reduced-motion` 关闭）；`kb-folder-dialog` 删掉自建 portal、Esc 捕获监听、Tab 首尾循环，改用 `onOpenAutoFocus` 聚焦全选、`onCloseAutoFocus` 还焦点、`onPointerDownOutside` 在提交中拦截；三条验证项待真机执行（Esc / 点遮罩 / 焦点归还）
- [x] 1.6 新建 `components/ui/dropdown-menu.tsx` 与 `ui/context-menu.tsx`，在 `components/task-row.tsx:152` 的「⋯」菜单替换，验证 `↑` / `↓` / `Home` / `End` / `Enter` / `Esc` 六条键盘路径与焦点归还。**已落地**：`dropdown-menu` 导出 Root / Trigger / Group / Label / Content / Item / Separator（Item 支持 `variant="danger"`，Content 走 portal + `sideOffset`），`context-menu` 导出 Trigger / Content / Item / Sub / SubTrigger / SubContent / Separator，共用 `app.css` 新增的 `.dropdown-menu*` 皮肤（`data-highlighted` 高亮、`data-variant="danger"` 红字）；`task-row` 删掉手写 `menu-pop` 与 `aria-expanded` 维护，改由 radix Trigger 承担。`components/kb/kb-context-menu.tsx` 按 D10 保留自建，迁移并入批次 4
- [x] 1.7 新建 `components/ui/popover.tsx`，在 `components/kb/typo-popover.tsx:76` 替换，验证点击外部与 `Esc` 关闭、点击面板内部不关闭、再点触发按钮可重新打开。**已落地（D11）**：`Popover` / `PopoverTrigger` 上移到 `components/kb/kb-item-detail.tsx`，`TypoPopover` 退化为纯内容（只收 `value` / `onChange`），点外、`Esc`、焦点归还与触发器开合全交 radix；portal 后在 `.kb-typo-pop` 补一套同名令牌映射（`--paper` / `--line-warm` / `--ink-*` / `--seal-*`），内部样式不改
- [x] 1.8 新建 `components/ui/tabs.tsx`，在 `views/PlannedView.tsx:122` / `:130` 替换，验证 `←` / `→` 切换且焦点跟随、未选中面板不在 tab 序列里。**已落地**：`TabsList` 沿用 `.tabs`、`TabsTrigger` 沿用 `.tab` 并用 `data-state="active"` 表达激活态（`.tab.on` 保留到 4.3 收口），两个面板用 `TabsContent asChild` 直接渲染原 `<section>`，DOM 层距由 `.tabs-root` 的 `flex column + var(--sp-4)` 接住；`ReviewView.tsx:121` 留给 4.3
- [x] 1.9 把 `components/ui/step-slider.tsx` 改为基于 radix `Slider`（`Root` / `Track` / `Range` / `Thumb`），保留轨道与刻度外观、`pointerdown` 继续 `stopPropagation`；验证 `typo-popover` 拖滑块时浮层不关闭、`←` / `→` 移一档、`Home` / `End` 到端点、`aria-valuetext` 随档位更新。**已落地，含一处口径修正**：`stopPropagation` 不再需要 —— 浮层已换成 radix `Popover`，它的点外判定看的是 `event.target` 是否落在内容里（拖动起点就在内容内），而文档级捕获监听本来也拦不住；继续在 Root 上 `stopPropagation` 只会挡住上层无关逻辑，故删除。刻度仍自渲染（radix 的 `Range` 只画填充条），`Range` 的宽度交给 radix 的 `right` 百分比，Thumb 的 `left` 与 `translateX(-50%)` 也由 radix 计算，CSS 里删掉了 `margin-left` 补偿
- [ ] 1.10 `switch` / `tooltip` 目前零使用点，本轮不建；若迁移过程中出现需求，按 D1 皮肤规则补建并在此行注明

## 2. 按钮迁移（161 处）

每批以 `grep -c '<button'` 归零为完成标准；每改一个文件跑一次 `npx tsc -b`。选哪个 `variant` / `size` 一律照 D9 原型的映射表，遇到映射表没有的写法先补表再改代码，不在页面里就地发明尺寸。

- [ ] 2.1 `views/` 71 处全部换成 `ui/button`，验证该目录 `<button` 计数为 0，且 `SettingsView`、`PlannedView` 两个最重的页面在真机上按钮布局无位移
  - 进度（2026-11-17）：`PlannedView` 7 + `InboxView` 6 + `SettingsView` 31 + `StickyView` 8 + `TodayView` 3 + `ReviewView` 2 + `KbView` 1 已换；`views/` 原生 `<button` 余 6，全仓余 95
  - 余 5 处均卡在映射缺口，非漏改：`SettingsView` 2（font-chip 文字钮）、`KbView` 2（`kb-error-retry`、`kb-list-toggle` 单点专用钮）、`ReviewView` 1（热力图 `heat-cell` 单点专用钮）；周期 tab 已在 4.3 换成 radix Tabs。补齐 D9 口径后 2.1 才能归零
  - 收尾（2026-11-17，D17）：`SettingsView` 的 `.font-chip-name` / `.font-chip-del` 换 `variant="bare"`，该文件按钮缺口清零；`views/` 原生 `<button` 余 3（`KbView` 2 个单点专用钮 + `ReviewView` 1 个热力格，均属 D12 行/格容器或待补口径）
  - 再收尾（2026-11-17，D9 补专钮口径）：`KbView` 的 `.kb-error-retry` 换 `variant="ghost" size="xs"`（`height:auto`、保留 destructive 描边），`.kb-list-toggle` 换 `variant="ghost"` 不传 size（20×44 盒子仍由 `kb.css` 的 `.kb-detail .kb-list-toggle` 决定）；`views/` 原生 `<button` 余 1，仅 `ReviewView` 热力格（D12 明确保留）
- [ ] 2.2 `components/kb/` 32 处：按 D12 只迁控制钮，整行可点击的行容器保留原生 `<button>`；验证文件内控制钮计数为 0，知识库列表 hover 浮起与工具栏图标钮尺寸不变
  - 进度（2026-11-17）：`kb-attach-picker` 2 + `kb-empty-state` 1 + `kb-folder-dialog` 2 + `kb-item-detail` 7（4 个 `.btn*` + 3 个 `kb-strip-btn`）已换；`kb-tree` 5 处控制钮已换（`kb-folder-add` 2 → `icon-xs`，文件夹重命名/删除 2 → `icon-xs`，确认删除 1 → `danger xs`）；`doc-outline` 关闭钮 → `icon-xs`；`kb-pin-row` 归入 D12 行容器（整行点击，非图标钮）。目录原生 `<button` 33 → 15，余下为 D12 行容器 9（`kb-tree` 6 + caret 2 + `kb-outline-link` 1）、`kb-context-menu` 3、`kb-item-detail` 2（`kb-tag-x` / `kb-tag-add`，见 design 映射缺口）、`typo-popover` 1（`kb-typo-seg-btn`，见 design 映射缺口）
  - 再收尾（2026-11-17）：`kb-item-detail` 的 `.kb-tag-x` 换 `variant="bare"`、`.kb-tag-add` 换 `ghost xs`；`typo-popover` 的 `.kb-typo-seg-btn` 换 `variant="ghost" size="sm"` + `aria-pressed`（`.on` 类删除，选择器改 `.kb-typo-seg .kb-typo-seg-btn[aria-pressed="true"]`）；`kb-context-menu` 整体在 4.2 迁 radix。`components/kb/` 原生 `<button` 余 8，全部是 `kb-tree` 的 D12 行容器
- [ ] 2.3 `rendering/` 19 处，验证该目录计数为 0；`tiptap-editor.tsx:1311` 起的工具条按钮保留 `onMouseDown` + `preventDefault`，点按钮后编辑器保持焦点、选区不丢
  - 进度（2026-11-17）：编辑工具条 4 枚（加粗/表格/链接/分割线走 `renderTool` 与单点钮）+ 查找栏 3 枚换成 `size="icon-sm"` / `icon-xs` + `variant="ghost"`，`aria-pressed` 承接原 `.on`；`.kb-editor-tool` 皮肤整段删除（改由 `.btn` 统一）
  - 收尾（2026-11-17，D16）：选区浮动条 4 枚 + 表格浮动条 5 枚换 `<Button data-surface="bubble" size="icon-xs|xs" aria-pressed>`，容器同步加 `data-surface="bubble"`；链接编辑条 2 枚换 `size="sm"`（移除用 `variant="danger"`）。`.kb-bubble button` 6 条规则删除，`.kb-link-inline-btn` 只留 `flex: 0 0 auto`。`rendering/` 9 → 1（仅余 `grid-cell` 网格选择器，见 2.3 原定工具条特例）
  - 未验证：工具条高度 24 → 26px 后工具栏行高与 `HeadingSelect` 基线是否齐；`onMouseDown preventDefault` 保焦点行为未真机点过；气泡按钮 24px 高、26px 宽与 `.kb-bubble` 3px 内边距的实际贴合
- [ ] 2.4 `components/` 根 19 处，验证该目录计数为 0，任务行与快捷键提示的按钮样式不变
  - 进度（2026-11-17）：`task-row` 3、`completion-timeline` 1（「更早」）已换；`components/` 根余 4：`undo-toast.tsx:91`/`:94`（`.undo-btn` / `.undo-x` 自带 toast 皮肤）、`completion-timeline.tsx:57`/`:70`（`.ctl-dot` 时间轴圆点），均卡 D9 映射缺口
  - 再收尾（2026-11-17）：`undo-toast` 撤销钮换 `variant="ghost" size="xs"`（`.undo-btn` 的 destructive 描边留在 `app.css`）、关闭 ✕ 换 `variant="ghost" size="icon-xs"`；`components/` 根余 2，均为 `completion-timeline` 的时间轴圆点（D12 视为行/图元容器）
- [ ] 2.5 `sticky/` 8、`todo/` 6、`capture/` 4、`src` 根 2 处，验证这四个位置计数为 0，便签三态切换与快速捕获条仍可用
  - 进度：`sticky/StickyWindow.tsx` 8 → 2（`sticky-mini-text` 行文字钮、`sticky-md-btn on` 文字动作，按 D12/状态 chip 暂留）；`todo/TodoFloatWindow.tsx` 6 → 2（`sticky-btn pin` 状态 chip、`sticky-title` 行文字钮）；`capture/CaptureWindow.tsx` 4 → 1（`.chip` 整块可点按 D12 留原生；注释掉的旧按钮已删）；`src` 根 2 未动。转换时保留 `className="sticky-btn"`，外观仍由 `sticky.css` 决定，等批次 5 删旧皮肤
  - 前提修复：D15 把原语皮肤抽到 `primitives.css` 由 `index.css` 引入——这三个窗口入口此前不加载 `app.css`，`.btn` / `.checkbox` 在小窗里没有底座（`TodoFloatWindow` 的复选框曾因此裸奔）

## 3. 输入框迁移（28 处）

- [x] 3.1 `views/` 换成 `ui/input`，验证该目录 `<input` 计数为 0，设置页各输入框焦点环与禁用态不变
  - 进度（2026-11-17）：`SettingsView` 4 个热键框、`PlannedView` 2（提醒日期 + 提醒内容）、`InboxView` 批量排期日期、`TodayView` 快速记录、`StickyView` 便签搜索、`components/task-row.tsx` 排期日期已换；全仓 `<input>` 22 → 12（含 `ui/input.tsx` 自身 1 处）。views 余 `SettingsView` 3 处，均按「无外观控件」白名单不迁：隐藏 `type="file"` 的 `.font-file` / `.backup-file-input`，以及 `type="range"` 的 `.fade-slider`（归 `ui/slider` 批次）
- [x] 3.2 `components/kb/` 6 处，验证计数为 0，附件选择器的复选框与搜索框行为不变
  - 进度（2026-11-17）：`kb-attach-picker` / `kb-search-bar` / `kb-folder-dialog` / `kb-item-detail` 的 `.kb-tag-input` 已全部换；标签输入走 `bare`（跳过 `.input` 底座，皮肤仍由 `kb.css` 的 `.kb-tag-input` 决定）
- [x] 3.3 `components/` 根 4 处，验证计数为 0
  - 进度（2026-11-17）：`time-text.tsx`（时长输入，`input xs mono time-text` + `bad` 态）已换成 `size="xs"` + `invalid`，`aria-invalid` 由原语输出；`task-row` 排期日期已换
- [x] 3.4 `rendering/` 2、`todo/` 1、`capture/` 1 处，验证计数为 0，快速捕获回车入库仍可用
  - 进度（2026-11-17）：`capture/CaptureWindow.tsx` 快速记录框已换（`className="input invalid"` → `invalid={!!error}`，`capture.css` 的 `.cap-row .input.invalid` 同步改成 `[aria-invalid="true"]`）；`todo/` 无文本输入（只有已迁的复选框）。`rendering/link-inline-editor.tsx` 与 `rendering/tiptap-editor.tsx` 查找框 2 处按 D13 用 `<Input bare>` 迁移：`.kb-link-inline-input` / `.kb-find-input` 保留在 `prosemirror.css`，不再与 `app.css` 的 `.input` 竞争特异性
  - 终态：全仓 `<input` 7 → 4，剩 `ui/input.tsx` 自身 1 + `SettingsView` 白名单 3（两个隐藏 file、一个 range）

## 4. 弹层、菜单与标签页收尾

- [ ] 4.1 剩余手写 dialog 换成原语：`components/task-detail.tsx:50`、`views/SettingsView.tsx:167` 换 `ui/dialog`；`rendering/link-inline-editor.tsx:90` 是贴链接的浮条，换 `ui/popover`（`bare`，不叠 `.popover` 皮肤）；`components/kb/kb-item-detail.tsx:505` 按 D14 暂留。验证都能 `Esc` 关闭且焦点归还，链接内联编辑的选区不被浮层打开动作清掉
  - 进度（2026-11-17）：三处已换。`task-detail` 删掉自建遮罩/监听 Esc/首帧聚焦，改 `data-align="top"` 保持顶部 48px 对齐（`app.css` 补 `.dialog-content[data-align="top"]` 与淡入）；`SettingsView` 的 `ImportPreviewDialog` 删掉 `createPortal`、捕获阶段 Esc、Tab 首尾循环，`applying` 时用 `onEscapeKeyDown` / `onPointerDownOutside` 拦关闭；`link-inline-editor` 改 `Popover` + `PopoverAnchor`（锚在链接底边 1×1），手动视口夹取改由 radix 碰撞检测负责，`onOpenAutoFocus` / `onCloseAutoFocus` 都 `preventDefault` 以保选区，3 秒宽限走 `onPointerDownOutside`
  - 未验证：三处的焦点归还与 Esc 时序未在真机上按过（沙箱限制同前）；`DialogContent` 的 `aria-describedby={undefined}` 只在 `task-detail` 用（标题走 `DialogTitle asChild`）
- [ ] 4.2 迁移 `components/kb/kb-context-menu.tsx`（按 D10 的落地修正）：保留 `{x,y,items,onClose}` 契约，内部改 `DropdownMenu` + 1×1 隐形锚点，子菜单「移动到…」用 `Sub` / `SubTrigger` / `SubContent`，`onSelect` 里 `preventDefault` 让「第一次确认、3 秒回退」继续靠父级换文案；验证四类区域（文件夹 / 条目 / 空白 / 输入框）、子菜单与键盘路径
  - 进度（2026-11-17）：`kb-context-menu.tsx` 429 → 152 行，删掉自建 portal、量宽高 clamp、↑↓/←→/Enter/Tab 循环与层路径 anchor 表；`ui/dropdown-menu.tsx` 补 `DropdownMenuSub` / `SubTrigger` / `SubContent`；`kb.css` 把 `.kb-context-menu-item:hover/.is-*` 改成 `[data-highlighted]` / `[data-state="open"]`，删 `.kb-context-menu-root` / `-slot` / `-arrow`
  - 未验证：四类右键区域与子菜单悬停、点外关闭未真机按过；radix 子菜单悬停有 100ms 延迟（旧实现即时）
- [x] 4.3 `views/ReviewView.tsx:121` 换成 `ui/tabs`，验证切换后 KPI 区与列表同步刷新
  - 已落地：`<Tabs value={range} onValueChange={(v) => setRange(v as Range)}>` + `<TabsList aria-label="统计周期">`，三个 `<TabsTrigger value>` 沿用 `.tab`，激活态由 `data-state="active"` 表达（`app.css` 的 `.tab.on` 保留到 5.6）；周期切换仍通过 `setRange` 驱动 `weeks` 与热力图重算，无面板内容，故不用 `TabsContent`。`role="tab"` 在 `views/` 归零
- [ ] 4.4 listbox 按 D8 保持现状：`views/KbView.tsx:1308`、`components/kb/kb-tree.tsx:646` 的结构、`role` 与行按钮都不动，验证本变更的 diff 未触及这两处；零引用的 `components/kb/kb-item-row.tsx` 已按 0.2 删除

## 5. 收口

- [ ] 5.1 全仓检索 `<button` / `<input` / `<select` / `<textarea` 与 `role="dialog|menu|tab"`，除 `components/ui/` 内部外计数为 0；白名单四类：① D8 的 listbox 行按钮（`components/kb/kb-tree.tsx:650` 起）② D12 的整行可点击行容器（`kb-tree-item`、`kb-folder-row`、`kb-folder-caret`、`kb-item-row`、`kb-tag-chip`、`kb-pin-row`、`heat-cell`；`kb-context-menu-item` 已随 4.2 换成 radix Item）③ 无外观控件（隐藏的 `type="file"` 输入：`SettingsView.tsx` 的 `.font-file` / `.backup-file-input`）④ D13 的编辑器内皮肤输入（`.kb-link-inline-input`、`.kb-find-input`、`.kb-tag-input`）⑤ D12 判定前的临时单点钮。记录白名单行号与剩余数
  - 审计（2026-11-17）：全仓 `<button` 43、`<input` 4（原语自身 1 + 白名单 3）；`role="dialog"` 3（`kb-item-detail.tsx:511` 按 D14 暂留、`ui/popover.tsx:42` 注释、`StickyWindow.tsx:45` 选择器字符串）；`role="menu"` / `role="tab"` / `role="tablist"` 均为 0；`role="listbox"` 3（D8 两处 + `StickyWindow.tsx:45` 选择器字符串）
  - 43 处分类（无漏改）：D12 行/整块 17 = `App.tsx:166`/`:178`、`capture/CaptureWindow.tsx:203`（chip）、`components/kb/doc-outline.tsx:47`、`kb/kb-tag-filter.tsx:12`/`:19`、`kb/kb-tree.tsx:354`/`:409`/`:422`/`:581`/`:627`/`:640`/`:653`、`sticky/StickyWindow.tsx:333`、`todo/TodoFloatWindow.tsx:163`、`views/ReviewView.tsx:133`、`rendering/tiptap-editor.tsx:1076`；D8 listbox 行 1 = `kb/kb-tree.tsx:704`；D9 映射缺口 24 = `kb-item-detail.tsx:449`/`:474`、`kb/typo-popover.tsx:73`、`SettingsView.tsx:652`/`:660`、`KbView.tsx:1272`/`:1337`、`StickyWindow.tsx:375`、`TodoFloatWindow.tsx:126`、`undo-toast.tsx:91`/`:94`、`link-inline-editor.tsx:106`/`:114`、`tiptap-editor.tsx:1332`–`:1335`、`:1347`/`:1348`/`:1356`/`:1357`/`:1359`、`completion-timeline.tsx:57`/`:70`；另有 1 处注释掉的旧按钮已删（`CaptureWindow.tsx:190`）
  - 结论：批次 2 的「有 D9 写法可套」部分已全部迁完；43 处余量 100% 落在已登记的白名单或映射缺口内，缺口补齐前不再改动
- [ ] 5.2 跑 `npx tsc -b`、`npm run lint`、`npm run build`，三项全过并记录输出
- [ ] 5.3 按 AGENTS §4.2 做 L4 冒烟五条：快速记录、今天视图计数、提醒触发、便签三态、知识库搜索
- [ ] 5.4 校对 `AGENTS.md` §2.2 与 `README.md` §目录结构 的文案与最终实现一致（三层描述、例外清单），过 `node test/docs.test.mjs`
- [ ] 5.5 更新 ledger 任务 `20261116-1316-313c-原生控件迁移到-components-ui-封装` 的 worklog 结论，与本变更的五批一一对应
- [ ] 5.6 迁移完成后删除 `app.css` 与各域 CSS 里被 D9 取代的旧修饰：`.btn.ghost` / `.btn.danger` / `.btn.sm` / `.btn.xs`、`.sticky-btn`、`.kb-strip-btn`、`.kb-folder-add`、`.kb-folder-action`、`.kb-tag-x`、`.todo-mini-btn`、`.sticky-md-btn` 的尺寸与 hover 段（保留 `:where(button,[tabindex]):focus-visible` 兜底），`grep` 确认无引用后再删
  - 进度（第十七轮）：`.btn.ghost` / `.btn.danger` / `.btn.sm` / `.btn.xs` 与 `.input.xs` / `.input.sm` 已从 `primitives.css` 删除；`app.css` 的 `.seg .btn.on`、`.schedule-pop .input.xs`、`.btn.danger` 三处死覆盖同步删除；grep 确认 TSX 已无旧修饰写法（仅 `CaptureWindow.tsx:190` 一行注释残留）。余下 `.sticky-btn` / `.kb-strip-btn` / `.kb-folder-add` / `.kb-folder-action` / `.kb-tag-x` / `.todo-mini-btn` / `.sticky-md-btn` 仍作为布局/盒子类随 `<Button>` 保留，待这些落点收完后删
  - 进度（第十八轮）：TSX 零引用的四组规则整段删除——`app.css` 的 `.titlebar-btn` 3 条（窗口键改 `size="window"`，宽度/无边框/透明由 `.btn[data-size="window"]` 覆盖，hover 由 `.btn:hover` 兜底）、`kb.css` 的 `.kb-strip-btn` 5 条（改 `variant="ghost" size="icon-sm"`）、`.kb-folder-add` 2 条（改 `size="icon-xs"`）、`.kb-outline-close` 3 条（改 `size="icon-xs"`）；CSS 字符数 `app.css 19231→18818`、`kb.css 39533→38622`，13 张表全部解析通过

## Workflow follow-up

- 变更归档前跑 `openspec validate radix-component-layer --strict` 并处理告警。
- 归档后确认 `openspec/specs/ui-components/spec.md` 的 Purpose 已由本变更的 delta 写入。
# Proposal

## Why

便签、托盘与编辑器标题这层「外壳」积累了五处体验缺口：便签只能靠在设置页的纯文本清单管理，没有可视化的卡片入口；标题级别用的是原生 `<select>`，展开列表由操作系统绘制，无法跟随纸面主题；免打扰（DND）链路（模型 / 调度 / 设置 UI / 文档）完整但收益低，夜间静默反而让提醒不可预期；托盘菜单 10 个可点项里有 5 项与其它入口重复，噪音大于价值；开启「移出淡化」后，用中文输入法在便签内打字时，输入法候选窗是独立 HWND，WebView2 可能因此收到 `mouseleave`，导致指针还在便签里就开始淡化。

现在做这件事的时机：视觉语言刷新 change（`refine-frontend-visual-language`）已确立「描图纸 / 纸面」材料二分与字号、颜色、动效令牌，本 change 的五个点全部落在这套语言之上，且**不新增命令、不新增依赖**（标题下拉改用依赖里已有的 `radix-ui`，不装新包），可以在一轮里把外壳打磨到同一水准。

## What Changes

1. **便签卡片墙（D1）**：侧栏新增「便签」路由 `#/sticky`，页面为响应式卡片墙——每张卡展示正文摘要（纯文本前约 120 字）、更新时间、知识来源 / 贴桌面标记，提供打开 / 删除 / 新建操作；支持本地搜索过滤；删除沿用现有两步确认；空态引导新建。复用现有 `list_stickies` / `open_sticky` / `delete_sticky` / `create_sticky` 命令。
2. **标题下拉重做（D2）**：把编辑器里的原生 `<select>` 换成应用自绘的下拉浮层——基于既有依赖 `radix-ui` 的 `Select`（`position="popper"`，portal 到 body，主题令牌全走纸面 token），选项扩为「正文 + H1~H6」（按用户确认新增 H5/H6，样式沿用现有 h5/h6 规则）；支持 ↑↓ 循环、Enter 选择、Esc 关闭并还焦触发按钮、点击外部关闭，`aria-haspopup` / `role="listbox"` / `role="option"`，当前项高亮；鼠标按下阻止默认行为，不抢编辑器选区；底层行为仍走 `setParagraph` / `toggleHeading`。
3. **废弃免打扰（D3）** **BREAKING**（用户可感知行为：原先被静默的提醒现在会照常触发）：删除设置页「免打扰」区块、`Dnd` 模型与默认值、`Settings.dnd` / `SettingsPayload.dnd`、`store.rs` 落库校验分支、`scheduler.rs` 的 `in_dnd` 判定与「静默入账 / 出窗合并补发」分支、前端 `Dnd` 类型与文档说明；旧 `data.json` 里的 `dnd` 字段读取时忽略，不做破坏性迁移。命令仍不增不删。
4. **托盘精简（D4）**：托盘菜单删除「显示全部便签」「隐藏全部便签」「快速记录」「本周汇总导出」「管理提醒」五项及其分发处理，保留「打开主界面 / 待办悬浮窗（勾选）/ 新建便签 / 打开数据文件夹 / 退出」；菜单分隔线同步调整；`docs/ARCHITECTURE.md` §5 托盘段落同步更新。被删项的功能入口不受影响（快速记录走全局热键；提醒管理在「计划」视图；周汇总导出在「回顾」视图；便签管理迁移到卡片页）。
5. **移出淡化 IME 修复（D5）**：淡化判定改为「指针在窗外 **且** 窗口无焦点 **且** 非输入法组合中 **且** 无弹层」才淡化；增加 `compositionstart` / `compositionend`、`window` `blur` / `focus`、`document.hasFocus()` 守卫，并在淡化前加约 200–300ms 去抖，任何按键 / 鼠标进入立即取消；设置项语义保持不变（移出淡化仍可按用户意愿开启）。

## Capabilities

### New Capabilities

- `sticky-notes/management`：便签的独立管理面——卡片墙、搜索过滤、打开 / 删除 / 新建、空态与可见上限提示。
- `sticky-notes/fade-guard`：便签「移出淡化」的触发守卫——焦点、输入法组合、弹层与去抖，避免打字时误淡化。
- `editor/heading-select`：编辑器标题级别的自绘下拉——键盘、可访问性、主题一致性与不抢选区。
- `app-shell/tray`：托盘菜单的最终项集合、勾选态同步与移除项不再出现。
- `app-shell/quiet-mode-removal`：免打扰能力移除后的提醒交付行为与旧数据兼容。

### Modified Capabilities

无。当前仓库 `openspec/specs/` 尚无已归档主 spec（在途 change 的 spec 也尚未归档），视觉语言对齐只作为约束引用，不在本 change 修改 `visual-language` 的既有需求。

## Impact

- **前端**：`src-react/src/App.tsx`（新增路由与侧栏项）、新增便签卡片页组件与样式（复用 `.view` 骨架、`--fs-*`、`--paper-*` / `--ink-*` / `--seal-*` 令牌）、`src-react/src/views/SettingsView.tsx`（移除免打扰区块；便签清单可简化为指向卡片页的入口，或保留最小清单）、`src-react/src/rendering/tiptap-editor.tsx` + `src-react/src/rendering/styles/prosemirror.css`（自绘标题下拉）、`src-react/src/sticky/StickyWindow.tsx` + `src-react/src/sticky/sticky.css`（淡化守卫）、`src-react/src/lib/api.ts`（删除 `Dnd` 类型与 `Settings.dnd` / `SettingsPayload.dnd`）。
- **后端**：`src-tauri/src/models.rs`（删 `Dnd` / `DndPayload` / `Settings.dnd` / 默认值与 coerce 校验）、`src-tauri/src/store.rs`（删 `patch.dnd` 落库分支）、`src-tauri/src/scheduler.rs`（删 `in_dnd`、`NightState.last_dnd` / `quiet`、静默与合并补发分支及相关单测）、`src-tauri/src/telemetry.rs`（移除 `reason=dnd` 的用例）、`src-tauri/src/runtime.rs`（托盘菜单减 5 项与对应分发）。`src-tauri/src/main.rs` 只有菜单装配调用，签名不变。
- **命令契约**：不新增、不删除命令，四方（`generate_handler![]` / `#[tauri::command]` / `api.ts` / `docs/ARCHITECTURE.md` §2）保持同步；命令总数保持门禁当前断言值（当前 `test/command-parity.test.mjs` 断言 71 条——68 是并行的 `kb-storage-backup` 落地前的旧基线，本 change 不自带数字，改以门禁实际值为准）。
- **数据**：无破坏性迁移；旧 `data.json` 的 `settings.dnd` 字段被 serde 忽略，首次保存后自然消失；不新增持久文件，不改变轮转备份策略。
- **文档**：`docs/ARCHITECTURE.md`（§2 命令清单不增删、§3/§4 提醒规则删免打扰、§5 托盘菜单、§10 计数说明）、`docs/capability/03-提醒与通知.md`、`docs/capability/04-便签系统.md`、`docs/capability/08-设置.md`、README 功能清单（若提及）。文档交付前需人工审核（AGENTS.md §4.6）。
- **依赖与构建**：无新增依赖（标题下拉只用已有的 `radix-ui`，`src-react/package.json` 与锁文件都不动）；无打包配置变更；不改版本号。
- **验证策略**：`npm run check`（`scripts/check.ps1` 真实 17 步，含命令四方对账 / 文档契约 / 类型检查 / lint / 生产构建 + `cargo check` / `cargo test`）、`node test/command-parity.test.mjs`、`npm run build`、L4 真机清单（重点：便签卡片页增删开、托盘项、夜间提醒照常触发、标题下拉键盘操作、中文输入法下打字不淡化）。
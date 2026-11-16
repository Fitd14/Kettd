# 便签缺陷复现证据（#11 / #12）

> 只读调查产物：未修改项目源码、未执行 git 写命令、未启动应用、未运行 build/编译。唯一写入文件为本文件。

## 环境

| 项 | 值 |
| --- | --- |
| 项目根 | `C:\Users\simy\project\mywork\kettd` |
| 分支 / HEAD | master @ `2167a48`（完整 `2167a48c238bb1a86c88cd33dd79157809ca3eea`） |
| 日期 | 2026-11-13（Asia/Shanghai） |
| 工作区 | 4 个未提交改动（`src-react/src/rendering/styles/prosemirror.css`、`src-react/src/rendering/tiptap-editor.tsx`、`src-tauri/Cargo.toml`、`src-tauri/src/kb.rs`）+ `openspec/` 未跟踪。本文件引用的全部源码文件 `git status` 干净，故行号即 HEAD `2167a48` 行号。 |
| 验证方式 | 逐行阅读 + 只读 grep + 无头静态断言脚本 `%TEMP%\verify-sticky.mjs`（19/19 PASS，运行 `node %TEMP%\verify-sticky.mjs` 后退出码 0；脚本已按要求删除，断言清单见各节） |
| 未执行 | 未 `cargo test`、未 `npm run build`、未启动 Tauri 应用（任务约束）→ 所有运行时/视觉结论只在「需真机确认点」给出 |

---

## #11 便签正文无默认 Markdown 渲染；待办悬浮窗无最小化形态

### 现象

1. **自由便签正文永远先是可编辑源文**：正文区是原生 `<textarea>`（显示 Markdown 原文），只有点标题条上的「MD」按钮才临时切到渲染视图；`mdPreview` 是组件局部 `useState(false)`，初值恒为编辑态，不读 Settings、不落 StickyNote、不持久化 → **不存在「默认渲染」开关**，每次便签窗重新挂载都回到编辑态。
2. **知识便签反而走 TipTap 富文本**（`TipTapEditor`，无「MD」按钮），与自由便签的两种状态并存。
3. **待办悬浮窗没有最小化形态**：标题条只有「置顶 Pin」「✕（隐藏）」两个按钮；窗口固定 380×456（`decorations:false` + `skip_taskbar:true`，连系统最小化按钮都没有）；后端没有 todo 的 mini 尺寸/形态切换函数，也没有 `todo_float_mini` 字段。便签则有 mini：380×40 悬浮条，形态持久化到 `StickyNote.mini`。

### 证据（均已逐行核实）

**自由便签 Markdown**

- `src-react/src/sticky/StickyWindow.tsx:37` — `const [mdPreview, setMdPreview] = useState(false)`（初值恒 false；局部 state，无持久化）
- `src-react/src/sticky/StickyWindow.tsx:241-252` — 唯一 MD 入口：标题条「MD」按钮 `className={\`sticky-md-btn${mdPreview ? ' on' : ''}\`}`，`onClick` 内 `setMdPreview(!mdPreview)` 并记 `sticky_md_toggle`（:248）
- `src-react/src/sticky/StickyWindow.tsx:262-270` — `{mdPreview ? ( ... <MdStaticRenderer markdown={freeText} />（:269）...)}`，预览态点击整块回编辑（:265 `onClick={() => setMdPreview(false)}`）
- `src-react/src/sticky/StickyWindow.tsx:271-285` — 非预览态即 `<textarea className="sticky-free" value={freeText} maxLength={FREE_MAX} ... onChange onBlur={commitFree} />`（textarea 元素 :273-282；失焦保存 :281；字数上限 `FREE_MAX = 500` :22）
- `src-react/src/sticky/StickyWindow.tsx:113-123` — `commitFree`：`updateSticky(self.id, { content: next })`，内容走纯文本惰性保存
- `src-react/src/sticky/StickyWindow.tsx:216-222` — 知识便签分支用 `TipTapEditor`（`markdown={kbItem.bodyMd}`），无 MD/MD切换按钮
- `src-tauri/src/models.rs:283-318` — `pub struct Settings` 字段全集：theme / sticky_paper / sticky_fade / sticky_fade_opacity / sticky_pattern / capture_hotkey / main_hotkey / sticky_hotkey / todo_float_pinned / todo_hotkey / dnd / remind_cap_per_hour / onboarded / export_dir / telemetry_enabled —— **无任何 Markdown/预览字段**
- `src-tauri/src/models.rs:406-424` — `pub struct StickyNote` 字段仅 id / content（:413）/ mini（:414-416）/ kb_ref（:417-421）/ created_at / updated_at —— **无渲染模式字段**
- `src-react/src/lib/api.ts:121-143` — 前端 `Settings` 接口同样无 md/preview 字段；`SettingsPayload`（:145-161）无补丁通道
- `src-react/src/lib/api.ts:340-349` — `updateSticky` 补丁类型只有 `{ content?: string; mini?: boolean }`，**无 md 通道**
- `src-react/src/views/SettingsView.tsx:183-263` — 设置页「便签」区只有：纸面花纹（:185-199）、移出淡化（:200-225）、新建便签（:226-232）、便签清单（:233-262）—— **无 Markdown 渲染偏好开关**
- 佐证文档 `docs/capability/04-便签系统.md:17`：「自由便签：失焦自动保存；Markdown 渲染开关（预览态点击切回编辑）。」= 文档口径也只承诺手动开关
- 历史调研文档 `docs/design/product/markdown_engine_design.md:35` 曾提「默认启用轻量级实时渲染预览……按 Shift+Alt+N 一键切换」，但当前源码 grep `Alt+N|renderMode|previewMode|mdPreview` 只命中 StickyWindow 的局部 state，**该历史设计未落地**（文档为 Vue 时代调研，与现实现无对应）

**待办悬浮窗无最小化形态**

- `src-react/src/todo/TodoFloatWindow.tsx:48-51` — `togglePin`（仅置顶）；`:69-77` Pin 钮；`:78-80` ✕ 调用 `hideTodoFloat()`（隐藏，不是销毁）
- `src-react/src/todo/TodoFloatWindow.tsx` 全文 113 行 — grep `mini|Mini` **0 命中**；组件只有一个 `boot` state，无形态 state
- `src-tauri/src/runtime.rs:117-119` — `NOTE_EXPAND_SIZE = (380.0, 456.0)` / `NOTE_MINI_SIZE = (380.0, 40.0)`（mini 只服务便签）
- `src-tauri/src/runtime.rs:123-134` — `note_builder(app, label, mini)` 按 `mini` 二选一尺寸
- `src-tauri/src/runtime.rs:293-305` — `apply_note_shape(app, id, mini)`：便签专用形切换（`set_size`，展开时 `set_focus`）
- `src-tauri/src/runtime.rs:309-321` — `todo_builder(app, pinned)`：签名无 `mini` 参数，`inner_size(NOTE_EXPAND_SIZE.0, NOTE_EXPAND_SIZE.1)`（:314），`decorations(false)`、`skip_taskbar(true)`（:316-319）
- 全仓 grep `apply_todo_shape|todo_float_mini|todoMini|setTodoMini|TODO_MINI` → **0 命中**
- `src-tauri/src/models.rs:416` — `pub mini: bool` 在 `StickyNote` 上；`Settings` 只有 `todo_float_pinned`（:304-306），待办无 mini 字段
- `src-tauri/src/runtime.rs:350-369` — `open_todo_float` 只按 pin 建窗/显示，无形态分支

### 复现步骤（用户操作路径）

1. 用默认热键 `Alt+Shift+S`（`DEFAULT_STICKY_HOTKEY`）或「设置 → 便签 → 新建便签」开一张自由便签。
2. 在正文输入 `**粗体**`、`# 标题`、`- 列表` 等 Markdown；输入框始终显示**源文**（textarea），失焦才保存。
3. 点标题条「MD」按钮 → 正文变渲染结果；再点按钮或点预览区 → 回到编辑。
4. 关掉便签窗再新建一张 / 重启应用 → 仍是编辑态（没有默认渲染开关可开）。
5. 打开「设置 → 便签」→ 仅纸面花纹 / 移出淡化 / 新建 / 清单，**无 Markdown 渲染选项**。
6. 呼出待办悬浮窗（`Alt+Shift+T` 或托盘）→ 标题条只有 Pin 与 ✕，**无缩小按钮**；对比便签标题条有「缩小为悬浮条」按钮（`StickyWindow.tsx:253-255`），缩小后为 380×40 悬浮条（`StickyWindow.tsx:126-152`）。

### 是否可无头验证

**是（限静态事实层面）**：`node %TEMP%\verify-sticky.mjs` 全绿，19/19 PASS，退出码 0。相关断言：

| 断言 | 结果 | 证据位置 |
| --- | --- | --- |
| 11-1 mdPreview 初值硬编码 false | PASS | `StickyWindow.tsx:37` |
| 11-2 MD 预览仅由手动按钮切换 | PASS | `StickyWindow.tsx:242-252` |
| 11-3 正文 textarea 属 mdPreview=false 分支 | PASS | `StickyWindow.tsx:273` / 分支 :262-285 |
| 11-4 后端 Settings 无 Markdown/预览字段 | PASS | `models.rs:283-318`（命中 0） |
| 11-5 前端 Settings 无 Markdown/预览字段 | PASS | `api.ts:121-143`（命中 0） |
| 11-6 TodoFloatWindow 无 mini 状态/形态 | PASS | `TodoFloatWindow.tsx`（命中 0 行） |
| 11-7 todo_builder 固定尺寸、无 mini 分支 | PASS | `runtime.rs:311-321` |
| 11-8 `StickyNote.mini` 只存在于便签模型 | PASS | `models.rs:416` |
| 11-9 便签 mini 后端实现存在 | PASS | `runtime.rs:119` / `runtime.rs:295` |

**不能无头验证**：真实渲染效果、按钮视觉态、输入体验、窗口真实尺寸/无边框手感——需真机。

### 需真机确认点

1. 自由便签正文输入 Markdown 后**确实显示原文**（未被任何 CSS/富文本层覆盖）。
2. 点「MD」后 `MdStaticRenderer` 的实际渲染质量（标题/列表/代码块/换行），以及点预览区回编辑是否生效（`StickyWindow.tsx:262-267`）。
3. MD 按钮 `.on` 高亮态是否可视；切 mini 再展开后按钮状态是否保持（局部 state 不重置 —— 推断，需实测）。
4. 待办悬浮窗确实没有任何隐藏的最小化入口（双击标题条、热键、拖拽到边缘等）；窗口无系统最小化按钮（`decorations:false`）。
5. 知识便签（TipTap）与自由便签（textarea + MD 切换）两种编辑体验并存的观感是否构成缺陷。

---

## #12 便签关闭即删数据、无 CRUD/重开入口、上限 6/4

### 现象

1. **关闭即销毁**：便签窗「✕」与 Alt+F4/系统关闭都直接 `delete_sticky` —— 数据从 `data.json` 的 `stickies` 删除、`runtime.json` 的 `note_pos` 位置记录一并删除、窗口关闭，**无确认、无撤销、无回收站**。重启应用只恢复 `data.json` 里仍存在的便签，被关的永不回来。
2. **无独立便签 CRUD 页、无重开入口**：视图只有今天/收件箱/计划/回顾/知识库/设置 6 个；便签管理仅寄居「设置 → 便签」区，能力只有「新建 / 展开-缩小 / 删除（两步确认）」，**没有正文编辑页，也没有「重新打开已关闭便签」**；命令表里没有 `open_sticky`（`create_sticky`/`list_stickies`/`update_sticky`/`delete_sticky`/`sticky_self`/`register_sticky_hotkey` 之外无便签命令）。
3. **上限 6/4**：自由便签 `STICKY_CAP = 6`、知识便签 `STICKY_KB_CAP = 4`，分池计数，达上限时 `create_sticky` 返回中文错误；设置页显示「自由便签 n/6 · 知识便签 n/4」。
4. **备份兜底存在但无 UI**：`save()` 覆盖写前轮转 5 份备份，`get_backups` / `restore_backup` 命令与前端封装均存在，但**前端任何视图都没有调用**（只有设置页一句「备份 N 份可回滚」文案 + 打开数据文件夹按钮）。

### 证据（均已逐行核实）

**关闭即删数据**

- `src-react/src/sticky/StickyWindow.tsx:107-111` — `close`：`await deleteSticky(self.id)`（:110），注释「✕ = 关闭即销毁（一次性工具语义）：数据与窗口一并回收」
- `src-react/src/sticky/StickyWindow.tsx:29-30` — 组件头注释：「关闭即销毁（一次性工具语义）：✕ 直接 delete_sticky，无确认无撤销，误关靠写前轮转备份兜底」
- `src-react/src/sticky/StickyWindow.tsx:147-149` — mini 悬浮条 ✕：`title="关闭并删除"` → `close()`
- `src-react/src/sticky/StickyWindow.tsx:256-258` — 展开态 ✕：`title="关闭并删除（一次性便签）"` → `close()`（无确认弹窗）
- `src-tauri/src/commands.rs:581-596` — `delete_sticky` 命令：`store.delete_sticky(&id)?`（:587）→ `store.save()?`（:588）→ `store.save_runtime_only()?`（:589）→ 关窗（:590-592）→ 埋点 `sticky_delete`（:593）→ 广播 `store-changed`（:594）
- `src-tauri/src/main.rs:291-308` — `CloseRequested if label.starts_with("note:")`（:293）：`guard.delete_sticky(&id)`（:297）→ `save()`/`save_runtime_only()`（:298-299）→ 埋点 `sticky_delete` `via=alt_f4`（:307）。即 **Alt+F4 与 ✕ 同语义删数据**
- `src-tauri/src/store.rs:914-923` — `delete_sticky`：`self.data.stickies.retain(|s| s.id != id)`（:916）+ `self.runtime.note_pos.remove(id)`（:921）
- `src-tauri/src/models.rs:406-424` — `StickyNote` **无 `visible` 字段**（全仓 grep `pub visible` 0 命中）→ 无法「隐藏保留」
- `src-tauri/src/main.rs:240-257` — 启动恢复：读 `g.data.stickies.clone()`（:247）→ `for note in &stickies { open_note_window(...) }`（:254-257），**无条件恢复全部**，无可见性过滤
- 对比：待办悬浮窗 Alt+F4/✕ 只隐藏 —— `src-tauri/src/main.rs:285-290` `prevent_close()` + `hide_todo_float`；`TodoFloatWindow.tsx:78-80` 调 `hideTodoFloat()`

**无 CRUD 页 / 无 open_sticky / 无重开**

- 视图全集：`src-react/src/views/` = InboxView / KbView / PlannedView / ReviewView / SettingsView / TodayView（6 个，无便签视图）
- `src-react/src/views/SettingsView.tsx:183-263` — 便签区：新建（:226-232，按钮 :230）→ `doCreateSticky`（:90-96）`createSticky()`；形态切换（:249，`doToggleSticky` :98-103 `updateSticky(id,{mini:!mini})`）；删除（:250-257 两步确认，`doDeleteSticky` :105-112）。**无正文编辑、无「打开/重开」**
- `src-react/src/views/SettingsView.tsx:235` — 自述文案：「便签是一次性工具，关闭即销毁，此处删除同样不可恢复」
- `src-tauri/src/commands.rs` 便签命令全集：`create_sticky`(:504 async) / `list_stickies`(:512) / `sticky_self`(:519) / `update_sticky`(:542) / `delete_sticky`(:581) / `register_sticky_hotkey`(:708) —— **无 `open_sticky`**
- 全仓（`src-tauri/src`、`src-react/src`、`test`、`docs`）grep `open_sticky`：**源码 0 命中**（仅 `openspec/changes/fix-known-defects-iteration/{design,proposal,tasks}.md` 计划文本中出现，即本迭代待新增项）
- `src-react/src/lib/api.ts:337-352` — 便签封装仅 create/list/update/delete/self
- `src-react/src/views/SettingsView.tsx:58-60` — 清单数据源 `listStickies()`；全仓 `listStickies` 仅 api.ts:339 定义 + SettingsView:59 调用
- 佐证文档 `docs/ARCHITECTURE.md:426`：「`delete_sticky` …便签数据、窗口与 `note_pos` 记录一并回收，无确认无撤销，误关靠写前轮转备份兜底；便签窗的 Alt+F4 走同一语义」；`docs/capability/04-便签系统.md:41`：「close = `delete_sticky` 销毁，无撤销」

**上限 6 / 4**

- `src-tauri/src/models.rs:476` — `pub const STICKY_CAP: usize = 6;`（注释：便签总数上限，防「贴满废纸」）
- `src-tauri/src/models.rs:478` — `pub const STICKY_KB_CAP: usize = 4;`（知识便签上限，与自由便签分池互不影响）
- `src-tauri/src/runtime.rs:245-256` — `create_note` 内分池校验：知识便签 `kb_count >= STICKY_KB_CAP`（:246-250，Err「知识便签已达上限（4张），请先拆掉不需要的」）；自由便签 `free_count >= STICKY_CAP`（:251-255，Err「自由便签已达上限（6张），请先关闭一张」）
- `src-react/src/views/SettingsView.tsx:227` — UI 文案：`自由便签 {freeCount}/6 · 知识便签 {kbCount}/4`；计数来源 `freeCount`/`kbCount`（:47-48，按 `stickyList` 的 `kbRef` 分池）
- `src-react/src/views/SettingsView.tsx:183-263`（清单 :233-262）— 达上限时新建按钮无额外置灰逻辑（错误由后端 Err 冒泡到设置页错误条，推断：`:91-92` `r.err` 分支）

**备份兜底现状**

- `src-tauri/src/store.rs:954-961` — `save()`：`rotate_backups()`（:958）→ `write_data`（:959）→ `write_runtime`（:960）；即删除前的 `data.json` 进入 bak-1
- `src-tauri/src/infra/fs_store.rs:10` — `pub const BACKUP_SLOTS: usize = 5;`
- `src-tauri/src/commands.rs:99-101` `get_backups`、`:773-779` `restore_backup` 后端存在；`src-react/src/lib/api.ts:378-379` 前端封装存在
- **但前端无任何视图调用**：全 `src-react/src` grep `restoreBackup|getBackups|restore_backup|get_backups` 仅命中 api.ts:378-379；无视图引用。设置页只有 `SettingsView.tsx:357` 文案「正常 · 备份 N 份可回滚」+ `:361`「打开数据文件夹」按钮
- 结论：误删便签的实际恢复路径 = 手工进 `%APPDATA%\todo-list` 用备份文件回滚（**推断**：需停止应用后覆盖 `data.json` 或调用未挂 UI 的 `restore_backup`），非产品内可达

### 复现步骤（用户操作路径）

1. 新建一张便签，输入内容，失焦保存（`StickyWindow.tsx:281`）。
2. 点便签标题条「✕」→ 窗口立即消失；**无确认框**。
3. 打开「设置 → 便签」→ 清单里该便签消失（`listStickies` 重拉）。
4. 退出并重启应用 → 该便签不再出现（`main.rs:240-257` 只恢复 data.json 现存条目）。
5. 另一路径：选中便签窗按 `Alt+F4`（或点系统关闭）→ 同样删数据（`main.rs:291-308`），不再是「只关窗」。
6. 试图重开已关闭便签：设置清单只有现有便签的「展开/缩小/删除」，无「重新打开」；主界面六视图无便签页 → 无入口（无 `open_sticky`）。
7. 上限：连续新建自由便签至第 7 张 → 后端 Err「自由便签已达上限（6张），请先关闭一张」；从知识库条目详情「贴到桌面」（`src-react/src/components/kb/kb-item-detail.tsx:86-88` → `KbView.tsx:219-227` → `create_sticky`）第 5 张知识便签 → Err「知识便签已达上限（4张），请先拆掉不需要的」；设置页显示 `自由便签 n/6 · 知识便签 n/4`。
8. 误删恢复：设置页只提供「打开数据文件夹」，无回滚 UI（`restoreBackup` 无视图调用）。

### 是否可无头验证

**是（限静态事实层面）**：同一脚本 12-1 ~ 12-10 全 PASS，退出码 0。

| 断言 | 结果 | 证据位置 |
| --- | --- | --- |
| 12-1 便签 ✕ → deleteSticky（无确认） | PASS | `StickyWindow.tsx:108-111`，调用 :110 |
| 12-2 delete_sticky 命令删数据+关窗 | PASS | `commands.rs:581`，删数据 :587 |
| 12-3 Alt+F4/系统关闭 note:* 窗 → 删数据 | PASS | `main.rs:293-307`，删数据 :297 |
| 12-4 store.delete_sticky 连 note_pos 一起删 | PASS | `store.rs:914-921` |
| 12-5 启动无条件恢复全部便签（无 visible 过滤） | PASS | `main.rs:247` / :254 |
| 12-6 StickyNote 无 visible 字段 | PASS | `models.rs:408-424`（命中 0） |
| 12-7 上限常量 STICKY_CAP=6 / STICKY_KB_CAP=4 | PASS | `models.rs:476` / :478 |
| 12-8 建便签分池校验 | PASS | `runtime.rs:248` / :253 |
| 12-9 无 open_sticky 命令 | PASS | `commands.rs`/`api.ts`/`main.rs` 均 0 命中 |
| 12-10 无重开入口；设置页仅清单 | PASS | `SettingsView.tsx:234`（命中 0） |

**未做但理论上可无头验证（本次受约束未执行）**：`Store::delete_sticky` 是纯内存函数，可在 `cargo test` 里补一条单测断言「stickies 少一条 + note_pos 同步移除」（**推断**：当前 `src-tauri` 测试中未见针对 `delete_sticky` 的现成用例；本次未编译未跑测试）。

### 需真机确认点

1. 关闭便签后 `%APPDATA%\todo-list\data.json` 的 `stickies` 条目确实消失、`runtime.json` 的 `note_pos` 键消失（可对照 `save()` 轮转后的 `data.bak-1.json`）。
2. `Alt+F4` 后窗口确实关闭且无残留进程内窗口（label `note:<id>` 释放）。
3. 上限文案在实际触发时是否如后端 Err 原样展示（第 7 张自由 / 第 5 张知识），以及达上限后「新建便签」按钮是否有可用性反馈。
4. 关闭后其他窗口（设置页清单）是否经 `store-changed` 即时刷新（`commands.rs:594` 广播）。
5. 误删后的实际可恢复性：备份文件是否为删除前完整档、`restore_backup` 无 UI 时手工恢复的可行路径（需在临时数据目录验证，**勿写生产数据目录**）。
6. 待办悬浮窗 Alt+F4 只隐藏、便签 Alt+F4 删除，同一手势两种结果是否造成用户误操作（产品层确认）。

---

## 附：本次调查使用过的只读命令（可复核）

```powershell
cd C:\Users\simy\project\mywork\kettd
git rev-parse --short HEAD          # 2167a48
git status --porcelain              # 4 个未提交改动 + openspec/ 未跟踪，引用源码文件均干净
# 无头断言（临时脚本已删除，断言清单见上表）
node %TEMP%\verify-sticky.mjs       # 19/19 PASS, exit 0
# 关键只读检索
Select-String -Path src-tauri\src\commands.rs -Pattern 'pub fn .*sticky'   # 无 open_sticky
Select-String -Path src-tauri\src\models.rs   -Pattern 'STICKY_CAP|STICKY_KB_CAP|pub mini'
Get-ChildItem src-react\src\views -File                                    # 6 视图，无便签页
```
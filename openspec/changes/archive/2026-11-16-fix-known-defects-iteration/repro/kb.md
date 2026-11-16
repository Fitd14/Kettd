# 缺陷复现证据 — 知识库类（#4 / #5 / #6 / #7 / #14）

> 只读调查记录。每项含：现象 / 证据（真实 file:line 或命令输出）/ 复现步骤（用户操作路径）/ 是否可无头验证 / 需真机确认点。

## 环境

- 项目根：`C:\Users\simy\project\mywork\kettd`（Tauri v1.8.3 + React 19）
- 调查日期：2026-11-13（Asia/Shanghai）
- git HEAD：`2167a48`（2026-11-13 22:38:47 +0800，提交信息 `20261113-1438-c285-处理-createwebview-错误并指定可写用户数据目录：记进展`）
- 工作区不干净：`git status --porcelain` 显示 4 个已跟踪文件状态为 M（`src-react/src/rendering/styles/prosemirror.css`、`src-react/src/rendering/tiptap-editor.tsx`、`src-tauri/Cargo.toml`、`src-tauri/src/kb.rs`）+ 未跟踪 `openspec/`。实际 `git diff --numstat` 为 3 files / +217 −36：`prosemirror.css` +21/−2、`tiptap-editor.tsx` +173/−32、`kb.rs` +23/−2；`Cargo.toml` 的 diff 为空（仅 CRLF/LF 归一化提示，无内容改动）。
  - 本报告行号以**当前工作区**为准。`KbView.tsx`、`SettingsView.tsx`、`api.ts`、`models.rs`、`store.rs`、`commands.rs`、`fs_store.rs`、`runtime.rs`、`App.tsx` 无未提交改动 → 行号等同 HEAD；`tiptap-editor.tsx` 有未提交改动，其行号为工作区行号（已复核：diff 中不含查找/快捷键相关改动，只有斜杠菜单、标题、正文归一等内容）。
- 取证方式：只读静态代码定位 + 只读磁盘结构核对。**未启动应用、未运行 build、未执行任何 git 写命令、未修改项目源码、未新增依赖、未落任何临时脚本**（唯一一次 node 以 `node -e` 内联执行，只读 `JSON.parse`）。
- 磁盘核对范围：`%APPDATA%\todo-list` 仅列文件名/大小/时间，并把 `notes.json` 解析为「是否数组 / 条数 / 键名」，报告中未引用任何条目正文；未向该目录写入任何内容。
- **重要边界**：因未启动应用，凡标注「推断」的 UI 表现均由代码路径推导，需真机复核；代码事实（存在/不存在某段逻辑、文件形状）均为本次实际验证。

---

## #4 知识库无文件夹层级

### 现象

知识点没有归属文件夹的概念：`KbItem` 无分组字段，`notes.json` 根是裸数组，左栏是单一平铺列表，只能按标签过滤。用户无法创建/重命名/删除文件夹，也无法把条目移入文件夹或按层级浏览。

### 证据

1. 数据模型无归属字段：`src-tauri/src/models.rs:497-504` 的 `KbItem` 只有 `id / title / body_md / tags / created_at / updated_at` 六个字段，无 `folder_id`/`parent_id`；`models.rs:463-470` 的入参 `KbPayload` 只有 `title / body_md / tags`，即写命令层面也无法写入归属。
2. `notes.json` 根为裸数组：
   - `src-tauri/src/store.rs:607`：`pub kb: Vec<KbItem>`；
   - `src-tauri/src/store.rs:964-969`：`save_notes()` 直接 `serde_json::to_string_pretty(&self.kb)` → 根节点恒为 `[...]`。
   - 磁盘实测（只读）：`notes.json` 首个非空白字符为 `[`；`node -e` 输出 `Array.isArray= true  length= 3  itemKeys= id,title,bodyMd,tags,createdAt,updatedAt`。
3. 全仓无 folders 概念：对 `src-tauri/src` 与 `src-react/src` 全文检索 `folder|Folder|文件夹`，仅命中 `open_data_folder` 相关（`api.ts:354`、`commands.rs:766`、`runtime.rs:970/979`、`main.rs:123`、`SettingsView.tsx:10/361`）——不存在文件夹模型、命令、存储文件或 UI。
4. 左栏平铺：
   - `src-react/src/views/KbView.tsx:182-189`：`filteredItems.map(item => <KbItemRow ... onSelect={setSelectedId} />)`，无树/分组结构；
   - `KbView.tsx:105-108`：`filteredItems` 仅做 `activeTag` 过滤（`items.filter(i => i.tags.includes(activeTag))`）；
   - `KbView.tsx:163-169`：左栏上方只有 `KbTagFilter`（标签）；
   - `KbView.tsx:148-157`：顶栏只有 `KbSearchBar` + 「+ 新建」按钮，无「新建文件夹」。
5. 磁盘目录实测（`%APPDATA%\todo-list`，只读列目录）：`data.json`、`data.json.bak-1..5`、`events.jsonl`、`notes.json`、`runtime.json` —— 无 `folders.json`，也无任何 `notes*` 备份。

### 复现步骤（用户操作路径）

1. 启动 Kettd（真机）→ 侧栏点「知识库」。
2. 观察左栏：只有「搜索知识库…」输入框、标签筛选行、条目平铺列表；没有文件夹树、没有「新建文件夹」入口、没有条目右键/拖拽「移动到文件夹」。
3. 新建 2 条知识并分别打不同标签 → 切换标签只是在同一平铺列表内过滤，层级始终只有一层。
4. 旁证（可不进应用）：打开 `%APPDATA%\todo-list`，可见 `notes.json` 根为 `[` 数组、目录内无 `folders.json`。

### 是否可无头验证

部分可。模型字段、`notes.json` 根形状、无 `folders.json`、左栏平铺 JSX 均已在无头状态下实际核对（见上）。「用户无法通过任何应用内操作建立文件夹」这一运行时结论需真机走查；即使有隐藏入口，本次全量代码扫描也未发现（判定无）。

### 需真机确认点

- 侧栏/知识库内是否还有未在代码中发现的文件夹入口（结论：无，但需真机点击走查确认）。
- 左栏在条目增多后的实际滚动表现（本次只确认了渲染结构，未确认列表是否虚拟化）。

---

## #5 打开知识库不默认选中第一个知识

### 现象

进入知识库时左栏有条目但无任何选中项，右侧显示占位文案「选择一条知识查看详情」。唯一自动选中路径是深链接 `#/kb?id=`；且视图往返后选中态丢失（组件重挂载，`selectedId` 回到 `null`）。需求要求的「打开 / 切换文件夹后默认选中可见第一项」不存在；其中「切换文件夹」子场景当前不可达（文件夹能力见 #4）。

### 证据

1. `src-react/src/views/KbView.tsx:26`：`const [selectedId, setSelectedId] = useState<string | null>(null)` —— 初值 `null`。
2. 唯一的自动选中逻辑是深链接：`KbView.tsx:74-102`（匹配 `window.location.hash` 的 `?id=`，hashchange 响应 + `pendingDeepRef` 等 items 到位后补选）。需求要求的「默认回退第一项」不存在。
3. `selectedId` 的其余全部写入点（全文仅 5 处 `setSelectedId`）：`KbView.tsx:85/99`（deep link）、`:142`（新建后选中新条目）、`:187`（用户点击 `KbItemRow` → `setSelectedId`）、`:216`（删除当前项后置 `null`）。**没有**在 `items`/`filteredItems` 变化后做「当前选中不在可见列表则选第一项」的校正 effect。
4. 无选中时的 UI：`KbView.tsx:208-210` `{!selected && !loading && filteredItems.length > 0 && <div className="kb-empty">选择一条知识查看详情</div>}`。
5. 视图切换即重挂载：`src-react/src/App.tsx:132` `{boot && route === '#/kb' && <KbView boot={boot} />}`（条件渲染，切走即卸载）→ 再进知识库 `selectedId` 再次为 `null`。
6. 磁盘旁证：`notes.json` 当前 3 条（只读结构核对），即「有条目但不选中」的场景可稳定复现。

### 复现步骤（用户操作路径）

1. 启动应用（真机）→ 侧栏点「知识库」。
2. 左栏已有 3 条知识；右侧无详情，显示「选择一条知识查看详情」，左栏没有任何高亮条目（推断：代码路径无默认选中调用）。
3. 手动点第一条 → 正常显示详情；点侧栏「今天」再点回「知识库」→ 选中丢失、回到占位文案（推断：`App.tsx:132` 条件渲染导致重挂载）。
4. 从任务详情经 `#/kb?id=<条目id>` 打开知识库 → 该条被自动选中（deep link 分支 `KbView.tsx:74-102`），说明「深链接优先」已存在，缺的只是默认回退。
5. 文件夹子场景：当前无文件夹（#4），「点击文件夹后选中其第一项」无法执行，待 #4 落地后回归。

### 是否可无头验证

代码级可无头验证：初值、全部写入点、缺失的默认选中 effect 均为静态事实（已核对）。UI 是否真的不选中需真机一次点击确认；若存在其他组件在挂载时注入选择，本次全量扫描未发现（判定无）。

### 需真机确认点

- 打开知识库时左栏第一项是否确实无高亮、右栏是否显示占位文案。
- 视图往返后选中是否丢失。
- 深链接 `#/kb?id=` 在「items 尚未加载完成」时是否仍能选中（代码有 pending 兜底，需真机确认时序）。
- 文件夹能力（#4）落地后才可验证「切换文件夹后默认选中第一项」。

---

## #6 无 Ctrl+F / Ctrl+Shift+F

### 现象

知识库内没有「条目内查找栏」，也没有「聚焦全库搜索框」的键盘入口：全仓无 `Ctrl+F` / `Ctrl+Shift+F` 处理；Tiptap 编辑器的 `onKeyDown` 仅绑定 `Ctrl+T`（插入表格）；搜索框也没有可被聚焦的 ref。`design.md:63` 也记录了「WebView2 没有浏览器级查找条，需自绘查找栏」这一约束（本次静态验证与之一致：前端无任何查找实现）。

### 证据

1. 全仓按键扫描（`src-react/src/**/*.ts*`，`keydown|keyup|ctrlKey|metaKey|Ctrl\+`）共 17 处命中，无一处匹配 `F` 键查找：
   - 全局 `window.addEventListener('keydown', …)` 只有 1 处：`src-react/src/components/task-detail.tsx:38-43`，仅处理 `Escape` 关闭任务详情；
   - `src-react/src/lib/list-nav.ts:65-67`：列表 `j`/`k` 导航，条件里显式排除 `ctrlKey/metaKey`；
   - `src-react/src/rendering/tiptap-editor.tsx:395-396/468/486` 等仅为工具栏标题文案（`Ctrl+B` / `Ctrl+I` / `Ctrl+T` / `Ctrl+K`），非查找。
2. Tiptap 唯一组合键处理：`tiptap-editor.tsx:432-438`
   ```tsx
   // Ctrl/Cmd+T 插入表格
   const onKeyDown = (e: React.KeyboardEvent) => {
     if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 't') { … insertTable(…) }
   }
   ```
   无查找栏组件、无 ProseMirror Decoration、无 `search`/`find` 状态。
3. 知识库视图无按键监听：`KbView.tsx:1-10` import 清单与 1-242 全文均无 `keydown`/`useRef` 指向搜索框；顶栏结构仅 `KbSearchBar` + 新建按钮（`KbView.tsx:148-157`）。
4. 搜索框无法被代码聚焦：`src-react/src/components/kb/kb-search-bar.tsx` 全文 22 行，`<input>` 无 `ref`、无 `id`，且 `KbView`（`:149-153`）未传 ref → 当前没有任何代码路径能实现 `Ctrl+Shift+F` 的「聚焦搜索框」。
5. 后端无查找命令/快捷键：`src-tauri/src/*.rs` 检索 `accelerator|Ctrl+F|ctrl+f|find_bar|FindBar` 零命中；前端全局热键默认值均为 `Alt+Shift+*`（`SettingsView.tsx:24-27`），不占用 `Ctrl+F`。
6. 外部队列（推断，需真机）：Tauri v1 / WebView2 本身不提供应用级查找条；即使 WebView2 默认按键行为对 `Ctrl+F` 有反应，也不会产生需求要求的「条目内高亮 + 自绘查找栏 + Esc 关闭」行为。

### 复现步骤（用户操作路径）

1. 启动应用（真机）→ 知识库 → 打开任一条目进入编辑态（点击正文）。
2. 按 `Ctrl+F` → 无查找栏出现、正文无匹配高亮、焦点行为无变化（推断：无 handler）。
3. 在知识库视图（不聚焦编辑器）按 `Ctrl+Shift+F` → 焦点不会进入「搜索知识库…」输入框（推断：无 handler 且搜索框无 ref）。
4. 在搜索框内输入关键字后按 `Ctrl+F` → 同样无任何查找 UI（推断）。
5. 对照组：在编辑器内按 `Ctrl+T` → 插入 3×3 表格（`tiptap-editor.tsx:432-438`），证明编辑器 `onKeyDown` 通路存在、且只绑了这一个组合键。

### 是否可无头验证

「代码中不存在 `Ctrl+F`/`Ctrl+Shift+F` 实现」可无头静态验证（已完成，证据见上）。按键在 WebView2 中的真实派发、是否被宿主/系统拦截、焦点在输入框/编辑器时的行为，需真机。

### 需真机确认点

- `Ctrl+F` 实际表现：完全无反应，还是触发 WebView2 的默认行为（两者都不满足需求，但要记录实际现象）。
- `Ctrl+Shift+F` 是否被系统/其他应用占用（尤其全局热键达到系统级时）。
- 实现后：输入框内不误触发、中文输入法组合态按键、`Esc` 关闭查找栏。

---

## #7 备份/恢复前端未接线（且 KB 数据未纳入轮转、损坏时假空态）

### 现象

设置页「数据健康」只显示「备份 N 份可回滚」与「打开数据文件夹」，没有备份列表、没有恢复按钮/确认流程；`get_backups` / `restore_backup` 前端封装存在但全仓零调用。`notes.json` 没有写前轮转（只有 `data.json` 有 5 份轮转备份），且解析失败时静默变成空库 → 知识库渲染「知识库空空如也」，不提示异常、不提供恢复入口。

### 证据

1. 后端能力已存在：
   - `src-tauri/src/commands.rs:98-102` `get_backups` → `store.backups()`；
   - `commands.rs:772-788` `restore_backup` → `store.restore_backup(&slot)`；
   - `src-tauri/src/main.rs:93,124` 已注册这两个命令；
   - `src-tauri/src/store.rs:937-939` `backups()` → `backend.list_backups()`；`store.rs:996-1027` `restore_backup`。
2. 前端封装存在但零调用：`src-react/src/lib/api.ts:378` `getBackups`、`api.ts:379` `restoreBackup`。对 `src-react/src` 全量检索 `getBackups|restoreBackup`：**仅命中 `api.ts` 定义本身**，无任何组件 import/调用。
3. 设置页仅展示条数：`src-react/src/views/SettingsView.tsx:353-362`（`aria-label="数据"` 区）
   - `:356-358`：`正常 · 备份 ${boot.health.backups.length} 份可回滚`（只读计数）；
   - `:361`：唯一的按钮是「打开数据文件夹」→ `openDataFolder()`；
   - 无 `getBackups` 列表、无「恢复」按钮、无二次确认、无恢复中/恢复结果提示。
   - 计数来源：`models.rs:675-686` `DataHealthV2 { health, backups, corrupt_file, last_error, writable }`，`backups` 字段（`:680`）。
4. 轮转只覆盖 `data.json`：
   - `src-tauri/src/infra/fs_store.rs:10` `BACKUP_SLOTS: usize = 5`；
   - `fs_store.rs:56-58` `backup_path()` 固定拼 `data.json.bak-{slot}`；
   - `fs_store.rs:113-139` `rotate_backups()` 只在 `data_path()` / `backup_path()` 之间搬移；
   - `src-tauri/src/store.rs:954-961` `save()` 调 `rotate_backups()` 后写 data；
   - `store.rs:964-970` `save_notes()` 直接 `backend.write_notes(&json)`，**无轮转**（注释自述「v1 不轮转，H2 成熟化」）；`fs_store.rs:288-291` `write_notes` 也只做原子写。
5. 备份列表/恢复只认 `data.json`：
   - `fs_store.rs:176-226` `list_backups()` 遍历 `backup_path(slot)`，`name` 恒为 `data.json.bak-{slot}`（`:206/:216`），解析的 `task_count` 取 data JSON 的 `tasks` 数组（`:199-203`）；
   - `store.rs:997-1027` `restore_backup` 只还原 `AppData`（任务数据），全程不触碰 `self.kb`。
6. 磁盘实测（`%APPDATA%\todo-list`，只读列目录）：`data.json.bak-1 .. bak-5` 齐全；`Get-ChildItem -Filter 'notes*'` 仅返回 `notes.json` —— **无任何 notes 备份**。
7. 损坏假空态（代码路径）：
   - `store.rs:755-763`：`read_notes().ok().flatten().and_then(|text| parse_value(&text).ok()).and_then(|value| serde_json::from_value::<Vec<KbItem>>(value).ok()).unwrap_or_default()` —— 注意 / 解析失败全被 `.ok()` 吞掉，得到空 `Vec`，且 **不改 `health` / `last_error`**（对比 data.json 损坏走 corrupt 通道：`store.rs:681-695`、`:766-774`）；
   - `fs_store.rs:145-150` `quarantine()` 只处理 `data_path()`，notes 无隔离文件；
   - `commands.rs:807-812` `get_kb_items` 原样返回 `store.kb.clone()`（空）；
   - 前端 `KbView` 在 `filteredItems.length === 0 && !query && !activeTag` 时渲染 `KbEmptyState variant="empty"`（`KbView.tsx:176-181`），其文案为「知识库空空如也」（`src-react/src/components/kb/kb-empty-state.tsx:8`）。
8. 附带缺口（实现 #7 时需处理）：当前 `restore_backup` 末尾用 `write_in_place()`（`store.rs:987-994` 注释「只写不轮转（回滚备份时使用）」，调用点 `store.rs:1024-1025`），与本次 spec「恢复前当前数据先轮转为新备份」不一致。

### 复现步骤（用户操作路径）

1. 启动应用（真机）→ 设置 →「数据」区：只有「正常 · 备份 5 份可回滚」和「打开数据文件夹」，**没有可点开的备份列表**（推断：无 `getBackups` 调用）。
2. 尝试「恢复」：UI 无任何入口；只能通过 devtools 手动 invoke（不建议），或改代码——即用户路径不存在。
3. 打开 `%APPDATA%\todo-list`：只有 `data.json.bak-1..5`，没有 `notes.json` 的备份。
4. 损坏场景（**需隔离环境/真机，勿直接改生产 notes.json**）：把 `notes.json` 改成非法 JSON 后启动 → 知识库显示「知识库空空如也」，设置页数据健康仍显示正常（推断：`store.rs:755-763` 吞错且不改 `health`）。
5. 验收对照：修复后应在设置页看到备份列表并可恢复；`notes.json` 同样有 5 份轮转；损坏时出现明确错误态/恢复入口而非空态。

### 是否可无头验证

- 可无头：前端零调用（静态检索）、设置页无恢复 UI（JSX 静态）、磁盘无 notes 备份（只读列目录）、轮转代码只覆盖 `data.json`（静态）。
- 需真机：实际执行一次 data 备份恢复后的数据一致性、损坏 notes.json 时的真实 UI（是否假空态、健康度是否仍 ok）、恢复前轮转行为。

### 需真机确认点

- 真机恢复一次 data 备份：是否成功、恢复后 UI 是否刷新（`store-changed("data")` 事件，`commands.rs:786`）。
- `notes.json` 损坏的真实 UI：必须用隔离数据目录或先备份生产目录，再破坏文件验证；本次未改生产文件。
- 设置页文案「备份 N 份」与磁盘实际份数是否一致（当前磁盘 bak-1..5 齐全，5 份）。
- 恢复确认弹窗/错误文案的最终形态（需求验收脚本，当前无实现）。

---

## #14 知识库无「打开本地数据目录」入口

### 现象

知识库视图内没有「打开数据目录」按钮。该能力后端已存在，且设置页与托盘菜单已有等价入口，但需求要求的**知识库入口**缺失（spec: knowledge-base/organization「知识库 SHALL 提供一个入口直接打开应用数据目录」，且 `design.md:24` 明确「#14 先只做『打开数据目录』」）。

### 证据

1. 后端命令已存在并注册：`src-tauri/src/commands.rs:765-768`
   ```rust
   #[tauri::command]
   pub fn open_data_folder() -> Result<(), String> {
     runtime::open_in_folder(&crate::store::data_dir())
   }
   ```
   `src-tauri/src/main.rs:123` 已注册；`src-tauri/src/runtime.rs:979-1002` `open_in_folder`（Windows 分支 `Command::new("explorer").arg(path).spawn()`，`:982-984`；失败文案「打不开这个位置，请手动前往」`:1000`）。
2. 数据目录定义：`src-tauri/src/infra/fs_store.rs:17-22` `default_dir()` = `dirs::data_dir().join("todo-list")` → Windows `%APPDATA%\todo-list`（`dirs::data_dir()` 在 Windows 即 `%APPDATA%`，同文件 `:17` 注释）。
3. 前端封装存在：`src-react/src/lib/api.ts:354` `export const openDataFolder = () => call<void>('open_data_folder')`。
4. 调用点全仓检索（`src-react/src`）：仅两处 —— `src-react/src/views/SettingsView.tsx:10`（import）与 `SettingsView.tsx:361`（「打开数据文件夹」按钮）。`src-react/src/views/KbView.tsx` 无 import、无调用；`src-react/src/components/kb/*` 无调用。
5. 知识库顶栏结构：`KbView.tsx:148-157` 只有 `KbSearchBar` +「+ 新建」按钮，无打开目录入口。
6. 既有等价入口（避免误判为「应用完全无入口」）：
   - 托盘菜单：`runtime.rs:904` `.add_item(CustomMenuItem::new("data_dir", "打开数据文件夹"))`，点击处理 `runtime.rs:969-970` 打开同一目录；
   - 设置页按钮：`SettingsView.tsx:361`。
   - 因此 #14 的缺口精确定位为：**知识库内缺入口**，不是后端/全局能力缺失（零新命令即可满足 spec）。

### 复现步骤（用户操作路径）

1. 启动应用（真机）→ 侧栏「知识库」→ 查看顶栏与左栏：只有搜索框和「+ 新建」，无「打开数据目录 / 打开数据文件夹」按钮（推断：`KbView.tsx:148-157`）。
2. 对照可用入口：设置 →「数据」→「打开数据文件夹」可点击（`SettingsView.tsx:361`）；托盘右键菜单 →「打开数据文件夹」也可点击（`runtime.rs:904`）。
3. 需求验收路径（修复后）：知识库视图点击「打开数据目录」→ 系统资源管理器打开 `%APPDATA%\todo-list`，可见 `data.json`、`notes.json` 等文件。

### 是否可无头验证

代码/调用点层可无头验证（已完成：命令存在、api 封装存在、KB 视图零调用、顶栏无按钮）。点击后是否真的拉起资源管理器（`explorer` spawn 成功、目录存在）需真机。

### 需真机确认点

- 真机点击既有设置页按钮/托盘项，确认能打开 `%APPDATA%\todo-list`（该目录当前存在，含 data.json、bak-1..5、events.jsonl、notes.json、runtime.json 共 9 个文件）。
- 修复后 KB 按钮的落点/文案，避免与设置页按钮重复造成信息架构混乱（spec 只要求「知识库提供入口」，未规定位置）。
- 数据目录被迁移或不存在时 `open_in_folder` 的表现（`explorer` 对不存在路径的行为，需真机）。

---

## 汇总（复现基线状态）

| 项 | 结论 | 无头验证 | 真机确认 |
| --- | --- | --- | --- |
| #4 无文件夹层级 | 已证实：模型无归属字段、notes.json 裸数组（实测 3 条、根 `[`）、无 folders.json、左栏平铺 | 结构/代码可无头 | 应用内是否隐藏入口、列表滚动表现 |
| #5 不默认选中 | 已证实：`selectedId` 初值 null，仅 deep link 自动选中；无默认回退 effect；视图往返重挂载丢失选中 | 代码可无头 | 实际高亮/占位、往返丢失、文件夹子场景待 #4 |
| #6 无 Ctrl+F / Ctrl+Shift+F | 已证实：全仓无查找 handler（唯一全局 keydown 为 Escape），Tiptap 仅 Ctrl+T，搜索框无 ref、后端无查找命令 | 代码可无头 | 按键实际派发、WebView2 默认行为、IME 组合态 |
| #7 备份/恢复未接线 | 已证实：api 封装零调用、设置页仅计数、notes 无轮转/无备份（磁盘实测）、损坏静默空库（代码路径） | 部分可无头 | 恢复一致性、损坏 UI、恢复前轮转 |
| #14 无打开数据目录入口 | 已证实：后端命令+api 封装存在；调用点仅设置页，KB 视图零调用；托盘/设置已有等价入口 | 代码可无头 | explorer 实际打开、按钮落点与重复入口 |

**本批仅 #7 的「损坏 notes.json → 假空态」与 #14/#4/#5/#6 的 UI 行为需要真机；其余均已静态/磁盘只读取证。**
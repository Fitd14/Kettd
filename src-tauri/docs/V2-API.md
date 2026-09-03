# Kettd v2 · Wave-1 后端契约（v2-api）

真相源：`spark-output/prd/kettd-v2.md` §6.1 / §6.2 / §6.3 / §6.4 / §6.6 + §6.9 状态矩阵。
本文档是前端对接的唯一契约：命令名、invoke 参数名（camelCase）、返回结构、事件、错误文案。
实现在 `src-tauri/src/{models,store,runtime,scheduler,export,commands,main}.rs`。

## 0. 通用约定

- 所有命令返回 `Result<_, String>`。**Err 字符串是可直接显示给用户的中文短句**，不含路径、堆栈、内部类型名。前端可直接 `toast(err)`。
- 时间全链路 **本地语义字符串**（不做任何 UTC / 时区换算，不用 `toISOString`）：
  - 完整时刻 `YYYY-MM-DDTHH:mm`
  - 仅日期 `YYYY-MM-DD`
  - 提醒循环时刻 `HH:MM`，兼容 v1 多时刻 `HH:MM/HH:MM`
  - 解析失败的旧值：置 `null` 并写入迁移报告（不猜测、不伪造）
- 数据目录沿用 v1：`%APPDATA%/todo-list`（`data.json`）。
- 写盘 = 读 → 内存改 → 序列化 `data.json.tmp` → `fs::rename` 原子替换；**每次覆盖写前**把当时的 `data.json` 轮转为 `data.json.bak-1`，旧档顺移，保留 5 份（`bak-1..bak-5`）。
- 未从 corrupt 状态成功恢复前，**所有写命令一律返回 Err**（`数据文件已损坏，请先从备份恢复后再操作`），杜绝 v1 的「静默空库」。
- `deletedAt` 非空 = 回收站；读取默认过滤，`get_tasks{includeDeleted:true}` 才带出。

### 存储与恢复的状态机（§6.9 的后端侧）

| 状态 | health.health | 触发条件 | 允许的操作 |
| --- | --- | --- | --- |
| ok | `ok` | 正常读写 | 全部命令 |
| corrupt | `corrupt` | data.json 缺失内容不可用（IO 失败 / JSON 截断 / 结构不可还原 / v1 迁移异常），坏件已另存 `data.corrupt.<ISO时间>` | 只读命令 + `get_backups` / `restore_backup` / `get_data_health`；写命令全部拒绝 |
| writeFailed | `writeFailed` | 轮转或写盘失败（磁盘满 / 只读目录 / 无权限） | 读命令可用；错误原文在 `lastError`（中文），下次成功写盘后自动回到 `ok` |

## 1. 领域模型（camelCase）

```ts
type Category = '工作' | '学习' | '生活'
type Priority = 'high' | 'med' | 'low'
type Source = 'capture' | 'manual' | 'seed'
type Repeat = 'none' | 'daily' | 'workdays' | 'weekly'
type FloatForm = 'topmost' | 'desktop' | 'mini'

interface Subtask { id: string; title: string; done: boolean }
interface Note { id: string; author: string; content: string; createdAt: string }

interface Task {
  id: string
  title: string
  note: string                     // 正文备注（v1 description）
  category: Category
  priority: Priority
  dueAt?: string | null            // YYYY-MM-DD 或 YYYY-MM-DDTHH:mm（本地语义）
  remindAt?: string | null         // 单次提醒；HH:MM 视作每日
  plannedDate?: string | null      // 加入今天的日期
  carriedFrom: number              // 逾期粘留天数
  done: boolean
  doneAt?: string | null
  deletedAt?: string | null        // 软删除时刻
  legacy?: boolean                 // v1 完成但无完成时刻（缺省 false 且不写出）
  subtasks: Subtask[]
  notes: Note[]
  source: Source
  createdAt: string
  updatedAt: string
}

interface Reminder {
  id: string
  title: string
  time: string                     // HH:MM（多时刻用 / 分隔）或 YYYY-MM-DDTHH:mm
  category: Category
  enabled: boolean
  completed: boolean               // true = 已完成/停用
  repeat: Repeat
  lastFired?: string | null
  snoozedUntil?: string | null     // 稍后重触发时刻
  legacy?: boolean
}

interface Dnd { enabled: boolean; from: string; to: string }   // 默认 true, 23:00, 07:30

interface Settings {
  theme: string                    // 主题名（float / dark / light，前端解释）
  floatForm: FloatForm
  captureHotkey: string            // 默认 "Alt+Shift+A"
  mainHotkey: string | null        // 打开主界面全局热键；null = 未绑定；默认 "Alt+Shift+O"
  dnd: Dnd
  remindCapPerHour: number         // 默认 3
  onboarded: boolean
  exportDir?: string | null
}

interface BackupInfo {
  slot: string                     // "1".."5"
  name: string                     // data.json.bak-1
  createdAt: string
  sizeKb: number
  taskCount: number
  readable: boolean
  error?: string                   // readable=false 时的中文原因
}

interface DataHealthV2 {
  health: 'ok' | 'corrupt' | 'writeFailed'
  backups: BackupInfo[]
  corruptFile?: string | null      // data.corrupt.<ISO时间>
  lastError?: string | null
  writable: boolean
}

interface MigrationIssue { collection: string; id: string; field: string; raw: string; action: string }
interface MigrationReport {
  taskCount: number
  reminderCount: number
  invalidDates: number             // 非法时间被置空的条数
  legacyDone: number               // completed 无时刻 → doneAt 置空 + legacy 的条数
  archivedTo: string               // data.v1.json
  issues: MigrationIssue[]
}

interface Bootstrap {
  tasks: Task[]
  reminders: Reminder[]
  settings: Settings
  health: DataHealthV2
  migration: MigrationReport | null
}

interface RestoreResult { restored: number; health: DataHealthV2 }
```

`AppData`（落盘结构，前端不必直接读，但备份文件就是这个形状）：
`{ tasks, reminders, settings, fired, migration? }`，`fired: string[]` 是已处理提醒的稳定键（内部字段，前端不写、不清）。

## 2. 命令清单（invoke 参数名一律 camelCase）

| 命令 | 参数 | 返回 | 说明 |
| --- | --- | --- | --- |
| `get_bootstrap` | — | `Bootstrap` | 一次拿齐 tasks/reminders/settings/health（+ migration） |
| `get_tasks` | `includeDeleted?: bool` | `Task[]` | 默认过滤软删 |
| `get_task` | `id` | `Task \| null` | |
| `add_task` | `args: Partial<Task>`（至少 `title`） | `Task` | id/createdAt/updatedAt 后端生成；`source` 缺省 `manual`，捕获条传 `capture` |
| `update_task` | `id`, `patch: Partial<Task>` | `Task` | **部分字段 MERGE**；未出现的字段保持原值；时间字段自动归一，非法格式直接 Err |
| `toggle_task` | `id` | `Task` | 勾选写 `doneAt`（本地时刻），取消勾选清空 |
| `delete_task` | `id` | `Task`（被删的完整对象） | **软删**：只写 `deletedAt`，子任务与备注原样保留 |
| `undo_delete` | — | `Task \| null` | 恢复最近一次删除（任务 / 子任务 / 备注，后进先出）；重启后仍可恢复最近一条软删任务 |
| `restore_task` | `id` | `Task` | 从回收站恢复指定任务 |
| `purge_task` | `id` | `Task` | 彻底清除（仅允许对回收站中的任务） |
| `add_subtask` | `taskId`, `title` | `Task`（父任务） | |
| `toggle_subtask` | `taskId`, `subtaskId` | `Task` | |
| `delete_subtask` | `taskId`, `subtaskId` | `Task` | 可 `undo_delete` 还原 |
| `add_note` | `taskId`, `note: { content, author?, createdAt? }` | `Task` | |
| `delete_note` | `taskId`, `noteId` | `Task` | 可 `undo_delete` 还原 |
| `get_reminders` | — | `Reminder[]` | |
| `add_reminder` | `reminder: { title, time, category?, repeat?, enabled? }` | `Reminder` | |
| `update_reminder` | `id`, `patch: Partial<Reminder>` | `Reminder` | 部分字段 MERGE |
| `delete_reminder` | `id` | `void` | 硬删（同时清 `fired` 里该提醒的稳定键） |
| `toggle_reminder` | `id` | `Reminder` | completed ⇄ enabled；重新启用会清 `snoozedUntil` |
| `snooze_reminder` | `id`, `minutes` | `Reminder` | 写 `snoozedUntil = now + minutes`（1–720） |
| `get_settings` | — | `Settings` | |
| `set_settings` | `patch: Partial<Settings>` | `Settings` | 部分字段 MERGE；改 `captureHotkey` 会真实重绑热键，失败整次回滚并 Err「快捷键被占用，请用备用入口」；改 `floatForm` 同步改窗口 |
| `open_main_window` | — | `void` | 显示主窗并请求焦点 |
| `show_float` | — | `void` | 显示悬浮面板并请求焦点 |
| `hide_float` | — | `void` | 隐藏悬浮面板（不退出进程） |
| `open_data_folder` | — | `void` | 在文件管理器里打开 `%APPDATA%/todo-list`；失败 Err「打不开这个位置，请手动前往」 |
| `set_float_form` | `form` | `void` | `mini` → 无边框 + 300×56 + 禁止 resize + 贴主屏右下角；`topmost` → 置顶 + 回 380×520；`desktop` → 取消置顶且失焦自动收起；结果写回 `settings.floatForm` |
| `register_capture_hotkey` | `combo` | `Settings` | 例 `Alt+Shift+A`；注册失败 → Err「快捷键被占用，请用备用入口」（保持旧值） |
| `register_main_hotkey` | `combo` | `Settings` | 打开主界面全局热键；`combo` 空串 = 解绑（`mainHotkey` → `null`）；与 `register_capture_hotkey` 同一约束：组合键互斥、失败保持旧值 |
| `open_capture_overlay` | — | `void` | label `capture`：560×112、无边框**不透明纸片卡**（背景跟随主题 `--card`，不依赖 WebView2 透明）、屏幕上部 28% 居中、置顶，显示后 emit `capture-opened` 让前端聚焦输入框；失焦自动收起 |
| `close_capture_overlay` | — | `void` | |
| `export_weekly` | `week?: string`, `format?: 'md' \| 'csv'`, `dir?: string` | `string`（完整路径） | `week` = `current`（默认）/ `last` / `YYYY-Www`；文件名含 ISO 周（如 `周汇总-2026-W36.md`）；二次导出不覆盖（自动 `-2`/`-3`）；目标不可写 → Err「这个位置写不了，换个位置」 |
| `get_backups` | — | `BackupInfo[]` | 含不可读备份（`readable:false` + 中文 error） |
| `restore_backup` | `slot`（`"1".."5"`，也吃 `data.json.bak-3` 这种整名） | `RestoreResult` | 成功即解除 corrupt 封锁；损坏原件已留存为 `data.corrupt.*`，`data.v1.json` 永不删 |
| `get_data_health` | — | `DataHealthV2` | 损坏恢复页数据源 |
| `clear_migration_report` | — | `MigrationReport \| null` | 前端展示完迁移报告后清账，避免每次启动重复提示 |
| `get_form_hints` | — | `{ categories, priorities, floatForms, sources, repeat, defaultHotkey, defaultCap, retentionDays, today }` | 表单常量，避免前端硬编码 |

参数名映射：Rust 侧 `snake_case` 形参由 Tauri v1 宏自动转成 camelCase（`task_id` → `taskId`，`include_deleted` → `includeDeleted`）。结构体入参（`TaskPayload` 等）本身带 `rename_all = "camelCase"`。

### patch 语义补充

- 时间字段传 `null` 或空串 = 清空；传字符串 = 归一后写入，归一失败 → Err（不会静默丢值）。
- `title` 传空串 → Err「标题不能为空」。
- `category` / `priority` / `floatForm` / `dnd.from` / `dnd.to` 非法值 → 中文 Err，不落库。
- `subtasks` / `notes` 传数组 = 整体替换（空标题项会被丢弃）。

## 3. 事件表（后端 → 前端）

| 事件 | 负载 | 发出时机 |
| --- | --- | --- |
| `store-changed` | `{ id: string, kind: 'changed' \| 'fired' \| 'missed' }` | 任何写库成功后（id = 受影响实体 id；`settings` / `data` 表示全局）；提醒触发 `fired`；免打扰静默入账、超 24h 失效、启动补发清账为 `missed` |
| `capture-opened` | `null` | `open_capture_overlay` 显示捕获条后，前端收到即聚焦输入框 |
| `theme-changed` | `string`（主题名） | `set_settings` 改 `theme` 成功后跨窗同步 |
| `navigate` | `string`（前端路由，如 `/planned/reminders`） | 托盘「管理提醒」「本周汇总导出」打开主窗后要求跳转 |
| `open-capture` | `null` | 降级路径：`capture.html` 资源不存在时，`open_capture_overlay`（含全局热键）改为全局广播该事件并唤起主窗，由前端自挂的快捷输入条接管；同时返回 Err「快速记录条没就绪，请用悬浮面板输入框」 |

## 4. 定时器与提醒规则（托盘常驻，A3 批复）

后台 1 个调度线程，1s tick（`chrono::Local`）：

1. 生成候选：`Reminder`（`enabled && !completed`，`HH:MM[/HH:MM]` 按 `repeat` 决定哪些天要响；带日期的 `time` 是单次）与 `Task.remindAt`（`HH:MM` = 每日，完整时刻 = 单次；软删/已完成不参与）。`snoozedUntil` 到期优先于常规排期。
2. 稳定键去重：`r|<id>|<YYYY-MM-DDTHH:mm>` / `t|<id>|<原串>`，记在 `data.fired` → 跨重启不重复响。
3. 到点 → 系统通知 title「待办提醒」body `<标题> · <YYYY-MM-DDTHH:mm>`（snooze 触发时后缀「（稍后提醒）」）；点击深链由前端窗口处理。
4. 每小时上限 `settings.remindCapPerHour`（默认 3）：内存滑窗，超上限的排队，下个 tick 窗口放开再补。
5. 免打扰 `settings.dnd`（默认 23:00–07:30，可关）：时段内静默入账（不清账），结束后合并成一条「免打扰期间有 N 条提醒」，事件仍按 `fired`/`missed` 上报。
6. 错过超过 24h：不再发通知，只记 `missed` 并清账。
7. 应用未运行期间错过的：启动后第一次 tick 合并成一条「错过 N 条提醒」，不逐条轰炸。
8. 每次落库（`lastFired` / 单次提醒的 `completed=true` 或 `remindAt` 清空 / `fired` 键）都走同一套原子写，并 emit `store-changed`。
9. 顺带：`floatForm === 'desktop'` 且悬浮面板可见又失焦（捕获条未显示）→ 自动隐藏。
10. corrupt 状态下调度线程不发通知也不写库（等用户恢复）。

## 5. 托盘与窗口（配置侧）

- `tauri.conf.json`：`productName: "待办列表"`；三窗 `main`(1100×720, min 1024×640, visible:false) / `float`(380×520, resizable:false, alwaysOnTop:true, skipTaskbar:false) / `capture`(560×112, decorations:false, transparent:false, alwaysOnTop:true, skipTaskbar:true, visible:false)；`allowlist` 沿用 v1 的 `api-all`。
- 托盘在代码里创建，`id = "main"`（v1 的 `tauri.systemTray` 配置项只吃 `iconPath`，没有 `id` 字段，v1 运行时的托盘菜单必须由代码构建）。
- 托盘菜单（术语表：悬浮面板 / 提醒 / 周汇总）：
  `打开主界面` · `快速记录`（等同 `open_capture_overlay`，前端也可监听 `capture-opened`）· 分隔 · `显示悬浮面板` / `隐藏悬浮面板` · 子菜单 `悬浮形态`(置于顶层 / 嵌入桌面 / 迷你条，当前形态带勾选) · 分隔 · `管理提醒` / `本周汇总导出` / `打开数据文件夹` · 分隔 · `退出`。

## 6. 错误文案表（前端可直接展示）

| 场景 | 文案 |
| --- | --- |
| 未恢复的损坏库 | 数据文件已损坏，请先从备份恢复后再操作 |
| 写盘失败 | 临时文件写入失败，本次没有保存：磁盘或路径不可用 / 没有写入权限 / 目录是只读的 … |
| 备份不可用 | 这个备份不存在，换一个试试 / 这个备份也读不了，换一个试试 |
| 导出目标不可写 | 这个位置写不了，换个位置 |
| 热键被占 | 快捷键被占用，请用备用入口 |
| 时间格式 | 提醒时间格式不对，用 HH:MM 或 YYYY-MM-DDTHH:mm / 截止日期格式不对，请用 YYYY-MM-DD |
| 找不到对象 | 找不到这条待办，可能已经被删除 / 找不到这条提醒，可能已经被删除 / 找不到这个子任务 / 找不到这条备注 |
| 撤销失效 | 没有可撤销的删除了 |
| 空输入 | 标题不能为空 / 子任务名不能为空 / 备注内容不能为空 |

## 7. v1 → v2 迁移（首次加载懒迁移）

1. `data.json` 有 `settings` 对象 → 按 v2 读；否则判为 v1。
2. 迁移前把原件复制为 `data.v1.json`（**永不删除、已存在不覆盖**）。
3. 字段 snake_case → camelCase（`due_date`→`dueAt`、`created_at`→`createdAt`、`description`→`note`、`subtasks[].completed`→`done`、`notes[].created_at`→`createdAt`）。
4. `"2026-08-29 18:00"` 空格制式 → `2026-08-29T18:00`；带秒/时区后缀只做截断不换算；无法解析 → `null` 并记 `issues[]` + `invalidDates++`。
5. `completed:true` 且无完成时刻 → `doneAt=null` + `legacy=true`，`legacyDone++`。
6. 分类「个人」→「生活」；优先级 `medium`→`med`（都记进 issues）。
7. 逾期未完成 → 一次性补 `plannedDate = 今天`、`carriedFrom = 拖留天数`。
8. v1 `reminders[].time` 的 `09:00` / `10:00/14:00/16:00` 原样保留为循环时刻；`completed` 翻成 `enabled=!completed`。
9. v1 顶层 `theme` 字符串 → v2 `settings.theme = "float"`（主题语义变更，旧值不解读）。
10. 报告存在 `data.migration`，前端展示后调 `clear_migration_report` 清账。迁移异常（tasks 不是列表 / 原件留存失败）→ 一并进 corrupt 通道。

## 8. 前端对接要点

- 启动流程：`get_bootstrap` → 若 `health.health !== 'ok'` 先渲染恢复页（列 `backups` 的 `createdAt`/`taskCount`/`readable`），选 `slot` 调 `restore_backup`，成功后再 `get_bootstrap`。
- 三处「今天」计数用同一 selector：`plannedDate === 今天` ∪ `dueAt` 前缀是今天（后端已保证 `carriedFrom` 与 date-only 语义，不再自算第二套口径）。
- 软删 UI：`delete_task` 返回的对象里有 `deletedAt`，10s 内 `undo_delete`；`purgeExpired` 由后端在启动时跑（30 天）。
- 捕获条落库：`add_task{ args: { title, source: 'capture', dueAt?, remindAt?, category?, priority? } }`，失败提示「没有保存 · 重试」并保留输入内容。
- 迷你条只渲染两行信息（今日剩 N · 下一条 HH:MM），不要再放装饰性计时卡。

---

## 9. 落地差异与工程裁定（相对任务契约原文）

1. `Cargo.toml`：新增 `chrono = { features = ["clock","std"] }`；同时必须给 `tauri` 追加 `system-tray` feature —— `tauri` 1.0/1.8 的 `api-all` **不包含** system-tray（`Builder::system_tray`、`SystemTrayMenu`、`tray_handle_by_id` 都在该 feature 后面），不加则托盘代码无法编译。依赖版本 `tauri = "1.0.0"` 未动。
2. 托盘 id 用代码里的 `SystemTray::new().with_id("main")`，因为 v1.8 的 `tauri-utils` 配置结构 `SystemTrayConfig` 是 `deny_unknown_fields` 且只有 `iconPath`/`iconAsTemplate`/`menuOnLeftClick`/`title`（没有 `id`，也没有 `trayIcon` 键）。故 `tauri.conf.json` 不写托盘配置，写 `trayIcon` 会让构建期配置解析直接失败。
3. 捕获条资源：前端目录只有 `index.html` / `float.html` / `capture.html`（本次不允许改前端），所以 `capture` 窗在 `tauri.conf.json` 里按契约声明（560×112 / 无边框 / **不透明纸片卡**：原透明设计在部分 WebView2 上被合成黑边，v2.0.0 改为整窗 `--card` 底色 / 置顶 / skipTaskbar / visible:false / `capture.html`），但后端在窗口缺失或资源未就绪时不硬创建，走上面的 `open-capture` 降级事件；等前端补上 `capture.html` 后无需改后端即自动生效。
4. 提醒落库去重新增一个内部字段 `AppData.fired: string[]`（稳定键，上限 400 条）——不加它就无法区分「应用没运行期间错过的」和「已经响过的」，会出现开机重复轰炸。前端只读不写。
5. `Reminder.legacy`/`Task.legacy` 落盘为 `true` 时写出、`false` 时省略（`skip_serializing_if`），所以 `types.ts` 里的 `legacy?: boolean` 仍成立。
6. `set_settings` 改 `captureHotkey` 失败时整次回滚（主题/形态一起回），保持「界面显示的热键 = 真实绑定的热键」。
7. 系统通知 title 固定「待办提醒」，body 为 `<标题> · <YYYY-MM-DDTHH:mm>`；免打扰合并为「免打扰期间有 N 条提醒」，启动补发为「错过 N 条提醒」。深链 (`?open=`) 由前端在窗口内处理，通知不带 payload（契约允许省略）。

## 10. 验证状态（如实声明）

- 语法闸：`rustfmt 1.8.0 --edition 2021 --emit stdout` 对 7 个源文件 **stderr 零 error**（rustfmt 用真 rustc 解析器，即语法合法）。
- 自查：命令清单与本文档表格 38↔38 一致；无 `unwrap_or_default()` 式空库回退（解析失败一律 corrupt）；无 `toISOString` / `Utc` / `to_utc` / `naive_utc` 混入；`tauri.conf.json` 通过 `json.load`。
- **未做**：`cargo build` / `cargo check` / `clippy` / `tauri dev`。本沙箱无 webkit2gtk-4.0 系统库（Debian trixie 只提供 4.1，tauri v1 的 `-sys` 构建脚本必定失败），因此类型检查/链接需在 Windows 实机（或装有 webkit2gtk-4.0-dev 的 Linux）执行；本文档中的 API 名与签名是逐条对照本机 `tauri-1.8.3`/`tauri-runtime-0.14.6`/`tauri-utils-1.6.2` 源码核验的，但仍可能有编译期才能暴露的问题。

> 工程规格：docs/PRD-v2.md（§6 AC / §6.9 状态矩阵 / §8 三波发布与 dogfood Gate）

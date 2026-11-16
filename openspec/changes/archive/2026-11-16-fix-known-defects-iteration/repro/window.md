# 建窗失败 / WebView2 用户数据目录受限复现证据（#17）

> 只读调查产物：未修改项目源码、未执行 git 写命令、未启动应用、未运行 build。唯一写入文件为本文件。系统目录探测全部为只读命令（`Test-Path` / `Get-Item` / `icacls` / `Get-ChildItem` / `Get-Process` / `Get-WinEvent` / 注册表读取），**未在 `%LOCALAPPDATA%`、`%APPDATA%` 创建、修改或删除任何文件/进程**。文中「推断」= 无法在本次只读环境验证、需真机确认的结论。

## 环境

| 项 | 值 |
| --- | --- |
| 项目根 | `C:\Users\simy\project\mywork\kettd` |
| 分支 / HEAD | master @ `2167a48`（完整 `2167a48c238bb1a86c88cd33dd79157809ca3eea`，2026-11-13 22:38:47 +0800，msg「20261113-1438-c285-处理-createwebview-错误并指定可写用户数据目录：记进展」） |
| 日期 | 2026-11-13 23:06（Asia/Shanghai，China Standard Time） |
| 系统 | Windows，注册表 `ProductName=Windows 10 Home China`、`DisplayVersion=26H2`、Build `26300.9974`（`[Environment]::OSVersion`=10.0.26300.0） |
| 用户 | `RAVEN\simy`（本机探测以该账户执行） |
| 工作区 | 未提交：`src-react/src/rendering/styles/prosemirror.css`、`src-react/src/rendering/tiptap-editor.tsx`、`src-tauri/Cargo.toml`、`src-tauri/src/kb.rs`（均与本缺陷无关）+ `openspec/` 未跟踪。本文件引用的源码文件 `git status` 干净，行号即 HEAD `2167a48` 行号。 |
| Tauri | `tauri 1.8.3`、`tauri-runtime 0.14.6`、`tauri-runtime-wry 0.14.11`、`tauri-utils 1.6.2`、`wry 0.24.12`、`webview2-com 0.19.1`、`dirs-next 2.0.0`（`src-tauri/Cargo.lock`；`src-tauri/Cargo.toml` 写 `tauri = "1.0.0"` 为 semver 范围，实际锁定 1.8.3） |
| WebView2 Runtime | `157.0.4341.46`（HKLM `…\EdgeUpdate\Clients\{F3017226-…}`，目录 `C:\Program Files (x86)\Microsoft\EdgeWebView\Application\157.0.4341.46`） |
| 应用 | 安装版 2.2.0：`C:\Users\simy\AppData\Local\Kettd\Kettd.exe`（FileVersion 2.2.0；MSI 安装记录 2026-11-13 22:23:27）；另有 debug 构建 `src-tauri\target\debug\kettd.exe`（2026-11-13 22:34:29） |
| 取证方式 | 逐行阅读 + 只读全文检索 + 只读 PowerShell 目录/ACL/进程/事件日志探测；依赖源码在 `C:\Users\simy\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\` 下核对 |
| 未执行 | 未启动应用、未 `cargo test` / `cargo build`、未构造受限目录 → 运行期/视觉结论一律进「需真机确认点」 |

---

## #17 建窗失败 / 数据目录受限

### 现象（用户可见）

1. **启动阶段建窗失败 = 静默启动失败**：`main` 与 `capture` 两个窗口由 `src-tauri/tauri.conf.json:68-100` 声明，Tauri 在进入 `setup` **之前**创建（`tauri-1.8.3/src/app.rs:1750-1758`）。任一亮窗创建失败 → `.build()` 返回 Err → `src-tauri/src/main.rs:321-322` 的 `.expect("待办列表启动失败，请检查数据目录权限后重试")` panic。release 构建带 `windows_subsystem = "windows"`（`src-tauri/src/main.rs:11-14`），**没有控制台**，用户看不到错误文本；进程随即退出，表现为「双击后无反应 / 一闪而过」，任务栏无窗口、托盘不常驻、热键无响应（此为代码路径推导出的可见后果，本次未启动应用实测，标**推断**）。
2. **运行期动态建窗失败 = 中文提示 + 埋点**：新建便签（`sticky_window_error`）、待办悬浮窗（`todo_window_error`）、快速记录条（`create_capture_window` 自定义文案）失败时，命令返回 `窗口创建失败：…` 一类字符串，前端以 `inline-err` 角色展示（`src-react/src/views/SettingsView.tsx:154`、`src-react/src/views/KbView.tsx:159-160`）。启动时恢复便签窗失败会被 `let _ =` 吞掉（`src-tauri/src/main.rs:256`）→ 便签静默缺失，只在 `events.jsonl` 留痕。
3. **用户报的「CreateWebview 错误」**：这是 `tauri_runtime::Error::CreateWebview` 的 Debug 枚举名（`tauri-runtime-0.14.6/src/lib.rs:222-224`），代表 **WebView2 webview 创建失败**，不是 JS 命令 `window > create` 的业务报错（见下方线索核实）。完整 panic 文本形如：
   `待办列表启动失败，请检查数据目录权限后重试: Runtime(CreateWebview(WebView2Error(WindowsError(Error { code: HRESULT(0x…), message: "…" }))))`（外层三层格式由源码证实；HRESULT/message 具体值需真机获取）。

### 已知线索逐条核实

| 线索 | 核实结果 | 证据（真实 file:line） |
| --- | --- | --- |
| Tauri v1.8.3 在 Windows 强制给每个窗口设 `data_directory`，来源为 `resolve_path(BaseDirectory::LocalData)` | **属实**（仅当用户/配置未指定时；Windows/Linux 生效） | `tauri-1.8.3/src/manager.rs:1059-1073`：`:1062` `if pending.webview_attributes.data_directory.is_none()`，`:1063-1069` `resolve_path(..., &config.tauri.bundle.identifier, Some(BaseDirectory::LocalData))`，`:1070-1071` 成功则设为 `Some(user_data_dir)` |
| `create_dir_all` 失败即建窗失败 | **基本属实、需精确**：仅「目录不存在」时才调用；失败经 `?` 上抛，等价建窗失败。**但**目录已存在（或被同名文件占位）时不走这里，失败发生在后续 WebView2 初始化 | `tauri-1.8.3/src/manager.rs:1075-1080`（注释 `make sure the directory is created and available to prevent a panic`；`:1076` `if let Some(user_data_dir)`；`:1078` `create_dir_all(user_data_dir)?`） |
| 本应用 identifier=`com.todo.list` ⇒ 默认目录 `%LOCALAPPDATA%\com.todo.list` | **属实** | `src-tauri/tauri.conf.json:45`；`tauri-1.8.3/src/api/path.rs:279`（`BaseDirectory::LocalData => local_data_dir()`）、`:318`（`base_dir_path_value.push(path)`，path=identifier）；`dirs-next-2.0.0/src/win.rs:5`（`data_local_dir` = `known_folder_local_app_data` = `FOLDERID_LocalAppData`）；组合点 `manager.rs:1067-1068` |
| wry 0.24.12 在 `data_directory` 为 `Some` 时显式传 UserDataFolder，覆盖 `WEBVIEW2_USER_DATA_FOLDER` 环境变量 | **显式传参属实**；「覆盖 env var」为 WebView2 API 语义层面的**推断** | `wry-0.24.12/src/webview/webview2/mod.rs:99-103`（从 `web_context.data_directory()` 取目录）、`:135-149`（`:135` `if let Some(data_directory)`，`:136-139` `CreateCoreWebView2EnvironmentWithOptions(…, PCWSTR::from_raw(encode_wide(data_directory)…), …)`；`:143-148` 无目录时传 `PCWSTR::null()`）；本机 `WEBVIEW2_USER_DATA_FOLDER` User/Machine 均未设置 |
| `WindowBuilder::data_directory` 存在；`WindowConfig` 无 `dataDirectory` 字段 | **属实** | `tauri-1.8.3/src/window.rs:713-719`；`tauri-utils-1.6.2/src/config.rs:1110`（`pub struct WindowConfig`，`rename_all="camelCase"` + `deny_unknown_fields`，字段中无 dataDirectory） |
| 主窗由 `tauri.conf.json:68-80` 声明（label=main）；便签/待办/捕获窗由 `runtime.rs:123/311/422` builder 创建 | **属实**（窗口对象实际 `:69-82`，label `:70`、visible `:79`；`capture` 窗在 `:83-100` 由配置声明；runtime 三处是运行期动态建窗） | `src-tauri/tauri.conf.json:68-100`；`src-tauri/src/runtime.rs:123`（`note_builder`）、`:311`（`todo_builder`）、`:422`（`create_capture_window` 内 builder）。三处均**未**调用 `.data_directory(...)`（全仓 `src-tauri/src` 检索 `data_directory` 0 命中） |
| `CreateWebview` 报错来源 `endpoints/window.rs:215/224`；前端 grep 不到 `createWebview` 调用 ⇒ 报错可能来自窗口创建内部 | **前半不成立、后半成立**：`endpoints/window.rs` 是 JS `window > create` 命令；panic 里的 `CreateWebview` 是 **runtime 错误类型**，由 tauri-runtime-wry 在真正建 WebView2 时包装产生 ⇒ 报错来自内部建窗 | `tauri-1.8.3/src/endpoints/window.rs:215`（`CreateWebview { options }`）、`:224`（`async fn create_webview`）；`tauri-runtime-wry-0.14.11/src/lib.rs:3304`、`:3400`（`.map_err(|e| Error::CreateWebview(Box::new(e)))?`）；`tauri-runtime-0.14.6/src/lib.rs:222-224`；前端检索 `createWebview|WebviewWindow` 0 命中 |

### 证据

#### E1 默认 WebView2 用户数据目录的完整链路

```
tauri.conf.json:45  identifier = com.todo.list
  → manager.rs:1059-1073  Windows 下若未指定 data_directory，用 identifier 解析 LocalData
  → path.rs:279/318        %LOCALAPPDATA% + "\" + identifier   ⇒ %LOCALAPPDATA%\com.todo.list
  → manager.rs:1075-1080   不存在则 create_dir_all；失败 `?` 上抛
  → runtime-wry lib.rs:3378  WebContext::new(webview_attributes.data_directory)
  → wry webview2/mod.rs:99-103 / 135-149  作为 UserDataFolder 传给 WebView2
```

- 本机佐证：`C:\Users\simy\AppData\Local\com.todo.list` 的 `CreationTime=2026/11/8 22:47:43`，比 `events.jsonl` 首条 `app_launch`（`2026-11-08T22:47:45`）早 2 秒 → **推断**：该目录正是首次启动建窗时由 `manager.rs:1078` 的 `create_dir_all` 创建（时间上高度吻合）。
- 这是 WebView2 浏览器 profile 目录，**不是应用数据目录**；应用真实数据在 `%APPDATA%\todo-list`（`src-tauri/src/infra/fs_store.rs:17-21`，`dirs::data_dir().join("todo-list")`）。
- 另一条**推断**边界：`manager.rs:1070` 只有 `resolve_path` 返回 `Ok` 才设 `Some`；若极端环境下 LocalData 解析失败，`data_directory` 保持 `None`，wry 走 `PCWSTR::null()`（`wry mod.rs:143-148`）→ WebView2 使用默认目录（可执行文件旁 `<exe>.WebView2`），若安装目录不可写同样会 `CreateWebview` 失败。本机未复现，仅记录为待验证边界。

#### E2 建窗失败的两条错误链（决定用户看到什么）

**A 型：`create_dir_all` 失败 → `tauri::Error::Io`**

```
manager.rs:1078  create_dir_all(user_data_dir)?   → io::Error
  → tauri-1.8.3/src/error.rs:63   Io(#[from] std::io::Error)
  → main.rs:322 panic："待办列表启动失败，请检查数据目录权限后重试: Io(Os { code: 5, kind: PermissionDenied, message: "..." })"
```

（`Io(Os { … })` 的具体 Debug 文本为**推断**，由 std `io::Error` Debug 格式决定；常见 `code: 5` = Access denied、`code: 112` = 磁盘满。）

**B 型：目录存在（或已建成）但 WebView2 初始化失败 → `CreateWebview`**

```
wry webview2/mod.rs:152-153  error_code?（WindowsError）
  → wry-0.24.12/src/lib.rs:159-161     Error::WebView2Error(#[error("WebView2 error: {0}")])
  → tauri-runtime-wry lib.rs:3400      Error::CreateWebview(Box::new(e))
  → tauri-runtime 0.14.6 lib.rs:222-224 CreateWebview(#[error("failed to create webview: {0}")])
  → tauri-1.8.3/src/error.rs:36        Runtime(#[from] tauri_runtime::Error)
  → main.rs:322 panic Debug：Runtime(CreateWebview(WebView2Error(WindowsError(...))))
```

- 典型触发（**推断**，需真机）：`%LOCALAPPDATA%\com.todo.list` 被**同名文件**占位（`exists()==true` 跳过 `create_dir_all`，WebView2 把文件当 UserDataFolder 用）；目录 ACL 拒写；`EBWebView` profile 被安全软件/残留进程锁定；WebView2 Runtime 缺失；磁盘满。
- 两处 `CreateWebview` 包装点：`tauri-runtime-wry-0.14.11/src/lib.rs:3304`（`WebViewBuilder::new`）与 `:3400`（`with_web_context(...).build()`）。**data_directory 相关失败发生在 `build()`（创建 WebView2 环境）阶段 ⇒ 对应 `:3400`**（**推断**：由 wry `create_environment` 被 `build` 调用决定）。
- `webview2-com-0.19.1/src/lib.rs:26-35`：`pub enum Error { WindowsError(windows::core::Error), … }`，其 `Display` 直接 `write!(f, "{:?}", self)` ⇒ 文案里是 `WindowsError(Error { code: HRESULT(0x…), message: "…" })` 的 Debug 形式，**不是**友好提示。

**为什么启动失败在 `events.jsonl` 里查不到**

- `telemetry::init` 在 `setup` 内调用（`src-tauri/src/main.rs:168`），而 config 窗口创建在 `app.rs:1750-1754`，`setup` 在 `app.rs:1758` 才执行 ⇒ 启动建窗失败时 telemetry 从未初始化。
- `telemetry::record` 在 `SINK` 未初始化时直接返回（`src-tauri/src/telemetry.rs:51-55`：`let Some(Some(tx)) = SINK.get() else { return };`）⇒ 启动失败**不会**留下任何 `events.jsonl` 记录，只能靠终端 panic / 系统侧证据。

**运行期动态建窗（便签/待办/捕获）的错误路径**

- 统一超时包装 `src-tauri/src/runtime.rs:140-188`：主线程内联执行；其它线程丢到 `win-build:{label}` helper 线程，`recv_timeout(6s)` 超时则记 `err=timeout_6s` + `loop_alive`，返回「窗口创建超时，请重试；若连续出现请重启应用」（`:185`）。
- 失败统一埋点 `src-tauri/src/runtime.rs:191-194` `log_build_error` → `telemetry::record_str(event, [label, err])` 并返回 `format!("窗口创建失败：{error}")`（`:193`）。
- 便签：`runtime.rs:197-203`（`build_with_timeout(..., "sticky_window_error")`，`:198`；`:201` 失败埋点）。新建便签命令在失败时会回滚已落盘的便签（`create_note`，`runtime.rs:234` 起；`:275-283`）。前端展示：`SettingsView.tsx:90-94` 取 `r.err` → `:154` `inline-err`。
- 待办悬浮：`runtime.rs:356-360`（`todo_window_error`）。
- 快速记录条：`runtime.rs:420-436`（失败文案「快速记录条创建失败，请用悬浮面板底部输入框」，`:436`）；`open_capture_overlay`（`:440`）在重建失败时降级 `emit_all("open-capture")` + 打开主窗。
- 启动恢复便签/待办悬浮窗失败被 `let _ =` 吞掉（`main.rs:256` 便签；`main.rs:259` `apply_todo_float_pinned`、`:261` `open_todo_float` 同类）→ 用户只感知「便签/悬浮窗没出现」。

#### E3 本机只读探测结果（目录存在且可写 ⇒ 本机无法自然复现）

探测时间 2026-11-13 23:05–23:06（`RAVEN\simy`，只读）：

| 探测 | 输出（原文要点） |
| --- | --- |
| `%LOCALAPPDATA%` / `%APPDATA%` | `C:\Users\simy\AppData\Local` / `C:\Users\simy\AppData\Roaming` |
| `Test-Path "$env:LOCALAPPDATA\com.todo.list"` | `True` |
| `Get-Item …\com.todo.list` | `Attributes=Directory`、`CreationTime=2026/11/8 22:47:43`、`LastWriteTime=2026/11/8 22:47:43`、`LinkType=`（空，非重解析点） |
| `icacls "$env:LOCALAPPDATA\com.todo.list"` | `NT AUTHORITY\SYSTEM:(OI)(CI)(F)`、`BUILTIN\Administrators:(OI)(CI)(F)`、`RAVEN\simy:(OI)(CI)(F)` |
| `icacls "$env:LOCALAPPDATA"` | 同上三组 `(OI)(CI)(F)` |
| `icacls "$env:APPDATA\todo-list"` | 同上三组 `(OI)(CI)(F)` |
| `com.todo.list` 内容 | 仅 `EBWebView\` 子目录；`EBWebView` 的 `LastWriteTime=2026/11/13 23:05:28`（当时仍在被写） |
| `%APPDATA%\todo-list` 内容 | `data.json` 1329B、`data.json.bak-1..5`、`events.jsonl` 3528B、`notes.json` 2724B、`runtime.json` 103B |
| `events.jsonl` 统计 | 8 条 `app_launch`（最早 2026-11-08T22:47:45，version 2.1.0）+ `sticky_create` 11 / `sticky_delete` 11 / `sticky_md_toggle` 4 / `sticky_free_edit` 4 / `kb_create` 3 / `capture_commit` 2；`window_error|window|webview` **0 命中** ⇒ 历史从未出现该缺陷 |
| `WEBVIEW2_USER_DATA_FOLDER` | User、Machine 均为空（未设置） |
| 事件查看器（Application，近 14 天） | 无 kettd 崩溃 / WER 记录，仅 MSI 安装日志（Kettd 2.2.0，2026-11-13 22:23:27、20:30:54，status 0） |
| 进程 | 取证时无 `kettd.exe`；存在 5 个 `msedgewebview2.exe`（StartTime 2026-11-13 22:35:18/19/19/27/27，应用 22:47 退出后仍**残留**——该现象属 #16，但 `Local State` 23:05:28 的写入同时证明**当前用户对该目录确实可写**） |

结论：**默认目录与真实数据目录均存在、ACL 可写、WebView2 今日仍在写入 ⇒ 本机不具备自然复现条件**；必须人造受限目录（见下）才能触发。

#### E4 排除项与修复面（供定位用，不含修复方案）

- 主窗/捕获窗的目录**无法**用 `tauri.conf.json` 指定：`WindowConfig` 无 `dataDirectory` 字段（`tauri-utils-1.6.2/src/config.rs:1110`）⇒ 主窗只能吃 Tauri 强制默认目录，或改用代码建窗；动态窗可用 `WindowBuilder::data_directory` 覆盖（`tauri-1.8.3/src/window.rs:713-719`），本应用三处 builder（`runtime.rs:123/311/422`）均未使用。
- `WEBVIEW2_USER_DATA_FOLDER` 单独设置**不可靠**：Tauri v1 Windows 会强制 `data_directory=Some(…)`（`manager.rs:1062-1071`），wry 会显式传 UserDataFolder（`wry mod.rs:135-149`），按 WebView2 API 语义显式参数优先于环境变量（**推断**，需真机验证设置后是否仍失败）。
- 前端不参与建窗：`src-react/src` 检索 `createWebview|WebviewWindow|WEBVIEW2|data_directory|dataDirectory` 0 命中；`mainWindowBridge` 只暴露 `minimize/toggleMaximize/close/isMaximized/onResized`（`src-react/src/lib/api.ts:226-256`，无 `show`）；`openMainWindow`（`api.ts:353`）仅 `TodoFloatWindow.tsx:59` 使用。⇒ panic 中的 `CreateWebview` 只能来自 Rust 内部建窗。
- 托盘优先设计的正常基准：`main` 窗 `visible:false`（`tauri.conf.json:79`）是**设计如此**（README：关闭主窗不退出、托盘常驻、Alt+Shift+O 唤出主窗）——判断启动失败不能只看「主窗有没有弹出」，要看**托盘图标是否常驻、热键是否响应**（**推断**：panic 前托盘已在 `app.rs:1720-1721` 创建，随即随进程退出销毁，用户可能只看到一闪或完全无感）。

### 复现步骤（真机；本任务约束下**未执行**，以下为待执行方案）

> 破坏性操作警告：以下会移动/改 ACL WebView2 profile 目录。**先备份** `%APPDATA%\todo-list` 与 `%LOCALAPPDATA%\com.todo.list`（整目录复制到别处），结束 `kettd.exe` 与残留 `msedgewebview2.exe`，在**测试 Windows 账户**操作；每步之后立即还原 ACL/目录，且一次只跑一个场景。`release` 版观察「无错误文本」的静默失败现象，`dev` 版（先 `npm run build:front` 再 `npm run tauri dev`，或直接运行 `src-tauri\target\debug\kettd.exe`）在终端看完整 panic。

**准备（还原基线）**

```powershell
# 只读确认基线（勿在正常账户上执行下述破坏性步骤）
Test-Path "$env:LOCALAPPDATA\com.todo.list"                 # 期望 True
Get-Process kettd,msedgewebview2 -ErrorAction SilentlyContinue | Stop-Process  # 仅测试环境
# 备份
Copy-Item "$env:APPDATA\todo-list"        "$env:APPDATA\todo-list.bak-repro" -Recurse
Copy-Item "$env:LOCALAPPDATA\com.todo.list" "$env:LOCALAPPDATA\com.todo.list.bak-repro" -Recurse
```

**场景 1（推荐，最易还原）：同名文件占位 → 期望 B 型 `CreateWebview`**

```powershell
Stop-Process -Name kettd,msedgewebview2 -ErrorAction SilentlyContinue
Move-Item "$env:LOCALAPPDATA\com.todo.list" "$env:LOCALAPPDATA\com.todo.list.bak-repro-moved"
New-Item -ItemType File "$env:LOCALAPPDATA\com.todo.list" | Out-Null
# 启动 dev 版：期望终端 panic 含 Runtime(CreateWebview(WebView2Error(WindowsError(...))))
# 启动 release 版：期望双击无窗口、进程随即退出
#   验证：Get-Process kettd  # 进程不存在
# 还原：
Remove-Item "$env:LOCALAPPDATA\com.todo.list"
Move-Item "$env:LOCALAPPDATA\com.todo.list.bak-repro-moved" "$env:LOCALAPPDATA\com.todo.list"
```

（`exists()==true` ⇒ `manager.rs:1076-1078` 跳过 `create_dir_all`，失败点落在 WebView2 初始化，正是用户报错形态；**推断** WebView2 对「UserDataFolder 是文件」返回错误 HRESULT。）

**场景 2：已有目录 ACL 拒写 → 期望 B 型 `CreateWebview`**

```powershell
icacls "$env:LOCALAPPDATA\com.todo.list" /deny "RAVEN\simy:(OI)(CI)(W)"
# 启动 dev 版，观察 panic；期望 Runtime(CreateWebview(WebView2Error(WindowsError(...))))（推断）
icacls "$env:LOCALAPPDATA\com.todo.list" /remove:d "RAVEN\simy"
```

**场景 3：父目录拒建 → 期望 A 型 `Io(PermissionDenied)`**（仅测试账户）

```powershell
Move-Item "$env:LOCALAPPDATA\com.todo.list" "$env:LOCALAPPDATA\com.todo.list.bak-repro-moved"
icacls "$env:LOCALAPPDATA" /deny "$($env:USERNAME):(AD)"     # 拒绝创建子目录；立即还原
# 启动 dev 版：期望 panic 含 Io(Os { code: 5, kind: PermissionDenied, ... })（推断）
icacls "$env:LOCALAPPDATA" /remove:d "$($env:USERNAME)"
Move-Item "$env:LOCALAPPDATA\com.todo.list.bak-repro-moved" "$env:LOCALAPPDATA\com.todo.list"
```

**场景 4：运行期动态建窗 → 期望前端中文错误 + `events.jsonl`**

```powershell
# 正常启动应用后，对目录加拒写，再点「设置 → 新建便签」
icacls "$env:LOCALAPPDATA\com.todo.list" /deny "RAVEN\simy:(OI)(CI)(W)"
# 期望：界面出现 inline 错误「窗口创建失败：…」；%APPDATA%\todo-list\events.jsonl 新增 sticky_window_error（label/err）
# 注：WebView2 环境可能已被既有 webview 复用而不失败，本场景是否有效需实测（推断）
icacls "$env:LOCALAPPDATA\com.todo.list" /remove:d "RAVEN\simy"
```

**场景 5：真实受限环境**（不必造目录 ACL，直接观察）：`%LOCALAPPDATA%` 被组策略/文件夹重定向到只读网络盘或 OneDrive 占位文件、磁盘写满、`EBWebView` 被安全软件锁、WebView2 Runtime 缺失/损坏。采集：终端 panic 全文、`Get-Process kettd`、`%LOCALAPPDATA%\com.todo.list` ACL、`events.jsonl`（仅运行期场景）。

### 是否可无头验证

**否（不能无头复现）。**

- 失败点在 `tauri::Builder::build()`，位于 `setup` 之前、且在真实 WebView2/窗口/托盘环境里；本任务约束又禁止启动应用与改动数据目录，无法无头构造。
- `cargo test` 无法覆盖：需要真实 runtime 与 OS 权限才能让 `create_window`/`CreateCoreWebView2EnvironmentWithOptions` 失败；仓库现有单测不触碰建窗（`src-tauri/src/runtime.rs` 无测试覆盖该路径）。
- 可无头做的部分（本文已完成）：静态核对全部 file:line、只读探测本机目录/ACL/事件日志，以及核对依赖源码版本。**能证明代码因果链与默认目录来源，不能证明用户可见行为与真实 HRESULT。**
- 复核方式（任意机器，只读）：按上表 file:line 用 `read`/grep 核对；依赖源码路径为 `C:\Users\simy\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\`（版本见「环境」），registry 版本变化后行号会漂移。

### 需真机确认点

1. **panic 原文**：受限目录下到底是 `Io(Os {…})`（A 型）还是 `Runtime(CreateWebview(WebView2Error(WindowsError(Error { code: HRESULT(0x…), message: "…" }))))`（B 型），以及真实 HRESULT/message（决定 `events`/日志该记什么）。
2. **release 用户体验**：双击后进程是否立即退出（`Get-Process kettd`）、是否出现 WER 崩溃框、托盘图标是否可见/一闪；对照正常基线（托盘常驻、Alt+Shift+O 唤出主窗）。
3. **场景 4 是否真的失败**：已有 WebView2 进程时，新建便签是否复用环境而**不**触发错误；若失败，确认前端文案与 `events.jsonl` 的 `sticky_window_error` 字段。
4. **`WEBVIEW2_USER_DATA_FOLDER` 兜底是否无效**：设置该环境变量后启动，仍失败 ⇒ 确认覆盖结论；若反而正常 ⇒ 结论需修正（可能受 WebView2 版本/策略影响）。
5. **修复回归**：受限默认目录下按 spec 回退到可写目录（应用数据目录/临时目录）后，主窗/托盘是否至少可用；错误提示是否含原因与尝试过的目录（`openspec/changes/fix-known-defects-iteration/specs/window-management/spec.md`）。
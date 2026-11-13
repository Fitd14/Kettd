# 20261113-1438-c285-处理-createwebview-错误并指定可写用户数据目录: 处理 CreateWebview 错误并指定可写用户数据目录

- **Tags**: WebView2, 建窗, 数据目录, 错误处理
- **Status**: doing · 2026-11-13 16:46:13 · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd
- **Created**: 2026-11-13 14:38:13 · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd

<!-- Header lines are maintained by the script, fixed order: Tags → Status → Created → Sealed.
     The `- **Tags**: …` line is inserted at the top by new --tags; omitted when there are no tags
     (category goes to the ledger and history only, never into this document).
     The `- **Sealed**: …` line is appended on close. -->

## Background

2026-11-13 用户要求按转述建议改进建窗健壮性：**处理 CreateWebview 的错误**，或**在创建 WebView 时显式指定一个可写的、基于临时目录的用户数据文件夹**，避免默认可能受限的目录；对 Tauri 应用也可在启动时检查并设置 `WEBVIEW2_USER_DATA_FOLDER` 指向确定可写的路径。

目标：建窗失败不再静默/崩溃，WebView2 用户数据目录可控、可写、可预期。

2026-11-14 00:18 用户复报：`npm run tauri dev` 启动即 panic（main.rs:331，`CreateWebview ... 0x800700AA 请求的资源在使用中`），要求查明原因。本轮把它定位到「WebView2 用户数据目录不可用」，并证实当前代码既没有探测/回退，也没有把错误可见化。

## To Do

- [x] 1. 真机报错已取到：`0x800700AA`（ERROR_BUSY）来自 `CreateCoreWebView2EnvironmentWithOptions`，被 main.rs:330 的 `.expect` 打成 panic——与「数据目录权限」文案无关。
- [x] 2. 默认目录确认：`%LOCALAPPDATA%\com.todo.list`（manager.rs:1059-1080），本机目录存在、账户可写、profile 本体健康。
- [x] 3. 显式 `data_directory` 的落点已确认：`note_builder`（runtime.rs:123）、`todo_builder`（:311）、capture 建窗（:422）；主窗在 tauri.conf.json:68-100，WindowConfig 无该字段 → 必须改代码建。
- [ ] 4. 落地探测 + 回退链：`%APPDATA%\todo-list\webview2` → `%LOCALAPPDATA%\com.todo.list` → `%TEMP%\kettd-webview2`（写探针文件判定），三处建窗与主窗统一传 `.data_directory()`。
- [ ] 5. 主窗从 conf 移出、在 `setup` 里建（失败可 catch）；`.expect` 换成用户可见错误，telemetry 提到建窗之前。
- [ ] 6. 验证：受限目录下启动、便签/待办/捕获窗建窗、cargo check + cargo test（#16 进程残留另案）。

## Next

先改 `src-tauri/src/main.rs`：在 `store::Store::load()` 之前加 `resolve_webview2_dir()`（按 To Do 4 的回退链写探针文件判定可写），目录透传给建窗；再把 tauri.conf.json:68-100 的 `main` 窗挪进 `setup`，用 `WindowBuilder::new(app, "main", WindowUrl::App("index.html".into())).data_directory(dir)` 建；`runtime.rs:123`（note_builder）、`:311`（todo_builder）、`:422`（capture）同步传同一目录（多窗必须同目录，否则 WebView2 进程/缓存分裂）。当前工作区有别人的未提交改动，只动启动/建窗相关行。

## Process

- 仓内当前无任何 `WEBVIEW2_USER_DATA_FOLDER` / `user_data` / `dataDirectory` 配置（全仓 grep 0 命中）。
- 版本：Cargo.toml `tauri = "1.0.0"` 解析到 1.8.3；tauri-runtime-wry 0.14.11；wry 0.24.12。
- 失败点：Tauri 强制 data_directory 后 `create_dir_all` 失败会直接返回错误（manager.rs:1076-1080）——正是「默认目录受限」会炸的位置。
- 多窗口共用同一 user data folder（wry WebContext）；各窗口不要给不同目录，否则 WebView2 进程与缓存分裂。
- 与 #16（退出后 msedgewebview2.exe 残留）相关但独立：数据目录修复不自动解决进程残留。

### 2026-11-14 实测（00:18–00:45，本机）

- 失败在建窗、`setup` 之前：`%APPDATA%\todo-list\events.jsonl` 停在 11-13 22:35 的第 43 行（无新 `app_launch`）；与 `tauri.conf.json:68-100` 的 `main`/`capture` 在 `Builder::build()` 内创建一致（tauri-1.8.3/src/app.rs:1750-1758）。
- 抛点：wry 0.24.12 `webview2/mod.rs:135-163`（`CreateCoreWebView2EnvironmentWithOptions` → `webview2_com::Error::WindowsError`）→ `CreateWebview` → main.rs:330 的 `.expect`。
- UDF = `%LOCALAPPDATA%\com.todo.list\EBWebView`：74 MB、`Last Version` 157.0.4341.46 与已装 Runtime 一致；目录 SYSTEM/Administrators/simy 全权；仅 leveldb/SessionStorage 下 9 个文件带显式 `Everyone:(DENY)(X)`（不影响打开写入）。
- 现场无占用者：无 Kettd 进程、无 `com.todo.list` 的 `msedgewebview2.exe`；一时看到的 6 个 webview 属 Windows 小组件（`MicrosoftWindows.Client.CBS_*` 的 EBWebView，杀掉即重生）。
- 对照：旧 release `%LOCALAPPDATA%\Kettd\Kettd.exe`（2.1.0，11-08）用同一 profile 正常启动，写出第 44 条 `app_launch`（00:37:30）并拉起 7 个 webview。
- 复现：在拿不到 `%LOCALAPPDATA%` 的受限会话里直接跑 `src-tauri\target\debug\Kettd.exe`，100% 复现同一条 panic（逐字一致）。
- 反证：`TAURI_AUTOMATION=true`（共用一个 WebContext，绕开每窗重复建环境）仍失败；profile 挪走 → `Io(Os 5)`；空目录 → `0x8000FFFF`。⇒ 既不是「环境重复创建」，也不是「profile 内容坏」。
- 判定：进程拿不到或用不了 UDF（受限上下文运行，或当时被别的 WebView2 占用）时必炸；代码缺探测、回退与可见化，才让一句误导的「检查数据目录权限」成为唯一线索。

## Conclusion

本轮只做定位、未改代码：根因链路已定（UDF 不可用 → `CreateCoreWebView2EnvironmentWithOptions` 返回 0x800700AA → `build()` 内建窗失败 → `.expect` panic，且崩在 telemetry 之前，日志空白），并排除了「profile 损坏」「残留进程占用」「每窗重复建环境」三种猜测；`repro/window.md` 里「本机无法自然复现」的结论需改为「受限上下文可稳定复现」。修复（可写探测 + 回退链 + 显式 data_directory + 错误可见化 + 主窗移出 conf）见 To Do 4-6，待确认后实施。

## Execution Log

<!-- Maintained by the script: appended by new / claim / set / release / log / close. Do not edit.
     Backfill with: progress log --id 20261113-1438-c285-处理-createwebview-错误并指定可写用户数据目录 --note "...". -->
- 2026-11-13 14:38:13 · new · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd · created
- 2026-11-13 14:38:20 · claim · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd · claimed → doing
- 2026-11-13 14:38:46 · log · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd · 认领并写入现状：Tauri 强制 data_directory（manager.rs:1059），主窗配置无该字段；env var 会被覆盖
- 2026-11-13 15:24:29 · log · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd · 复现核查：本机数据目录存在且可写、无失败事件；炸点链与启动失败无日志时序落 repro/window.md
- 2026-11-13 16:46:13 · log · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd · 定位完成：0x800700AA 出自 wry webview2/mod.rs:135-163 的 CreateCoreWebView2EnvironmentWithOptions，被 main.rs:330 的 .expect 打成 panic 且早于 telemetry；实测排除 profile 损坏/残留进程/每窗重复建环境（TAURI_AUTOMATION 共用上下文仍失败），受限上下文可 100% 复现；修复见 To Do 4-6

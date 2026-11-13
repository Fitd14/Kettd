# 20261113-1438-c285-处理-createwebview-错误并指定可写用户数据目录: 处理 CreateWebview 错误并指定可写用户数据目录

- **Tags**: WebView2, 建窗, 数据目录, 错误处理
- **Status**: doing · 2026-11-13 15:24:29 · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd
- **Created**: 2026-11-13 14:38:13 · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd

<!-- Header lines are maintained by the script, fixed order: Tags → Status → Created → Sealed.
     The `- **Tags**: …` line is inserted at the top by new --tags; omitted when there are no tags
     (category goes to the ledger and history only, never into this document).
     The `- **Sealed**: …` line is appended on close. -->

## Background

2026-11-13 用户要求按转述建议改进建窗健壮性：**处理 CreateWebview 的错误**，或**在创建 WebView 时显式指定一个可写的、基于临时目录的用户数据文件夹**，避免默认可能受限的目录；对 Tauri 应用也可在启动时检查并设置 `WEBVIEW2_USER_DATA_FOLDER` 指向确定可写的路径。

目标：建窗失败不再静默/崩溃，WebView2 用户数据目录可控、可写、可预期。

## To Do

- [ ] 1. 定位「CreateWebview」错误来源：tauri 1.8.3 中 `CreateWebview` 是 JS 侧 `window > create` 命令（tauri-1.8.3/src/endpoints/window.rs:215、:224），本项目前端未调用（grep `createWebview|WebviewWindow` 无命中）→ 需取真机报错文本与 events.jsonl，确认实际是启动建窗失败还是别处错误。
- [ ] 2. 确认默认目录是否受限：Tauri v1.8.3 在 Windows 给每个窗口强制 `data_directory = resolve_path(BaseDirectory::LocalData)`（manager.rs:1059-1073）并 `create_dir_all`（:1076-1080）；本应用 identifier=`com.todo.list`（tauri.conf.json:45）→ 默认 `%LOCALAPPDATA%\com.todo.list`；目录不可创建时建窗直接失败。
- [ ] 3. 显式指定可写目录：`WindowBuilder::data_directory(PathBuf)`（tauri-1.8.3/src/window.rs:713）用于 `note_builder`（runtime.rs:123-134）、`todo_builder`（:311）、capture 建窗（:422）；主窗由 tauri.conf.json `windows`（:68-80，label=main）创建，WindowConfig 无 dataDirectory 字段 → 主窗需改代码建或接受默认。目录建议 `%APPDATA%\todo-list\webview2` 或 `std::env::temp_dir()`，启动时确保存在且可写。
- [ ] 4. 评估 `WEBVIEW2_USER_DATA_FOLDER`：wry 0.24.12 在 data_directory 为 Some 时显式传 UserDataFolder（wry/src/webview/webview2/mod.rs:135-138），会覆盖环境变量；Tauri v1 窗口恒被强制设目录 ⇒ 单靠 env var 预期无效，结论倾向「以 data_directory 为主、env var 仅文档化兜底」。
- [ ] 5. 错误处理加固：建窗失败统一中文提示 + 一次性重试 + telemetry（已有 `build_with_timeout` 6s 超时与 `log_build_error` 格式「窗口创建失败：{error}」，runtime.rs:140-194）；补主窗/启动失败兜底。
- [ ] 6. 验证：cargo check/test；真机验证受限目录下启动、便签/待办/捕获窗建窗、退出后 WebView2 进程（进程残留归任务 #16，另案）。

## Next

先做 To Do 1+2：取实际报错文本与 `%LOCALAPPDATA%\com.todo.list` 的可写性结论（真机 + events.jsonl）。确认后改 `src-tauri/src/runtime.rs:123`（note_builder）、`:311`（todo_builder）、`:422`（capture）加 `.data_directory(...)` 指向 `%APPDATA%\todo-list\webview2`，并在 `main()` 最早期做目录创建/可写探测。

## Process

- 仓内当前无任何 `WEBVIEW2_USER_DATA_FOLDER` / `user_data` / `dataDirectory` 配置（全仓 grep 0 命中）。
- 版本：Cargo.toml `tauri = "1.0.0"` 解析到 1.8.3；tauri-runtime-wry 0.14.11；wry 0.24.12。
- 失败点：Tauri 强制 data_directory 后 `create_dir_all` 失败会直接返回错误（manager.rs:1076-1080）——正是「默认目录受限」会炸的位置。
- 多窗口共用同一 user data folder（wry WebContext）；各窗口不要给不同目录，否则 WebView2 进程与缓存分裂。
- 与 #16（退出后 msedgewebview2.exe 残留）相关但独立：数据目录修复不自动解决进程残留。

## Conclusion

<!-- What got done, what got left behind -->

## Execution Log

<!-- Maintained by the script: appended by new / claim / set / release / log / close. Do not edit.
     Backfill with: progress log --id 20261113-1438-c285-处理-createwebview-错误并指定可写用户数据目录 --note "...". -->
- 2026-11-13 14:38:13 · new · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd · created
- 2026-11-13 14:38:20 · claim · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd · claimed → doing
- 2026-11-13 14:38:46 · log · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd · 认领并写入现状：Tauri 强制 data_directory（manager.rs:1059），主窗配置无该字段；env var 会被覆盖
- 2026-11-13 15:24:29 · log · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd · 复现核查：本机数据目录存在且可写、无失败事件；炸点链与启动失败无日志时序落 repro/window.md

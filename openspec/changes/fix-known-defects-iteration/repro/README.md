# 缺陷复现总览（17 项）

> 复现基线：git HEAD `2167a48`（2026-11-13），工作区另有上批四缺陷修复的 3 个未提交文件（`src-react/src/rendering/tiptap-editor.tsx`、`src-react/src/rendering/styles/prosemirror.css`、`src-tauri/src/kb.rs`）。本目录所有证据均为只读取证，行号以当前工作区为准。
>
> 方法边界：**未启动 GUI、未构建、未修改项目源码**。编辑器类用无头 jsdom 复刻同扩展栈实测；其余为源码路径定位 + 只读磁盘结构核对。凡标 🔶 的结论由代码路径推导，需真机复核。

## 环境

| 项 | 值 |
|---|---|
| 项目 | Kettd（Tauri v1.8.3 + React 19 + Tiptap 3） |
| Tauri / wry | tauri 1.8.3、tauri-runtime-wry 0.14.11、wry 0.24.12 |
| WebView2 Runtime | 157.0.4341.46 |
| Node | v22.23.3 |
| 调查日期 | 2026-11-13（Asia/Shanghai） |

## 17 项总表

图例：✅ 已复现（静态/无头证据充分）｜🔶 静态定位成立，表现需真机确认｜⬜ 本机未能自然复现

| # | 需求 | 结论 | 关键证据 | 验证方式 |
|---|---|---|---|---|
| 1 | 保存时闪烁 | ✅ 保存刷新时 `loading` 空态与旧列表**同时渲染**，每次保存闪一次 | `KbView.tsx:48`、`:174`、`:182-189`；`kb-item-detail.tsx:61-68` | 无头快照复现；闪烁强度需真机 |
| 2 | 编辑不流畅 | ✅ 每键全组件重渲染 + 扩展每次 render 重建触发 `setOptions`；整篇序列化超线性（9.7KB 2.18ms → 99KB 219ms，`atBlank()` O(n²)） | `tiptap-editor.tsx:377-382`、useEditor 无 deps；`@tiptap/react` dist:396-411/447-463 | 无头机制；帧率/IPC 耗时需真机 |
| 3 | 右键有开发者工具 | ✅ 仓库无自定义 contextmenu；wry 默认开启右键菜单；devtools 由 debug/feature 决定 | wry `webview2/mod.rs:246`；tauri-runtime-wry `lib.rs:3392-3395`；`src-tauri/Cargo.toml:17` 未启用 devtools | dev/release 菜单截图需真机 |
| 4 | 文件夹层级 | ✅ 无归属字段、无 folders 文件/命令/UI；`notes.json` 根为裸数组（3 条） | `models.rs:497-504`；`store.rs:607`、`:964-969`；`KbView.tsx:182-189` | 无头静态 |
| 5 | 选择文件夹默认选中第一个 | ✅ `selectedId` 初值 null，唯一自动选中是 deep link；视图往返重挂载丢失选中 | `KbView.tsx:26`、`:74-102`；`App.tsx:132` | 无头静态 + 真机 |
| 6 | Ctrl+F / Ctrl+Shift+F | ✅ 全仓无对应 handler，无查找命令；Tiptap 仅绑 Ctrl+T | `task-detail.tsx:38-43`；`tiptap-editor.tsx:432-438` | 无头静态 + 真机按键 |
| 7 | 备份/恢复 | ✅ 前端零调用（`getBackups`/`restoreBackup`）；仅 data.json 轮转 5 份，notes 无备份；损坏被 `.ok()` 吞成静默空库 | `api.ts:378-379`；`SettingsView.tsx:353-362`；`fs_store.rs:113-139`；`store.rs:755-763` | 静态 + 磁盘实测；损坏/恢复一致性需真机 |
| 8 | 链接插入不居中/非原始组件 | 🔶 仅 `window.prompt` 单点实现；无居中弹窗组件（`.dialog-overlay` 有引用无样式） | `tiptap-editor.tsx:490` | 真机 UI |
| 9 | 标题分级下拉 + `###` | ✅ 工具栏仅 H1/H2 且无下拉、`SLASH_ITEMS` 仅 h1/h2；`### ` 输入可转 H3（1–6 均生效）；H4–H6 无样式；斜体/删除线已有，下划线无入口且序列化丢弃 | `prosemirror.css` 仅 h1–h3；Heading 默认 levels 1–6 | 无头静态；字号/IME 转换需真机 |
| 10 | 保存后 Markdown 变化 | ✅ 59 样例 **31 项变化、1 项非幂等**；图片整体丢失、表格对齐 `:---:` 全丢、`\|` 二次保存丢内容、HTML 转义等 | 详见 `editor.md`（含逐项对照表） | 无头 jsdom |
| 11 | 便签 MD 转换 + 待办最小化 | ✅ 便签正文是原生 textarea，渲染只靠手动「MD」按钮（状态不持久）；待办无 mini 形态（mini 仅服务便签） | `StickyWindow.tsx:273-282`、`:241-252`；`TodoFloatWindow.tsx:48-80`；`runtime.rs:117-119`、`:293-305` | 无头静态；观感需真机 |
| 12 | 便签持久化/CRUD/上限 5 | ✅ ✕ 与 Alt+F4 直接 `delete_sticky` 删条目 + `note_pos`；无 `visible`、无 CRUD 页、无 `open_sticky`；上限 6/4 分池 | `StickyWindow.tsx:107-111`；`commands.rs:581-596`；`store.rs:914-923`；`models.rs:476/478` | 静态 + 真机 |
| 13 | 菜单/知识库折叠 | ✅ 侧栏无图标、无折叠钮、无持久化，窄屏仅压成 56px 空条；知识库折叠为 0 宽 + `display:none` 丢标题，标题无省略号 | `App.tsx:21-28`、`:99-112`；`app.css:391-404`、`:441-454`；`kb.css:60-68`、`:149-152` | 无头静态 + 真机观感 |
| 14 | 打开本地数据目录 | ✅ 后端命令与 api 封装齐备，调用点仅设置页与托盘，知识库视图无入口 | `commands.rs:765-768`；`api.ts:354`；`KbView.tsx:148-157` | 无头静态 + 真机 explorer |
| 15 | 截图上传 | 🔶 编辑器无 Image 扩展（保存即丢图，见 #10 实测）；无粘贴/文件处理；便签正文 500 字上限 | `editor.md` #10；编辑器扩展清单 | 真机剪贴板粘贴 |
| 16 | 退出残留 msedgewebview2 | ✅ 托盘「退出」= `app.exit(0)`，tauri 1.8.3 直接 `cleanup_before_exit()` + `process::exit`，跳过窗口销毁链，不 Close CoreWebView2Controller；多窗口共享同一用户数据目录进程组 | `main.rs:272`；tauri `app.rs:564-568`、`:576-588` | 静态成因确定；残留现象需真机任务管理器观察 |
| 17 | CreateWebview 报错/可写用户数据目录 | ⬜ 本机不可自然复现（目录存在且可写、无失败事件）；炸点链已核准：`%LOCALAPPDATA%\com.todo.list` 建不出来→panic，或目录存在但 WebView2 初始化失败→`CreateWebview` panic；启动建窗失败因 telemetry 时序**零日志** | `tauri.conf.json:45`；`manager.rs:1059-1080`；`main.rs:322`；`telemetry.rs:55`；`app.rs:1750-1758` | 需受限环境/占位目录等真机方案（见 `window.md` 5 组方案） |

## 分组详情

| 文件 | 覆盖项 | 要点 |
|---|---|---|
| [`editor.md`](./editor.md) | #1 #2 #9 #10 | 闪烁快照复现；重渲染/序列化实测数据；59 例 Markdown 对照表 |
| [`kb.md`](./kb.md) | #4 #5 #6 #7 #14 | 磁盘 `notes.json` 实测（裸数组/3 条/无 folders.json）；备份零调用；损坏静默空库 |
| [`sticky.md`](./sticky.md) | #11 #12 | 关窗即删数据链路；上限分池；备份接口齐备但前端零调用（误删无处恢复） |
| [`shell.md`](./shell.md) | #3 #13 #16 | devtools/右键菜单判定；折叠 CSS；`app.exit(0)` 绕过窗口销毁链 |
| [`window.md`](./window.md) | #17 | 本机目录可写实测；A/B 型炸点链；启动失败无日志时序；5 组真机复现方案 |

## 与 tasks.md 的对应

本目录即 `tasks.md` 第 1 组（复现基线）的产出：1.2–1.6 分别对应上面 5 个证据文件，1.7 即本文件。1.1 的「可重复脚本骨架」未单独沉淀（无头脚本用后即清理），如需回归可后续补。17 项与台账 16+1 条一一对应（#16 = 退出 WebView2 残留，#17 = CreateWebview/数据目录）。

## 真机待确认清单

1. #1 空态块实际高度与闪烁可感知程度（IPC 延迟下）。
2. #2 输入掉帧程度、Rust 写盘/IPC 实际耗时。
3. #3 dev 与 release 右键菜单条目对照截图。
4. #5/#6/#14 实际选中、快捷键、explorer 行为。
5. #7 损坏数据下的恢复页/静默空库表现，恢复后一致性。
6. #9 标题各字号字重、中文输入法下 `### ` 即时转换、prompt UI。
7. #10 真实粘贴/导入路径与 `notes.json` 落盘内容。
8. #11 便签渲染观感、待办无最小化入口的目视确认。
9. #12 data.json 实际删除、Alt+F4 行为、上限报错文案。
10. #13 侧栏 56px 观感、知识库折叠可用性、长标题溢出。
11. #15 剪贴板截图粘贴行为。
12. #16 退出后 `msedgewebview2.exe` 残留数量与时长（`Get-CimInstance Win32_Process` 对照）。
13. #17 panic 原文/HRESULT、release 双击行为、`WEBVIEW2_USER_DATA_FOLDER` 设置后是否仍失败。
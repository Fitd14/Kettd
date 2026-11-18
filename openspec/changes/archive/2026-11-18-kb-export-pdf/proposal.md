# Proposal

## Why

Word 导出已经落地（提交 `e6d9b42`，`export_kb_docx`），但用户要的 PDF 仍缺着：拿到 `.docx` 还要再开 Word 转一次，交付与打印场景都多一道手。

此前判定 PDF 做不了，是因为两条同时成立：`webview2-com 0.19` 的绑定里没有 `PrintToPdf`，而 crates.io 又不可达、升不了版本。现在这两条都不再成立：网络已恢复（`cargo search` 与 crates.io 下载均通），并且技术验证已跑通——**不升 Tauri**，只加 `webview2-com 0.39` + `windows 0.62`，用 `ICoreWebView2_7::PrintToPdf` 在离屏 WebView2 里打印 HTML，实际产出 `%PDF-1.4`、1 页、47191 字节、`Producer Skia/PDF m157`（spike 提交 `117a3ee`，分支 `tauri-2-upgrade`）。

## What Changes

- **新增「导出为 PDF」**：知识库详情页工具条在现有 Word 导出旁增加 PDF 导出，把当前文档导出为 `.pdf`。
- **打印链路**：前端把正文渲染成自包含 HTML（复用现有 `markdown-it` 实例、跟随外观主题令牌、图片转 data URI），Rust 建离屏 WebView2 加载该 HTML，再调 `ICoreWebView2_7::PrintToPdf` 输出到用户选定路径。
- **目录选择与默认位置**：沿用 Word 导出的 `__TAURI__.dialog.save` 流程，默认文件名 `<标题>.pdf`、默认目录为系统下载目录；PDF 过滤器为 `pdf`。
- **目标依赖**：`src-tauri/Cargo.toml` 的 Windows 目标依赖新增 `webview2-com = "0.39"` 与 `windows = "0.62"`；**不动 Tauri 版本、不动 wry**（Tauri 2 升级是另一件已登记任务 `20261117-1558-4ade`）。
- **WebView2 用户数据目录**：固定放在数据目录下（例如 `<数据目录>/webview2/pdf-export`），MUST NOT 用 `%TEMP%`——spike 里指向 `%TEMP%` 时 `CreateCoreWebView2Controller` 直接报 `0x8000FFFF 灾难性故障`。
- **命令契约**：新增 `export_kb_pdf`，命令总数 76 → 77，四方同步 + parity 门禁。
- **失败回退**：超时（30 秒）或打印回调失败时给出中文短句提示，且不留下半截文件（临时文件 + rename，与 Word 导出同一套写盘约束）。
- **工具条口径修正**：`knowledge-base/doc-shell` 现有的「工具条上的文档动作」写的是「三个文档动作」，Word 导出落地后已与实现不符；本 change 把该 Requirement 改成含导出的动作集合，一次把文档与实现对齐。
- 不做：页眉页脚与页码、分页控制、PDF 加密/水印、批量导出、导出历史。

## Capabilities

### New Capabilities

（无：PDF 能力并入已有的导出能力，不另起新能力。）

### Modified Capabilities

- `knowledge-base/export`: 在 Word 之外增加 PDF 导出的行为约束（入口、目标路径、自包含 HTML、离屏 WebView2 打印、超时与失败回退、打印设置默认值）。**注意：该能力由 `kb-export-word` 引入，当前尚未归档；本 change 的 delta 以它存在为前提，归档顺序必须 `kb-export-word` 在前、`kb-export-pdf` 在后。**
- `knowledge-base/doc-shell`: 「工具条上的文档动作」由「三个文档动作（排版入口、复制 Markdown、大纲开合）」改为含「导出（Word / PDF）」的动作集合，并保留导出中与不可导出时按钮状态的约束。

## Impact

- 前端：`src-react/src/components/kb/kb-item-detail.tsx`（PDF 按钮、导出中状态、与 Word 导出共享的提示位）、新增 `src-react/src/lib/pdf-html.ts`（块模型/正文 → 自包含打印 HTML）、`src-react/src/lib/api.ts`（`exportKbPdf` 封装与 `pdf` 过滤器）、`src-react/src/views/kb.css`（若需按钮间距）。
- 后端：新增 `src-tauri/src/pdf/`（`webview.rs` 离屏窗口与 PrintToPdf、`mod.rs` 入口与超时）、`src-tauri/src/commands.rs` 新命令 `export_kb_pdf`、`src-tauri/src/main.rs` 注册；复用 `src-tauri/src/docx/mod.rs` 的 `resolve_target_path` / `write_bytes`（抽到共享模块或 `pub(crate)` 复用）。
- 依赖：新增两个 Windows 专用 crate（`webview2-com 0.39`、`windows 0.62`），需要提交 `Cargo.lock`；不加 npm 包。
- 契约与文档：`docs/ARCHITECTURE.md` 附录 §2 增一行、§10 计数 76 → 77；`test/command-parity.test.mjs` 同步；`README.md` 的「导出为 Word」小节扩成「导出 Word 与 PDF」并写明 PDF 依赖 WebView2 运行时。
- 风险面：unsafe Win32/COM 代码进入主链路（需 STA 线程 + 消息泵）、离屏窗口与进程生命周期、打印超时与清理。
# Tasks

## 1. 依赖与共享写盘工具

- [x] 1.1 在 `src-tauri/Cargo.toml` 的 Windows 目标依赖加 `webview2-com = "0.39"` 与 `windows = "0.62"`（features 按 design D2），验证：`cargo check --offline` 通过，`git diff src-tauri/Cargo.lock` 只新增这两个 crate 侧条目。
  验证记录：`cargo check --offline` exit 0（仅既有 17 条 warning）；Cargo.lock 新增 `webview2-com 0.39.1`、`webview2-com-macros`、`webview2-com-sys 0.39.1` 三个包，`windows 0.62.2` 原已在 lock 中、未重复解析。
- [x] 1.2 把 `src-tauri/src/docx/mod.rs` 的 `resolve_target_path` 与 `write_bytes` 改为 `pub(crate)`（不复制实现），验证：`cargo test --offline docx::` 仍 34 passed。
  验证记录：两函数改为 `pub(crate)` 后 `cargo test --offline docx::` → 34 passed / 0 failed。
- [x] 1.3 新建 `src-tauri/src/pdf/mod.rs` 与 `src-tauri/src/pdf/webview.rs` 骨架（安全外壳 + 内部 unsafe），在 `main.rs` 挂 `mod pdf;`，验证：`cargo check --offline` 通过且 `cargo clippy` 无新增 unsafe 警告。
  验证记录：`main.rs` 已有 `mod pdf;`；`cargo check --offline` exit 0。unsafe 全部收在 `webview.rs` 的 `mod imp`（Windows 实现）里，外层只暴露 `print_html`/`export_document` 安全函数。

## 2. 离屏打印链路

- [x] 2.1 实现窗口/环境/控制器创建与 `NavigateToString` + `NavigationCompleted` 等待（STA 线程 + 消息泵，窗口不显示、放在屏幕外），验证：`cargo run --example pdf_bridge_spike` 能走通导航回调并打印中文 HTML。
  验证记录：spike `src-tauri/examples/pdf_bridge_spike.rs`（worktree 提交 `117a3ee`）已走通并生成含中文的 PDF；正式实现 `webview.rs::print_html_inner` 同一套流程（`CoInitializeEx` + 离屏窗口 + `wait_for` 轮询 + 消息泵）。
- [x] 2.2 实现 `PrintToPdf`：优先 `ICoreWebView2Environment6::CreatePrintSettings` 设 A4 英寸尺寸，拿不到则传 `null` 并在 HTML 兜底 `@page { size: A4 }`；完成回调 `isSuccessful` 与 HRESULT 都转结果，验证：spike 路径打印出的 PDF 头部为 `%PDF-` 且 `/Type /Page` 计数 ≥ 1。
  验证记录：`print_settings()` 设 8.27×11.69 英寸、开背景、关页眉页脚，失败退回 `None`（浏览器默认），HTML 另有 `@page` 兜底；`cargo test --offline pdf::` 的 `smoke_prints_real_pdf`（ignored，需 danger-full-access）断言 `%PDF-1.4` 与 `/Type /Page`。
- [x] 2.3 加 30 秒硬超时与失败清理：临时 PDF 写到目标目录 `.<名>.<pid>.tmp.pdf`，成功后走 `write_bytes` 落盘，任何失败都删临时文件与临时 HTML，验证：单测模拟超时分支返回中文错误且目标目录无残留（`cargo test --offline pdf::`）。
  验证记录：`PRINT_TIMEOUT = 30s`；临时 PDF 命名 `.<名>.<pid>.tmp.pdf`；失败分支删临时文件再返回，单测 `失败不留残留且不覆盖目标` 通过。实现用 `NavigateToString` 直接吃 HTML 字符串，不写临时 HTML 文件（无临时 HTML 需要清理）。
- [x] 2.4 用进程内 `Mutex` 串行化导出，user data 目录固定 `<数据目录>/webview2/pdf-export`，验证：单测里两线程先后调用时第二个等待完成而非报错。
  验证记录：`pdf/mod.rs` 的 `static EXPORT_LOCK: Mutex<()>` 在打印期间持有，单测 `串行化` 断言两线程并发时峰值并发为 1。**实现差异**：user data 目录落在 `<数据目录>/pdf-webview/`（`commands.rs` 里 `crate::store::data_dir().join("pdf-webview")`），比任务写的更短，且仍不碰 `%TEMP%`（那里的 ACL 在门禁/沙箱下不放行，spike 实测报 `HRESULT(0x8000FFFF)`）。

## 3. 前端 HTML 与导出入口

- [x] 3.1 新建 `src-react/src/lib/pdf-html.ts`：`buildPdfHtml({ title, bodyHtml, theme })` 产出自带 `<style>`（纸面/墨色/标题/代码/引用/表格/图片 + `@page size: A4`）的完整文档，对标题与正文做 HTML 转义边界处理，验证：新增 `src-react/test/pdf-html.test.mjs` 覆盖转义、`@page` 存在、空正文拒绝、图片 data URI 拼接与主题变量注入。
  验证记录：`node src-react/test/pdf-html.test.mjs` → 7 passed / 0 failed（空正文、A4 与背景开关、标题转义、令牌注入、`assets/` 原样保留、脏值回落、正文与规则齐备）。**实现差异**：图片不在前端拼 data URI，而是保留 `src="assets/..."` 交给 Rust 内联（见 3.4），所以用例改为断言「保留 assets 引用、不出现 data URI」。
- [x] 3.2 在 `src-react/src/lib/api.ts` 加 `exportKbPdf(itemId, targetPath, html)`，并让 `pickExportPath` 接受格式参数（`docx` / `pdf` 过滤器与扩展名），验证：`tsc -b src-react` 通过，`exportKbPdf` 调用的命令名与 Rust 侧一致。
  验证记录：`exportKbPdf` → `call<string>('export_kb_pdf', { itemId, targetPath, html })`；`pickExportPath(defaultName, format: 'docx' | 'pdf' = 'docx')` 按格式选过滤器；`src-react/node_modules/.bin/tsc.cmd -b src-react --force` exit 0。
- [x] 3.3 在 `src-react/src/components/kb/kb-item-detail.tsx` 增加「导出为 PDF」按钮：与 Word 并列、独立 `exporting` 状态、共用 `exportNote` 提示位、空正文禁用，验证：`tsc -b src-react` 与 `oxlint` 无新增警告。
  验证记录：新增 `pdfExporting` 独立状态与 `pdfDisabled`；按钮用 `Printer` 图标、`aria-label="导出为 PDF"`；`exportPdf` 走 `currentMarkdown()` → `buildPdfHtml` → `pickExportPath(..., 'pdf')` → `exportKbPdf`；`tsc -b src-react` exit 0，`oxlint` 31 warnings / 0 errors（与改造前基线一致）。
- [x] 3.4 图片内联所需的字节读取路径（把 `assets/<sha>.<ext>` 读成 data URI）接入 `pdf-html.ts` 的调用方，验证：`node src-react/test/pdf-html.test.mjs` 里 data URI 用例通过。
  验证记录：**实现差异**：内联改为 Rust 侧 `src-tauri/src/pdf/inline.rs::inline_assets`（`NavigateToString` 的文档没有资源根，`convertFileSrc` 的绝对路径也不适用），读 `<数据目录>/assets/` 字节 → data URI，单层文件名防穿越、≤5 MB、缺失转 `[图片缺失：…]`；`inline.rs` 5 条单测 + `pdf/mod.rs` 的 `真实打印产物` 冒烟覆盖，前端 `pdf-html.test.mjs` 断言图片仍以 `assets/` 引用交给后端。

## 4. 命令契约与文档

- [x] 4.1 在 `src-tauri/src/commands.rs` 新增 `export_kb_pdf(state, item_id, target_path, html)`：锁内校验条目存在与 HTML 大小上限（8 MB）后放锁，转调 `pdf::export_document`；在 `main.rs` 的 `generate_handler![]` 注册，验证：`cargo check --offline` 通过，命令名与前端封装一致。
  验证记录：命令锁内只做 `ensure_writable`、条目存在性、取 `assets_dir`，随后放锁；HTML 大小上限在 `pdf::export_document` 的 `validate_html` 里（8 MB）；`main.rs` 注册 `commands::export_kb_pdf`；`cargo check --offline` exit 0。
- [x] 4.2 更新 `docs/ARCHITECTURE.md` 附录 §2（新增 `export_kb_pdf` 行）与 §10 命令计数 76 → 77，并同步 `test/command-parity.test.mjs` 的计数与断言，验证：`node test/command-parity.test.mjs` 6/6、`node test/docs.test.mjs` 4/4。
  验证记录：§2 新行 + §10 `77 ↔ 77 ↔ 77` + 台账说明追加；`node test/command-parity.test.mjs` 6 passed / 0 failed，`node test/docs.test.mjs` 4 passed / 0 failed。
- [x] 4.3 更新 `README.md`：「导出为 Word」小节扩成「导出 Word 与 PDF」，补 PDF 依赖 WebView2 运行时、默认 A4 与无页眉页脚、失败提示与回滚说明，验证：`node test/docs.test.mjs` 通过且 README 中的命令计数与 §10 一致。
  验证记录：小节改为「导出为 Word 与 PDF」，补 PDF 走 WebView2 打印、样式跟随当前主题、缺少运行时的中文提示；四处命令计数 76 → 77；`node test/docs.test.mjs` 4 passed。

## 5. 验证

- [x] 5.1 跑齐单层验证：`cargo test --offline pdf::`、`cargo test --offline docx::`、`node src-react/test/pdf-html.test.mjs`、`node src-react/test/docx-blocks.test.mjs`、`src-react/node_modules/.bin/tsc.cmd -b src-react`、`node scripts/css-parse.mjs src-react/src/views/kb.css`，验证：全部 exit 0、无新增警告。
  验证记录：`cargo test --offline pdf::` → 14 passed / 1 ignored；`cargo test --offline docx::` → 34 passed；`pdf-html` 7 passed；`docx-blocks` 18 passed；`tsc -b src-react --force` exit 0；CSS 解析随门禁通过（本轮未改 `.css`）。
- [x] 5.2 跑 `npm run check`（danger-full-access），验证：21 项全 OK、0 FAIL、0 SKIP，日志末行为「门禁全绿，可以提交。」
  验证记录：`npm run check` → 21 项全 OK / 0 FAIL / 0 SKIP，日志 `logs/check-20261118-083547.log`，末行「门禁全绿，可以提交。」。
- [x] 5.3 真机冒烟：导出含中文标题、列表、引用、代码块、表格与图片的文档，核对页数、中文无方框、图片在页内、无页眉页脚；连续导出两次；导出 PDF 后 Word 导出仍可用；空正文时按钮禁用；删除目标目录权限后提示可读，验证：五条全部通过并记录到 tasks 勾选说明。
  验证记录：2026-11-18 09:33 用户实机测试通过。同轮还按反馈修正了一次口径——打印件不带应用外观主题（纸色/墨色/圆角/斑马纹），只保留中性文档基线，修正后用户复查确认「实测已没有问题」。

## Workflow follow-up

- 真机五条通过后归档本 change，并确认 `knowledge-base/export` 主规格同时含 Word 与 PDF 需求（若 `kb-export-word` 尚未归档，先归档它再归档本 change）。
- 归档后跑 `openspec validate --all --strict`，确认全部 spec 与 change 通过。
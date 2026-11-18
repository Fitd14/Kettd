# Design

## Context

见 `proposal.md` 的 Why。当前实现状态与约束：

- Word 导出已落地：`src-tauri/src/docx/`（`model.rs` 块模型、`zip.rs` store-only ZIP、`ooxml.rs` 组装、`mod.rs` 入口）已提供 `resolve_target_path` / `write_bytes`（tmp + `.bak` + rename）；前端 `src-react/src/lib/docx-blocks.ts` 已把正文解析成块模型，`src-react/src/rendering/markdown.ts` 持有唯一 `MarkdownIt` 实例。
- 打印能力的版本事实：`PrintToPdf` 属于 `ICoreWebView2_7`（不是 `_16`），运行时 1.0.1020.30（2021 年）起提供；项目当前 `tauri 1.8.3 → wry 0.24.12 → webview2-com 0.19.1`，0.19 的绑定里没有 `PrintToPdf`。
- spike 已验证（提交 `117a3ee`，分支 `tauri-2-upgrade`）：新增 `webview2-com 0.39` + `windows 0.62` 后，在独立窗口里 `CreateCoreWebView2EnvironmentWithOptions` → `CreateCoreWebView2Controller` → `NavigateToString` → `ICoreWebView2_7::PrintToPdf(path, null, handler)` 能产出 `%PDF-1.4`、`Producer Skia/PDF m157`。
- spike 的硬坑：user data 目录指向 `%TEMP%` 时 `CreateCoreWebView2Controller` 返回 `0x8000FFFF 灾难性故障`；换到可写目录即通过。与 docx 测试被 `%TEMP%` ACL 拦住同源。
- 主 `Cargo.lock` 已有 `windows 0.62.2`，新增 `webview2-com 0.39` 复用同一 `windows` 版本，不引入第二套 Windows 绑定版本。

## Goals / Non-Goals

**Goals:**

- 在不升 Tauri、不动 wry 的前提下，用离屏 WebView2 把正文打印成 PDF。
- PDF 的正文语义与阅读视图同源（同一个 `MarkdownIt` 实例、同一套图片解析），不出现「网页版和导出版两套渲染」。
- 打印在独立 OS 线程内完成，带硬超时；失败路径不产生半截文件、不留临时目录垃圾。
- 与 Word 导出共享入口、标题清洗、默认下载目录与写盘约束。

**Non-Goals:**

- 不做页眉页脚、页码、目录页、分页控制与 PDF 加密。
- 不做批量导出、导出历史、导出模板。
- 不改 Tauri / wry 版本（Tauri 2 升级见台账任务 `20261117-1558-4ade`）。
- 不把 PDF 生成挪到前端（WebView2 打印必须由原生侧发起）。

## Decisions

### D1 用 `ICoreWebView2_7::PrintToPdf`，不升 Tauri

spike 证明 v1 下可用。备选方案与否决理由：

- 升 Tauri 2 / 升 wry：越过依赖冻结线，且迁移面大（`emit_all` 9 处、`get_window(` 21 处、窗口与托盘构建 12 处、全局快捷键 5 处、`tauri::api` 1 处、前端 `__TAURI__` 10 处、allowlist → capabilities 重写），与本次功能无关。
- Edge headless 子进程：依赖用户机器装了 Edge、要管子进程与临时文件，且输出与运行时 WebView2 版本可能不一致。
- 纯 Rust PDF 库：需要 CJK 字体子集化，中文体积与实现复杂度都高。

### D2 依赖只加两个 Windows 专用 crate

`src-tauri/Cargo.toml` 的 `[target.'cfg(target_os = "windows")'.dependencies]` 增加 `webview2-com = "0.39"` 与 `windows = "0.62"`（features 至少含 `Win32_Foundation`、`Win32_Graphics_Gdi`、`Win32_System_Com`、`Win32_System_LibraryLoader`、`Win32_UI_WindowsAndMessaging`）。两者与 tauri 自带的 0.19 系列是不同 crate 版本，可共存；`windows 0.62` 已在 lock 中，实际新增解析只有 `webview2-com` 侧。

### D3 目录与模块边界

```
src-tauri/src/pdf/mod.rs      入口：export_document(html, target) / 超时 / 临时文件
src-tauri/src/pdf/webview.rs  离屏窗口 + WebView2 环境 + PrintToPdf 封装
```

`mod.rs` 只暴露一个同步函数，内部负责线程、COM 初始化与消息泵；`webview.rs` 不感知路径策略与错误文案。写盘与路径校验复用 `docx::resolve_target_path` / `docx::write_bytes`（改成 `pub(crate)`，不复制一份）。

### D4 打印在独立线程，命令不阻塞

Tauri 命令跑在异步运行时线程上，WebView2 要求 STA + 消息泵，因此 `export_document` 内部 `std::thread::spawn`：线程内 `CoInitializeEx(COINIT_APARTMENTTHREADED)`，建窗（不 `ShowWindow`，窗口放在 `-32000,-32000` 之外不可见区，但仍 `SetIsVisible(true)` 以触发渲染）、建环境与控制器、`NavigateToString`、等 `NavigationCompleted`，再 `PrintToPdf`，用 `PeekMessageW`/`DispatchMessageW` 泵消息直到完成回调或超时。结果经 `mpsc` 回传并 `join`。

否决「用 Tauri 主窗 `with_webview` 打印」：主窗渲染的是应用界面，打印它得到的是 UI 截图；切到「临时渲染正文再打印」又要改主窗 DOM，风险更高。

### D5 user data 目录固定在数据目录下

路径：`<数据目录>/webview2/pdf-export`，首次使用时 `create_dir_all`。MUST NOT 用 `%TEMP%`（spike 实证 `0x8000FFFF`）。同一目录不允许两个 WebView2 环境同时打开，因此进程内用一把 `Mutex` 串行化导出；并发触发时后者排队而不是失败。

### D6 纸张 A4，边距取打印默认值

优先用 `ICoreWebView2Environment6::CreatePrintSettings`（从环境对象 `cast`）拿到设置对象，设 `PageWidth = 8.27` / `PageHeight = 11.69` 英寸，其余字段保持默认（即默认边距、无页眉页脚）。拿不到 `Environment6` 时回退 `PrintToPdf(path, null, handler)`，并在 HTML 里写 `@page { size: A4 }` 兜底。

### D7 HTML 由前端生成，Rust 只负责打印

命令形状：`export_kb_pdf(itemId, targetPath, html)`，与 `export_kb_docx(itemId, targetPath, blocks)` 对称——前端已经掌握 Markdown 语义，Rust 不重复实现。

- 新模块 `src-react/src/lib/pdf-html.ts`：`buildPdfHtml({ title, bodyHtml })`，产出 `<!doctype html>` + 内联 `<style>`（中性文档基线：白纸黑字、标题层级、列表、引用、代码、表格、图片与 `@page`）+ 正文 HTML。**不读外观主题令牌**：打印件是文档而非应用界面，主题纸色/圆角/斑马纹落到纸面反而碍事（用户 2026-11-18 实机反馈后修正）。
- 图片：读 `assets/<sha>.<ext>` 后转 `data:` URI 内联（与 docx 一样走 `resolveAssetUrl` 的取值路径，但导出时必须拿到字节而不是 URL）。
- HTML 长度上限 8 MB，超出由命令返回中文错误，避免把巨量字符串塞进打印流程。

### D8 输出走同目录临时文件再 rename

WebView2 直接写目标路径时，失败会留下半截 PDF。改为：打印到 `<目标目录>/.<名>.<pid>.tmp.pdf`，回调成功后读字节并用 `docx::write_bytes` 落到目标（同样 `.bak` 回滚语义），最后删临时文件；任何失败路径都删临时文件。

### D9 入口：工具条上与 Word 并列的第二个按钮

两个按钮同组相邻，各自独立 `exporting` 状态，共用一条 `exportNote` 提示位（`kb-strip-note`）。PDF 按钮的标签是「导出为 PDF」；导出中显示 `Loader2` 旋转图标。

### D10 测试策略

- 可在 Node 下测：`pdf-html.ts` 的模板转义、`assets/` 引用原样保留（交 Rust 内联）、`@page` 是否写入、空正文拒绝。
- 可在 Rust 单测：路径解析与扩展名补齐（复用 docx 已有用例）、HTML 大小上限、互斥串行化（两线程调 `export_document` 时第二个阻塞而非报错）。
- 打印链路本身不可在沙箱内自动化（WebView2 需要交互桌面），保留 `src-tauri/examples/pdf_bridge_spike.rs` 作为手工回归手段，并写入真机冒烟清单。

## Risks / Trade-offs

- [unsafe Win32/COM 代码进入主链路] → 把 unsafe 限定在 `pdf/webview.rs` 内，对外只暴露安全函数；所有 HRESULT 转中文错误；不裸解引用跨线程指针（句柄只在本线程内使用）。
- [用户机器缺 WebView2 运行时或用企业策略禁用] → 环境创建失败即返回「这台电脑缺少 PDF 导出所需的运行组件」，不影响 Word 导出。
- [首次导出慢（环境创建 + 冷启动）] → 按钮进入进行中状态，超过 300ms 才显示「正在准备…」，避免闪一下。
- [离屏窗口抢焦点或闪现] → 窗口创建时不 `ShowWindow` 且在屏幕外，`SetIsVisible(true)` 仅让 WebView2 认为可见；打印完成后立即 `DestroyWindow`。
- [同一数据目录被两处打开] → 进程内 `Mutex` 串行；不做跨进程锁（单实例应用已由运行时保证）。
- [PDF 与 Word 的保真度不一致] → 两者共用同一 `MarkdownIt` 实例与同一块模型/HTML 源，差异只允许出现在分页与字体替换上。
- [大文档打印超时] → 30 秒上限；真机上若常见超时，再评估改为按页数分片或提高上限。

## Migration Plan

纯增量，无数据迁移：

1. 合入后首次导出时创建 `<数据目录>/webview2/pdf-export`。
2. 回滚：删除 `export_kb_pdf` 命令注册与 `src-tauri/src/pdf/`，移除两个 Windows 依赖；设置与数据文件均无改动，回滚不需要恢复数据。

## Open Questions

- 默认边距的具体数值是否要固定（当前取 WebView2 默认），等真机看过排版再定。
- 是否需要在 PDF 里保留链接可点击（取决于打印设置；当前默认行为，不额外处理）。
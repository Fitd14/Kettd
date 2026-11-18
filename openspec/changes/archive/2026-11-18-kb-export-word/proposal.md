# Proposal

## Why

知识库正文目前只存在于应用内：用户想把它带到 Word 里交付、打印或继续加工，只能全选复制，标题层级、列表、引用、表格与图片都会散架。用户明确要「导出 Word 与 PDF，导出时自己选目录，默认落在系统下载位置」。

本机与仓库同时受两条硬约束：crates.io 不可达（`cargo search` 报 `schannel: SEC_E_NO_CREDENTIALS`），cargo 缓存里也没有 docx / zip / pdf 相关 crate；PDF 若走 WebView2 打印，需要 `webview2-com ≥0.21` 才有的 `ICoreWebView2_16::PrintToPdf`，而缓存里只有 `webview2-com-sys 0.19.0`、绑定里连 `PrintToPdf` 都没有。因此本期只做 Word，且必须零新依赖；PDF 另开一个 change，等依赖升级窗口。

## What Changes

- **新增「导出为 Word」**：知识库详情页工具条加一个「导出」按钮，把当前文档导出为 `.docx`。
- **导出前选目录**：用系统保存对话框，默认文件名 `<标题>.docx`、默认目录取系统下载目录；用户可在对话框里换目录与文件名，取消则什么都不做。
- **正文转换**：Markdown 先在前端解析成统一块模型（标题、段落与行内样式、列表、引用、代码块、表格、图片），再交给 Rust 生成 OOXML，避免前后端各写一套 Markdown 语义。
- **零新依赖**：Rust 侧手写最小 OOXML 与 store-only ZIP 写入器（自算 CRC32）；图片从 `<数据目录>/assets/<sha>.<ext>` 读入并作为 `word/media/*` 打包。
- **入口只有一处**：本期只放详情页工具条，文件树右键、批量导出、PDF 都不做。
- 不改变知识库存储格式、不改备份导出格式、不新增设置项。

## Capabilities

### New Capabilities

- `knowledge-base/export`: 知识库文档导出为外部文件的能力（本期覆盖 Word/`.docx`），包括入口、目标路径选择与默认下载目录、块模型保真范围、图片嵌入、失败回退与文件名处理。

### Modified Capabilities

（无：知识库持久化、备份恢复、编辑器与写作布局的既有 Requirement 都不变。）

## Impact

- 前端：`src-react/src/components/kb/kb-item-detail.tsx`（工具条按钮与导出流程）、新增 `src-react/src/lib/docx-blocks.ts`（Markdown→块模型）、`src-react/src/rendering/markdown.ts`（把 `md-static-render.tsx` 里的 MarkdownIt 实例抽成共享模块，行为不变）、`src-react/src/lib/api.ts`（新命令封装与 `__TAURI__.dialog` / `__TAURI__.path` 桥接类型）。
- 后端：新增 `src-tauri/src/docx/`（`model.rs` 块模型反序列化、`zip.rs` store-only 写入器、`ooxml.rs` 组装、`mod.rs` 入口）、`src-tauri/src/commands.rs` 新命令 `export_kb_docx`、`src-tauri/src/main.rs` 注册。
- 契约：`docs/ARCHITECTURE.md` 附录 §2 增一行、§10 命令计数 75 → 76；`test/command-parity.test.mjs` 计数与断言同步。
- 依赖：无新增 crate、无新增 npm 包（`tauri.conf.json` 已 `allowlist.all = true`，`dialog` 与 `path` 可用）。
- 不在本期：PDF 导出（等 `webview2-com` 可升级）、批量/多篇导出、导出模板与样式主题联动、导出历史记录。
# 20261117-1500-be76-知识库文档导出-word-与-pdf: 知识库文档导出 Word 与 PDF

- **Tags**: Word, PDF, 导出, 下载目录
- **Status**: doing · 2026-11-17 15:40:28 · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd
- **Created**: 2026-11-17 15:00:34 · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd

<!-- Header lines are maintained by the script, fixed order: Tags → Status → Created → Sealed.
     The `- **Tags**: …` line is inserted at the top by new --tags; omitted when there are no tags
     (category goes to the ledger and history only, never into this document).
     The `- **Sealed**: …` line is appended on close. -->

## Background

2026-11-17 22:57 用户原话：新建任务 知识库文档支持word和pdf格式导出，导出时需要用户指定目录进行导出 默认放在系统默认下载位置

## To Do

- [x] 确认 PDF 实现路线（见 Process 的三选一），再定 OpenSpec change 范围 → 用户选「等网络恢复，用 WebView2 打印」，本期只做 Word
- [x] 目录选择：保存对话框，默认 `<系统下载目录>/<条目标题>.docx`
- [x] Word 保真范围：标题 / 段落 / 粗斜体 / 列表 / 引用 / 代码块 / 表格 / 图片（无页码、无目录页）
- [x] 图片嵌入：Rust 读 `<数据目录>/assets` 字节写入 `word/media/`，最大宽度 15 cm、单张上限 5 MB
- [ ] 门禁与真机验证（门禁已绿；真机冒烟待用户）

## Next

用户在运行中的应用里跑五条冒烟（导出成功、默认下载目录、取消不报错、空文档按钮禁用、导出后原文与列表未变），确认后在 `openspec/changes/kb-export-word/tasks.md` 勾 5.2/5.3 并归档。

## Process

- 2026-11-17 23:03 可行性调研（未改代码）：
  - 可复用：`src-tauri/tauri.conf.json:7` `withGlobalTauri: true`、`:15` `allowlist.all = true` → 前端 `__TAURI__.dialog.save/open` 可用；Rust 依赖已含 `dirs 6.0`（可取 `download_dir()`）；正文是 `bodyMd`，前端有 `markdown-it`（`src-react/src/rendering/md-static-render.tsx`）；图片是 `assets/<sha256>.<ext>` 内容寻址（`src-react/src/rendering/asset-url.ts`）；Rust 侧已有 `write_export_file` 原子写先例（`src-tauri/src/infra/fs_store.rs:567`）。
  - **硬约束**：crates.io 不可达（`cargo search docx-rs` → `schannel: AcquireCredentialsHandle failed: SEC_E_NO_CREDENTIALS`），`~/.cargo/registry/cache` 里没有 docx / pdf / zip 相关 crate → 现在**加不了新 Rust 依赖**，npm 同样不可用。
  - Word 路线：A 手写 OOXML + 最小 ZIP（store-only + 自算 CRC32，零新依赖，约 500–700 行含测试）；B 网络恢复后加 `docx-rs`。
  - PDF 路线：A `msedge --headless=new --print-to-pdf`（本机 Edge 157 存在；沙箱内实跑被 crashpad/mojo 拒绝，属环境限制）；B WebView2 `PrintToPdfAsync`（要 `webview2-com`，网络受阻）；C 纯 Rust PDF 写出 + CJK 字体子集化（要 crate，体积与复杂度高）；D `window.print()` 交用户（零依赖但无法指定目录）。
  - 目录语义：Windows 的「保存」对话框本身就能选目录 → 一次交互同时满足「指定目录 + 默认下载位置」。
- 2026-11-17 23:45 实现（OpenSpec change `kb-export-word`，15 项任务中 1.1–4.3 已勾）：
  - 前端：`src-react/src/rendering/markdown.ts` 抽出共享 MarkdownIt 实例（阅读视图与导出共用，防漂移）；`src-react/src/lib/docx-blocks.ts` 把正文解析成块模型（标题/段落/列表/引用/代码/表格/图片，18 项单测）；`src-react/src/lib/api.ts` 增 `pickExportPath`（`dialog.save`，默认下载目录）与 `exportKbItemDocx`；详情页工具条加「导出为 Word」按钮与状态提示。
  - 后端：`src-tauri/src/docx/{model,zip,media,ooxml,mod}.rs` 手写 OOXML + store-only ZIP（自算 CRC32）+ 临时文件 rename，34 项单测；`commands.rs::export_kb_docx` 在锁内校验条目与可写性、锁外生成；`main.rs` 注册。
  - 契约四方同步：命令数 75 → 76（`main.rs` / `commands.rs` / `api.ts` / `ARCHITECTURE.md` §2+§10 / `test/command-parity.test.mjs` / README 四处计数）。
  - 外部校验：`python-docx 1.2.0` 打开富样本 → Heading 1/2 样式、中文完好、1 张表、1 个内嵌图（9525×9525 EMU）、A4 页面、超链接存在；`.NET ZipFile` 读出 7 个条目、6 份 XML 全部 well-formed。
  - 门禁：`npm run check` 首跑因测试写 `%TEMP%` 被 ACL 拒绝（13 红），改为 `test_dir` 回退 `target/docx-test-tmp` 后 `cargo test docx::` 34 passed，重跑门禁全绿。
  - 遗留：真机五条冒烟与「导出前后数据未变」由用户体验确认；PDF 待网络恢复后升级 `webview2-com` 再做。

## Conclusion

<!-- What got done, what got left behind -->

## Execution Log

<!-- Maintained by the script: appended by new / claim / set / release / log / close. Do not edit.
     Backfill with: progress log --id 20261117-1500-be76-知识库文档导出-word-与-pdf --note "...". -->
- 2026-11-17 15:00:34 · new · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd · created
- 2026-11-17 15:01:43 · claim · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd · claimed → doing
- 2026-11-17 15:03:53 · log · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd · 可行性调研：crates.io 不可达且无 docx/pdf 缓存 crate，暂不能加依赖；Word 走手写 OOXML+ZIP，PDF 三选一（Edge headless / 等网络加 crate / 先只做 Word）；目录走 dialog.save 默认下载目录
- 2026-11-17 15:40:28 · log · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd · 实现完成：共享 MarkdownIt、块模型 18 项单测、详情页导出入口、Rust OOXML+store-only ZIP 34 项单测、export_kb_docx 契约四方同步 76、门禁 21 项全绿（logs/check-20261117-233944.log）；真机 5.2/5.3 待验

# 20261117-1500-be76-知识库文档导出-word-与-pdf: 知识库文档导出 Word 与 PDF

- **Tags**: Word, PDF, 导出, 下载目录
- **Status**: doing · 2026-11-17 15:03:53 · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd
- **Created**: 2026-11-17 15:00:34 · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd

<!-- Header lines are maintained by the script, fixed order: Tags → Status → Created → Sealed.
     The `- **Tags**: …` line is inserted at the top by new --tags; omitted when there are no tags
     (category goes to the ledger and history only, never into this document).
     The `- **Sealed**: …` line is appended on close. -->

## Background

2026-11-17 22:57 用户原话：新建任务 知识库文档支持word和pdf格式导出，导出时需要用户指定目录进行导出 默认放在系统默认下载位置

## To Do

- 确认 PDF 实现路线（见 Process 的三选一），再定 OpenSpec change 范围
- 目录选择：保存对话框，默认 `<系统下载目录>/<条目标题>.<ext>`
- Word 保真范围未定：标题 / 段落 / 粗斜体 / 列表 / 引用 / 代码块 / 表格 / 图片，是否要页码、目录页
- 图片嵌入方式未定（DOCX 需读 assets 字节；PDF 走 data URI）
- 门禁与真机验证（三平台只要求 Windows）

## Next

等用户选 PDF 路线：`Edge headless 打印`（零新依赖）/ `等网络恢复后加 Rust crate` / `先只做 Word`；选定后跑 `openspec new change` 起 change。

## Process

- 2026-11-17 23:03 可行性调研（未改代码）：
  - 可复用：`src-tauri/tauri.conf.json:7` `withGlobalTauri: true`、`:15` `allowlist.all = true` → 前端 `__TAURI__.dialog.save/open` 可用；Rust 依赖已含 `dirs 6.0`（可取 `download_dir()`）；正文是 `bodyMd`，前端有 `markdown-it`（`src-react/src/rendering/md-static-render.tsx`）；图片是 `assets/<sha256>.<ext>` 内容寻址（`src-react/src/rendering/asset-url.ts`）；Rust 侧已有 `write_export_file` 原子写先例（`src-tauri/src/infra/fs_store.rs:567`）。
  - **硬约束**：crates.io 不可达（`cargo search docx-rs` → `schannel: AcquireCredentialsHandle failed: SEC_E_NO_CREDENTIALS`），`~/.cargo/registry/cache` 里没有 docx / pdf / zip 相关 crate → 现在**加不了新 Rust 依赖**，npm 同样不可用。
  - Word 路线：A 手写 OOXML + 最小 ZIP（store-only + 自算 CRC32，零新依赖，约 500–700 行含测试）；B 网络恢复后加 `docx-rs`。
  - PDF 路线：A `msedge --headless=new --print-to-pdf`（本机 Edge 157 存在；沙箱内实跑被 crashpad/mojo 拒绝，属环境限制）；B WebView2 `PrintToPdfAsync`（要 `webview2-com`，网络受阻）；C 纯 Rust PDF 写出 + CJK 字体子集化（要 crate，体积与复杂度高）；D `window.print()` 交用户（零依赖但无法指定目录）。
  - 目录语义：Windows 的「保存」对话框本身就能选目录 → 一次交互同时满足「指定目录 + 默认下载位置」。

## Conclusion

<!-- What got done, what got left behind -->

## Execution Log

<!-- Maintained by the script: appended by new / claim / set / release / log / close. Do not edit.
     Backfill with: progress log --id 20261117-1500-be76-知识库文档导出-word-与-pdf --note "...". -->
- 2026-11-17 15:00:34 · new · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd · created
- 2026-11-17 15:01:43 · claim · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd · claimed → doing
- 2026-11-17 15:03:53 · log · @simy · afcfcca0-be4c-4afd-931b-16fdade9d4fd · 可行性调研：crates.io 不可达且无 docx/pdf 缓存 crate，暂不能加依赖；Word 走手写 OOXML+ZIP，PDF 三选一（Edge headless / 等网络加 crate / 先只做 Word）；目录走 dialog.save 默认下载目录

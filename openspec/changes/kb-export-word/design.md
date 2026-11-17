# Design

## Context

参考 `proposal.md` 的 Why。实现这条链路只需要现有能力，不需要新依赖：

- 前端已有 `markdown-it@15`，只读视图 `src-react/src/rendering/md-static-render.tsx` 里建了实例（`html: false`、`linkify: true`、`breaks: true`，并覆盖了 `fence`/`table_open`/`image` 三个 renderer）。
- Rust 侧已有原子写与文件名兜底：`src-tauri/src/infra/fs_store.rs:59` `safe_export_file_name`、`:567` `write_export_file`；导出目录解析在 `src-tauri/src/export.rs:353` `target_dir`；命令先例是 `src-tauri/src/commands.rs:1280` `export_weekly`（同步命令、锁内只取数据、放在锁外做 I/O）。
- 图片是内容寻址资产：正文写 `assets/<sha256>.<ext>`，绝对目录来自 `src-tauri/src/store.rs:1253` `assets_dir()`（`models.rs:1144` 的 `assets_dir` 字段把它传给前端）。
- `src-tauri/tauri.conf.json:15` `allowlist.all = true`、`:23-25` `path.all = true`，`withGlobalTauri: true`，所以前端可以直接用 `__TAURI__.dialog.save` 与 `__TAURI__.path.downloadDir`。
- 两条外部约束决定了技术选型：crates.io 不可达、cargo 缓存无 docx/zip 类 crate；`webview2-com-sys 0.19.0` 的绑定里没有 `PrintToPdf`（`ICoreWebView2_16` 未生成），PDF 只能等依赖升级。

## Goals / Non-Goals

**Goals:**
- 用一份块模型同时喂给 DOCX 生成器与将来的 PDF 生成器，避免每次导出各写一套 Markdown 语义。
- 零新依赖、离线可用、导出过程只读知识库数据。
- 生成的 `.docx` 能被 Word / WPS 直接打开，标题、列表、引用、表格、图片都在。

**Non-Goals:**
- 不做 PDF、不做批量导出、不做导出模板与主题联动（见 proposal）。
- 不追求逐像素还原阅读视图（不复制纸色、投影、字距）。
- 不实现 DOCX deflate 压缩、不做增量写入、不支持导出加密文档。

## Decisions

### D1 解析在前端，组装在 Rust

前端把 `bodyMd` 用已有的 `markdown-it` 实例解析成 token 流，再走一遍 token → 块模型；Rust 只反序列化块模型并写 OOXML。

- 理由：Markdown 语义只有一份实现，将来 PDF 复用同一份块模型；Rust 端不引 md 解析器（无 crate 可用，且必然与前端渲染有细微分叉）。
- 否决：Rust 端重新解析 Markdown（语义漂移 + 额外维护面）；前端直接生成 HTML 再包一层 `.docx`（Word 对 HTML-in-OOXML 的支持是兼容路径，表格/图片/分页都不可控）。

### D2 块模型契约

前端产出、命令入参，序列化为 JSON（camelCase，`kind` 判别）：

```
ExportBlock =
  | { kind: "heading", level: 1|2|3, runs: Run[] }
  | { kind: "paragraph", runs: Run[] }
  | { kind: "bullets", items: { depth: number, runs: Run[] }[] }
  | { kind: "numbers", items: { depth: number, runs: Run[] }[] }
  | { kind: "quote", blocks: ExportBlock[] }
  | { kind: "code", lang?: string, text: string }
  | { kind: "table", header: boolean, rows: string[][] }
  | { kind: "image", asset: string, alt?: string }
Run = { text: string, bold?: boolean, italic?: boolean, code?: boolean, link?: string }
```

- 引用最多保留两层，第三层起拍平进父级（Word 缩进再深就看不出层级）。
- 表格单元格只保留纯文本（含行内标记的单元格丢掉标记、保留文字）。
- 任务列表 `- [ ]` 由 `markdown-it` 解析成普通列表项 + 字面方括号，本期不做 checkbox 还原（proposal 已声明）。

### D3 ZIP 用 store-only，自算 CRC32

`src-tauri/src/docx/zip.rs` 手写 ZIP：local file header + file data + central directory + EOCD，压缩方法固定 0（stored），CRC32 用查表实现。

- 理由：没有 zip crate 可用；`.docx` 体积通常几百 KB，不压缩只损失体积、不影响兼容性。
- 否决：手写 deflate（复杂度与出错面远超收益）；等网络恢复再用 `zip` crate（本期就要能用）。

### D4 OOXML 最小集

`word/document.xml` + `word/styles.xml` + `word/numbering.xml`，加 `[Content_Types].xml`、`_rels/.rels`、`word/_rels/document.xml.rels`、`word/media/*`，可选 `docProps/core.xml`（写标题与生成时间，失败不影响打开）。

- 页面 A4（`w:pgSz w:w="11906" w:h="16838"`）、页边距 1440 twips，正文默认「等线」小四（`w:sz 24`，即 12pt），1.5 倍行距（`w:spacing w:line="360" w:lineRule="auto"`）。
- 标题 1/2/3 映射到 `w:pStyle Heading1/2/3`，样式在 `styles.xml` 里给出字号与段前段后；不依赖 Word 内置样式表存在与否。
- 列表用 `numbering.xml`：一个 `abstractNum` 定义 bullet、一个定义 decimal，`numId` 分别引用；嵌套用 `w:ilvl`（最多 3 级）。
- 引用块用 `w:ind w:left="720"` + 左侧 `w:pBdr` 竖线；代码块用等宽字体（Consolas）+ 浅灰底 `w:shd`。
- 表格用 `w:tbl` + 单线边框，首行 `w:tblHeader` + 加粗。

### D5 图片：直接嵌原始字节，超限降级

从 `assets_dir` 读 `<sha>.<ext>`，按 96dpi 把像素换算成 EMU（1 px = 9525 EMU），等比缩到正文可用宽度（约 15 cm）以内。

- 单图上限 5 MB，超过则写 alt 占位并附一句「图片过大未嵌入」，不中断导出。
- WebP 按原样嵌入（Word 2019+ 支持；更老的 Word 可能显示空白），在 README 与设置页说明里不承诺 WebP 兼容——若要彻底解决需转码，而转码要 image crate，不在本期。
- 读不到文件：占位文本取 alt，缺省 `[图片缺失]`。

### D6 目录选择交给系统对话框

前端 `__TAURI__.dialog.save({ defaultPath, filters })`；`defaultPath` = `await __TAURI__.path.downloadDir()` + `/` + 清洗后的标题 + `.docx`，`downloadDir()` 抛错时退回 `documentDir()`，都失败则省略 `defaultPath`（让系统用上次目录）。

- 理由：Windows 保存框本身就是「选目录 + 选文件名 + 覆盖确认」三合一，取消返回 `null`，天然满足「取消不写盘」。
- 否决：Rust 端 `FileDialogBuilder`（阻塞对话框在命令线程里调用有挂起风险，且拿不到「取消」这条干净信号）。

### D7 命令契约：`export_kb_docx`

```rust
#[tauri::command]
pub fn export_kb_docx(
  state: State<'_, Shared>,
  item_id: String,
  target_path: String,
  blocks: Vec<crate::docx::model::ExportBlock>,
) -> Result<String, String>
```

- 锁内只做两件事：`ensure_writable()`、确认条目仍存在（不存在返回「这篇文档已经不存在了」）、取 `assets_dir`；随后立刻 drop 锁。
- 路径校验：必须是绝对路径、扩展名不是 `.docx` 就补上、父目录必须存在；写同目录 `.<文件名>.tmp` 再 rename，失败删临时文件并返回中文短句。
- 返回最终绝对路径，供界面提示「已导出到 …」。

### D8 前端流程与入口

`src-react/src/lib/docx-blocks.ts` 导出 `toDocxBlocks(markdown: string): ExportBlock[]` 与 `sanitizeDocxName(title: string): string`（去 `<>:"/\|?*`、折叠空白、截断 80 字、空则「未命名文档」）。

详情页工具条（`src-react/src/components/kb/kb-item-detail.tsx` 的 `kb-strip-tools` 容器旁）加「导出」按钮：`disabled = exporting || !item.bodyMd?.trim()`；点击后依次取块、取下载目录、弹保存框、调命令；成功后 `setNote('已导出到 …')`，失败显示后端中文错误。导出期间按钮转 loading，重复点击直接忽略。

### D9 抽出共享 MarkdownIt 实例

新建 `src-react/src/rendering/markdown.ts`，把 `md-static-render.tsx` 里的构造与三个 renderer 覆盖原样搬过去并 `export const md`；`md-static-render.tsx` 改为 import。`docx-blocks.ts` 用同一个实例 `md.parse()`。

- 理由：`html:false` / `breaks:true` 这类配置一旦两处各写一份就会漂移；共享实例保证「导出看到的结构 = 阅读视图看到的结构」。
- 风险：`md.renderer.rules` 的覆盖对 `parse()` 无影响（token 流与 renderer 无关），已确认。

### D10 契约四方同步

新命令按 AGENTS §2.4 四处同改：`src-tauri/src/main.rs` 的 `generate_handler![]`、`src-tauri/src/commands.rs`、`src-react/src/lib/api.ts`、`docs/ARCHITECTURE.md` §2 表格与 §10 计数（75 → 76）；`test/command-parity.test.mjs` 的总数与 `handler.has(...)` 断言同步。

## Risks / Trade-offs

- [手写 OOXML 结构错，Word 报「内容有问题」] → 先写最小可通过的结构（段落 + 标题），再逐类加；单测用 ZIP 读回校验各 XML 能被解析、关系表指向的文件都存在；最后真机用 Word 打开冒烟。
- [store-only ZIP 体积偏大] → 可接受；docx 主要成分是 XML 与图片，本身已压缩过的不多。单图 5 MB 上限兜底。
- [WebP 在老 Word 里空白] → 在 README 的导出说明里写明 WebP 兼容性不做承诺。
- [大表格 / 超长文档生成慢] → 全部在命令线程同步跑，超过 5 MB 文档时按钮先禁用再提示；后续若真慢再迁到异步命令。
- [标题清洗后为空或只剩符号] → 回落「未命名文档」。
- [路径校验放宽导致越界写] → 只接受绝对路径且父目录存在，写临时文件后 rename；不做目录内沙箱（用户已在系统对话框里亲手选过）。
- [markdown.ts 抽取引入渲染回归] → 抽取后跑 `node test/md-fidelity.test.mjs` 与现有阅读视图快照，行为应逐字节一致。

## Migration Plan

- 无数据迁移；老数据（`bodyMd` + `assets/`）天然满足导出输入。
- 回滚：删掉工具条按钮与 `export_kb_docx` 注册、命令、前端封装即可；不涉及存储格式，回滚后已导出的 `.docx` 仍可用。
- 发布：随下一次版本一起出，无需数据版本号变更。

## Open Questions

- PDF 导出单独立 change 的排期：等 `webview2-com` 可升级到带 `ICoreWebView2_16` 的版本再评估（可能牵动 wry/Tauri v1 冻结线）。
- 将来是否要「导出时套用当前外观主题」（标题字体、纸色）——本期不做，用户提了再单独设计。
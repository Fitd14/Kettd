# Tasks

## 1. 前端解析层

- [x] 1.1 新建 `src-react/src/rendering/markdown.ts`，把 `md-static-render.tsx` 里的 MarkdownIt 实例与 `fence`/`table_open`/`image` 三个 renderer 覆盖原样搬过去并导出 `md`，组件改为 import；验证：`node test/md-fidelity.test.mjs` 全通过、`src-react/node_modules/.bin/tsc.cmd -b src-react` 无错，且阅读视图渲染结果与改造前逐字节一致（对比一段含表格/图片/代码的样例输出）
- [x] 1.2 新建 `src-react/src/lib/docx-blocks.ts`：导出 `toDocxBlocks(markdown)`（按 design D2 的块模型输出标题/段落/列表/引用/代码/表格/图片，引用最多两层、表格取纯文本）与 `sanitizeDocxName(title)`（去 `<>:"/\|?*`、折叠空白、截断 80 字、空则「未命名文档」）；验证：新建 `src-react/test/docx-blocks.test.mjs`，覆盖标题层级、行内加粗/斜体/行内代码/链接、嵌套列表深度、引用套引用拍平、表格含表头、`assets/` 图片、任务列表退化为字面方括号，全部通过（18 项 18 通过；引用深度改按 token.level 判断，嵌套列表改为单一外层上下文以保住嵌套项）

## 2. 前端导出入口

- [x] 2.1 扩展 `src-react/src/lib/api.ts`：`TauriGlobal` 增 `dialog?: { save?: (options: { defaultPath?: string; filters?: { name: string; extensions: string[] }[] }) => Promise<string | null> }` 与 `path?: { downloadDir?: () => Promise<string>; documentDir?: () => Promise<string> }`；新增 `ExportBlock`/`InlineRun` 类型与 `exportKbItemDocx(itemId, targetPath, blocks)` 封装（沿用 `call()` 的中文错误约定）；验证：`tsc -b` 通过，且 `grep` 确认未把 `__TAURI__` 提前缓存到模块级变量（`tsc -b src-react` exit 0；桥仍由 `tauriBridge()` 每次惰性取，无 `globalThis.__TAURI__` 缓存行）
- [x] 2.2 在 `src-react/src/components/kb/kb-item-detail.tsx` 的工具条加「导出」按钮并接完整流程：取块模型 → `downloadDir()` 失败退回 `documentDir()` → `dialog.save`（默认名 `sanitizeDocxName(title).docx`）→ 取消直接返回 → 调命令 → 成功提示「已导出到 …」、失败显示后端中文错误；`disabled` 条件为正文空或导出中，重复点击忽略；验证：`tsc -b` 与 `node src-react/node_modules/oxlint/bin/oxlint` 均通过（无新增 warning），并人工确认空文档时按钮禁用（`tsc -b src-react` exit 0；oxlint 31 warnings / 0 errors 与改造前基线持平；`disabled={exporting || item.bodyMd.trim() === ''}`，`exporting` 为首次进入即置位——真机空文档外观待 5.2 冒烟时确认）

## 3. Rust DOCX 生成

- [x] 3.1 新建 `src-tauri/src/docx/model.rs`：按 design D2 定义 `ExportBlock`（`#[serde(tag = "kind", rename_all = "camelCase")]`）与 `Run`，字段与前端 JSON 对齐；验证：`cargo test docx_model` 覆盖空块、各类块的 JSON 反序列化与未知 `kind` 报错（`cargo test --offline docx::model` → 5 passed；未知 kind 断言报错含 `unknown variant`）
- [x] 3.2 新建 `src-tauri/src/docx/zip.rs`：store-only ZIP 写入器（local header + central directory + EOCD、查表 CRC32、UTF-8 文件名位标记），提供 `ZipWriter::add_file(name, bytes)` 与 `finish()`；验证：单测用 `std::fs` 写临时文件后按偏移读回，断言 EOCD、条目数与 CRC 与自行计算值一致，且空 ZIP 合法（`cargo test --offline docx::zip` → 4 passed；CRC 已知向量 `123456789 → 0xCBF4_3926`；修正过一处测试偏移——标志在 6、方法在 8）
- [x] 3.3 新建 `src-tauri/src/docx/ooxml.rs`：生成 `[Content_Types].xml`、`_rels/.rels`、`word/document.xml`、`word/_rels/document.xml.rels`、`word/styles.xml`、`word/numbering.xml`；正文覆盖标题 1–3、段落与行内 run（加粗/斜体/等宽/超链接）、有序/无序列表（含 `w:ilvl` 缩进）、引用（缩进 + 左竖线）、代码块（Consolas + 浅灰底）、表格（单线边框 + 首行表头）；XML 特殊字符一律转义；验证：单测对每类块断言关键标记存在（如 `w:pStyle w:val="Heading1"`、`w:numPr`、`w:tbl`），并用 `quick-xml` 之外的轻量检查（well-formed 由真机 Word 兜底）（`cargo test --offline docx::ooxml` → 10 passed；样本经 python-docx 1.2 打开通过：Heading 1/2 样式、表格 1 张、图片 9525×9525 EMU、A4 节 7560310×10692130 EMU；Word 未参与本次校验，见 5.2）
- [x] 3.4 新建 `src-tauri/src/docx/media.rs`：从 `assets_dir` 读图片字节，按 96dpi → EMU 等比缩到正文可用宽度（15 cm）以内，单图上限 5 MB，读不到或超限返回占位（alt，缺省 `[图片缺失]`/`图片过大未嵌入`）；验证：单测覆盖「文件存在」「文件缺失」「超 5 MB」三条路径，断言占位文案与图片关系表条目（`cargo test --offline docx::media` → 6 passed，覆盖 PNG/GIF/BMP/JPEG 尺寸探测与占位文案）
- [x] 3.5 新建 `src-tauri/src/docx/mod.rs` 把块模型流式写入 ZIP（图片先落 `word/media/image{n}.{ext}` 再写关系与引用），导出 `pub fn build(blocks: &[ExportBlock], assets_dir: &str) -> Result<Vec<u8>, String>`；验证：单测生成一段含各类块与一张真实小 PNG 的文档，按 ZIP 读回断言必需条目齐全、`document.xml` 含正文文字与图片引用、条目顺序与关系表一致（`cargo test --offline docx::` → 30 passed；条目清单 `[Content_Types].xml`/`_rels/.rels`/`document.xml`/`document.xml.rels`/`styles.xml`/`numbering.xml`/`media/image1.png`，EOCD 在末尾 22 字节内，rels 里 `rIdM1` 与 `rIdL100` 不冲突）

## 4. 命令与契约

- [x] 4.1 在 `src-tauri/src/commands.rs` 新增 `export_kb_docx(state, item_id, target_path, blocks)`：锁内 `ensure_writable()`、确认条目存在、取 `assets_dir` 后立刻放锁；校验绝对路径与父目录、必要时补 `.docx`、同目录临时文件 + rename；在 `src-tauri/src/main.rs` 的 `generate_handler![]` 注册；验证：`cargo check` 通过，`cargo test export_kb_docx` 覆盖「条目不存在」「父目录不存在」「相对路径」「正常写入」四条并断言失败时不留临时文件（`cargo check --offline` exit 0；`cargo test --offline export_kb_docx` → 4 passed；写入用 `.<名>.tmp` + 旧文件先转 `.bak` 再换名，失败回滚，测试断言不留 `.tmp`/`.bak`）
- [x] 4.2 同步命令契约：`docs/ARCHITECTURE.md` 附录 §2 增 `export_kb_docx` 一行、§10 计数 75 → 76，`test/command-parity.test.mjs` 总数与 `handler.has('export_kb_docx')` 断言同步；验证：`node test/command-parity.test.mjs` 与 `node test/docs.test.mjs` 全通过（parity 6/6、docs 4/4；README 四处计数同步 76，`grep` 已无「75 个 Tauri 命令」残留）
- [x] 4.3 `README.md` 补「导出为 Word」小节：入口位置、默认下载目录、保真范围、已知不保真（任务列表方括号、代码不高亮、WebP 老版本 Word 可能空白）、PDF 尚未支持；验证：`node test/docs.test.mjs` 通过，且 README 的导出说明与 spec 的保真清单逐条对得上（docs 4/4；功能一览也补了「当前文档一键导出 Word」）

## 5. 集成验证

- [x] 5.1 跑 `npm run check`（沙箱需 danger-full-access），21 项全绿、0 FAIL；验证：日志末行为「门禁全绿，可以提交。」（`logs/check-20261117-233944.log`：21 项全 OK、0 FAIL、0 SKIP，末行「门禁全绿，可以提交。」；此前 23:36 那轮因测试写 `%TEMP%` 的 ACL 拒绝红 13 项，已把测试临时目录改为 `test_dir` 回退 `target/docx-test-tmp` 后复跑）
- [ ] 5.2 真机冒烟五条：Word 打开导出文件无「内容有问题」提示；取消对话框不产生文件；同名文件覆盖确认生效；删除一张引用图片后仍能导出且出现占位；只读目录导出报中文错误且不留半截文件
- [ ] 5.3 导出前后对比知识库数据：`notes/<id>.md` 与 `assets/` 的修改时间不变、`notes.json` 未变；验证：导出前后各取一次文件时间戳与 `notes.json` 哈希，断言一致

## Workflow follow-up

- 全部任务完成、用户真机验收后归档本 change，并核对 `openspec validate --all --strict`。
- PDF 导出单独立 change，等 `webview2-com` 可升级到带 `ICoreWebView2_16` 的版本再评估。
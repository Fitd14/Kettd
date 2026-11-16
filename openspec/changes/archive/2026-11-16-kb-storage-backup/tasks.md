# Tasks

> 约定：所有涉及真实文件读写的测试/冒烟 MUST 指向临时数据目录（`KETTD_DATA_DIR`），禁止写生产数据；每批末尾跑本批门禁，最终批跑全量门禁。命令数 68 → 71（`export_backup`、`import_backup`、`register_kb_hotkey`），每新增一条命令的同一任务内完成四方同步。

## 1. 纯 Rust 基础组件（无 IO）

- [x] 1.1 新增 `src-tauri/src/digest.rs`：手写 SHA-256（`sha256_hex(&[u8]) -> String`）与 base64 编码/解码、data URL 解析（`data:image/<mime>;base64,<payload>`），不引入新 crate；补标准测试向量（空串、`"abc"`、长串、含 padding/无 padding、非法输入）；验证：`cargo test digest` 全绿，含 `sha256("abc") == "ba7816bf..."` 断言。
- [x] 1.2 新增 `src-tauri/src/backup.rs` 纯函数计划器：定义 `MergeAction{Add,Update,Skip,Conflict}`、`MergePlan` 与 `plan_kb / plan_tasks / plan_reminders / plan_folders`（只吃内存数据，不碰 IO），按 id + `updatedAt` + 内容指纹决策；单测覆盖「新增 / 备份较新 / 本地较新 / 指纹相同 / 冲突 / 备份旧条目」六类矩阵；验证：`cargo test backup::` 全绿。
- [x] 1.3 统一中文错误常量与用户可见文案（备份格式不支持、文件过大、资源校验失败、知识库升级失败等），不包含路径/堆栈；验证：单测断言常量非空且不含 `\` 或 `/` 路径片段，`cargo test` 通过。

## 2. 存储拆分与迁移（S1）

- [x] 2.1 `src-tauri/src/models.rs`：新增 `KbItemMeta`（`id/title/tags/folder_id/order/pinned/created_at/updated_at/content_hash`，`notes.json` 用）与 `KbItemMeta` 的 serde camelCase 形状；`KbItem` 增加 `content_hash: Option<String>`（`skip_serializing_if` 兼容 IPC 旧类型或保持必填需同步 api.ts）；验证：`cargo test models` 序列化快照断言 camelCase 字段，`npx tsc -b` 通过。
- [x] 2.2 `src-tauri/src/ports/store_backend.rs`：新增 `read_note_body / write_note_body / delete_note_body`、`asset_exists / read_asset / write_asset`、`write_export_file / create_backups_dir`、`archive_notes_v1 / read_notes_v1_archive`；`FsBackend` 全部走 tmp + rename 原子写并自动建目录；`InMemoryBackend` 同步实现；验证：`cargo test` 编译通过 + 新增 InMemory/Fs 往返单测（临时目录）。
- [x] 2.3 `src-tauri/src/infra/fs_store.rs`：`read_notes/write_notes` 切换为「索引 meta 数组」；保留旧格式探测所需能力（读取时先按 `Vec<Value>` 解析）；`notes.json` 写前仍轮转 5 份；验证：新增单测「写入索引后文件不含 `bodyMd`；旧格式文件读取可识别」。
- [x] 2.4 `src-tauri/src/store.rs`：`load_kb/reload_kb_items` 改为「读索引 → 逐条读正文 → 校验 `contentHash`」，正文缺失降级为空串、hash 不一致以正文为准修正内存；`save_notes` 只写索引；新增 `save_kb_item_body(id, body)` 原子写正文；验证：单测「保存→重载往返正文一致 / 正文缺失不判损坏 / hash 篡改被正文纠正」。
- [x] 2.5 `src-tauri/src/commands.rs`：`add/update/delete_kb_item` 改为先写/删正文再轮转并写索引；删除时清理正文文件；迁移 `get_kb_items` 增加可选 `ids: Option<Vec<String>>`（缺省仅元数据、传 id 回完整正文），保持旧调用兼容；验证：单测「add 后 notes.json 无正文、notes/<id>.md 存在」「delete 后正文文件消失」「get_kb_items() 的 bodyMd 为空、ids 版本有正文」。
- [x] 2.6 `src-react/src/lib/api.ts` + 消费点：`KbItem` 类型保持兼容；`getKbItems(ids?)` 封装；`KbItemDetail`、`StickyWindow`、`kb-attach-picker`、`SettingsView`、`task-detail` 复核并在需要正文处按 id 取；验证：`npx tsc -b` 通过 + 真机打开条目详情/知识便签正文完整。
- [x] 2.7 迁移器（`store.rs`）：旧格式探测（元素含 `bodyMd`/`body_md`）→ 写 `notes.v1.json` 永久存档（不存在才写）→ `rotate_notes_backups` → 逐条抽图片/写正文 → 最后原子写索引；失败保留旧 `notes.json` 并置专属中文 `kb_error`；`restore_kb_backup` 复用同一迁移；验证：单测（旧格式→新格式条目数/标题/正文一致；重复执行幂等；注入只读/写失败不覆盖旧文件；旧 bak 恢复后自动迁移；`notes.json` 损坏仍报 `KB_CORRUPT_MESSAGE`）。
- [x] 2.8 性能基准：构造 5000 条临时知识库（临时目录），测启动加载与列表 IPC 序列化耗时，记录到交付报告；若启动加载超过预算（目标 1.5s 内），在 design 的备选方案（惰性读正文）上标注并上报；验证：`cargo test --release kb_bench_5000 -- --nocapture` 输出实测值。
- [x] 2.9 文档：`docs/ARCHITECTURE.md` 数据文件/存储说明、`README.md` 数据文件表、`docs/capability/09-数据存储与安全.md` 更新「notes.json 索引 + notes/ + assets/ + notes.v1.json」；验证：`npm run check` 中文档结构项通过。
- [x] 2.10 S1 门禁：`cargo check` + `cargo test` + `npx tsc -b`；用临时 `KETTD_DATA_DIR` 手工演练一次真实迁移并记录前后条目数；验证：命令真实输出贴入交付报告。

## 3. 图片外置与渲染（S2）

- [x] 3.1 `src-tauri/src/kb.rs`：新增 `extract_images(body) -> (body, assets)`，把 data URL 抽为 `assets/<sha256>.<ext>`（MIME 白名单 png/jpg/webp/gif）、保留 alt、按 sha 去重、格式异常保留内联并计数；接入 `save`/迁移路径；验证：单测覆盖「单图 / 多图 / 重复图去重 / 非白名单 MIME 不抽取 / 非法 base64 保留原样」。
- [x] 3.2 `Bootstrap` 增加 `assets_dir` 字段（`src-tauri/src/models.rs` + `commands.rs` 组装 + `api.ts` 类型）；验证：`cargo test` + `npx tsc -b` 通过，真机 Bootstrapping 数据含绝对资源目录。
- [x] 3.3 前端渲染：新增 `src-react/src/rendering/asset-url.ts`（`resolveAssetUrl(src, assetsDir)`，仅处理 `assets/` 前缀、`window.__TAURI__.tauri.convertFileSrc` 惰性取、非 Tauri 原样返回），接入 `image-extension.ts` 的 `renderHTML` 与 Markdown 静态渲染；验证：`npx tsc -b` + 真机粘贴图片显示正常。
- [x] 3.4 `src-tauri/tauri.conf.json`：`allowlist` 增加 `"protocol": { "asset": true, "assetScope": ["$APPDATA/todo-list/assets/**"] }`；验证：`cargo check`/启动不报配置错误，真机重启后旧图片仍显示；文档记录 `KETTD_DATA_DIR` 自定义目录下图片不显示的已知限制。
- [x] 3.5 文档：`README.md` 数据文件表与图片说明、`docs/capability/06-知识库.md` 存储/图片段、`docs/capability/09` 资源目录；验证：`npm run check` 文档结构项通过。
- [ ] 3.6 S2 门禁：`cargo test` + `npx tsc -b`；真机专项：「粘贴新图→保存→重启→仍显示」「两条目复用同一图片→assets 只有一份」「删除后其他条目图片不坏」。

## 4. 手动备份与增量恢复（S3）

- [x] 4.1 `backup.rs`：实现 `.kettdbackup` v1 结构序列化/反序列化与校验（`format`、`formatVersion=1`、`appVersion`、`exportedAt` 本地语义字符串、`data`、`kb{items,folders}`、`assets[{sha256,ext,base64}]`），非法格式/未知版本/超限给中文错误；单测往返 + 版本拒绝；验证：`cargo test backup` 全绿。
- [x] 4.2 新增命令 `export_backup`（async + `spawn_blocking`）：组装当前内存数据与引用资源、写 `<数据目录>/backups/kettd-backup-<YYYYMMDD-HHmmss>.kettdbackup`、返回 `{path, bytes, itemCount, assetCount, skipped}`；同步 `main.rs` 注册、`commands.rs` 定义、`api.ts` 封装、`ARCHITECTURE.md` §2 行 + §10 计数（69）、`test/command-parity.test.mjs` 断言 69、README 计数同步；验证：`node --test test/command-parity.test.mjs` + 真机导出文件存在且可被 `serde_json` 解析。
- [x] 4.3 新增命令 `import_backup(payload: String, dry_run: bool)`（async）：dry-run 只返回 `ImportReport`（新增/更新/冲突/跳过/资源新增/复用 + 冲突标题列表）不写盘；apply 复用 1.2 计划器，应用前 `rotate_backups` + `rotate_notes_backups` 各一次，按「资源→正文→文件夹→索引→data→runtime」顺序 `write_in_place` 落盘，冲突生成「（导入冲突）」副本、提醒冲突保留本地、本地独有绝不删除、settings/stickies 不覆盖，完成后 emit `store-changed`；四方同步（70）；验证：单测覆盖合并矩阵 + 「dry-run 前后目录 mtime/内容不变」+「重复导入第二次全跳过」；parity 70 通过。
- [x] 4.4 `SettingsView.tsx` 备份区：导出按钮（成功提示路径 + 复用打开数据文件夹）、导入 `<input type="file" accept=".kettdbackup">` → `file.text()` → dry-run 预览对话框（计数 + 冲突标题）→ 确认后 apply → 结果提示；`api.ts` 类型与封装；验证：`npx tsc -b` + 真机完整走一遍「导出→预览→取消无副作用→确认」。
- [x] 4.5 备份往返演练（临时目录 + 真机）：导出 → 本地新增/修改/删除条目 → dry-run 核对计数 → 导入 → 逐项核对「备份较新被更新、本地较新跳过、冲突副本出现、本地独有保留、settings 未变、重复导入全跳过」；验证：把核对结果表贴入交付报告。
- [x] 4.6 文档：`README.md` 备份与恢复段（手动导出/导入/预览/不覆盖设置）、`docs/capability/06-知识库.md` 备份段、`docs/capability/09` 备份格式、`docs/ARCHITECTURE.md` 命令说明；验证：`npm run check` 文档结构项通过。
- [x] 4.7 S3 门禁：`cargo test` + `npm run check`（12 项，此时 parity 70）+ 真机导出导入演练；验证：日志 `logs/check-<时间戳>.log` 贴入报告。

## 5. 搜索收窄与直达知识库热键（S4）

- [x] 5.1 `src-tauri/src/kb.rs`：`search(items, folders, query) -> Vec<String>` 改为只匹配标题 + 祖先文件夹名（防环上限 64），打分表（标题 100/50/20、文件夹 8/4/2、取祖先最高分不累加），同分 `updatedAt` 倒序、去重，空查询全量倒序；删除 tags/body 匹配路径；验证：单测覆盖「正文命中不返回 / 标签命中不返回 / 文件夹命中 / 标题优先于文件夹 / 完全匹配优先 / 重名去重 / 大小写 / 空查询」。
- [x] 5.2 `commands.rs` `search_kb` 返回 `Vec<String>`；`api.ts` 类型；`KbView` 用 id 过滤并保留面包屑，`localKbMatch` 改为标题 + 文件夹路径匹配，无结果空状态，标签筛选独立链路不变；验证：`npx tsc -b` + 真机搜索（正文关键词不出结果、文件夹名出结果、路径面包屑可见、清空恢复全量）。
- [x] 5.3 后端热键：`Settings.kb_hotkey: Option<String>`（`#[serde(default = "default_kb_hotkey")]` 默认 `Alt+Shift+K`，`coerce` 空串→`None`）、`HotkeyStatus.kb`、`HotkeyAction::OpenKb`、`HotkeyRegistry` 第 5 槽、`register_kb_hotkey` 命令（复用注册/互斥/回滚）、`main.rs` 启动注册顺序扩到 5 组、`bind_global_hotkey` OpenKb → `show_main(Some("/kb"))`、`open_main_window(route: Option<String>)`；验证：`cargo test`（含「五槽互斥」「失败回滚旧键」「旧配置缺省默认值」「open_main_window 带 route」单测）。
- [x] 5.4 命令四方同步 `register_kb_hotkey`（70→71）：`main.rs`/`commands.rs`/`api.ts`/`ARCHITECTURE.md` §2 + §10、`test/command-parity.test.mjs` 断言 71、README 所有 64/68 陈旧计数统一为 71；验证：`node --test test/command-parity.test.mjs` 通过 + `node --test test/docs.test.mjs` 通过。
- [x] 5.5 前端：`App.tsx` 增加 `onEvent('navigate')` 监听（白名单校验后切 hash）、`SettingsView.tsx` 第 5 个热键录入项（清空/恢复默认/绑定状态）、`api.ts` 的 `registerKbHotkey`/`HotkeyStatus.kb`/`openMainWindow(route)` 生效；验证：`npx tsc -b` + 真机「隐藏主窗→按 Alt+Shift+K→主窗显示且停在知识库」。
- [x] 5.6 文档：`README.md` 热键表加直达知识库、`docs/capability/06-知识库.md` 搜索段（范围/排序/标签独立）与热键段、`docs/ARCHITECTURE.md` 搜索与热键说明；验证：`npm run check` 文档结构项通过。
- [ ] 5.7 S4 门禁：`cargo test` + `npm run check`（`scripts/check.ps1` 真实 17 步，「12 项」是最早旧口径；parity 71）+ 真机搜索/热键专项；验证：日志与实测输出贴入报告。

## 6. 集成验收与交付

- [x] 6.1 跑 `npm run check` 全量（`scripts/check.ps1` 真实 17 步；落笔时 12 项）并贴日志路径与「N OK + M SKIP + 0 FAIL」，SKIP 项单列原因；验证：`logs/check-<时间戳>.log` 原文摘录。
- [x] 6.2 跑 `cargo test`（原有 103 项 + 新增）并贴总数与失败数；验证：命令输出。
- [x] 6.3 命令四方对账：`node --test test/command-parity.test.mjs`、`node --test test/docs.test.mjs`、`node --test test/packaging.test.mjs` 全绿，确认 71↔71↔71 与 §2 表格含三条新命令。
- [ ] 6.4 L4 真机五条 + 本 change 专项清单（首启迁移、图片重启、备份往返、搜索口径、直达热键、降级演练），逐条记录操作与结果；验证：交付报告「验证证据」段，UI 类附截图。
- [x] 6.5 降级演练：备份现场 → 用 `notes.v1.json`（或 `notes.json.bak-1`）覆盖回 `notes.json` → 确认旧格式可被旧逻辑解析（或临时构建旧版本冒烟），记录副作用；验证：演练记录 + 数据文件校验。
- [x] 6.6 交付报告按 AGENTS §4.5 五段输出（改动摘要/验证证据/未验证项与原因/影响面与回滚/遗留 TODO），并更新 `ledger/` 对应任务状态；文档类改动按 §4.6 待人工审核后再 commit/push；验证：报告链接 + 台账状态。

## Workflow follow-up

- 用户确认规划与待拍板问题后，按本清单进入 apply；全部勾选、门禁全绿且人工验收通过后执行 `openspec archive kb-storage-backup`。
- 备份/热键等 A 类文档改动遵循 AGENTS §4.6：先给内容待人工审核，批准前不 commit、不 push。

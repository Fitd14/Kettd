# Proposal

## Why

- **知识库单文件存储已近临界点**：`notes.json` 是单个 `Vec<KbItem>`，正文 `bodyMd` 与图片 data URL 全部内联（单图 ≤300KB、单条 ≤2.5MB、上限 5000 条）。每次保存都要整文件序列化、整文件轮转 5 份备份（`src-tauri/src/store.rs:1047`、`src-tauri/src/infra/fs_store.rs:132`），文件体积和写放大随内容线性膨胀，正是用户所说的「文章信息过多会信息爆炸」。
- **备份完全由软件自动完成，用户没有主动权**：现在只有 `data.json` 写前轮转 5 份、设置页整文件恢复（`src-tauri/src/store.rs:1085`）；知识库没有手动导出，也没有「比对后增量恢复」的通道。用户希望自己整理、自己保存一个文件，换机或归档时能合并回来，而不是整库覆盖。
- **全局知识库搜索与用户预期不符**：后端 `kb::search` 对标题 +2、标签 +1、正文 +1 打分（`src-tauri/src/kb.rs:94`），实际是「全文搜索」；用户只希望按**文件名称与文件夹目录名称**找文件。文件夹名完全不参与匹配，正文命中带来大量噪音，而知识库列表本身已带全量条目，后端再回传全文纯属浪费。
- **缺少「直达知识库」的快捷键**：主窗热键 Alt+Shift+O 只能打开主界面，`show_main` 虽然会 `window.emit("navigate", route)`（`src-tauri/src/runtime.rs:145-159`），但前端 `App.tsx` 从未监听，`navigate` 事件实际无人消费（`src-react/src/App.tsx`）。

## What Changes

1. **知识库存储拆分（D1）**：正文改为 `notes/<id>.md` 独立文件，`notes.json` 退化为元数据索引（`id/title/tags/folderId/order/pinned/createdAt/updatedAt/contentHash`）；列表 IPC 默认只回元数据，正文按需读取（复用 `get_kb_items` 的可选 `ids` 参数，不新增命令）。加载时对旧格式（内联 `bodyMd`）做一次性、幂等迁移，迁移前先轮转 notes 备份。
2. **图片外置与去重（D2）**：图片从 `bodyMd` 的 data URL 抽取为 `assets/<sha256>.<ext>`（内容寻址，天然去重），正文改存相对引用 `assets/<sha256>.<ext>`；前端粘贴/压缩流程不变，渲染层统一把相对引用解析为 `convertFileSrc` 可显示地址；备份包必须包含 assets。
3. **手动备份与增量恢复（D3/D4）**：新增 `.kettdbackup` 单文件格式（JSON，含 `data.json` 内容 + 知识库元数据/正文 + 文件夹 + assets 的 base64）；`export_backup` 导出到数据目录 `backups/kettd-backup-<时间戳>.kettdbackup` 并返回路径；`import_backup` 先 dry-run 预览（新增/更新/冲突/跳过/资源计数），确认后按 id + `updatedAt`（相等再比 `contentHash`）增量合并，冲突另存「（导入冲突）」副本，**绝不删除**备份中不存在的本地条目，应用前先轮转备份。
4. **全局搜索收窄（D5）**：后端只匹配标题与祖先文件夹名，标题精确 > 前缀 > 包含，文件夹名低权重；前端继续做命中过滤并显示文件夹路径面包屑，标签筛选保留为独立过滤。
5. **直达知识库热键（D6）**：新增独立全局热键 `kbHotkey`（默认 `Alt+Shift+K`，允许清空），注册失败回滚并给中文提示；命中后显示主窗并导航到 `#/kb`，前端 `App.tsx` 监听 `navigate` 事件完成切页；顺带补齐 `open_main_window` 的 `route` 参数（`api.ts` 已传但后端忽略）。
6. **迁移与兼容（D7）**：迁移失败保留旧数据与中文错误、下次启动重试；`notes.json` 无法解析时损坏通道语义不变（`KB_CORRUPT_MESSAGE`，不渲染假空态）；旧 `notes.json.bak-N` 仍可读取/恢复（恢复时走迁移）。

## Capabilities

### New Capabilities

- `knowledge-base/storage`: 知识库分文件存储、元数据索引、图片内容寻址外置、旧数据迁移与损坏/回滚语义。
- `knowledge-base/backup-restore`: `.kettdbackup` 单文件手动导出、dry-run 预览、按 id/updatedAt 的增量合并、冲突副本与「不删除本地」保证。
- `knowledge-base/search`: 全局知识库搜索范围（仅标题与文件夹名）、排序权重、前端过滤与路径面包屑、标签独立筛选。
- `app-shell/shortcuts`: 直达知识库全局热键的设置、注册/互斥/回滚、主窗显示与 `navigate` 导航。

### Modified Capabilities

（无：本项目 `openspec/specs/` 目前只有 `.gitkeep`，尚无已归档的主规格；本次四个能力均以新增能力记录。）

## Impact

- **前端**：`src-react/src/App.tsx`（监听 `navigate`）、`views/KbView.tsx`（搜索范围、元数据列表 + 正文按需加载、导入预览入口）、`components/kb/kb-item-detail.tsx`、`sticky/StickyWindow.tsx`（按需取正文）、`rendering/image-extension.ts`（相对引用解析渲染）、`lib/api.ts`（命令封装与类型）、`views/SettingsView.tsx`（导入导出、直达知识库热键）。
- **后端**：`src-tauri/src/kb.rs`（新搜索、正文/图片抽取）、`src-tauri/src/store.rs`（加载/保存/迁移/恢复）、`src-tauri/src/commands.rs`、`src-tauri/src/runtime.rs`（第 5 个热键槽）、`src-tauri/src/main.rs`、`src-tauri/src/models.rs`、`src-tauri/src/infra/fs_store.rs`、`src-tauri/src/ports/store_backend.rs`（含 `InMemoryBackend`）。
- **命令契约**：68 → 71（新增 `export_backup`、`import_backup`、`register_kb_hotkey`；`get_kb_items` 增加可选 `ids`；`search_kb` 返回 id 列表）。按 AGENTS §2.4 四方同步：`main.rs` / `commands.rs` / `api.ts` / `docs/ARCHITECTURE.md` §2 表格 + §10 计数，并同步 `test/command-parity.test.mjs` 断言。
- **数据**：`notes.json` 结构升级为索引；新增 `notes/`、`assets/`、`backups/` 目录；迁移前轮转 `notes.json.bak-1..5`；旧格式 `notes.json.bak-N` 继续可恢复。
- **依赖**：不新增 Rust crate 与 npm 依赖（SHA-256、base64、data URL 解析以仓内纯 Rust 小模块 + 单测实现）；Tauri 仍为 v1.8，仅在 `tauri.conf.json` 增加 `allowlist.protocol.asset` + `assetScope` 配置。
- **文档**：`docs/ARCHITECTURE.md` §2/§10、`README.md`（热键表、数据文件表、备份说明、陈旧命令数 64/68）、`docs/capability/06-知识库.md`、`docs/capability/09-数据存储与安全.md`。
- **验证**：`npm run check`（12 项，日志 `logs/check-<时间戳>.log`）+ `cargo test` + 命令 parity + 备份往返演练 + L4 真机五条（含「导出→改内容→dry-run→导入→逐条比对」）。
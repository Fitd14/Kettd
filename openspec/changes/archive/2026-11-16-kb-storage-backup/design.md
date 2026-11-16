# Design

## Context

动机与范围见 `proposal.md` 与四个 spec：`specs/knowledge-base/storage/spec.md`、`specs/knowledge-base/backup-restore/spec.md`、`specs/knowledge-base/search/spec.md`、`specs/app-shell/shortcuts/spec.md`。这里只记录现状与约束。

**存储现状**

- `src-tauri/src/kb.rs:18` `NOTES_FILE_MAX_ITEMS = 5_000`；`KbItem` 定义在 `src-tauri/src/models.rs:607-644`，正文 `bodyMd` 与图片 data URL 一起内联在 `notes.json` 的单个 `Vec<KbItem>` 里（`src-tauri/src/store.rs:810` `reload_kb_items`）。
- 每次保存知识库都整文件轮转 5 份再写：`src-tauri/src/store.rs:1047` `save_notes`（`rotate_notes_backups` + `write_notes`），底层 `src-tauri/src/infra/fs_store.rs:132` `rotate_notes_backups`、`write_data_at` 为 tmp + rename 原子写。`folders.json` 独立且**不轮转**（`src-tauri/src/store.rs:1057`）。
- 图片粘贴链路：`src-react/src/rendering/paste-image.ts` 剪贴板 → canvas 压缩（单图 data URL ≤300KB、单文档 ≤2.5MB、长边 1600、WebP 质量 0.82–0.5）→ `rendering/image-extension.ts` 把 data URL 写进 Tiptap image 节点 → tiptap-markdown 序列化成 `![alt](data:image/...)` → 直接进 `notes.json`。
- 损坏通道：`src-tauri/src/store.rs:796-807` `load_kb` 在 notes/folders 任一解析失败时清空集合且置 `kb_error = KB_CORRUPT_MESSAGE`；AGENTS §2.3 要求不渲染假空态。`src-tauri/src/ports/store_backend.rs` 的 `StoreBackend` trait 含全部持久化操作，文件底部 `InMemoryBackend` 是测试替身，新增方法必须同步实现。

**备份现状**

- 只有写前自动轮转：`src-tauri/src/store.rs:1031` `save`（`data.json` bak-1..5）、`src-tauri/src/infra/fs_store.rs:132-158`；设置页整文件恢复 `src-tauri/src/store.rs:1085` + `src-tauri/src/commands.rs:894`；知识库备份列表/恢复 `src-tauri/src/store.rs:1411-1453`、`src-tauri/src/commands.rs:1093-1112`。没有任何手动导出/导入通道，也没有增量合并。

**搜索现状**

- `src-tauri/src/kb.rs:94-124` `search(items, query)`：`title.contains` +2 / 任一 tag +1 / `body.contains` +1，按分数降序、同分 `updated_at` 降序；空查询返回全量。注释写「tag ×1.5」但代码是 +1（注释与实现不一致）。
- 列表命令 `src-tauri/src/commands.rs:930` 直接 `store.kb.clone()`，正文全量走 IPC；`search_kb`（`src-tauri/src/commands.rs:1013`）再 clone 命中条目全文。
- 前端 `src-react/src/views/KbView.tsx:493-518` 已按 `searchKb` 命中集合过滤，`components/kb/kb-tree-utils.ts:240-264` `flattenResults` 已给每行 `pathLabel`，`components/kb/kb-tree.tsx:647-668` 已渲染路径面包屑；但 `localKbMatch`（`KbView.tsx:270-276`）仍在匹配 title + tags + bodyMd，文件夹名不参与。

**热键现状**

- `Settings` 在 `src-tauri/src/models.rs:287-325`，已有 `capture_hotkey`/`main_hotkey`/`sticky_hotkey`/`todo_hotkey`；`HotkeyStatus` 四个字段在 `src-tauri/src/models.rs:853-860`。
- `src-tauri/src/runtime.rs:115-121` `HotkeyAction { Capture, OpenMain, NewSticky, ToggleTodoFloat }`；`register_global_hotkey`（`src-tauri/src/runtime.rs:1114-1145`）遍历这四槽做互斥与失败回滚；`bind_global_hotkey`（`:1147-1197`）把 OpenMain 绑成 `show_main(&handle, None)`；`show_main`（`:145-159`）在窗口存在时 `window.emit("navigate", path)`，`restore_main_window`（`:165-197`）重建窗口后也尽力 emit。
- 前端 `src-react/src/App.tsx` 只监听 `store-changed`，**没有任何 `navigate` 监听**；`src-react/src/lib/api.ts:429` `openMainWindow(route?)` 传了 `{ route }`，但后端 `src-tauri/src/commands.rs:720-723` 的 `open_main_window(app)` 根本没有该参数（死参数）。
- 启动注册在 `src-tauri/src/main.rs:207` 起的 setup 线程：读 4 组组合、sleep 400ms、逐组注册，失败 `notify_title` 中文提示。

**全局约束**

- Tauri 冻结 v1（`src-tauri/Cargo.toml` tauri = "1.8"，实际 1.8.3；`api-all` 已含 `protocol-all`→`protocol-asset`）。
- 不新增 Rust/npm 依赖：`src-tauri/Cargo.toml` 只有 serde/serde_json/chrono 等，**没有 base64、sha2、hex、regex**；`src-react/package.json` 无 `@tauri-apps/api`，前端一律走 `window.__TAURI__` 惰性桥（`src-react/src/lib/api.ts:296-299`）。
- 时间只用本地语义字符串（`YYYY-MM-DDTHH:mm` 等），禁止 UTC/ISO `Z`；持久化一律走 `StoreBackend` 原子写与既有轮转；数据目录默认 `%APPDATA%\todo-list`，`KETTD_DATA_DIR` 可覆盖。
- 命令契约四方同步（`main.rs` / `commands.rs` / `api.ts` / `docs/ARCHITECTURE.md` §2 + §10），`test/command-parity.test.mjs` **落笔时**硬断言 68 条（本 change 落地 71 条后已更新）；`npm run check` 落笔时 12 项，现为 `scripts/check.ps1` 的 **17 步**。
- 知识库 AI 混合检索已废弃，不重新引入 embedding/hybrid。

## Goals / Non-Goals

**Goals:**

- 让 `notes.json` 只承载元数据，单条文章正文与图片不再放大整库读写与备份轮转。
- 图片内容寻址、去重、可被备份完整带走，前端渲染透明。
- 用户可手动导出单文件备份，导入前预览、导入时增量合并、冲突留副本、绝不删本地。
- 搜索行为与用户预期一致：只按文件名与文件夹名找文件，结果可解释。
- 新增直达知识库热键，失败可回滚，导航链路真正接通。
- 旧数据一次性幂等迁移，迁移前留档；命令数增长控制在最小集合内。

**Non-Goals:**

- 不改 `data.json` 的 schema、任务/提醒/便签模型与既有自动轮转策略。
- 不做定时自动备份、云同步、加密备份、压缩打包（`.zip`）；`.kettdbackup` 就是单个 JSON 文件。
- 不改前端图片压缩参数与单条/单文档上限；不做图片缩略图/懒加载。
- 不做资源孤儿文件自动回收（删除条目后 `assets/` 允许暂时留存）。
- 不引入全文/标签搜索，不恢复 AI 检索；标签筛选维持现状。
- 不升级 Tauri/TypeScript，不新增依赖。
- 不支持新格式 `notes.json` 被旧版本应用正确读取（降级需走回滚步骤，见 Migration Plan）。

## Decisions

### D1 知识库存储拆分：索引 + 正文分文件

**做法**

- `notes.json` 改为索引数组，元素为 `KbItemMeta { id, title, tags, folderId, order, pinned, createdAt, updatedAt, contentHash }`；正文写 `notes/<id>.md`。
- `Store` 内存仍持有完整 `Vec<KbItem>`（载入时读索引 + 逐个读正文），知识库列表 IPC 只回元数据：扩展 `get_kb_items`，新增可选参数 `ids: Option<Vec<String>>`——缺省返回元数据（`bodyMd` 为空串），传 `ids` 时只返回这些 id 的完整正文。**不新增 `get_kb_item` 命令**，遵守「命令数最小且兼容现有前端」。
- 写路径：命令层先 `write_note_body` 原子写正文，再 `save_notes` 只写索引（仍然写前轮转 `notes.json` 5 份）。删除条目时同时 `delete_note_body`。
- `contentHash = sha256(规范化正文)`；载入时若正文 hash 与索引不一致，以正文为准修正内存索引（下次保存落盘），视为可修复不一致，不判整库损坏。
- 正文文件缺失：该条正文按空串处理并在内存标记，不丢条目、不判损坏。

**理由**

- 真正的痛点是写放大与轮转体积：现在改一条笔记 = 序列化全部正文/图片 + 复制 5 份整文件；拆分后索引小、正文单写，问题消失。内存保留正文可让列表/详情/搜索/迁移/导出沿用现有纯函数与 `store.kb` 语义，改动面最小。
- 惰性只做在 IPC 边界，正好对应用户能感知的「列表卡顿、payload 大」，不引入跨层异步读取复杂度。

**备选**

- 内存只存元数据、正文每次落盘读：内存更省、启动 IO 更少，但 `store.kb` 现有语义（搜索/更新/恢复/测试都依赖带正文的 `Vec<KbItem>`）要大改，且详情/便签每次 IO。放弃。
- 新增 `get_kb_item`：契约更清晰，但命令 68→72、四方同步与文档成本更高；`get_kb_items(ids)` 已满足需求。放弃（若评审偏好清晰契约可改回，见 Open Questions）。

### D2 图片内容寻址外置

**做法**

- 保存时后端（`kb.rs` 新增 `extract_images(body)`）扫描 `](data:image/<mime>;base64,<payload>)`，base64 解码后 `sha256` 命名 `assets/<sha256>.<ext>`（ext 白名单 `png`/`jpg`/`webp`/`gif`，由 MIME 决定）；正文改写为 `assets/<sha256>.<ext>`，保留 alt；同内容只写一份。
- 解码失败或 data URL 格式异常时保留原内联数据（宁可牺牲外置也不丢图），并计入迁移/保存 warning（不阻塞保存）。
- 渲染透明解析：`rendering/asset-url.ts` 提供 `resolveAssetUrl(src)`——仅当 `src` 以 `assets/` 开头时，用 `window.__TAURI__.tauri.convertFileSrc(<数据目录>/assets/<文件名>)` 转成可显示 URL；非 Tauri 环境原样返回。`image-extension.ts` 的 `renderHTML` 与 Markdown 静态渲染统一走该函数，编辑器节点属性仍存相对引用（序列化回 Markdown 不变），保存时无需反向转换。
- 数据目录经 `Bootstrap` 新增 `assetsDir` 字段下发（StickyWindow 已调 `getBootstrap`，KbView 已有 boot 数据），不新增命令。
- `tauri.conf.json` 的 `allowlist` 增加 `"protocol": { "asset": true, "assetScope": ["$APPDATA/todo-list/assets/**"] }`；无需 Cargo feature（`api-all` 已含 `protocol-asset`）。
- 备份包收集正文引用到的 assets 字节（base64）。

**理由**

- 内容寻址天然去重、文件名可校验（导入时重算 sha 可防篡改/损坏），且迁移只需按正文重算，不需要数据库。
- assetProtocol 是 Tauri v1 官方本地文件读取方式；`assetScope` 只放 assets 目录，避免把整个数据目录暴露给 WebView。

**备选**

- 直接 `fs.readBinaryFile` + blob URL：每次渲染都要 IPC + blob 生命周期管理，复杂且易泄漏。放弃。
- 继续内联但把图片单独存 `images/<id>` 并在内存拼 data URL：没有去重、IPC 依旧大。放弃。
- 用 `assetScope: ["$APPDATA/**"]`：更宽松、能覆盖 `KETTD_DATA_DIR`，但暴露面大。放弃；换取更小 scope，接受自定义数据目录下图片不显示的限制（见 Risks）。

### D3 手动备份导出

**做法**

- 新命令 `export_backup() -> Result<BackupExportResult, String>`（async，重活 `spawn_blocking`）：把当前 `data`（任务/提醒/设置/便签）、知识库 `{items(含正文), folders}`、被引用 `assets[{ sha256, ext, base64 }]` 组装为单个 JSON：
  `{ format: "kettd-backup", formatVersion: 1, appVersion, exportedAt: "YYYY-MM-DDTHH:mm", data, kb, assets }`。
- 写入 `<数据目录>/backups/kettd-backup-<YYYYMMDD-HHmmss>.kettdbackup`（本地时间，无 UTC），返回 `{ path, bytes, itemCount, assetCount, skipped }`；前端提示成功并提供「打开所在文件夹」（复用 `open_data_folder`）。
- 只导出正文引用到的资源；某条正文缺失按空、某资源读取失败跳过并计数，不整体失败。
- 导出不触发轮转、不写 `data.json`/`notes.json`。

**理由**

- 单文件 JSON 可直接审阅、可移植、实现最简；时间戳沿用项目本地语义规则。
- async + `spawn_blocking` 避免大备份阻塞 UI 与主线程锁（AGENTS 锁纪律）。

**备选**

- ZIP + `zip` crate：体积小但要新依赖，违反「无新依赖」。放弃。
- 导出到用户自选路径：Tauri v1 需要 dialog/fs 写权限与更多交互，先用数据目录 + 打开文件夹。放弃（可延后）。

### D4 导入预览与增量合并

**做法**

- 新命令 `import_backup(payload: String, dry_run: bool) -> Result<ImportReport, String>`（async）：`dry_run=true` 只解析 + 计算计划，绝不写盘；`false` 按同一计划落盘。参数在 JS 侧为 `payload`、`dryRun`（Tauri v1 默认 camelCase 映射）。
- 前端 `SettingsView` 用 `<input type="file" accept=".kettdbackup">` 读 `file.text()`，先 invoke `dryRun=true` 展示预览对话框（新增/更新/冲突/跳过/资源新增/复用计数 + 冲突标题列表），确认后同 payload 再 invoke `dry_run=false`。目前不做「大文件流式/base64 文件本体」——payload 就是文件文本，限制 256MB，超限返回中文错误。
- 合并规则（KB 条目、任务、提醒统一）：按 `id` 匹配；`updatedAt` 本地语义字符串可直接字典序比较；备份较新→覆盖；本地较新→跳过；相等→比内容指纹（KB 用 `contentHash` + 标题/标签/folderId 规范化 JSON；任务/提醒用序列化 JSON 的 SHA-256），相同跳过、不同冲突。
- 冲突副本：本地保留；备份版本以新 id、标题 `（导入冲突）` 后缀、`folderId` 跟随本地、时间戳取导入时刻写入；提醒冲突只保留本地跳过（避免双提醒）。
- 合并范围：任务、提醒、知识条目、文件夹、资源；**设置与便签**不覆盖（本地优先）。文件夹按 id 合并，父级缺失时上提到根；合并后按文件夹重排 `order`（去重密集排序）。
- 本地独有条目/文件夹绝不删除；备份缺字段保留本地值。
- 落盘顺序：资源 → 正文 → 文件夹 → 索引 → data.json → runtime；应用前显式 `rotate_backups` + `rotate_notes_backups`（各一次，避免 `save`/`save_notes` 二次轮转），写盘走 `write_in_place` / 新 store 方法，完成后 emit `store-changed`。
- 幂等：计划基于当前磁盘状态，重复导入同一文件第二次全部跳过。

**理由**

- 两步式（预览/应用）直接满足「比对后恢复」的用户预期，且预览不写盘可避免误操作。
- 以 `updatedAt` 为主、指纹兜底的规则简单可解释，符合「增量、不直接覆盖」；冲突副本保证不丢任何一方修改。

**备选**

- 以备份为全量覆盖：用户明确反对。放弃。
- 以本地为全量保留（只补缺失）：会丢备份侧更新。放弃。
- 导入设置：会把别机热键/主题/数据目录带过来，风险高。放弃（列为待确认）。

### D5 搜索收窄为文件名 + 文件夹名

**做法**

- `kb::search(items, folders, query) -> Vec<String>`（返回 id）：大小写不敏感、去首尾空白。匹配集合 = 标题 + 所有祖先文件夹名（防环，深度上限 64）。
- 打分：标题完全相等 100 / 前缀 50 / 包含 20；文件夹名完全相等 8 / 前缀 4 / 包含 2（取所有祖先中的最高分，**不累加**，防止深层路径虚高）；同分 `updatedAt` 倒序、再标题升序；结果按 id 去重。空查询返回全量 id（`updatedAt` 倒序，保持浏览语义）。
- 后端不读正文、不匹配 tags；`search_kb` 返回 `Vec<String>`。
- 前端：`api.ts` `searchKb` 返回 id 数组；`KbView` 用 id 集合真正过滤（现行结构不变），`localKbMatch` 改为「标题 + 祖先文件夹名」，保证后端不可用时兜底口径一致；路径面包屑沿用 `flattenResults`；标签筛选保持独立 `tagFiltered` 链路；无结果显示空状态文案。

**理由**

- 用户诉求就是「按文件名/文件夹名找」，范围收窄即去噪；打分分层让「同名文件 / 名称开头 / 中间命中」可预期。
- 返回 id 而非条目全文，配合 D1 把 IPC 负载压到最小，5000 条时键盘输入不卡。

**备选**

- 保留正文/标签加分、只降权：仍是全文搜索，噪音无法根治。放弃。
- 前端 `includes` 全量过滤：需要全量元数据（可行）但排序口径会分叉成两份。放弃，统一后端。

### D6 直达知识库热键

**做法**

- `Settings` 新增 `kbHotkey: Option<String>`，默认 `Alt+Shift+K`（`#[serde(default = "default_kb_hotkey")]`，旧配置缺字段即得默认值）；`Settings::coerce` 空串→`None`（表示用户主动清空，不注册）。`HotkeyStatus` 增加 `kb` 字段。
- `runtime.rs`：`HotkeyAction` 增加 `OpenKb`，`HotkeyRegistry` 增加槽位，`register_kb_hotkey` 命令复用现有 `register_global_hotkey`（互斥遍历扩到 5 个动作，失败回滚旧绑定、中文提示文案复用）；`bind_global_hotkey` 的 `OpenKb` 分支调 `show_main(&handle, Some("/kb"))`。
- `main.rs` setup 线程按 capture → main → sticky → todo → kb 顺序注册；kb 失败只提示、不阻塞启动；`get_hotkey_status`/`hotkey-state` 事件带 5 个组合。
- 前端 `SettingsView` 增加第 5 个热键录入项（可清空、恢复默认、显示绑定状态）；`App.tsx` 增加 `onEvent('navigate', payload)`：payload 为字符串且在白名单 `ROUTES` 内才切换 hash，非法忽略；`open_main_window` 命令补 `route: Option<String>` 参数（修掉 `api.ts:429` 的死参数），命令总数不变。

**理由**

- 复用既有热键注册/互斥/回滚机制，只加槽位与动作，风险最小；默认 Alt+Shift+K 与现有 O/A/S/T 同族且不冲突。
- 前端监听 navigate 是 `/kb` 导航真正的落地环节；主窗隐藏场景 JS 常驻，事件可靠。

**备选**

- 复用 main_hotkey 加「带路由」开关：语义混乱、用户无法分别设置。放弃。
- 单独做系统托盘/菜单直达：不满足「快捷键直达」诉求。放弃。
- 用 URL query 传路由建窗：Tauri v1 `WindowUrl::App` 不适合带 query，方案脆。放弃；主窗销毁重建的导航竞态保持既有「尽力送达」语义（见 Risks）。

### D7 迁移、兼容与存档

**做法**

- 载入 `notes.json` 后先按 `Vec<serde_json::Value>` 探测：任一元素含 `bodyMd`/`body_md` 即旧格式，走迁移：
  1) 若 `notes.v1.json` 不存在，写一份**永久旧格式存档**（仿 `data.v1.json` 先例）并轮转 `notes.json.bak-1..5`；2) 逐条规范化正文、抽取 assets、算 `contentHash`、写 `notes/<id>.md`；3) 最后原子写新索引。
- 迁移幂等：中断后重试只是覆盖已写正文/资源（assets 按 sha 去重），索引最后写保证「索引出现即正文齐」；每次重试最多多轮转一次备份，可接受。
- 迁移失败（只读盘/空间不足）：不写新索引、旧 `notes.json` 原样保留，`kb_error` 置专属中文提示「知识库升级失败，原数据已保留，请重启后重试」，下次启动重试；不进入假空态。
- 旧备份兼容：`list_kb_backups`/`restore_kb_backup` 继续按 `Vec<Value>` 解析；恢复旧格式备份时先轮转当前 notes，再把旧条目按迁移流程写成分文件结构。
- 损坏语义不变：`notes.json` 解析失败仍 `KB_CORRUPT_MESSAGE`；`folders.json` 解析失败语义不变。降级路径：旧版本不认新索引，需先用 `notes.v1.json` 或 `notes.json.bak-1` 覆盖回 `notes.json` 再启动旧版本。

**理由**

- 「正文/资源先写、索引最后写」把崩溃窗口压到最小；永久 `notes.v1.json` 让降级与事故恢复有确定抓手，而不依赖会被轮转覆盖的 bak。
- 恢复旧备份复用同一迁移函数，避免两条转换路径分叉。

**备选**

- 迁移时不写永久存档、只靠 bak：写几次后旧格式备份就被轮转掉，降级不可逆。放弃。
- 新文件用 `notes-v2.json` 且保留 `notes.json` 旧格式：长期双文件双真相，违反单源。放弃。

## Risks / Trade-offs

- **迁移中断导致半套数据** → 顺序固定为 assets → bodies → index，索引原子写；失败不覆盖旧 `notes.json`，`notes.v1.json` + bak 兜底，下次启动幂等重试；任务含专门的中断注入测试。
- **降级不兼容** → 迁移前写 `notes.v1.json` 永久存档；Migration Plan 给出用旧存档覆盖回退步骤；发布说明需写明降级方法。
- **assetProtocol scope 与 `KETTD_DATA_DIR` 冲突** → scope 固定 `$APPDATA/todo-list/assets/**`，自定义数据目录下图片可能不显示（渲染层降级为占位、不崩溃）。接受此限制并在 README 说明；测试/冒烟仍指临时目录，不依赖图片渲染。
- **assetProtocol 暴露面** → scope 只允许 `assets/**` 且资源文件名由 sha 决定（无路径穿越、无任意文件读取）；CSP 维持 `null` 现状不变。
- **5000 条启动读 5000 个正文文件变慢** → 载入阶段顺序读 + 只在内存保留；任务中加 5000 条基准测试（`cargo test -- --nocapture` 观察耗时），若超预算再降级为「启动只读索引、正文按需」（备选已记录）。这是 D1 的主要 trade-off。
- **手写 SHA-256 / base64 出错** → 只用于资源命名/内容指纹，不用于安全加密；用标准测试向量（`""`、`"abc"`、长串、padding）单测，并在导入时用重算 sha 交叉校验。
- **备份体积膨胀（base64 ≈ ×1.37）** → 导出只含被引用资源并给出 `bytes`；导入限制 256MB 并返回中文错误；预览阶段显示规模。若实测过大，后续可加 ZIP（Non-Goal，需新依赖）。
- **导入大文件阻塞 UI / IPC 字符串过大** → 两条命令都 async + `spawn_blocking`，重 IO 不持锁；限制 256MB；若仍不满足，改用原生对话框传路径（Open Questions）。
- **冲突副本膨胀** → 预览先展示冲突数量与标题，用户可取消；副本标题带「（导入冲突）」便于检索与清理。
- **双写不一致（正文已写、索引失败）** → 载入时以正文为准修正内存索引（hash 对不上不判损坏），下次保存落盘修正。
- **删除条目不回收 assets** → 内容寻址资源可能变孤儿占空间；本次不做自动 GC（Non-Goal），导出只带被引用资源；后续可加「清理未引用资源」维护功能。
- **navigate 事件早于前端监听（主窗被销毁后重建）** → 现有 `restore_main_window` 是「尽力送达」，可能落到默认视图；主窗隐藏（常态）场景不受影响。接受为已知限制，不新增 runtime.json 待办路由字段。
- **第 5 个热键与系统/其他软件冲突** → 默认值可能与个别环境冲突；沿用既有互斥 + 回滚 + 中文提示 + 设置页可清空；任务含「故意占用组合」的失败路径验证。
- **文档陈旧计数冲突** → README/ARCHITECTURE 已有 64 与 68 并存，本次统一改为 71 并同步 parity 断言；若不改会被 parity 门禁当场拦下。

## Migration Plan

1. **实现与单测先行**：纯 Rust `sha256`/`base64`/data URL 解析、`backup` 计划器（不碰 IO）、`search` 新口径，均先加单测（标准向量 + 合并矩阵 + 搜索排序用例）。
2. **持久层扩展**：`StoreBackend` 增加正文/资源/导出文件/旧格式存档方法 + `InMemoryBackend` 同步实现；`FsBackend` 用 tmp+rename 原子写；保持 `notes.json` 写前轮转。
3. **迁移实现**：`Store::load` 探测旧格式 → 写 `notes.v1.json` 永久存档 → 轮转 → assets → bodies → 新索引；失败路径置专属 `kb_error`；`restore_kb_backup` 复用同一迁移。
4. **切换读路径**：`get_kb_items` 默认元数据、`ids` 取正文；`save_notes` 只写索引；命令层先写正文再存索引；`KbItemDetail`/`StickyWindow`/导入导出按需取正文。
5. **图片外置**：保存/迁移时抽取；渲染层 `resolveAssetUrl` + `Bootstrap.assetsDir`；`tauri.conf.json` 配 asset protocol；手动验证旧条目内联图、粘贴新图、跨条目重复图。
6. **备份导出/导入**：`export_backup` / `import_backup`（dry-run 与 apply 共用计划器）、设置页入口与预览对话框；四方同步命令与文档；`cargo test` 覆盖「往返」「冲突」「本地较新」「重复导入幂等」「非法格式」。
7. **搜索与热键**：后端搜索收窄 + 前端过滤口径；`kbHotkey` 设置/注册/导航；`App.tsx` navigate 监听；`open_main_window(route)`。
8. **文档与门禁**：更新 `docs/ARCHITECTURE.md` §2/§10、README 热键/数据文件/备份说明与命令计数、`docs/capability/06`、`09`；`test/command-parity.test.mjs` 断言 71；跑 `npm run check`（`scripts/check.ps1` 真实 17 步）+ `cargo test` + 备份往返演练 + L4 真机五条。
9. **发布与观察**：首启即在真实数据上迁移，发布说明写明「会自动迁移、迁移前留 `notes.v1.json` 存档、降级方法」；观察首启耗时与 `logs/`。

**回滚策略**

- 迁移后想退回旧版本：先恢复旧格式存档——把 `<数据目录>/notes.v1.json` 复制为 `notes.json`（或设置页恢复 `notes.json.bak-1`），再安装旧版本。
- 功能回滚：`git revert` 本次提交后，旧版本会忽略 `notes.json` 里不认识的索引字段（`KbItem` 有 serde default），但正文可能为空——因此**降级必须先恢复存档**，这是发布说明的硬要求。
- 备份/导入功能异常：不触碰 `notes/`、`assets/` 与索引结构，禁用设置页入口并回退代码即可；导入前的轮转备份可恢复导入前数据。

## 决策确认

用户已于 2026-11-14 确认本 change 全部推荐取法：① 用 `get_kb_items(ids?)` 扩展读取正文，不新增 `get_kb_item`；② 导入用前端 `<input type="file">` + IPC 传文本（256MB 上限），不改原生文件对话框；③ 合并范围只含 tasks / reminders / KB 条目 / folders / assets，settings 与便签不覆盖；④ 直达知识库默认 `Alt+Shift+K`、允许清空；⑤ 图片资源用 Tauri v1 assetProtocol + 窄 scope `$APPDATA/todo-list/assets/**`。

以下非阻断议题明确不在本 change 范围：`assets/` 孤儿资源回收入口、`events.jsonl` / 遥测是否入包（可留 `formatVersion: 2`）、备份加密 / 口令。Open Questions：无。
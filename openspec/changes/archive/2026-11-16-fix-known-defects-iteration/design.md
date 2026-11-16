# Design

## Context

见 `proposal.md`（Why / What Changes）与 `specs/` 下的五个能力契约。这里只记录影响实现方式的现状与约束（均已在本次调查中核实）：

- 编辑器：`src-react/src/rendering/tiptap-editor.tsx`（约 600 行）每次渲染都新建 extensions；`onUpdate` 800ms 防抖后 `getMarkdown()` 经 `onSave` 交给 `kb-item-detail.tsx` 的 `handleSave`，后者调用 `onRefresh()`；`KbView.tsx:48` 的 refresh 会把 `loading=true`，而 `KbView.tsx:174` 的 `{loading && <KbEmptyState/>}` 与列表并列渲染。
- 知识库数据：`KbItem` 无分组字段（`models.rs:497-504`）；`notes.json` 根为裸数组（`store.rs:964-970`）且没有写前轮转；解析失败进入「静默空库」通道（`store.rs:756-763`）。
- 命令契约：55 条，四方同步（`main.rs:85-141`、`commands.rs`、`api.ts:281-391`、`ARCHITECTURE.md §2/§10`），`test/command-parity.test.mjs` 硬断言。
- 便签：`✕` 直接 `delete_sticky`（`StickyWindow.tsx:107-111` → `commands.rs:581-596`）；`STICKY_CAP=6`、`STICKY_KB_CAP=4`（`models.rs:476/478`）；启动恢复全部便签窗（`main.rs:240-257`）。
- 外壳：Tauri 1.8.3 / wry 0.24.12；WebView2 默认右键菜单由 wry 开启（`wry-0.24.12/src/webview/webview2/mod.rs:246`），devtools 仅在 debug/devtools feature 下可开；Tauri v1 为每个窗口强制设置 data_directory（`tauri-1.8.3/src/manager.rs:1059-1080`），`data_directory` 会覆盖 `WEBVIEW2_USER_DATA_FOLDER`；`WindowConfig` 没有 dataDirectory 字段。
- 编辑器与便签共用同一个 `TipTapEditor`（`StickyWindow.tsx:217`），改编辑器会同时影响知识库条目与知识便签。

## Goals / Non-Goals

**Goals:**
- 先建立可复核的复现基线：17 项逐项给出「现状证据 + 复现步骤 + 是否可无头验证」，再动代码。
- 修复顺序按「风险/收益」分批：前端小改（闪烁、右键、链接、标题、折叠）→ 检索与选中 → 便签数据语义 → 编辑器保真与图片 → WebView2 建窗稳健性。
- 任何持久化变更都保持 `notes.json` / `data.json` 的兼容加载，不引入静默空库路径。
- 新增命令全部四方同步并过 parity 门禁；每批结束跑全套门禁。

**Non-Goals:**
- 不引入新的编辑器框架、不重写 Tiptap 扩展栈。
- 不做「外置 Markdown 文件夹作为知识库源」的双向同步（#14 先只做「打开数据目录」）。
- 不做图片的独立文件存储方案（#15 先走 data URL MVP；`save_image` + assetProtocol 留作后续迁移）。
- 不实现 WebView2 用户数据目录的自动跨机迁移（只做可写性探测与回退）。
- 不在本次处理 `notes.json` 的数据库化/分片。

## Decisions

### D1 闪烁与刷新（#1）
- **做法**：`KbView` 的空态只在 `items.length === 0` 且非加载态时渲染；保存成功后本地回填当前 item（`onSave` 返回保存结果或由父级 `setItems` 局部替换），不再调用全量 `refresh()`；`store-changed` 触发的刷新做 300ms 合并，且焦点在本编辑器时跳过回灌。
- **备选**：把空态放到独立 overlay（可缓解但仍有全量重拉）；改用增量查询（需要新命令，收益不足）。
- **理由**：闪烁的直接触发是「loading 与空态并列」，根因是保存后不必要地整库重拉。

### D2 编辑器渲染成本（#2）
- **做法**：`extensions` 与回调用 `useMemo`/`useRef` 稳定引用；用 `useEditorState` 或选区订阅替代「每次 transaction 都 `bumpRender` 全组件」；Markdown 序列化保留 800ms 防抖，并在模糊/切条目前 `flush`；知识库列表（最多 5000 条）改用窗口化渲染或 `content-visibility: auto`。
- **备选**：仅加 `React.memo`（收益有限，不解决 `setOptions`）。
- **理由**：`compareOptions` 恒 false 会触发 `view.updateState`，是每键卡顿的结构性原因。

### D3 Markdown 保真（#10）
- **做法**：以 tiptap-markdown 的序列化结果为唯一保存形态；新增 round-trip 门禁用例（fixture：`*`/`+`、`1)`、setext、`~~~`、表格对齐、硬换行、裸 URL、图片、HTML）；表格通过扩展保留 `alignment` attr 并自定义 serializer 输出对齐标记；硬换行统一为反斜杠形式；补 `@tiptap/extension-image`（与 #15 共用）。
- **备选**：保存原文并只在差异时提示（需要双份内容，复杂度高）；换用 remark/unified 序列化（改动面过大）。
- **理由**：23/50 的样例发生一次归一化，说明多数是「可修的序列化缺口」，而不是必须接受的差异。

### D4 标题分级与链接（#9 / #8）
- **做法**：工具栏 H1/H2 两个按钮替换为一个标题下拉（正文/H1–H4），`SLASH_ITEMS` 增加 h3 与「正文」；补 `h4–h6` 的编辑器与阅读态样式；链接改用居中 `LinkDialog`（复用 `app.css:614-637` 的 overlay/card 尺寸并新增 `align-items:center`）。
- **理由**：斜体与删除线已存在，缺口只在标题层级与链接弹窗；复用现有样式可避免视觉不一致。

### D5 截图（#15）
- **做法（MVP，范围仅知识库条目）**：在 `TipTapEditor` 注册 Image 扩展（新增 `allowImages` 开关，便签不启用）；`handlePaste` 捕获图片文件 → canvas 压缩（长边 ≤1600px、≤300KB）→ `data:image/webp|png;base64,...` 写入正文；超限提示拒绝。渲染侧 `md-static-render` 已可显示 Markdown 图片。
- **后续（非本次）**：`save_image` 命令 + `security.assetProtocol` + `convertFileSrc`，把图片迁到 `%APPDATA%\todo-list\assets\`，并清理孤儿文件。
- **理由**：MVP 不需要新命令与安全配置，能覆盖「截图上传」的主场景；data URL 的体积风险用压缩 + 上限 + 轮转控制。

### D6 文件夹模型（#4）
- **做法**：`KbItem` 增加 `folder_id: Option<String>`（serde default，旧数据 = None → 未分类）；新增 `folders.json`：`[{id, name, parent_id, order}]`，走 `StoreBackend` trait 的 Fs/InMemory 双实现；前端左栏把扁平列表构树渲染。新增命令：`list_folders`、`create_folder`、`rename_folder`、`delete_folder`（`update_kb_item` 增加 `folder_id` 字段即可，不新增移动命令）。
- **备选**：直接在 `KbItem` 里存 `./` 路径字符串（重命名需要批量改）；改 `notes.json` 根为对象（会触发静默空库风险）。
- **理由**：独立文件 + 引用字段保持旧数据零迁移，且不触碰「根形状不可变」的红线。
- **删除语义**：仅当文件夹为空时允许删除；非空时提示用户先移走或删除其中的条目，避免误删内容。

### D7 默认选中与快捷键（#5 / #6）
- **做法**：`KbView` 在 `filteredItems` 变化后做一次选择校正：若当前 `selectedId` 不在可见列表，选第一项；`#/kb?id=` 深链接优先。切条前调用编辑器的 `flush`，避免 800ms 防抖丢字。`Ctrl+F` 条目内查找用 ProseMirror Decoration + 自绘查找栏，`Ctrl+Shift+F` 聚焦 KbSearchBar；判断事件目标是否为可编辑元素，避免输入框内误触发。
- **理由**：纯前端可完成，不需要新命令；查找栏自绘可绕开 WebView2 没有浏览器级查找条的问题。

### D8 备份与恢复（#7）
- **做法**：第一步接通已有 `get_backups` / `restore_backup`（设置页列表 + 确认恢复，恢复前自动轮转）；第二步给 `notes.json` 增加同样的写前轮转与 `restore_kb_backup` / `list_kb_backups`（`StoreBackend` 增加轮转方法）；解析失败不再走「静默空库」，改为可恢复的错误态。
- **理由**：data 侧后端能力已存在，前端接入是零命令成本；KB 侧必须补轮转才能真正满足「文件备份/恢复」。

### D9 便签生命周期（#12 / #11）
- **做法**：`StickyNote` 增加 `visible: bool`（serde default = true）；`✕`/Alt+F4 改为 `visible=false` + 隐藏窗口；启动只恢复 `visible=true` 的便签；新增 `open_sticky(id)`（55→57，另加 `set_todo_float_mini`）供 CRUD 列表重新打开；上限改为 5（所有可见便签合计口径，自由便签与知识便签统一计算，隐藏便签不计入）；设置页扩「便签」区域作为 CRUD 入口。待办最小化参照便签 mini：`runtime.json` 增加 `todo_float_mini`，Rust 增加 `apply_todo_shape` 与启动恢复。
- **便签编辑器**：便签正文改为 Tiptap WYSIWYG（复用 `TipTapEditor` 的 `allowImages=false` 精简配置，禁用图片/表格等重组件，保留 500 字上限），旧纯文本正文按 Markdown 兼容解析；移除「MD 预览」手工开关。
- **风险控制**：旧数据默认 visible=true 时，已有 6 张的用户启动不会被强制关闭，只挡新建；删除路径仍须清理 `note_pos`；WYSIWYG 便签需保留纯文本回退（解析失败时按原文展示）。

### D10 右键菜单与折叠（#3 / #13）
- **做法**：在 Rust 侧建窗后用 `with_webview` 调用 `SetAreDefaultContextMenusEnabled(false)`（主窗 + 便签 + 待办 + 捕获四处建窗点）；dev 构建保留 devtools 热键，release 不暴露「检查」。侧栏增加图标（lucide 已有）+ 折叠按钮 + `localStorage('shell.sideCollapsed')`；知识库左栏折叠从 0px 改为 72–110px 窄轨，标题单行 `text-overflow: ellipsis`。
- **理由**：只在 CSS 层隐藏无法阻止 WebView2 原生菜单；Rust 侧关闭是唯一彻底方案。文本框复制粘贴依赖 `Ctrl+C/V`（禁用右键不影响快捷键）。

### D11 WebView2 建窗稳健性（#16 / #17）
- **做法**：
  1. 启动早期计算可写的 WebView2 用户数据目录：优先 `%APPDATA%\todo-list\webview2`，失败回退 `%LOCALAPPDATA%`、最后 `%TEMP%\kettd-webview2`，逐级探测可写。
  2. 主窗改为代码创建：从 `tauri.conf.json` 的 `windows` 移除主窗，在 `setup` 中用 `WindowBuilder::new(app, "main", ...)` + `.data_directory(可写目录)` 创建（Tauri v1 的 config 窗口无法设置 data_directory）；便签/待办/捕获窗的 builder 同样显式设置。
  3. 建窗失败路径：`build_with_timeout` 已有 6s 超时与 `log_build_error`，补充把失败原因回传前端并提示；启动建窗失败时展示可读错误而非静默。
  4. `WEBVIEW2_USER_DATA_FOLDER` 仅作为文档化兜底（Tauri v1 会覆盖，需在文档中说明）。
- **备选**：只在辅助窗设置 data_directory（主窗仍可能失败）；仅依赖环境变量（已被证伪）。
- **理由**：主窗是应用入口，必须亲自控制目录。

## Risks / Trade-offs

- [便签「关闭」语义变更] → 迁移说明 + 设置页列出隐藏便签；旧数据默认 visible=true，不强制关闭已有便签。
- [data URL 使 notes.json 膨胀] → 压缩（长边 ≤1600、≤300KB）+ 单条图片数量/总体积上限 + KB 轮转；后续迁 assetProtocol。
- [文件夹功能误改 notes.json 根形状 → 静默空库] → 保持根为数组，只加字段；补「旧数据加载」单测与损坏通道测试。
- [恢复 data.json 后跨引用悬空] → 恢复后重建 `task.kbRefs` / 便签 `kb_ref` 的校验，悬空引用降级为普通文本链接。
- [主窗移出 config 改变启动时机] → 保持 label=`main`，托盘/热键/单实例逻辑按 label 查找；启动冒烟覆盖冷启动与二次启动。
- [Ctrl+F 被 WebView2 占用] → 在 keydown 阶段 `preventDefault`，真机验证。
- [每批修复跨前后端，回归面大] → 每批独立跑门禁（cargo check/test、tsc、build、parity、md-fidelity）+ 记录真机冒烟项。

## Migration Plan

1. 无数据迁移脚本：所有新字段用 serde default，旧文件直接可读。
2. 便签可见性：旧数据无 `visible` 字段 → 视为 true；启动时若可见便签数超过 5，不关闭已有窗口，只拒绝新建。
3. 新增 `folders.json`：首次启动时若不存在则视为空文件夹表，不创建破坏性文件。
4. KB 轮转：先在打开时检测旧文件是否已有轮转目录，没有则从下一次写入开始建立，不动历史数据。
5. 回滚：每批改动保持独立提交，可用 `git revert <commit>` 回退；数据格式变更均向下兼容（新字段旧版本会忽略）。

## Open Questions

无。四项影响范围的决策已由用户确认：可见便签合计上限 5、便签改 WYSIWYG、文件夹仅允许删空、截图仅知识库编辑器。
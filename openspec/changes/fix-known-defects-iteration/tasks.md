# Tasks

阶段 0 先复现取证，再按批次修复；批次顺序与 `design.md` 的 Decisions 对齐，编号与台账 17 项对应（#1–#15 为知识库/编辑器/便签/外壳需求，#16 为 WebView2 进程残留，#17 为建窗用户数据目录）。每项完成时须在过程文档/README 勾选并附验证证据。

## 1. 复现基线（17 项逐项取证）

- [x] 1.1 建立复现证据目录（`openspec/changes/fix-known-defects-iteration/repro/`）与可重复脚本骨架，记录环境（Node 版本、tauri/wry 版本、当前提交）；验证：脚本可独立运行并输出环境信息
- [x] 1.2 复现编辑器类：#1 保存闪烁（`KbView.tsx:48` loading 与 `:174` 空态同频）、#2 每键重渲染/`setOptions` 卡顿、#10 Markdown 格式漂移（jsdom round-trip 样例与 23/50 归一化清单）、#9 标题分级缺失与 `### ` 输入/H4–H6 无样式；证据写入 `repro/editor.md`
- [x] 1.3 复现知识库类：#4 无文件夹层级、#5 `selectedId` 初始为 null、#6 无 Ctrl+F / Ctrl+Shift+F 入口、#7 备份命令未接线、#14 无打开数据目录入口；证据写入 `repro/kb.md`
- [x] 1.4 复现便签类：#11 正文仅 textarea + 手动 MD 预览、#12 `✕` 直接 `delete_sticky` 与上限 6/4；证据写入 `repro/sticky.md`
- [x] 1.5 复现外壳类：#3 WebView2 默认右键（dev/release 差异）、#13 侧栏无折叠且知识库折叠丢标题、#16 退出后 `msedgewebview2.exe` 残留；证据写入 `repro/shell.md`
- [x] 1.6 复现窗口类：#17 探测 `%LOCALAPPDATA%\com.todo.list` 存在与可写性、模拟不可写目录下的建窗失败路径、确认 `CreateWebview` 报错来源；证据写入 `repro/window.md`
- [x] 1.7 汇总 `repro/README.md`：17 项逐条「现象 / 证据 / 复现步骤 / 是否可无头验证 / 需真机确认点」，并与台账 16+1 条一一对应；验证：每项都有证据链接或明确「需真机」标注

## 2. 批次 1：前端小改（#1 #9 #8 #13 #3）

- [x] 2.1 #1 空态只在 `items.length === 0` 且非加载态渲染，保存成功后局部回填条目、不再全量 `onRefresh`；验证：保存时左栏无空态闪烁（手工步骤 + 代码检查）
- [x] 2.2 #9 标题下拉（正文/H1–H4）+ `SLASH_ITEMS` 增 h3/正文 + `prosemirror.css` 补 h4–h6 编辑器与阅读态样式；验证：下拉可应用级别、输入 `### ` 生效、`npx tsc -b` 通过
- [x] 2.3 #8 新增居中 `LinkDialog` 替换 `window.prompt`（预填/确定/取消/移除、Enter/Esc）；验证：插入链接不再出现原生 prompt，Esc 可关闭
- [x] 2.4 #13 侧栏加图标 + 折叠按钮 + `localStorage('shell.sideCollapsed')`；知识库左栏折叠改窄轨（72–110px）保留标题省略号；验证：折叠/展开正常且重启后保持
- [x] 2.5 #3 Rust 建窗后用 `with_webview` 关闭 WebView2 默认右键菜单（主窗/便签/待办/捕获四处）；验证：release 构建右键无「检查」，文本框 Ctrl+V 仍可用
- [x] 2.6 批次 1 门禁：`cargo check`、`cargo test`、`npx tsc -b`、`npm run build`、`node test/command-parity.test.mjs`、`node src-react/test/md-fidelity.test.mjs` 全绿并记录输出

## 3. 批次 2：知识库检索与流畅度（#5 #6 #2）

- [x] 3.1 #5 默认选中可见第一项（deep link 优先）+ 切条前 flush 800ms 防抖；验证：打开/切换文件夹自动选中，快速切条不丢字
- [x] 3.2 #6 Ctrl+F 条目内查找（Decoration + 自绘查找栏、preventDefault、Esc 关闭）与 Ctrl+Shift+F 聚焦搜索框；验证：真机快捷键生效，输入框内不误触发
- [x] 3.3 #2 编辑器 extensions/回调 `useMemo`/`useRef` 化，去掉每键全组件重渲染（改选区订阅），列表虚拟化或 `content-visibility`；验证：长文连续输入无扩展重建，构建通过
- [x] 3.4 批次 2 门禁（同 2.6 六项）并记录输出

## 4. 批次 3：便签数据与形态（#12 #11）

- [x] 4.1 `StickyNote` 增 `visible: bool`（serde default true）+ 旧数据兼容单测；验证：cargo test 新增用例通过
- [x] 4.2 关闭即隐藏：`✕`/Alt+F4 改 `visible=false` 并隐藏窗口，启动只恢复可见便签，删除仍清理 `note_pos`；验证：关闭后重开内容仍在
- [x] 4.3 上限改「可见便签合计 5」（`STICKY_CAP`/`STICKY_KB_CAP` 校验合一，隐藏不计入）；验证：第 6 张被拒绝并给出中文提示
- [x] 4.4 新增 `open_sticky(id)` 命令并四方同步（`main.rs`/`commands.rs`/`api.ts`/`ARCHITECTURE.md §2+§10`）；验证：parity 57 通过
- [x] 4.5 设置页便签 CRUD 区域（列表含隐藏项、新建/打开/编辑/删除）；验证：可从列表重新打开隐藏便签
- [x] 4.6 #11 便签编辑器改 WYSIWYG（复用 `TipTapEditor`，`allowImages=false`，保留 500 字上限），移除 MD 预览开关，旧纯文本兼容；验证：输入 `**粗体**` 实时生效、旧便签正常打开
- [x] 4.7 待办最小化：`runtime.json` 增 `todo_float_mini`、Rust `apply_todo_shape` + 启动恢复、前端迷你条与展开钮；验证：最小化后重启仍为迷你
- [x] 4.8 批次 3 门禁 + 真机冒烟（便签可见/隐藏/删除、CRUD、上限）

## 5. 批次 4：知识库组织与备份（#4 #7 #14）

- [x] 5.1 数据模型：`KbItem.folder_id: Option<String>`（serde default）+ `folders.json`（StoreBackend Fs/InMemory 双实现）+ 旧数据加载测试；验证：cargo test 覆盖无字段旧数据
- [x] 5.2 新增 `list_folders`/`create_folder`/`rename_folder`/`delete_folder`（+4）并让 `update_kb_item` 支持 `folder_id`，四方同步；验证：parity 通过（57→61）
- [x] 5.3 左栏文件夹树 UI（新建/重命名/删除/移动条目），删除仅允许空文件夹；验证：非空删除被拒绝并提示
- [x] 5.4 #7 data 备份前端接入（列表 + 恢复确认 + 恢复前轮转）；`notes.json` 增写前轮转与 `list_kb_backups`/`restore_kb_backup`（+2 → 63），损坏走恢复态不再空库；验证：恢复后数据一致、损坏提示可见
- [x] 5.5 #14 知识库「打开数据目录」按钮复用 `open_data_folder`（零新命令）；验证：点击打开 `%APPDATA%\todo-list`
- [x] 5.6 批次 4 门禁 + 真机冒烟（文件夹增删移、备份恢复）

## 6. 批次 5：编辑器保真与截图（#10 #15）

- [x] 6.1 新增 Markdown round-trip 门禁 fixture 与 node 测试（`*`/`+`、`1)`、setext、`~~~`、表格对齐、硬换行、裸 URL、任务列表空行）；验证：先记录当前差异基线
- [x] 6.2 序列化修正：表格 alignment attr/serializer、硬换行统一反斜杠、图片与不可表达 HTML 的保存前提示；验证：round-trip 门禁通过且二次保存幂等
- [x] 6.3 #15 Image 扩展 + 粘贴截图（canvas 压缩长边≤1600/≤300KB、data URL、超限拒绝、`allowImages` 仅知识库启用）；验证：粘贴截图可见、保存重开仍在、便签不响应图片
- [x] 6.4 批次 5 门禁（含 md-fidelity 与新增 round-trip）

## 7. 批次 6：窗口稳健性（#16 #17）

- [x] 7.1 启动早期解析并探测可写 WebView2 用户数据目录（`%APPDATA%\todo-list\webview2` → `%LOCALAPPDATA%` → `%TEMP%` 回退），日志记录实际使用目录；验证：模拟只读目录仍能启动
- [x] 7.2 主窗移出 `tauri.conf.json` 改 `setup` 代码创建（label 仍 `main` + `.data_directory()`），便签/待办/捕获 builder 显式设置；验证：冷启动、托盘、热键、单实例均正常
- [x] 7.3 #17 建窗错误可见化：`build_with_timeout` 失败回传中文提示，日志含尝试目录；验证：模拟不可写目录时提示且不挂起
- [x] 7.4 #16 排查并修复退出残留 `msedgewebview2.exe`（退出顺序、WebView 释放）；验证：任务管理器无残留进程
- [x] 7.5 批次 6 门禁 + 真机冒烟（冷启动、多窗、退出）

## 8. 集成验收

- [ ] 8.1 全套门禁（cargo check/test、tsc、build、parity、md-fidelity、round-trip）全绿并保存输出
- [ ] 8.2 对照 `repro/README.md` 逐项复核 17 项（含真机项），记录通过/未通过
- [ ] 8.3 更新文档：`docs/ARCHITECTURE.md` 命令表与计数、README/AGENTS 相关说明（文档类交付先人工审核，审核通过前不提交）
- [ ] 8.4 真机冒烟：快速记录 / 今天视图 / 提醒 / 便签三态 / 知识库搜索 / 退出，全通过

## Workflow follow-up

- 上批「知识库斜杠菜单与表格交互」四缺陷修复的 3 个文件仍未提交，等待用户真机验收后再单独提交。
- 每批改动保持独立中文 conventional-commit；全部完成后推送 master。
- 文档类改动经人工审核后再提交。
- 全部任务完成并验收后，用 `/openspec-archive-change` 归档本 change。
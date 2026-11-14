# Proposal

## Why

Kettd 2.2.0 目前累积了 17 项已知缺陷与缺失能力，集中在知识库编辑器体验、知识库组织与检索、便签生命周期、应用外壳交互和 WebView2 建窗稳健性五个方面。其中若干项直接造成数据风险（便签「关闭」即删除数据、Markdown 内容被保存链路改写），另一些（保存后闪烁、输入卡顿、面板遮挡、建窗失败无恢复路径）则持续破坏日常使用信心。先逐项复现、再分批修复，可以避免只按猜测改代码，并让每一项交付都有可复核的验收证据。

## What Changes

按主题列出本次要改变的行为（编号与台账 17 项一致）：

- **知识库编辑器**
  - 消除「保存后闪烁」：保存回包不再让左栏整列表被空态/加载态顶掉。
  - 提升输入流畅度：避免每次击键触发全组件重渲染与编辑器 `setOptions`，保存链路不再全量重建。
  - 收敛 Markdown 格式漂移：明确编辑器保存时允许的归一化集合，禁止静默丢失内容（如表格对齐、图片、HTML）。
  - 标题分级：H1/H2 按钮改为标题下拉（正文 / H1–H4），`### ` 输入式标题在样式与序列化上可用。
  - 链接插入：用居中对话框替换 `window.prompt`。
  - 截图粘贴：知识库条目编辑器支持粘贴/选择图片并按压缩后的 data URL 写入正文（便签不支持图片，500 字上限不变）。
- **知识库组织与检索**
  - 文件夹层级：知识点支持归属文件夹（独立 `folders.json` 存树，`notes.json` 根形状不变）；删除仅允许空文件夹。
  - 默认选中：打开知识库或切换文件夹时默认选中可见列表第一项。
  - 快捷键：`Ctrl+F` 条目内查找、`Ctrl+Shift+F` 聚焦知识库搜索。
  - 备份与恢复：接通已有 `get_backups` / `restore_backup` 的前端入口；KB 数据纳入轮转与恢复。
  - 打开本地目录：知识库入口可直接打开数据目录（复用 `open_data_folder`）。
- **便签**
  - 关闭不再删数据：`✕`/Alt+F4 改为隐藏（`visible=false`），启动只恢复可见便签；新增打开已有便签的命令与便签 CRUD 页面。
  - 单次打开数量上限调整为 5（所有可见便签合计口径，自由与知识便签统一计算）。
  - 便签编辑改为所见即所得：复用 Tiptap 编辑器，移除「MD 预览」手工开关；待办窗口支持与便签一致的最小化形态。
- **应用外壳**
  - 右键菜单不再暴露「检查」等 WebView2 默认项（至少 release 构建彻底禁用；保留文本框编辑菜单的替代方案）。
  - 侧栏支持完全折叠并保留图标；知识库左栏折叠改为保留标题的窄轨（截断策略按宽度自适应）。
  - 退出应用时清理 WebView2 子进程，避免 `msedgewebview2.exe` 残留。
- **窗口稳健性**
  - 建窗失败可诊断：捕获 `CreateWebview` / 建窗错误并给出用户可读提示与日志。
  - 显式指定可写的 WebView2 用户数据目录（`data_directory` 指向应用数据目录），启动时做目录可写探测；`WEBVIEW2_USER_DATA_FOLDER` 仅作文档化兜底。

## Capabilities

### New Capabilities

- `knowledge-base/editor`: 知识库条目与知识便签共用的 Markdown 编辑器的输入、格式命令、链接/图片插入、保存与外部回填行为。
- `knowledge-base/organization`: 知识库条目的文件夹组织、选中策略、视图内检索、数据备份/恢复与本地目录访问。
- `sticky-notes`: 便签与待办悬浮窗的持久化、显示/隐藏语义、CRUD 入口、数量上限与形态切换。
- `app-shell`: 应用外壳交互（侧栏折叠、WebView2 右键菜单策略）与退出时的进程生命周期。
- `window-management`: 窗口创建稳健性（WebView2 用户数据目录选择、错误处理、可写性探测与失败提示）。

### Modified Capabilities

无（项目此前没有 `openspec/specs/`，本次全部为新能力）。

## Impact

- **前端**：`src-react/src/rendering/tiptap-editor.tsx`、`src-react/src/rendering/styles/prosemirror.css` 与 `app.css`、`src-react/src/views/KbView.tsx`、`src-react/src/views/SettingsView.tsx`、`src-react/src/components/kb/*`、`src-react/src/sticky/StickyWindow.tsx`、`src-react/src/todo/TodoFloatWindow.tsx`、`src-react/src/App.tsx`、`src-react/src/lib/api.ts`、`src-react/src/kernel/*`。
- **后端**：`src-tauri/src/models.rs`、`store.rs`、`runtime.rs`、`kb.rs`、`commands.rs`、`main.rs`、`infra/fs_store.rs`、`ports/store_backend.rs`、`tauri.conf.json`。
- **命令契约**：预计 55 → 最多 62（文件夹 4、备份/恢复 2、`open_sticky` 1、`save_image` 视方案），必须四方同步并通过 `test/command-parity.test.mjs`。
- **持久化数据**：`notes.json` 根形状保持不变；新增 `folders.json`；`runtime.json`/便签增加 `visible`、`todo_float_mini` 等带 serde 默认值的字段；KB 数据加入写前轮转。
- **风险**：便签「关闭」语义从删除变为隐藏（行为变更，需要迁移说明）；图片 data URL 会让 `notes.json` 膨胀，需要压缩与体积上限；默认 Markdown 渲染与「编辑原文」的交互形态需要先定。
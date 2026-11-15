# Proposal

## Why

知识库左栏现在是「文件夹树（只做筛选）+ 独立条目列表」两区结构：文件夹和文件不在一起展示，不能拖拽调整归属或顺序，也没有右键菜单；条目归属只能回详情页用下拉修改，新建后焦点不落名称。写作区正文固定在左侧、窗口变宽只增加右侧空白。用户需要把知识库当作文件管理器来使用（树状合并、右键、拖拽、排序、置顶、沉浸式写作），本变更补上这些交互缺口。

## What Changes

- 左栏合并为**单棵文件树**：文件夹与知识条目同树展示，支持展开/折叠、选中定位、默认展开当前选中路径；搜索/标签过滤时切换为**扁平结果列表 + 文件夹路径面包屑**，过滤态禁用结构操作。
- 新增**右键菜单**（树节点与树空白区）：新建文件夹、新建文件、重命名、删除、置顶/取消置顶、移动到…；标题/标签等可编辑输入框补「剪切/复制/粘贴/全选」迷你菜单；正文编辑器保持不接管右键。
- 新增**拖拽**：条目移入文件夹、文件夹移入文件夹（禁止拖入自身或后代）、同级手动排序（行中部=移入，上下边缘=插入线），悬停 600ms 自动展开文件夹；同时保留右键「移动到…」作为键盘与远距离操作通道。
- 新增**置顶**：文件与文件夹均可置顶，树顶部固定「置顶」快捷区（原位置保留）；`KbItem` 与 `KbFolder` 各增 `pinned` 字段。
- 新增**同级手动排序**：`order` 字段，写端语义为「目标同级索引」，由后端把同级整体重排为连续序号。
- 详情页**移除「所属文件夹」下拉**，移动统一走拖拽或右键「移动到…」。
- **新建文件/文件夹后焦点落入名称编辑态**（focus + select），支持直接改名。
- **移除知识条目的「转为待办」按钮**及其弹窗组件。
- **BREAKING**：删除 `rename_folder` 命令，新增 `update_folder(id, name?, parentId?, order?, pinned?)`（命令总数保持 64）；`update_kb_item` 的 payload 扩展 `order`、`pinned`。
- 知识库**写作区布局**：正文列水平居中，标题/标签/工具栏与正文同列对齐；正文列宽 `min(100% − 2×clamp(28px, 4vw, 64px), 78ch)`，底部写作留白 `max(64px, 26vh)`；窗口变宽只增加两侧留白，不拉长行宽。
- 同步契约与文档：`docs/ARCHITECTURE.md` §2/§10、`AGENTS.md` 命令数口径、`docs/capability/06-知识库.md`。

## Capabilities

### New Capabilities

- `knowledge-base/workspace`: 知识库工作区行为——单树展示与过滤、右键菜单、拖拽与移动、同级排序、文件/文件夹置顶、新建即命名、移除所属文件夹下拉与创建待办入口。
- `knowledge-base/writing-layout`: 知识库写作区布局——居中正文列、最小侧留白、行宽上限、标题与工具栏对齐、底部写作留白、窄窗响应式退化。

### Modified Capabilities

（无：原有 `fix-known-defects-iteration` 变更尚未归档、主 specs 为空，本变更按独立能力名立项，避免未归档 delta 冲突。）

## Impact

- 前端：`src-react/src/views/KbView.tsx`、`src-react/src/views/kb.css`、`src-react/src/components/kb/kb-folder-tree.tsx`、`src-react/src/components/kb/kb-item-detail.tsx`，新增右键菜单 / 编辑菜单 / 移动到… 组件，`src-react/src/rendering/styles/prosemirror.css`；删除 `src-react/src/components/kb/kb-new-task-dialog.tsx` 及其引用。
- 后端：`src-tauri/src/models.rs`（`KbItem.pinned/order`、`KbFolder.pinned`、`KbPayload` 扩展）、`src-tauri/src/store.rs`（移动/环校验/排序/置顶/旧数据 order 兜底）、`src-tauri/src/commands.rs`（删 `rename_folder`、加 `update_folder`、扩展 `update_kb_item`）、`src-tauri/src/main.rs`（注册）。
- 契约：`src-react/src/lib/api.ts`、`docs/ARCHITECTURE.md` 附录 §2/§10、`test/command-parity.test.mjs`（本 change 落地时保持 64 条；并行 `kb-storage-backup` 追加 3 条后现行基线为 71 条）。
- 数据：`notes.json` 的 `KbItem` 与 `folders.json` 的 `KbFolder` 新增字段都走 serde default（旧数据零迁移；`order` 缺失时首次写入按 `updatedAt` 倒序生成，避免升级后顺序漂移）。
- 依赖：不新增第三方依赖。
- 验证：`npm run check`（12 项，SKIP 不算通过）+ L4 真机证据（拖拽/右键/置顶/排序 + 1440/1920/1024 三档窗口布局截图）。
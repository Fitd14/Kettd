# Proposal

## Why

知识库左栏目前**拖不动任何东西**：用户在应用里按住条目或文件夹拖动没有任何反馈，只能靠右键「移动到…」调整归属。代码里其实已经写了一整套 HTML5 拖拽（`kb-tree.tsx` 的 `draggable` / `onDragStart` / `onDragOver`），但 Tauri v1 在 Windows 上给主窗开了窗口级拖放接管（`src-tauri/tauri.conf.json` 未显式关闭 `dragDropEnabled`，默认开启），WebView2 的 drag 事件在到达页面之前就被吞掉，`draggable` 行永远收不到 `dragstart`。同一份代码在浏览器里能拖，在应用里不能，用户看到的就是「没有这个功能」。

拖拽是左栏最高频的组织动作，缺了它，层级文件夹只能靠「右键 → 移动到…」逐条搬。这次把它做成真的能用。

## What Changes

- **用指针拖拽替换 HTML5 拖拽**：改为 Pointer Events（鼠标 / 触摸 / 笔同一套），按下行 4px 阈值起拖，跟随指针绘制落点；不再依赖 `dataTransfer` 与自定义 MIME，绕开 WebView2 对原生 drag 的接管。
- **保留并复用现有落点算法**：`resolveDropTarget`（上/中/下三区）与 `planKbDrop` / `planAppendDrop`（自身、后代、成环预判）不动，只把事件源从 drag 换成指针。
- **补齐拖拽期间的体验**：拖到滚动区上下边缘自动滚动、悬停文件夹 600ms 自动展开、目标行落点高亮（沿用 `kb-tree-drop-before/after/inside`）、拖拽源行半透明、`Esc` / 窗口失焦 / 指针离开窗口取消。
- **固定行语义**：`未分类` 接受条目拖入（= 移出文件夹），`全部知识` 不接受放置；文件夹不能拖进 `未分类`。
- **落库路径不变**：仍走 `update_kb_item(folderId, order)` / `update_folder(parentId, order)`，乐观回填 + 失败整组快照回滚（`KbView.tsx` 的 `handleDropNode` 已实现，直接复用）。
- **键盘 / 触摸替代路径保留**：右键菜单「移动到…」「置顶」继续可用；触摸不依赖长按，按住即拖。
- 不新增后端命令、不改 `notes.json` / `folders.json` 结构、不动排序规则（`pinned → order → updatedAt → name`）。

## Capabilities

### New Capabilities

无。拖拽属于既有的知识库组织能力，不新开能力目录。

### Modified Capabilities

- `knowledge-base/organization`: 新增「拖拽调整归属与顺序」Requirement（含起拖、落点、自动滚动、取消、失败回滚、固定行语义、替代路径七类场景），并修正原「新建文件夹并移动条目」场景里「拖入」这一动作的可验证口径。

## Impact

- 前端：`src-react/src/components/kb/kb-tree.tsx`（事件源替换）、`src-react/src/components/kb/kb-tree-utils.ts`（保留纯函数，新增指针坐标 → 落点的辅助）、`src-react/src/views/KbView.tsx`（`handleDropNode` 复用，去掉 drag 专属清理）、`src-react/src/views/kb.css`（拖拽态与自动滚动的视觉）。
- 测试：`src-react/test/kb-tree.test.mjs` 增补指针落点用例；不涉及 Rust 单测与命令契约。
- 无后端、无依赖、无数据结构变化；`npm run check` 的命令计数与 parity 不受影响。
- 回滚：只动上述四个前端文件，`git checkout -- src-react/src/components/kb src-react/src/views/kb.css src-react/src/views/KbView.tsx` 即回到现状（HTML5 拖拽代码仍在，只是不触发）。
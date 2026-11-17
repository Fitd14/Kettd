# Design

## Context

- 左栏拖拽的代码已经存在，但跑不起来。`src-react/src/components/kb/kb-tree.tsx` 给条目行与文件夹行加了 `draggable={dndEnabled}`、`onDragStart`、`onDragOver`、`onDrop`（行 359-380、391-450、616），落点算法 `resolveDropTarget`（`kb-tree-utils.ts:383-400`，上/下 25%、中 50%）与落库计划 `planKbDrop`（`:553`）/`planAppendDrop`（`:666`）都是纯函数，`KbView.tsx:819-869` 的 `handleDropNode` 已完成乐观回填、失败整组回滚、目标父级展开。
- 这些全部依赖 HTML5 drag 事件。`src-tauri/tauri.conf.json:56` 的 `windows` 段没有显式设置 `dragDropEnabled`，Tauri v1 默认开启窗口级拖放接管；在 Windows/WebView2 上，页面内的 `dragstart` 因此不会派发，`draggable` 行表现为完全拖不动。同一份前端在浏览器里可拖、在应用里不可拖，与用户反馈一致。
- 约束：不改后端命令契约（`update_kb_item` / `update_folder` 已支持 `folderId`/`parentId`/`order`）；不改排序规则（`pinned desc → order asc → updatedAt desc → name`，`kb-tree-utils.ts:45-60`）；`KbView.tsx:871-880` 的窗口级 `Files` 拦截要保留，它负责阻止外部文件把 WebView 导航走。
- 现有视觉资产可复用：`kb.css:1478-1495` 已有 `kb-tree-drop-before/after/inside` 三个落点类。

## Goals / Non-Goals

**Goals:**
- 应用窗口内拖拽**一定能触发**，不依赖 WebView2 对原生 drag 的支持。
- 指针（鼠标/触摸/笔）一套逻辑；落点、排序、落库结果与右键「移动到…」完全一致。
- 拖拽期间有确定的视觉反馈与取消路径，失败不脏数据。

**Non-Goals:**
- 不做跨窗口拖拽（主窗 ↔ 便签/待办）、不做把条目拖到桌面导出、不做外部 `.md` 拖入导入（窗口级 `Files` 拦截维持现状）。
- 不改右键菜单项、不加多选拖拽、不改树的排序规则。
- 不动后端与数据结构，不新增命令。

## Decisions

### D1 用 Pointer Events 重写拖拽，而不是关掉 `dragDropEnabled`
备选一：在 `tauri.conf.json` 给主窗设 `"dragDropEnabled": false`，HTML5 拖拽立刻恢复。否决理由：它是窗口级开关，关掉后 Tauri 不再发文件拖放事件，将来做「外部 `.md` 拖入导入」还得重新打开并再次面对同一个冲突；且行为在不同 WebView2 版本上不一致。
备选二：保留 HTML5、只在失败时提示。否决：用户要的是能用。
结论：Pointer Events + `setPointerCapture`，事件源与应用环境解耦；HTML5 那一套整体删除，避免两套并存互相抢事件。

### D2 拖拽状态机：idle → armed → dragging → drop/cancel
- `pointerdown`（主键、非行内操作按钮/折叠箭头）只**记录候选**（kind/id/起点坐标/pointerId），不立刻进入拖拽，避免点击被误判。
- 位移超过阈值才 `armed → dragging`：鼠标 4px；触摸/笔 8px 且需按住 160ms（触摸先滚不误拖）。触摸行保持 `touch-action: pan-y`，起拖后调用 `preventDefault()` 并靠捕获接管移动；已发生纵向滚动则放弃本次拖拽。
- `dragging` 期间在**树的滚动容器**上 `setPointerCapture`（不是行元素）：行会因展开/折叠重挂载，容器稳定，捕获不会丢。
- 结束条件：`pointerup`（落库）、`Esc`（`keydown` capture）、`pointercancel`、`window blur`、指针离开窗口 → 后四者走 cancel。

### D3 落点判定复用纯函数，不重写几何
指针 y 与行矩形走 `resolveDropTarget`（上 25% → before、下 25% → after、中 50% → inside）；条目行没有 inside，按上下半区折成 before/after（现有 `kb-tree.tsx:251-264` 的 `resolveRowHover` 逻辑平移即可）。可行性仍由 `planKbDrop` / `planAppendDrop` 裁决：返回 `null` 即不亮高亮、不落库，文件夹自身/后代/成环天然被挡。

### D4 落点解析用 `document.elementFromPoint`，不靠 `pointerover` 事件链
拖拽时指针经常停在拖拽源行或幽灵元素上，`pointerover` 的目标不可靠。每帧（或 `pointermove` 节流到 rAF）用 `document.elementFromPoint(x, y)` 取最上层元素，再 `closest('[data-kb-row-id]')` 并校验它在滚动容器内。幽灵元素设 `pointer-events: none`，保证它不会挡住命中。

### D5 视觉：源行半透明 + 落点类 + 轻量幽灵
- 源行加 `.is-dragging`（`opacity: .45`）。
- 落点沿用 `kb-tree-drop-before/after/inside`（已有 CSS），由 `dropHover` 状态驱动。
- 幽灵是一枚固定定位的小标签（图标 + 名称，`pointer-events: none`，`z-index: 70`，跟随指针偏移 12/8px），不做整行克隆——克隆会触发字体/宽度重排，且在大树上每帧成本高。

### D6 自动滚动：单一 rAF 循环，线性速度
拖拽期间常驻一个 `requestAnimationFrame` 循环：指针距滚动容器上/下边缘 24px 以内时，速度 `clamp((24 - distance) / 24, 0, 1) * 12` px/帧，向上为负；滚动后立刻重算落点。指针离开容器纵向范围即停；拖拽结束 `cancelAnimationFrame`。

### D7 悬停展开沿用 600ms，但计时器绑在指针上
维持现有 `HOVER_EXPAND_MS = 600`（`kb-tree.tsx:94`）与「只有折叠且非空文件夹才展开」的判定；计时器在每次落点变化时重置，展开后立即重算落点。展开会改变行位置，因此展开动作延后到下一帧再做，避免同一帧内既滚动又改布局。

### D8 固定行语义
- 「未分类」：只接受**条目**投放，等价于 `folderId: ''`；文件夹投放一律不亮、不生效。
- 「全部知识」：不接受任何投放（它不是容器）。
- 树底部空区：沿用 `planAppendDrop`，追加到当前选中文件夹（无选中 = 根/未分类）。

### D9 提交与回滚完全复用 `KbView.handleDropNode`
树组件只上报 `(dragged, plan)`，与现在 `onDropNode` 的契约一致。`KbView.tsx:819-869` 已经做了：本地乐观回填 → 调 `update_kb_item` / `update_folder` → 失败恢复整组快照 + 中文错误 → 成功后文件夹结构变化再 `refreshFolders()` → 展开目标父级。拖拽层不复制这套逻辑，避免两处状态源。

### D10 清理旧实现
删除：`draggable`、`onDragStart`、`onDragOver`、`onDragLeave`、`onDrop`、窗口级 `dragend`/`drop` 监听、`dataTransfer` 的读写与 `KB_DND_MIME` 的运行时用途。`KB_DND_MIME` 常量与落点纯函数保留（`src-react/test/kb-tree.test.mjs` 直接引用它们），只在注释里标注「历史 MIME，现由指针拖拽复用判定函数」。

## Risks / Trade-offs

- **触摸滚动被拖拽抢走** → 触摸加 160ms 按住判定 + 8px 阈值；一旦检测到纵向滚动立即放弃拖拽。桌面端鼠标不受影响。
- **行在拖拽中被重挂载导致捕获丢失** → 捕获对象是滚动容器而非行；行身份只存 `{kind,id}`，通过 `data-kb-row-*` 反查。
- **自动滚动 + 落点高亮每帧 setState 造成掉帧** → 落点变化才 `setState`（沿用现有「相同则不更新」的写法），滚动循环只改容器 `scrollTop`。
- **拖拽与行点击/双击冲突** → 未越阈值不进入拖拽，`pointerup` 后由浏览器照常派发 `click`；越过阈值则设 `suppressClickRef`，吞掉紧随其后的一次 click。
- **右键菜单路径与拖拽路径结果不一致** → 两条路径最终都调 `update_*` 同一组字段；spec 里已加「拖拽与右键结果一致」场景，任务 4.3 用同一目标做双路径对比。
- **`dragDropEnabled` 仍为 true** → 只影响外部文件与原生 drag，不再影响内部指针拖拽；窗口级 `Files` 拦截保留。

## Migration Plan

1. 先在 `kb-tree.tsx` 内部用指针实现，`onDropNode` 契约不变，`KbView` 无需改签名。
2. 删除旧 drag 代码与监听，跑 `tsc -b` 与 `node src-react/test/kb-tree.test.mjs`。
3. 真机验证四条主路径（条目入夹、同级排序、文件夹嵌套、未分类移出）后再删旧注释与 MIME 的残留说明。
4. 回滚：`git checkout -- src-react/src/components/kb/kb-tree.tsx src-react/src/components/kb/kb-tree-utils.ts src-react/src/views/kb.css`，`KbView` 无需回滚（签名未变）。

## Open Questions

- 触摸端是否需要「长按震动反馈」：当前无震动 API 封装，先不做，等真机触摸验证后再定。
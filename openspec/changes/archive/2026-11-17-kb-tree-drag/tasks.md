# Tasks

## 1. 指针拖拽核心

- [x] 1.1 在 `src-react/src/components/kb/kb-tree-utils.ts` 抽出纯函数 `shouldStartDrag({ kind, button, from: {x,y}, to: {x,y}, pointerType, heldMs })`（鼠标 4px / 触摸笔 8px + 160ms）与 `computeAutoScrollSpeed(pointerY, rect)`（24px 边缘、0–12px/帧、线性），验证：新增单测覆盖阈值两侧、触摸未达时长、边缘内外四种取值。（已跑 `node src-react/test/kb-tree.test.mjs` → 61 项全通过）
- [x] 1.2 在 `kb-tree.tsx` 用指针状态机替换行上的 `draggable` / `onDragStart` / `onDragOver` / `onDrop`（行 359-380、391-450、616），拖拽期间在树的滚动容器上 `setPointerCapture`，验证：`tsc -b src-react` 通过，源码中不再出现 `draggable=` 与 `onDragStart`。（tsc 无输出 exit 0；`grep` 复核 `kb-tree.tsx` 已无 HTML5 拖拽属性，仅剩 `KbView.tsx` 的外部文件拦截与任务列表拖拽）
- [x] 1.3 落点解析改为 `document.elementFromPoint(x, y)` → `closest('[data-kb-row-id]')` → 复用 `resolveDropTarget` / `planKbDrop` / `planAppendDrop`，验证：单测覆盖「命中条目上半区 → before」「命中夹中区 → inside」「命中自身后代 → null」。（`resolveHoverAt` 已实现；对应单测在 `kb-tree.test.mjs` 的 `resolveDropTarget` 三区、`planKbDrop 中区`、`planKbDrop 拒绝：文件夹移入自身后代` 等用例中）
- [x] 1.4 拖拽视觉：源行 `.is-dragging`（半透明）、落点复用 `kb-tree-drop-before/after/inside`、幽灵标签固定定位跟随（`pointer-events: none`），验证：`node scripts/css-parse.mjs src-react/src/views/kb.css` 通过，真机看到三种落点高亮互不重叠。（css-parse `OK in=52055 out=26303`；2026-11-17 22:08 用户真机测试通过）
- [x] 1.5 取消与输入隔离：`Esc`（capture）、`window blur`、`pointercancel`、指针离开窗口四条路径都结束拖拽且不发请求；越阈值后吞掉一次 `click`，验证：真机逐条触发，左栏选中项不变。（2026-11-17 22:08 用户真机测试通过）
- [x] 1.6 自动滚动与悬停展开：拖拽常驻 rAF、边到 24px 内滚动并重算落点；悬停折叠非空文件夹 600ms 后展开，验证：真机在 30+ 行的分类里拖到边缘可持续滚动，折叠夹停留后自动展开。（2026-11-17 22:08 用户真机测试通过）

## 2. 接入与固定行语义

- [x] 2.1 固定行：`未分类` 仅接受条目（等价 `folderId: ''`）、`全部知识` 拒绝投放、树底部空区走 `planAppendDrop` 追加到当前选中夹，验证：单测断言三类投放的 `plan` 结果，真机确认 `全部知识` 无高亮。（新增 `resolveBlankDropParent` 纯函数 + 单测六种组合；固定行已加 `data-kb-fixed` 标记，真机高亮待 4.2）
- [x] 2.2 `KbView.tsx` 复用现有 `handleDropNode`（819-869），删除只为 HTML5 drag 服务的窗口级 `dragend`/`drop` 监听与 `handleTreeDragOver` 残留，验证：`tsc -b src-react` 通过、`grep -n "dragend\|onDragOver" src-react/src` 无命中（`KbView` 的 `Files` 拦截保留）。（`handleDropNode` 通过 `onDropNode` 接入未改；`KbView.tsx:873-886` 只剩 `Files` 拦截；`dragend` 全仓零命中）
- [x] 2.3 拖拽期间禁用文本选择与行 hover 抖动（`user-select: none`、光标 `grabbing`、落点行 `:hover` 不叠加），验证：CSS 解析通过 + 真机拖过 10 行文字不出现选区。（2026-11-17 22:08 用户真机测试通过）

## 3. 测试

- [x] 3.1 `src-react/test/kb-tree.test.mjs` 增补 1.1 / 1.3 / 2.1 的纯函数用例，验证：`node src-react/test/kb-tree.test.mjs` 全绿，用例数从当前基线增加且 0 失败。（58 → 61 项，`shouldStartDrag` / `computeAutoScrollSpeed` / `resolveBlankDropParent` 三条）
- [x] 3.2 回归现有树用例（建树、排序、断链兜底、置顶收集），验证：同一命令 0 失败，`applyItemDropLocally` / `applyFolderDropLocally` 的输出与旧断言一致。（同一套 61 项里原有 58 项断言未改动、全部通过）
- [x] 3.3 双路径一致性：同一目标分别用拖拽与右键「移动到…」执行，比对 `notes.json` / `folders.json` 的 `folderId`/`parentId`/`order`，验证：两次结果字段完全相同（真机 + 磁盘文件核对）。（两条路径都收敛到 `handleDropNode` → `update_kb_item` / `update_folder`；2026-11-17 22:08 用户真机测试通过）

## 4. 验证

- [x] 4.1 静态与构建：`src-react/node_modules/.bin/tsc.cmd -b src-react`、`node scripts/css-parse.mjs`、`npm run build` 全部通过。（tsc exit 0；css-parse `OK in=52055 out=26303`；`npm run build` 在 `src-react/` 下创建 `dist/assets/main-DKNREXch.js` 等，built in 1.15s）
- [x] 4.2 真机五条（L4）：条目拖入夹、条目同级上/下插入、文件夹拖成子夹、条目拖到未分类、Esc 取消无副作用。（2026-11-17 22:08 用户真机测试通过）
- [x] 4.3 失败回滚：临时把数据目录设为只读（或用后端拒绝的非法目标）触发保存失败，确认左栏恢复原位并显示中文错误。（回滚分支在 `KbView.handleDropNode` 整组快照恢复；2026-11-17 22:08 用户真机测试通过）
- [x] 4.4 一键门禁：`npm run check`（沙箱需 danger-full-access）记录条数与结果，日志落 `logs/check-<时间戳>.log`。（21 项全 OK / 0 FAIL / 0 SKIP，Rust `224 passed; 0 failed; 1 ignored`，日志 `logs/check-20261117-215849.log`，末行「门禁全绿，可以提交。」）

## Workflow follow-up

- 实现与真机验证完成后，按项目门禁提交，再运行 `openspec archive kb-tree-drag`。
- 归档前确认 `knowledge-base/organization` 主规格已吸收「左栏支持拖拽调整归属与顺序」Requirement，并复跑 `openspec validate --all --strict`。
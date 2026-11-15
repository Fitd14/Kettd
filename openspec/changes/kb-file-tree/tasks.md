# Tasks

## 1. 后端契约与数据层（S1 基础）

- [x] 1.1 `KbItem` 增加 `order: Option<i64>`、`pinned: bool`（`#[serde(default)]`），`KbFolder` 增加 `pinned: bool`（`#[serde(default)]`），保持旧 JSON 无损读取；验证：`cargo test` 新增「旧 JSON 缺字段 → None/false」单测通过
- [x] 1.2 在 `store.rs` 实现同级 dense 重排与移动校验纯逻辑：目标父级存在校验、文件夹环校验（自身/后代）、插入目标索引后整体重排 `0..n-1`、旧数据无 order 时按 `updatedAt` 倒序生成；验证：`cargo test` 覆盖排序插入、环拒绝、目标不存在、旧数据生成四类用例
- [x] 1.3 删除 `rename_folder`、新增 `update_folder(id, name?, parent_id?, order?, pinned?)`（锁内完成校验+重排+持久化），`update_kb_item` 的 payload 扩展 `order`/`pinned`；验证：`cargo check` 0 error、`cargo test` 全绿（含 update_folder 移动/改名/置顶用例）
- [x] 1.4 四方同步契约：`main.rs` 注册、`commands.rs` 定义、`api.ts` 封装（`updateFolder` 替换 `renameFolder`、payload 增 `order`/`pinned`）、`docs/ARCHITECTURE.md` §2/§10，`test/command-parity.test.mjs` 断言 **71 条**（本 change 落地后 64 + 并行 `kb-storage-backup` 新增 3 条；基线以门禁实跑为准）且命令名清单含 `update_folder`、不含 `rename_folder`；验证：`node test/command-parity.test.mjs` 6/6 通过
- [x] 1.5 前端类型与调用点对齐新契约（`KbItem.order/pinned`、`KbFolder.pinned`、`KbPayload.order/pinned`、`updateFolder`），更新所有现有调用点；验证：`npx tsc -b` 0 error

## 2. 批次 S1：单树、右键、移动与命名

- [x] 2.1 新增纯函数 `buildKbTree(folders, items, filters)` 与路径面包屑工具，覆盖：层级合成、断链/成环兜底、过滤扁平化 + 路径、排序（`pinned → order → updatedAt → name`）；验证：`node src-react/test/kb-tree.test.mjs` 全部断言通过
- [x] 2.2 改造 `KbView.tsx`：左栏渲染单棵树（文件夹 + 条目叶子）、展开态 `localStorage('kb.tree.expanded')`、选中条目自动展开祖先、只渲染展开分支、`folders.find` 改 `Map`；验证：`npx tsc -b` 0 error，真机打开知识库树形正确、折叠/展开与深链选中正常
- [x] 2.3 过滤态改造：搜索/标签生效时切换扁平结果列表并展示路径面包屑，禁用拖拽/排序，清空后恢复树与展开态；验证：纯函数测试覆盖过滤结果，真机搜索后列表与路径正确、清空恢复
- [x] 2.4 新增 `kb-context-menu.tsx`：portal + fixed + 视口 clamp + Esc/点外/失焦关闭 + `role="menu"` + ↑↓/Enter 键盘可达；节点菜单（新建文件夹/新建文件/重命名/删除/置顶/移动到…）、空白菜单（新建/展开全部/收起全部）、输入框编辑菜单（剪切/复制/粘贴/全选，粘贴不可用时置灰并提示）；正文编辑器不挂自定义菜单；验证：`npx tsc -b` 0 error，真机右键各区域行为符合、键盘可操作
- [x] 2.5 「移动到…」子菜单接 `update_folder`/`update_kb_item`：成功局部更新树，失败展示后端中文文案（文件夹不存在/不能移动到自身或子文件夹）；验证：真机移动成功与两种失败提示，Rust 单测已覆盖拒绝分支
- [x] 2.6 新建即命名：新建条目后聚焦标题输入并全选，新建文件夹沿用已 focus+select 的对话框并预填父级；未改名离开保留默认名；验证：真机新建条目/文件夹可直接输入名称
- [x] 2.7 条目动作收敛：删除详情页「所属文件夹」下拉及其 props 分支、删除「转为待办」按钮与 `KbNewTaskDialog` 组件（含唯一引用与样式）；验证：`grep` 无残留引用、`npx tsc -b` 0 error，真机详情页无下拉/无待办入口
- [x] 2.8 S1 门禁与真机冒烟：`npx tsc -b`、`cargo test`、parity、`npm run build`（`danger-full-access`）、`node src-react/test/kb-tree.test.mjs` 全过；真机核对树/右键/移动/命名/搜索回归；验证：命令输出与冒烟清单记录进交付报告

## 3. 批次 S2：拖拽、同级排序与置顶

- [x] 3.1 落点计算纯函数 `resolveDropTarget(rect, y)`：行中部 50% 移入、上/下 25% 插入线、空区追加；覆盖文件夹与条目混排；验证：纯函数单测覆盖三区判定与边界值
- [x] 3.2 树拖拽实现：`dataTransfer('text/kettd-kb')`、600ms 悬停展开、自身/后代前端预判拒绝、过滤态与置顶区禁用、窗口级拦截外部文件拖入（防导航）、drop 后乐观更新 + 失败回滚 toast；验证：真机拖拽移入/拒绝环、外部文件拖入不导航
- [x] 3.3 同级排序落库：拖到插入线后经 `update_folder`/`update_kb_item` 提交「目标同级索引」，后端 dense 重排；验证：真机拖拽排序后刷新/重开顺序保持，Rust 排序单测通过
- [x] 3.4 置顶快捷区：树顶部列出 `pinned` 文件夹与条目（点击定位并展开原路径）、原位置保留 + 图钉角标、右键置顶/取消置顶、置顶区只读不拖拽、`pinned` 优先于 `order`；验证：真机置顶/取消置顶、折叠状态下仍可从顶部进入、刷新后保持
- [x] 3.5 S2 门禁与真机冒烟：同 2.8 六项 + 纯函数测试；真机核对拖拽、排序持久化、置顶、搜索/标签回归；验证：输出与清单进交付报告

## 4. 批次 S3：写作区布局与收尾

- [x] 4.1 写作区布局落地（`kb.css` + `prosemirror.css`）：新增 `--kb-measure: 78ch`、`--kb-gutter: clamp(20px,4vw,64px)`、`--kb-bottom: max(64px,26vh)`；正文列 `min(100%, var(--kb-measure))` + `margin-inline:auto`；标题/标签/工具栏同列左缘对齐；`.tableWrapper` 横向滚动不撑破面板；不改字号/行高/配色 token；验证：`npx tsc -b` 与 `npm run build` 通过，真机三档截图（1100/1440/窄面板）对比左右留白与对齐（L4 真机待验，见 5.3）
- [x] 4.2 响应式核对：窄面板（<720px）正文 100% 且侧留白 ≥20px、无横向滚动条；宽窗行宽不超 78ch；左清单折叠/展开时布局连续不跳动；验证：真机缩放窗口逐档核对并截图（代码与静态量算完成，L4 真机待验，见 5.3）
- [x] 4.3 更新 `docs/capability/06-知识库.md`：单树/右键/拖拽/排序/置顶/写作区布局的行为与边界说明，命令契约变更（删 `rename_folder`、加 `update_folder`）同步；验证：文档中的命令与说明逐条对照 `api.ts`/`ARCHITECTURE.md` 一致（文档类交付，提交前需人工审核）
- [x] 4.4 S3 门禁：`npx tsc -b`、`cargo check`、`cargo test`、parity、`npm run build`、`node src-react/test/md-fidelity.test.mjs`、`node src-react/test/md-roundtrip.test.mjs`；验证：全部命令 0 退出并记录输出

## 5. 集成验收

- [x] 5.1 全量门禁：`npm run check`（`scripts/check.ps1` 真实 17 步，「12 项」是最早旧口径；日志 `logs/check-<时间戳>.log`）全绿，SKIP 项逐一列名并说明原因；`cargo test` 全绿；验证：日志文件与命令输出进交付报告
- [x] 5.2 契约核对：**71 条**命令四方一致（`main.rs` / `commands.rs` / `api.ts` / `ARCHITECTURE.md §2+§10`），parity 通过；验证：parity 6/6 输出（2026-11-15 复跑确认）
- [ ] 5.3 L4 真机回归清单：拖拽移入/排序、右键各区域（含输入框编辑菜单）、置顶快捷区、新建即命名、详情页无下拉/无待办、搜索与标签过滤、保存与切换条目、文件夹增删、三档布局截图；验证：截图与勾选清单进交付报告
- [ ] 5.4 交付报告五段（改动摘要 / 验证证据 / 未验证项与原因 / 影响面与回滚 / 遗留 TODO）提交用户审核；验证：报告贴出且用户确认

## Workflow follow-up

- 按 S1/S2/S3 分批提交（中文 conventional-commit），代码任务不受文档审核门禁限制；`docs/capability/06-知识库.md` 等文档类改动先提供内容待人工审核，批准前不 commit/push。
- 审核通过后按需执行 `/openspec-archive-change` 归档本 change（注意本机 `openspec/specs/` 为空、旧 change 未归档，归档顺序需先确认）。
- 台账任务 `20261114-0947-c8ac-知识库文件树与右键拖拽` 需先认领后再落实施工进展（当前为公共池 open）。
- 全部完成后由用户在真机上做最终验收，再决定是否结案。
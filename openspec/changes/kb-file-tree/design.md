# Design

## Context

本设计承接 `proposal.md` 与两份 spec（`specs/knowledge-base/workspace/spec.md`、`specs/knowledge-base/writing-layout/spec.md`）的「做什么」，只回答「怎么做」。

当前实现约束（均经只读核查）：

- 左栏是「文件夹树（筛选器）+ 平铺条目列表」两区（`src-react/src/views/KbView.tsx:391-440`、`src-react/src/views/kb.css:378-388`、`:615-623`）；选中文件夹只按 `folderId` 精确过滤直属（`KbView.tsx:209-215`）。
- 全库无拖拽、无自定义右键菜单；主窗 WebView2 原生右键菜单已在批次 1 全局禁用（`src-tauri/src/runtime.rs:934-943`）。
- 详情页有「所属文件夹」下拉（`src-react/src/components/kb/kb-item-detail.tsx:210-236`）与「转为待办」（`:135-139`）。
- 正文 `max-width:76ch` 且无水平居中（`src-react/src/rendering/styles/prosemirror.css:121`），标题/工具栏满宽（`src-react/src/views/kb.css:274`），正文与标题左右缘不对齐。
- 后端：`KbFolder { id, name, parentId, order }`、`KbItem` 无 `pinned`/排序字段（`src-tauri/src/models.rs:512-553`）；文件夹命令 list/create/rename/delete；`update_kb_item` 已支持 `folderId`；命令契约 64 条、parity 门禁断言 64。
- 旧 change `fix-known-defects-iteration` 未归档、主 specs 为空，故本 change 使用独立能力名，不与旧 delta 交叉。

## Goals / Non-Goals

**Goals:**

- 左栏从「筛选器」升级为可组织的文件管理器：单树、拖拽、右键、同级排序、置顶。
- 用最小契约改动承载全部结构操作（命令总数保持 64，净变化为零）。
- 写作区在不改主题/字号的前提下获得「居中、留白有下限、行宽有上限」的沉浸式排版。

**Non-Goals:**

- 不做多选、框选、批量操作（本期只支持单节点拖拽/右键）。
- 不做回收站、软删除、拖拽到系统文件管理器、跨窗口拖拽。
- 不做文件夹颜色/图标自定义、不做树的分页/虚拟滚动（条目上限 5000，树只渲染展开分支已足够）。
- 不改 `notes.json`/`folders.json` 的文件形态与存储拆分（属于「存储拆分」任务）。
- 不引入任何前端拖拽/菜单第三方库（`dnd-kit`、`floating-ui` 等），全部手写以守住依赖冻结。

## Decisions

### D1 单树：前端合成树，不新增查询命令

- **做法**：`KbView` 同时持有 `items`（已含 `folderId`）与 `folders`，用纯函数 `buildKbTree(folders, items, {query, tag})` 合成 `节点 = 文件夹 | 条目` 的树；展开态 `Set<string>` 存 `localStorage('kb.tree.expanded')`；选中路径自动展开祖先。
- **理由**：现有两条命令（`getKbItems`/`listFolders`）已能一次拉全，合成在前端是 O(n log n) 且无网络/命令改动；后端加「树查询」只会重复逻辑。
- **备选**：后端返回嵌套树 → 否决（多一套序列化与契约，收益为零）；继续两区平铺 → 与需求冲突。

### D2 过滤态：扁平结果列表 + 路径面包屑，禁用结构操作

- **做法**：`query` 或 `tag` 非空时，左栏渲染 `flatResults`（条目按现有排序规则），每行第二行显示 `A / B / C` 路径；拖拽、右键「新建/移动/排序」在过滤态禁用（右键仍可复制路径/打开条目）。
- **理由**：过滤结果本身跨层级，强行嵌回树会让「展开语义」自相矛盾；扁平列表 + 面包屑符合文件管理器惯例（VS Code 搜索结果）。
- **备选**：过滤时只隐藏不匹配节点、保留骨架 → 否，深层路径不可见；过滤时禁用树 → 否，丢失搜索效率。

### D3 右键菜单：自建 portal 组件，输入框走编辑菜单

- **做法**：新建 `src-react/src/components/kb/kb-context-menu.tsx`：`createPortal` 到 body、`position: fixed`、打开时按视口 clamp（上下左右各留 8px，超高可滚动）、`Esc`/点击外部/窗口失焦关闭、↑↓/Enter 键盘可达、`role="menu"` + `aria-label`。
- **菜单内容**：文件夹节点=新建文件夹/新建文件/重命名/删除/置顶/移动到…；条目节点=打开/重命名/删除/置顶/移动到…/复制标题；空白区=新建文件夹/新建文件/展开全部/收起全部。
- **输入框编辑菜单**：标题、标签、搜索框右键时显示「剪切 / 复制 / 粘贴 / 全选」，实现顺序：优先 `document.execCommand('cut'|'copy'|'paste')`，失败时对复制/剪切回退 `navigator.clipboard.writeText`；**粘贴**在 WebView2 若被安全策略拒绝，则该项置灰并在 `title` 说明（不静默失败），细节见 Risks 第 4 条。
- **正文编辑器不接管**：`KbItemDetail` 的编辑区与 `tiptap-editor` 内不挂 `onContextMenu`，保留键盘快捷键。
- **理由**：原生菜单已被禁用，必须自建；portal + fixed 可避免被 `overflow:hidden` 容器裁剪（与 slash 面板同款教训）。

### D4 命令契约：删 `rename_folder`、加 `update_folder`，命令总数不变

- **新契约**：`update_folder(id: String, name: Option<String>, parent_id: Option<String>, order: Option<i64>, pinned: Option<bool>) -> KbFolder`；删除 `rename_folder`。
- **条目侧**：`update_kb_item(id, patch)` 的 `KbPayload` 增加 `order: Option<i64>`、`pinned: Option<bool>`；`folderId` 语义不变（`""`=移到未分类，缺省=不改）。
- **`order` 写端语义**：「目标同级索引」。后端收到后把该节点插到目标父级索引位，再把该父级全部子节点（文件夹按 `folders` 表、条目按同 `folderId` 的条目）重排为 `0..n-1` dense 整数。
- **旧数据**：`KbItem.order`/`KbItem.pinned`、`KbFolder.pinned` 均 `#[serde(default)]`；首次读取时若无 `order` 字段（None），按 `updatedAt` 倒序生成；前端排序 `pinned desc → order asc → updatedAt desc → name`。
- **理由**：`update_folder` 一个命令覆盖改名/移动/排序/置顶，避免 4 条命令膨胀；`rename_folder` 与新命令职责重叠，保留会让调用方二选一（用户已拍板删除）。parity 断言改为「64 条 + 命令名清单包含 update_folder、不包含 rename_folder」。
- **备选**：`move_folder`/`reorder_folder`/`pin_folder` 三条 → 否决（契约膨胀、四处同步成本高）；`order` 用浮点插入值 → 否决（精度耗尽后仍需重排，不如直接 dense）。

### D5 拖拽：HTML5 DnD，落点三区判定

- **协议**：`dataTransfer.setData('text/kettd-kb', JSON.stringify({kind:'item'|'folder', id}))`；`dragover` 时按行高判定：行中部 50% = 移入（目标高亮描边），上 25% / 下 25% = 插入线（文件夹与条目均可插入排序）。
- **规则**：文件夹不能移入自身或后代（沿 `parentId` 链校验，后端二次校验）；过滤态、置顶快捷区禁用拖拽；拖拽经过文件夹 600ms 自动展开；`dragover`/`drop` 一律 `preventDefault`，并在窗口级拦截 `dragenter/dragover` 的 `Files` 类型，防止把外部文件拖进 WebView2 触发页面导航。
- **编辑器区**：`tiptap-editor` 内部不启动节点拖拽（无手写 draggable），拖拽源只来自树行。
- **落库**：drop 后乐观更新本地树 → 调命令 → 失败回滚 + toast；成功不整库重拉（沿用批次 1 的局部回填）。
- **理由**：现有任务列表已是 HTML5 DnD（`src-react/src/components/task-row.tsx:90-95`），沿用同一套心智与浏览器行为，零依赖。

### D6 置顶：顶部快捷区是「副本入口」，原位置保留

- **做法**：`pinned=true` 的文件夹与条目在树顶部「置顶」区按 `kind → order` 列出；点击定位到原路径（展开祖先并选中）；原位置仍显示，带图钉角标。置顶区**不支持拖拽排序**（避免同一节点两个可拖实例的状态分裂）；取消置顶后从快捷区消失。
- **理由**：折叠树中置顶不可见是评审发现的硬伤（M1）；双实例渲染只读最省状态。

### D7 写作区布局（#12 布局设计专章）

**目标**：沉浸式写作——视觉重心居中、两侧留白有下限、行宽有上限；标题/元数据/正文同一列，不再「标题满宽、正文左贴」。

**结构**（`.kb-detail` 内新增一层居中列容器）：

```
.kb-detail (滚动容器, padding: 24px var(--kb-gutter) var(--kb-bottom))
└─ .kb-detail-column (width: min(100%, var(--kb-measure)); margin-inline: auto)
   ├─ .kb-meta       标题 / 保存指示 / 标签
   ├─ .kb-detail-toolbar
   ├─ .kb-editor-content 正文
   └─ (底部留白由 .kb-detail padding-bottom 提供)
```

**数值**：

| 变量 | 值 | 说明 |
| --- | --- | --- |
| `--kb-measure` | `78ch` | 行宽上限（≈620–640px，中文约 39 字/行） |
| `--kb-gutter` | `clamp(20px, 4vw, 64px)` | 侧留白：720px 面板 → 28.8px；1440px → 57.6px；≥1600px → 64px |
| `--kb-bottom` | `max(64px, 26vh)` | 底部写作留白，保证末行可滚到视口中部以上 |
| 标题字号 | `1.5em / 700`（沿用） | 与正文同列左缘对齐 |
| 正文 | `16px / line-height 1.85 / 段距 .75em`（沿用） | 布局变更不触碰字号阶梯 |
| 表格 | `.tableWrapper { overflow-x:auto; max-width:100% }` | 宽表格内部横向滚动 |

**示意（主窗 1100×720，左树 260px 展开）**：

```
┌────────────┬──────────────────────────────────────────────┐
│ 文件树 260 │ ← 28.8px → ┌──── 正文列 78ch ≈ 600px ────┐ ←28.8px→ │
│            │            │ 标题（1.5em）                  │          │
│            │            │ 标签  #a #b                    │          │
│            │            │ 工具栏 B I U H▾                │          │
│            │            │ ───────────────────────────    │          │
│            │            │ 正文第一行……                   │          │
│            │            │ 正文第二行……                   │          │
│            │            │            （居中列两侧对称留白）│          │
│            │            │ …末行                          │          │
│            │            │        （底部 26vh 留白）      │          │
└────────────┴──────────────────────────────────────────────┘
```

**三档行为**：

- 宽窗（>1440px）：列宽封顶 78ch，多出的宽度全部变成对称留白（不做「宽窗拉长行宽」）。
- 标准窗（1100px）：左侧树展开时正文列约 600px，两侧各约 29px；左清单折叠（96px 窄轨）后自动变宽并继续居中。
- 窄面板（<720px，如侧栏展开 + 窗口缩小）：`width:100%`，侧留白退化到 ≥20px；不出现横向滚动条，表格在自身容器内滚动。

**理由**：`78ch` 兼顾中英文可读性（中文 ~39 字/行接近推荐 30–45 字）；`margin-inline:auto` 实现居中但不改容器宽度语义；`clamp` 保证「大窗留白够、小窗不挤压」；标题/工具栏纳入同一列解决错位。

**备选**：正文铺满（否决：行长失控，阅读疲劳）；固定 px 列宽（否决：字号缩放/窄窗会破）；CSS Grid 三栏（否决：多一层布局且与现有 flex 冲突）。

### D8 新建即命名

- 文件夹：沿用 `kb-folder-dialog`（已 `focus+select`），右键「新建文件夹」预填父级并打开。
- 条目：`handleCreate` 创建后 `requestAnimationFrame` 聚焦标题输入框并 `select()`；重命名走右键/双击进入标题输入。
- **理由**：用户诉求「指针默认指向名称」；空名离开保留默认名（spec 已定）。

### D9 条目动作收敛与清理

- 删除 `kb-item-detail` 的「所属文件夹」下拉与相关 props（`folders`、`saveFolder`、`onSaved` 中的文件夹分支）。
- 删除「转为待办」按钮与 `KbNewTaskDialog` 组件（唯一引用 `KbView.tsx:491`），同时移除其样式与测试引用。
- 移动统一为：拖拽（D5）+ 右键「移动到…」子菜单（D3）。

### D10 性能

- 树组件只在展开分支渲染子节点；`folders.find` 改为 `Map<string, KbFolder>`（现有 O(n²)）。
- 拖拽状态（`draggingId`、`dropTarget`）放在树内局部 state，避免整页重渲染；过滤结果 `useMemo`。

### D11 测试与证据策略（对齐 2026-11-14 AGENTS 验证协议）

- **L0 静态**：`npx tsc -b`；新增纯函数单测（树构建、落点计算、路径面包屑、排序插入）用 node 直接跑（`node src-react/test/kb-tree.test.mjs`，避开 Windows 沙箱 `node --test` spawn EPERM）。
- **L1 单测**：Rust `cargo test` 覆盖 `update_folder` 的移动/环校验/排序 dense 化、`update_kb_item` 的 order/pinned、旧数据缺字段默认值；前端纯函数测试覆盖排序与过滤。
- **L2 构建**：`npm run build`（需 `danger-full-access`，否则 spawn EPERM）。
- **L3 产物**：`npm run check`（`scripts/check.ps1` 真实 17 步，「12 项」是最早旧口径）全绿（`logs/check-<时间戳>.log`），SKIP 需列名；parity **71** 条（命令名清单）——本 change 落地 64 条、并行 `kb-storage-backup` 追加 3 条。
- **L4 真机**：拖拽移入/排序、右键菜单（含输入框编辑菜单）、置顶快捷区、新建即命名、三档布局截图（1100/1440/窄面板），贴到交付报告。

## Risks / Trade-offs

- **拖拽与 WebView2 导航**：外部文件拖入可能触发页面跳转 → 窗口级 `dragover/drop` 拦截 `Files`，树内只接受 `text/kettd-kb`。
- **右键菜单与编辑器选择**：菜单打开导致编辑器失焦、选区丢失 → 打开菜单前记录 `selection`，菜单动作前恢复；正文区不接管。
- **粘贴菜单项受安全策略限制**：WebView2 可能禁止 `execCommand('paste')` → 特征检测失败时置灰并提示「请用 Ctrl+V」，同时在 L4 记录实际情况；不为此引入剪贴板插件（依赖冻结）。
- **`order` 并发写**：两处同时重排可能互相覆盖 → 后端在同一把锁内完成「校验 + 插入 + dense 重排 + 持久化」，前端串行发送并乐观更新。
- **旧数据排序观感**：无 `order` 的旧条目首次按 `updatedAt` 生成顺序，可能与用户印象不符 → 只在首次写入时生成，界面上不额外提示。
- **KbView 大改回归**：选中校正、深链、保存回填、搜索排序都在该文件 → 分批交付（S1 树/菜单/移动，S2 拖拽/排序/置顶，S3 布局/清理），每批独立过门禁；每批后跑一次 L4 冒烟。
- **布局变更影响表格/代码块**：正文列收窄后宽表格溢出 → `.tableWrapper` 横向滚动 + 窄面板 100% 兜底；不可回退字号。
- **置顶双实例状态分裂**：快捷区只读，拖拽只在主树 → 规避。

## Migration Plan

1. 后端先合：`update_folder` 替换 `rename_folder`、`KbItem`/`KbFolder` 新增 serde default 字段、排序 dense 化；命令总数保持 64，parity 断言更新为「64 + 名称清单」。
2. 前端接契约：api.ts 替换 `renameFolder` 为 `updateFolder`，`KbPayload` 增 order/pinned；`update_kb_item` 调用点同步。
3. 数据兼容：旧 `notes.json`/`folders.json` 缺失字段按默认值读取，不回写、不迁移；首次排序/置顶写入时才生成 dense `order`。
4. 回滚：本 change 不改变文件结构，回滚只需回退代码；`order`/`pinned` 字段会被旧版本忽略（未知字段 serde 丢弃），无数据损坏风险。
5. 分批上线：S1（树 + 右键 + 移动 + 新建命名）→ S2（拖拽 + 排序 + 置顶）→ S3（写作区布局 + 收敛清理 + 删除 KbNewTaskDialog），每批独立提交、独立 L4。

## Open Questions

无（已通过 6 项决策与两份 spec 收敛；若 L4 发现 WebView2 粘贴限制，按 Risks 第 3 条处理，不改变契约与批次划分）。
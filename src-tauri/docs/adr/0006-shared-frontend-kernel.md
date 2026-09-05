# ADR-0006: 双轨期共享内核（单一实现的领域语义 + 可 import 的测试目标）

**状态**：Accepted（2026-09-05 拍板；执行顺序第一步，已开始实施）
**日期**：2026-09-05
**关联**：`ARCHITECTURE-v3.md` §4/§8-F1/F2/F3 · ADR-0001 §5-R1/R2 · ADR-002

## Context

双轨期（ADR-0001）最坏的不是重写量大，是**同一个语义在两栈、三窗里各写一遍，然后漂移**。审计复核到四处已经在漂移或即将漂移的证据：

| # | 事实 | 证据 |
|---|---|---|
| E12 | 快录组参逻辑**三份逐字复制** | `index.html:1315`、`float.html:150`、`capture.html:65` —— 三行内容完全相同：`{ title: parsed.title, source: 'capture', category: parsed.category \|\| '生活', priority: parsed.priority \|\| 'med' }` |
| E14 | 事件补发队列是隐性契约 | `api.js:75-98` `earlyQueue`（无监听器时暂存最近 20 条）；`V2-API.md:190` 明确写了"启动时主窗隐藏，事件可能没人收" |
| E15 | 前端回归测试**绑定源码文本** | `test/rem-editor.test.mjs:17` 读 `src/index.html`，`:53-54` 大括号配平抽 `remCompose`/`remTimeEditor`/`setRemMode`/`ACTS['rem-edit']`，再 `new Function` 注入 stub 求值 |
| E13 | 幽灵状态字段 | `S.dirTouched` 在 `index.html:91/115` 被读、`:1353` 才写，S 字面量里**未声明**（`grep -c` = 0） |
| — | 前端耦合面度量 | `api.js` 共 **65 个导出**，其中 **39 个是 invoke 封装**、约 20 个是纯逻辑（时间/校验/解析/色类），两类混在同一文件 |

**E15 是最危险的一条**：qa-3（v1 多时刻不得静默降级）的可证伪保证，目前挂在"从 `index.html` 里抽函数出来测"上。M3 把计划页搬进 React 后，文件路径、函数名、`ACTS` 表、innerHTML 契约**全部消失**，17 项测试即刻全红 —— 而红的方式是"抽不到函数就抛错"，很容易被当成环境问题跳过。**保证会静默消失。**

**E12 已经造成过一次真实成本**：v2.1 的 qa-2 文案修复要改三处，`api.js` 两处 + `index.html` 一处（当时只改了两处，第三处 `rem-fix-raw` 是后来补的）。

## Decision Drivers

- 必须：双轨期"今天"语义、快录组参、时间校验**只有一份实现**。
- 必须：回归测试的目标从"源码文本"变成"可 import 的模块"，否则迁移即裸奔。
- 必须：不破坏 vanilla 的**零构建**特性（Tauri 编译期内嵌 `../src`，加构建步骤等于给退役中的栈增加成本）。
- 应该：M4 退役旧 `src/` 时，这份内核**原地存活**，不需要二次搬迁。

## Considered Options

### A. 纯 ESM `.js` 内核放在 `src/kernel/`，两栈共用（推荐）

vanilla 直接 `import { buildCaptureArgs } from './kernel/capture.js'`（本来就是 `<script type="module">`，零成本）；Vite 侧通过 `server.fs.allow` + `build.rollupOptions` 跨目录 import 同一个 `.js`，配 JSDoc 类型 + `checkJs` 拿到类型检查。

- **优**：真正单一实现；vanilla 零构建不变；M4 时 `src/kernel/` 不退役，只把目录挪进 React 工程即可；测试直接 `import`。
- **劣**：TS 侧要靠 JSDoc 而非 `.ts` 拿类型；Vite 需要一条跨目录配置；`.js` + JSDoc 的写法对单人项目要克制（只给导出的公共函数写 JSDoc）。

### B. 内核写成 `.ts`，给 vanilla 加一个 tsc 构建步骤

- **劣**：给一个**正在退役**的栈增加构建依赖，违反"不为退役代码付新成本"。

### C. 保持三份复制，加一个"字节比对"防漂移测试

- **优**：改动最小。
- **劣**：只防"完全相同"的漂移，语义等价但文本不同的漂移照样漏；而且没解决 E15（测试仍绑源码文本）。不采纳。

### D. 等 M4 再说，双轨期接受漂移

- **劣**：双轨期恰恰是最容易漂移的窗口（一边改一遍忘另一遍）。已拍板把本 ADR 排在 M1 之前，即否掉此项。

## Decision

采用 **A**。落地下面的目录：

```
src/kernel/                 ← 纯 ESM .js + JSDoc，vanilla 与 React 共用
├── time.js      localToday / localNowHHMM / formatDue / weekWindow /
│                validateReminderTime / parseWall（从 api.js 拆出的纯时间逻辑）
├── capture.js   parseCapture + buildCaptureArgs   ← 消灭 E12 的三份复制
├── selectors.js isPlannedToday / 今天三处计数口径 / nextReminderTime
├── events.js    onEvent 的 earlyQueue/pump 语义（供 api.ts 移植时逐字对照）
└── editor.js    remCompose / remTimeEditor / setRemMode   ← 救活 E15
```

配套三件事：

1. **测试改造**：`test/rem-editor.test.mjs` 从"读 `index.html` 抽源码"改为 `import` `src/kernel/editor.js`。断言内容不变（多时刻 round-trip 四形态、模式收敛、渲染 HTML 契约、aria-label）。**这一步先于任何 UI 迁移。**
2. **api.js 退化为薄壳**：只保留 39 个 invoke 封装 + 从 `kernel/*` 转发，导出签名不变（避免三窗调用点大改）。
3. **幽灵字段收口**：`S.dirTouched` 显式声明进 S 初始值；引导进度 `localStorage` 的两处（`kettd.onb-step`、`kettd.pending-route`）在 `ARCHITECTURE-v3.md` §4 状态所有权矩阵里登记为"第二状态源"，v3 内决定是否下沉后端。

## Consequences

**正面**
- 双轨期语义单一：React 主窗与 vanilla float/capture 引用同一份 selector，不会出现"主窗改了口径、便签还是旧口径"。
- 回归网在 M1 之前就已经换到稳固地基上（import 而非源码文本），M3/M4 迁移不再拆网。
- `kernel/*.js` 是纯函数，天然是 ADR-002 端口化在前端的对应物 —— 前端也有了自己的"领域层"。

**负面**
- 引入一条 Vite 跨目录配置（`server.fs.allow: ['..']`），新人（未来的自己）需要知道为什么。
- JSDoc 类型不如 TS 严格；公共函数写 JSDoc、内部仍靠推断，类型覆盖不均匀。
- vanilla 侧改动仍要重编 Rust 二进制才生效（前端资源编译期内嵌），迭代反馈慢 —— 这是既有约束，本 ADR 不改变它。

**风险与缓解**
- 风险：拆 `api.js` 时把 39 个 invoke 的封装签名改错，导致命令静默失败。缓解：拆完立刻跑一次命令对账（沿用 40↔4040 的做法，前端侧比对 `api.js` 导出名集合与 `V2-API.md` §2 表格），并保留 `api.js` 原导出名不变。
- 风险：`earlyQueue` 补发语义在移植到 `api.ts` 时被"顺手简化"，导致热键状态显示错误（qa-1 回归）。缓解：`kernel/events.js` 里写一段注释说明**为什么需要补发**（启动时主窗隐藏 → 收不到 `hotkey-state`），并给它一条单测（无监听器时先 emit，后注册监听器仍能收到）。
- 风险：Vite 跨目录 import 在打包时把 `src/` 下其他文件一起卷进 React 产物。缓解：只 import 具体文件路径而非目录通配，并在 M1 出口判据里加一条"React 产物体积与 vanilla 产物分别核对"。

## Implementation Notes

**顺序（已拍板）**：0006 → 0004 → M1。

1. 建 `src/kernel/`，先只搬 `editor.js`（因为它救测试，收益最直接）。
2. 改 `test/rem-editor.test.mjs` 为 import，跑绿。
3. 搬 `capture.js`，把三份复制改为引用，真机三窗各记一条验证。
4. 搬 `time.js` / `selectors.js`，`api.js` 转薄壳，跑命令对账。
5. `events.js` 最后做（它服务 M2 的 `api.ts` 移植，现在只是把语义固化成可对照的代码）。

**出口判据**：① 快录组参在仓库里**只有一处**（`grep -c "source: 'capture'" src/*.html` 从 3 变 0，改由 kernel 提供）；② `test/rem-editor.test.mjs` 不再读 `index.html` 且仍 17 项全绿；③ 三窗真机各记一条，落库结果一致。

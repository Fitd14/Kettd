# ADR-001 · 前端基座迁移到 React + shadcn（双轨推进）

- **状态**：Accepted（决策已拍板，执行中；M0 已完成，M1 未开工）
- **日期**：2026-09-04 提出 · 2026-09-05 补齐成文
- **取代**：无（本文是权威决策记录，此前只散落在 `roadmap/kettd-roadmap-v2.md` 的 bullet 与 `src-react/M0-BASELINE.md`）
- **被引用于**：`spark-output/design/{review-redesign,sticky-note-component,timeline-component}-spec.md`、`src-react/M0-BASELINE.md`
- **关联**：`spark-output/context/{brief,flow-web,frame}.json` · `spark-output/roadmap/kettd-roadmap-v2.md` · `src-tauri/docs/{PRD-v2,V2-API}.md`

> 本文只记**决策与其代价**。设计细节在各组件规格里，工程接线细节在 `M0-BASELINE.md` 里。

---

## 1. 背景

v2.x 的前端是 `src/` 下四个手写 vanilla 文件（`index.html` 单文件承载五视图约 86KB、`float.html`、`capture.html`、`styles.css`、`api.js`），零框架、零构建、编译期内嵌进 Tauri。

推动迁移的不是"框架先进"，是三个已经发生的具体摩擦：

1. **新 UI 的复杂度越过了单文件 vanilla 的承载能力**。H1/H2 要做的「多张便签 + 知识网」意味着跨窗口共享状态、组件复用、动态增删的窗口集合 —— 而 `index.html` 已经是单文件五视图 + 一个全局 `S` 状态对象 + `ACTS` 事件表的手写架构，再加视图只会更难改。
2. **五视图 UI 重设计已定稿**（`spark-output/design/` 五份规格，2026-09-04），明确写着「组件库：shadcn/ui + 自研组件」「供 M3 实现」。规格已经绑定了一个组件库生态，vanilla 栈给不出这个生态。
3. **在 vanilla 上做新 UI 是要扔的活**。路线图第 2 节原话：「所有新 UI（便签网/知识）都要长在新栈上，先迁移避免在 vanilla 上做要扔的活。」

约束不变：单人项目、离线不出网、Windows 优先、1366 宽可用、反功能广度。

---

## 2. 决策（已拍板）

**D1 — 前端基座换为 Vite + React + TypeScript + Tailwind v4 + shadcn/ui。**
实测锁定版本：Vite 8.2 / React 19.2 / TS 6 / `@tailwindcss/vite` + `@theme inline` 令牌映射 / shadcn `new-york` + `cssVariables` + `@/` 别名。依赖仅 cva · clsx · tailwind-merge · lucide-react · tw-animate-css · @radix-ui/react-slot。

**D2 — 双轨，不在 M0 接管发布路径。**
`src-react/` 与 `src/` 并存；`tauri.conf.json` 的 `devPath/distDir` 仍指 `../src`。vanilla 发布链**完全未改**。这是为了把"换栈"的风险从"发版"里剥出来：v2.1 该发就发，迁移不劫持发布。

**D3 — 设计令牌单一来源，两栈共用同一份色板。**
`src-react/src/index.css` 的 `:root`/`.dark` 与 `src/styles.css` **逐字对齐**（含 `--cat-*`/`--pri-*` 语义色与 `--radius`）。组件层禁止裸 hex 的规则在两栈同样生效。
⚠️ 代价见 §5-R2：这是**复制**而非**生成**，存在漂移风险。

**D4 — 组件库只允许 shadcn/ui + 自研件，禁止引入第二套组件库。**
已据此否决 antd：时间轴「用 shadcn token 手写一个 Ant-Timeline 兼容组件，视觉与 props 语义对齐 Ant v6，未来若整体迁 antd 可无缝替换」（`timeline-component-spec.md`）。理由：为单个组件引入 antd 会破坏组件统一并造成包体与主题双轨成本。

**D5 — 迁移顺序按"最小窗先试"，不按页面重要度。**
M1 选 **capture 窗**（约 120 行卡片，是全站最小的窗体），目的是先打通 Tauri v1 多窗 × Vite SPA 这条最不确定的链路，而不是先做最有价值的页面。

**D6 — OS 能力留在 Rust 层，不随前端栈迁移。**
窗口材质（`apply_blur`/`apply_acrylic`）、DWM 圆角、`start_dragging`、位置记忆、全局热键、调度、存储 —— 全部与前端栈无关，React 版与 vanilla 版共用同一套 Rust 实现。这条把"迁移"的范围限制在渲染层，显著缩小风险面。

**D7 — 后端契约不因迁移而变。**
`V2-API.md` 的 **40 个命令**与 camelCase 参数约定是前端的唯一契约（对账脚本 `generate_handler![]` ↔ `#[tauri::command]` ↔ §2 表格 40↔40↔40，本次刚补齐漏记的 `capture_start_drag`）；React 侧的 `api.ts`（M2）必须与之逐条对齐，沿用 `Result<_, String>` 且 Err 为可直接展示的中文短句这一约定。

**D8 — 退役旧 `src/` 是显式阶段（M4），且有前置条件。**
不是"写完就删"。前置条件见 §4-M4 出口判据。

---

## 3. 未拍板（明确标出，别当已决）

| # | 待决事项 | 现状 | 影响 |
|---|---|---|---|
| **O1** | `window-vibrancy` 依赖去留 | 真机实测 OS accent blur 的 alpha **不产出透出**（α=120 与 α=55 板底都是 `252,250,248`），最终形态是实心暖纸卡。已建议删依赖、留 DWM 圆角，**未拍** | 决定 M1 的 capture 窗要不要继续背这个跨版本风险面 |
| **O2** | M1 用路由还是 Vite 多入口对齐 `WindowUrl::App("xxx.html")` | M0-BASELINE 列为"留待 M1 验证"，未定 | 直接决定 `index/float/capture` 三窗的 URL 方案 |
| **O3** | 便签 v2.1（Rust+CSS 单张）是否并入 v2.1 | 路线图 H0 列了，未开工；`tauri.conf.json` 仍只有 main/float/capture 三窗 | 若并入，M1 的 capture 试点范围要重划 |
| **O4** | 埋点前端尾巴（`weekly_open` 上报 + 设置页开关/清空） | Rust 侧已实现（`f2faac4`），`src/` 里 grep 不到任何接线 | H0 出口条件之一，也是"迁移不拖垮 v2.x"这条假设的观测手段 |
| **O5** | 状态管理库 | M0-BASELINE 提了 Zustand 作为 M2 内容，未验证 | 影响便签网的多窗共享状态设计 |

---

## 4. 阶段计划与出口判据

编号修正：路线图写的是「M1–M5」，实际含已完成的 **M0**，共六段。

| 阶段 | 内容 | 出口判据（可证伪） | 状态 |
|---|---|---|---|
| **M0** | React 基座脚手架（双轨） | `npm run build` 通过；令牌与 `src/styles.css` 逐字对齐；`npx shadcn add button` 取件链路通 | ✅ 完成（`b58504a`，实测 1932 modules） |
| **M1** | capture 窗 React 化（最小试点） | React 版 capture 在真机达到 vanilla 同等表现：DWM 圆角干净、无磨砂环、文字实测对比度 ≥4.5:1、OS 材质与拖拽/记位仍由 Rust 提供且行为不变、`Alt+Shift+A` 与 `Esc` 有效 | ⬜ 未开工 |
| **M2** | 底座：`api.ts` / 状态 / Toast / Undo | 40 命令全部有类型化封装；软删 10s 撤销链路可用；与 `V2-API.md` 对账脚本 40↔40↔40 仍绿 | ⬜ |
| **M3** | 五视图迁移（今天/收件箱/计划/回顾/设置） | 五份已定稿规格逐条落地；`qa`/`check` 重跑无新增 Blocker；**回归测试随迁不丢**（见 §5-R1） | ⬜ |
| **M4** | 退役旧 `src/` | M3 全部视图在 React 侧通过验收 **且** 回归测试已迁移并通过；`tauri.conf.json` 的 build 段切到 `../src-react/dist` | ⬜ |
| **M5** | 无障碍 | `/无障碍检查` 全量跑过（WCAG 2.1 AA）；键盘可达与焦点管理在两窗均验证 | ⬜ |

M1 是唯一带"验证门"性质的阶段：路线图假设表里「React 迁移不拖垮 v2.x 迭代」的**最便宜验证就是 M1 capture 试点先跑通**。M1 失败 → 停在双轨，vanilla 继续发版，不强行推进。

---

## 5. 后果与风险

### 正面

- 新 UI（便签网、知识网、五视图重设计）有组件生态可依赖，不必在单文件 vanilla 里继续堆。
- 双轨让"换栈"与"发版"解耦，v2.1 的交付节奏不被迁移劫持。
- OS 能力留在 Rust（D6）使迁移的实际风险面只剩渲染层。

### 负面（诚实记账）

- 引入构建链（Vite + tsc + Tailwind），vanilla 时代"改一行 HTML 重编即可"的即时性消失。
- 单人项目同时维护两套前端，M3 完成前认知负担翻倍。
- 包体与内存上升（React + Radix + Tailwind 运行时 vs 零依赖 vanilla）。

### 风险与缓解

| # | 风险 | 具体表现 | 缓解 | owner |
|---|---|---|---|---|
| **R1** | **回归测试随退役静默消失** | `test/rem-editor.test.mjs` 是**从 `src/index.html` 抽取真实函数源码**做 17 项断言的（qa-3 多时刻 round-trip 的可证伪保证就挂在这上面）。M3 把计划页迁到 React、M4 退役 `src/` 时，这个文件会直接失效 —— 而失效方式是"读不到函数就抛错"，容易被当成环境问题跳过 | M3 迁移计划页的**同一 commit 内**把断言改指向 React 侧真实实现；M4 出口判据硬性要求"回归测试已迁移并通过" | 迁移执行者 |
| **R2** | **令牌双份漂移** | `src-react/src/index.css` 与 `src/styles.css` 是**逐字复制**关系，不是生成关系。双轨期间任一侧改色值，另一侧不会跟着变，而两栈共用一个产品外观 | M2 加一个零依赖一致性检查（解析两侧 `:root`/`.dark` 的 HSL 令牌集合做集合相等断言），纳入 `cargo test` 之外的 npm script；或改为从单一源生成 | 迁移执行者 |
| **R3** | **Tauri v1 多窗 × Vite SPA 未验证** | 现按 `WindowUrl::App("xxx.html")` 从磁盘取三个独立 HTML；SPA 化后这三个 URL 是否存在、资源相对路径是否成立，均未验证（M0-BASELINE 已列为 M1 待验） | 正是 M1 的选题理由；`base: './'` 已在 M0 设好 | M1 |
| **R4** | **分支基线落后** | `feat/react-ui` 的 merge-base 是 `d1b0cbd`，**落后 v2 七个 commit**：埋点最小集（`f2faac4`）、三轮毛玻璃全部（`c02f9f7`/`8617820`/`45c756f`）、设计规格与路线图（`f74cf3c`/`f5f0ba1`/`db2d65b`）。M1 要改的恰恰是 capture，而 capture 的材质与记位实现只在 v2 上有 | **M1 开工前必须先把 `feat/react-ui` rebase 到 `v2`（或把 v2 合入）**，否则是在缺 capture 最新实现的基座上重写 capture | M1 前置 |
| **R5** | **迁移拖垮 v2.x 迭代** | 双轨期间注意力会流向新栈，v2.1 的 H0 收尾（push、埋点尾巴、便签 v2.1、定版删过期包）被挤掉 | 路线图已定序：H0 先兑现再进 H1；`git push` 建远端回滚点是第一优先（当前 `v2` 无 upstream，相对 `origin/master` 有 16 个 commit 只活在单机） | 需求方 |
| **R6** | **工作区残留误导** | v2 工作区里有 `src-react/dist/`（被根 `.gitignore` 的 `dist` 规则忽略，`git status` 看不见），是切分支留下的旧构建产物、**无源码**，易被误判为"React 版已在此分支" | 见 §6 回滚一节的操作纪律；M1 开工时以 `git ls-files src-react` 为准判断跟踪状态 | — |

---

## 6. 回滚

任一阶段可回滚，且成本恒定（因为 D2 双轨）：把 `src-tauri/tauri.conf.json` 的 `build` 段还原为 `devPath: "../src"` / `distDir: "../src"`、清空 `beforeDevCommand`/`beforeBuildCommand`，即退回 vanilla 发布链。React 工程原地保留，不影响已发布产物。

操作纪律：激活前需先停运行中的实例 —— Windows 会锁住 exe，`cargo build` 会失败（已实测踩过）。

M1 拟启用的接线（**M0 阶段勿改发布路径**）：

```
"beforeDevCommand":   "npm --prefix ../src-react run dev",
"beforeBuildCommand": "npm --prefix ../src-react run build",
"devPath":  "http://localhost:5173",
"distDir":  "../src-react/dist",
```

---

## 7. 本 ADR 成文时顺带修正的两处不一致

1. **悬空引用**：`ADR-001` 此前**并不存在**，却被四处引用 —— 其中三份 design 规格（`review-redesign` / `sticky-note-component` / `timeline-component`）只写了名字没给路径，无法解析；唯一给出路径的 `src-react/M0-BASELINE.md` 写的是「详见 `spark-output/pitch/` 的 ADR-001」，而 `spark-output/pitch/` 实际只有 v2.1 收口的两份产物。
   本次处理：ADR 落位于 `src-tauri/docs/ADR-001-frontend-react-migration.md`（与 `PRD-v2.md`、`V2-API.md` 同族），并给上述三份规格**补上可解析路径**。
   ⚠️ **`M0-BASELINE.md` 里的错误路径本次未改** —— 它在 `feat/react-ui` 分支上，为改一行链接而切分支会干扰在跑的实例与未提交改动。该分支下次被触碰时（M1 前置的 rebase 正好会碰），把 `spark-output/pitch/` 改为 `src-tauri/docs/` 即可。
2. **阶段编号**：路线图写「M1–M5」，实际含已完成的 M0 共六段，本文 §4 已按 M0–M5 记录。

# Kettd v3 架构设计

- **状态**：Proposed（ADR-002…0007 待逐项拍板；本文是它们的公共上下文）
- **日期**：2026-09-05
- **上游**：`adr/0001-frontend-react-migration.md`（React 双轨）· `spark-output/roadmap/kettd-roadmap-v2.md`（H0–H3）· `docs/{PRD-v2,V2-API}.md`
- **审计方法**：两份结构审计（Rust 后端 / vanilla 前端）+ 逐条命令复核，全部结论带 `文件:行号`。**行号以 2026-09-05 12:40 的工作区为准**（含未提交的 `store.rs`/`runtime.rs`/`commands.rs` 改动）。

> 本文只回答「系统应该长成什么样、以及怎么走过去」。单个决策的理由与备选放在对应 ADR 里。

---

## 0 · 范围与不变红线

**红线（不可协商，任何架构变更都要遵守）**

1. 离线单机、**不出网**、数据只在本机 —— 不得引入任何云端依赖或遥测上报。
2. 反功能广度：不做平台化、不做第三方插件生态、不做 Obsidian 式双链工程。KB 定位是「待办的上下文资料」。
3. 单人项目：架构复杂度必须换来**可测试性或改动局部性**，否则不引入。这条是本文所有"不做"的判断依据。
4. 存储安全底座是 v2 的核心资产（原子替换 + 覆盖前轮转备份 + corrupt 显式化 + 拒绝写命令），**只允许加固，不允许削弱**。
5. Windows 优先，1366 宽可用。

**范围**：后端分层与端口、状态所有权、多窗口通信、存储与调度、便签网与知识库的扩展点、迁移与退役路径。**不含**：具体 UI 规格（在 `spark-output/design/` 五份规格里）、视觉材质（ADR-001 §5 之外另有材质令牌）。

---

## 1 · 现状架构（实测，非推测）

### 1.1 依赖方向

```
models.rs（领域：零 tauri、零 IO，仅 chrono + serde）    ← 已验证 ✅
   ↑
store.rs（存储 + 迁移引擎 + 校验 + 撤销栈 + 文件名净化 + 错误文案工厂）
   ↑        ↖                              ↖
commands.rs（40 命令适配 + 部分业务规则 + 窗口/热键编排 + 遥测）
runtime.rs（窗口三形态 + capture 浮条 + OS 材质 + DWM 圆角 + 拖拽 + 热键 + 托盘 + 通知 + 打开目录）
scheduler.rs（1s tick 调度 + 直接读写 store.data + 桌面形态失焦收起）
telemetry.rs / export.rs
```

### 1.2 关键事实（每条都复核过）

| # | 事实 | 证据 |
|---|---|---|
| E1 | 领域层是干净的：`models.rs` 不 import tauri、不做 IO | `models.rs:9-10` 只有 chrono 与 serde |
| E2 | **全 crate 零个自定义 trait** —— 没有任何端口抽象 | `grep -rc "^pub trait\|^trait " src/*.rs` = 0 |
| E3 | 时钟不可注入：`Local::now()` 直调 **9 处**，散在 4 个文件 | `models.rs:576/582/586`、`store.rs:54/1283/1287`、`scheduler.rs:82`、`telemetry.rs:65/238` |
| E4 | 存储目录硬编码，无法注入 | `store.rs:34-38` `data_dir()` 直取 `%APPDATA%` |
| E5 | **调度器持全局 Store 锁跨 OS 通知与磁盘写** | `scheduler.rs:68` 取锁 → `:132/:165/:185` 发通知 → `:227` `save()` → `:74` 才 drop |
| E6 | `Store.data` 是 pub 字段，被三个模块跨层直改 | `scheduler.rs:198-210`、`commands.rs:244-360`、`runtime.rs:250-268` |
| E7 | 命令层内嵌业务规则（非薄适配） | `commands.rs:444-452` enabled/completed 互逆、`501-519` 热键失败整轮回滚、`465-466` snooze 1..720 校验 |
| E8 | 运行态与用户数据混在同一份 `data.json` | `models.rs:353` `fired`、`:356` `migration`、`:292` `capture_pos`；`store.rs:530-537` Store 把 health/corrupt/undo 与 data 并列 |
| E9 | 前端零模块化：单文件 1530 行、约 120 个顶层函数、仅 1 处 import | `index.html:25-1528`，`index.html:26` |
| E10 | 全量 `innerHTML` 重绘，焦点/滚动/动画靠手工找回 | `index.html:337` render → `:399-407` focusRow 找回 |
| E11 | 三窗无共享内存，靠后端广播 + 重拉 + **30s 心跳兜底** | `float.html:169` setInterval；`index.html:1491` store-changed |
| E12 | 快录组参逻辑**三份逐字复制** | `index.html:1315`、`float.html:150`、`capture.html:65` 三行完全相同 |
| E13 | 幽灵状态字段：读在写之前 | `S.dirTouched` 在 `index.html:91/115` 被读、`:1353` 才写，S 字面量里**未声明** |
| E14 | 事件补发队列是隐性契约，丢了会出"热键显示已绑定实则未生效" | `api.js:75-98` earlyQueue；`V2-API.md:190` |
| E15 | 前端回归测试**绑定源码文本**：读 `index.html` 抽函数求值 | `test/rem-editor.test.mjs:17/53-54` |
| E16 | React 侧 M0 零业务代码，且**缺 `--vellum-*` 材质令牌** | `git show feat/react-ui:src-react/src/index.css \| grep -c vellum` = 0 |
| E17 | 命令对账 40↔40↔40；`capture_start_drag` 已注册但前端未接线 | `V2-API.md` §2/§10；`api.js` 无 startDrag 封装 |

### 1.3 已经做对的事（架构设计的起点，不要推翻）

- **领域层零框架依赖**（E1）—— Clean Architecture 的最内环天然存在，缺的只是中间的端口。
- **写盘协议正确**：读 → 内存改 → 序列化 tmp → `fs::rename` 原子替换，覆盖前轮转 5 份备份；corrupt 时拒绝一切写命令。
- **纯函数可测性已被证明可行**：`migrate_v1`（`store.rs:447`）与时间解析全是纯函数，`cargo test` 30 项全绿。
- **正在进行中的写规则收敛**（未提交改动）：fired 键前缀改由生成器唯一拥有（`commands.rs:427` 用 `reminder_key(&id,"")`）、`purge_expired` 修掉了「按第一段取 id」导致误清全部任务 fired 键的缺陷、位置写入去重且走不轮转的 `save_light()`。**这正是 ADR-003 的方向，已经在发生。**

---

## 2 · 目标架构

**结论：单 crate 内做六边形分层，不拆 workspace、不拆多 crate。**

理由见 ADR-002。拆 crate 对单人项目的主要收益（编译隔离、强制边界）可以用模块 + trait + 一条 `cargo test` 门禁拿到，而代价（版本同步、路径噪音、IDE 摩擦）天天付。

```
src-tauri/src/
├── domain/          ← models.rs 演进：实体、值对象、时间语义、校验、纯规则
│   └── (零 tauri / 零 fs / 零 chrono::now —— 时钟经端口)
├── ports/           ← 新增：4 个 trait（唯一允许跨边界的地方）
│   ├── clock.rs         now() -> DateTime<Local>
│   ├── store_backend.rs read / write / rotate / quarantine
│   ├── notifier.rs      notify(title, body) -> bool
│   └── hotkeys.rs       register / unregister -> Result
├── app/             ← 新增：use-case 层（业务规则的家，从 commands 下沉而来）
│   ├── reminders.rs     启停互逆、snooze 边界、fired 键回收
│   ├── tasks.rs         计划/逾期/拖留、软删与撤销
│   ├── capture.rs       快录组参（唯一实现，前端三份复制的内 counterpart 在 TS 侧）
│   └── settings.rs      热键换绑事务与回滚
├── infra/           ← 新增：端口的实现（把今天直调 OS/IO 的代码收进来）
│   ├── fs_store.rs      原 store.rs 的读写/轮转/quarantine
│   ├── tauri_clock.rs / tauri_notifier.rs / tauri_hotkeys.rs
│   └── migration.rs     原 migrate_v1（保持纯函数 + 已有 16 项测试）
├── ui/              ← 原 runtime.rs 拆分：windows.rs / tray.rs / material.rs / notes.rs
├── scheduler.rs     保留，但按 ADR-004 重排锁边界
├── commands.rs      退化为薄适配：解析参数 → 调 app → 映射返回
└── main.rs          组合根（composition root）：注入端口的真实实现
```

**依赖规则（违反即 CI 红）**

1. `domain` 不得 `use` tauri / std::fs / chrono::Local::now。
2. `app` 只依赖 `domain` + `ports`，**永不**依赖 `infra` 或 `ui`。
3. `commands` 只做参数解析与调用，出现业务分支即视为待下沉。
4. `ui`/`scheduler`/`infra` 可实现端口，但不得互相穿透（scheduler 不再直接改 `Store.data`）。

**可测试性验收**：`cargo test` 能在**不创建窗口、不碰 `%APPDATA%`、不真实等待时间**的前提下跑完 use-case 层与调度判定逻辑（用 `FixedClock` + `InMemoryStore`）。这是 ADR-002 的唯一出口判据。

---

## 3 · 限界上下文与聚合

| 上下文 | 聚合根 | 成员 | 一致性边界 | v3 变化 |
|---|---|---|---|---|
| **Task** | `Task` | `Subtask`、`Note` | 子任务/备注只经 Task 根修改 | 不变 |
| **Reminder** | `Reminder` | 循环时刻、snooze | 启停/完成互逆规则收进 `app/reminders.rs` | 规则下沉 |
| **Schedule 簿记** | — | `fired` 去重键、`last_fired` | 与用户数据同生命周期但**不同文件** | 迁到 runtime（ADR-005） |
| **Capture** | — | 语法解析、组参 | 前端唯一实现（ADR-006） | 消除三份复制 |
| **Telemetry** | — | 埋点计数 | 只读用户数据、**绝不回写 data.json** | 迁到 runtime（ADR-005） |
| **StickyNote** | `Note`（新） | 位置、钉状态、淡化策略 | 通过 `noteId ↔ targetId` **单向引用** Task | H0/H1 新增（ADR-007） |
| **KnowledgeItem** | `KbItem`（新） | 笔记/片段、全文索引 | **ACL**：只持有 `KbRef(id)`，不 import Task 实体 | H2 才开工 |

**为什么 KB 用 ACL 而不是共享模型**：roadmap H2 的命门假设是「用户真会把资料钉到桌面常驻吗」。共享实体意味着假设失败时两套模型都脏；`KbRef(id)` 让 KB 可以整体降级为"任务上下文资料"而不伤 Task 聚合。

---

## 4 · 状态所有权矩阵

| 状态 | 唯一拥有者 | 可写方 | 读方 | 跨窗一致机制 |
|---|---|---|---|---|
| 任务/提醒/设置 | 后端 `Store` | `app/` 层（经命令） | 三窗 | `store-changed` 广播 → 重拉 |
| fired / 调度簿记 | 后端 | scheduler（经 `app/`） | 不暴露给 UI | 无需 |
| 视图草稿（`S.drafts` 40 个 `data-in`） | 前端 | 仅当前窗 | 仅当前窗 | 不参与跨窗 |
| 路由 / 焦点行 / 展开态 | 前端 | 仅当前窗 | 仅当前窗 | 主窗 `?open=`/`navigate` 事件 |
| 窗口位置 | 后端 `settings.capture_pos` → 未来 `runtime.json` 的 map | ui 层（`save_light`，值未变不写） | ui 层 | 无需 |
| 引导进度 | 前端 + `localStorage` | 主窗 | 主窗 | 建议改存后端 settings（消除第二状态源，见风险 F3） |
| 待跳转路由 | `localStorage` 兜底 | api.js | 主窗 focus 时消费 | **保留但加注释**：它是"事件补发"的补丁，删了会回归（E14） |

**目标**：三窗共享的**语义**（"今天"、快录组参）必须是同一份代码（ADR-006 共享内核），而不是三份复制（E12）；三窗共享的**数据**必须只有一个真相源（后端 Store），前端不得有第二持久状态源。

---

## 5 · 多窗口通信

**现状**：三个独立文档，无共享内存；后端 `emit_all` 广播 + 前端重拉 + 30s 心跳兜底（E11）+ `earlyQueue` 补发（E14）+ localStorage 路由兜底。

**目标**

1. **事件契约集中定义**：`src-tauri/src/events.rs` 用枚举列出全部事件与负载类型，`V2-API.md` §3 由它生成/对账（命令已有 40↔4040 对账先例，事件也应有）。
2. **心跳降级为可见时轮询**：窗口不可见时不跑 30s 心跳（省电、省锁竞争）。
3. **补发语义必须原样移植到 `api.ts`**（ADR-006 附带要求）—— 它是"启动时主窗隐藏收不到事件"这一真实竞态的唯一补丁。
4. **动态便签窗口**：引入 `WindowRegistry { label → noteId }`，位置记忆从单值 `capture_pos` 改为 map（ADR-005/007）。当前 label 是硬编码常量（`runtime.rs:12-14`、`main.rs:178/183` 按 label 匹配），是新增窗口类别的主要摩擦点。

---

## 6 · 存储与调度

**存储**：写盘协议不动（§1.3）。唯一变化是 **schema 分层**（ADR-005）：

```
%APPDATA%/todo-list/
├── data.json      用户数据（Task/Reminder/Settings）—— 轮转备份、可回滚、可迁移
├── runtime.json   运行态（fired 键、health、窗口位置 map、埋点计数）—— 单文件覆盖写、不轮转
└── data.v1.json   永久留档（现有承诺不变）
```

理由：`fired` 会随提醒数单调增长（上限 400），埋点会随使用增长；把它们留在 `data.json` 里意味着**每次轮转备份都在复制运行垃圾**，且用户"导出我的数据"时会带出一堆无语义键。

**调度**：按 ADR-004 重排为三段式，消除持锁跨 IO：

```
tick:
  ① 短锁读快照（提醒列表 + settings + now）      ← 拿锁 → clone → 放锁
  ② 无锁计算到期集合、去重、DND/上限判定
  ③ 短锁写回（fired 键、completed、last_fired）  ← 拿锁 → 改 → save → 放锁
  ④ 锁外：发系统通知、emit store-changed/fired/missed
```

---

## 7 · 迁移路径与门禁（与 ADR-001 对齐）

ADR-001 的 M0–M5 是**前端**视角。后端解耦不依赖它，但双轨期需要回归网，因此插入一道前置门：

| 阶段 | 内容 | 出口判据 | 依赖 |
|---|---|---|---|
| **H0 收尾** | v2.1 发布：`git push` 建远端、埋点前端尾巴、便签 v2.1、定版删过期包 | v2.1 正式发出，`v2` 有 upstream | — |
| **M0.5（新增门）** | ADR-006 共享内核抽模块 + ADR-004 调度锁边界 | ① 三份快录复制收敛为一份；② `test/rem-editor.test.mjs` 改为 `import` 而非抽源码且仍全绿；③ `cargo test` 覆盖 scheduler 判定（用 `FixedClock`）；④ 真机确认提醒到点仍响、不重复 | 在 M1 之前 |
| **M1** | capture 窗 React 化（Tauri×Vite 多窗接线验证门） | 真机：DWM 圆角干净、无磨砂环、实测对比度 ≥4.5:1、`Alt+Shift+A`/`Esc` 有效 | M0.5 |
| **M2** | `api.ts` + 状态 + Toast + Undo 底座 | 40 命令有类型化封装；事件补发队列语义保真并单测 | M1 |
| **M2.5** | ADR-002 端口注入 + ADR-003 写规则下沉 | `cargo test` 可在不建窗、不碰 `%APPDATA%` 下跑完 use-case 层 | 可与 M1–M2 并行 |
| **M3** | 五视图迁移 | 五份设计规格逐条落地；`qa`/`check` 无新增 Blocker；回归网随迁不丢 | M2 |
| **M4** | 退役旧 `src/` | M3 验收通过 **且** 测试已迁移并通过 | M3 |
| **M5** | 无障碍全量（WCAG 2.1 AA） | `/无障碍检查` 跑过；键盘可达与焦点管理在两窗验证 | M4 |

**顺序已拍板**：内核抽模块（0006）→ 调度锁（0004）→ M1。

---

## 8 · 风险登记（合并两份审计，按对 v3 的阻碍度排序）

| # | 风险 | 证据 | 后果 | 缓解 | 落点 |
|---|---|---|---|---|---|
| R1 | 无端口抽象 + 目录硬编码 → 核心逻辑离线不可测 | E2 E3 E4 | 双轨期没有回归网，迁移全靠手点真机 | 4 个端口 + 组合根注入；`cargo test` 门禁 | ADR-002 |
| R2 | `Store.data` 公开字段被三模块跨层直改 | E6 | 新增实体时写规则散落 4 处，双轨必踩 | 字段私有化 + 领域操作下沉（已部分发生，见 §1.3） | ADR-003 |
| R3 | 调度器持锁跨通知 + 磁盘写 | E5 | 每次到点全应用写命令排队；便签窗口越多越明显 | tick 三段式，通知移出临界区 | ADR-004 |
| R4 | 运行态混入用户数据文件 | E8 | 备份被运行垃圾顶掉、数据导出含噪声、多窗口位置无处安放 | schema 三分 + 懒迁移 + 回滚 | ADR-005 |
| F1 | 前端测试绑定源码文本 | E15 | 迁移即全红，qa-3 的可证伪保证静默消失 | 被测函数先抽成模块，测试改 `import` | ADR-006 |
| F2 | 事件补发队列语义易被丢 | E14 | 设置页显示"已绑定"实则未生效（qa-1 回归） | `api.ts` 原样移植 earlyQueue/pump 并单测 | ADR-006 |
| F3 | 三窗语义靠复制而非共享 | E12 E13 | 双轨期 vanilla 窗与 React 主窗口径漂移 | 共享内核 + 消除幽灵字段与第二状态源 | ADR-006 |
| F4 | React 侧缺材质令牌、多窗 URL 接线未验证 | E16 | capture 窗 404、暗色/降级视觉回归 | M1 同批补 `--vellum-*` 并真机对拍 | ADR-001 §5-R3 |
| R5 | 命令层内嵌业务规则 | E7 | 前端换栈时规则无法复用，双轨要改两遍 | 下沉 `app/`，commands 只解析 | ADR-003 |
| R6 | 动态窗口需改 5 文件 8–10 处 | `runtime.rs:12-14`、`main.rs:178/183`、`capture_pos` 单值 | 便签网开发成本高 | `WindowRegistry` + 位置 map | ADR-007 |

---

## 9 · 明确不做（以及为什么）

| 不做 | 原因 | 重新评估条件 |
|---|---|---|
| 拆 cargo workspace / 多 crate | 单人项目收益（边界强制）可用 trait + 一条测试门禁取得，成本天天付 | 若 `cargo check` 增量编译成为瓶颈 |
| 引入 ORM / SQLite / tantivy 全文索引 | KB 一期只需内存线性扫（`Cargo.toml:14-22` 无相关依赖）；引依赖等于放弃"零构建复杂度"资产 | 笔记条目 > 5k 或搜索 P95 > 200ms |
| 事件溯源 / CQRS | 单用户、无并发写、无审计需求；现有"原子写 + 轮转备份"已覆盖恢复诉求 | 永不做（除非引入多端同步，而那在红线 1 之外） |
| 前后端共享类型生成（schema codegen） | 40 命令契约已有对账脚本兜住漂移；codegen 引入构建期依赖 | 命令数 > 80 或出现双轨手写不一致事故 |
| 把三窗合并为单窗多视图 | 便签网的产品前提就是"多张独立窗口" | H1 验证门失败时 |

---

## 10 · 一句话

**后端缺的不是分层，是端口 —— 领域层今天就已经是干净的；把 4 个端口注入做掉、把调度器的锁边界重排、把三份复制的前端语义收敛成一份共享内核，双轨期才有回归网，M1 之后才是"迁移"而不是"重写"。**

# Kettd 架构设计

- **状态**：当前架构（ADR-0001…0007 决策已全部落地）
- **初版**：2026-09-05 · 最后更新：2026-11-08
- **决策档案**：`adr/`（历史决策记录，含背景与备选方案）
- **命令契约**：本文附录（55 命令，四方对账门禁）
- **需求规格**：`requirements/总体需求.md`

> 本文描述系统当前架构与不变红线。§1 现状审计为初版时的历史快照（vanilla 前端已退役、端口已引入），仅供溯源；§2 起为当前设计。

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

---

# 附录：后端命令契约（原 V2-API.md）


真相源：`spark-output/prd/kettd-v2.md` §6.1 / §6.2 / §6.3 / §6.4 / §6.6 + §6.9 状态矩阵。
本文档是前端对接的唯一契约：命令名、invoke 参数名（camelCase）、返回结构、事件、错误文案。
实现在 `src-tauri/src/{models,store,runtime,scheduler,export,commands,main}.rs`。

## 0. 通用约定

- 所有命令返回 `Result<_, String>`。**Err 字符串是可直接显示给用户的中文短句**，不含路径、堆栈、内部类型名。前端可直接 `toast(err)`。
- 时间全链路 **本地语义字符串**（不做任何 UTC / 时区换算，不用 `toISOString`）：
  - 完整时刻 `YYYY-MM-DDTHH:mm`
  - 仅日期 `YYYY-MM-DD`
  - 提醒循环时刻 `HH:MM`，兼容 v1 多时刻 `HH:MM/HH:MM`
  - 解析失败的旧值：置 `null` 并写入迁移报告（不猜测、不伪造）
- 数据目录沿用 v1：`%APPDATA%/todo-list`（`data.json`）。
- 写盘 = 读 → 内存改 → 序列化 `data.json.tmp` → `fs::rename` 原子替换；**每次覆盖写前**把当时的 `data.json` 轮转为 `data.json.bak-1`，旧档顺移，保留 5 份（`bak-1..bak-5`）。
- 未从 corrupt 状态成功恢复前，**所有写命令一律返回 Err**（`数据文件已损坏，请先从备份恢复后再操作`），杜绝 v1 的「静默空库」。
- `deletedAt` 非空 = 回收站；读取默认过滤，`get_tasks{includeDeleted:true}` 才带出。

### 存储与恢复的状态机（§6.9 的后端侧）

| 状态 | health.health | 触发条件 | 允许的操作 |
| --- | --- | --- | --- |
| ok | `ok` | 正常读写 | 全部命令 |
| corrupt | `corrupt` | data.json 缺失内容不可用（IO 失败 / JSON 截断 / 结构不可还原 / v1 迁移异常），坏件已另存 `data.corrupt.<ISO时间>` | 只读命令 + `get_backups` / `restore_backup` / `get_data_health`；写命令全部拒绝 |
| writeFailed | `writeFailed` | 轮转或写盘失败（磁盘满 / 只读目录 / 无权限） | 读命令可用；错误原文在 `lastError`（中文），下次成功写盘后自动回到 `ok` |

## 1. 领域模型（camelCase）

```ts
type Category = '工作' | '学习' | '生活'
type Priority = 'high' | 'med' | 'low'
type Source = 'capture' | 'manual' | 'seed'
type Repeat = 'none' | 'daily' | 'workdays' | 'weekly'
type FloatForm = 'topmost' | 'desktop' | 'mini'

interface Subtask { id: string; title: string; done: boolean }
interface Note { id: string; author: string; content: string; createdAt: string }

interface Task {
  id: string
  title: string
  note: string                     // 正文备注（v1 description）
  category: Category
  priority: Priority
  dueAt?: string | null            // YYYY-MM-DD 或 YYYY-MM-DDTHH:mm（本地语义）
  remindAt?: string | null         // 单次提醒；HH:MM 视作每日
  plannedDate?: string | null      // 加入今天的日期
  carriedFrom: number              // 逾期粘留天数
  done: boolean
  doneAt?: string | null
  deletedAt?: string | null        // 软删除时刻
  legacy?: boolean                 // v1 完成但无完成时刻（缺省 false 且不写出）
  subtasks: Subtask[]
  notes: Note[]
  source: Source
  createdAt: string
  updatedAt: string
}

interface Reminder {
  id: string
  title: string
  time: string                     // HH:MM（多时刻用 / 分隔）或 YYYY-MM-DDTHH:mm
  category: Category
  enabled: boolean
  completed: boolean               // true = 已完成/停用
  repeat: Repeat
  lastFired?: string | null
  snoozedUntil?: string | null     // 稍后重触发时刻
  legacy?: boolean
}

interface Dnd { enabled: boolean; from: string; to: string }   // 默认 true, 23:00, 07:30

interface Settings {
  theme: string                    // 主题名（float / dark / light，前端解释）
  stickyPaper: string              // 便签纸色：warm | kraft | cyan | ink（§12.1 四选一）
  captureHotkey: string            // 默认 "Alt+Shift+A"
  mainHotkey: string | null        // 打开主界面全局热键；null = 未绑定；默认 "Alt+Shift+O"
  stickyHotkey: string | null      // 新建便签全局热键；null = 未绑定；默认 "Alt+Shift+S"
  todoFloatPinned: boolean         // 待办悬浮窗置顶（todo-float，默认开）
  todoHotkey: string | null        // 待办悬浮窗呼出热键；null = 未绑定；默认 "Alt+Shift+T"
  dnd: Dnd
  remindCapPerHour: number         // 默认 3
  onboarded: boolean
  exportDir?: string | null
}

interface BackupInfo {
  slot: string                     // "1".."5"
  name: string                     // data.json.bak-1
  createdAt: string
  sizeKb: number
  taskCount: number
  readable: boolean
  error?: string                   // readable=false 时的中文原因
}

interface DataHealthV2 {
  health: 'ok' | 'corrupt' | 'writeFailed'
  backups: BackupInfo[]
  corruptFile?: string | null      // data.corrupt.<ISO时间>
  lastError?: string | null
  writable: boolean
}

interface MigrationIssue { collection: string; id: string; field: string; raw: string; action: string }
interface MigrationReport {
  taskCount: number
  reminderCount: number
  invalidDates: number             // 非法时间被置空的条数
  legacyDone: number               // completed 无时刻 → doneAt 置空 + legacy 的条数
  archivedTo: string               // data.v1.json
  issues: MigrationIssue[]
}

interface Bootstrap {
  tasks: Task[]
  reminders: Reminder[]
  settings: Settings
  health: DataHealthV2
  migration: MigrationReport | null
}

interface RestoreResult { restored: number; health: DataHealthV2 }
```

`AppData`（落盘结构，前端不必直接读，但备份文件就是这个形状）：
`{ tasks, reminders, settings, fired, migration? }`，`fired: string[]` 是已处理提醒的稳定键（内部字段，前端不写、不清）。

## 2. 命令清单（invoke 参数名一律 camelCase）

| 命令 | 参数 | 返回 | 说明 |
| --- | --- | --- | --- |
| `get_bootstrap` | — | `Bootstrap` | 一次拿齐 tasks/reminders/settings/health（+ migration） |
| `get_tasks` | `includeDeleted?: bool` | `Task[]` | 默认过滤软删 |
| `get_task` | `id` | `Task \| null` | |
| `add_task` | `args: Partial<Task>`（至少 `title`） | `Task` | id/createdAt/updatedAt 后端生成；`source` 缺省 `manual`，捕获条传 `capture` |
| `update_task` | `id`, `patch: Partial<Task>` | `Task` | **部分字段 MERGE**；未出现的字段保持原值；时间字段自动归一，非法格式直接 Err |
| `toggle_task` | `id` | `Task` | 勾选写 `doneAt`（本地时刻），取消勾选清空 |
| `delete_task` | `id` | `Task`（被删的完整对象） | **软删**：只写 `deletedAt`，子任务与备注原样保留 |
| `undo_delete` | — | `Task \| null` | 恢复最近一次删除（任务 / 子任务 / 备注，后进先出）；重启后仍可恢复最近一条软删任务 |
| `restore_task` | `id` | `Task` | 从回收站恢复指定任务 |
| `purge_task` | `id` | `Task` | 彻底清除（仅允许对回收站中的任务） |
| `add_subtask` | `taskId`, `title` | `Task`（父任务） | |
| `toggle_subtask` | `taskId`, `subtaskId` | `Task` | |
| `delete_subtask` | `taskId`, `subtaskId` | `Task` | 可 `undo_delete` 还原 |
| `add_note` | `taskId`, `note: { content, author?, createdAt? }` | `Task` | |
| `delete_note` | `taskId`, `noteId` | `Task` | 可 `undo_delete` 还原 |
| `get_reminders` | — | `Reminder[]` | |
| `add_reminder` | `reminder: { title, time, category?, repeat?, enabled? }` | `Reminder` | |
| `update_reminder` | `id`, `patch: Partial<Reminder>` | `Reminder` | 部分字段 MERGE |
| `delete_reminder` | `id` | `void` | 硬删（同时清 `fired` 里该提醒的稳定键） |
| `toggle_reminder` | `id` | `Reminder` | completed ⇄ enabled；重新启用会清 `snoozedUntil` |
| `snooze_reminder` | `id`, `minutes` | `Reminder` | 写 `snoozedUntil = now + minutes`（1–720） |
| `get_settings` | — | `Settings` | |
| `set_settings` | `patch: Partial<Settings>` | `Settings` | 部分字段 MERGE；改任一热键（`captureHotkey`/`mainHotkey`/`stickyHotkey`/`todoHotkey`）会真实重绑热键，任一失败整次回滚（四槽位事务）并 Err「快捷键被占用，请用备用入口」；`todoFloatPinned` 即时同步悬浮窗 always_on_top；`stickyPaper` 校验四预设 |
| `open_main_window` | — | `void` | 显示主窗并请求焦点 |
| `hide_todo_float` | — | `void` | 隐藏待办悬浮窗（✕ 同款）：只藏窗，显隐状态落 runtime.json，重启恢复 |
| `open_data_folder` | — | `void` | 在文件管理器里打开 `%APPDATA%/todo-list`；失败 Err「打不开这个位置，请手动前往」 |
| `register_capture_hotkey` | `combo` | `Settings` | 例 `Alt+Shift+A`；注册失败 → Err「快捷键被占用，请用备用入口」（保持旧值） |
| `register_main_hotkey` | `combo` | `Settings` | 打开主界面全局热键；`combo` 空串 = 解绑（`mainHotkey` → `null`）；与其他槽位互斥、失败保持旧值 |
| `register_sticky_hotkey` | `combo` | `Settings` | 新建便签全局热键（sticky-separation，默认 Alt+Shift+S）；`combo` 空串 = 解绑；与其他槽位互斥、失败保持旧值 |
| `register_todo_hotkey` | `combo` | `Settings` | 待办悬浮窗呼出/隐藏热键（todo-float，默认 Alt+Shift+T）；`combo` 空串 = 解绑；与其他槽位互斥、失败保持旧值 |
| `open_capture_overlay` | — | `void` | label `capture`：560×**内容实高**（动态高度，见 `capture_resize`）、无边框、「硫酸纸」材质（OS `apply_blur` 暖纸 tint + DWM 圆角，CSS 层负责反光描边/厚度/可读性，圆角全权归 DWM、CSS `border-radius:0`）、屏幕上部 28% 居中、置顶，显示后 emit `capture-opened` 让前端聚焦输入框；失焦自动收起 |
| `close_capture_overlay` | — | `void` | |
| `capture_start_drag` | — | `void` | 让无边框捕获条可被鼠标拖动（转调 `window.start_dragging()`）。**当前前端未接线** —— 实际拖拽走 Tauri 的 `data-tauri-drag-region` 属性（见 `capture.html` 的 ✎ 把手）；此命令作为备用入口保留，位置记忆由 `settings.capturePos` 承担 |
| `capture_resize` | `height: f64` | `void` | 真·动态高度（方案 B）：前端 ResizeObserver 测 `#frame` 实高（ceil）上报，窗口顶边锚定向下伸缩；宽度恒 560。后端 `sanitize_capture_height` 钳制 `36..200`、NaN 回落 36（`f64::clamp` 遇 NaN 会毒化 set_size）。内容长高先于 set_size 落地的一帧差由 CSS `overflow:hidden` 兜住；高度变化只在内容结构变化时发生（chips 出没/错误行/回执），不在逐键输入时 |
| `export_weekly` | `week?: string`, `format?: 'md' \| 'csv'`, `dir?: string` | `string`（完整路径） | `week` = `current`（默认）/ `last` / `YYYY-Www`；文件名含 ISO 周（如 `周汇总-2026-W36.md`）；二次导出不覆盖（自动 `-2`/`-3`）；目标不可写 → Err「这个位置写不了，换个位置」 |
| `get_backups` | — | `BackupInfo[]` | 含不可读备份（`readable:false` + 中文 error） |
| `restore_backup` | `slot`（`"1".."5"`，也吃 `data.json.bak-3` 这种整名） | `RestoreResult` | 成功即解除 corrupt 封锁；损坏原件已留存为 `data.corrupt.*`，`data.v1.json` 永不删 |
| `get_kb_items` | — | `KbItem[]` | 全量条目（frame H1b） |
| `add_kb_item` | `args: { title, bodyMd?, tags? }` | `KbItem` | 标题非空；tags 去空白去重 |
| `update_kb_item` | `id`, `patch: KbPayload` | `KbItem` | 未给字段不动；`updatedAt` 刷新 |
| `delete_kb_item` | `id` | `void` | 任务侧 kbRefs 会悬空，前端展示「已失效」 |
| `search_kb` | `query?` | `KbItem[]` | 内存扫：title×2/tag×1.5/body×1 相关度排序；空查询=全量按更新时间倒序 |
| `rollback_schema_split` | — | `String` | **调试入口，不进 UI**（ADR-0005）：把 `data.pre-split.json` 复制回 `data.json` 并删 `runtime.json`，重启生效。回滚窗口期内新增提醒的 fired 键会丢 → 后果仅是可能重复响一次 |
| `get_data_health` | — | `DataHealthV2` | 损坏恢复页数据源 |
| `get_hotkey_status` | — | `HotkeyStatus` | 四个全局热键的**实际注册**快照 `{capture, main, sticky, todo}`（未绑上/已解绑为 `null`）；设置页与 `settings.*Hotkey` 比对，不一致即标「未生效」（qa-1）。运行期状态，不落盘 |
| `clear_migration_report` | — | `MigrationReport \| null` | 前端展示完迁移报告后清账，避免每次启动重复提示 |
| `reorder_tasks` | `idsInOrder: string[]` | `usize`（实际移动条数） | 同列表手动排序（便签规格 §12.2）：按传入顺序整表落 `sort_order`（v3/M3）；`sortOrder` 缺省 = 未手动排过，展示按 `createdAt` 兜底 |
| `clear_events` | — | `void` | 清空本地统计（planned-settings B.5★）：重置 `events.jsonl` 为空并记一条 `events_cleared`；写线程在途一批（≤64 条）可随后落盘，属可接受残留 |
| `track_event` | `name: string`, `props?: string`（JSON 文本） | `void` | 通用前端埋点通道（metric 蓝图：weekly_open 等 UI 侧事件）；仅落本机 `events.jsonl` 绝不出网；name ≤48 字符非空，props 需为合法 JSON |
| `create_sticky` | `kbRef?` | `StickyNote` | 新建便签（Phase 2 分池）：`kbRef` 缺省=自由便签（≤500 字，上限 6 张）；`kbRef`=知识便签——KB 条目的桌面视口（正文存 KB 不落便签，上限 4 张），Err「知识便签已达上限（4张）」。窗口按 ADR-0007 动态创建（label `note:<id>`），常驻置顶；位置从最近一张便签窗级联 +32px；记 `sticky_create`。与全局热键「新建便签」共用 `runtime::create_note`（热键恒建自由便签） |
| `list_stickies` | — | `StickyNote[]` | 便签清单；设置页清单数据源。`content` **始终序列化**（含空串）——曾因 skip_serializing_if 吞空串导致前端 `content` undefined、设置页渲染 `.split` 抛 TypeError 整树卸载黑屏（真机复盘 2026-09-09），前端类型契约 `content: string` 必填 |
| `update_sticky` | `id`, `content?`, `mini?` | `StickyNote` | 内容（≤500 字）/ 形态（`mini:true` → 窗口 380×40 置顶悬浮文本条；`mini:false` → 380×456 纸片并聚焦）；形态持久化，重启按原样恢复 |
| `delete_sticky` | `id` | `void` | 关闭即销毁（一次性工具语义）：便签数据、窗口与 `note_pos` 记录一并回收，无确认无撤销，误关靠写前轮转备份兜底；记 `sticky_delete`。便签窗的 Alt+F4 走同一语义（`CloseRequested` 分支） |
| `sticky_self` | — | `{ id, content, mini, kbRef? }` | 便签窗启动自述：label（`note:<id>`）→ 身份与形态；`kbRef` 非空 = 知识便签（视口指向 KB 条目，skip_if_none） |
| `get_form_hints` | — | `{ categories, priorities, sources, repeat, defaultHotkey, defaultCap, retentionDays, today }` | 表单常量，避免前端硬编码 |

参数名映射：Rust 侧 `snake_case` 形参由 Tauri v1 宏自动转成 camelCase（`task_id` → `taskId`，`include_deleted` → `includeDeleted`）。结构体入参（`TaskPayload` 等）本身带 `rename_all = "camelCase"`。

### patch 语义补充

- 时间字段传 `null` 或空串 = 清空；传字符串 = 归一后写入，归一失败 → Err（不会静默丢值）。
- `title` 传空串 → Err「标题不能为空」。
- `category` / `priority` / `stickyPaper` / `dnd.from` / `dnd.to` 非法值 → 中文 Err，不落库。
- `subtasks` / `notes` 传数组 = 整体替换（空标题项会被丢弃）。

## 3. 事件表（后端 → 前端）

| 事件 | 负载 | 发出时机 |
| --- | --- | --- |
| `store-changed` | `{ id: string, kind: 'changed' \| 'fired' \| 'missed' }` | 任何写库成功后（id = 受影响实体 id；`settings` / `data` 表示全局）；提醒触发 `fired`；免打扰静默入账、超 24h 失效、启动补发清账为 `missed` |
| `capture-opened` | `null` | `open_capture_overlay` 显示捕获条后，前端收到即聚焦输入框 |
| `theme-changed` | `string`（主题名） | `set_settings` 改 `theme` 成功后跨窗同步 |
| `hotkey-state` | `HotkeyStatus` `{ capture, main }` | 每次热键注册尝试之后（启动线程 + 换绑/解绑），成败都发。注意：启动时主窗口是隐藏的，这条可能没人接收 —— 前端仍要在启动与进设置页时主动 `get_hotkey_status` 兜一次，广播只负责让已打开的页面即时更新 |
| `navigate` | `string`（前端路由，如 `/planned/reminders`） | 托盘「管理提醒」「本周汇总导出」打开主窗后要求跳转 |
| `open-capture` | `null` | 降级路径：`capture.html` 资源不存在时，`open_capture_overlay`（含全局热键）改为全局广播该事件并唤起主窗，由前端自挂的快捷输入条接管；同时返回 Err「快速记录条没就绪，请用悬浮面板输入框」 |

## 4. 定时器与提醒规则（托盘常驻，A3 批复）

后台 1 个调度线程，1s tick（`chrono::Local`）：

1. 生成候选：`Reminder`（`enabled && !completed`，`HH:MM[/HH:MM]` 按 `repeat` 决定哪些天要响；带日期的 `time` 是单次）与 `Task.remindAt`（`HH:MM` = 每日，完整时刻 = 单次；软删/已完成不参与）。`snoozedUntil` 到期优先于常规排期。
2. 稳定键去重：`r|<id>|<YYYY-MM-DDTHH:mm>` / `t|<id>|<原串>`，记在 `data.fired` → 跨重启不重复响。
3. 到点 → 系统通知 title「待办提醒」body `<标题> · <YYYY-MM-DDTHH:mm>`（snooze 触发时后缀「（稍后提醒）」）；点击深链由前端窗口处理。
4. 每小时上限 `settings.remindCapPerHour`（默认 3）：内存滑窗，超上限的排队，下个 tick 窗口放开再补。
5. 免打扰 `settings.dnd`（默认 23:00–07:30，可关）：时段内静默入账（不清账），结束后合并成一条「免打扰期间有 N 条提醒」，事件仍按 `fired`/`missed` 上报。
6. 错过超过 24h：不再发通知，只记 `missed` 并清账。
7. 应用未运行期间错过的：启动后第一次 tick 合并成一条「错过 N 条提醒」，不逐条轰炸。
8. 每次落库（`lastFired` / 单次提醒的 `completed=true` 或 `remindAt` 清空 / `fired` 键）都走同一套原子写，并 emit `store-changed`。
9. 便签与调度器无耦合（sticky-separation：便签是纯自由文本，不进今天不参与提醒）。
10. corrupt 状态下调度线程不发通知也不写库（等用户恢复）。

## 5. 托盘与窗口（配置侧）

- `tauri.conf.json`：`productName: "待办列表"`；两窗 `main`(1100×720, min 1024×640, visible:false, **decorations:false** —— 自定义标题栏，见下) / `capture`(560×46 首帧值——挂载后由 `capture_resize` 按内容校正, decorations:false, transparent:true, alwaysOnTop:true, skipTaskbar:true, visible:false)；`allowlist` 沿用 v1 的 `api-all`。
  - **float 窗已退役**（sticky-separation，多便签规格 H1b 落地）：所有便签都是动态 `note:<id>` 窗（`runtime::note_builder` 建，380×456 展开 / 380×40 缩小悬浮条，常驻置顶），启动按 store 遍历恢复。
  - main 窗取向为「**标题栏归前端令牌管**」（方案B）：原生栏颜色只跟系统走、喂不进应用暖黑令牌（暗色下「系统黑 ≠ 应用暖黑」色差的根因），故 `decorations:false`，栏由 React `title-bar` 组件绘制（`.titlebar`，拖动/双击最大化走 `data-tauri-drag-region`，─ □ ⧉ × 走 `__TAURI__.window.appWindow`，均经 `api.ts` 惰性取桥）。窗口框架侧由 `runtime::watch_main_window_frame` 接管：还原态 DWM 圆角（`DWMWCP_ROUND`）、最大化态收直角（`DWMWCP_DONOTROUND`，最大化还圆角会在屏幕四角露缺口）。已知取舍：Win11 hover 最大化钮的贴齐布局菜单随原生栏一起消失（tauri v1 无 WCO）；无边框窗边缘缩放由 tao 命中测试承担，手感需真机验证。
  - capture 窗取向为「**窗口即硫酸纸条**」：透明窗 + OS 磨砂材质（`apply_blur` 暖纸 tint）+ DWM 圆角，CSS 只画反光描边与厚度（`--vellum-*` 令牌）。
  - 该窗实际由 `runtime::create_capture_window` 用 `WindowBuilder` 创建，`tauri.conf.json` 的声明必须与 `inner_size` 手工对齐（宽度另有 `runtime::CAPTURE_W` 常量供 `capture_resize` 共用）；`center_capture` 已改为读窗口实际 `outer_size` 居中，不再重复写死尺寸常量。
- 托盘在代码里创建，`id = "main"`（v1 的 `tauri.systemTray` 配置项只吃 `iconPath`，没有 `id` 字段，v1 运行时的托盘菜单必须由代码构建）。
- 托盘菜单（术语表：便签 / 提醒 / 周汇总；sticky-separation 后不再有「悬浮面板」与置顶勾选）：
  `打开主界面` · `快速记录`（等同 `open_capture_overlay`，前端也可监听 `capture-opened`）· 分隔 · `新建便签`（热键同款入口） / `显示全部便签` / `隐藏全部便签` · 分隔 · `管理提醒` / `本周汇总导出` / `打开数据文件夹` · 分隔 · `退出`。

## 6. 错误文案表（前端可直接展示）

| 场景 | 文案 |
| --- | --- |
| 未恢复的损坏库 | 数据文件已损坏，请先从备份恢复后再操作 |
| 写盘失败 | 临时文件写入失败，本次没有保存：磁盘或路径不可用 / 没有写入权限 / 目录是只读的 … |
| 备份不可用 | 这个备份不存在，换一个试试 / 这个备份也读不了，换一个试试 |
| 导出目标不可写 | 这个位置写不了，换个位置 |
| 热键被占 | 快捷键被占用，请用备用入口 |
| 提醒时间（选择器组装，仅兜底触发） | 提醒时间没设好，请重新选一次时间 |
| 任务时间格式（手输/解析兜底） | 截止日期格式不对，请用 YYYY-MM-DD / 提醒时间格式不对，请用 YYYY-MM-DDTHH:mm |
| 找不到对象 | 找不到这条待办，可能已经被删除 / 找不到这条提醒，可能已经被删除 / 找不到这个子任务 / 找不到这条备注 |
| 撤销失效 | 没有可撤销的删除了 |
| 空输入 | 标题不能为空 / 子任务名不能为空 / 备注内容不能为空 |

## 7. v1 → v2 迁移（首次加载懒迁移）

1. `data.json` 有 `settings` 对象 → 按 v2 读；否则判为 v1。
2. 迁移前把原件复制为 `data.v1.json`（**永不删除、已存在不覆盖**）。
3. 字段 snake_case → camelCase（`due_date`→`dueAt`、`created_at`→`createdAt`、`description`→`note`、`subtasks[].completed`→`done`、`notes[].created_at`→`createdAt`）。
4. `"2026-08-29 18:00"` 空格制式 → `2026-08-29T18:00`；带秒/时区后缀只做截断不换算；无法解析 → `null` 并记 `issues[]` + `invalidDates++`。
5. `completed:true` 且无完成时刻 → `doneAt=null` + `legacy=true`，`legacyDone++`。
6. 分类「个人」→「生活」；优先级 `medium`→`med`（都记进 issues）。
7. 逾期未完成 → 一次性补 `plannedDate = 今天`、`carriedFrom = 拖留天数`。
8. v1 `reminders[].time` 的 `09:00` / `10:00/14:00/16:00` 原样保留为循环时刻；`completed` 翻成 `enabled=!completed`。
9. v1 顶层 `theme` 字符串 → v2 `settings.theme = "float"`（主题语义变更，旧值不解读）。
10. 报告存在 `data.migration`，前端展示后调 `clear_migration_report` 清账。迁移异常（tasks 不是列表 / 原件留存失败）→ 一并进 corrupt 通道。

## 8. 前端对接要点

- 启动流程：`get_bootstrap` → 若 `health.health !== 'ok'` 先渲染恢复页（列 `backups` 的 `createdAt`/`taskCount`/`readable`），选 `slot` 调 `restore_backup`，成功后再 `get_bootstrap`。
- 三处「今天」计数用同一 selector：`plannedDate === 今天` ∪ `dueAt` 前缀是今天（后端已保证 `carriedFrom` 与 date-only 语义，不再自算第二套口径）。
- 软删 UI：`delete_task` 返回的对象里有 `deletedAt`，10s 内 `undo_delete`；`purgeExpired` 由后端在启动时跑（30 天）。
- 捕获条落库：`add_task{ args: { title, source: 'capture', dueAt?, remindAt?, category?, priority? } }`，失败提示「没有保存 · 重试」并保留输入内容。
- 迷你条只渲染两行信息（今日剩 N · 下一条 HH:MM），不要再放装饰性计时卡。

---

## 9. 落地差异与工程裁定（相对任务契约原文）

1. `Cargo.toml`：新增 `chrono = { features = ["clock","std"] }`；同时必须给 `tauri` 追加 `system-tray` feature —— `tauri` 1.0/1.8 的 `api-all` **不包含** system-tray（`Builder::system_tray`、`SystemTrayMenu`、`tray_handle_by_id` 都在该 feature 后面），不加则托盘代码无法编译。依赖版本 `tauri = "1.0.0"` 未动。
2. 托盘 id 用代码里的 `SystemTray::new().with_id("main")`，因为 v1.8 的 `tauri-utils` 配置结构 `SystemTrayConfig` 是 `deny_unknown_fields` 且只有 `iconPath`/`iconAsTemplate`/`menuOnLeftClick`/`title`（没有 `id`，也没有 `trayIcon` 键）。故 `tauri.conf.json` 不写托盘配置，写 `trayIcon` 会让构建期配置解析直接失败。
3. 捕获条资源：前端目录只有 `index.html` / `float.html` / `capture.html`（本次不允许改前端），所以 `capture` 窗在 `tauri.conf.json` 里按契约声明（560×112 / 无边框 / **不透明纸片卡**：原透明设计在部分 WebView2 上被合成黑边，v2.0.0 改为整窗 `--card` 底色 / 置顶 / skipTaskbar / visible:false / `capture.html`），但后端在窗口缺失或资源未就绪时不硬创建，走上面的 `open-capture` 降级事件；等前端补上 `capture.html` 后无需改后端即自动生效。
4. ~~提醒落库去重新增一个内部字段 `AppData.fired: string[]`（稳定键，上限 400 条）~~ **ADR-0005 已把它迁出 `data.json`**：运行态（`fired` / `capturePos` / `notePos` / 迁移报告）现在住 `runtime.json` —— 单文件覆盖写、不轮转、损坏静默重建（绝不进 corrupt 通道）。语义不变：稳定键上限 400 条，前端只读不写。

### v2→v2.1 schema 拆分（ADR-0005，2026-09-06）

```
%APPDATA%/todo-list/
├── data.json          用户数据（tasks / reminders / settings，已去掉 capturePos）
│                      原子替换 + 覆盖前轮转 5 份（不变）
├── runtime.json       运行态（fired / capturePos / notePos / migration）
│                      原子替换、不轮转；损坏 = 静默重建为空，不传染 data.json
├── data.pre-split.json 拆分前完整旧档（永不删除、已存在不覆盖）
├── data.v1.json       永久留档（承诺不变）
└── data.corrupt.<ts>  损坏原件留存（承诺不变）
```

- **触发**：启动加载时判定 `data.json` 含 `fired` / `migration` / `settings.capturePos` 任一字段 → 一次性拆分；幂等（runtime 已有 fired 取并集）。
- **顺序**：留档 `data.pre-split.json` → `runtime.json` 先写 → `data.json` 走正常轮转原子写（bak-1 即拆分前完整旧档）。
- **失败**：原件不动、不进 corrupt、启动不阻塞；本会话按拆分结果在内存继续，下次保存自动收敛。
- **前端影响**：无感知。`Settings` 不再返回 `capturePos`（前端从未消费）；`get_bootstrap.migration` 改由运行态供给，契约字段不变。
5. `Reminder.legacy`/`Task.legacy` 落盘为 `true` 时写出、`false` 时省略（`skip_serializing_if`），所以 `types.ts` 里的 `legacy?: boolean` 仍成立。
6. `set_settings` 改 `captureHotkey` 失败时整次回滚（主题/形态一起回），保持「界面显示的热键 = 真实绑定的热键」。
7. 系统通知 title 固定「待办提醒」，body 为 `<标题> · <YYYY-MM-DDTHH:mm>`；免打扰合并为「免打扰期间有 N 条提醒」，启动补发为「错过 N 条提醒」。深链 (`?open=`) 由前端在窗口内处理，通知不带 payload（契约允许省略）。

## 10. 验证状态（如实声明 · v2.1 更新于 2026-09-03 真机）

**工具链**：Windows 22621 · cargo/rustc 1.96.1 · WebView2 Runtime 152.0.4191.53 · node v22.12.0。
（本节原先写的是「本沙箱无 webkit2gtk-4.0，`cargo check` / `build` 全部未做」——该声明已过期并被下面的真机结果取代；保留提醒：**过期的"未验证"声明会把后续会话的推断整体带偏**。）

### 已证

| 项 | 手段 | 结果 |
| --- | --- | --- |
| 类型检查与链接 | `cargo check` / `cargo build`（debug） | **0 error**；7 条 `dead_code` warning（`models.rs:21/187/243/385/573/748`、`store.rs:688`） |
| 命令对账 | 脚本比对 `generate_handler![]` ↔ `#[tauri::command]` ↔ 本文档 §2 表格 | **55 ↔ 55 ↔ 55**（v2.1 40 项 + ADR-0005 调试命令 + frame H1b 知识库 5 项 + v3/M3 `reorder_tasks` / `clear_events` / `track_event` + 方案B `capture_resize` + sticky-separation 4 项 + todo-float `register_todo_hotkey` / `hide_todo_float`；Phase 3 AI 4 命令于 2026-11-07 随混合检索放弃整体摘除） |
| 存储安全自查 | 人工 | 无 `unwrap_or_default()` 式空库回退；无 `toISOString`/`Utc`/`naive_utc` 混入；`tauri.conf.json` JSON 合法 |
| 提醒编辑器逻辑 | `node test/rem-editor.test.mjs`（从 `index.html` 抽真实函数源码断言，17 项） | 全绿：多时刻 round-trip 四形态、模式收敛、渲染契约、aria-label、文案不含手输格式 |
| 时间契约 + v1 迁移十规则 | `cargo test`（**30 项** = `models` 14 时间契约 + `store` 16 迁移规则，纯内存 fixture，不碰数据目录） | 全绿。过程逼出 `parse_clock` 两处加固：带秒输入归零（否则永不命中整分 tick = 到点不响）、接受裸 `HH:MM:SS` 与单位数小时 |
| 打开主界面热键 | 真机注入 `Alt+Shift+O` | 主窗 1116×759 唤起，标题「待办列表」，侧栏 5 项 + 底部一行提示 |
| 快速记录热键 | 真机注入 `Alt+Shift+A` | 记录条 560×80 居中唤起；`Esc` 收起生效 |
| 计划页两种模式 | 真机目视 | 「每天」出 `+ 时刻`、「单次」出日期框；空时刻点「添加」被拦并显示「先选一个提醒时刻」 |
| 到点真的弹系统通知 | 真机（用户截图 `test/f6a7238c-*.png`） | 通知「待办提醒 / 取快递 · 2026-09-04T08:11」，title 与 body 与 §9.7 契约逐字一致 |
| 二进制同代 | mtime + 体积比对 | 曾发现运行实例比源码旧一代（差 43KB），重编后对齐 |

### 未证（不要当成已通过）

1. **真实 v1 历史样本的迁移回归**：§7 十条规则已被 16 个 fixture 单测覆盖（含多时刻原样保留、非法时间置 null、非列表必须走 Err），但真机数据目录里没有 `data.v1.json` —— **真实用户历史数据长什么样仍未验**，fixture 只能证明代码按规则走。
2. **悬浮面板本体**：窗体标题与图标按钮已在同类控件上实证，面板本身需托盘「显示悬浮面板」目视一次。
3. **`cargo clippy`**：未跑。
4. **release 产物**：v2.1 冻结前统一重打（`nsis` 单包 + 版本号 2.1.0），过期 2.0.0 bundle 届时删除。
5. **PRD 5 个成功指标**：目前**全不可测**（无埋点与统计口径），"达标"不可证伪。

### 已知取舍

- **capture 窗不透明**（§5）：需求 5 字面的「四周透出桌面」未达成。透明方案在 Windows 上出黑边（有截图证据），当前取向为「窗口即纸片卡」。翻回 `transparent:true` + 消黑边属独立 spike，未排期完成。
- 历史语法闸：`rustfmt --edition 2021` 对 7 个源文件 stderr 零 error（沙箱期手段，现已被真机编译取代）。

> 工程规格：docs/PRD-v2.md（§6 AC / §6.9 状态矩阵 / §8 三波发布与 dogfood Gate）

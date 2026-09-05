# ADR-0005: 存储 schema 分层（data.json / runtime.json）

**状态**：Proposed —— **已拍板要做，且要求带迁移与回滚方案**（2026-09-05）
**日期**：2026-09-05
**关联**：`ARCHITECTURE-v3.md` §6/§8-R4 · ADR-002（`StoreBackend`）· ADR-007（便签位置 map）
**⚠️ 本 ADR 会动用户数据文件**，因此迁移方案是决策的一部分，不是附录。

## Context

`data.json` 目前同时装着三类东西：

| 类别 | 字段 | 证据 |
|---|---|---|
| **用户数据**（丢了要命） | `tasks`、`reminders`、`settings` | `models.rs` AppData |
| **运行态簿记**（丢了只是体验退化） | `fired: Vec<String>`（去重键，上限 400）、`migration: Option<MigrationReport>` | `models.rs:353`、`:356` |
| **窗口/UI 态** | `settings.capture_pos: Option<[i32;2]>` | `models.rs:292` |

`Store` 结构体又把第四类混进来：`health`、`corrupt_file`、`last_error`、`undo` 栈与 `data` 并列（`store.rs:530-537`）—— 前三个是运行态，第四个纯内存。

四个具体代价：

1. **备份被运行垃圾顶掉**。`save()` 每次覆盖写前轮转 5 份备份（`store.rs:761`）。而 fired 键每次提醒都变 → 触发写 → 轮转 → 真正有价值的旧档被挤出。已经为此加了 `save_light()`（不轮转）来缓解位置写入，但那是**补丁式绕开**，没有解决"fired 混在用户数据里"的根因。
2. **数据导出/迁移带噪声**。用户"把我的数据拿走"时会带出一堆 `t|xxx|2026-09-03T09:00` 无语义键。
3. **埋点会继续膨胀它**。`telemetry.rs` 已有计数逻辑（`Cargo.toml` 无外部依赖，只能落本地），埋点增长与用户数据同文件同备份策略。
4. **多窗口位置无处安放**。ADR-007 要 N 张便签，每张一个位置；`capture_pos` 是单值，加 `note_pos: HashMap` 等于把更多运行态塞进用户数据。

## Decision Drivers

- 必须：用户数据的备份语义纯净（轮转只轮转用户数据）。
- 必须：**零丢失**（brief 硬约束）。迁移失败必须能退回原状，且原件永不删除 —— 沿用 `data.v1.json` 的既有承诺。
- 必须：迁移是**懒式**（启动时一次性判定），不能要求用户操作。
- 应该：corrupt 判定不被新文件复杂化 —— 运行态文件损坏不应让用户数据进 corrupt 通道。

## Considered Options

- **A. 双文件：`data.json`（用户数据）+ `runtime.json`（运行态）**（推荐）。职责清晰，备份策略各自独立，损坏互不传染。
- **B. 三文件：再拆 `telemetry.json`**。多一个文件与一套 IO；埋点量级小，收益不足。留作后续可选项。
- **C. 不拆文件，只在代码层用不同结构体隔离**。零风险，但代价 1（备份被顶掉）与代价 4（位置 map）无法解决 —— 而这两条正是拍板要解决的。
- **D. 换 SQLite**。违反红线（引入依赖、放弃"人可读可手改"的资产），且当前数据量完全不需要。已在 `ARCHITECTURE-v3.md` §9 列为不做。

## Decision

采用 **A**。目标布局：

```
%APPDATA%/todo-list/
├── data.json          tasks / reminders / settings（去掉 capture_pos）
│                      —— 保留：原子替换 + 覆盖前轮转 5 份（bak-1..5）
├── runtime.json       fired / capture_pos / note_pos(map) / telemetry 计数 / health 快照
│                      —— 只原子替换，**不轮转**；损坏可安全重建（见下）
├── data.json.bak-1..5 仅用户数据的历史
├── data.v1.json       永久留档（承诺不变）
└── data.corrupt.<ts>  损坏原件留存（承诺不变）
```

## 迁移方案（本 ADR 的核心，必须照此实现）

**触发**：启动加载时。判定条件：`data.json` 里存在 `fired` 或 `migration` 或 `settings.capture_pos` 任一字段 → 视为旧 schema，执行一次拆分迁移。

**步骤（全程不丢原件）**

1. 读 `data.json`。解析失败 → 走**现有** corrupt 通道，行为与今天完全一致（不新增失败模式）。
2. 若已是新 schema（无上述三字段）→ 什么都不做，直接返回。
3. 迁移前把当前 `data.json` **复制**为 `data.pre-split.json`（新文件名，**永不删除、已存在不覆盖** —— 与 `data.v1.json` 同一承诺）。
4. 在内存里拆出运行态对象，`runtime.json` **先写**（原子 tmp + rename）。
5. `runtime.json` 写成功后，才把去掉三字段的 `data.json` 走一次**正常轮转 + 原子写**（这样 bak-1 就是拆分前的完整旧档 —— 用户手里始终有一份"什么都没丢"的历史）。
6. 任一步失败：`Err("数据升级失败，已保持原样，可重试")`，**不进入 corrupt**（因为原件未动），下次启动重试。

**幂等性**：步骤 4/5 可重入 —— 若 `runtime.json` 已存在且含 `fired`，以 `data.json` 里的为准做一次并集合并（fired 是集合语义，合并无害），然后清空 data 侧字段。

**回滚**：把 `data.pre-split.json` 复制回 `data.json` 并删除 `runtime.json` 即可（启动时会自动重新迁移）。提供一条命令 `rollback_schema_split`（仅从命令行/托盘调试入口调用，不进 UI）。回滚**不丢用户数据**：回滚窗口期内新产生的提醒会丢 `fired` 键 → 后果仅是"可能重复响一次"，明确写进命令说明。

**损坏隔离**：`runtime.json` 读失败或 JSON 截断 → **静默重建为空**（记一条 issue 到 health），**绝不**因此让 `data.json` 进 corrupt 通道。理由：fired 丢了只是重复提醒，位置丢了只是窗口回居中，不值得冻结用户的待办数据。

## Consequences

**正面**
- 轮转备份只覆盖用户数据，真正有价值的历史不再被 fired 键写入挤出。
- 便签位置 map、埋点计数有了自然归属（ADR-007 不再需要往 settings 里塞）。
- 数据导出/迁移只搬 `data.json`，语义干净。
- 运行态损坏的爆炸半径从"整个应用拒绝写"降到"重复响一次"。

**负面**
- 两个文件的写不再是原子的：极端断电下可能出现 `data.json` 已更新而 `runtime.json` 未更新（或反之）。可接受 —— 二者的语义耦合只有"fired 键可能少记一条"，后果是重复提醒一次，不是丢数据。
- 备份目录多一个文件族，用户手动恢复时需要理解哪个是哪个 → 在恢复页 UI 里只列 `data.json.bak-*`，`runtime.json` 不出现在恢复列表（它不该被"恢复"，只该被重建）。
- 迁移期代码要同时兼容两种 schema 一个版本周期。

**风险与缓解**
- 风险：迁移逻辑自身有 bug 导致用户数据被写坏 —— 这是**最严重的可能后果**。缓解三条叠加：① 步骤 3 的 `data.pre-split.json` 永久留档；② 步骤 5 走正常轮转，bak-1 即拆分前完整档；③ 迁移函数必须是**纯函数**（输入旧 JSON 字符串，输出两个新 JSON 字符串），并像 `migrate_v1` 那样用 fixture 单测覆盖（现有 16 项迁移测试是范式）。**先写测试再写迁移。**
- 风险：真机没有历史样本可验（`%APPDATA%` 里已无 `data.v1.json`，现库已是 v2 态）。缓解：沿用 Ask 3 的拍板 —— 手写 fixture 覆盖，不依赖真库。

## Implementation Notes

1. 依赖 ADR-002 的 `StoreBackend`（否则两个文件的 IO 会散落各处）。
2. 落点：M2.5 之后、M4 之前（前端迁移不受它影响，但便签网 ADR-007 需要 `note_pos`）。
3. 出口判据：① 迁移纯函数有 ≥6 项 fixture 测试（含"已是新 schema"、"runtime 已存在需合并"、"写失败保持原样"三种）；② 真机升级一次后 `data.json` 不再含三字段、`runtime.json` 存在、提醒仍不重复响；③ 手动删除 `runtime.json` 后应用仍能启动且只是位置回居中。
4. `V2-API.md` §0 的存储约定与 §7 迁移章节要同步补一节「v2→v2.1 schema 拆分」，并更新命令对账（新增 `rollback_schema_split` 会变 41↔4141）。

# ADR-0003: Store 所有权收敛与写规则下沉

**状态**：Accepted（2026-09-05 拍板；分两期，一期已部分自发发生）
**日期**：2026-09-05
**关联**：`ARCHITECTURE-v3.md` §8-R2/R5 · ADR-002 · ADR-005

## Context

`Store.data` 是 `pub` 字段，被三个模块跨层直接读写：

- `scheduler.rs:198-210`、`:243-287` —— 直接改 `reminder.completed`、`task.remind_at`、`fired` 键；
- `commands.rs:244-360` —— 直接改任务字段；
- `runtime.rs:250-268` —— 直接写 `settings.capture_pos`。

同时命令层内嵌了业务规则，不是薄适配：

| 证据 | 内嵌的规则 |
|---|---|
| `commands.rs:444-452` | `toggle_reminder` 的 enabled/completed 互逆 |
| `commands.rs:501-519` | `set_settings` 热键失败时的**整回滚事务**（主题/形态一起回） |
| `commands.rs:465-466` | `snooze_reminder` 的 1..720 分钟边界 |
| `commands.rs:129-133` | `add_task` 的 `carried_from` 计算 |
| `commands.rs:427` | `delete_reminder` 的 fired 键前缀格式 |

**已经自发开始收敛的部分**（2026-09-05 未提交改动，值得记为方向验证）：

1. `commands.rs:427` 不再手写 `"r|{id}|"` 前缀，改用 `store::reminder_key(&id, "")` —— **键格式由生成器唯一拥有**；
2. `store.rs` `purge_expired` 修掉了「按 `|` 第一段取 id」的缺陷（键格式是 `t|{id}|{stamp}`，id 在第二段），改为 `.nth(1)` 且只回收被清任务自己的键、解析失败保守保留；
3. 新增 `save_light()`（不轮转备份）+ 位置未变不落盘 —— 高频低价值写不再顶掉有价值的历史备份。

这三处恰好证明：规则散落的代价是**同一份键格式在两个地方各写一遍，一处改错就静默丢数据**。

## Decision Drivers

- 新增数据实体（StickyNote、KbItem）时，写规则必须只有一个落点。
- 双轨期（ADR-0001）后端行为要被测试固定，而测试需要单一入口。
- 单人项目：不能承受"大重构周"，必须可分期、可停在中途且仍然自洽。

## Considered Options

- **A. 全量私有化 + 一次性下沉**：`Store.data` 改私有，所有跨层直改改为方法调用。改动面最大（4 文件核心路径），中途不可停。
- **B. 分期：一期只下沉"写规则"，二期再私有化字段**（推荐）。一期把 §Context 表中 5 条规则 + fired 键回收搬进 `app/`，`Store.data` 暂留 pub；二期在 v3 稳定后收口。
- **C. 只做约定不重构**：靠 code review 约束。已失败过一次（`purge_expired` 缺陷就是散落导致的），不采纳。

## Decision

采用 **B（分期）**。

**一期（与 ADR-002 同期，M2.5）**

1. 新建 `app/reminders.rs`：收 enabled/completed 互逆、snooze 边界、fired 键回收三件。
2. 新建 `app/tasks.rs`：收 `carried_from` 计算、软删与撤销语义。
3. 新建 `app/settings.rs`：收热键换绑事务与回滚（这条最危险，单独一个函数、单独一组测试）。
4. `commands.rs` 对应函数退化为「解析 → 调 app → 映射返回」。
5. fired 键格式：`reminder_key`/`task_key` 是**唯一**生产者与解析者，禁止任何地方手写 `"t|"` 字面量。

**二期（v3 稳定后，M4 之后）**

6. `Store.data` 字段改私有或 `pub(crate)`，跨层直改全部改为方法。

## Consequences

**正面**
- 新增实体时写规则有唯一落点；双轨期后端行为可被 `cargo test` 固定。
- 一期不动字段可见性 → 可以停在中途，随时发布，不会卡在半重构状态。

**负面**
- 一期结束时 `Store.data` 仍是 pub，约束靠约定 + 测试而非编译器。这是有意的取舍。
- `app/` 层是新增的一层认知，读代码要多跳一次。

**风险与缓解**
- 风险：`set_settings` 的回滚事务下沉时改错，导致热键换绑把主题/形态一起写坏（这是**用户可感知**的数据问题）。缓解：先为它写 3 条测试（成功/热键冲突回滚/部分字段非法），测试绿了再搬。
- 风险：下沉过程中手改 fired 键格式，触发 `purge_expired` 那类静默丢数据。缓解：一期第一件事就是把 fired 键的生成与解析收进一处并加表驱动测试。

## Implementation Notes

顺序：fired 键收口 → `app/reminders.rs` → `app/tasks.rs` → `app/settings.rs`（回滚事务最后做，先测后搬）。每步出口判据都是「对应测试从 0 变绿，且 `cargo test` 总数只增不减」。

## 不做

不把 `Store` 拆成多个 repository（TaskRepo/ReminderRepo/…）。单文件 JSON 存储下 repository 拆分只会制造样板；聚合边界靠 `app/` 的函数签名表达即可。

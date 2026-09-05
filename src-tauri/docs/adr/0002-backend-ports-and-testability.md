# ADR-0002: 引入四个自有端口（Clock / StoreBackend / Notifier / Hotkeys）

**状态**：Proposed
**日期**：2026-09-05
**决定者**：需求方（单人项目）
**关联**：`ARCHITECTURE-v3.md` §2/§8-R1 · ADR-0001 §5-R2 · ADR-003

## Context

审计复核得到三条硬事实：

- 全 crate **0 个自定义 trait**（`grep "^pub trait\|^trait " src/*.rs` = 0）。唯一的泛型缝是 `bind_global_hotkey<M: GlobalShortcutManager>`（`runtime.rs:403`），那是**框架的 trait**，不是我们的端口。
- 时钟直调 `Local::now()` **9 处**，散在 4 个文件：`models.rs:576/582/586`、`store.rs:54/1283/1287`、`scheduler.rs:82`、`telemetry.rs:65/238`。
- 存储目录硬编码：`store.rs:34-38` 的 `data_dir()` 直接取 `%APPDATA%`，测试注释自己承认无法离线跑。

后果已经显现：`cargo test` 现有 30 项只能覆盖**纯函数**（时间解析、`migrate_v1`）。而真正需要测的东西 —— 提醒到期判定、DND 合并、每小时上限排队、fired 去重、软删 30 天清理 —— 全都要求「有 AppHandle + 真时钟 + 真目录 + 真 OS 通知」才能跑，于是**一行都没测**。v2.1 期间发现的 `purge_expired` 误清 fired 键缺陷（按 `|` 第一段取 id，而键格式是 `t|{id}|{stamp}`），正是这类"测不到的地方"。

## Decision Drivers

- **必须**：use-case 层能在不建窗口、不碰 `%APPDATA%`、不真实等待时间的前提下测试。
- **必须**：不引入新 crate、不引入 async 运行时（项目现状是同步 + 线程）。
- 应该：改动面可控，单人项目付得起。
- 应该：不破坏现有写盘协议（原子替换 + 轮转备份）。

## Considered Options

### 选项 1：四个 trait + 组合根注入（推荐）

`ports/` 放 trait，`infra/` 放真实实现，`main.rs` 作为 composition root 注入。测试用 `FixedClock` + `InMemoryStore`。

- **优**：领域/app 层立刻可测；改动集中在"调用点"而非"逻辑"；不新增依赖；trait 数量只有 4 个，认知负担可控。
- **劣**：`Local::now()` 的 9 个调用点都要改签名或改注入路径，涉及 4 个文件。

### 选项 2：拆 cargo workspace（domain / app / infra 三个 crate）

- **优**：边界由编译器强制，越界 import 直接编译失败。
- **劣**：单人项目里版本同步、路径噪音、IDE 摩擦天天付；而"边界强制"这个收益可以用一条 `cargo test` + 依赖规则检查脚本取得。审计结论 §9 已把"不拆 crate"列为明确不做。

### 选项 3：只加 `Clock`，其余不动

- **优**：最小改动。
- **劣**：`Store` 仍绑死 `%APPDATA%`，scheduler 与 commands 依然测不了 —— 只解决 1/4 问题，却留下"已经端口化了"的错觉。

### 选项 4：用 mock 框架（mockall 等）

- **劣**：引入 proc-macro 依赖与构建期复杂度，且 mock 的前提仍然是先有 trait。顺序反了。

## Decision

采用**选项 1**：新增 `ports/`，定义四个 trait，`main.rs` 注入真实实现，测试注入内存实现。

```rust
// ports/clock.rs
pub trait Clock { fn now(&self) -> DateTime<Local>; }
// ports/store_backend.rs
pub trait StoreBackend {
    fn read_data(&self) -> Result<Option<String>, String>;
    fn write_data(&self, json: &str) -> Result<(), String>;
    fn rotate_backups(&self) -> Result<(), String>;
    fn quarantine(&self, reason: &str) -> Option<String>;
}
// ports/notifier.rs
pub trait Notifier { fn notify(&self, title: &str, body: &str) -> bool; }
// ports/hotkeys.rs
pub trait HotkeyPort { fn register(&mut self, combo: &str) -> Result<(), String>;
                       fn unregister(&mut self, combo: &str) -> Result<(), String>; }
```

`Store` 的目录改为构造注入：`Store::with_backend(Box<dyn StoreBackend>)`，保留 `Store::load()` 作为「用真实 FS backend」的便捷构造，避免动到 `main.rs` 之外的启动路径。

## Consequences

**正面**
- use-case 层与调度判定首次可测；`purge_expired` 那类缺陷以后能在单测里被抓住。
- 迁移期（ADR-0001 M1–M4）后端行为可以用测试固定，前端双轨不再是"唯一验证手段是真机点"。
- 时钟可注入 → DND 跨午夜、每周重复、错过 24h 降级这些**时间边界规则**第一次可测。

**负面**
- 4 个文件的调用点要改，短期 diff 变大（估 150 行新增 + 9 处调用点改写）。
- trait 对象带来 `dyn` + 装箱；对单人桌面应用性能无关紧要，但要接受。

**风险与缓解**
- 风险：端口抽象做了一半就停（只动 clock 不动 store），得到错觉。缓解：出口判据只有一条 —— 「`cargo test` 能在不建窗、不碰 `%APPDATA%` 下跑完 use-case 层」，做不到就是没做完。
- 风险：`Notifier` 注入后，真机通知行为失去覆盖。缓解：保留 `V2-API.md` §10 的「到点真的弹系统通知」真机项，测试不能替代它（已有用户截图证据为基线）。

## Implementation Notes

1. 先加 `Clock`（改动最小、收益立现：可测 DND/周重复/24h 降级）。
2. 再加 `StoreBackend`，`store.rs` 的 `data_dir()` 退化为 FS 实现的构造函数。
3. `Notifier`/`Hotkeys` 最后做 —— 它们主要服务 scheduler 与启动路径。
4. 每步都要补一条对应测试，否则视为未完成。
5. 落点排在 ADR-0001 的 **M2.5**（可与 M1/M2 并行），因为它是双轨期的回归网。

## 相关决策

- ADR-003（Store 所有权）依赖本 ADR 的 `StoreBackend` 就位。
- ADR-004（调度锁边界）需要 `Clock` 才能测到期判定。
- ADR-0001 §5-R2 的令牌漂移风险与本 ADR 无关，但同属"双轨期缺网"这一类。

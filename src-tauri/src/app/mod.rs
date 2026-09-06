//! 用例层（ADR-0003 一期）：业务规则的**唯一落点**。
//!
//! 纪律：
//! - 只依赖 `models` + `ports` + `store`，**永不**依赖 `infra` / `ui` / `commands`。
//! - 不解析前端参数（Payload 解析留在 commands）、不落盘、不广播 ——
//!   「解析 → 调 app → save_and_emit」是 commands 的薄适配职责。
//! - 时间经参数注入（now / today），不得直调 `models::now()/today()`。
//! - 出口判据：每条下沉的规则都有对应用例，`cargo test` 总数只增不减。
pub mod reminders;
pub mod settings;
pub mod tasks;

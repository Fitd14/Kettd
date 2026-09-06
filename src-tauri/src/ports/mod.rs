//! 端口层（ADR-0002）：领域与应用层通往外部世界的**唯一**缝隙。
//!
//! 依赖规则（违反即待修）：
//! - `models`（领域）不 import 本层以外任何模块；本层只依赖 `models` 的纯类型。
//! - `app` 层只依赖 `models` + `ports`，永不依赖 `infra` / `ui`。
//! - 真实实现住在 `infra/`；组合根是 `main.rs`。
//!
//! 出口判据（ADR-0002）：`cargo test` 能在不建窗、不碰 `%APPDATA%`、
//! 不真实等待时间的前提下跑完 use-case 层与调度判定（FixedClock + InMemoryBackend）。

pub mod clock;
pub mod hotkeys;
pub mod notifier;
pub mod store_backend;

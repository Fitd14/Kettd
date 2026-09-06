//! 基础设施层：端口的真实实现（ADR-0002）。
//! 这里是唯一允许直调 OS/IO/Tauri 的地方之一（另一处是 ui/runtime）。

pub mod fs_store;
pub mod tauri_hotkeys;
pub mod tauri_notifier;

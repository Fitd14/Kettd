//! AI 模块：配置管理 + 密钥安全 + embedding + 混合检索。
//!
//! 架构（kb-spec.md §8/§9）：
//! - SecretsKeeper trait 抽象密钥存储（DPAPI Windows / InMemory 测试）
//! - AiConfig 双轨独立配置（chat + embedding）
//! - 命令：set_ai_config / get_ai_status / test_ai_connection / search_kb_hybrid

pub mod secrets;
pub mod config;
pub mod commands;

use std::sync::Arc;
use secrets::SecretsKeeper;

/// Tauri managed state 包装——避免与 Mutex<Store> 类型冲突。
pub struct AiKeeper(pub Arc<dyn SecretsKeeper>);

impl AiKeeper {
    pub fn new(keeper: Arc<dyn SecretsKeeper>) -> Self {
        Self(keeper)
    }
}

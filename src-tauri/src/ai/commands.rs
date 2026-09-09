//! Tauri 命令包装层——桥接 config 模块函数与 Tauri 状态管理。

use tauri::State;
use super::{AiKeeper, config};

/// set_ai_config：保存 AI 配置（patch 语义）。
#[tauri::command]
pub fn set_ai_config(
  keeper: State<'_, AiKeeper>,
  patch: config::AiConfigPatch,
) -> Result<config::AiStatusPayload, String> {
  config::set_ai_config_cmd(keeper.0.clone(), patch)
}

/// get_ai_status：获取 AI 配置状态。
#[tauri::command]
pub fn get_ai_status(
  keeper: State<'_, AiKeeper>,
) -> Result<config::AiStatusPayload, String> {
  Ok(config::get_ai_status_cmd(keeper.0.clone()))
}

/// test_ai_connection：测试连接（async，网络调用）。
#[tauri::command]
pub async fn test_ai_connection(
  keeper: State<'_, AiKeeper>,
  track: String,
) -> Result<config::TestResult, String> {
  Ok(config::test_ai_connection_cmd(keeper.0.clone(), track).await)
}

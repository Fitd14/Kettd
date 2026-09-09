//! Tauri 命令包装层——桥接 config 模块函数与 Tauri 状态管理。

use std::sync::Mutex;
use tauri::State;
use super::{AiKeeper, config, embedding, hybrid};

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

/// search_kb_hybrid：混合检索（本地 + 向量）。
/// 返回带 source 标记的结果列表。
#[tauri::command]
pub fn search_kb_hybrid(
  query: String,
  state: State<'_, std::sync::Mutex<crate::store::Store>>,
  keeper: State<'_, AiKeeper>,
) -> Result<Vec<hybrid::HybridResult>, String> {
  let store = state.lock().map_err(|_| "store lock".to_string())?;
  let items = store.kb.clone();
  let index = store.kb_index.clone();
  let ai_config = config::load_config(keeper.0.as_ref());
  let embedding_configured = ai_config.enabled
    && !ai_config.embedding_base_url.is_empty()
    && !ai_config.embedding_model.is_empty()
    && !ai_config.embedding_api_key.is_empty();
  Ok(hybrid::hybrid_search(&query, &items, &index, embedding_configured))
}

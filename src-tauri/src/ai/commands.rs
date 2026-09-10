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

/// search_kb_hybrid：混合检索（本地 + 向量，渐进增强）。
/// 锁纪律：先短锁 clone 数据，drop 后再 await 网络（绝不持锁跨 await，假死教训同源）。
#[tauri::command]
pub async fn search_kb_hybrid(
  app: tauri::AppHandle,
  query: String,
  state: tauri::State<'_, Mutex<crate::store::Store>>,
  keeper: State<'_, AiKeeper>,
) -> Result<Vec<hybrid::HybridResult>, String> {
  // 短锁取数
  let (items, index, index_empty) = {
    let store = state.lock().map_err(|_| "store lock".to_string())?;
    (store.kb.clone(), store.kb_index.clone(), store.kb_index.is_empty())
  };
  let ai_config = config::load_config(keeper.0.as_ref());
  let embedding_ready = ai_config.enabled
    && !ai_config.embedding_base_url.is_empty()
    && !ai_config.embedding_model.is_empty()
    && !ai_config.embedding_api_key.is_empty();

  // 索引空且已配 → 后台全量重建；本次查询照常先回本地
  if embedding_ready && index_empty && !items.is_empty() && !query.trim().is_empty() {
    crate::commands::spawn_rebuild_full_index(&app, &keeper);
  }

  let (results, used_ai) = hybrid::hybrid_search_async(
    &query,
    &items,
    &index,
    if embedding_ready { Some(&ai_config) } else { None },
  )
  .await;

  // 埋点（无内容：只记命中数与成败）
  if embedding_ready && used_ai {
    let ai_hits = results.iter().filter(|r| r.source == hybrid::SearchSource::Ai).count();
    crate::telemetry::record_str("ai_call_ok", &[("kind", "query_embed"), ("ai_hits", &ai_hits.to_string())]);
  } else if embedding_ready {
    crate::telemetry::record_str("ai_call_fail", &[("kind", "query_embed")]);
  }
  Ok(results)
}

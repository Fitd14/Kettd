//! 混合检索：本地全文扫 ⊕ 向量召回 → 合并重排。
//!
//! 搜索管线（kb-spec.md §9 + kb-qa-fixes qa-1 渐进增强）：
//! 1. 本地全文扫（kb.rs::search，带字面高亮位）——同步、零成本
//! 2. 向量召回（query embedding × kb_index.json chunk 余弦 top-k）
//! 3. 合并重排：local_norm + 0.4 × ai_norm
//! 4. 单列表输出：纯 AI 命中标「AI」徽标

use crate::kb;
use crate::models::KbItem;
use super::config::AiConfig;
use super::embedding::{self, ChunkIndex};

/// AI 命中阈值：向量分低于此视为噪声
const AI_THRESHOLD: f64 = 0.25;
/// 向量召回权重
const AI_WEIGHT: f64 = 0.4;

/// 搜索结果来源。
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum SearchSource {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "ai")]
    Ai,
}

/// 单条搜索结果。
#[derive(Debug, Clone, serde::Serialize)]
pub struct HybridResult {
    pub id: String,
    pub source: SearchSource,
}

/// 混合检索（async）：本地扫 + 真实 query embedding 向量召回。
///
/// - ai_config None / index 空 / embed 失败 → 纯本地回落（调用方记 ai_call_fail）
/// - 网络调用只针对 query 一个文本；chunk 向量已在索引中（保存时预计算）
pub async fn hybrid_search_async(
    query: &str,
    items: &[KbItem],
    index: &[ChunkIndex],
    ai_config: Option<&AiConfig>,
) -> (Vec<HybridResult>, bool) {
    // 1. 本地全文扫（同步）
    let local_ids = kb::search(items, query);
    let local_map: std::collections::HashMap<String, f64> = local_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), 1.0 - (i as f64 / local_ids.len().max(1) as f64)))
        .collect();

    // 2. 向量召回
    let mut ai_map: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    let mut used_ai = false;
    if let Some(config) = ai_config {
        if !query.trim().is_empty() && !index.is_empty() {
            match embedding::embed_texts(&[query.trim().to_string()], config).await {
                Ok(vectors) => {
                    if let Some(q_vec) = vectors.first() {
                        for chunk in index {
                            let score = embedding::cosine(q_vec, &chunk.vector);
                            let entry = ai_map.entry(chunk.item_id.clone()).or_insert(0.0);
                            if score > *entry {
                                *entry = score;
                            }
                        }
                        used_ai = true;
                    }
                }
                Err(_) => { /* 调用方记 ai_call_fail；ai_map 保持空 → 纯本地 */ }
            }
        }
    }

    let results = merge(local_map, ai_map);
    (results, used_ai)
}

/// 本地/AI 评分合并重排（纯函数，可测）
fn merge(
    local_map: std::collections::HashMap<String, f64>,
    ai_map: std::collections::HashMap<String, f64>,
) -> Vec<HybridResult> {
    let mut all_ids: std::collections::HashSet<String> = local_map.keys().cloned().collect();
    all_ids.extend(ai_map.keys().cloned());

    let max_local = local_map.values().copied().fold(0.0f64, f64::max).max(1.0);
    let max_ai = ai_map.values().copied().fold(0.0f64, f64::max).max(1.0);

    let mut scored: Vec<(f64, String, SearchSource)> = all_ids
        .iter()
        .map(|id| {
            let local_norm = local_map.get(id).unwrap_or(&0.0) / max_local;
            let ai_norm = ai_map.get(id).unwrap_or(&0.0) / max_ai;
            let combined = local_norm + AI_WEIGHT * ai_norm;
            let source = if ai_map.get(id).copied().unwrap_or(0.0) > AI_THRESHOLD {
                SearchSource::Ai
            } else {
                SearchSource::Local
            };
            (combined, id.clone(), source)
        })
        .collect();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    scored
        .into_iter()
        .map(|(_, id, source)| HybridResult { id, source })
        .collect()
}

// ─── 测试 ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_local_only_when_ai_empty() {
        let mut local = std::collections::HashMap::new();
        local.insert("k1".to_string(), 1.0);
        local.insert("k2".to_string(), 0.5);
        let results = merge(local, std::collections::HashMap::new());
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.source == SearchSource::Local));
        assert_eq!(results[0].id, "k1", "高分在前");
    }

    #[test]
    fn merge_marks_ai_source_above_threshold() {
        let mut local = std::collections::HashMap::new();
        local.insert("k1".to_string(), 1.0);
        let mut ai = std::collections::HashMap::new();
        ai.insert("k2".to_string(), 0.8); // 纯语义命中（本地无）
        let results = merge(local, ai);
        let k2 = results.iter().find(|r| r.id == "k2").unwrap();
        assert_eq!(k2.source, SearchSource::Ai);
    }

    #[test]
    fn merge_ai_below_threshold_stays_local() {
        let mut local = std::collections::HashMap::new();
        local.insert("k1".to_string(), 1.0);
        let mut ai = std::collections::HashMap::new();
        ai.insert("k1".to_string(), 0.1); // 弱向量分
        let results = merge(local, ai);
        assert!(results.iter().all(|r| r.source == SearchSource::Local));
    }

    #[test]
    fn merge_boosts_dual_hits_ranking() {
        // 本地+AI 双命中的排位应高于纯本地高分（0.8+0.4×0.9=1.16 > 1.0）
        let mut local = std::collections::HashMap::new();
        local.insert("k1".to_string(), 1.0);
        local.insert("k2".to_string(), 0.8);
        let mut ai = std::collections::HashMap::new();
        ai.insert("k2".to_string(), 0.9);
        let results = merge(local, ai);
        assert_eq!(results[0].id, "k2", "双命中加权后应反超");
    }

    #[tokio::test]
    async fn async_search_falls_back_to_local_without_config() {
        let items = vec![KbItem {
            id: "k1".into(), title: "Rust".into(), body_md: "lang".into(),
            tags: vec![], created_at: "t".into(), updated_at: "t".into(),
        }];
        let (results, used_ai) = hybrid_search_async("rust", &items, &[], None).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, SearchSource::Local);
        assert!(!used_ai);
    }
}

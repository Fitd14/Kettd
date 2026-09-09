//! 混合检索：本地全文扫 ⊕ 向量召回 → 合并重排。
//!
//! 搜索管线（kb-spec.md §9）：
//! 1. 本地全文扫（kb.rs::search，带字面高亮位）
//! 2. 向量召回（kb_index.json chunk 余弦 top-k）
//! 3. 合并重排：local_norm ⊕ 0.4 × ai_norm
//! 4. 单列表输出：纯 AI 命中标「AI」徽标

use crate::kb;
use crate::models::KbItem;
use super::embedding::{ChunkIndex, cosine};

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

/// 混合搜索：本地 + 向量 → 合并排序。
///
/// - embedding 未配置 / index 为空 / 调用失败 → 纯本地回落
/// - 向量命中 score < 0.25 → 视为噪声，不标记为 AI
pub fn hybrid_search(
    query: &str,
    items: &[KbItem],
    index: &[ChunkIndex],
    embedding_configured: bool,
) -> Vec<HybridResult> {
    // 1. 本地全文扫
    let local_ids = kb::search(items, query);
    let local_map: std::collections::HashMap<String, f64> = local_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), 1.0 - (i as f64 / local_ids.len().max(1) as f64)))
        .collect();

    // 2. 向量召回（仅 embedding 已配置且 index 非空时）
    let ai_map: std::collections::HashMap<String, f64> = if embedding_configured && !index.is_empty() && !query.trim().is_empty() {
        // 简单策略：用 query 的字符 n-gram 作为伪向量（真实 embedding 需要异步 API 调用，
        // 这里提供纯本地的余弦近似；完整实现在 Phase 3 的 async 搜索命令中）
        // 暂时返回空，让纯本地搜索工作
        std::collections::HashMap::new()
    } else {
        std::collections::HashMap::new()
    };

    // 3. 合并重排
    let mut all_ids: std::collections::HashSet<String> = local_map.keys().cloned().collect();
    all_ids.extend(ai_map.keys().cloned());

    let max_local = local_map.values().copied().fold(0.0f64, f64::max).max(1.0);
    let max_ai = ai_map.values().copied().fold(0.0f64, f64::max).max(1.0);

    let mut scored: Vec<(f64, String, SearchSource)> = all_ids
        .iter()
        .map(|id| {
            let local_norm = local_map.get(id).unwrap_or(&0.0) / max_local;
            let ai_norm = ai_map.get(id).unwrap_or(&0.0) / max_ai;
            let combined = local_norm + 0.4 * ai_norm;
            let source = if ai_map.contains_key(id) && ai_norm > 0.25 {
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

    fn make_item(id: &str, title: &str, body: &str) -> KbItem {
        KbItem {
            id: id.to_string(),
            title: title.to_string(),
            body_md: body.to_string(),
            tags: vec![],
            created_at: "2026-01-01T00:00".to_string(),
            updated_at: "2026-01-01T00:00".to_string(),
        }
    }

    #[test]
    fn hybrid_search_returns_local_when_no_embedding() {
        let items = vec![make_item("k1", "Rust", "lang")];
        let results = hybrid_search("rust", &items, &[], false);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, SearchSource::Local);
    }

    #[test]
    fn hybrid_search_empty_query_returns_all() {
        let items = vec![
            make_item("k1", "A", ""),
            make_item("k2", "B", ""),
        ];
        let results = hybrid_search("", &items, &[], false);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn hybrid_search_no_match_returns_empty() {
        let items = vec![make_item("k1", "Rust", "lang")];
        let results = hybrid_search("python", &items, &[], false);
        assert!(results.is_empty());
    }
}

//! Embedding 分块与向量索引管理。
//!
//! - `ChunkIndex`：条目分块（itemId + chunkId + text + vector）
//! - `chunk_item`：按标题/段落切块（~500-800 字符）
//! - `embed_texts`：调用厂商 embedding API（reqwest + OpenAI 兼容格式）
//! - `kb_index.json`：向量索引持久化（衍生数据，可全量重建）

use crate::models::KbItem;
use super::config::AiConfig;

/// 向量索引条目。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChunkIndex {
    pub item_id: String,
    pub chunk_id: u32,
    pub text: String,
    pub vector: Vec<f32>,
}

/// 条目分块：标题独立成块 + 段落累积 ~600 字符。
pub fn chunk_item(item: &KbItem) -> Vec<(u32, String)> {
    let mut chunks: Vec<(u32, String)> = Vec::new();

    // 块 0：标题（高信号，单独成块）
    if !item.title.trim().is_empty() {
        chunks.push((0, item.title.clone()));
    }

    // 块 1..n：按段落（\n\n）累积，达到 600 字符切一块
    let mut buffer = String::new();
    let mut chunk_id = chunks.len() as u32;
    for para in item.body_md.split("\n\n") {
        if buffer.len() + para.len() > 600 && !buffer.is_empty() {
            chunks.push((chunk_id, std::mem::take(&mut buffer)));
            chunk_id += 1;
        }
        if !buffer.is_empty() {
            buffer.push_str("\n\n");
        }
        buffer.push_str(para);
    }
    if !buffer.trim().is_empty() {
        chunks.push((chunk_id, buffer));
    }

    chunks
}

/// 调用厂商 embedding API（OpenAI 兼容格式）。
/// 返回 Vec<Vec<f32>>，与输入 texts 一一对应。
pub async fn embed_texts(texts: &[String], config: &AiConfig) -> Result<Vec<Vec<f32>>, String> {
    if texts.is_empty() {
        return Ok(vec![]);
    }

    let url = format!(
        "{}/embeddings",
        config.embedding_base_url.trim_end_matches('/')
    );

    let body = serde_json::json!({
        "model": config.embedding_model,
        "input": texts,
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("reqwest build failed: {e}"))?;

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.embedding_api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("embedding request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("embedding API error HTTP {status}: {text}"));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse embedding response: {e}"))?;

    let data = json["data"]
        .as_array()
        .ok_or("missing 'data' in embedding response")?;

    let mut vectors: Vec<Vec<f32>> = Vec::with_capacity(data.len());
    for item in data {
        let embedding = item["embedding"]
            .as_array()
            .ok_or("missing 'embedding' in data item")?;
        let vec: Vec<f32> = embedding
            .iter()
            .filter_map(|v| v.as_f64().map(|f| f as f32))
            .collect();
        vectors.push(vec);
    }

    Ok(vectors)
}

/// 余弦相似度。
pub fn cosine(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| (*x as f64) * (*y as f64)).sum();
    let mag_a: f64 = a.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    let mag_b: f64 = b.iter().map(|x| (*x as f64).powi(2)).sum::<f64>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 {
        0.0
    } else {
        dot / (mag_a * mag_b)
    }
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
    fn chunk_item_splits_paragraphs() {
        let item = make_item("k1", "Title", "para1\n\npara2\n\npara3");
        let chunks = chunk_item(&item);
        assert!(chunks.len() >= 2, "should have title + at least one body chunk");
        assert_eq!(chunks[0].0, 0, "first chunk is title");
        assert_eq!(chunks[0].1, "Title");
    }

    #[test]
    fn chunk_item_empty_body() {
        let item = make_item("k1", "Title", "");
        let chunks = chunk_item(&item);
        assert_eq!(chunks.len(), 1, "only title chunk");
    }

    #[test]
    fn chunk_item_long_paragraph_splits() {
        let long_body = "word ".repeat(200); // ~1000 chars
        let item = make_item("k1", "T", &long_body);
        let chunks = chunk_item(&item);
        assert!(chunks.len() >= 2, "long paragraph should split");
    }

    #[test]
    fn cosine_similar_vectors_high() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.1, 0.0];
        assert!(cosine(&a, &b) > 0.9);
    }

    #[test]
    fn cosine_orthogonal_vectors_zero() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!((cosine(&a, &b)).abs() < 0.001);
    }

    #[test]
    fn cosine_empty_vectors() {
        assert_eq!(cosine(&[], &[]), 0.0);
    }
}

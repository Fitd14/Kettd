//! AI 配置管理与命令。
//!
//! - `AiConfig`：chat + embedding 双轨独立配置
//! - `set_ai_config`：保存加密配置
//! - `get_ai_status`：返回配置状态 + 掩码
//! - `test_ai_connection`：测试连接（chat / embedding）
//!
//! 密钥安全（kb-spec.md §8）：
//! - KEY 永不进 WebView（前端只拿 configured + keyMask）
//! - DPAPI per-user 加密落 secrets.json
//! - 传输仅 HTTPS（豁免 127.0.0.1 Ollama）

use std::sync::Arc;
use super::secrets::SecretsKeeper;

const AI_CONFIG_KEY: &str = "ai_config";

/// AI 双轨配置（chat + embedding 独立）。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct AiConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    pub embedding_base_url: String,
    pub embedding_model: String,
    pub embedding_api_key: String,
    pub enabled: bool,
}

/// 前端 patch 载荷（只传需要更新的字段）。
#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct AiConfigPatch {
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub embedding_base_url: Option<String>,
    pub embedding_model: Option<String>,
    pub embedding_api_key: Option<String>,
    pub enabled: Option<bool>,
}

/// 前端读取的状态。
#[derive(Debug, Clone, serde::Serialize)]
pub struct AiStatusPayload {
    pub chat: AiTrackStatus,
    pub embedding: AiTrackStatus,
    pub enabled: bool,
    pub key_mask: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AiTrackStatus {
    pub configured: bool,
    pub model: Option<String>,
    pub base_url: Option<String>,
}

/// 测试连接结果。
#[derive(Debug, Clone, serde::Serialize)]
pub struct TestResult {
    pub ok: bool,
    pub message: String,
}

// ─── 内部工具 ─────────────────────────────────────────────────────────────

fn mask_key(key: &str) -> Option<String> {
    if key.is_empty() {
        return None;
    }
    if key.len() <= 4 {
        return Some("*".repeat(key.len()));
    }
    let last4 = &key[key.len() - 4..];
    Some(format!("sk-***{last4}"))
}

pub fn load_config(keeper: &dyn SecretsKeeper) -> AiConfig {
    keeper
        .load(AI_CONFIG_KEY)
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn save_config(keeper: &dyn SecretsKeeper, config: &AiConfig) -> Result<(), String> {
    let json = serde_json::to_string(config)
        .map_err(|e| format!("序列化 AI 配置失败: {e}"))?;
    keeper.save(AI_CONFIG_KEY, &json)
}

fn to_status(config: &AiConfig) -> AiStatusPayload {
    AiStatusPayload {
        chat: AiTrackStatus {
            configured: !config.base_url.is_empty() && !config.model.is_empty() && !config.api_key.is_empty(),
            model: if config.model.is_empty() { None } else { Some(config.model.clone()) },
            base_url: if config.base_url.is_empty() { None } else { Some(config.base_url.clone()) },
        },
        embedding: AiTrackStatus {
            configured: !config.embedding_base_url.is_empty() && !config.embedding_model.is_empty() && !config.embedding_api_key.is_empty(),
            model: if config.embedding_model.is_empty() { None } else { Some(config.embedding_model.clone()) },
            base_url: if config.embedding_base_url.is_empty() { None } else { Some(config.embedding_base_url.clone()) },
        },
        enabled: config.enabled,
        key_mask: mask_key(&config.api_key),
    }
}

// ─── Tauri 命令 ───────────────────────────────────────────────────────────

/// 保存 AI 配置（patch 语义：只更新非 None 字段）。
pub fn set_ai_config_cmd(
    keeper: Arc<dyn SecretsKeeper>,
    patch: AiConfigPatch,
) -> Result<AiStatusPayload, String> {
    let mut config = load_config(keeper.as_ref());
    if let Some(v) = patch.base_url { config.base_url = v; }
    if let Some(v) = patch.model { config.model = v; }
    if let Some(v) = patch.api_key { config.api_key = v; }
    if let Some(v) = patch.embedding_base_url { config.embedding_base_url = v; }
    if let Some(v) = patch.embedding_model { config.embedding_model = v; }
    if let Some(v) = patch.embedding_api_key { config.embedding_api_key = v; }
    if let Some(v) = patch.enabled { config.enabled = v; }
    save_config(keeper.as_ref(), &config)?;
    Ok(to_status(&config))
}

/// 获取 AI 配置状态。
pub fn get_ai_status_cmd(keeper: Arc<dyn SecretsKeeper>) -> AiStatusPayload {
    let config = load_config(keeper.as_ref());
    to_status(&config)
}

/// 测试连接（track: "chat" | "embedding"）。
pub async fn test_ai_connection_cmd(
    keeper: Arc<dyn SecretsKeeper>,
    track: String,
) -> TestResult {
    let config = load_config(keeper.as_ref());
    let (base_url, model, api_key) = match track.as_str() {
        "chat" => (config.base_url, config.model, config.api_key),
        "embedding" => (config.embedding_base_url, config.embedding_model, config.embedding_api_key),
        _ => return TestResult { ok: false, message: format!("未知 track: {track}") },
    };

    if base_url.is_empty() || model.is_empty() || api_key.is_empty() {
        return TestResult { ok: false, message: format!("{} 轨未配置完整", track) };
    }

    // 网络请求：最小调用验证连通性
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 1,
    });

    match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .danger_accept_invalid_certs(false)
        .build()
    {
        Ok(client) => {
            let start = std::time::Instant::now();
            match client.post(&url)
                .header("Authorization", format!("Bearer {api_key}"))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => {
                    let ms = start.elapsed().as_millis() as u64;
                    let status = resp.status();
                    if status.is_success() {
                        TestResult { ok: true, message: format!("连接成功（{ms}ms）") }
                    } else {
                        let text = resp.text().await.unwrap_or_default();
                        TestResult { ok: false, message: format!("HTTP {status}: {text}") }
                    }
                }
                Err(e) => TestResult { ok: false, message: format!("请求失败: {e}") },
            }
        }
        Err(e) => TestResult { ok: false, message: format!("构建客户端失败: {e}") },
    }
}

// ─── 测试 ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::secrets::InMemoryKeeper;

    #[test]
    fn set_then_get_round_trip() {
        let keeper: Arc<dyn SecretsKeeper> = Arc::new(InMemoryKeeper::new());
        let patch = AiConfigPatch {
            base_url: Some("https://api.test.com".into()),
            model: Some("m1".into()),
            api_key: Some("sk-test1234".into()),
            enabled: Some(true),
            ..Default::default()
        };
        let status = set_ai_config_cmd(keeper.clone(), patch).unwrap();
        assert!(status.enabled);
        assert!(status.chat.configured);
        assert_eq!(status.key_mask, Some("sk-***1234".into()));

        let status2 = get_ai_status_cmd(keeper.clone());
        assert!(status2.chat.configured);
        assert_eq!(status2.chat.model, Some("m1".into()));
    }

    #[test]
    fn mask_key_empty_returns_none() {
        assert_eq!(mask_key(""), None);
    }

    #[test]
    fn mask_key_short() {
        assert_eq!(mask_key("ab"), Some("**".into()));
    }

    #[test]
    fn mask_key_normal() {
        assert_eq!(mask_key("sk-1234567890"), Some("sk-***7890".into()));
    }

    #[test]
    fn patch_preserves_unset_fields() {
        let keeper: Arc<dyn SecretsKeeper> = Arc::new(InMemoryKeeper::new());
        // First save
        set_ai_config_cmd(keeper.clone(), AiConfigPatch {
            base_url: Some("https://a.com".into()),
            model: Some("m1".into()),
            api_key: Some("k1".into()),
            enabled: Some(true),
            ..Default::default()
        }).unwrap();
        // Partial patch: only change model
        set_ai_config_cmd(keeper.clone(), AiConfigPatch {
            model: Some("m2".into()),
            ..Default::default()
        }).unwrap();
        let status = get_ai_status_cmd(keeper);
        assert_eq!(status.chat.model, Some("m2".into()));
        assert_eq!(status.chat.base_url, Some("https://a.com".into()));
        assert!(status.enabled);
    }
}

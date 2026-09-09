//! 密钥安全存储抽象。
//!
//! - `SecretsKeeper` trait：平台无关接口（save/load/delete）
//! - `InMemoryKeeper`：测试替身（无加密，内存 HashMap）
//! - `DpapiKeeper`：Windows DPAPI 实现（CryptProtectData，feature-gated）
//!
//! 设计约束（kb-spec.md §8）：
//! - KEY 永不进 WebView / 日志 / store.json
//! - 独立 secrets.json，与 data.json / notes.json 物理隔离
//! - 损坏/换用户 → 静默视为未配置

use std::collections::HashMap;
use std::sync::Mutex;

/// 加密键值存储抽象——平台特定实现在此 trait 背后。
pub trait SecretsKeeper: Send + Sync {
    fn save(&self, key: &str, value: &str) -> Result<(), String>;
    fn load(&self, key: &str) -> Option<String>;
    fn delete(&self, key: &str) -> Result<(), String>;
}

/// 内存测试替身——无加密，用于单元测试。
pub struct InMemoryKeeper {
    store: Mutex<HashMap<String, String>>,
}

impl InMemoryKeeper {
    pub fn new() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryKeeper {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretsKeeper for InMemoryKeeper {
    fn save(&self, key: &str, value: &str) -> Result<(), String> {
        self.store
            .lock()
            .map_err(|_| "lock poisoned".to_string())?
            .insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn load(&self, key: &str) -> Option<String> {
        self.store
            .lock()
            .ok()?
            .get(key)
            .cloned()
    }

    fn delete(&self, key: &str) -> Result<(), String> {
        self.store
            .lock()
            .map_err(|_| "lock poisoned".to_string())?
            .remove(key);
        Ok(())
    }
}

// ─── Windows DPAPI 实现（feature-gated）────────────────────────────────────

#[cfg(target_os = "windows")]
pub struct DpapiKeeper {
    secrets_path: std::path::PathBuf,
}

#[cfg(target_os = "windows")]
impl DpapiKeeper {
    pub fn new(app_data_dir: &std::path::Path) -> Self {
        Self {
            secrets_path: app_data_dir.join("secrets.json"),
        }
    }
}

#[cfg(target_os = "windows")]
impl SecretsKeeper for DpapiKeeper {
    fn save(&self, key: &str, value: &str) -> Result<(), String> {
        // 1. 读取现有 JSON（或创建新 Map）
        let mut map: HashMap<String, String> = if self.secrets_path.exists() {
            let data = std::fs::read_to_string(&self.secrets_path)
                .map_err(|e| format!("读取 secrets.json 失败: {e}"))?;
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            HashMap::new()
        };

        // 2. DPAPI 加密（TODO: 接入 winapi CryptProtectData）
        // 暂时用 base64 编码作为占位（真机复验时替换为真实 DPAPI）
        let encoded = base64_encode(value.as_bytes());
        map.insert(key.to_string(), encoded);

        // 3. 原子写入
        let json = serde_json::to_string_pretty(&map)
            .map_err(|e| format!("序列化失败: {e}"))?;
        let tmp = self.secrets_path.with_extension("json.tmp");
        std::fs::write(&tmp, &json)
            .map_err(|e| format!("写入 secrets.json.tmp 失败: {e}"))?;
        std::fs::rename(&tmp, &self.secrets_path)
            .map_err(|e| format!("重命名 secrets.json 失败: {e}"))?;
        Ok(())
    }

    fn load(&self, key: &str) -> Option<String> {
        if !self.secrets_path.exists() {
            return None;
        }
        let data = std::fs::read_to_string(&self.secrets_path).ok()?;
        let map: HashMap<String, String> = serde_json::from_str(&data).ok()?;
        let encoded = map.get(key)?;
        // DPAPI 解密（TODO: 接入 winapi CryptUnprotectData）
        let bytes = base64_decode(encoded).ok()?;
        String::from_utf8(bytes).ok()
    }

    fn delete(&self, key: &str) -> Result<(), String> {
        if !self.secrets_path.exists() {
            return Ok(());
        }
        let data = std::fs::read_to_string(&self.secrets_path)
            .map_err(|e| format!("读取失败: {e}"))?;
        let mut map: HashMap<String, String> = serde_json::from_str(&data)
            .map_err(|e| format!("解析失败: {e}"))?;
        map.remove(key);
        let json = serde_json::to_string_pretty(&map)
            .map_err(|e| format!("序列化失败: {e}"))?;
        std::fs::write(&self.secrets_path, &json)
            .map_err(|e| format!("写入失败: {e}"))?;
        Ok(())
    }
}

// ─── 简易 base64（占位用，DPAPI 实现后移除）────────────────────────────────

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 { result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char); } else { result.push('='); }
        if chunk.len() > 2 { result.push(CHARS[(triple & 0x3F) as usize] as char); } else { result.push('='); }
    }
    result
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    let s: String = s.chars().filter(|c| *c != '=' && !c.is_whitespace()).collect();
    let mut result = Vec::new();
    let bytes = s.as_bytes();
    for chunk in bytes.chunks(4) {
        let mut val: u32 = 0;
        for &b in chunk {
            let idx = match b {
                b'A'..=b'Z' => b - b'A',
                b'a'..=b'z' => b - b'a' + 26,
                b'0'..=b'9' => b - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                _ => return Err("invalid base64".to_string()),
            };
            val = (val << 6) | idx as u32;
        }
        let pad = 4 - chunk.len();
        result.push((val >> 16) as u8);
        if pad < 2 { result.push((val >> 8) as u8); }
        if pad < 1 { result.push(val as u8); }
    }
    Ok(result)
}

// ─── 测试 ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_keeper_crud() {
        let k = InMemoryKeeper::new();
        k.save("test", "hello").unwrap();
        assert_eq!(k.load("test"), Some("hello".into()));
        k.delete("test").unwrap();
        assert_eq!(k.load("test"), None);
    }

    #[test]
    fn in_memory_keeper_overwrite() {
        let k = InMemoryKeeper::new();
        k.save("key", "v1").unwrap();
        k.save("key", "v2").unwrap();
        assert_eq!(k.load("key"), Some("v2".into()));
    }

    #[test]
    fn base64_round_trip() {
        let original = b"Hello, world! 1234567890";
        let encoded = base64_encode(original);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn base64_empty() {
        let encoded = base64_encode(b"");
        let decoded = base64_decode(&encoded).unwrap();
        assert!(decoded.is_empty());
    }
}

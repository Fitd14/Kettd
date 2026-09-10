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

// ─── Windows DPAPI 实现（kb-qa-fixes qa-2：真加密，整张密文）────────────────
//
// 文件格式：secrets.json = base64(DPAPI(json(map)))——整体一次加密，
// 连键名都不以明文出现。per-user 熵（pbDataDescr=NULL）：同 Windows 用户可解，
// 换用户/换机不可解 → 静默视为未配置（kb-spec §8 降级语义）。

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

    /// 读取整张明文 map：文件缺失/损坏/解密失败 → 空 map（写入时覆盖，无损降级）
    fn read_map(&self) -> HashMap<String, String> {
        let Ok(data) = std::fs::read_to_string(&self.secrets_path) else {
            return HashMap::new();
        };
        let Ok(cipher) = base64_decode(data.trim()) else {
            return HashMap::new();
        };
        let Ok(plain) = dpapi_decrypt(&cipher) else {
            return HashMap::new();
        };
        serde_json::from_str(&String::from_utf8_lossy(&plain)).unwrap_or_default()
    }

    /// 整张加密原子写
    fn write_map(&self, map: &HashMap<String, String>) -> Result<(), String> {
        let json = serde_json::to_string(map).map_err(|e| format!("序列化失败: {e}"))?;
        let cipher = dpapi_encrypt(json.as_bytes())?;
        let encoded = base64_encode(&cipher);
        let tmp = self.secrets_path.with_extension("json.tmp");
        std::fs::write(&tmp, encoded).map_err(|e| format!("写入 secrets.json.tmp 失败: {e}"))?;
        std::fs::rename(&tmp, &self.secrets_path)
            .map_err(|e| format!("重命名 secrets.json 失败: {e}"))?;
        Ok(())
    }
}

#[cfg(target_os = "windows")]
impl SecretsKeeper for DpapiKeeper {
    fn save(&self, key: &str, value: &str) -> Result<(), String> {
        let mut map = self.read_map();
        map.insert(key.to_string(), value.to_string());
        self.write_map(&map)
    }

    fn load(&self, key: &str) -> Option<String> {
        self.read_map().get(key).cloned()
    }

    fn delete(&self, key: &str) -> Result<(), String> {
        let mut map = self.read_map();
        map.remove(key);
        self.write_map(&map)
    }
}

/// DPAPI 加密（per-user）
#[cfg(target_os = "windows")]
fn dpapi_encrypt(plain: &[u8]) -> Result<Vec<u8>, String> {
    use winapi::um::dpapi::CryptProtectData;
    use winapi::um::wincrypt::DATA_BLOB;
    use winapi::um::winbase::LocalFree;

    unsafe {
        let mut in_blob = DATA_BLOB {
            cbData: plain.len() as u32,
            pbData: plain.as_ptr() as *mut u8,
        };
        let mut out_blob = DATA_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        let ok = CryptProtectData(
            &mut in_blob,
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
            &mut out_blob,
        );
        if ok == 0 {
            return Err("CryptProtectData 失败".to_string());
        }
        let out = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec();
        LocalFree(out_blob.pbData as _);
        Ok(out)
    }
}

/// DPAPI 解密（per-user）
#[cfg(target_os = "windows")]
fn dpapi_decrypt(cipher: &[u8]) -> Result<Vec<u8>, String> {
    use winapi::um::dpapi::CryptUnprotectData;
    use winapi::um::wincrypt::DATA_BLOB;
    use winapi::um::winbase::LocalFree;

    unsafe {
        let mut in_blob = DATA_BLOB {
            cbData: cipher.len() as u32,
            pbData: cipher.as_ptr() as *mut u8,
        };
        let mut out_blob = DATA_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        let ok = CryptUnprotectData(
            &mut in_blob,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            0,
            &mut out_blob,
        );
        if ok == 0 {
            return Err("CryptUnprotectData 失败（换用户/系统？）".to_string());
        }
        let out = std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec();
        LocalFree(out_blob.pbData as _);
        Ok(out)
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

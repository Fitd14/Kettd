//! 阅读面字体的存放：`<数据目录>/fonts/` 下的一批字体文件。
//!
//! 只做「文件进出」，不认识设置项（设置里存的是 `file:<文件名>`，校验在 `models`）。
//! 所有公开入口都有一份 `*_in(dir, …)` 版本，测试用临时目录，不碰真实数据目录。

use std::path::{Path, PathBuf};

use crate::models::is_safe_font_file_name;

/// 单个字体上限：中文全量字体 8–13 MB，留出余量但不允许拿 100 MB 的文件拖死 IO。
pub const MAX_BYTES: usize = 16 * 1024 * 1024;

/// 允许的扩展名（小写比较）。
const ALLOWED_EXT: [&str; 4] = ["ttf", "otf", "woff", "woff2"];

pub fn dir() -> PathBuf {
  crate::store::data_dir().join("fonts")
}

/// 文件名 → 绝对路径；文件名不合法直接拒。
pub fn path_of(name: &str) -> Result<PathBuf, String> {
  path_of_in(&dir(), name)
}

pub fn path_of_in(base: &Path, name: &str) -> Result<PathBuf, String> {
  if !is_safe_font_file_name(name) {
    return Err("字体文件名不合法".to_string());
  }
  Ok(base.join(name))
}

/// 列出已安装字体（文件名, KB），按文件名排序；目录不存在视为空。
pub fn list() -> Vec<(String, u32)> {
  list_in(&dir())
}

pub fn list_in(base: &Path) -> Vec<(String, u32)> {
  let mut found: Vec<(String, u32)> = Vec::new();
  let Ok(entries) = std::fs::read_dir(base) else {
    return found;
  };
  for entry in entries.flatten() {
    let path = entry.path();
    if !path.is_file() {
      continue;
    }
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
      continue;
    };
    if !is_safe_font_file_name(name) {
      continue;
    }
    let size = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
    found.push((name.to_string(), (size / 1024) as u32));
  }
  found.sort_by(|a, b| a.0.cmp(&b.0));
  found
}

/// 落盘一个字体文件（覆盖同名）。先写 `.tmp` 再 rename，避免半个文件被读到。
pub fn install(name: &str, bytes: &[u8]) -> Result<(), String> {
  install_in(&dir(), name, bytes)
}

pub fn install_in(base: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
  let target = path_of_in(base, name)?;
  if bytes.is_empty() {
    return Err("字体文件是空的".to_string());
  }
  if bytes.len() > MAX_BYTES {
    return Err("字体文件超过 16 MB，换一个更小的版本".to_string());
  }
  if !signature_matches(name, bytes) {
    return Err("这个文件不像是字体（扩展名与内容对不上）".to_string());
  }
  std::fs::create_dir_all(base).map_err(|_| "字体目录创建失败".to_string())?;
  let temp = base.join(format!(".{name}.tmp"));
  std::fs::write(&temp, bytes).map_err(|_| "字体写入失败".to_string())?;
  std::fs::rename(&temp, &target).map_err(|_| "字体保存失败".to_string())?;
  Ok(())
}

/// 读回文件内容（base64），给前端 `FontFace` 注册用。
pub fn read_base64(name: &str) -> Result<String, String> {
  read_base64_in(&dir(), name)
}

pub fn read_base64_in(base: &Path, name: &str) -> Result<String, String> {
  let target = path_of_in(base, name)?;
  let meta = std::fs::metadata(&target).map_err(|_| "找不到这个字体".to_string())?;
  if meta.len() as usize > MAX_BYTES {
    return Err("字体文件过大，读不出来".to_string());
  }
  let bytes = std::fs::read(&target).map_err(|_| "字体读取失败".to_string())?;
  Ok(crate::infra::text_codec::encode(&bytes))
}

/// 删除一个字体文件；本来就不存在不算错（幂等）。
pub fn remove(name: &str) -> Result<(), String> {
  remove_in(&dir(), name)
}

pub fn remove_in(base: &Path, name: &str) -> Result<(), String> {
  let target = path_of_in(base, name)?;
  match std::fs::remove_file(&target) {
    Ok(()) => Ok(()),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
    Err(_) => Err("字体删除失败".to_string()),
  }
}

fn extension_of(name: &str) -> String {
  Path::new(name)
    .extension()
    .and_then(|value| value.to_str())
    .unwrap_or("")
    .to_ascii_lowercase()
}

/// 扩展名与文件头一致性：防止把 .zip 改名成 .ttf 塞进来。
fn signature_matches(name: &str, bytes: &[u8]) -> bool {
  let ext = extension_of(name);
  if !ALLOWED_EXT.contains(&ext.as_str()) {
    return false;
  }
  match ext.as_str() {
    "woff" => bytes.starts_with(b"wOFF"),
    "woff2" => bytes.starts_with(b"wOF2"),
    "otf" => bytes.starts_with(b"OTTO"),
    // TrueType：0x00010000，或旧式 'true' / 'typ1' 标签
    _ => bytes.starts_with(&[0x00, 0x01, 0x00, 0x00])
      || bytes.starts_with(b"true")
      || bytes.starts_with(b"typ1"),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// 临时目录固定在 target/ 下：`std::env::temp_dir()` 在提权/受限环境下可能不可写
  /// （本次门禁就撞到 `Os { code: 5, kind: PermissionDenied }`），构建目录一定可写且被 gitignore。
  fn scratch(tag: &str) -> PathBuf {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("target")
      .join("tmp-fonts")
      .join(format!("{tag}-{}", crate::models::new_id("t")));
    std::fs::create_dir_all(&base).unwrap();
    base
  }

  fn ttf_bytes() -> Vec<u8> {
    let mut bytes = vec![0x00, 0x01, 0x00, 0x00];
    bytes.extend_from_slice(&[0u8; 64]);
    bytes
  }

  #[test]
  fn install_then_list_and_read() {
    let base = scratch("install");
    install_in(&base, "文楷.ttf", &ttf_bytes()).unwrap();
    let listed = list_in(&base);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].0, "文楷.ttf");
    let text = read_base64_in(&base, "文楷.ttf").unwrap();
    let back = crate::infra::text_codec::decode(&text).unwrap();
    assert_eq!(back, ttf_bytes());
    std::fs::remove_dir_all(&base).ok();
  }

  #[test]
  fn rejects_traversal_and_bad_extension() {
    let base = scratch("reject");
    assert!(path_of_in(&base, "../evil.ttf").is_err());
    assert!(path_of_in(&base, "a/b.ttf").is_err());
    assert!(path_of_in(&base, "note.txt").is_err());
    assert!(install_in(&base, "fake.ttf", b"PK\x03\x04").is_err(), "改名的 zip 要拒");
    assert!(install_in(&base, "empty.ttf", b"").is_err());
    std::fs::remove_dir_all(&base).ok();
  }

  #[test]
  fn overwrite_replaces_and_remove_is_idempotent() {
    let base = scratch("overwrite");
    install_in(&base, "a.woff2", b"wOF2\x00\x00").unwrap();
    install_in(&base, "a.woff2", b"wOF2\x01\x01").unwrap();
    let text = read_base64_in(&base, "a.woff2").unwrap();
    assert_eq!(crate::infra::text_codec::decode(&text).unwrap(), b"wOF2\x01\x01".to_vec());
    remove_in(&base, "a.woff2").unwrap();
    remove_in(&base, "a.woff2").unwrap();
    assert!(list_in(&base).is_empty());
    std::fs::remove_dir_all(&base).ok();
  }
}
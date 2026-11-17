//! 阅读主题的存放：`<数据目录>/themes/` 下的一批 `.css` 文件。
//!
//! 只做「文件进出」：不认识设置项、不解析 CSS（解析与白名单校验在前端
//! `src-react/src/lib/reading-theme.js`；这里只挡外部资源与超限文件）。
//! 所有公开入口都有一份 `*_in(dir, …)` 版本，测试用临时目录，不碰真实数据目录。

use std::path::{Path, PathBuf};

use crate::models::is_safe_theme_file_name;

/// 单个主题文件上限：主题只是变量清单，512 KB 已经宽裕。
pub const MAX_BYTES: usize = 512 * 1024;

pub fn dir() -> PathBuf {
  crate::store::data_dir().join("themes")
}

/// 文件名 → 绝对路径；文件名不合法直接拒。
pub fn path_of(name: &str) -> Result<PathBuf, String> {
  path_of_in(&dir(), name)
}

pub fn path_of_in(base: &Path, name: &str) -> Result<PathBuf, String> {
  if !is_safe_theme_file_name(name) {
    return Err("主题文件名不合法".to_string());
  }
  Ok(base.join(name))
}

/// 列出已安装主题（文件名, KB），按文件名排序；目录不存在视为空。
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
    if !is_safe_theme_file_name(name) {
      continue;
    }
    let size = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
    found.push((name.to_string(), (size / 1024) as u32));
  }
  found.sort_by(|a, b| a.0.cmp(&b.0));
  found
}

/// 主题里禁止出现的字符串：外部资源与表达式一律拒收（前端解析器同一份清单）。
const FORBIDDEN: [&str; 4] = ["@import", "url(", "expression(", "javascript:"];

/// 落盘一个主题文件（覆盖同名）。先写 `.tmp` 再 rename，避免半个文件被读到。
pub fn install(name: &str, text: &str) -> Result<(), String> {
  install_in(&dir(), name, text)
}

pub fn install_in(base: &Path, name: &str, text: &str) -> Result<(), String> {
  let target = path_of_in(base, name)?;
  let text = text.strip_prefix('\u{feff}').unwrap_or(text);
  if text.trim().is_empty() {
    return Err("主题文件是空的".to_string());
  }
  if text.len() > MAX_BYTES {
    return Err("主题文件超过 512 KB，换一个更小的版本".to_string());
  }
  let lowered = text.to_lowercase();
  if FORBIDDEN.iter().any(|needle| lowered.contains(needle)) {
    return Err("主题里不允许引用外部资源或表达式".to_string());
  }
  std::fs::create_dir_all(base).map_err(|_| "主题目录创建失败".to_string())?;
  let temp = base.join(format!(".{name}.tmp"));
  std::fs::write(&temp, text.as_bytes()).map_err(|_| "主题写入失败".to_string())?;
  std::fs::rename(&temp, &target).map_err(|_| "主题保存失败".to_string())?;
  Ok(())
}

/// 读回主题文本，给前端解析。
pub fn read(name: &str) -> Result<String, String> {
  read_in(&dir(), name)
}

pub fn read_in(base: &Path, name: &str) -> Result<String, String> {
  let target = path_of_in(base, name)?;
  let meta = std::fs::metadata(&target).map_err(|_| "找不到这个主题".to_string())?;
  if meta.len() as usize > MAX_BYTES {
    return Err("主题文件过大，读不出来".to_string());
  }
  let bytes = std::fs::read(&target).map_err(|_| "主题读取失败".to_string())?;
  let text = String::from_utf8(bytes).map_err(|_| "主题不是 UTF-8 文本".to_string())?;
  Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_string())
}

/// 删除一个主题文件；本来就不存在不算错（幂等）。
pub fn remove(name: &str) -> Result<(), String> {
  remove_in(&dir(), name)
}

pub fn remove_in(base: &Path, name: &str) -> Result<(), String> {
  let target = path_of_in(base, name)?;
  match std::fs::remove_file(&target) {
    Ok(()) => Ok(()),
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
    Err(_) => Err("主题删除失败".to_string()),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const SAMPLE: &str = ".kb-theme-light { --paper: #fff; }\n.kb-theme-dark { --paper: #1a1a1a; }\n";

  /// 临时目录固定在 target/ 下：`std::env::temp_dir()` 在提权/受限环境下可能不可写，
  /// 构建目录一定可写且被 gitignore。
  fn scratch(tag: &str) -> PathBuf {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("target")
      .join("tmp-themes")
      .join(format!("{tag}-{}", crate::models::new_id("t")));
    std::fs::create_dir_all(&base).unwrap();
    base
  }

  #[test]
  fn install_then_list_and_read() {
    let base = scratch("install");
    install_in(&base, "我的主题.css", SAMPLE).unwrap();
    let listed = list_in(&base);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].0, "我的主题.css");
    assert_eq!(read_in(&base, "我的主题.css").unwrap(), SAMPLE);
    std::fs::remove_dir_all(&base).ok();
  }

  #[test]
  fn strips_bom_on_write_and_read() {
    let base = scratch("bom");
    install_in(&base, "a.css", &format!("\u{feff}{SAMPLE}")).unwrap();
    assert_eq!(read_in(&base, "a.css").unwrap(), SAMPLE);
    std::fs::remove_dir_all(&base).ok();
  }

  #[test]
  fn rejects_traversal_bad_extension_and_oversize() {
    let base = scratch("reject");
    assert!(path_of_in(&base, "../evil.css").is_err());
    assert!(path_of_in(&base, "a/b.css").is_err());
    assert!(path_of_in(&base, "note.txt").is_err());
    assert!(install_in(&base, "empty.css", "  ").is_err());
    let big = "x".repeat(MAX_BYTES + 1);
    assert!(install_in(&base, "big.css", &big).is_err());
    std::fs::remove_dir_all(&base).ok();
  }

  #[test]
  fn rejects_external_resources_and_expressions() {
    let base = scratch("forbidden");
    for text in [
      ".kb-theme-light { --paper: url(https://a/b.png); }",
      "@import url(https://a/b.css);",
      ".kb-theme-light { --paper: expression(alert(1)); }",
      ".kb-theme-light { --paper: javascript:alert(1); }",
    ] {
      assert!(install_in(&base, "bad.css", text).is_err(), "应拒收：{text}");
    }
    std::fs::remove_dir_all(&base).ok();
  }

  #[test]
  fn overwrite_replaces_and_remove_is_idempotent() {
    let base = scratch("overwrite");
    install_in(&base, "a.css", ".kb-theme-light { --paper: #fff; }").unwrap();
    install_in(&base, "a.css", SAMPLE).unwrap();
    assert_eq!(read_in(&base, "a.css").unwrap(), SAMPLE);
    remove_in(&base, "a.css").unwrap();
    remove_in(&base, "a.css").unwrap();
    assert!(list_in(&base).is_empty());
    std::fs::remove_dir_all(&base).ok();
  }
}
//! 文件系统后端：data.json 的真实 IO（从 store.rs 的原语收编而来）。
//! 写协议保持不变：tmp + rename 原子替换，覆盖前轮转 5 份备份（PRD 6.6）。

use crate::models::{BackupInfo, KbBackupInfo};
use crate::ports::store_backend::StoreBackend;
use chrono::{DateTime, Local};
use std::fs;
use std::path::{Path, PathBuf};

pub const BACKUP_SLOTS: usize = 5;
pub const DATA_FILE: &str = "data.json";
pub const ARCHIVE_FILE: &str = "data.v1.json";
pub const RUNTIME_FILE: &str = "runtime.json";
pub const PRE_SPLIT_FILE: &str = "data.pre-split.json";
pub const NOTES_FILE: &str = "notes.json";
pub const FOLDERS_FILE: &str = "folders.json";
/// kb-storage-backup D1/D2/D7：正文分文件、图片内容寻址、手动导出、旧格式永久存档
pub const NOTES_DIR: &str = "notes";
// 批 3（D2/D3）才调用：本批先交付端口能力，暂不接图片抽取/手动导出，避免 dead_code 噪音
#[allow(dead_code)]
pub const ASSETS_DIR: &str = "assets";
#[allow(dead_code)]
pub const BACKUPS_DIR: &str = "backups";
pub const NOTES_V1_FILE: &str = "notes.v1.json";

/// 图片扩展名白名单（design D2：png/jpg/webp/gif）
#[allow(dead_code)]
const ASSET_EXTS: [&str; 4] = ["png", "jpg", "webp", "gif"];

/// 条目 id 只允许 `[A-Za-z0-9_-]`：new_id("k") 生成值天然满足，
/// 任何带分隔符/点段的输入直接拒绝，杜绝拼出 `notes/../` 路径穿越。
fn safe_note_id(id: &str) -> Result<(), String> {
  let ok = !id.is_empty()
    && id.chars().all(|ch| {
      ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'
    });
  if ok {
    Ok(())
  } else {
    Err("知识条目编号不合法".to_string())
  }
}

/// 资源 sha 只接受十六进制串（长度 64 = sha256；短串仅供测试替身），杜绝路径穿越
#[allow(dead_code)] // 批 3（D2 图片抽取）接入后启用
fn safe_asset_key(sha: &str, ext: &str) -> Result<(), String> {
  let sha_ok = !sha.is_empty() && sha.chars().all(|ch| ch.is_ascii_hexdigit());
  if !sha_ok {
    return Err("资源编号不合法".to_string());
  }
  if !ASSET_EXTS.contains(&ext) {
    return Err("图片格式不支持".to_string());
  }
  Ok(())
}

/// 导出文件名：不许带路径分隔符/点段/控制字符（导出名由 export 模块生成，这里兜底）
#[allow(dead_code)] // 批 3（D3 手动导出）接入后启用
fn safe_export_file_name(name: &str) -> Result<(), String> {
  let trimmed = name.trim();
  let ok = !trimmed.is_empty()
    && trimmed == name
    && trimmed.chars().count() <= 120
    && !trimmed.starts_with('.')
    && trimmed != ".."
    && !trimmed
      .chars()
      .any(|ch| ch.is_control() || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'));
  if ok {
    Ok(())
  } else {
    Err("导出文件名不合法".to_string())
  }
}

/// 返回给用户的导出路径：数据目录通常是绝对路径，相对路径补上当前目录
#[allow(dead_code)] // 批 3（D3）write_export_file 返回值用
fn absolute_path(path: &Path) -> String {
  let full = if path.is_absolute() {
    path.to_path_buf()
  } else {
    std::env::current_dir()
      .map(|cwd| cwd.join(path))
      .unwrap_or_else(|_| path.to_path_buf())
  };
  full.to_string_lossy().into_owned()
}

/// KETTD_DATA_DIR 覆盖语义：非空 → 原样作为数据根（不拼 todo-list）；空/全空白/未设 → 默认。
/// 纯函数：测试不碰进程 env，避免并行测试竞态。
pub fn resolve_data_dir(override_dir: Option<&str>) -> PathBuf {
  match override_dir.map(str::trim).filter(|value| !value.is_empty()) {
    Some(value) => PathBuf::from(value),
    None => dirs::data_dir()
      .unwrap_or_else(|| PathBuf::from("."))
      .join("todo-list"),
  }
}

/// v1 同款目录：Windows 下 `dirs::data_dir()` == `%APPDATA%`；KETTD_DATA_DIR 非空时原样覆盖
pub fn default_dir() -> PathBuf {
  resolve_data_dir(std::env::var("KETTD_DATA_DIR").ok().as_deref())
}

pub struct FsBackend {
  dir: PathBuf,
}

impl FsBackend {
  pub fn new(dir: PathBuf) -> Self {
    FsBackend { dir }
  }

  fn data_path(&self) -> PathBuf {
    self.dir.join(DATA_FILE)
  }

  /// tmp + rename 原子替换；tmp 与目标同目录、同名加 .tmp，保证同盘 rename。
  /// 父目录不存在时自动建（notes/、assets/、backups/ 都是按需创建）
  fn write_bytes_at(&self, target: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = target.parent() {
      fs::create_dir_all(parent)
        .map_err(|error| file_error("无法建立数据目录", &error))?;
    }
    let mut tmp_name = target
      .file_name()
      .map(|value| value.to_os_string())
      .unwrap_or_default();
    tmp_name.push(".tmp");
    let tmp = target.with_file_name(tmp_name);
    if let Err(error) = fs::write(&tmp, bytes) {
      let _ = fs::remove_file(&tmp);
      return Err(file_error("临时文件写入失败，本次没有保存", &error));
    }
    if let Err(error) = fs::rename(&tmp, target) {
      let _ = fs::remove_file(&tmp);
      return Err(file_error("替换数据文件失败，本次没有保存", &error));
    }
    Ok(())
  }

  fn write_data_at(&self, target: &Path, json: &str) -> Result<(), String> {
    self.write_bytes_at(target, json.as_bytes())
  }

  fn notes_dir(&self) -> PathBuf {
    self.dir.join(NOTES_DIR)
  }

  fn assets_dir(&self) -> PathBuf {
    self.dir.join(ASSETS_DIR)
  }

  fn backups_dir(&self) -> PathBuf {
    self.dir.join(BACKUPS_DIR)
  }

  fn note_body_path(&self, id: &str) -> PathBuf {
    self.notes_dir().join(format!("{}.md", id))
  }

  #[allow(dead_code)] // 批 3（D2）资源读写用
  fn asset_path(&self, sha: &str, ext: &str) -> PathBuf {
    self.assets_dir().join(format!("{}.{}", sha, ext))
  }

  #[allow(dead_code)] // 批 3（D3）手动导出用
  fn export_path(&self, file_name: &str) -> PathBuf {
    self.backups_dir().join(file_name)
  }

  fn notes_v1_archive_path(&self) -> PathBuf {
    self.dir.join(NOTES_V1_FILE)
  }

  fn backup_path(&self, slot: usize) -> PathBuf {
    self.dir.join(format!("{}.bak-{}", DATA_FILE, slot))
  }

  /// 知识库备份：notes.json.bak-1..5（与 data.json 同一套轮转语义，D8）
  fn notes_backup_path(&self, slot: usize) -> PathBuf {
    self.dir.join(format!("{}.bak-{}", NOTES_FILE, slot))
  }

  fn folders_path(&self) -> PathBuf {
    self.dir.join(FOLDERS_FILE)
  }

  fn runtime_path(&self) -> PathBuf {
    self.dir.join(RUNTIME_FILE)
  }

  fn pre_split_path(&self) -> PathBuf {
    self.dir.join(PRE_SPLIT_FILE)
  }

  fn notes_path(&self) -> PathBuf {
    self.dir.join(NOTES_FILE)
  }

  fn archive_path(&self) -> PathBuf {
    self.dir.join(ARCHIVE_FILE)
  }

  fn read_text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| file_error("数据文件读取失败", &error))
  }
}

/// 面向 UI 的中文短句（不含路径与堆栈）；与 store.rs 同一套口径
fn io_kind_text(error: &std::io::Error) -> &'static str {
  match error.kind() {
    std::io::ErrorKind::PermissionDenied => "没有写入权限",
    std::io::ErrorKind::NotFound => "找不到文件",
    std::io::ErrorKind::ReadOnlyFilesystem => "目录是只读的",
    _ => "磁盘或路径不可用",
  }
}

fn file_error(prefix: &str, error: &std::io::Error) -> String {
  format!("{}：{}", prefix, io_kind_text(error))
}

fn stamp_text(value: &DateTime<Local>) -> String {
  value.format("%Y-%m-%dT%H-%M-%S").to_string()
}

impl StoreBackend for FsBackend {
  fn read_data(&self) -> Result<Option<String>, String> {
    let path = self.data_path();
    if !path.exists() {
      return Ok(None);
    }
    Self::read_text(&path).map(Some)
  }

  fn write_data(&self, json: &str) -> Result<(), String> {
    let target = self.data_path();
    self.write_data_at(&target, json)
  }

  fn rotate_backups(&self) -> Result<(), String> {
    // bak-4 → bak-5 … bak-1 → bak-2，最后当前档 → bak-1（与 v2.1 既有行为一致；
    // Windows 上 rename 不覆盖已存在目标，必须先删）
    for slot in (1..BACKUP_SLOTS).rev() {
      let from = self.backup_path(slot);
      let to = self.backup_path(slot + 1);
      if from.exists() {
        if to.exists() {
          let _ = fs::remove_file(&to);
        }
        if let Err(error) = fs::rename(&from, &to) {
          return Err(file_error("备份轮转失败，本次没有保存", &error));
        }
      }
    }
    let current = self.data_path();
    if current.exists() {
      let first = self.backup_path(1);
      if first.exists() {
        let _ = fs::remove_file(&first);
      }
      if let Err(error) = fs::copy(&current, &first) {
        return Err(file_error("备份轮转失败，本次没有保存", &error));
      }
    }
    Ok(())
  }

  fn data_exists(&self) -> bool {
    self.data_path().exists()
  }

  fn quarantine(&self) -> Option<String> {
    let source = self.data_path();
    if !source.exists() {
      return None;
    }
    let name = format!("data.corrupt.{}", stamp_text(&Local::now()));
    let target = self.dir.join(&name);
    if fs::rename(&source, &target).is_ok() {
      return Some(name);
    }
    if fs::copy(&source, &target).is_ok() {
      let _ = fs::remove_file(&source);
      return Some(name);
    }
    None
  }

  fn archive_exists(&self) -> bool {
    self.archive_path().exists()
  }

  fn read_archive(&self) -> Result<String, String> {
    Self::read_text(&self.archive_path())
  }

  fn copy_data_to_archive(&self) -> Result<(), String> {
    fs::copy(self.data_path(), self.archive_path())
      .map(|_| ())
      .map_err(|error| file_error("无法留存 v1 原件", &error))
  }

  fn list_backups(&self) -> Vec<BackupInfo> {
    let mut out: Vec<BackupInfo> = Vec::new();
    for slot in 1..=BACKUP_SLOTS {
      let path = self.backup_path(slot);
      if !path.exists() {
        continue;
      }
      let meta = fs::metadata(&path).ok();
      let created_at = meta
        .as_ref()
        .and_then(|value| value.modified().ok())
        .map(|time| {
          let local: DateTime<Local> = time.into();
          local.format("%Y-%m-%dT%H:%M").to_string()
        })
        .unwrap_or_else(|| "未知时间".to_string());
      let size_kb = meta.map(|value| (value.len() + 1023) / 1024).unwrap_or(0);
      let parsed = Self::read_text(&path).and_then(|text| {
        serde_json::from_str::<serde_json::Value>(&text)
          .map_err(|error| error.to_string())
      });
      match parsed {
        Ok(raw) => {
          let task_count = raw
            .get("tasks")
            .and_then(|value| value.as_array())
            .map(|items| items.len() as u32)
            .unwrap_or(0);
          out.push(BackupInfo {
            slot: slot.to_string(),
            name: format!("{}.bak-{}", DATA_FILE, slot),
            created_at,
            size_kb,
            task_count,
            readable: true,
            error: None,
          });
        }
        Err(error) => out.push(BackupInfo {
          slot: slot.to_string(),
          name: format!("{}.bak-{}", DATA_FILE, slot),
          created_at,
          size_kb,
          task_count: 0,
          readable: false,
          error: Some(error),
        }),
      }
    }
    out
  }

  fn read_backup(&self, slot: u32) -> Result<String, String> {
    if slot == 0 || slot as usize > BACKUP_SLOTS {
      return Err(format!("备份编号 {} 不在 1..={} 内", slot, BACKUP_SLOTS));
    }
    Self::read_text(&self.backup_path(slot as usize))
  }

  fn read_runtime(&self) -> Result<Option<String>, String> {
    let path = self.runtime_path();
    if !path.exists() {
      return Ok(None);
    }
    Self::read_text(&path).map(Some)
  }

  fn write_runtime(&self, json: &str) -> Result<(), String> {
    self.write_data_at(&self.runtime_path(), json)
  }

  fn archive_pre_split(&self) -> Result<(), String> {
    let target = self.pre_split_path();
    if target.exists() {
      return Ok(()); // 已存在不覆盖：保留最早那份（与 data.v1.json 同一承诺）
    }
    let source = self.data_path();
    if !source.exists() {
      return Ok(());
    }
    fs::copy(&source, &target)
      .map(|_| ())
      .map_err(|error| file_error("无法留存拆分前原件", &error))
  }

  fn restore_pre_split(&self) -> Result<(), String> {
    let source = self.pre_split_path();
    if !source.exists() {
      return Err("拆分前原件不存在".to_string());
    }
    let target = self.data_path();
    fs::copy(&source, &target)
      .map(|_| ())
      .map_err(|error| file_error("无法回滚数据文件", &error))
  }

  fn remove_runtime(&self) -> Result<(), String> {
    let path = self.runtime_path();
    if path.exists() {
      fs::remove_file(&path).map_err(|error| file_error("无法删除运行态文件", &error))?;
    }
    Ok(())
  }

  fn read_notes(&self) -> Result<Option<String>, String> {
    let path = self.notes_path();
    if !path.exists() {
      return Ok(None);
    }
    Self::read_text(&path).map(Some)
  }

  fn write_notes(&self, json: &str) -> Result<(), String> {
    let target = self.notes_path();
    self.write_data_at(&target, json)
  }

  fn rotate_notes_backups(&self) -> Result<(), String> {
    // bak-4 → bak-5 … bak-1 → bak-2，最后当前 notes.json → bak-1（与 rotate_backups 对称；
    // Windows 上 rename 不覆盖已存在目标，必须先删）
    for slot in (1..BACKUP_SLOTS).rev() {
      let from = self.notes_backup_path(slot);
      let to = self.notes_backup_path(slot + 1);
      if from.exists() {
        if to.exists() {
          let _ = fs::remove_file(&to);
        }
        if let Err(error) = fs::rename(&from, &to) {
          return Err(file_error("备份轮转失败，本次没有保存", &error));
        }
      }
    }
    let current = self.notes_path();
    if current.exists() {
      let first = self.notes_backup_path(1);
      if first.exists() {
        let _ = fs::remove_file(&first);
      }
      if let Err(error) = fs::copy(&current, &first) {
        return Err(file_error("备份轮转失败，本次没有保存", &error));
      }
    }
    Ok(())
  }

  fn list_kb_backups(&self) -> Vec<KbBackupInfo> {
    let mut out: Vec<KbBackupInfo> = Vec::new();
    for slot in 1..=BACKUP_SLOTS {
      let path = self.notes_backup_path(slot);
      if !path.exists() {
        continue;
      }
      let meta = fs::metadata(&path).ok();
      let created_at = meta
        .as_ref()
        .and_then(|value| value.modified().ok())
        .map(|time| {
          let local: DateTime<Local> = time.into();
          local.format("%Y-%m-%dT%H:%M").to_string()
        })
        .unwrap_or_else(|| "未知时间".to_string());
      let size_kb = meta.map(|value| (value.len() + 1023) / 1024).unwrap_or(0);
      let parsed = Self::read_text(&path).and_then(|text| {
        serde_json::from_str::<serde_json::Value>(&text).map_err(|_| "备份内容无法解析".to_string())
      });
      match parsed {
        Ok(raw) => {
          let note_count = raw
            .as_array()
            .map(|items| items.len() as u32)
            .unwrap_or(0);
          out.push(KbBackupInfo {
            slot: slot.to_string(),
            name: format!("{}.bak-{}", NOTES_FILE, slot),
            created_at,
            size_kb,
            note_count,
            readable: true,
            error: None,
          });
        }
        Err(error) => out.push(KbBackupInfo {
          slot: slot.to_string(),
          name: format!("{}.bak-{}", NOTES_FILE, slot),
          created_at,
          size_kb,
          note_count: 0,
          readable: false,
          error: Some(error),
        }),
      }
    }
    out
  }

  fn read_notes_backup(&self, slot: u32) -> Result<String, String> {
    if slot == 0 || slot as usize > BACKUP_SLOTS {
      return Err(format!("备份编号 {} 不在 1..={} 内", slot, BACKUP_SLOTS));
    }
    Self::read_text(&self.notes_backup_path(slot as usize))
  }

  fn read_folders(&self) -> Result<Option<String>, String> {
    let path = self.folders_path();
    if !path.exists() {
      return Ok(None);
    }
    Self::read_text(&path).map(Some)
  }

  fn write_folders(&self, json: &str) -> Result<(), String> {
    let target = self.folders_path();
    self.write_data_at(&target, json)
  }

  // ------------------------------------------------ kb-storage-backup（D1/D2/D7）

  fn read_note_body(&self, id: &str) -> Result<Option<String>, String> {
    safe_note_id(id)?;
    let path = self.note_body_path(id);
    if !path.exists() {
      return Ok(None); // 正文缺失按空串加载，不判损坏（design D1）
    }
    Self::read_text(&path).map(Some)
  }

  fn write_note_body(&self, id: &str, body: &str) -> Result<(), String> {
    safe_note_id(id)?;
    let target = self.note_body_path(id);
    self.write_bytes_at(&target, body.as_bytes())
  }

  fn delete_note_body(&self, id: &str) -> Result<(), String> {
    safe_note_id(id)?;
    let path = self.note_body_path(id);
    if path.exists() {
      fs::remove_file(&path).map_err(|error| file_error("删除正文文件失败", &error))?;
    }
    Ok(())
  }

  fn asset_exists(&self, sha: &str, ext: &str) -> bool {
    safe_asset_key(sha, ext).is_ok() && self.asset_path(sha, ext).exists()
  }

  fn read_asset(&self, sha: &str, ext: &str) -> Result<Vec<u8>, String> {
    safe_asset_key(sha, ext)?;
    fs::read(self.asset_path(sha, ext))
      .map_err(|error| file_error("资源文件读取失败", &error))
  }

  fn write_asset(&self, sha: &str, ext: &str, bytes: &[u8]) -> Result<(), String> {
    safe_asset_key(sha, ext)?;
    let target = self.asset_path(sha, ext);
    self.write_bytes_at(&target, bytes)
  }

  /// Bootstrap.assetsDir 用：给前端绝对路径，convertFileSrc 只认绝对路径。
  fn assets_dir_abs(&self) -> String {
    absolute_path(&self.assets_dir())
  }

  fn write_export_file(&self, file_name: &str, text: &str) -> Result<String, String> {
    safe_export_file_name(file_name)?;
    let target = self.export_path(file_name);
    self.write_bytes_at(&target, text.as_bytes())?;
    Ok(absolute_path(&target))
  }

  fn create_backups_dir(&self) -> Result<(), String> {
    fs::create_dir_all(self.backups_dir())
      .map_err(|error| file_error("无法建立备份目录", &error))
  }

  fn notes_v1_archive_exists(&self) -> bool {
    self.notes_v1_archive_path().exists()
  }

  fn write_notes_v1_archive(&self, json: &str) -> Result<(), String> {
    let target = self.notes_v1_archive_path();
    self.write_bytes_at(&target, json.as_bytes())
  }

  fn read_notes_v1_archive(&self) -> Result<Option<String>, String> {
    let path = self.notes_v1_archive_path();
    if !path.exists() {
      return Ok(None);
    }
    Self::read_text(&path).map(Some)
  }
}

#[cfg(test)]
mod tests {
  use super::{resolve_data_dir, FsBackend, StoreBackend, NOTES_V1_FILE};
  use crate::models::new_id;
  use std::path::PathBuf;

  /// 临时目录守卫：无论断言失败与否都清理，绝不残留测试文件
  struct TempDir(PathBuf);

  impl TempDir {
    fn new(tag: &str) -> Self {
      let name = format!("kettd-fs-{}-{}", tag, new_id("t"));
      // 先试系统 temp；受限令牌 / 沙箱下可能不可写（门禁实测 Os error 5），
      // 再退到构建目录（一定可写且被 gitignore）。
      let candidates = [
        std::env::temp_dir().join(&name),
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
          .join("target")
          .join("test-tmp")
          .join(&name),
      ];
      let mut last_error = None;
      for candidate in candidates {
        match std::fs::create_dir_all(&candidate) {
          Ok(()) => return TempDir(candidate),
          Err(error) => last_error = Some((candidate, error)),
        }
      }
      let (path, error) = last_error.expect("至少一个候选临时目录");
      panic!(
        "测试临时目录必须可建（不碰真实 %APPDATA%）：{} — {error}",
        path.display()
      );
    }

    fn path(&self) -> PathBuf {
      self.0.clone()
    }
  }

  impl Drop for TempDir {
    fn drop(&mut self) {
      let _ = std::fs::remove_dir_all(&self.0);
    }
  }

  #[test]
  fn override_used_verbatim_without_subdir() {
    assert_eq!(
      resolve_data_dir(Some("D:\\kettd-test-data")),
      PathBuf::from("D:\\kettd-test-data")
    );
  }

  #[test]
  fn none_falls_back_to_appdata_todo_list() {
    assert!(resolve_data_dir(None).ends_with("todo-list"));
  }

  #[test]
  fn empty_or_whitespace_falls_back_to_default() {
    assert!(resolve_data_dir(Some("")).ends_with("todo-list"));
    assert!(resolve_data_dir(Some("   ")).ends_with("todo-list"));
  }

  #[test]
  fn fs_backend_round_trips_bodies_with_atomic_write() {
    let temp = TempDir::new("body");
    let dir = temp.path();
    let backend = FsBackend::new(dir.clone());

    assert_eq!(
      backend.read_note_body("k1").unwrap(),
      None,
      "正文文件不存在 → None（调用方按空串处理）"
    );
    backend.write_note_body("k1", "# 标题\n正文").unwrap();
    let body_path = dir.join("notes").join("k1.md");
    assert_eq!(
      std::fs::read_to_string(&body_path).unwrap(),
      "# 标题\n正文"
    );
    assert!(
      !dir.join("notes").join("k1.md.tmp").exists(),
      "原子写完成后不得残留 tmp"
    );
    assert_eq!(
      backend.read_note_body("k1").unwrap().as_deref(),
      Some("# 标题\n正文")
    );

    // 覆盖写（迁移幂等依赖此语义）
    backend.write_note_body("k1", "改写").unwrap();
    assert_eq!(
      backend.read_note_body("k1").unwrap().as_deref(),
      Some("改写")
    );

    backend.delete_note_body("k1").unwrap();
    assert!(!body_path.exists());
    assert_eq!(backend.read_note_body("k1").unwrap(), None);
    backend.delete_note_body("k1").unwrap(); // 幂等：不存在也算成功
  }

  #[test]
  fn fs_backend_rejects_path_traversal_ids() {
    let temp = TempDir::new("bad-id");
    let backend = FsBackend::new(temp.path());
    for bad in ["", "../evil", "k1/../x", "k1\\x", "k 1", "k:1"] {
      assert!(
        backend.write_note_body(bad, "x").is_err(),
        "非法 id 必须拒绝：{bad:?}"
      );
      assert!(backend.read_note_body(bad).is_err());
      assert!(backend.delete_note_body(bad).is_err());
    }
    assert!(backend.write_note_body("k-1_A", "ok").is_ok(), "允许的字符集应通过");
  }

  #[test]
  fn fs_backend_round_trips_assets_and_v1_archive() {
    let temp = TempDir::new("asset");
    let dir = temp.path();
    let backend = FsBackend::new(dir.clone());
    let sha = crate::digest::sha256_hex(b"png-bytes");

    assert!(!backend.asset_exists(&sha, "png"));
    backend.write_asset(&sha, "png", b"png-bytes").unwrap();
    assert!(backend.asset_exists(&sha, "png"));
    assert_eq!(backend.read_asset(&sha, "png").unwrap().as_slice(), b"png-bytes");
    assert!(dir.join("assets").join(format!("{sha}.png")).exists());
    assert!(!dir.join("assets").join(format!("{sha}.png.tmp")).exists());

    // 白名单外的扩展名与非法 sha 一律拒绝
    assert!(backend.write_asset(&sha, "exe", b"x").is_err());
    assert!(backend.read_asset("../../evil", "png").is_err());
    assert!(!backend.asset_exists("../../evil", "png"));

    // notes.v1.json 旧格式永久存档
    assert!(!backend.notes_v1_archive_exists());
    backend
      .write_notes_v1_archive("[{\"id\":\"k1\",\"bodyMd\":\"旧\"}]")
      .unwrap();
    assert!(backend.notes_v1_archive_exists());
    assert_eq!(
      backend.read_notes_v1_archive().unwrap().as_deref(),
      Some("[{\"id\":\"k1\",\"bodyMd\":\"旧\"}]")
    );
    assert!(dir.join(NOTES_V1_FILE).exists());
  }

  #[test]
  fn fs_backend_writes_export_files_into_backups_dir() {
    let temp = TempDir::new("export");
    let dir = temp.path();
    let backend = FsBackend::new(dir.clone());

    backend.create_backups_dir().unwrap();
    assert!(dir.join("backups").is_dir());

    let path = backend
      .write_export_file("周报-2026-W36.md", "# 本周")
      .unwrap();
    assert!(
      std::path::Path::new(&path).is_absolute(),
      "导出路径必须是绝对路径：{path}"
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "# 本周");
    assert!(dir.join("backups").join("周报-2026-W36.md").exists());
    assert!(!dir.join("backups").join("周报-2026-W36.md.tmp").exists());

    for bad in ["", " ../逃逸.md", "../逃逸.md", "a/b.md", "a\\b.md", "a:b.md"] {
      assert!(
        backend.write_export_file(bad, "x").is_err(),
        "非法导出名必须拒绝：{bad:?}"
      );
    }
  }
}

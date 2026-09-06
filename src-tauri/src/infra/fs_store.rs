//! 文件系统后端：data.json 的真实 IO（从 store.rs 的原语收编而来）。
//! 写协议保持不变：tmp + rename 原子替换，覆盖前轮转 5 份备份（PRD 6.6）。

use crate::models::BackupInfo;
use crate::ports::store_backend::StoreBackend;
use chrono::{DateTime, Local};
use std::fs;
use std::path::{Path, PathBuf};

pub const BACKUP_SLOTS: usize = 5;
pub const DATA_FILE: &str = "data.json";
pub const ARCHIVE_FILE: &str = "data.v1.json";
pub const RUNTIME_FILE: &str = "runtime.json";
pub const PRE_SPLIT_FILE: &str = "data.pre-split.json";

/// v1 同款目录：Windows 下 `dirs::data_dir()` == `%APPDATA%`
pub fn default_dir() -> PathBuf {
  dirs::data_dir()
    .unwrap_or_else(|| PathBuf::from("."))
    .join("todo-list")
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

  /// tmp + rename 原子替换；tmp 与目标同目录、同名加 .tmp，保证同盘 rename
  fn write_data_at(&self, target: &Path, json: &str) -> Result<(), String> {
    let mut tmp_name = target
      .file_name()
      .map(|value| value.to_os_string())
      .unwrap_or_default();
    tmp_name.push(".tmp");
    let tmp = target.with_file_name(tmp_name);
    if let Err(error) = fs::write(&tmp, json.as_bytes()) {
      let _ = fs::remove_file(&tmp);
      return Err(file_error("临时文件写入失败，本次没有保存", &error));
    }
    if let Err(error) = fs::rename(&tmp, target) {
      let _ = fs::remove_file(&tmp);
      return Err(file_error("替换数据文件失败，本次没有保存", &error));
    }
    Ok(())
  }

  fn backup_path(&self, slot: usize) -> PathBuf {
    self.dir.join(format!("{}.bak-{}", DATA_FILE, slot))
  }

  fn runtime_path(&self) -> PathBuf {
    self.dir.join(RUNTIME_FILE)
  }

  fn pre_split_path(&self) -> PathBuf {
    self.dir.join(PRE_SPLIT_FILE)
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
}

//! 存储后端端口（ADR-0002）。`data_dir()` 硬编码 `%APPDATA%` 曾让核心逻辑离线不可测；
//! 现在 IO 细节全部收进实现，`Store` 只面对字节串与备份语义。

use crate::models::BackupInfo;

/// data.json 的物理读写端口。**只搬字节，不做任何 JSON/领域解析** ——
/// 解析与校验是 Store（存储安全层）的职责，端口保持哑。
pub trait StoreBackend: Send {
  /// 读 data.json 原文；`None` = 文件不存在（全新安装）
  fn read_data(&self) -> Result<Option<String>, String>;

  /// 原子替换写（tmp + rename）。对应 save / save_light 的落盘动作
  fn write_data(&self, json: &str) -> Result<(), String>;

  /// 覆盖写前轮转备份（data.json.bak-1..5）
  fn rotate_backups(&self) -> Result<(), String>;

  /// data.json 是否存在
  fn data_exists(&self) -> bool;

  /// 把（疑似损坏的）data.json 另存为 data.corrupt.<时间戳>，返回留档文件名。
  /// 原件永不删除，是 corrupt 通道的一部分。
  fn quarantine(&self) -> Option<String>;

  /// v1 原件留档（data.v1.json）是否存在
  fn archive_exists(&self) -> bool;

  /// 读 v1 原件
  fn read_archive(&self) -> Result<String, String>;

  /// 把当前 data.json 复制为 v1 原件留档（已存在不覆盖的判定由 Store 先行）
  fn copy_data_to_archive(&self) -> Result<(), String>;

  /// 恢复页列表：bak-1..N 的时间 / 大小 / 任务数 / 可读性
  fn list_backups(&self) -> Vec<BackupInfo>;

  /// 读第 slot 份备份原文（恢复用）
  fn read_backup(&self, slot: u32) -> Result<String, String>;

  /// 读 runtime.json（ADR-0005 运行态）；None = 不存在
  fn read_runtime(&self) -> Result<Option<String>, String>;

  /// 原子写 runtime.json（不轮转：运行态没有历史价值）
  fn write_runtime(&self, json: &str) -> Result<(), String>;

  /// schema 拆分留档：把当前 data.json 复制为 data.pre-split.json。
  /// 已存在则**跳过**（保留最早那份），原件永不删除。
  fn archive_pre_split(&self) -> Result<(), String>;

  /// schema 回滚（仅调试入口）：把 data.pre-split.json 复制回 data.json
  fn restore_pre_split(&self) -> Result<(), String>;

  /// schema 回滚（仅调试入口）：删除 runtime.json
  fn remove_runtime(&self) -> Result<(), String>;

  /// 读 notes.json（知识库，frame H1b）；None = 不存在
  fn read_notes(&self) -> Result<Option<String>, String>;

  /// 原子写 notes.json
  fn write_notes(&self, json: &str) -> Result<(), String>;

  /// 读 kb_index.json（AI 向量索引，Phase 3）；None = 不存在
  fn read_kb_index(&self) -> Result<Option<String>, String>;

  /// 原子写 kb_index.json
  fn write_kb_index(&self, json: &str) -> Result<(), String>;
}

#[cfg(test)]
pub mod test_double {
  use super::{BackupInfo, StoreBackend};
  use std::cell::RefCell;

  /// 内存后端：一个 `RefCell<String>` 就是整个"磁盘"。
  /// 磁盘满 / 只读目录这类故障注入 `fail_writes` 后再验证写路径的报错文案。
  pub struct InMemoryBackend {
    pub data: RefCell<Option<String>>,
    pub archive: RefCell<Option<String>>,
    pub runtime: RefCell<Option<String>>,
    pub notes: RefCell<Option<String>>,
    pub kb_index: RefCell<Option<String>>,
    pub pre_split: RefCell<Option<String>>,
    pub backups: RefCell<Vec<String>>,
    pub fail_writes: RefCell<bool>,
    /// 调用序列记录（编排顺序断言用：runtime 先于 data 落盘等）
    pub log: RefCell<Vec<String>>,
    quarantine_count: RefCell<u32>,
  }

  impl InMemoryBackend {
    pub fn new() -> Self {
      Self {
        data: RefCell::new(None),
        archive: RefCell::new(None),
        runtime: RefCell::new(None),
        notes: RefCell::new(None),
        kb_index: RefCell::new(None),
        pre_split: RefCell::new(None),
        backups: RefCell::new(Vec::new()),
        fail_writes: RefCell::new(false),
        log: RefCell::new(Vec::new()),
        quarantine_count: RefCell::new(0),
      }
    }

    /// 预置已存在的 data.json（JSON 字符串）
    pub fn with_data(json: &str) -> Self {
      Self {
        data: RefCell::new(Some(json.to_string())),
        ..Self::new()
      }
    }

    pub fn set_fail_writes(&self, on: bool) {
      *self.fail_writes.borrow_mut() = on;
    }

    pub fn quarantines(&self) -> u32 {
      *self.quarantine_count.borrow()
    }

    fn note(&self, what: &str) {
      self.log.borrow_mut().push(what.to_string());
    }
  }

  impl Default for InMemoryBackend {
    fn default() -> Self {
      Self::new()
    }
  }

  impl StoreBackend for InMemoryBackend {
    fn read_data(&self) -> Result<Option<String>, String> {
      Ok(self.data.borrow().clone())
    }

    fn write_data(&self, json: &str) -> Result<(), String> {
      self.note("write_data");
      if *self.fail_writes.borrow() {
        return Err("没有写入权限".to_string());
      }
      *self.data.borrow_mut() = Some(json.to_string());
      Ok(())
    }

    fn rotate_backups(&self) -> Result<(), String> {
      self.note("rotate_backups");
      if *self.fail_writes.borrow() {
        return Err("备份轮转失败".to_string());
      }
      let mut slots = self.backups.borrow_mut();
      let current = self.data.borrow().clone();
      if let Some(previous) = current {
        slots.insert(0, previous);
      }
      slots.truncate(5);
      Ok(())
    }

    fn data_exists(&self) -> bool {
      self.data.borrow().is_some()
    }

    fn quarantine(&self) -> Option<String> {
      if self.data.borrow().is_none() {
        return None;
      }
      *self.quarantine_count.borrow_mut() += 1;
      let n = self.quarantine_count.borrow();
      Some(format!("data.corrupt.test-{}", n))
    }

    fn archive_exists(&self) -> bool {
      self.archive.borrow().is_some()
    }

    fn read_archive(&self) -> Result<String, String> {
      self
        .archive
        .borrow()
        .clone()
        .ok_or_else(|| "v1 原件不存在".to_string())
    }

    fn copy_data_to_archive(&self) -> Result<(), String> {
      let current = self.data.borrow().clone();
      match current {
        Some(json) => {
          *self.archive.borrow_mut() = Some(json);
          Ok(())
        }
        None => Err("没有可留档的数据文件".to_string()),
      }
    }

    fn list_backups(&self) -> Vec<BackupInfo> {
      self
        .backups
        .borrow()
        .iter()
        .enumerate()
        .map(|(index, json)| BackupInfo {
          slot: (index + 1).to_string(),
          name: format!("data.json.bak-{}", index + 1),
          created_at: "测试时间".to_string(),
          size_kb: (json.len() as u64 + 1023) / 1024,
          task_count: 0,
          readable: true,
          error: None,
        })
        .collect()
    }

    fn read_backup(&self, slot: u32) -> Result<String, String> {
      if slot == 0 {
        return Err("备份编号从 1 开始".to_string());
      }
      let slots = self.backups.borrow();
      slots
        .get(slot as usize - 1)
        .cloned()
        .ok_or_else(|| format!("备份 bak-{} 不存在", slot))
    }

    fn read_runtime(&self) -> Result<Option<String>, String> {
      self.note("read_runtime");
      Ok(self.runtime.borrow().clone())
    }

    fn write_runtime(&self, json: &str) -> Result<(), String> {
      self.note("write_runtime");
      if *self.fail_writes.borrow() {
        return Err("没有写入权限".to_string());
      }
      *self.runtime.borrow_mut() = Some(json.to_string());
      Ok(())
    }

    fn archive_pre_split(&self) -> Result<(), String> {
      self.note("archive_pre_split");
      if *self.fail_writes.borrow() {
        return Err("没有写入权限".to_string());
      }
      let mut pre = self.pre_split.borrow_mut();
      if pre.is_some() {
        return Ok(()); // 已存在不覆盖：保留最早那份
      }
      *pre = self.data.borrow().clone();
      Ok(())
    }

    fn restore_pre_split(&self) -> Result<(), String> {
      self.note("restore_pre_split");
      let pre = self.pre_split.borrow().clone();
      match pre {
        Some(json) => {
          *self.data.borrow_mut() = Some(json);
          Ok(())
        }
        None => Err("拆分前原件不存在".to_string()),
      }
    }

    fn remove_runtime(&self) -> Result<(), String> {
      self.note("remove_runtime");
      *self.runtime.borrow_mut() = None;
      Ok(())
    }

    fn read_notes(&self) -> Result<Option<String>, String> {
      Ok(self.notes.borrow().clone())
    }

    fn write_notes(&self, json: &str) -> Result<(), String> {
      if *self.fail_writes.borrow() {
        return Err("没有写入权限".to_string());
      }
      *self.notes.borrow_mut() = Some(json.to_string());
      Ok(())
    }

    fn read_kb_index(&self) -> Result<Option<String>, String> {
      Ok(self.kb_index.borrow().clone())
    }

    fn write_kb_index(&self, json: &str) -> Result<(), String> {
      if *self.fail_writes.borrow() {
        return Err("没有写入权限".to_string());
      }
      *self.kb_index.borrow_mut() = Some(json.to_string());
      Ok(())
    }
  }
}

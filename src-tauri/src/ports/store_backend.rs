//! 存储后端端口（ADR-0002）。`data_dir()` 硬编码 `%APPDATA%` 曾让核心逻辑离线不可测；
//! 现在 IO 细节全部收进实现，`Store` 只面对字节串与备份语义。

use crate::models::{BackupInfo, KbBackupInfo};

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

  /// 覆盖写前轮转知识库备份（notes.json.bak-4→5 … bak-1→2，当前→bak-1）。
  /// 与 `rotate_backups` 对称；写 notes.json 前必须调用（D8）。
  fn rotate_notes_backups(&self) -> Result<(), String>;

  /// 知识库恢复页列表：notes.json.bak-1..N 的时间 / 大小 / 条目数 / 可读性
  fn list_kb_backups(&self) -> Vec<KbBackupInfo>;

  /// 读第 slot 份知识库备份原文（恢复用）
  fn read_notes_backup(&self, slot: u32) -> Result<String, String>;

  /// 读 folders.json（知识库文件夹，D6）；None = 不存在（视为空表，零迁移）
  fn read_folders(&self) -> Result<Option<String>, String>;

  /// 原子写 folders.json
  fn write_folders(&self, json: &str) -> Result<(), String>;

  // ---------------------------------------------------------------- kb-storage-backup（D1/D2/D7）
  // 端口仍只搬字节：路径规则、建目录、tmp+rename 都是实现的职责，Store 不做 IO。

  /// 读 `notes/<id>.md` 正文；None = 文件不存在（按空正文加载，不判损坏）
  fn read_note_body(&self, id: &str) -> Result<Option<String>, String>;

  /// 原子写 `notes/<id>.md`（tmp + rename；目录不存在自动建）
  fn write_note_body(&self, id: &str, body: &str) -> Result<(), String>;

  /// 删除 `notes/<id>.md`；文件不存在视为成功（删除天然幂等）
  fn delete_note_body(&self, id: &str) -> Result<(), String>;

  /// 绝对 assets 目录（Bootstrap.assetsDir：前端用它把 `assets/<sha>.<ext>` 拼成
  /// `convertFileSrc` 可用的绝对路径）。
  ///
  /// 默认值给内存/测试后端用：它们没有真实目录，只能给一个临时占位（不会被前端用于生产显示）。
  fn assets_dir_abs(&self) -> String {
    std::env::temp_dir()
      .join("kettd-assets")
      .to_string_lossy()
      .into_owned()
  }

  /// `assets/<sha>.<ext>` 是否存在（内容寻址天然去重）
  #[allow(dead_code)] // 批 3（D2 图片抽取）接入后启用
  fn asset_exists(&self, sha: &str, ext: &str) -> bool;

  /// 读资源文件
  #[allow(dead_code)] // 批 3（D2 图片抽取）接入后启用
  fn read_asset(&self, sha: &str, ext: &str) -> Result<Vec<u8>, String>;

  /// 原子写资源文件（同名内容寻址文件可覆盖，内容相同）
  #[allow(dead_code)] // 批 3（D2 图片抽取）接入后启用
  fn write_asset(&self, sha: &str, ext: &str, bytes: &[u8]) -> Result<(), String>;

  /// 写手动导出文件到 `<data_dir>/backups/<file_name>`，返回绝对路径（目录自动建）
  #[allow(dead_code)] // 批 3（D3 手动导出）接入后启用
  fn write_export_file(&self, file_name: &str, text: &str) -> Result<String, String>;

  /// 建 `<data_dir>/backups/`（打开导出目录前调用）
  #[allow(dead_code)] // 批 3（D3 手动导出）接入后启用
  fn create_backups_dir(&self) -> Result<(), String>;

  /// 旧格式 notes.json 永久存档 `notes.v1.json` 是否存在（存在则不再写，保留最早那份）
  fn notes_v1_archive_exists(&self) -> bool;

  /// 原子写 `notes.v1.json`（迁移前旧格式原文，永久保留）
  fn write_notes_v1_archive(&self, json: &str) -> Result<(), String>;

  /// 读 `notes.v1.json`；None = 不存在（降级/诊断用）
  #[allow(dead_code)] // 批 3（回滚/诊断）接入后启用
  fn read_notes_v1_archive(&self) -> Result<Option<String>, String>;
}

#[cfg(test)]
pub mod test_double {
  use super::{BackupInfo, KbBackupInfo, StoreBackend};
  use std::cell::RefCell;

  /// 内存后端：一个 `RefCell<String>` 就是整个"磁盘"。
  /// 磁盘满 / 只读目录这类故障注入 `fail_writes` 后再验证写路径的报错文案。
  pub struct InMemoryBackend {
    pub data: RefCell<Option<String>>,
    pub archive: RefCell<Option<String>>,
    pub runtime: RefCell<Option<String>>,
    pub notes: RefCell<Option<String>>,
    pub notes_backups: RefCell<Vec<String>>,
    pub folders: RefCell<Option<String>>,
    pub pre_split: RefCell<Option<String>>,
    pub backups: RefCell<Vec<String>>,
    /// notes/<id>.md 正文（kb-storage-backup D1）
    pub note_bodies: RefCell<std::collections::HashMap<String, String>>,
    /// assets/<sha>.<ext> 资源字节（D2）
    pub assets: RefCell<std::collections::HashMap<String, Vec<u8>>>,
    /// backups/<file_name> 导出文件（D3 手动导出）
    pub exports: RefCell<Vec<(String, String)>>,
    /// notes.v1.json 旧格式永久存档（D7）
    pub notes_v1_archive: RefCell<Option<String>>,
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
        notes_backups: RefCell::new(Vec::new()),
        folders: RefCell::new(None),
        pre_split: RefCell::new(None),
        backups: RefCell::new(Vec::new()),
        note_bodies: RefCell::new(std::collections::HashMap::new()),
        assets: RefCell::new(std::collections::HashMap::new()),
        exports: RefCell::new(Vec::new()),
        notes_v1_archive: RefCell::new(None),
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

    fn rotate_notes_backups(&self) -> Result<(), String> {
      self.note("rotate_notes_backups");
      if *self.fail_writes.borrow() {
        return Err("备份轮转失败".to_string());
      }
      let mut slots = self.notes_backups.borrow_mut();
      let current = self.notes.borrow().clone();
      if let Some(previous) = current {
        slots.insert(0, previous);
      }
      slots.truncate(5);
      Ok(())
    }

    fn list_kb_backups(&self) -> Vec<KbBackupInfo> {
      self
        .notes_backups
        .borrow()
        .iter()
        .enumerate()
        .map(|(index, json)| {
          let parsed = serde_json::from_str::<Vec<serde_json::Value>>(json);
          KbBackupInfo {
            slot: (index + 1).to_string(),
            name: format!("notes.json.bak-{}", index + 1),
            created_at: "测试时间".to_string(),
            size_kb: (json.len() as u64 + 1023) / 1024,
            note_count: parsed.as_ref().map(|v| v.len() as u32).unwrap_or(0),
            readable: parsed.is_ok(),
            error: parsed
              .err()
              .map(|_| "备份内容无法解析".to_string()),
          }
        })
        .collect()
    }

    fn read_notes_backup(&self, slot: u32) -> Result<String, String> {
      if slot == 0 {
        return Err("备份编号从 1 开始".to_string());
      }
      let slots = self.notes_backups.borrow();
      slots
        .get(slot as usize - 1)
        .cloned()
        .ok_or_else(|| format!("备份 bak-{} 不存在", slot))
    }

    fn read_folders(&self) -> Result<Option<String>, String> {
      Ok(self.folders.borrow().clone())
    }

    fn write_folders(&self, json: &str) -> Result<(), String> {
      self.note("write_folders");
      if *self.fail_writes.borrow() {
        return Err("没有写入权限".to_string());
      }
      *self.folders.borrow_mut() = Some(json.to_string());
      Ok(())
    }

    fn read_note_body(&self, id: &str) -> Result<Option<String>, String> {
      Ok(self.note_bodies.borrow().get(id).cloned())
    }

    fn write_note_body(&self, id: &str, body: &str) -> Result<(), String> {
      self.note("write_note_body");
      if *self.fail_writes.borrow() {
        return Err("没有写入权限".to_string());
      }
      self
        .note_bodies
        .borrow_mut()
        .insert(id.to_string(), body.to_string());
      Ok(())
    }

    fn delete_note_body(&self, id: &str) -> Result<(), String> {
      self.note("delete_note_body");
      if *self.fail_writes.borrow() {
        return Err("没有写入权限".to_string());
      }
      self.note_bodies.borrow_mut().remove(id);
      Ok(())
    }

    fn asset_exists(&self, sha: &str, ext: &str) -> bool {
      self.assets.borrow().contains_key(&format!("{sha}.{ext}"))
    }

    fn read_asset(&self, sha: &str, ext: &str) -> Result<Vec<u8>, String> {
      self
        .assets
        .borrow()
        .get(&format!("{sha}.{ext}"))
        .cloned()
        .ok_or_else(|| "资源文件不存在".to_string())
    }

    fn write_asset(&self, sha: &str, ext: &str, bytes: &[u8]) -> Result<(), String> {
      self.note("write_asset");
      if *self.fail_writes.borrow() {
        return Err("没有写入权限".to_string());
      }
      self
        .assets
        .borrow_mut()
        .insert(format!("{sha}.{ext}"), bytes.to_vec());
      Ok(())
    }

    fn write_export_file(&self, file_name: &str, text: &str) -> Result<String, String> {
      self.note("write_export_file");
      if *self.fail_writes.borrow() {
        return Err("没有写入权限".to_string());
      }
      self
        .exports
        .borrow_mut()
        .push((file_name.to_string(), text.to_string()));
      // 测试替身不落盘，但返回形态与真实实现一致（绝对路径 = 临时根 + backups/ + 文件名）
      Ok(
        std::env::temp_dir()
          .join("kettd-memory-backups")
          .join(file_name)
          .to_string_lossy()
          .into_owned(),
      )
    }

    fn create_backups_dir(&self) -> Result<(), String> {
      self.note("create_backups_dir");
      if *self.fail_writes.borrow() {
        return Err("没有写入权限".to_string());
      }
      Ok(())
    }

    fn notes_v1_archive_exists(&self) -> bool {
      self.notes_v1_archive.borrow().is_some()
    }

    fn write_notes_v1_archive(&self, json: &str) -> Result<(), String> {
      self.note("write_notes_v1_archive");
      if *self.fail_writes.borrow() {
        return Err("没有写入权限".to_string());
      }
      *self.notes_v1_archive.borrow_mut() = Some(json.to_string());
      Ok(())
    }

    fn read_notes_v1_archive(&self) -> Result<Option<String>, String> {
      Ok(self.notes_v1_archive.borrow().clone())
    }
  }
}

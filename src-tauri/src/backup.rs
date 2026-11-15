#![allow(dead_code)]

//! 备份导入的增量合并计划器（kb-storage-backup 1.2）。
//!
//! 边界：
//! - **纯函数**：只吃内存数据，不碰 IO、不读文件；落盘由后续 `import_backup` 消费计划完成。
//! - 时间一律本地语义字符串（`YYYY-MM-DDTHH:mm`），可直接按字典序比较，禁止 UTC 换算。
//! - 计划**只覆盖备份里出现的条目**：本地独有条目永远不会被删除，因此不会产生动作。
//!
//! 合并规则（design D4）：
//! 1. 本地没有该 id → [`MergeAction::Add`]；
//! 2. 两边都有：备份 `updatedAt` 更新 → [`MergeAction::Update`]（备份覆盖本地）；
//! 3. 本地 `updatedAt` 更新 / 备份更旧 → [`MergeAction::Skip`]（本地优先，绝不回退）；
//! 4. `updatedAt` 相同（或双方都没有时间）→ 比内容指纹：相同 `Skip`、不同 `Conflict`；
//! 5. 指纹缺失无法比较 → `Skip`（保守：无证据不覆盖本地）；
//! 6. 提醒的冲突由调用方按「保留本地、跳过备份」处理（避免双提醒）。

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::digest::{base64_decode, base64_encode, sha256_hex};
use crate::kb_errors;
use crate::models::{new_id, AppData, KbFolder, KbItem, Reminder, Task};
use crate::store_notes;

/// 单条目的合并动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeAction {
  /// 本地没有，导入新增
  Add,
  /// 备份较新，覆盖本地
  Update,
  /// 本地不旧于备份（或无法判断），保留本地
  Skip,
  /// 时间相同但内容不同，需另存「（导入冲突）」副本
  Conflict,
}

/// 单条目动作；`id` 同时指代本地与备份里的同一条目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeEntry {
  pub id: String,
  pub action: MergeAction,
}

/// 一个实体的完整合并计划：条目动作列表 + 各动作计数。
///
/// 字段与 [`MergeEntry`] 一起可直接被 `import_backup` 消费：按 `entries` 顺序遍历，
/// 用 `id` 在本地 / 备份载荷里双向查表；`Add/Update` 写入、`Skip` 不动、`Conflict` 生成副本。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergePlan {
  /// 与备份条目同序的动作列表
  pub entries: Vec<MergeEntry>,
  pub add: usize,
  pub update: usize,
  pub skip: usize,
  pub conflict: usize,
}

impl MergePlan {
  /// 条目总数（= 备份条目数）
  pub fn total(&self) -> usize {
    self.entries.len()
  }

  /// 指定动作的条目数
  pub fn count(&self, action: MergeAction) -> usize {
    match action {
      MergeAction::Add => self.add,
      MergeAction::Update => self.update,
      MergeAction::Skip => self.skip,
      MergeAction::Conflict => self.conflict,
    }
  }

  fn from_entries(entries: Vec<MergeEntry>) -> Self {
    let mut plan = MergePlan {
      entries,
      ..Default::default()
    };
    for entry in &plan.entries {
      match entry.action {
        MergeAction::Add => plan.add += 1,
        MergeAction::Update => plan.update += 1,
        MergeAction::Skip => plan.skip += 1,
        MergeAction::Conflict => plan.conflict += 1,
      }
    }
    plan
  }
}

/// 统一比较输入：本地 / 备份各准备一份。
///
/// - `updated_at`：本地语义时间字符串；任务、知识条目有，提醒、文件夹无（传 `None`）。
/// - `fingerprint`：内容指纹（序列化 JSON 或规范化字段串的 SHA-256 hex）；`None` = 未知。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeCandidate {
  pub id: String,
  pub updated_at: Option<String>,
  pub fingerprint: Option<String>,
}

impl MergeCandidate {
  pub fn new(id: &str, updated_at: Option<&str>, fingerprint: Option<&str>) -> Self {
    Self {
      id: id.to_string(),
      updated_at: updated_at.map(str::to_string),
      fingerprint: fingerprint.map(str::to_string),
    }
  }
}

/// 通用比较器：四个实体计划器共用；`backup` 顺序即计划顺序。
pub fn plan_merge(local: &[MergeCandidate], backup: &[MergeCandidate]) -> MergePlan {
  let local_by_id: HashMap<&str, &MergeCandidate> =
    local.iter().map(|item| (item.id.as_str(), item)).collect();
  let entries = backup
    .iter()
    .map(|candidate| MergeEntry {
      id: candidate.id.clone(),
      action: decide(local_by_id.get(candidate.id.as_str()).copied(), candidate),
    })
    .collect();
  MergePlan::from_entries(entries)
}

/// 知识条目计划器。
/// `updated_at` = `KbItem.updatedAt`；`fingerprint` 由调用方用内容指纹
/// （`contentHash` + 标题/标签/folderId 的规范化 JSON 摘要）填充。
pub fn plan_kb(local: &[MergeCandidate], backup: &[MergeCandidate]) -> MergePlan {
  plan_merge(local, backup)
}

/// 任务计划器。
/// `updated_at` = `Task.updatedAt`；`fingerprint` = 任务序列化 JSON 的 SHA-256。
pub fn plan_tasks(local: &[MergeCandidate], backup: &[MergeCandidate]) -> MergePlan {
  plan_merge(local, backup)
}

/// 提醒计划器。`Reminder` 没有 `updatedAt`，只按 id + 指纹比较；
/// 冲突由调用方按「保留本地、跳过备份」处理。
pub fn plan_reminders(local: &[MergeCandidate], backup: &[MergeCandidate]) -> MergePlan {
  plan_merge(local, backup)
}

/// 知识库文件夹计划器。`KbFolder` 没有 `updatedAt`，按 id + 指纹
/// （名称 / parentId / pinned 等规范化字段串）比较。
pub fn plan_folders(local: &[MergeCandidate], backup: &[MergeCandidate]) -> MergePlan {
  plan_merge(local, backup)
}

/// 单条目决策，规则见模块文档。
fn decide(local: Option<&MergeCandidate>, backup: &MergeCandidate) -> MergeAction {
  let Some(local) = local else {
    return MergeAction::Add;
  };
  // 时间为主：只有两边都是字符串时才作比较（本地语义字符串可直接字典序比较）
  if let (Some(local_stamp), Some(backup_stamp)) =
    (local.updated_at.as_deref(), backup.updated_at.as_deref())
  {
    if backup_stamp > local_stamp {
      return MergeAction::Update;
    }
    if backup_stamp < local_stamp {
      return MergeAction::Skip;
    }
  }
  // 时间相同 / 任一侧无时间：指纹兜底；任一侧未知时保守跳过
  match (local.fingerprint.as_deref(), backup.fingerprint.as_deref()) {
    (Some(local_fp), Some(backup_fp)) if local_fp == backup_fp => MergeAction::Skip,
    (Some(_), Some(_)) => MergeAction::Conflict,
    _ => MergeAction::Skip,
  }
}

// ---------------------------------------------------------------- .kettdbackup v1（D3）
//
// 本段是备份包的文件契约与纯函数实现：序列化结构、导入校验、导出组装。
// 磁盘 IO 与内存 Store 的落盘顺序由 `store.rs` 的 `export_inputs` /
// `write_export_payload` / `run_import` 负责，这里不碰任何文件系统。

/// 备份包格式标识（`format` 字段）。
pub const BACKUP_FORMAT: &str = "kettd-backup";
/// 当前应用支持的备份包版本（`formatVersion` 字段）。
pub const BACKUP_FORMAT_VERSION: u32 = 1;
/// 备份文件统一前缀 / 后缀：`kettd-backup-<YYYYMMDD-HHmmss>.kettdbackup`。
pub const BACKUP_FILE_PREFIX: &str = "kettd-backup-";
pub const BACKUP_FILE_SUFFIX: &str = ".kettdbackup";
/// 导入 payload 文本上限：256MB（design D3 的体积上限，前端先拦一次、后端兜底）。
pub const BACKUP_MAX_TEXT_BYTES: usize = 256 * 1024 * 1024;
/// 单个图片资源解码后的字节上限：5MB。
pub const BACKUP_MAX_ASSET_BYTES: usize = 5 * 1024 * 1024;
/// 单次导入的资源数量上限 / 条目数量上限（防内存放大）。
pub const BACKUP_MAX_ASSETS: usize = 5_000;
pub const BACKUP_MAX_ENTRIES: usize = 20_000;
/// 允许的图片扩展名（与 `kb.rs` 的 MIME 白名单一致）。
pub const BACKUP_ASSET_EXTENSIONS: [&str; 4] = ["png", "jpg", "webp", "gif"];
/// 冲突副本的标题后缀（design D4）。
pub const CONFLICT_TITLE_SUFFIX: &str = "（导入冲突）";

fn invalid_format() -> String {
  kb_errors::BACKUP_FORMAT_INVALID.to_string()
}

fn incomplete() -> String {
  kb_errors::BACKUP_CONTENT_INCOMPLETE.to_string()
}

fn invalid_asset() -> String {
  kb_errors::BACKUP_ASSET_INVALID.to_string()
}

fn import_failed() -> String {
  kb_errors::IMPORT_FAILED.to_string()
}

/// 备份包顶层结构（`format` / `formatVersion` / `appVersion` / `exportedAt` + 三段数据）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
  pub format: String,
  pub format_version: u32,
  pub app_version: String,
  /// 本地语义时间字符串 `YYYY-MM-DDTHH:mm`，禁止 UTC。
  pub exported_at: String,
  pub data: BackupData,
  pub kb: BackupKb,
  #[serde(default)]
  pub assets: Vec<BackupAsset>,
}

/// `data` 段：设置与便签只做导出快照，导入时**不应用**（design D4）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupData {
  #[serde(default)]
  pub tasks: Vec<Task>,
  #[serde(default)]
  pub reminders: Vec<Reminder>,
  #[serde(default)]
  pub settings: Value,
  #[serde(default)]
  pub stickies: Vec<Value>,
}

/// `kb` 段：知识条目（含正文）与文件夹。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupKb {
  #[serde(default)]
  pub items: Vec<KbItem>,
  #[serde(default)]
  pub folders: Vec<BackupFolder>,
}

/// 备份里的文件夹：`order` 在磁盘格式里是必填 i64，备份格式放宽为可选以容旧 /
/// 容其他版本导出（缺失时按密集排序兜底）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFolder {
  pub id: String,
  #[serde(default)]
  pub name: String,
  #[serde(default)]
  pub parent_id: Option<String>,
  #[serde(default)]
  pub order: Option<i64>,
  #[serde(default)]
  pub pinned: bool,
}

impl From<&KbFolder> for BackupFolder {
  fn from(folder: &KbFolder) -> Self {
    Self {
      id: folder.id.clone(),
      name: folder.name.clone(),
      parent_id: folder.parent_id.clone(),
      order: Some(folder.order),
      pinned: folder.pinned,
    }
  }
}

/// 备份里的单个图片资源：sha256 + 扩展名 + base64 字节。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupAsset {
  pub sha256: String,
  pub ext: String,
  pub base64: String,
}

/// 校验并解码后的资源；`hash_ok=false` 表示声明 sha256 与实际内容不符，
/// 导入时跳过该资源但**不阻断**整包（正文照常导入）。
#[derive(Debug, Clone)]
pub struct ParsedAsset {
  pub sha256: String,
  pub ext: String,
  pub bytes: Vec<u8>,
  pub hash_ok: bool,
}

impl ParsedAsset {
  pub fn is_reusable(&self) -> bool {
    self.hash_ok
  }
}

/// 解析完成、可直接参与计划与应用的备份包。
#[derive(Debug)]
pub struct ParsedBackup {
  pub file: BackupFile,
  /// 已解码资源（sha256 与 ext 已规范化、按 (sha256, ext) 去重）。
  assets: Vec<ParsedAsset>,
  /// 原始 JSON 快照：Update 时用于 overlay —— 备份里缺的字段保留本地值。
  raw_tasks: HashMap<String, Value>,
  raw_reminders: HashMap<String, Value>,
  raw_kb_items: HashMap<String, Value>,
  raw_folders: HashMap<String, Value>,
}

impl ParsedBackup {
  pub fn asset(&self, sha256: &str, ext: &str) -> Option<&ParsedAsset> {
    self.assets.iter().find(|asset| asset.sha256 == sha256 && asset.ext == ext)
  }

  pub fn asset_count(&self) -> usize {
    self.assets.len()
  }

  pub fn reusable_asset_count(&self) -> usize {
    self.assets.iter().filter(|asset| asset.hash_ok).count()
  }

  /// 全部解析后的资源（含 hash 校验失败项）：导入时逐个判断 hash_ok 与是否已存在。
  /// `store.rs` 需要写入「备份里全部可复用资源」而不只是正文引用的，故暴露只读切片。
  pub fn assets(&self) -> &[ParsedAsset] {
    &self.assets
  }

  fn raw_task(&self, id: &str) -> Option<&Value> {
    self.raw_tasks.get(id)
  }

  fn raw_reminder(&self, id: &str) -> Option<&Value> {
    self.raw_reminders.get(id)
  }

  fn raw_kb_item(&self, id: &str) -> Option<&Value> {
    self.raw_kb_items.get(id)
  }

  fn raw_folder(&self, id: &str) -> Option<&Value> {
    self.raw_folders.get(id)
  }
}

/// 解析并校验导入 payload。错误一律返回 `kb_errors` 里的中文短句。
pub fn parse_backup(text: &str) -> Result<ParsedBackup, String> {
  if text.len() > BACKUP_MAX_TEXT_BYTES {
    return Err(kb_errors::BACKUP_FILE_TOO_LARGE.to_string());
  }
  let root: Value = serde_json::from_str(text).map_err(|_| invalid_format())?;
  let map = root.as_object().ok_or_else(invalid_format)?;
  match map.get("format") {
    Some(Value::String(format)) if format == BACKUP_FORMAT => {}
    _ => return Err(invalid_format()),
  }
  let format_version = map
    .get("formatVersion")
    .and_then(Value::as_u64)
    .ok_or_else(incomplete)?;
  if format_version != u64::from(BACKUP_FORMAT_VERSION) {
    return Err(kb_errors::BACKUP_VERSION_UNSUPPORTED.to_string());
  }
  let app_version = map
    .get("appVersion")
    .and_then(Value::as_str)
    .unwrap_or_default()
    .to_string();
  let exported_at = map
    .get("exportedAt")
    .and_then(Value::as_str)
    .unwrap_or_default()
    .to_string();

  let data_map = map
    .get("data")
    .and_then(Value::as_object)
    .ok_or_else(incomplete)?;
  let data = BackupData {
    tasks: typed_array(data_map, "tasks")?,
    reminders: typed_array(data_map, "reminders")?,
    settings: data_map.get("settings").cloned().unwrap_or(Value::Null),
    stickies: raw_array(data_map, "stickies")?,
  };
  let kb_map = map
    .get("kb")
    .and_then(Value::as_object)
    .ok_or_else(incomplete)?;
  let kb = BackupKb {
    items: typed_array(kb_map, "items")?,
    folders: typed_array(kb_map, "folders")?,
  };
  if data.tasks.len() + data.reminders.len() + kb.items.len() + kb.folders.len()
    > BACKUP_MAX_ENTRIES
  {
    return Err(kb_errors::BACKUP_TOO_MANY_ENTRIES.to_string());
  }

  let mut raw_tasks = HashMap::new();
  for raw in raw_array(data_map, "tasks")? {
    if let Some(id) = value_id(&raw) {
      raw_tasks.entry(id.to_string()).or_insert(raw);
    }
  }
  let mut raw_reminders = HashMap::new();
  for raw in raw_array(data_map, "reminders")? {
    if let Some(id) = value_id(&raw) {
      raw_reminders.entry(id.to_string()).or_insert(raw);
    }
  }
  let mut raw_kb_items = HashMap::new();
  for raw in raw_array(kb_map, "items")? {
    if let Some(id) = value_id(&raw) {
      raw_kb_items.entry(id.to_string()).or_insert(raw);
    }
  }
  let mut raw_folders = HashMap::new();
  for raw in raw_array(kb_map, "folders")? {
    if let Some(id) = value_id(&raw) {
      raw_folders.entry(id.to_string()).or_insert(raw);
    }
  }

  let (assets, parsed_assets) = parse_assets(map)?;
  Ok(ParsedBackup {
    file: BackupFile {
      format: BACKUP_FORMAT.to_string(),
      format_version: BACKUP_FORMAT_VERSION,
      app_version,
      exported_at,
      data,
      kb,
      assets,
    },
    assets: parsed_assets,
    raw_tasks,
    raw_reminders,
    raw_kb_items,
    raw_folders,
  })
}

fn parse_assets(map: &Map<String, Value>) -> Result<(Vec<BackupAsset>, Vec<ParsedAsset>), String> {
  let raws = raw_array(map, "assets")?;
  if raws.len() > BACKUP_MAX_ASSETS {
    return Err(kb_errors::BACKUP_TOO_MANY_ENTRIES.to_string());
  }
  let mut declared = Vec::with_capacity(raws.len());
  let mut decoded: Vec<ParsedAsset> = Vec::new();
  let mut seen: HashSet<(String, String)> = HashSet::new();
  for raw in raws {
    let asset: BackupAsset = serde_json::from_value(raw).map_err(|_| invalid_asset())?;
    let sha256 = asset.sha256.trim().to_ascii_lowercase();
    if sha256.len() != 64 || !sha256.chars().all(|c| c.is_ascii_hexdigit()) {
      return Err(invalid_asset());
    }
    let ext = asset.ext.trim().to_ascii_lowercase();
    if !BACKUP_ASSET_EXTENSIONS.contains(&ext.as_str()) {
      return Err(invalid_asset());
    }
    let bytes = base64_decode(&asset.base64).map_err(|_| invalid_asset())?;
    if bytes.len() > BACKUP_MAX_ASSET_BYTES {
      return Err(invalid_asset());
    }
    let hash_ok = sha256_hex(&bytes) == sha256;
    if seen.insert((sha256.clone(), ext.clone())) {
      decoded.push(ParsedAsset {
        sha256: sha256.clone(),
        ext: ext.clone(),
        bytes,
        hash_ok,
      });
    }
    // 已解码的字节是唯一真源；声明段的 base64 不再保留，避免双倍内存占用。
    declared.push(BackupAsset {
      sha256,
      ext,
      base64: String::new(),
    });
  }
  Ok((declared, decoded))
}

fn raw_array(map: &Map<String, Value>, key: &str) -> Result<Vec<Value>, String> {
  match map.get(key) {
    None | Some(Value::Null) => Ok(Vec::new()),
    Some(Value::Array(items)) => Ok(items.clone()),
    Some(_) => Err(incomplete()),
  }
}

fn typed_array<T: serde::de::DeserializeOwned>(
  map: &Map<String, Value>,
  key: &str,
) -> Result<Vec<T>, String> {
  raw_array(map, key)?
    .into_iter()
    .map(|raw| serde_json::from_value(raw).map_err(|_| incomplete()))
    .collect()
}

fn value_id(raw: &Value) -> Option<&str> {
  raw
    .as_object()
    .and_then(|object| object.get("id"))
    .and_then(Value::as_str)
    .map(str::trim)
    .filter(|id| !id.is_empty())
}

/// 导出组装：把当前 Store 快照转成备份包结构（纯函数，不写盘）。
pub fn build_backup(app_version: &str, exported_at: &str, inputs: ExportInputs) -> BackupFile {
  let ExportInputs {
    data,
    kb,
    folders,
    assets,
    skipped: _,
  } = inputs;
  let settings = serde_json::to_value(&data.settings).unwrap_or(Value::Null);
  let stickies = data
    .stickies
    .iter()
    .filter_map(|sticky| serde_json::to_value(sticky).ok())
    .collect();
  let kb_items = kb
    .into_iter()
    .map(|mut item| {
      // 导出时按实际正文重算 contentHash，避免内存里的旧 hash 污染备份。
      item.content_hash = Some(store_notes::content_hash(&item.body_md));
      item
    })
    .collect();
  let backup_assets = assets
    .into_iter()
    .map(|asset| BackupAsset {
      sha256: asset.sha256,
      ext: asset.ext,
      base64: base64_encode(&asset.bytes),
    })
    .collect();
  BackupFile {
    format: BACKUP_FORMAT.to_string(),
    format_version: BACKUP_FORMAT_VERSION,
    app_version: app_version.to_string(),
    exported_at: exported_at.to_string(),
    data: BackupData {
      tasks: data.tasks,
      reminders: data.reminders,
      settings,
      stickies,
    },
    kb: BackupKb {
      items: kb_items,
      folders: folders.iter().map(BackupFolder::from).collect(),
    },
    assets: backup_assets,
  }
}

/// 序列化备份包文本（紧凑 JSON，减少 IPC 体积）。
pub fn export_payload(file: &BackupFile) -> Result<String, String> {
  serde_json::to_string(file).map_err(|_| kb_errors::EXPORT_FAILED.to_string())
}

/// 备份文件名：`kettd-backup-<YYYYMMDD-HHmmss>.kettdbackup`（同秒去重由上层加序号）。
pub fn backup_file_name(stamp: &str) -> String {
  format!("{BACKUP_FILE_PREFIX}{stamp}{BACKUP_FILE_SUFFIX}")
}

/// 扫描一段正文里引用的 `assets/<sha256>.<ext>`（去重保序，白名单扩展名）。
pub fn asset_refs_in_body(body: &str) -> Vec<(String, String)> {
  const MARKER: &str = "assets/";
  let bytes = body.as_bytes();
  let mut refs = Vec::new();
  let mut seen: HashSet<(String, String)> = HashSet::new();
  let mut cursor = 0usize;
  while let Some(found) = body[cursor..].find(MARKER) {
    let start = cursor + found;
    let mut index = start + MARKER.len();
    let mut sha = String::new();
    while index < bytes.len() && sha.len() < 64 && bytes[index].is_ascii_hexdigit() {
      sha.push(bytes[index] as char);
      index += 1;
    }
    let matched = if sha.len() == 64 && index < bytes.len() && bytes[index] == b'.' {
      let ext_start = index + 1;
      let mut end = ext_start;
      while end < bytes.len() && bytes[end].is_ascii_alphabetic() && end - ext_start < 8 {
        end += 1;
      }
      let ext = &body[ext_start..end];
      BACKUP_ASSET_EXTENSIONS.contains(&ext).then(|| {
        let key = (sha.to_ascii_lowercase(), ext.to_string());
        if seen.insert(key.clone()) {
          refs.push(key);
        }
        end
      })
    } else {
      None
    };
    cursor = matched.unwrap_or(start + 1);
  }
  refs
}

/// 收集全部知识条目正文引用的资源（(sha256, ext) 去重保序）。
pub fn collect_asset_refs(items: &[KbItem]) -> Vec<(String, String)> {
  let mut seen: HashSet<(String, String)> = HashSet::new();
  let mut refs = Vec::new();
  for item in items {
    for (sha256, ext) in asset_refs_in_body(&item.body_md) {
      if seen.insert((sha256.clone(), ext.clone())) {
        refs.push((sha256, ext));
      }
    }
  }
  refs
}

// ---------------------------------------------------------------- 内容指纹

fn serialized_fingerprint<T: Serialize>(value: &T) -> Option<String> {
  serde_json::to_string(value)
    .ok()
    .map(|text| sha256_hex(text.as_bytes()))
}

/// 任务指纹：完整序列化 JSON 的 sha256（两边同版本序列化时可比）。
pub fn task_fingerprint(task: &Task) -> Option<String> {
  serialized_fingerprint(task)
}

/// 提醒指纹：完整序列化 JSON 的 sha256（提醒没有 updatedAt，指纹是唯一兜底）。
pub fn reminder_fingerprint(reminder: &Reminder) -> Option<String> {
  serialized_fingerprint(reminder)
}

/// 知识条目指纹：contentHash + 标题 + 排序去重标签 + folderId 的规范化 JSON 的 sha256。
pub fn kb_fingerprint(item: &KbItem) -> String {
  let mut tags: Vec<&str> = item
    .tags
    .iter()
    .map(String::as_str)
    .map(str::trim)
    .filter(|tag| !tag.is_empty())
    .collect();
  tags.sort_unstable();
  tags.dedup();
  let canonical = json!({
    "contentHash": store_notes::content_hash(&item.body_md),
    "title": item.title.trim(),
    "tags": tags,
    "folderId": &item.folder_id,
  });
  sha256_hex(canonical.to_string().as_bytes())
}

/// 文件夹指纹：备份形态（order 可选）序列化 JSON 的 sha256。
pub fn folder_fingerprint(folder: &KbFolder) -> String {
  let canonical = serde_json::to_string(&BackupFolder::from(folder)).unwrap_or_default();
  sha256_hex(canonical.as_bytes())
}

// ---------------------------------------------------------------- 导入计划与应用（D4）
//
// 计划只覆盖备份里出现的条目：本地独有条目不会被删除；备份缺字段时保留本地值
// （通过 raw JSON overlay 实现）。应用阶段产出「目标状态」，不直接写盘。

/// 导出组装所需的 Store 快照；由 `store.rs` 填充（资源已读成字节）。
pub struct ExportInputs {
  pub data: AppData,
  pub kb: Vec<KbItem>,
  pub folders: Vec<KbFolder>,
  pub assets: Vec<ExportAsset>,
  /// 组装时被跳过的资源数（缺失 / 读失败），不阻断导出。
  pub skipped: usize,
}

/// 待写入备份包的资源（已完成读取）。
pub struct ExportAsset {
  pub sha256: String,
  pub ext: String,
  pub bytes: Vec<u8>,
}

/// 导出结果（命令返回值，camelCase 与 `api.ts` 对齐）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
  pub path: String,
  pub bytes: usize,
  pub item_count: usize,
  pub asset_count: usize,
  pub skipped: usize,
}

/// 导入报告（命令返回值，camelCase 与 `api.ts` 对齐）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
  /// 新增条目（任务 / 提醒 / 知识条目 / 文件夹合计，含冲突副本）。
  pub added: usize,
  /// 备份较新、覆盖本地。
  pub updated: usize,
  /// 冲突条目数（本地保留 + 生成副本）。
  pub conflicts: usize,
  /// 跳过条目数（含本地较新、指纹相同、提醒冲突、资源校验失败等）。
  pub skipped: usize,
  pub asset_added: usize,
  pub asset_reused: usize,
  pub conflict_titles: Vec<String>,
  pub dry_run: bool,
}

/// 本地当前状态快照（只读）。
#[derive(Debug, Clone)]
pub struct LocalSnapshot {
  pub tasks: Vec<Task>,
  pub reminders: Vec<Reminder>,
  pub kb: Vec<KbItem>,
  pub folders: Vec<KbFolder>,
}

impl LocalSnapshot {
  pub fn new(
    tasks: Vec<Task>,
    reminders: Vec<Reminder>,
    kb: Vec<KbItem>,
    folders: Vec<KbFolder>,
  ) -> Self {
    Self {
      tasks,
      reminders,
      kb,
      folders,
    }
  }
}

/// 四类实体的合并计划。
pub struct PlanBundle {
  pub tasks: MergePlan,
  pub reminders: MergePlan,
  pub kb: MergePlan,
  pub folders: MergePlan,
}

impl PlanBundle {
  /// 计划条目总数。
  pub fn total(&self) -> usize {
    self.tasks.total() + self.reminders.total() + self.kb.total() + self.folders.total()
  }

  /// 需要落盘的动作数（新增 + 覆盖 + 冲突副本）。
  pub fn changed(&self) -> usize {
    self.tasks.add
      + self.tasks.update
      + self.tasks.conflict
      + self.reminders.add
      + self.reminders.update
      + self.reminders.conflict
      + self.kb.add
      + self.kb.update
      + self.kb.conflict
      + self.folders.add
      + self.folders.update
      + self.folders.conflict
  }
}

fn updated_at_opt(value: &str) -> Option<&str> {
  let trimmed = value.trim();
  if trimmed.is_empty() {
    None
  } else {
    Some(trimmed)
  }
}

fn task_candidates(tasks: &[Task]) -> Vec<MergeCandidate> {
  tasks
    .iter()
    .map(|task| {
      MergeCandidate::new(
        &task.id,
        updated_at_opt(&task.updated_at),
        task_fingerprint(task).as_deref(),
      )
    })
    .collect()
}

fn reminder_candidates(reminders: &[Reminder]) -> Vec<MergeCandidate> {
  reminders
    .iter()
    .map(|reminder| {
      MergeCandidate::new(
        &reminder.id,
        None,
        reminder_fingerprint(reminder).as_deref(),
      )
    })
    .collect()
}

fn kb_candidates(items: &[KbItem]) -> Vec<MergeCandidate> {
  items
    .iter()
    .map(|item| {
      MergeCandidate::new(
        &item.id,
        updated_at_opt(&item.updated_at),
        Some(&kb_fingerprint(item)),
      )
    })
    .collect()
}

fn folder_candidates(folders: &[KbFolder]) -> Vec<MergeCandidate> {
  folders
    .iter()
    .map(|folder| {
      let fingerprint = folder_fingerprint(folder);
      MergeCandidate::new(&folder.id, None, Some(&fingerprint))
    })
    .collect()
}

/// 备份形态文件夹的合并候选：指纹与本地 `folder_fingerprint` 同口径
/// （同样对 `BackupFolder` 的规范化 JSON 取 sha256），保证可比较。
fn backup_folder_candidates(folders: &[BackupFolder]) -> Vec<MergeCandidate> {
  folders
    .iter()
    .map(|folder| {
      let canonical = serde_json::to_string(folder).unwrap_or_default();
      let fingerprint = sha256_hex(canonical.as_bytes());
      MergeCandidate::new(&folder.id, None, Some(&fingerprint))
    })
    .collect()
}

fn skip_conflict_actions(plan: MergePlan) -> MergePlan {
  let entries = plan
    .entries
    .into_iter()
    .map(|mut entry| {
      if entry.action == MergeAction::Conflict {
        entry.action = MergeAction::Skip;
      }
      entry
    })
    .collect();
  MergePlan::from_entries(entries)
}

/// 重复导入判定（任务）：本地已有标题为「原标题（导入冲突）」的同名条目时，
/// 说明上一次导入已经生成过副本，不再重复生成（幂等）。
fn task_conflict_already_applied(local: &LocalSnapshot, backup: &ParsedBackup, id: &str) -> bool {
  backup
    .file
    .data
    .tasks
    .iter()
    .find(|task| task.id == id)
    .map(|task| {
      let expected = conflict_title(&task.title);
      local.tasks.iter().any(|local_task| local_task.title == expected)
    })
    .unwrap_or(false)
}

/// 重复导入判定（知识条目）：标题相同且正文 hash 相同才算已生成过副本。
fn kb_conflict_already_applied(local: &LocalSnapshot, backup: &ParsedBackup, id: &str) -> bool {
  backup
    .file
    .kb
    .items
    .iter()
    .find(|item| item.id == id)
    .map(|item| {
      let expected_title = conflict_title(&item.title);
      let expected_hash = store_notes::content_hash(&item.body_md);
      local.kb.iter().any(|local_item| {
        local_item.title == expected_title
          && store_notes::content_hash(&local_item.body_md) == expected_hash
      })
    })
    .unwrap_or(false)
}

fn downgrade_task_conflicts(
  local: &LocalSnapshot,
  backup: &ParsedBackup,
  plan: MergePlan,
) -> MergePlan {
  let entries = plan
    .entries
    .into_iter()
    .map(|mut entry| {
      if entry.action == MergeAction::Conflict
        && task_conflict_already_applied(local, backup, &entry.id)
      {
        entry.action = MergeAction::Skip;
      }
      entry
    })
    .collect();
  MergePlan::from_entries(entries)
}

fn downgrade_kb_conflicts(
  local: &LocalSnapshot,
  backup: &ParsedBackup,
  plan: MergePlan,
) -> MergePlan {
  let entries = plan
    .entries
    .into_iter()
    .map(|mut entry| {
      if entry.action == MergeAction::Conflict
        && kb_conflict_already_applied(local, backup, &entry.id)
      {
        entry.action = MergeAction::Skip;
      }
      entry
    })
    .collect();
  MergePlan::from_entries(entries)
}

/// 计算导入计划（纯函数，不写盘）。提醒与文件夹的冲突按 design D4 保留本地 → Skip。
pub fn plan_import(local: &LocalSnapshot, backup: &ParsedBackup) -> PlanBundle {
  let tasks = downgrade_task_conflicts(
    local,
    backup,
    plan_tasks(&task_candidates(&local.tasks), &task_candidates(&backup.file.data.tasks)),
  );
  let reminders = skip_conflict_actions(plan_reminders(
    &reminder_candidates(&local.reminders),
    &reminder_candidates(&backup.file.data.reminders),
  ));
  let kb = downgrade_kb_conflicts(
    local,
    backup,
    plan_kb(&kb_candidates(&local.kb), &kb_candidates(&backup.file.kb.items)),
  );
  let folders = skip_conflict_actions(plan_folders(
    &folder_candidates(&local.folders),
    &backup_folder_candidates(&backup.file.kb.folders),
  ));
  PlanBundle {
    tasks,
    reminders,
    kb,
    folders,
  }
}

/// 冲突条目标题清单（按本地条目顺序：任务在前、知识条目在后）。
pub fn conflict_titles(local: &LocalSnapshot, plan: &PlanBundle) -> Vec<String> {
  let mut titles = Vec::new();
  for entry in &plan.tasks.entries {
    if entry.action == MergeAction::Conflict {
      if let Some(task) = local.tasks.iter().find(|task| task.id == entry.id) {
        titles.push(task.title.clone());
      }
    }
  }
  for entry in &plan.kb.entries {
    if entry.action == MergeAction::Conflict {
      if let Some(item) = local.kb.iter().find(|item| item.id == entry.id) {
        titles.push(item.title.clone());
      }
    }
  }
  titles
}

/// 应用计划后的「目标状态」；`body_writes` 是需要写 `notes/<id>.md` 的 (id, 正文)。
pub struct ApplyPlan {
  pub tasks: Vec<Task>,
  pub reminders: Vec<Reminder>,
  pub kb: Vec<KbItem>,
  pub folders: Vec<KbFolder>,
  pub body_writes: Vec<(String, String)>,
  /// 本次生成的冲突副本数（任务 + 知识条目）。
  pub conflict_copies: usize,
}

/// 按合并计划把备份内容套到本地快照上（纯函数，不写盘）。
pub fn apply_import_plan(
  local: &LocalSnapshot,
  backup: &ParsedBackup,
  plan: &PlanBundle,
  stamp: &str,
) -> Result<ApplyPlan, String> {
  // ---- 文件夹：Add / Update 后统一做父级可行性校验与密集排序
  let mut folders = local.folders.clone();
  for entry in &plan.folders.entries {
    match entry.action {
      MergeAction::Add => {
        let folder = backup
          .file
          .kb
          .folders
          .iter()
          .find(|folder| folder.id == entry.id)
          .ok_or_else(incomplete)?;
        folders.push(KbFolder {
          id: folder.id.clone(),
          name: folder_name(&folder.name),
          parent_id: folder.parent_id.clone(),
          order: folder.order.unwrap_or(i64::MAX),
          pinned: folder.pinned,
        });
      }
      MergeAction::Update => {
        let raw = backup.raw_folder(&entry.id).ok_or_else(incomplete)?;
        let index = folders
          .iter()
          .position(|folder| folder.id == entry.id)
          .ok_or_else(incomplete)?;
        let base = serde_json::to_value(&folders[index]).map_err(|_| import_failed())?;
        let merged = overlay(base, raw);
        let mut folder: KbFolder =
          serde_json::from_value(merged).map_err(|_| incomplete())?;
        folder.id = entry.id.clone();
        folder.name = folder_name(&folder.name);
        folders[index] = folder;
      }
      _ => {}
    }
  }
  resolve_folder_parents(&mut folders);
  let folder_order = order_folder_ids(&folders);
  let folder_ranks: HashMap<String, i64> = folder_order
    .iter()
    .enumerate()
    .map(|(index, id)| (id.clone(), index as i64))
    .collect();
  for folder in &mut folders {
    folder.order = folder_ranks.get(&folder.id).copied().unwrap_or(0);
  }

  // ---- 知识条目：Add / Update / Conflict 副本
  let mut kb = local.kb.clone();
  let mut body_writes: Vec<(String, String)> = Vec::new();
  let mut conflict_copies = 0usize;
  for entry in &plan.kb.entries {
    match entry.action {
      MergeAction::Add => {
        let raw = backup.raw_kb_item(&entry.id).ok_or_else(incomplete)?;
        let item = kb_item_from_value(raw, stamp)?;
        body_writes.push((item.id.clone(), item.body_md.clone()));
        kb.push(item);
      }
      MergeAction::Update => {
        let index = kb
          .iter()
          .position(|item| item.id == entry.id)
          .ok_or_else(incomplete)?;
        let raw = backup.raw_kb_item(&entry.id).ok_or_else(incomplete)?;
        let base = serde_json::to_value(&kb[index]).map_err(|_| import_failed())?;
        let merged = overlay(base, raw);
        let mut item: KbItem = serde_json::from_value(merged).map_err(|_| incomplete())?;
        item.id = entry.id.clone();
        item.title = title_or_default(&item.title, "未命名条目");
        item.content_hash = Some(store_notes::content_hash(&item.body_md));
        body_writes.push((item.id.clone(), item.body_md.clone()));
        kb[index] = item;
      }
      MergeAction::Conflict => {
        let raw = backup.raw_kb_item(&entry.id).ok_or_else(incomplete)?;
        let mut copy = kb_item_from_value(raw, stamp)?;
        let local_folder = local
          .kb
          .iter()
          .find(|item| item.id == entry.id)
          .and_then(|item| item.folder_id.clone());
        copy.id = new_id("k");
        copy.title = conflict_title(&copy.title);
        copy.updated_at = stamp.to_string();
        // 冲突副本跟随本地条目所在文件夹（design D4）。
        copy.folder_id = local_folder;
        copy.order = None;
        body_writes.push((copy.id.clone(), copy.body_md.clone()));
        kb.push(copy);
        conflict_copies += 1;
      }
      MergeAction::Skip => {}
    }
  }
  // 知识条目引用的文件夹必须存在，否则退回未分类（不阻断导入）。
  let folder_ids: HashSet<String> = folders.iter().map(|folder| folder.id.clone()).collect();
  for item in &mut kb {
    if let Some(folder) = &item.folder_id {
      if !folder_ids.contains(folder) {
        item.folder_id = None;
      }
    }
  }
  let kb_order = order_kb_ids(&kb);
  let kb_ranks: HashMap<String, i64> = kb_order
    .iter()
    .enumerate()
    .map(|(index, id)| (id.clone(), index as i64))
    .collect();
  for item in &mut kb {
    item.order = kb_ranks.get(&item.id).copied().map(Some).unwrap_or(None);
  }

  // ---- 任务：Add / Update / Conflict 副本
  let mut tasks = local.tasks.clone();
  for entry in &plan.tasks.entries {
    match entry.action {
      MergeAction::Add => {
        let raw = backup.raw_task(&entry.id).ok_or_else(incomplete)?;
        tasks.push(task_from_value(raw, stamp)?);
      }
      MergeAction::Update => {
        let index = tasks
          .iter()
          .position(|task| task.id == entry.id)
          .ok_or_else(incomplete)?;
        let raw = backup.raw_task(&entry.id).ok_or_else(incomplete)?;
        let base = serde_json::to_value(&tasks[index]).map_err(|_| import_failed())?;
        let merged = overlay(base, raw);
        let mut task: Task = serde_json::from_value(merged).map_err(|_| incomplete())?;
        task.id = entry.id.clone();
        task.title = title_or_default(&task.title, "未命名条目");
        tasks[index] = task;
      }
      MergeAction::Conflict => {
        let raw = backup.raw_task(&entry.id).ok_or_else(incomplete)?;
        let mut copy = task_from_value(raw, stamp)?;
        copy.id = new_id("t");
        copy.title = conflict_title(&copy.title);
        copy.updated_at = stamp.to_string();
        tasks.push(copy);
        conflict_copies += 1;
      }
      MergeAction::Skip => {}
    }
  }

  // ---- 提醒：Add / Update；冲突已在计划阶段降级为 Skip（保留本地）
  let mut reminders = local.reminders.clone();
  for entry in &plan.reminders.entries {
    match entry.action {
      MergeAction::Add => {
        let raw = backup.raw_reminder(&entry.id).ok_or_else(incomplete)?;
        reminders.push(reminder_from_value(raw)?);
      }
      MergeAction::Update => {
        let index = reminders
          .iter()
          .position(|reminder| reminder.id == entry.id)
          .ok_or_else(incomplete)?;
        let raw = backup.raw_reminder(&entry.id).ok_or_else(incomplete)?;
        let base = serde_json::to_value(&reminders[index]).map_err(|_| import_failed())?;
        let merged = overlay(base, raw);
        let mut reminder: Reminder =
          serde_json::from_value(merged).map_err(|_| incomplete())?;
        reminder.id = entry.id.clone();
        reminder.title = title_or_default(&reminder.title, "未命名提醒");
        reminders[index] = reminder;
      }
      _ => {}
    }
  }

  Ok(ApplyPlan {
    tasks,
    reminders,
    kb,
    folders,
    body_writes,
    conflict_copies,
  })
}

fn overlay(base: Value, raw: &Value) -> Value {
  match (base, raw) {
    (Value::Object(mut base_map), Value::Object(raw_map)) => {
      for (key, value) in raw_map {
        base_map.insert(key.clone(), value.clone());
      }
      Value::Object(base_map)
    }
    (_, raw) => raw.clone(),
  }
}

fn title_or_default(title: &str, fallback: &str) -> String {
  let trimmed = title.trim();
  if trimmed.is_empty() {
    fallback.to_string()
  } else {
    trimmed.to_string()
  }
}

fn folder_name(name: &str) -> String {
  title_or_default(name, "未命名文件夹")
}

fn conflict_title(title: &str) -> String {
  format!("{}{}", title.trim(), CONFLICT_TITLE_SUFFIX)
}

fn import_stamp(value: &str, stamp: &str) -> String {
  let trimmed = value.trim();
  if trimmed.is_empty() {
    stamp.to_string()
  } else {
    trimmed.to_string()
  }
}

fn task_from_value(raw: &Value, stamp: &str) -> Result<Task, String> {
  let mut task: Task = serde_json::from_value(raw.clone()).map_err(|_| incomplete())?;
  task.title = title_or_default(&task.title, "未命名条目");
  task.updated_at = import_stamp(&task.updated_at, stamp);
  if task.created_at.trim().is_empty() {
    task.created_at = stamp.to_string();
  }
  Ok(task)
}

fn kb_item_from_value(raw: &Value, stamp: &str) -> Result<KbItem, String> {
  let mut item: KbItem = serde_json::from_value(raw.clone()).map_err(|_| incomplete())?;
  item.title = title_or_default(&item.title, "未命名条目");
  item.updated_at = import_stamp(&item.updated_at, stamp);
  if item.created_at.trim().is_empty() {
    item.created_at = stamp.to_string();
  }
  item.content_hash = Some(store_notes::content_hash(&item.body_md));
  Ok(item)
}

fn reminder_from_value(raw: &Value) -> Result<Reminder, String> {
  let mut reminder: Reminder = serde_json::from_value(raw.clone()).map_err(|_| incomplete())?;
  reminder.title = title_or_default(&reminder.title, "未命名提醒");
  Ok(reminder)
}

fn resolve_folder_parents(folders: &mut [KbFolder]) {
  let ids: HashSet<String> = folders.iter().map(|folder| folder.id.clone()).collect();
  for folder in folders.iter_mut() {
    if let Some(parent) = &folder.parent_id {
      if parent == &folder.id || !ids.contains(parent) {
        folder.parent_id = None;
      }
    }
  }
  // 成环保护：沿父链上溯，一旦回到已访问节点就把该节点的父级上提为根。
  for index in 0..folders.len() {
    let mut seen: HashSet<String> = HashSet::new();
    let mut cursor = folders[index].parent_id.clone();
    while let Some(parent_id) = cursor {
      if !seen.insert(parent_id.clone()) {
        folders[index].parent_id = None;
        break;
      }
      cursor = folders
        .iter()
        .find(|folder| folder.id == parent_id)
        .and_then(|folder| folder.parent_id.clone());
    }
  }
}

fn order_folder_ids(folders: &[KbFolder]) -> Vec<String> {
  let mut by_parent: HashMap<Option<&str>, Vec<&KbFolder>> = HashMap::new();
  for folder in folders {
    by_parent
      .entry(folder.parent_id.as_deref())
      .or_default()
      .push(folder);
  }
  let mut ordered = Vec::new();
  let mut visited: HashSet<String> = HashSet::new();
  let mut queue: Vec<Option<&str>> = vec![None];
  while let Some(parent) = queue.pop() {
    let mut children = by_parent.remove(&parent).unwrap_or_default();
    children.sort_by(|left, right| {
      left
        .order
        .cmp(&right.order)
        .then_with(|| left.name.cmp(&right.name))
        .then_with(|| left.id.cmp(&right.id))
    });
    for folder in children {
      if visited.insert(folder.id.clone()) {
        ordered.push(folder.id.clone());
        queue.push(Some(&folder.id));
      }
    }
  }
  for folder in folders {
    if visited.insert(folder.id.clone()) {
      ordered.push(folder.id.clone());
    }
  }
  ordered
}

fn order_kb_ids(items: &[KbItem]) -> Vec<String> {
  let mut by_folder: HashMap<Option<&str>, Vec<&KbItem>> = HashMap::new();
  for item in items {
    by_folder
      .entry(item.folder_id.as_deref())
      .or_default()
      .push(item);
  }
  let mut folders: Vec<Option<&str>> = by_folder.keys().copied().collect();
  folders.sort_by_key(|folder| folder.is_some());
  let mut ordered = Vec::new();
  let mut visited: HashSet<String> = HashSet::new();
  for folder in folders {
    let mut children = by_folder.remove(&folder).unwrap_or_default();
    children.sort_by(|left, right| {
      left
        .order
        .cmp(&right.order)
        .then_with(|| right.updated_at.cmp(&left.updated_at))
        .then_with(|| left.id.cmp(&right.id))
    });
    for item in children {
      if visited.insert(item.id.clone()) {
        ordered.push(item.id.clone());
      }
    }
  }
  for item in items {
    if visited.insert(item.id.clone()) {
      ordered.push(item.id.clone());
    }
  }
  ordered
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
  use super::*;

  // 测试断言直接比对 kb_errors 常量文本；实现侧用的是 `kb_errors::XXX` 限定名。
  use crate::kb_errors::{
    BACKUP_ASSET_INVALID, BACKUP_CONTENT_INCOMPLETE, BACKUP_FORMAT_INVALID,
    BACKUP_TOO_MANY_ENTRIES, BACKUP_VERSION_UNSUPPORTED,
  };

  fn cand(id: &str, updated_at: Option<&str>, fingerprint: Option<&str>) -> MergeCandidate {
    MergeCandidate::new(id, updated_at, fingerprint)
  }

  #[test]
  fn plan_handles_empty_inputs() {
    let empty = plan_kb(&[], &[]);
    assert_eq!(empty.total(), 0);
    assert_eq!(empty.add, 0);
    assert_eq!(empty.update, 0);
    assert_eq!(empty.skip, 0);
    assert_eq!(empty.conflict, 0);

    // 本地有、备份为空：不产生动作（本地独有条目绝不删除）
    let local = [cand("k1", Some("2026-11-01T09:00"), Some("fp1"))];
    let plan = plan_kb(&local, &[]);
    assert_eq!(plan.total(), 0, "备份空时不产生任何条目动作");
    assert!(plan.entries.is_empty());
  }

  #[test]
  fn plan_adds_items_missing_locally() {
    let local = [cand("k1", Some("2026-11-01T09:00"), Some("fp1"))];
    let backup = [
      cand("k1", Some("2026-11-01T09:00"), Some("fp1")),
      cand("k2", None, None),
    ];
    let plan = plan_kb(&local, &backup);
    assert_eq!(plan.add, 1);
    assert_eq!(plan.entries[1].id, "k2");
    assert_eq!(plan.entries[1].action, MergeAction::Add, "本地没有 → 新增（无时间/指纹也新增）");
  }

  #[test]
  fn plan_updates_when_backup_is_newer() {
    let local = [cand("k1", Some("2026-11-01T09:00"), Some("same"))];
    let backup = [cand("k1", Some("2026-11-02T09:00"), Some("same"))];
    let plan = plan_kb(&local, &backup);
    assert_eq!(plan.entries[0].action, MergeAction::Update, "备份较新即覆盖，指纹相同不影响");
    assert_eq!(plan.update, 1);
  }

  #[test]
  fn plan_skips_when_local_is_newer() {
    let local = [cand("k1", Some("2026-11-02T09:00"), Some("fp1"))];
    let backup = [cand("k1", Some("2026-11-01T09:00"), Some("fp2"))];
    let plan = plan_tasks(&local, &backup);
    assert_eq!(plan.entries[0].action, MergeAction::Skip);
    assert_eq!(plan.skip, 1);
  }

  #[test]
  fn plan_skips_when_timestamp_and_fingerprint_match() {
    let local = [cand("k1", Some("2026-11-01T09:00"), Some("fp1"))];
    let backup = [cand("k1", Some("2026-11-01T09:00"), Some("fp1"))];
    let plan = plan_kb(&local, &backup);
    assert_eq!(plan.entries[0].action, MergeAction::Skip);
  }

  #[test]
  fn plan_conflicts_when_timestamp_equal_but_fingerprint_differs() {
    let local = [cand("k1", Some("2026-11-01T09:00"), Some("fp1"))];
    let backup = [cand("k1", Some("2026-11-01T09:00"), Some("fp2"))];
    let plan = plan_kb(&local, &backup);
    assert_eq!(plan.entries[0].action, MergeAction::Conflict);
    assert_eq!(plan.conflict, 1);
  }

  #[test]
  fn plan_skips_when_backup_is_older() {
    let local = [cand("k1", Some("2026-11-05T09:00"), Some("fp-local"))];
    let backup = [cand("k1", Some("2026-10-01T09:00"), Some("fp-backup"))];
    let plan = plan_kb(&local, &backup);
    assert_eq!(plan.entries[0].action, MergeAction::Skip, "备份更旧的条目一律跳过");
  }

  #[test]
  fn plan_preserves_backup_order_and_counts_mixed_actions() {
    let local = [
      cand("newer_local", Some("2026-11-05T09:00"), Some("a")),
      cand("backup_newer", Some("2026-10-30T09:00"), Some("d")),
      cand("same", Some("2026-11-03T09:00"), Some("b")),
      cand("tie_diff", Some("2026-11-02T09:00"), Some("c")),
    ];
    let backup = [
      cand("brand_new", None, None),
      cand("newer_local", Some("2026-11-01T09:00"), Some("a")),
      cand("backup_newer", Some("2026-11-04T09:00"), Some("d")),
      cand("same", Some("2026-11-03T09:00"), Some("b")),
      cand("tie_diff", Some("2026-11-02T09:00"), Some("c2")),
    ];
    let plan = plan_folders(&local, &backup);
    let actions: Vec<(&str, MergeAction)> = plan
      .entries
      .iter()
      .map(|entry| (entry.id.as_str(), entry.action))
      .collect();
    assert_eq!(
      actions,
      vec![
        ("brand_new", MergeAction::Add),
        ("newer_local", MergeAction::Skip),
        ("backup_newer", MergeAction::Update),
        ("same", MergeAction::Skip),
        ("tie_diff", MergeAction::Conflict),
      ],
      "动作列表与备份顺序一致"
    );
    assert_eq!((plan.add, plan.update, plan.skip, plan.conflict), (1, 1, 2, 1));
    assert_eq!(plan.total(), 5);
    assert_eq!(plan.count(MergeAction::Update), 1);
  }

  #[test]
  fn plan_skips_when_fingerprint_unknown() {
    // 时间相同但一侧指纹缺失 / 两侧都缺：无法判断，保守跳过
    let local = [cand("k1", Some("2026-11-01T09:00"), Some("fp1"))];
    let backup = [cand("k1", Some("2026-11-01T09:00"), None)];
    assert_eq!(plan_kb(&local, &backup).entries[0].action, MergeAction::Skip);

    let local = [cand("k1", None, None)];
    let backup = [cand("k1", None, None)];
    assert_eq!(plan_kb(&local, &backup).entries[0].action, MergeAction::Skip);
  }

  #[test]
  fn plan_falls_back_to_fingerprint_when_one_side_has_no_timestamp() {
    let local = [cand("k1", Some("2026-11-01T09:00"), Some("fp1"))];
    let backup = [cand("k1", None, Some("fp1"))];
    assert_eq!(plan_kb(&local, &backup).entries[0].action, MergeAction::Skip, "指纹相同");

    let backup = [cand("k1", None, Some("fp2"))];
    assert_eq!(
      plan_kb(&local, &backup).entries[0].action,
      MergeAction::Conflict,
      "指纹不同"
    );
  }

  #[test]
  fn plan_reminders_compares_by_id_and_fingerprint_only() {
    let local = [
      cand("r1", None, Some("fp1")),
      cand("r2", None, Some("fp2")),
    ];
    let backup = [
      cand("r1", None, Some("fp1")),
      cand("r2", None, Some("fp2-different")),
      cand("r3", None, Some("fp3")),
    ];
    let plan = plan_reminders(&local, &backup);
    assert_eq!(plan.entries[0].action, MergeAction::Skip, "提醒指纹相同 → 跳过");
    assert_eq!(plan.entries[1].action, MergeAction::Conflict, "提醒指纹不同 → 冲突");
    assert_eq!(plan.entries[2].action, MergeAction::Add, "本地没有 → 新增");
    assert_eq!((plan.add, plan.skip, plan.conflict), (1, 1, 1));
  }

  #[test]
  fn plan_compares_local_semantic_timestamps_lexicographically() {
    // 同一日期内按时刻比较；跨日期前缀比较
    let local = [cand("k1", Some("2026-11-01T10:00"), Some("x"))];
    let backup = [cand("k1", Some("2026-11-01T09:59"), Some("x"))];
    assert_eq!(plan_kb(&local, &backup).entries[0].action, MergeAction::Skip);

    let local = [cand("k1", Some("2026-11-09T23:00"), Some("x"))];
    let backup = [cand("k1", Some("2026-11-10T00:00"), Some("x"))];
    assert_eq!(plan_kb(&local, &backup).entries[0].action, MergeAction::Update);
  }

  // ---------------------------------------------------------------- 4.1 备份包 v1

  use serde_json::json;

  fn task_value(id: &str, title: &str, updated_at: &str) -> Value {
    json!({
      "id": id,
      "title": title,
      "createdAt": "2026-09-01T09:00",
      "updatedAt": updated_at
    })
  }

  fn kb_value(
    id: &str,
    title: &str,
    body: &str,
    updated_at: &str,
    folder: Option<&str>,
  ) -> Value {
    json!({
      "id": id,
      "title": title,
      "bodyMd": body,
      "tags": ["demo"],
      "createdAt": "2026-09-01T09:00",
      "updatedAt": updated_at,
      "folderId": folder,
      "order": 0,
      "pinned": false
    })
  }

  fn folder_value(id: &str, name: &str, parent: Option<&str>) -> Value {
    json!({"id": id, "name": name, "parentId": parent, "order": 0, "pinned": false})
  }

  fn sample_backup() -> BackupFile {
    let sha = sha256_hex(b"asset-bytes");
    BackupFile {
      format: BACKUP_FORMAT.to_string(),
      format_version: BACKUP_FORMAT_VERSION,
      app_version: "0.0.2-rc.1".to_string(),
      exported_at: "2026-11-14T10:00".to_string(),
      data: BackupData {
        tasks: vec![serde_json::from_value(task_value(
          "t-shared",
          "备份较新任务",
          "2026-11-10T09:00",
        ))
        .unwrap()],
        reminders: Vec::new(),
        settings: json!({"theme": "paper"}),
        stickies: vec![json!({"id": "s1", "content": "便签"})],
      },
      kb: BackupKb {
        items: vec![
          serde_json::from_value(kb_value(
            "k-shared",
            "备份较新知识",
            "备份正文",
            "2026-11-10T09:00",
            None,
          ))
          .unwrap(),
          serde_json::from_value(kb_value(
            "k-add",
            "新增知识",
            "新正文",
            "2026-11-10T09:00",
            None,
          ))
          .unwrap(),
        ],
        folders: vec![
          serde_json::from_value(folder_value("f-add", "新文件夹", None)).unwrap(),
          serde_json::from_value(folder_value("f-child", "子文件夹", Some("f-missing"))).unwrap(),
        ],
      },
      assets: vec![BackupAsset {
        sha256: sha,
        ext: "png".to_string(),
        base64: base64_encode(b"asset-bytes"),
      }],
    }
  }

  #[test]
  fn backup_roundtrip_keeps_entries_and_assets() {
    let file = sample_backup();
    let text = export_payload(&file).unwrap();
    let parsed = parse_backup(&text).unwrap();
    assert_eq!(parsed.file.format, BACKUP_FORMAT);
    assert_eq!(parsed.file.format_version, BACKUP_FORMAT_VERSION);
    assert_eq!(parsed.file.exported_at, "2026-11-14T10:00");
    assert_eq!(parsed.file.data.tasks.len(), 1);
    assert_eq!(parsed.file.kb.items.len(), 2);
    assert_eq!(parsed.file.kb.folders.len(), 2);
    assert_eq!(parsed.file.kb.folders[0].order, Some(0));
    assert_eq!(parsed.asset_count(), 1);
    let sha = sha256_hex(b"asset-bytes");
    let asset = parsed.asset(&sha, "png").expect("资源应被解码");
    assert!(asset.hash_ok);
    assert_eq!(asset.bytes, b"asset-bytes");
    // 序列化 → 反序列化 → 再序列化：结构保持稳定。
    // 解析后资源以解码后的 bytes 为唯一真源（file.assets[].base64 被清空，避免双份内存），
    // 所以再序列化时仅 base64 占位为空串，其余结构必须逐字一致。
    let again = export_payload(&parsed.file).unwrap();
    let normalize = |text: &str| -> Value {
      let mut value: Value = serde_json::from_str(text).unwrap();
      if let Some(assets) = value.get_mut("assets").and_then(Value::as_array_mut) {
        for asset in assets {
          asset["base64"] = json!("");
        }
      }
      value
    };
    assert_eq!(normalize(&text), normalize(&again));
  }

  #[test]
  fn backup_rejects_bad_format_unknown_version_and_incomplete() {
    let file = sample_backup();
    let mut bad = serde_json::to_value(&file).unwrap();
    bad["format"] = json!("other-backup");
    assert_eq!(
      parse_backup(&bad.to_string()).unwrap_err(),
      BACKUP_FORMAT_INVALID
    );

    let mut bad = serde_json::to_value(&file).unwrap();
    bad["formatVersion"] = json!(2);
    assert_eq!(
      parse_backup(&bad.to_string()).unwrap_err(),
      BACKUP_VERSION_UNSUPPORTED
    );

    let mut bad = serde_json::to_value(&file).unwrap();
    bad["data"] = Value::Null;
    assert_eq!(
      parse_backup(&bad.to_string()).unwrap_err(),
      BACKUP_CONTENT_INCOMPLETE
    );

    assert_eq!(parse_backup("这不是JSON").unwrap_err(), BACKUP_FORMAT_INVALID);

    // assets / stickies 等可选段缺失时按空兜底（旧包兼容）
    let mut minimal = serde_json::to_value(&file).unwrap();
    minimal.as_object_mut().unwrap().remove("assets");
    minimal["data"]
      .as_object_mut()
      .unwrap()
      .remove("stickies");
    assert!(parse_backup(&minimal.to_string()).is_ok());
  }

  #[test]
  fn backup_validates_asset_base64_ext_and_size() {
    let mut bad = sample_backup();
    bad.assets[0].base64 = "不是base64!!".to_string();
    assert_eq!(
      parse_backup(&export_payload(&bad).unwrap()).unwrap_err(),
      BACKUP_ASSET_INVALID
    );

    let mut bad = sample_backup();
    bad.assets[0].ext = "exe".to_string();
    assert_eq!(
      parse_backup(&export_payload(&bad).unwrap()).unwrap_err(),
      BACKUP_ASSET_INVALID
    );

    let big = vec![0u8; BACKUP_MAX_ASSET_BYTES + 1];
    let mut bad = sample_backup();
    bad.assets[0].sha256 = sha256_hex(&big);
    bad.assets[0].base64 = base64_encode(&big);
    assert_eq!(
      parse_backup(&export_payload(&bad).unwrap()).unwrap_err(),
      BACKUP_ASSET_INVALID
    );
  }

  #[test]
  fn backup_asset_hash_mismatch_is_flagged_not_fatal() {
    let mut file = sample_backup();
    let wrong_sha = sha256_hex(b"different-bytes");
    file.assets[0].sha256 = wrong_sha.clone();
    let parsed = parse_backup(&export_payload(&file).unwrap()).unwrap();
    let asset = parsed.asset(&wrong_sha, "png").expect("资源仍应被解码");
    assert!(!asset.hash_ok, "sha256 与内容不符只标记，不阻断整包");
    assert!(!asset.is_reusable());

    // 条目数量超限 → 中文拒绝
    let mut huge = sample_backup();
    huge.data.tasks = (0..=BACKUP_MAX_ENTRIES)
      .map(|index| {
        serde_json::from_value(task_value(
          &format!("t{index}"),
          "任务",
          "2026-11-10T09:00",
        ))
        .unwrap()
      })
      .collect();
    assert_eq!(
      parse_backup(&export_payload(&huge).unwrap()).unwrap_err(),
      BACKUP_TOO_MANY_ENTRIES
    );
  }

  #[test]
  fn backup_scans_body_asset_refs_deduped() {
    let sha = sha256_hex(b"asset-bytes");
    let body = format!(
      "图 ![a](assets/{sha}.png) 与 ![b](assets/{sha}.png)，忽略 assets/nothex.png 和 assets/{sha}.exe"
    );
    let refs = asset_refs_in_body(&body);
    assert_eq!(refs, vec![(sha.clone(), "png".to_string())]);
    let item: KbItem = serde_json::from_value(kb_value(
      "k1",
      "标题",
      &body,
      "2026-11-01T09:00",
      None,
    ))
    .unwrap();
    assert_eq!(collect_asset_refs(&[item]).len(), 1);
  }

  #[test]
  fn backup_kb_fingerprint_ignores_tag_order_and_trim() {
    let mut first: KbItem = serde_json::from_value(kb_value(
      "k1",
      "标题",
      "正文",
      "2026-11-01T09:00",
      None,
    ))
    .unwrap();
    let mut second = first.clone();
    first.tags = vec![" beta ".to_string(), "alpha".to_string()];
    second.tags = vec!["alpha".to_string(), "  beta".to_string()];
    assert_eq!(kb_fingerprint(&first), kb_fingerprint(&second));
    second.body_md = "正文改了".to_string();
    assert_ne!(kb_fingerprint(&first), kb_fingerprint(&second));
  }

  fn local_snapshot(
    tasks: Vec<Value>,
    kb: Vec<Value>,
    folders: Vec<Value>,
  ) -> LocalSnapshot {
    LocalSnapshot::new(
      tasks
        .into_iter()
        .map(|raw| serde_json::from_value(raw).unwrap())
        .collect(),
      Vec::new(),
      kb.into_iter()
        .map(|raw| serde_json::from_value(raw).unwrap())
        .collect(),
      folders
        .into_iter()
        .map(|raw| serde_json::from_value(raw).unwrap())
        .collect(),
    )
  }

  #[test]
  fn backup_plan_and_apply_create_conflict_copies_and_keep_locals() {
    let backup = sample_backup();
    let parsed = parse_backup(&export_payload(&backup).unwrap()).unwrap();
    let local = local_snapshot(
      vec![
        task_value("t-shared", "本地任务", "2026-11-10T09:00"),
        task_value("t-local", "本地独有任务", "2026-11-09T09:00"),
      ],
      vec![
        kb_value("k-shared", "本地知识", "本地正文", "2026-11-10T09:00", Some("f-local")),
      ],
      vec![folder_value("f-local", "本地文件夹", None)],
    );

    let plan = plan_import(&local, &parsed);
    assert_eq!(plan.tasks.conflict, 1, "时间相同、指纹不同 → 冲突");
    assert_eq!(plan.kb.conflict, 1);
    assert_eq!(plan.kb.add, 1);
    assert_eq!(plan.folders.add, 2);
    assert_eq!(plan.changed(), 5);
    assert_eq!(conflict_titles(&local, &plan), vec!["本地任务", "本地知识"]);

    let applied = apply_import_plan(&local, &parsed, &plan, "2026-11-14T11:00").unwrap();
    assert_eq!(applied.tasks.len(), 3, "本地独有 + 本地原条目 + 冲突副本");
    assert_eq!(applied.tasks[0].title, "本地任务", "冲突保留本地版本");
    assert_eq!(applied.tasks[1].title, "本地独有任务");
    assert!(applied
      .tasks
      .iter()
      .any(|task| task.title == "备份较新任务（导入冲突）"));
    assert_eq!(applied.kb.len(), 3);
    assert_eq!(applied.kb[0].title, "本地知识");
    let copy = applied
      .kb
      .iter()
      .find(|item| item.title == "备份较新知识（导入冲突）")
      .expect("知识冲突副本应存在");
    assert_eq!(copy.folder_id.as_deref(), Some("f-local"), "副本跟随本地文件夹");
    assert_eq!(copy.updated_at, "2026-11-14T11:00");
    assert_eq!(applied.folders.len(), 3, "新增两个备份文件夹");
    let child = applied
      .folders
      .iter()
      .find(|folder| folder.id == "f-child")
      .expect("子文件夹应新增");
    assert_eq!(child.parent_id, None, "父级缺失时上提为根");
    assert_eq!(applied.conflict_copies, 2);
    // 正文写入：冲突副本 + 新增条目各一条
    assert_eq!(applied.body_writes.len(), 2);

    // 同一备份再导入一次：冲突副本已存在 → 全部跳过，幂等
    let after = LocalSnapshot::new(
      applied.tasks.clone(),
      applied.reminders.clone(),
      applied.kb.clone(),
      applied.folders.clone(),
    );
    let plan_again = plan_import(&after, &parsed);
    assert_eq!(plan_again.changed(), 0, "重复导入不应产生任何动作");
    assert_eq!(plan_again.tasks.skip, 1);
    assert_eq!(plan_again.kb.skip, 2);
  }

  #[test]
  fn backup_plan_updates_newer_and_skips_older() {
    let mut backup = sample_backup();
    backup.kb.items.clear();
    backup.kb.folders.clear();
    backup.assets.clear();
    backup.data.tasks = vec![
      serde_json::from_value(task_value("t-newer", "备份新", "2026-11-12T09:00")).unwrap(),
      serde_json::from_value(task_value("t-older", "备份旧", "2026-10-01T09:00")).unwrap(),
    ];
    let parsed = parse_backup(&export_payload(&backup).unwrap()).unwrap();
    let local = local_snapshot(
      vec![
        task_value("t-newer", "本地旧", "2026-11-01T09:00"),
        task_value("t-older", "本地新", "2026-11-09T09:00"),
      ],
      Vec::new(),
      Vec::new(),
    );
    let plan = plan_import(&local, &parsed);
    assert_eq!(plan.tasks.update, 1);
    assert_eq!(plan.tasks.skip, 1);
    let applied = apply_import_plan(&local, &parsed, &plan, "2026-11-14T11:00").unwrap();
    assert_eq!(applied.tasks[0].title, "备份新", "备份较新 → 覆盖");
    assert_eq!(applied.tasks[1].title, "本地新", "本地较新 → 保持");
  }

  #[test]
  fn backup_plan_keeps_local_reminder_conflicts() {
    let mut backup = sample_backup();
    backup.data.reminders = vec![serde_json::from_value(json!({
      "id": "r-shared",
      "title": "备份提醒",
      "time": "2026-11-15T09:00",
      "enabled": true,
      "completed": false
    }))
    .unwrap()];
    let parsed = parse_backup(&export_payload(&backup).unwrap()).unwrap();
    let local_reminder: Reminder = serde_json::from_value(json!({
      "id": "r-shared",
      "title": "本地提醒",
      "time": "2026-11-15T09:00",
      "enabled": true,
      "completed": false
    }))
    .unwrap();
    let local = LocalSnapshot::new(
      Vec::new(),
      vec![local_reminder],
      Vec::new(),
      Vec::new(),
    );
    let plan = plan_import(&local, &parsed);
    assert_eq!(plan.reminders.conflict, 0, "提醒冲突不生成副本");
    assert_eq!(plan.reminders.skip, 1);
    let applied = apply_import_plan(&local, &parsed, &plan, "2026-11-14T11:00").unwrap();
    assert_eq!(applied.reminders.len(), 1);
    assert_eq!(applied.reminders[0].title, "本地提醒");
  }

  #[test]
  fn backup_export_assembly_counts_skipped_assets() {
    let inputs = ExportInputs {
      data: AppData::default(),
      kb: Vec::new(),
      folders: Vec::new(),
      assets: vec![ExportAsset {
        sha256: sha256_hex(b"one"),
        ext: "png".to_string(),
        bytes: b"one".to_vec(),
      }],
      skipped: 2,
    };
    let file = build_backup("0.0.2-rc.1", "2026-11-14T10:00", inputs);
    assert_eq!(file.format, BACKUP_FORMAT);
    assert_eq!(file.assets.len(), 1);
    assert_eq!(file.assets[0].base64, base64_encode(b"one"));
    // AppData 默认设置照常序列化进包（导出的是用户数据快照，不是空值）
    assert!(file.data.settings.is_object());
    assert_eq!(file.data.settings["theme"], "float");
  }
}
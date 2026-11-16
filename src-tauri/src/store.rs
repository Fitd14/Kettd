//! 存储安全层（PRD 6.6）
//!
//! 根治 v1 三个病灶：
//! 1. `unwrap_or_default()` 静默清空 → 解析失败一律进 corrupt 通道，写命令全部拒绝
//! 2. 裸 `fs::write` 全量覆盖 → 轮转备份 + tmp + rename 原子替换
//! 3. 时间漂移 / snake_case → 本地语义归一 + camelCase 序列化 + 迁移前原件永久留档
//!
//! 数据目录沿用 v1 的 `%APPDATA%/todo-list`（兼容发现）。

use crate::backup;
use crate::infra::fs_store::{default_dir, FsBackend, ARCHIVE_FILE};
use crate::models::{
  carry_days, clocks_of, fmt_day, fmt_dt, new_id, normalize_date_text, normalize_datetime, now_text, today,
  patch_datetime, patch_text, parse_wall, to_local, AppData, BackupInfo, CATEGORIES,
  DataHealthV2, KbBackupInfo, KbFolder, KbItem, MigrationIssue, MigrationReport, Note, PRIORITIES, Reminder,
  ReminderPayload,
  RuntimeState,
  Repeat, Settings, SettingsPayload, Source, STICKY_PAPERS, Subtask, Task, TaskPayload,
  TRASH_RETENTION_DAYS,
};
use crate::ports::clock::{Clock, SystemClock};
use crate::ports::store_backend::StoreBackend;
use crate::store_notes;
use chrono::{DateTime, Duration, Local};
use serde_json::{json, Value};

pub use crate::infra::fs_store::BACKUP_SLOTS;

pub const HEALTH_OK: &str = "ok";
pub const HEALTH_CORRUPT: &str = "corrupt";
pub const HEALTH_WRITE_FAILED: &str = "writeFailed";

/// 知识库损坏时的可恢复错误文案（D8）：notes.json / folders.json 解析失败不再静默空库，
/// 前端据此引导「设置 → 备份与恢复」，绝不渲染假空态。
pub const KB_CORRUPT_MESSAGE: &str = "知识库数据损坏，可在设置 → 备份与恢复中恢复";

/// 文件夹名称长度上限（按字符数，中文算 1 个）
const FOLDER_NAME_MAX_CHARS: usize = 40;

/// v1 同款目录：Windows 下 `dirs::data_dir()` == `%APPDATA%`。
/// 只给"打开数据文件夹 / 导出目录 / 埋点落盘"这类**知道路径**的场景用；
/// Store 的读写一律走 StoreBackend，不再直接碰这个路径。
pub fn data_dir() -> std::path::PathBuf {
  default_dir()
}

// ---------------------------------------------------------------- 读盘与迁移

fn text_of(object: &Value, keys: &[&str]) -> Option<String> {
  for key in keys {
    if let Some(value) = object.get(*key) {
      match value {
        Value::Null => continue,
        Value::String(text) => {
          if !text.trim().is_empty() {
            return Some(text.trim().to_string());
          }
          continue;
        }
        Value::Bool(_) | Value::Number(_) => return Some(value.to_string()),
        _ => continue,
      }
    }
  }
  None
}

fn bool_of(object: &Value, keys: &[&str]) -> Option<bool> {
  for key in keys {
    if let Some(value) = object.get(*key) {
      match value {
        Value::Bool(flag) => return Some(*flag),
        Value::String(text) => {
          let lower = text.trim().to_lowercase();
          if lower == "true" || lower == "1" {
            return Some(true);
          }
          if lower == "false" || lower == "0" {
            return Some(false);
          }
        }
        Value::Number(number) => {
          if let Some(parsed) = number.as_i64() {
            return Some(parsed != 0);
          }
        }
        _ => {}
      }
    }
  }
  None
}

fn number_of(object: &Value, keys: &[&str]) -> Option<u32> {
  for key in keys {
    if let Some(value) = object.get(*key) {
      if let Some(parsed) = value.as_u64() {
        return Some(parsed.min(u32::MAX as u64) as u32);
      }
    }
  }
  None
}

fn push_issue(
  report: &mut MigrationReport,
  collection: &str,
  id: &str,
  field: &str,
  raw: &str,
  action: String,
) {
  report.issues.push(MigrationIssue {
    collection: collection.to_string(),
    id: id.to_string(),
    field: field.to_string(),
    raw: raw.to_string(),
    action,
  });
}

fn migrate_category(raw: Option<String>, report: &mut MigrationReport, id: &str) -> String {
  let value = match raw {
    None => return "生活".to_string(),
    Some(text) => text,
  };
  let trimmed = value.trim();
  if trimmed.is_empty() {
    return "生活".to_string();
  }
  if CATEGORIES.contains(&trimmed) {
    return trimmed.to_string();
  }
  // v1「个人」→ v2「生活」
  let mapped = match trimmed {
    "个人" => "生活",
    _ => "",
  };
  if !mapped.is_empty() {
    push_issue(
      report,
      "task",
      id,
      "category",
      trimmed,
      format!("重映射为 {}", mapped),
    );
    return mapped.to_string();
  }
  push_issue(report, "task", id, "category", trimmed, "回落到 生活".to_string());
  "生活".to_string()
}

fn migrate_priority(raw: Option<String>, report: &mut MigrationReport, id: &str) -> String {
  let value = match raw {
    None => return "med".to_string(),
    Some(text) => text,
  };
  let trimmed = value.trim().to_lowercase();
  if PRIORITIES.contains(&trimmed.as_str()) {
    return trimmed;
  }
  let mapped = match trimmed.as_str() {
    "medium" | "mid" | "normal" => "med",
    "高" => "high",
    "中" => "med",
    "低" => "low",
    _ => "med",
  };
  push_issue(
    report,
    "task",
    id,
    "priority",
    &trimmed,
    format!("归一为 {}", mapped),
  );
  mapped.to_string()
}

/// 迁移期时间归一：无法解析 → 置 null 并记入报告（绝不猜测、不做时区换算）
fn migrate_time(
  object: &Value,
  keys: &[&str],
  report: &mut MigrationReport,
  collection: &str,
  id: &str,
  field: &str,
) -> Option<String> {
  let raw = match text_of(object, keys) {
    None => return None,
    Some(text) => text,
  };
  match normalize_datetime(&raw) {
    Some(normalized) => {
      if normalized != raw {
        push_issue(
          report,
          collection,
          id,
          field,
          &raw,
          format!("归一为 {}", normalized),
        );
      }
      Some(normalized)
    }
    None => {
      report.invalid_dates += 1;
      push_issue(report, collection, id, field, &raw, "无法解析，已置空".to_string());
      None
    }
  }
}

fn migrate_task(object: &Value, report: &mut MigrationReport) -> Task {
  let id = text_of(object, &["id"]).unwrap_or_else(|| new_id("t"));
  let title = text_of(object, &["title"]).unwrap_or_else(|| "（v1 无标题）".to_string());
  let note = text_of(object, &["note", "description"]).unwrap_or_default();
  let category = migrate_category(text_of(object, &["category"]), report, &id);
  let priority = migrate_priority(text_of(object, &["priority"]), report, &id);
  let due_at = migrate_time(object, &["dueAt", "due_date"], report, "task", &id, "dueAt");
  let remind_at = migrate_time(object, &["remindAt", "remind_date"], report, "task", &id, "remindAt");
  let planned_date = migrate_time(object, &["plannedDate", "planned_date"], report, "task", &id, "plannedDate")
    .and_then(|value| normalize_date_text(&value));
  let created_at = migrate_time(object, &["createdAt", "created_at"], report, "task", &id, "createdAt")
    .unwrap_or_else(|| format!("{}T00:00", fmt_day(&today())));
  let updated_at = migrate_time(object, &["updatedAt", "updated_at"], report, "task", &id, "updatedAt")
    .unwrap_or_else(|| created_at.clone());
  let deleted_at = migrate_time(object, &["deletedAt", "deleted_at"], report, "task", &id, "deletedAt");
  let carried_from = number_of(object, &["carriedFrom", "carried_from"]).unwrap_or(0);
  let done = bool_of(object, &["done", "completed"]).unwrap_or(false);

  let mut done_at = migrate_time(object, &["doneAt", "done_at"], report, "task", &id, "doneAt");
  let mut legacy = bool_of(object, &["legacy"]).unwrap_or(false);
  if done && done_at.is_none() {
    // v1「完成」没有时刻：doneAt 置空 + legacy 标记（不伪造完成时间）
    legacy = true;
    report.legacy_done += 1;
    push_issue(
      report,
      "task",
      &id,
      "doneAt",
      "（缺失）",
      "置空并标 legacy".to_string(),
    );
  }
  if !done {
    done_at = None;
  }

  let mut subtasks: Vec<Subtask> = Vec::new();
  if let Some(items) = object.get("subtasks").and_then(|value| value.as_array()) {
    for item in items {
      if !item.is_object() {
        push_issue(
          report,
          "subtask",
          &id,
          "subtasks",
          &item.to_string(),
          "非对象条目已丢弃".to_string(),
        );
        continue;
      }
      let sub_title = text_of(item, &["title"]).unwrap_or_default();
      if sub_title.trim().is_empty() {
        continue;
      }
      subtasks.push(Subtask {
        id: text_of(item, &["id"]).unwrap_or_else(|| new_id("s")),
        title: sub_title,
        done: bool_of(item, &["done", "completed"]).unwrap_or(false),
      });
    }
  }

  let mut notes: Vec<Note> = Vec::new();
  if let Some(items) = object.get("notes").and_then(|value| value.as_array()) {
    for item in items {
      if !item.is_object() {
        push_issue(
          report,
          "note",
          &id,
          "notes",
          &item.to_string(),
          "非对象条目已丢弃".to_string(),
        );
        continue;
      }
      let content = text_of(item, &["content"]).unwrap_or_default();
      let author = text_of(item, &["author"]).unwrap_or_else(|| "我".to_string());
      if content.trim().is_empty() && author.trim().is_empty() {
        continue;
      }
      let created = migrate_time(item, &["createdAt", "created_at"], report, "note", &id, "createdAt");
      notes.push(Note {
        id: text_of(item, &["id"]).unwrap_or_else(|| new_id("n")),
        author,
        content,
        created_at: created.unwrap_or_else(|| created_at.clone()),
      });
    }
  }

  let source = match text_of(object, &["source"]).as_deref() {
    Some("capture") => Source::Capture,
    Some("seed") => Source::Seed,
    _ => Source::Manual,
  };

  let mut task = Task {
    id,
    title,
    note,
    category,
    priority,
    due_at,
    remind_at,
    planned_date,
    carried_from,
    kb_refs: Vec::new(),
    sort_order: None,
    done,
    done_at,
    deleted_at,
    legacy,
    subtasks,
    notes,
    source,
    created_at,
    updated_at,
  };
  if task.planned_date.is_none() && !task.done && !task.in_trash() {
    // 迁移一次性生成初始计划：逾期未完成 → 粘到今天并计拖留天数
    let days = carry_days(&task.due_at.clone());
    if days > 0 {
      task.planned_date = Some(fmt_day(&today()));
      task.carried_from = days;
    }
  }
  task
}

fn migrate_reminder(object: &Value, report: &mut MigrationReport) -> Reminder {
  let id = text_of(object, &["id"]).unwrap_or_else(|| new_id("r"));
  let title = text_of(object, &["title"]).unwrap_or_else(|| "提醒".to_string());
  let raw_time = text_of(object, &["time", "remindAt", "remind_date"]).unwrap_or_default();
  let multi_clock = raw_time.contains('/') && clocks_of(&raw_time).len() > 1;
  let time = if raw_time.is_empty() {
    report.invalid_dates += 1;
    String::new()
  } else if multi_clock {
    // v1 的 "10:00/14:00/16:00"：原样保留，调度器按多时刻触发
    raw_time.clone()
  } else {
    match normalize_datetime(&raw_time) {
      Some(value) => value,
      None => {
        report.invalid_dates += 1;
        push_issue(
          report,
          "reminder",
          &id,
          "time",
          &raw_time,
          "无法解析，已置空".to_string(),
        );
        String::new()
      }
    }
  };
  let category = migrate_category(text_of(object, &["category"]), report, &id);
  let completed = bool_of(object, &["completed", "done"]).unwrap_or(false);
  let repeat = match text_of(object, &["repeat"]).as_deref() {
    Some("daily") => Repeat::Daily,
    Some("workdays") => Repeat::Workdays,
    Some("weekly") => Repeat::Weekly,
    _ => Repeat::None,
  };
  Reminder {
    id,
    title,
    time,
    category,
    enabled: !completed,
    completed: false,
    repeat,
    last_fired: None,
    snoozed_until: None,
    legacy: true,
  }
}

/// v1 顶层 theme（字符串）→ v2 settings.theme（float 主题），旧值不解读只保底
fn migrate_settings(raw: &Value, settings: &mut Settings, report: &mut MigrationReport) {
  if let Some(Value::String(text)) = raw.get("theme") {
    push_issue(
      report,
      "settings",
      "settings",
      "theme",
      text,
      "v2 主题语义变更，回落到 float".to_string(),
    );
    settings.theme = "float".to_string();
  }
  // v1/v2 旧 floatForm（topmost/desktop/mini）与 stickyPinned 一律忽略：置顶已常驻无开关（sticky-separation）
  if let Some(value) = raw.get("captureHotkey").and_then(|item| item.as_str()) {
    if !value.trim().is_empty() {
      settings.capture_hotkey = value.trim().to_string();
    }
  }
  if let Some(value) = raw.get("remindCapPerHour").and_then(|item| item.as_u64()) {
    settings.remind_cap_per_hour = value.min(60) as u32;
  }
  if let Some(value) = raw.get("onboarded").and_then(|item| item.as_bool()) {
    settings.onboarded = value;
  }
  settings.coerce();
}

/// snake_case → camelCase 的 v1→v2 懒迁移（首次加载触发，原件永久留档 data.v1.json）
pub fn migrate_v1(raw: &Value) -> Result<(AppData, MigrationReport), String> {
  let mut report = MigrationReport::default();
  let mut data = AppData::default();

  match raw.get("tasks") {
    Some(Value::Array(items)) => {
      for item in items {
        if !item.is_object() {
          push_issue(
            &mut report,
            "task",
            "unknown",
            "tasks",
            &item.to_string(),
            "非对象条目已丢弃".to_string(),
          );
          continue;
        }
        data.tasks.push(migrate_task(item, &mut report));
      }
    }
    Some(Value::Null) | None => {}
    Some(_) => return Err("旧数据里的 tasks 不是列表".to_string()),
  }
  match raw.get("reminders") {
    Some(Value::Array(items)) => {
      for item in items {
        if !item.is_object() {
          continue;
        }
        data.reminders.push(migrate_reminder(item, &mut report));
      }
    }
    Some(Value::Null) | None => {}
    Some(_) => return Err("旧数据里的 reminders 不是列表".to_string()),
  }
  // v1 的 fired 键不再进 AppData —— 由 from_v1 收进 RuntimeState（ADR-0005）
  if let Some(value) = raw.get("settings") {
    if value.is_object() {
      if let Ok(parsed) = serde_json::from_value::<Settings>(value.clone()) {
        data.settings = parsed;
      }
    }
  }
  migrate_settings(raw, &mut data.settings, &mut report);
  report.task_count = data.tasks.len() as u32;
  report.reminder_count = data.reminders.len() as u32;
  report.archived_to = ARCHIVE_FILE.to_string();
  Ok((data, report))
}

/// 判别 v1 文件：v2 一定有 settings 对象；v1 顶层 theme 是字符串
fn looks_like_v1(raw: &Value) -> bool {
  if let Some(value) = raw.get("settings") {
    if value.is_object() {
      return false;
    }
  }
  if let Some(value) = raw.get("theme") {
    if value.is_string() {
      return true;
    }
  }
  raw.get("tasks").is_some() || raw.get("reminders").is_some()
}

/// 便签分离迁移（sticky-separation，v2 便签 schema → v3）：在 JSON 层归一历史便签。
/// - kind="todo" 的镜像便签丢弃（内容活在 tasks，零损失）
/// - hidden=true（旧「收进托盘」）升格为 mini=true（缩小悬浮条）
/// - kind/pinned/hidden 字段清除（自由便签无类别、置顶常驻无开关）
/// 返回是否发生变化；幂等——已是新形态的数据原样返回 false。
fn normalize_stickies_v3(data: &mut Value) -> bool {
  let Some(list) = data.get_mut("stickies").and_then(|v| v.as_array_mut()) else {
    return false;
  };
  let mut changed = false;
  list.retain(|entry| {
    let is_todo = entry.get("kind").and_then(|v| v.as_str()) == Some("todo");
    if is_todo {
      changed = true;
    }
    !is_todo
  });
  for entry in list.iter_mut() {
    let Some(obj) = entry.as_object_mut() else { continue };
    if let Some(hidden) = obj.get("hidden").and_then(|v| v.as_bool()) {
      if hidden {
        obj.insert("mini".to_string(), Value::Bool(true));
      }
      changed = true;
    }
    for dead in ["kind", "pinned", "hidden"] {
      changed |= obj.remove(dead).is_some();
    }
  }
  changed
}

/// 判别 v2 旧 schema（ADR-0005）：运行态（fired / migration / settings.capturePos）
/// 还混在 data.json 里 → 需要一次拆分迁移
pub fn is_legacy_schema(raw: &Value) -> bool {
  if raw.get("fired").is_some() || raw.get("migration").is_some() {
    return true;
  }
  raw
    .get("settings")
    .map(|settings| settings.get("capturePos").is_some())
    .unwrap_or(false)
}

/// schema 拆分（**纯函数**，ADR-0005 的迁移核心）：
/// 旧 data 文档 + 已存在的 runtime 文档（可 None）→ （新 data 文档，runtime 文档）。
/// 幂等：fired 与已存在 runtime 取**并集**（集合语义，合并无害）；
/// 迁移报告与 capturePos 以 runtime 侧已有值为先（可能已被读后清账）。
fn split_schema(raw: &Value, existing_runtime: Option<&Value>) -> (Value, Value) {
  let mut data = raw.clone();
  let mut runtime = existing_runtime.cloned().unwrap_or_else(|| json!({}));
  if !runtime.is_object() {
    runtime = json!({}); // 已存在的 runtime 不是对象 → 视为空（与静默重建同口径）
  }

  // fired：并集（非字符串键丢弃，与 v1 迁移同口径）
  let mut fired: Vec<String> = runtime
    .get("fired")
    .and_then(|value| value.as_array())
    .map(|items| {
      items
        .iter()
        .filter_map(|value| value.as_str().map(str::to_string))
        .collect()
    })
    .unwrap_or_default();
  if let Some(items) = data.get("fired").and_then(|value| value.as_array()) {
    for value in items.iter() {
      if let Some(key) = value.as_str() {
        if !fired.iter().any(|existing| existing == key) {
          fired.push(key.to_string());
        }
      }
    }
  }
  runtime["fired"] = Value::Array(fired.into_iter().map(Value::String).collect());

  // 迁移报告：data 侧有、runtime 侧没有才搬（runtime 已有 = 已在账上）
  if runtime.get("migration").is_none() {
    if let Some(report) = data.get("migration").filter(|value| !value.is_null()) {
      runtime["migration"] = report.clone();
    }
  }

  // capturePos：settings → runtime
  if runtime.get("capturePos").map(|value| value.is_null()).unwrap_or(true) {
    if let Some(pos) = data
      .get("settings")
      .and_then(|settings| settings.get("capturePos"))
      .filter(|value| !value.is_null())
    {
      runtime["capturePos"] = pos.clone();
    }
  }

  // data 侧清干净三样
  if let Some(object) = data.as_object_mut() {
    object.remove("fired");
    object.remove("migration");
    if let Some(settings) = object.get_mut("settings").and_then(|value| value.as_object_mut()) {
      settings.remove("capturePos");
    }
  }
  (data, runtime)
}

// ---------------------------------------------------------------- Store

/// 撤销栈条目
#[derive(Debug, Clone)]
pub enum Undoable {
  Task(Box<Task>),
  Subtask { task_id: String, subtask: Subtask },
  Note { task_id: String, note: Note },
}

pub struct Store {
  /// 物理读写经端口（生产 = FsBackend，测试 = InMemoryBackend）
  backend: Box<dyn StoreBackend>,
  /// 时间经端口（生产 = SystemClock，测试 = FixedClock）
  clock: Box<dyn Clock>,
  pub data: AppData,
  /// 运行态（runtime.json）：fired / capturePos / notePos / 迁移报告（ADR-0005）
  pub runtime: RuntimeState,
  /// 知识条目（notes.json，frame H1b）：与 data.json 物理隔离
  pub kb: Vec<KbItem>,
  /// 知识库文件夹（folders.json，D6）：条目只持 folderId 引用，零迁移
  pub folders: Vec<KbFolder>,
  /// 知识库损坏错误态（D8）：None = 正常；Some(文案) = notes/folders 读或解析失败。
  /// 不走 data.json 的 corrupt 通道（KB 可整体降级），但读取命令必须 Err 而非空库。
  kb_error: Option<String>,
  /// 索引相对内存的「可修复差异」待落盘标记（D1）：载入时 contentHash 以正文为准修正、
  /// 或 folderId 失效回退未分类后置位，下一次 save_notes 成功后清零。绝不据此判损坏。
  kb_index_dirty: bool,
  pub health: String,
  pub corrupt_file: Option<String>,
  pub last_error: Option<String>,
  pub undo: Vec<Undoable>,
}

impl Store {
  /// 组合点：测试注入 InMemoryBackend + FixedClock，即不碰磁盘、不等真实时间
  pub fn with_backend(backend: Box<dyn StoreBackend>, clock: Box<dyn Clock>) -> Self {
    Self {
      backend,
      clock,
      data: AppData::default(),
      runtime: RuntimeState::default(),
      kb: Vec::new(),
      folders: Vec::new(),
      kb_error: None,
      kb_index_dirty: false,
      health: HEALTH_OK.to_string(),
      corrupt_file: None,
      last_error: None,
      undo: Vec::new(),
    }
  }

  /// 当前时刻（Store 内一切"现在"的唯一来源）
  fn clock_now(&self) -> DateTime<Local> {
    self.clock.now()
  }

  /// 用户可见时刻文本（updated_at / deleted_at 等戳）
  fn stamp(&self) -> String {
    fmt_dt(self.clock_now())
  }

  /// 启动加载：任何失败都进 corrupt 通道，绝不返回空集合
  pub fn load() -> Store {
    let dir = default_dir();
    if let Err(error) = std::fs::create_dir_all(&dir) {
      let mut store =
        Store::with_backend(Box::new(FsBackend::new(dir)), Box::new(SystemClock));
      store.health = HEALTH_WRITE_FAILED.to_string();
      store.last_error = Some(format!("无法创建数据文件夹：{}", brief_io(&error)));
      return store;
    }
    Self::load_with(Box::new(FsBackend::new(dir)), Box::new(SystemClock))
  }

  pub fn load_with(backend: Box<dyn StoreBackend>, clock: Box<dyn Clock>) -> Store {
    let mut store = Store::with_backend(backend, clock);
    if !store.backend.data_exists() {
      if store.backend.archive_exists() {
        // 上次迁移中途失败：从永久留档的 v1 原件重来
        let raw = store.backend.read_archive().and_then(|text| parse_value(&text));
        return match raw {
          Ok(value) => {
            let mut migrated = store.from_v1(&value, true);
            migrated.load_kb();
            migrated
          }
          Err(error) => {
            store.health = HEALTH_CORRUPT.to_string();
            store.last_error = Some(error);
            store
          }
        };
      }
      // 全新安装：空库落盘（不塞演示数据，PRD 6.2）
      if let Err(error) = store.save() {
        store.last_error = Some(error);
        store.health = HEALTH_WRITE_FAILED.to_string();
      }
      // 残留的 notes.json / folders.json 仍要加载（否则下一次 KB 写入会覆盖历史条目）
      store.load_kb();
      return store;
    }
    let raw_text = match store
      .backend
      .read_data()
      .and_then(|option| option.ok_or_else(|| "数据文件读不了，已停止写入".to_string()))
    {
      Ok(text) => text,
      Err(error) => {
        store.corrupt_file = store.backend.quarantine();
        store.health = HEALTH_CORRUPT.to_string();
        store.last_error = Some(error);
        return store;
      }
    };
    let raw = match parse_value(&raw_text) {
      Ok(value) => value,
      Err(error) => {
        store.corrupt_file = store.backend.quarantine();
        store.health = HEALTH_CORRUPT.to_string();
        store.last_error = Some(error);
        return store;
      }
    };
    if looks_like_v1(&raw) {
      let mut migrated = store.from_v1(&raw, false);
      migrated.load_kb();
      return migrated;
    }

    // 已存在的 runtime.json：读失败 → 静默重建（Option::None 通道），绝不传染 data.json
    let existing_runtime = store
      .backend
      .read_runtime()
      .ok()
      .flatten()
      .and_then(|text| parse_value(&text).ok());

    // v2 旧 schema → 一次性拆分（ADR-0005）：留档 → runtime 先写 → data 走轮转原子写
    let (mut data_value, runtime_value) = if is_legacy_schema(&raw) {
      let pair = split_schema(&raw, existing_runtime.as_ref());
      if let Err(error) = store.persist_split(&pair.0, &pair.1) {
        // 原件未动（pre-split 留档失败除外——那也没改 data.json）；
        // 本会话按拆分结果在内存里继续，下次保存即收敛，启动时不阻塞用户
        store.last_error = Some(format!(
          "运行状态拆分将在下次保存时自动完成（本次原因：{}）",
          error
        ));
      }
      pair
    } else {
      (raw, existing_runtime.unwrap_or_else(|| json!({})))
    };

    // 便签分离迁移（sticky-separation）：JSON 层归一历史便签，再进强类型解析
    let sticky_schema_changed = normalize_stickies_v3(&mut data_value);
    match serde_json::from_value::<AppData>(data_value) {
      Ok(mut data) => {
        data.settings.coerce();
        for task in data.tasks.iter_mut() {
          if !task.done && !task.in_trash() {
            task.carried_from = task.carried_from.max(carry_days(&task.due_at.clone()));
          }
        }
        store.data = data;
        // 运行态解析失败 = 静默重建为空（重复响一次的代价，不冻结数据）
        store.runtime = serde_json::from_value::<RuntimeState>(runtime_value)
          .unwrap_or_default();
        if sticky_schema_changed {
          store.runtime.migration = Some(MigrationReport {
            issues: vec![MigrationIssue {
              collection: "stickies".to_string(),
              id: "*".to_string(),
              field: "kind/hidden/pinned".to_string(),
              raw: "v2".to_string(),
              action: "便签分离迁移：待办镜像便签已丢弃；收起态升格为缩小态；置顶改为常驻无开关".to_string(),
            }],
            ..Default::default()
          });
          if let Err(error) = store.save() {
            store.last_error = Some(error);
            store.health = HEALTH_WRITE_FAILED.to_string();
          }
        }
        // 知识库：缺失/空文件 → 空库；读失败或 JSON 损坏 → 可恢复错误态（D8，绝不静默空库）
        store.load_kb();
        store
      }
      Err(error) => {
        store.corrupt_file = store.backend.quarantine();
        store.health = HEALTH_CORRUPT.to_string();
        store.last_error = Some(format!(
          "数据文件格式不对，已停止写入：{}",
          brief(&error.to_string())
        ));
        store
      }
    }
  }

  /// 知识库加载（D8）：notes.json / folders.json 任一读失败或解析失败 → 置 kb_error，
  /// 两个集合都清空（不展示半截数据），读取命令转「可恢复」错误；缺失 / 空文件 = 空库。
  /// 旧格式（内联 bodyMd）走一次性迁移（D7）：成功 = 索引 + 分文件正文；
  /// 迁移失败 = KB_UPGRADE_FAILED（旧 notes.json 原样保留，下次启动重试，绝不假空态）。
  fn load_kb(&mut self) {
    self.kb = Vec::new();
    self.folders = Vec::new();
    self.kb_error = None;
    self.kb_index_dirty = false;
    let notes_result = self.reload_kb_items();
    let folders_ok = self.reload_folders();
    match notes_result {
      Ok(()) if folders_ok => {
        // 迁移后的兼容（storage spec）：folderId 指向不存在的文件夹 → 回退未分类，条目绝不丢。
        // 只在 folders.json 读成功时执行；folders 损坏时不能把条目误判成未分类。
        self.fallback_missing_kb_folders();
        // D2 回填（best-effort）：已迁移但正文仍含内联 data:image 的库，启动时顺手外置；
        // 抽不动/写不进的单条保留内联，绝不丢图丢条目。
        self.backfill_inline_images();
        // 可修复差异（hash 被正文纠正 / folderId 回退 / 图片回填）：可写盘则顺手收敛索引；
        // 写不进去不算损坏，下次保存再落盘（D1）。
        if self.kb_index_dirty {
          let _ = self.save_notes();
        }
      }
      Ok(()) => {
        self.kb = Vec::new();
        self.folders = Vec::new();
        self.kb_error = Some(KB_CORRUPT_MESSAGE.to_string());
      }
      Err(message) => {
        self.kb = Vec::new();
        self.folders = Vec::new();
        self.kb_error = Some(message);
      }
    }
  }

  /// 读 notes.json 进内存；Ok = 正常（含文件缺失 / 空文件）。
  /// - 旧格式（任一元素内联 bodyMd/body_md）→ 一次性迁移（D7），失败置 KB_UPGRADE_FAILED；
  /// - 新格式 → 索引 meta + 逐条 notes/<id>.md 正文；正文缺失按空串（条目不丢、不判损坏）；
  /// - contentHash 与正文不一致：以正文为准（D1，可修复），只改内存并标记待落盘。
  fn reload_kb_items(&mut self) -> Result<(), String> {
    let text = match self.backend.read_notes() {
      Ok(None) => return Ok(()),
      Ok(Some(text)) if text.trim().is_empty() => return Ok(()),
      Ok(Some(text)) => text,
      Err(_) => return Err(KB_CORRUPT_MESSAGE.to_string()),
    };
    let value = match parse_value(&text) {
      Ok(value) => value,
      Err(_) => return Err(KB_CORRUPT_MESSAGE.to_string()),
    };
    if store_notes::is_legacy_index(&value) {
      return self.migrate_notes_legacy(&text);
    }
    let metas = match store_notes::parse_meta_index(value) {
      Ok(metas) => metas,
      Err(_) => return Err(KB_CORRUPT_MESSAGE.to_string()),
    };
    let mut items: Vec<KbItem> = Vec::with_capacity(metas.len());
    let mut dirty = false;
    for meta in metas {
      let body = match self.backend.read_note_body(&meta.id) {
        Ok(Some(body)) => body,
        Ok(None) => String::new(),
        Err(_) => return Err(KB_CORRUPT_MESSAGE.to_string()),
      };
      if store_notes::content_hash(&body) != meta.content_hash {
        dirty = true;
      }
      // item_with_body 按实际正文重算 contentHash：不一致时正文为准（不判损坏）
      items.push(store_notes::item_with_body(meta, body));
    }
    self.kb = items;
    self.kb_index_dirty = dirty;
    // 旧 notes.json 无 order：加载后按 updated_at 倒序补齐 dense，只改内存不改文件形态
    self.backfill_kb_item_orders();
    Ok(())
  }

  /// 旧格式 notes.json → 「索引 + notes/<id>.md」（D7 一次性迁移，幂等）。
  ///
  /// 顺序：① `notes.v1.json` 不存在才写（永久存档，回滚旧版本用）→ ② 轮转 5 份备份
  /// （copy 不是 move，旧 notes.json 原样保留）→ ③ 正文先逐条落盘（D2：正文里的内联
  /// data:image 同时外置到 assets/）→ ④ 最后原子写新索引。
  /// 失败（只读盘 / 空间不足）：不写新索引、旧 notes.json 不动、错误态 KB_UPGRADE_FAILED，
  /// 下次启动重试；绝不进入空库语义。
  fn migrate_notes_legacy(&mut self, legacy_json: &str) -> Result<(), String> {
    let value = parse_value(legacy_json).map_err(|_| KB_CORRUPT_MESSAGE.to_string())?;
    let mut items =
      store_notes::parse_legacy_items(&value).map_err(|_| KB_CORRUPT_MESSAGE.to_string())?;
    let failed = || crate::kb_errors::KB_UPGRADE_FAILED.to_string();
    if !self.backend.notes_v1_archive_exists() {
      self
        .backend
        .write_notes_v1_archive(legacy_json)
        .map_err(|_| failed())?;
    }
    self.backend.rotate_notes_backups().map_err(|_| failed())?;
    for item in &mut items {
      // D2：迁移时把旧正文里的内联图外置（先写 asset 再写正文）；抽不动的保留内联。
      let report = crate::kb::extract_images_report(&item.body_md);
      for asset in &report.assets {
        self
          .backend
          .write_asset(&asset.sha256, &asset.ext, &asset.bytes)
          .map_err(|_| failed())?;
      }
      if report.skipped > 0 {
        crate::telemetry::record_str(
          "kb_image_skipped",
          &[("count", &report.skipped.to_string())],
        );
      }
      if !report.assets.is_empty() {
        item.body_md = report.body;
        item.content_hash = Some(store_notes::content_hash(&item.body_md));
      }
      self
        .backend
        .write_note_body(&item.id, &item.body_md)
        .map_err(|_| failed())?;
    }
    let json = store_notes::index_json(&items).map_err(|_| failed())?;
    self.backend.write_notes(&json).map_err(|_| failed())?;
    self.kb = items;
    self.kb_index_dirty = false;
    self.backfill_kb_item_orders();
    Ok(())
  }

  /// folderId 指向不存在的文件夹 → 回退未分类（storage spec：MUST NOT 丢弃条目）。
  /// 只在 notes/folders 都读成功后调用；有修正则标记索引待落盘。
  fn fallback_missing_kb_folders(&mut self) {
    use std::collections::HashSet;
    let known: HashSet<String> = self
      .folders
      .iter()
      .map(|folder| folder.id.clone())
      .collect();
    let mut changed = false;
    for item in self.kb.iter_mut() {
      if let Some(folder_id) = item.folder_id.clone() {
        if !known.contains(&folder_id) {
          item.folder_id = None;
          changed = true;
        }
      }
    }
    if changed {
      self.backfill_kb_item_orders();
      self.kb_index_dirty = true;
    }
  }

  /// D2 回填（best-effort，幂等）：已迁移但正文里还留着内联 `data:image` 的条目，
  /// 启动加载时顺手把图片外置到 `assets/`，正文改成相对引用。
  ///
  /// - 正文不含 `data:` 直接跳过（零 IO）；
  /// - 写资源或写正文失败只跳过该条：正文保持内联，绝不丢图、绝不丢条目；
  /// - 成功则重算 contentHash 并标记索引待落盘（调用方随后 save_notes 统一收口）。
  fn backfill_inline_images(&mut self) {
    let mut changed = false;
    for item in self.kb.iter_mut() {
      if !item.body_md.contains("data:") {
        continue;
      }
      let (body, assets) = crate::kb::extract_images(&item.body_md);
      if assets.is_empty() {
        continue; // 没有可外置的图片（或全部抽不动）：正文原样保留
      }
      let mut assets_ok = true;
      for asset in &assets {
        if self
          .backend
          .write_asset(&asset.sha256, &asset.ext, &asset.bytes)
          .is_err()
        {
          assets_ok = false;
          break;
        }
      }
      if !assets_ok || self.backend.write_note_body(&item.id, &body).is_err() {
        continue; // 单条失败：保内联，不阻塞其它条目
      }
      item.body_md = body;
      item.content_hash = Some(store_notes::content_hash(&item.body_md));
      changed = true;
    }
    if changed {
      self.kb_index_dirty = true;
    }
  }

  /// 读 folders.json 进内存；true = 正常（含文件缺失 / 空文件，视为空表，零迁移）
  fn reload_folders(&mut self) -> bool {
    match self.backend.read_folders() {
      Ok(None) => true,
      Ok(Some(text)) if text.trim().is_empty() => true,
      Ok(Some(text)) => match parse_value(&text).and_then(|value| {
        serde_json::from_value::<Vec<KbFolder>>(value).map_err(|error| error.to_string())
      }) {
        Ok(folders) => {
          self.folders = folders;
          true
        }
        Err(_) => false,
      },
      Err(_) => false,
    }
  }

  /// 把 schema 拆分结果落盘（ADR-0005 步骤 3–5）：
  /// ① data.json 复制为 data.pre-split.json（已存在不覆盖，永不删除）
  /// ② runtime.json **先**写（原子）
  /// ③ data.json 走正常轮转 + 原子写 —— bak-1 就是拆分前的完整旧档
  fn persist_split(&mut self, data: &Value, runtime: &Value) -> Result<(), String> {
    let data_json = serde_json::to_string_pretty(data)
      .map_err(|error| format!("数据序列化失败：{}", brief(&error.to_string())))?;
    let runtime_json = serde_json::to_string_pretty(runtime)
      .map_err(|error| format!("运行态序列化失败：{}", brief(&error.to_string())))?;
    self.backend.archive_pre_split()?;
    self.backend.write_runtime(&runtime_json)?;
    self.backend.rotate_backups()?;
    self.backend.write_data(&data_json)
  }

  fn from_v1(mut self, raw: &Value, from_archive: bool) -> Store {
    if !from_archive && !self.backend.archive_exists() {
      // 迁移前原件永久留档（已存在则保留最早那份，绝不覆盖）
      if let Err(error) = self.backend.copy_data_to_archive() {
        self.health = HEALTH_CORRUPT.to_string();
        self.corrupt_file = self.backend.quarantine();
        self.last_error = Some(error);
        return self;
      }
    }
    match migrate_v1(raw) {
      Ok((mut data, report)) => {
        data.settings.coerce();
        self.runtime.migration = Some(report);
        // v1 的 fired 键是透明字符串，原样搬进运行态（跨重启不重复响）
        if let Some(items) = raw.get("fired").and_then(|value| value.as_array()) {
          for item in items.iter() {
            if let Some(key) = item.as_str() {
              if !self.runtime.fired.iter().any(|existing| existing == key) {
                self.runtime.fired.push(key.to_string());
              }
            }
          }
        }
        self.data = data;
        if let Err(error) = self.save() {
          self.last_error = Some(error);
          self.health = HEALTH_WRITE_FAILED.to_string();
        }
        self
      }
      Err(error) => {
        if !from_archive {
          self.corrupt_file = self.backend.quarantine();
        }
        self.health = HEALTH_CORRUPT.to_string();
        self.last_error = Some(error);
        self
      }
    }
  }

  // -------------------------------------------------------------- 查询

  pub fn is_corrupt(&self) -> bool {
    self.health == HEALTH_CORRUPT
  }

  /// 未恢复成功前所有写命令必须拒绝（杜绝静默空库）
  pub fn ensure_writable(&self) -> Result<(), String> {
    if self.is_corrupt() {
      return Err("数据文件已损坏，请先从备份恢复后再操作".to_string());
    }
    if self.health == HEALTH_WRITE_FAILED {
      return Err(write_error(self.last_error.as_deref()));
    }
    Ok(())
  }

  /// 知识库不可用时拒绝读写（D8）：返回可恢复文案，绝不静默按空库继续。
  pub fn ensure_kb_ok(&self) -> Result<(), String> {
    match &self.kb_error {
      Some(text) => Err(text.clone()),
      None => Ok(()),
    }
  }

  pub fn tasks(&self, include_deleted: bool) -> Vec<Task> {
    if include_deleted {
      return self.data.tasks.clone();
    }
    self
      .data
      .tasks
      .iter()
      .filter(|task| !task.in_trash())
      .cloned()
      .collect()
  }

  pub fn find(&self, id: &str) -> Option<&Task> {
    self.data.tasks.iter().find(|task| task.id == id)
  }

  pub fn find_index(&self, id: &str) -> Option<usize> {
    self.data.tasks.iter().position(|task| task.id == id)
  }

  /// 同列表手动排序（便签规格 §12.2）：按传入顺序落 sort_order = 下标。
  /// 只动列表里出现的 id；未传到的条目保持原值（前端整表传，天然全覆盖）。
  pub fn reorder_tasks(&mut self, ids_in_order: &[String]) -> Result<usize, String> {
    if ids_in_order.is_empty() {
      return Err("没有要排序的条目".to_string());
    }
    let mut moved = 0usize;
    for (index, id) in ids_in_order.iter().enumerate() {
      if let Some(task) = self.data.tasks.iter_mut().find(|t| &t.id == id) {
        task.sort_order = Some(index as i64);
        task.updated_at = crate::models::now_text();
        moved += 1;
      }
    }
    if moved == 0 {
      return Err("没有匹配到任何条目，可能已被删除".to_string());
    }
    Ok(moved)
  }

  // ---------------------------------------------------------------- 便签（多便签 H1）

  pub fn stickies(&self) -> Vec<crate::models::StickyNote> {
    self.data.stickies.clone()
  }

  pub fn find_sticky(&self, id: &str) -> Option<crate::models::StickyNote> {
    self.data.stickies.iter().find(|s| s.id == id).cloned()
  }

  pub fn add_sticky(&mut self, mut note: crate::models::StickyNote) -> crate::models::StickyNote {
    note.created_at = crate::models::now_text();
    note.updated_at = note.created_at.clone();
    self.data.stickies.push(note.clone());
    note
  }

  pub fn find_sticky_mut(&mut self, id: &str) -> Option<&mut crate::models::StickyNote> {
    self.data.stickies.iter_mut().find(|s| s.id == id)
  }

  pub fn delete_sticky(&mut self, id: &str) -> Result<(), String> {
    let before = self.data.stickies.len();
    self.data.stickies.retain(|s| s.id != id);
    if self.data.stickies.len() == before {
      return Err("找不到这张便签，可能已经被删除".to_string());
    }
    // 位置记录一并回收（ADR-0007：runtime.json 不无限增长）
    self.runtime.note_pos.remove(id);
    Ok(())
  }

  pub fn find_reminder(&self, id: &str) -> Option<&Reminder> {
    self
      .data
      .reminders
      .iter()
      .find(|item| item.id == id)
  }

  pub fn find_reminder_index(&self, id: &str) -> Option<usize> {
    self.data.reminders.iter().position(|item| item.id == id)
  }

  pub fn backups(&self) -> Vec<BackupInfo> {
    self.backend.list_backups()
  }

  pub fn health_view(&self) -> DataHealthV2 {
    DataHealthV2 {
      health: self.health.clone(),
      backups: self.backups(),
      corrupt_file: self.corrupt_file.clone(),
      last_error: self.last_error.clone(),
      writable: !self.is_corrupt(),
    }
  }

  // -------------------------------------------------------------- 写盘

  /// 读—改—内存—序列化 tmp—fs::rename 原子替换；覆盖写前轮转备份
  pub fn save(&mut self) -> Result<(), String> {
    self.ensure_writable()?;
    let data_json = self.serialized()?;
    let runtime_json = self.serialized_runtime()?;
    self.backend.rotate_backups()?;
    self.backend.write_data(&data_json)?;
    self.backend.write_runtime(&runtime_json)
  }

  /// 只写运行态：窗口位置这类不改用户数据的高频写（runtime.json 不轮转）
  pub fn save_runtime_only(&mut self) -> Result<(), String> {
    let runtime_json = self.serialized_runtime()?;
    self.backend.write_runtime(&runtime_json)
  }

  /// 只写知识库索引（notes.json，原子写；写前轮转 5 份备份，D8）。
  /// D1：正文单独存 notes/<id>.md（命令层先 save_kb_item_body），索引只存元数据，不含正文/图片。
  pub fn save_notes(&mut self) -> Result<(), String> {
    self.ensure_writable()?;
    self.ensure_kb_ok()?;
    let json = store_notes::index_json(&self.kb)?;
    self.backend.rotate_notes_backups()?;
    self.backend.write_notes(&json)?;
    self.kb_index_dirty = false;
    Ok(())
  }

  /// 写单条正文（D1：先正文后索引；索引由调用方随后 save_notes 落盘）。
  /// D2：正文里的内联 Markdown 图片先外置到 `assets/<sha>.<ext>`（经 StoreBackend），
  /// 正文只留相对引用；内存里的 bodyMd / contentHash 用改写后的正文。
  pub fn save_kb_item_body(&mut self, id: &str, body: &str) -> Result<(), String> {
    self.ensure_writable()?;
    self.ensure_kb_ok()?;
    let position = self
      .kb
      .iter()
      .position(|item| item.id == id)
      .ok_or_else(|| "找不到这条资料，可能已经被删除".to_string())?;
    let report = crate::kb::extract_images_report(body);
    // 顺序纪律：先写资源，再写正文（正文一旦指向 assets/，文件必须已经在）。
    for asset in &report.assets {
      self
        .backend
        .write_asset(&asset.sha256, &asset.ext, &asset.bytes)?;
    }
    if report.skipped > 0 {
      crate::telemetry::record_str("kb_image_skipped", &[("count", &report.skipped.to_string())]);
    }
    let body = report.body;
    self.backend.write_note_body(id, &body)?;
    self.kb[position].body_md = body.clone();
    self.kb[position].content_hash = Some(store_notes::content_hash(&body));
    Ok(())
  }

  /// 资源目录绝对路径（Bootstrap.assetsDir：前端拼 `convertFileSrc` 路径用）。
  pub fn assets_dir(&self) -> String {
    self.backend.assets_dir_abs()
  }

  /// 删除单条正文（D1：删除顺序 = 先删正文、再写索引，索引不会长期指向缺失正文）
  pub fn delete_kb_item_body(&mut self, id: &str) -> Result<(), String> {
    self.ensure_writable()?;
    self.ensure_kb_ok()?;
    self.backend.delete_note_body(id)
  }

  /// 只写文件夹（folders.json，原子写；条目量小，不需要轮转）
  pub fn save_folders(&mut self) -> Result<(), String> {
    self.ensure_writable()?;
    self.ensure_kb_ok()?;
    let json = serde_json::to_string_pretty(&self.folders)
      .map_err(|error| format!("文件夹序列化失败：{}", brief(&error.to_string())))?;
    self.backend.write_folders(&json)
  }

  fn serialized_runtime(&self) -> Result<String, String> {
    serde_json::to_string_pretty(&self.runtime)
      .map_err(|error| format!("运行态序列化失败：{}", brief(&error.to_string())))
  }

  fn serialized(&self) -> Result<String, String> {
    serde_json::to_string_pretty(&self.data)
      .map_err(|error| format!("数据序列化失败，本次没有保存：{}", brief(&error.to_string())))
  }

  /// 只写不轮转（回滚备份时使用，避免挤掉刚选中的备份）
  fn write_in_place(&mut self) -> Result<(), String> {
    let json = self.serialized()?;
    self.backend.write_data(&json)?;
    self.health = HEALTH_OK.to_string();
    self.last_error = None;
    Ok(())
  }

  /// 从第 slot 份备份回滚；成功后解除 corrupt 封锁
  pub fn restore_backup(&mut self, slot: &str) -> Result<usize, String> {
    let digits: String = slot
      .trim()
      .chars()
      .filter(|ch| ch.is_ascii_digit())
      .collect();
    let number: usize = digits
      .parse()
      .map_err(|_| "请指定 1 到 5 之间的备份编号".to_string())?;
    if number == 0 || number > BACKUP_SLOTS {
      return Err("备份只有 1 到 5 号，请重新选择".to_string());
    }
    let raw = self
      .backend
      .read_backup(number as u32)
      .and_then(|text| parse_value(&text))
      .map_err(|_| "这个备份也读不了，换一个试试".to_string())?;
    let mut restored = match looks_like_v1(&raw) {
      true => {
        let (data, _report) = migrate_v1(&raw).map_err(|_| "这个备份无法还原成可用数据".to_string())?;
        data
      }
      false => serde_json::from_value::<AppData>(raw).map_err(|_| "这个备份无法还原成可用数据".to_string())?,
    };
    restored.settings.coerce();
    self.data = restored;
    self.undo.clear();
    // 损坏原件早已另存为 data.corrupt.*，这里覆盖写但不轮转
    self.write_in_place()?;
    Ok(self.tasks(true).into_iter().filter(|task| !task.in_trash()).count())
  }

  // ---------------------------------------------- 知识库：文件夹（D6）+ notes 备份恢复（D8）

  pub fn list_folders(&self) -> Vec<KbFolder> {
    self.folders.clone()
  }

  pub fn folder_exists(&self, id: &str) -> bool {
    self.folders.iter().any(|folder| folder.id == id)
  }

  /// 三态 folderId 解析（KbPayload 语义）：None = 不改（返回 None 由调用方判）；
  /// Some("") = 移到未分类；Some(id) = 校验存在后归到该文件夹。
  pub fn folder_target(&self, raw: Option<&str>) -> Result<Option<String>, String> {
    match raw {
      None => Ok(None),
      Some(text) if text.trim().is_empty() => Ok(None),
      Some(id) => {
        if self.folder_exists(id) {
          Ok(Some(id.to_string()))
        } else {
          Err("文件夹不存在".to_string())
        }
      }
    }
  }

  /// 旧数据排序补齐：凡 order 为 None 的同级条目，按 updated_at 倒序生成 dense 0..n-1。
  /// 只在内存生成（notes.json 文件格式不变），下一次 save_notes 自然落盘。
  fn backfill_kb_item_orders(&mut self) {
    use std::collections::HashMap;
    let mut groups: HashMap<Option<String>, Vec<usize>> = HashMap::new();
    for (index, item) in self.kb.iter().enumerate() {
      groups.entry(item.folder_id.clone()).or_default().push(index);
    }
    for indices in groups.values() {
      if indices.iter().all(|&index| self.kb[index].order.is_some()) {
        continue;
      }
      let mut sorted = indices.clone();
      sorted.sort_by(|&left, &right| {
        let first = &self.kb[left];
        let second = &self.kb[right];
        first
          .order
          .unwrap_or(i64::MAX)
          .cmp(&second.order.unwrap_or(i64::MAX))
          .then_with(|| second.updated_at.cmp(&first.updated_at))
          .then_with(|| first.id.cmp(&second.id))
      });
      for (position, index) in sorted.iter().enumerate() {
        self.kb[*index].order = Some(position as i64);
      }
    }
  }

  /// 同级文件夹 id（按 order 升序，order 相同按名称再按 id 兜底；exclude 排除正在移动的自身）
  fn folder_sibling_ids(&self, parent_id: Option<&str>, exclude: Option<&str>) -> Vec<String> {
    let mut siblings: Vec<&KbFolder> = self
      .folders
      .iter()
      .filter(|folder| folder.parent_id.as_deref() == parent_id)
      .filter(|folder| exclude != Some(folder.id.as_str()))
      .collect();
    siblings.sort_by(|first, second| {
      first
        .order
        .cmp(&second.order)
        .then_with(|| first.name.cmp(&second.name))
        .then_with(|| first.id.cmp(&second.id))
    });
    siblings.into_iter().map(|folder| folder.id.clone()).collect()
  }

  /// 同级条目 id（未分类 = folder_id None；order 缺省排最后，同值按 updated_at 倒序）
  fn kb_item_sibling_ids(&self, folder_id: Option<&str>, exclude: Option<&str>) -> Vec<String> {
    let mut siblings: Vec<&KbItem> = self
      .kb
      .iter()
      .filter(|item| item.folder_id.as_deref() == folder_id)
      .filter(|item| exclude != Some(item.id.as_str()))
      .collect();
    siblings.sort_by(|first, second| {
      first
        .order
        .unwrap_or(i64::MAX)
        .cmp(&second.order.unwrap_or(i64::MAX))
        .then_with(|| second.updated_at.cmp(&first.updated_at))
        .then_with(|| first.id.cmp(&second.id))
    });
    siblings.into_iter().map(|item| item.id.clone()).collect()
  }

  /// 按给定 id 顺序把文件夹 order 写成 dense 0..n-1
  fn set_folder_orders(&mut self, ids: &[String]) {
    for (position, id) in ids.iter().enumerate() {
      if let Some(folder) = self.folders.iter_mut().find(|folder| &folder.id == id) {
        folder.order = position as i64;
      }
    }
  }

  /// 按给定 id 顺序把条目 order 写成 dense 0..n-1
  fn set_kb_item_orders(&mut self, ids: &[String]) {
    for (position, id) in ids.iter().enumerate() {
      if let Some(item) = self.kb.iter_mut().find(|item| &item.id == id) {
        item.order = Some(position as i64);
      }
    }
  }

  /// 条目落位（kb-file-tree D4）：folder 为 folder_target 的三态解析结果
  /// （None = 不改；Some(None) = 未分类；Some(Some(id)) = 该文件夹）。
  /// order = 目标同级索引（越界 clamp 到 0..len，缺省且发生移动时追加末尾），pinned 直接置位。
  /// 归属或顺序变化时对旧父级与新父级做 dense 重排（0..n-1）。
  pub fn place_kb_item(
    &mut self,
    id: &str,
    folder: Option<Option<String>>,
    order: Option<i64>,
    pinned: Option<bool>,
  ) -> Result<KbItem, String> {
    let index = self
      .kb
      .iter()
      .position(|item| item.id == id)
      .ok_or_else(|| "找不到这条资料，可能已经被删除".to_string())?;
    let old_folder = self.kb[index].folder_id.clone();
    let new_folder = match folder {
      None => old_folder.clone(),
      Some(target) => target,
    };
    if let Some(flag) = pinned {
      self.kb[index].pinned = flag;
    }
    if new_folder != old_folder || order.is_some() {
      if new_folder != old_folder {
        // 旧父级移除后 dense 重排
        let old_ids = self.kb_item_sibling_ids(old_folder.as_deref(), Some(id));
        self.set_kb_item_orders(&old_ids);
        self.kb[index].folder_id = new_folder.clone();
      }
      // 新父级插入到目标索引后整体 dense 重排；缺省索引 = 追加末尾
      let mut target_ids = self.kb_item_sibling_ids(new_folder.as_deref(), Some(id));
      let position = match order {
        Some(value) => value.clamp(0, target_ids.len() as i64) as usize,
        None => target_ids.len(),
      };
      target_ids.insert(position, id.to_string());
      self.set_kb_item_orders(&target_ids);
    }
    Ok(self.kb[index].clone())
  }

  fn folder_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
      return Err("文件夹名称不能为空".to_string());
    }
    if trimmed.chars().count() > FOLDER_NAME_MAX_CHARS {
      return Err("文件夹名称最多 40 个字".to_string());
    }
    Ok(trimmed.to_string())
  }

  pub fn create_folder(
    &mut self,
    name: &str,
    parent_id: Option<&str>,
  ) -> Result<KbFolder, String> {
    let name = Self::folder_name(name)?;
    // 空串父级按「根」处理（前端清空下拉框时会传 ""）
    let parent = parent_id.filter(|id| !id.trim().is_empty());
    if let Some(id) = parent {
      if !self.folder_exists(id) {
        return Err("文件夹不存在".to_string());
      }
    }
    // 同级 dense 追加：order = 同级数量（空父级 = 0），新文件夹不置顶
    let order = self
      .folders
      .iter()
      .filter(|folder| folder.parent_id.as_deref() == parent)
      .count() as i64;
    let folder = KbFolder {
      id: new_id("f"),
      name,
      parent_id: parent.map(|id| id.to_string()),
      order,
      pinned: false,
    };
    self.folders.push(folder.clone());
    Ok(folder)
  }

  /// 文件夹更新（kb-file-tree D4）：改名 / 移动 / 同级排序 / 置顶统一入口。
  /// parent_id 三态：None = 不改；Some("") = 移到顶层（根）；Some(id) = 目标文件夹
  /// （必须存在，且不能是自身或后代）。order = 目标同级索引（越界 clamp 到 0..len，
  /// 缺省且发生移动时追加末尾）。移动或 order 变化后对旧父级与新父级做 dense 重排（0..n-1）。
  pub fn update_folder(
    &mut self,
    id: &str,
    name: Option<String>,
    parent_id: Option<String>,
    order: Option<i64>,
    pinned: Option<bool>,
  ) -> Result<KbFolder, String> {
    let index = self
      .folders
      .iter()
      .position(|folder| folder.id == id)
      .ok_or_else(|| "文件夹不存在".to_string())?;
    let name = match name {
      Some(raw) => Some(Self::folder_name(&raw)?),
      None => None,
    };
    let old_parent = self.folders[index].parent_id.clone();
    let new_parent = match parent_id {
      None => old_parent.clone(),
      // 前端「移到顶层」传 ""（清空父级下拉框），按根处理
      Some(raw) if raw.trim().is_empty() => None,
      Some(raw) => {
        let target = raw.trim().to_string();
        if !self.folder_exists(&target) {
          return Err("文件夹不存在".to_string());
        }
        // 环校验：沿目标父级的 parentId 链上溯，命中自身即拒绝（步数兜底防旧数据成环死循环）
        let mut cursor = Some(target.clone());
        let mut steps = 0usize;
        while let Some(current) = cursor {
          if current == id {
            return Err("不能把文件夹移动到自身或它的子文件夹".to_string());
          }
          cursor = self
            .folders
            .iter()
            .find(|folder| folder.id == current)
            .and_then(|folder| folder.parent_id.clone());
          steps += 1;
          if steps > self.folders.len() {
            break;
          }
        }
        Some(target)
      }
    };
    let moved = new_parent != old_parent;
    {
      let folder = &mut self.folders[index];
      if let Some(value) = name {
        folder.name = value;
      }
      if let Some(flag) = pinned {
        folder.pinned = flag;
      }
    }
    if moved || order.is_some() {
      if moved {
        // 旧父级移除后 dense 重排
        let old_ids = self.folder_sibling_ids(old_parent.as_deref(), Some(id));
        self.set_folder_orders(&old_ids);
        self.folders[index].parent_id = new_parent.clone();
      }
      // 新父级插入到目标索引后整体 dense 重排；缺省索引 = 追加末尾
      let mut target_ids = self.folder_sibling_ids(new_parent.as_deref(), Some(id));
      let position = match order {
        Some(value) => value.clamp(0, target_ids.len() as i64) as usize,
        None => target_ids.len(),
      };
      target_ids.insert(position, id.to_string());
      self.set_folder_orders(&target_ids);
    }
    Ok(self.folders[index].clone())
  }

  /// 删除空文件夹：仍有条目引用或有子文件夹时拒绝（D6 删空语义）
  pub fn delete_folder(&mut self, id: &str) -> Result<(), String> {
    if !self.folder_exists(id) {
      return Err("文件夹不存在".to_string());
    }
    let has_children = self
      .folders
      .iter()
      .any(|folder| folder.parent_id.as_deref() == Some(id));
    let has_items = self
      .kb
      .iter()
      .any(|item| item.folder_id.as_deref() == Some(id));
    if has_children || has_items {
      return Err("文件夹非空，请先移走其中的条目或子文件夹".to_string());
    }
    self.folders.retain(|folder| folder.id != id);
    Ok(())
  }

  /// notes.json.bak-1..5 列表（恢复页）
  pub fn list_kb_backups(&self) -> Vec<KbBackupInfo> {
    self.backend.list_kb_backups()
  }

  /// 从第 slot 份 notes 备份恢复（D8）。兼容两种备份格式（D7）：
/// - 旧格式（元素内联 bodyMd）→ 先轮转当前 notes，再走与启动迁移同一个
///   `migrate_notes_legacy`（其内部顺序 = 存档 → 轮转 → 正文 → 新索引，不重复轮转）；
/// - 新格式索引 → 按索引逐条从当前 notes/<id>.md 取正文（缺失按空串），再轮转 + 原子写索引。
  /// 解析成功才动盘，失败返回可换一个备份的中文短句。
  pub fn restore_kb_backup(&mut self, slot: &str) -> Result<usize, String> {
    let digits: String = slot
      .trim()
      .chars()
      .filter(|ch| ch.is_ascii_digit())
      .collect();
    let number: usize = digits
      .parse()
      .map_err(|_| "请指定 1 到 5 之间的备份编号".to_string())?;
    if number == 0 || number > BACKUP_SLOTS {
      return Err("备份只有 1 到 5 号，请重新选择".to_string());
    }
    let text = self
      .backend
      .read_notes_backup(number as u32)
      .map_err(|_| "这个备份也读不了，换一个试试".to_string())?;
    let value = parse_value(&text)
      .map_err(|_| "这个备份无法还原成可用的知识库".to_string())?;
    let count = if store_notes::is_legacy_index(&value) {
      self
        .migrate_notes_legacy(&text)
        .map_err(|_| "这个备份无法还原成可用的知识库".to_string())?;
      self.kb.len()
    } else {
      let metas = store_notes::parse_meta_index(value)
        .map_err(|_| "这个备份无法还原成可用的知识库".to_string())?;
      let mut items: Vec<KbItem> = Vec::with_capacity(metas.len());
      for meta in metas {
        let body = match self.backend.read_note_body(&meta.id) {
          Ok(Some(body)) => body,
          Ok(None) => String::new(),
          Err(_) => return Err("这个备份无法还原成可用的知识库".to_string()),
        };
        items.push(store_notes::item_with_body(meta, body));
      }
      let json = store_notes::index_json(&items)
        .map_err(|_| "这个备份无法还原成可用的知识库".to_string())?;
      self.backend.rotate_notes_backups()?;
      self.backend.write_notes(&json)?;
      let count = items.len();
      self.kb = items;
      self.kb_index_dirty = false;
      self.backfill_kb_item_orders();
      count
    };
    self.kb_error = None;
    // folders.json 独立于 notes：若它仍损坏，保持错误态（绝不静默空文件夹）。
    // 读成功时再做 folderId 失效回退（恢复的旧备份可能引用已删除文件夹）。
    if !self.reload_folders() {
      self.folders = Vec::new();
      self.kb_error = Some(KB_CORRUPT_MESSAGE.to_string());
    } else {
      self.fallback_missing_kb_folders();
    }
    Ok(count)
  }

  /// 前端读过迁移报告后清掉，避免每次启动重复弹提示
  pub fn clear_migration(&mut self) {
    self.runtime.migration = None;
  }

  /// schema 回滚（仅调试入口，ADR-0005）：pre-split 原件回 data.json + 删 runtime.json。
  /// 回滚窗口期内新增提醒的 fired 键会丢 → 后果仅是"可能重复响一次"。
  pub fn rollback_schema_split(&mut self) -> Result<(), String> {
    self.backend.restore_pre_split()?;
    self.backend.remove_runtime()?;
    self.health = HEALTH_OK.to_string();
    self.last_error = None;
    Ok(())
  }

  /// 启动时物理清理超过保留期的软删任务（含其子任务与备注）
  pub fn purge_expired(&mut self, days: i64) -> usize {
    let limit = self.clock_now() - Duration::days(days);
    let mut purged_ids: Vec<String> = Vec::new();
    self.data.tasks.retain(|task| {
      let keep = match &task.deleted_at {
        None => true,
        Some(stamp) => match parse_wall(stamp).and_then(|wall| to_local(&wall)) {
          Some(when) => when > limit,
          None => true,
        },
      };
      if !keep {
        purged_ids.push(task.id.clone());
      }
      keep
    });
    if !purged_ids.is_empty() {
      // fired 键是 t|{id}|{stamp}，id 在第二段；只回收被清任务自己的键
      self.runtime.fired.retain(|key| match key.split('|').nth(1) {
        Some(id) => !purged_ids.iter().any(|value| value == id),
        None => true,
      });
    }
    purged_ids.len()
  }

  // -------------------------------------------------------------- 领域写操作

  pub fn push_undo(&mut self, entry: Undoable) {
    self.undo.push(entry);
    while self.undo.len() > 50 {
      self.undo.remove(0);
    }
  }

  /// 软删除：只写 deletedAt，返回被删的完整对象；子任务与备注原样保留
  pub fn soft_delete_task(&mut self, id: &str) -> Result<Task, String> {
    let index = self
      .find_index(id)
      .ok_or_else(|| "找不到这条待办，可能已经被删除".to_string())?;
    if self.data.tasks[index].in_trash() {
      return Err("这条待办已经在回收站里".to_string());
    }
    let stamp = self.stamp();
    {
      let task = &mut self.data.tasks[index];
      task.deleted_at = Some(stamp.clone());
      task.updated_at = stamp;
    }
    let snapshot = self.data.tasks[index].clone();
    self.push_undo(Undoable::Task(Box::new(snapshot.clone())));
    Ok(snapshot)
  }

  /// 撤销最近一次删除（任务 / 子任务 / 备注），并兼容重启后的软删回收
  pub fn undo_last(&mut self) -> Result<Option<Task>, String> {
    let entry = match self.undo.pop() {
      Some(value) => value,
      None => {
        let candidate = self
          .data
          .tasks
          .iter()
          .filter(|task| task.in_trash())
          .max_by(|left, right| left.deleted_at.cmp(&right.deleted_at))
          .map(|task| task.id.clone());
        return match candidate {
          Some(id) => Ok(Some(self.restore_task(&id)?)),
          None => Err("没有可撤销的删除了".to_string()),
        };
      }
    };
    match entry {
      Undoable::Task(snapshot) => {
        let stamp = self.stamp();
        let index = self.find_index(&snapshot.id);
        match index {
          Some(position) => {
            let mut restored = *snapshot;
            restored.deleted_at = None;
            restored.updated_at = stamp;
            self.data.tasks[position] = restored.clone();
            Ok(Some(restored))
          }
          None => {
            let mut restored = *snapshot;
            restored.deleted_at = None;
            restored.updated_at = stamp;
            self.data.tasks.push(restored.clone());
            Ok(Some(restored))
          }
        }
      }
      Undoable::Subtask { task_id, subtask } => {
        let index = self
          .find_index(&task_id)
          .ok_or_else(|| "原任务已经不在了，子任务无法放回".to_string())?;
        if !self.data.tasks[index]
          .subtasks
          .iter()
          .any(|item| item.id == subtask.id)
        {
          self.data.tasks[index].subtasks.push(subtask);
        }
        self.data.tasks[index].updated_at = self.stamp();
        Ok(Some(self.data.tasks[index].clone()))
      }
      Undoable::Note { task_id, note } => {
        let index = self
          .find_index(&task_id)
          .ok_or_else(|| "原任务已经不在了，备注无法放回".to_string())?;
        if !self.data.tasks[index].notes.iter().any(|item| item.id == note.id) {
          self.data.tasks[index].notes.push(note);
        }
        self.data.tasks[index].updated_at = self.stamp();
        Ok(Some(self.data.tasks[index].clone()))
      }
    }
  }

  /// 从回收站恢复指定任务
  pub fn restore_task(&mut self, id: &str) -> Result<Task, String> {
    let index = self.find_index(id).ok_or_else(|| "找不到这条待办".to_string())?;
    if !self.data.tasks[index].in_trash() {
      return Err("这条待办不在回收站".to_string());
    }
    self.data.tasks[index].deleted_at = None;
    self.data.tasks[index].updated_at = self.stamp();
    self
      .undo
      .retain(|entry| !matches!(entry, Undoable::Task(boxed) if boxed.id == id));
    Ok(self.data.tasks[index].clone())
  }

  /// 彻底清除（只允许对回收站里的任务）
  pub fn purge_task(&mut self, id: &str) -> Result<Task, String> {
    let index = self.find_index(id).ok_or_else(|| "找不到这条待办".to_string())?;
    if !self.data.tasks[index].in_trash() {
      return Err("请先删除这条待办，再彻底清除".to_string());
    }
    let removed = self.data.tasks.remove(index);
    // 用键生成器本身造前缀，格式永远与写入侧一致（task_key 是 t|{id}|{stamp}）
    let prefix = task_key(&removed.id, "");
    self.runtime.fired.retain(|key| !key.starts_with(&prefix));
    self
      .undo
      .retain(|entry| !matches!(entry, Undoable::Task(boxed) if boxed.id == id));
    Ok(removed)
  }

  /// 勾选 / 取消勾选（doneAt 只在真正完成时写本地时刻）
  pub fn toggle_task(&mut self, id: &str) -> Result<Task, String> {
    let index = self
      .find_index(id)
      .ok_or_else(|| "找不到这条待办，可能已经被删除".to_string())?;
    if self.data.tasks[index].in_trash() {
      return Err("回收站里的待办不能勾选，请先恢复".to_string());
    }
    let stamp = self.stamp();
    {
      let task = &mut self.data.tasks[index];
      task.done = !task.done;
      if task.done {
        task.done_at = Some(stamp.clone());
        task.legacy = false;
      } else {
        task.done_at = None;
      }
      task.updated_at = stamp;
    }
    Ok(self.data.tasks[index].clone())
  }

  pub fn toggle_subtask(&mut self, task_id: &str, subtask_id: &str) -> Result<Task, String> {
    let index = self.find_index(task_id).ok_or_else(|| "找不到这条待办".to_string())?;
    let position = self.data.tasks[index]
      .subtasks
      .iter()
      .position(|item| item.id == subtask_id)
      .ok_or_else(|| "找不到这个子任务".to_string())?;
    self.data.tasks[index].subtasks[position].done = !self.data.tasks[index].subtasks[position].done;
    self.data.tasks[index].updated_at = self.stamp();
    Ok(self.data.tasks[index].clone())
  }

  // -------------------------------------------------------------- patch 合并

  pub fn apply_task_patch(task: &mut Task, patch: &TaskPayload) -> Result<(), String> {
    if let Some(value) = &patch.title {
      if value.trim().is_empty() {
        return Err("标题不能为空".to_string());
      }
      task.title = value.trim().to_string();
    }
    if let Some(value) = &patch.note {
      task.note = value.trim().to_string();
    }
    if let Some(value) = &patch.category {
      if !CATEGORIES.contains(&value.as_str()) {
        return Err("分类只能是 工作 / 学习 / 生活".to_string());
      }
      task.category = value.clone();
    }
    if let Some(value) = &patch.kb_refs {
      // 单向引用集合整体替换（前端以「当前挂载列表」提交，避免增量同步复杂度）
      task.kb_refs = value.clone();
    }
    if let Some(value) = &patch.priority {
      if !PRIORITIES.contains(&value.as_str()) {
        return Err("优先级只能是 high / med / low".to_string());
      }
      task.priority = value.clone();
    }
    if let Some(value) = &patch.due_at {
      let next = patch_datetime(value);
      if value.is_string() && next.is_none() {
        return Err("截止日期格式不对，请用 YYYY-MM-DD".to_string());
      }
      task.due_at = next;
    }
    if let Some(value) = &patch.remind_at {
      let next = patch_datetime(value);
      if value.is_string() && next.is_none() {
        return Err("提醒时间格式不对，请用 YYYY-MM-DDTHH:mm".to_string());
      }
      task.remind_at = next;
    }
    if let Some(value) = &patch.planned_date {
      let next = match value {
        Value::Null => None,
        other => match patch_text(other) {
          None => None,
          Some(text) => normalize_date_text(&text),
        },
      };
      if value.is_string() && next.is_none() && patch_text(value).is_some() {
        return Err("计划日期格式不对，请用 YYYY-MM-DD".to_string());
      }
      task.planned_date = next;
    }
    if let Some(value) = patch.carried_from {
      task.carried_from = value.min(9999);
    }
    if let Some(value) = &patch.done_at {
      let next = patch_datetime(value);
      if value.is_string() && next.is_none() {
        return Err("完成时刻格式不对，请用 YYYY-MM-DDTHH:mm".to_string());
      }
      task.done_at = next;
    }
    if let Some(value) = &patch.deleted_at {
      task.deleted_at = patch_datetime(value);
    }
    if let Some(value) = patch.legacy {
      task.legacy = value;
    }
    if let Some(values) = &patch.subtasks {
      let mut cleaned: Vec<Subtask> = Vec::new();
      for item in values {
        if item.title.trim().is_empty() {
          continue;
        }
        cleaned.push(Subtask {
          id: if item.id.trim().is_empty() {
            new_id("s")
          } else {
            item.id.clone()
          },
          title: item.title.trim().to_string(),
          done: item.done,
        });
      }
      task.subtasks = cleaned;
    }
    if let Some(values) = &patch.notes {
      let mut cleaned: Vec<Note> = Vec::new();
      for item in values {
        if item.content.trim().is_empty() && item.author.trim().is_empty() {
          continue;
        }
        cleaned.push(Note {
          id: if item.id.trim().is_empty() {
            new_id("n")
          } else {
            item.id.clone()
          },
          author: item.author.clone(),
          content: item.content.clone(),
          created_at: normalize_datetime(&item.created_at).unwrap_or_else(now_text),
        });
      }
      task.notes = cleaned;
    }
    if let Some(value) = patch.source {
      task.source = value;
    }
    if let Some(value) = &patch.created_at {
      if let Some(normalized) = normalize_datetime(value) {
        task.created_at = normalized;
      }
    }
    if let Some(done) = patch.done {
      let stamp = now_text();
      task.done = done;
      if done {
        if task.done_at.is_none() {
          task.done_at = Some(stamp.clone());
        }
        task.legacy = false;
      } else {
        task.done_at = None;
      }
      drop(stamp);
    }
    Ok(())
  }

  pub fn apply_reminder_patch(reminder: &mut Reminder, patch: &ReminderPayload) -> Result<(), String> {
    if let Some(value) = &patch.title {
      if value.trim().is_empty() {
        return Err("提醒名不能为空".to_string());
      }
      reminder.title = value.trim().to_string();
    }
    if let Some(value) = &patch.time {
      let trimmed = value.trim();
      if trimmed.is_empty() {
        reminder.time = String::new();
      } else if normalize_datetime(trimmed).is_none() && clocks_of(trimmed).is_empty() {
        return Err("提醒时间没设好，请重新选一次时间".to_string());
      } else {
        reminder.time = trimmed.to_string();
      }
    }
    if let Some(value) = &patch.category {
      if !CATEGORIES.contains(&value.as_str()) {
        return Err("分类只能是 工作 / 学习 / 生活".to_string());
      }
      reminder.category = value.clone();
    }
    if let Some(value) = patch.enabled {
      reminder.enabled = value;
    }
    if let Some(value) = patch.completed {
      reminder.completed = value;
      reminder.enabled = !value;
    }
    if let Some(value) = patch.repeat {
      reminder.repeat = value;
    }
    if let Some(value) = &patch.snoozed_until {
      reminder.snoozed_until = normalize_datetime(value);
    }
    Ok(())
  }

  pub fn apply_settings_patch(&mut self, patch: &SettingsPayload) -> Result<(), String> {
    let default_capture = crate::models::DEFAULT_HOTKEY.to_string();
    {
      let settings = &mut self.data.settings;
      if let Some(value) = &patch.theme {
        if value.trim().is_empty() {
          return Err("主题名不能为空".to_string());
        }
        settings.theme = value.trim().to_string();
      }
      if let Some(value) = &patch.sticky_paper {
        let trimmed = value.trim();
        if !STICKY_PAPERS.contains(&trimmed) {
          return Err("便签纸色只能是 warm / kraft / cyan / ink".to_string());
        }
        settings.sticky_paper = trimmed.to_string();
      }
      if let Some(value) = patch.sticky_fade {
        settings.sticky_fade = value;
      }
      if let Some(value) = patch.sticky_fade_opacity {
        if !(10..=100).contains(&value) {
          return Err("淡化透明度要在 10 到 100 之间".to_string());
        }
        settings.sticky_fade_opacity = value;
      }
      if let Some(value) = &patch.sticky_pattern {
        let trimmed = value.trim();
        if !crate::models::STICKY_PATTERNS.contains(&trimmed) {
          return Err("纸面花纹只能是 none / bamboo / mountain".to_string());
        }
        settings.sticky_pattern = trimmed.to_string();
      }
      if let Some(value) = patch.telemetry_enabled {
        settings.telemetry_enabled = value;
      }
      if let Some(value) = &patch.reading_font {
        settings.reading_font = crate::models::normalize_reading_font(value)?;
      }
      if let Some(value) = patch.reading_size {
        settings.reading_size = crate::models::normalize_reading_size(value);
      }
      if let Some(value) = patch.reading_tracking {
        settings.reading_tracking = crate::models::normalize_reading_tracking(value);
      }
      if let Some(value) = &patch.reading_measure {
        settings.reading_measure = crate::models::normalize_reading_measure(value);
      }
      if let Some(value) = &patch.capture_hotkey {
        let trimmed = value.trim();
        if trimmed.is_empty() {
          return Err("快捷键不能为空，已改回默认".to_string());
        }
        settings.capture_hotkey = trimmed.to_string();
      }
      if let Some(value) = &patch.main_hotkey {
        let trimmed = value.trim();
        settings.main_hotkey = if trimmed.is_empty() {
          None
        } else {
          Some(trimmed.to_string())
        };
      }
      if let Some(value) = &patch.sticky_hotkey {
        let trimmed = value.trim();
        settings.sticky_hotkey = if trimmed.is_empty() {
          None
        } else {
          Some(trimmed.to_string())
        };
      }
      if let Some(value) = patch.todo_float_pinned {
        settings.todo_float_pinned = value;
      }
      if let Some(value) = &patch.todo_hotkey {
        let trimmed = value.trim();
        settings.todo_hotkey = if trimmed.is_empty() {
          None
        } else {
          Some(trimmed.to_string())
        };
      }
      if let Some(value) = patch.remind_cap_per_hour {
        if value == 0 || value > 60 {
          return Err("每小时提醒上限要在 1 到 60 之间".to_string());
        }
        settings.remind_cap_per_hour = value;
      }
      if let Some(value) = patch.onboarded {
        settings.onboarded = value;
      }
      if let Some(value) = &patch.export_dir {
        let trimmed = value.trim();
        settings.export_dir = if trimmed.is_empty() {
          None
        } else {
          Some(trimmed.to_string())
        };
      }
      settings.coerce();
      if settings.capture_hotkey != default_capture && settings.capture_hotkey.trim().is_empty() {
        settings.capture_hotkey = default_capture.clone();
      }
    }
    Ok(())
  }

  /// 一次性任务提醒触发后清掉 remindAt（避免重复轰炸）
  pub fn consume_task_remind(&mut self, id: &str, key: &str) {
    if let Some(index) = self.find_index(id) {
      self.data.tasks[index].remind_at = None;
      self.data.tasks[index].updated_at = self.stamp();
    }
    self.remember_fired(key);
  }

  pub fn remember_fired(&mut self, key: &str) {
    if !self.runtime.fired.iter().any(|value| value == key) {
      self.runtime.fired.push(key.to_string());
    }
    if self.runtime.fired.len() > 400 {
      let excess = self.runtime.fired.len() - 400;
      self.runtime.fired.drain(0..excess);
    }
  }

  pub fn has_fired(&self, key: &str) -> bool {
    self.runtime.fired.iter().any(|value| value == key)
  }

  /// 触发/落库后统一走这里：写盘 + 事件由各命令层负责
  pub fn touch_task_time(&mut self, id: &str) {
    if let Some(index) = self.find_index(id) {
      let stamp = self.stamp();
      self.data.tasks[index].updated_at = stamp;
    }
  }

  pub fn retention_days(&self) -> i64 {
    TRASH_RETENTION_DAYS
  }

  // ---------------------------------------------------------------- 手动备份（D3 / D4）

  /// 导出当前数据为单个 `.kettdbackup` 文件（design D3）。
  ///
  /// - 只导出知识条目正文引用到的图片资源：缺失 / 读失败的资源计入 `skipped`，不阻断导出；
  /// - 导出不轮转、不写 `data.json` / `notes.json`（纯读），因此命令层无需 emit；
  /// - `exported_at` 由命令层传入本地语义文本；文件名里的时间戳取 Store 时钟的本地时刻
  ///   （`YYYYMMDD-HHmmss`，无 UTC 换算；固定时钟下可复现）。
  pub fn export_backup_to_file(
    &self,
    app_version: &str,
    exported_at: &str,
  ) -> Result<backup::ExportResult, String> {
    let mut assets: Vec<backup::ExportAsset> = Vec::new();
    let mut skipped = 0usize;
    for (sha256, ext) in backup::collect_asset_refs(&self.kb) {
      if !self.backend.asset_exists(&sha256, &ext) {
        skipped += 1;
        continue;
      }
      match self.backend.read_asset(&sha256, &ext) {
        Ok(bytes) => assets.push(backup::ExportAsset { sha256, ext, bytes }),
        Err(_) => skipped += 1,
      }
    }
    let item_count = self.data.tasks.len()
      + self.data.reminders.len()
      + self.data.stickies.len()
      + self.kb.len();
    let file = backup::build_backup(
      app_version,
      exported_at,
      backup::ExportInputs {
        data: self.data.clone(),
        kb: self.kb.clone(),
        folders: self.folders.clone(),
        assets,
        skipped,
      },
    );
    let payload = backup::export_payload(&file)?;
    let stamp = self.clock_now().format("%Y%m%d-%H%M%S").to_string();
    self
      .backend
      .create_backups_dir()
      .map_err(|_| crate::kb_errors::EXPORT_FAILED.to_string())?;
    let path = self
      .backend
      .write_export_file(&backup::backup_file_name(&stamp), &payload)
      .map_err(|_| crate::kb_errors::EXPORT_FAILED.to_string())?;
    Ok(backup::ExportResult {
      path,
      bytes: payload.len(),
      item_count,
      asset_count: file.assets.len(),
      skipped,
    })
  }

  /// 从备份文本导入（design D4）。
  ///
  /// - `dry_run = true`：只解析 + 算合并计划，绝不写盘、不改内存；
  /// - `dry_run = false`：按 D4 顺序落盘 —— 资源 → 正文 → 文件夹 → 索引 → data；
  ///   轮转直接沿用既有 `save_folders` / `save_notes` / `save` 的写前轮转（每层各一次，
  ///   不额外重复轮转），因此轮转发生在资源/正文写完之后，见交付报告偏离说明；
  /// - 全量 Skip 的重复导入不触发任何写盘（幂等，无二次副作用）；
  /// - `runtime.json` 内容不动；应用后由命令层 emit。
  pub fn import_backup_from_payload(
    &mut self,
    payload: &str,
    dry_run: bool,
    stamp: &str,
  ) -> Result<backup::ImportReport, String> {
    self.ensure_kb_ok()?;
    let parsed = backup::parse_backup(payload)?;
    let local = backup::LocalSnapshot::new(
      self.data.tasks.clone(),
      self.data.reminders.clone(),
      self.kb.clone(),
      self.folders.clone(),
    );
    let plan = backup::plan_import(&local, &parsed);
    let conflict_titles = backup::conflict_titles(&local, &plan);
    // 冲突副本既计入 added 也计入 conflicts（口径见 backup::ImportReport 注释）。
    let conflicts = plan.tasks.conflict + plan.kb.conflict;
    let added = plan.tasks.add
      + plan.reminders.add
      + plan.kb.add
      + plan.folders.add
      + conflicts;
    let updated = plan.tasks.update
      + plan.reminders.update
      + plan.kb.update
      + plan.folders.update;
    let invalid_assets = parsed
      .assets()
      .iter()
      .filter(|asset| !asset.is_reusable())
      .count();
    let skipped = plan.tasks.skip
      + plan.reminders.skip
      + plan.kb.skip
      + plan.folders.skip
      + invalid_assets;
    let mut asset_added = 0usize;
    let mut asset_reused = 0usize;
    for asset in parsed.assets().iter().filter(|asset| asset.is_reusable()) {
      if self.backend.asset_exists(&asset.sha256, &asset.ext) {
        asset_reused += 1;
      } else {
        asset_added += 1;
      }
    }
    let report = backup::ImportReport {
      added,
      updated,
      conflicts,
      skipped,
      asset_added,
      asset_reused,
      conflict_titles,
      dry_run,
    };
    if dry_run {
      return Ok(report);
    }
    // 先算目标状态（纯函数）：解析 / 覆盖失败时一行都不写。
    let apply = backup::apply_import_plan(&local, &parsed, &plan, stamp)?;
    self.ensure_writable()?;
    // ① 资源：hash 校验失败跳过（已计入 skipped）；已存在则复用，不重复写。
    for asset in parsed.assets().iter().filter(|asset| asset.is_reusable()) {
      if self.backend.asset_exists(&asset.sha256, &asset.ext) {
        continue;
      }
      self
        .backend
        .write_asset(&asset.sha256, &asset.ext, &asset.bytes)
        .map_err(|_| crate::kb_errors::IMPORT_FAILED.to_string())?;
    }
    // ② 正文（先正文后索引，D1/D4）③ 文件夹 ④ 索引 ⑤ data —— runtime 不动。
    for (id, body) in &apply.body_writes {
      self
        .backend
        .write_note_body(id, body)
        .map_err(|_| crate::kb_errors::IMPORT_FAILED.to_string())?;
    }
    if plan.changed() > 0 {
      self.folders = apply.folders;
      self
        .save_folders()
        .map_err(|_| crate::kb_errors::IMPORT_FAILED.to_string())?;
      self.kb = apply.kb;
      self
        .save_notes()
        .map_err(|_| crate::kb_errors::IMPORT_FAILED.to_string())?;
      self.data.tasks = apply.tasks;
      self.data.reminders = apply.reminders;
      self
        .save()
        .map_err(|_| crate::kb_errors::IMPORT_FAILED.to_string())?;
    }
    Ok(report)
  }
}

// ---------------------------------------------------------------- 工具

/// JSON 字符串 → Value（保留原 corrupt 通道的两种用户可见文案）
fn parse_value(text: &str) -> Result<Value, String> {
  serde_json::from_str::<Value>(text).map_err(|error| {
    let text = error.to_string();
    if text.contains("EOF") || text.contains("expected") {
      "数据文件被截断或格式不对，已停止写入".to_string()
    } else {
      "数据文件读不了，已停止写入".to_string()
    }
  })
}

/// 只给用户看短句，不泄露路径/堆栈
pub fn brief(text: &str) -> String {
  let trimmed = text.trim();
  let chars: Vec<char> = trimmed.chars().collect();
  if chars.len() <= 80 {
    return trimmed.to_string();
  }
  let mut out: String = chars.iter().take(80).collect();
  out.push('…');
  out
}

/// 启动目录创建失败时的用户可见短句
fn brief_io(error: &std::io::Error) -> String {
  let kind = match error.kind() {
    std::io::ErrorKind::PermissionDenied => "没有写入权限",
    std::io::ErrorKind::NotFound => "找不到文件",
    std::io::ErrorKind::ReadOnlyFilesystem => "目录是只读的",
    _ => "磁盘或路径不可用",
  };
  kind.to_string()
}

fn write_error(detail: Option<&str>) -> String {
  match detail {
    Some(text) if !text.trim().is_empty() => text.to_string(),
    _ => "本地数据保存失败，请检查磁盘空间或目录权限".to_string(),
  }
}

/// 文件名字符串净化（保留中文与字母数字）
pub fn safe_file_stem(text: &str) -> String {
  let mut out = String::new();
  for ch in text.chars() {
    if ch.is_alphanumeric() || ch == '-' || ch == '_' {
      out.push(ch);
    } else {
      out.push('-');
    }
  }
  while out.contains("--") {
    out = out.replace("--", "-");
  }
  let trimmed = out.trim_matches('-').to_string();
  if trimmed.is_empty() {
    return "week-export".to_string();
  }
  trimmed
}

/// 提醒的稳定键（跨重启去重）
pub fn reminder_key(id: &str, stamp: &str) -> String {
  format!("r|{}|{}", id, stamp)
}

pub fn task_key(id: &str, stamp: &str) -> String {
  format!("t|{}|{}", id, stamp)
}

// ---------------------------------------------------------------- 迁移契约单测
// 只喂内存 fixture 给纯函数 migrate_v1，绝不触碰 %APPDATA% 真库
// （data_dir() 是硬编码的，手点回归必然污染唯一的 dogfood 样本）。
// 逐条对应 V2-API §7 的十规则。

#[cfg(test)]
mod tests {
  use super::*;
  use crate::models::KbPayload;
  use crate::ports::clock::test_double::FixedClock;
  use crate::ports::store_backend::test_double::InMemoryBackend;
  use chrono::TimeZone;
  use serde_json::json;

  /// 内存库 + 固定时钟（2026-09-05 09:00）：不碰磁盘、不等真实时间
  fn mem_store() -> Store {
    Store::with_backend(
      Box::new(InMemoryBackend::new()),
      Box::new(FixedClock::at(Local.with_ymd_and_hms(2026, 9, 5, 9, 0, 0).unwrap())),
    )
  }

  /// 一份最小 v1 文档（顶层 theme 是字符串 = v1 特征）
  fn v1_doc(tasks: Value, reminders: Value) -> Value {
    json!({ "tasks": tasks, "reminders": reminders, "theme": "纸白" })
  }

  fn migrate(tasks: Value, reminders: Value) -> (AppData, MigrationReport) {
    migrate_v1(&v1_doc(tasks, reminders)).expect("fixture 迁移不应失败")
  }

  fn issue_for<'a>(report: &'a MigrationReport, field: &str) -> Option<&'a MigrationIssue> {
    report.issues.iter().find(|item| item.field == field)
  }

  #[test]
  fn rule0_detects_v1_by_absent_settings_object() {
    assert!(looks_like_v1(&v1_doc(json!([]), json!([]))));
    // v2 一定有 settings 对象 → 不再走迁移
    assert!(!looks_like_v1(&json!({ "settings": { "theme": "float" }, "tasks": [] })));
    // 既无 settings 也无 theme，但有 tasks → 仍按 v1 处理
    assert!(looks_like_v1(&json!({ "tasks": [] })));
  }

  #[test]
  fn rule3_maps_snake_case_fields_to_camel_case() {
    let (data, _report) = migrate(
      json!([{
        "id": "t1",
        "title": "取快递",
        "description": "柜机 B 栋",
        "due_date": "2026-08-29 18:00",
        "created_at": "2026-08-01T09:00",
        "completed": false,
        "subtasks": [{ "id": "s1", "title": "带小票", "completed": true }],
        "notes": [{ "id": "n1", "author": "我", "content": "备注一", "created_at": "2026-08-02T10:00" }]
      }]),
      json!([]),
    );
    let task = &data.tasks[0];
    assert_eq!(task.note.as_str(), "柜机 B 栋", "description → note");
    assert_eq!(
      task.due_at.as_deref(),
      Some("2026-08-29T18:00"),
      "due_date → dueAt 且空格制式转 T 制式"
    );
    assert_eq!(task.created_at.as_str(), "2026-08-01T09:00", "created_at → createdAt");
    assert_eq!(task.subtasks.len(), 1);
    assert!(task.subtasks[0].done, "subtasks[].completed → done");
    assert_eq!(task.notes[0].created_at.as_str(), "2026-08-02T10:00");
  }

  #[test]
  fn rule4_nulls_unparsable_time_and_counts_it() {
    let (data, report) = migrate(
      json!([{ "id": "t1", "title": "坏时间", "due_date": "昨天下午", "completed": false }]),
      json!([]),
    );
    assert_eq!(data.tasks[0].due_at, None, "解析失败必须置 null，不许猜");
    assert_eq!(report.invalid_dates, 1);
    let issue = issue_for(&report, "dueAt").expect("必须留痕");
    assert!(issue.action.contains("无法解析"), "留痕要说明处置：{}", issue.action);
    assert_eq!(issue.raw, "昨天下午");
  }

  #[test]
  fn rule5_completed_without_done_at_is_marked_legacy_not_faked() {
    let (data, report) = migrate(
      json!([{ "id": "t1", "title": "老完成项", "completed": true }]),
      json!([]),
    );
    let task = &data.tasks[0];
    assert!(task.done, "v1 completed → done");
    assert_eq!(task.done_at, None, "绝不伪造完成时刻");
    assert!(task.legacy, "缺完成时刻要标 legacy");
    assert_eq!(report.legacy_done, 1);
  }

  #[test]
  fn rule6_remaps_personal_category_and_medium_priority_with_trail() {
    let (data, report) = migrate(
      json!([{ "id": "t1", "title": "枚举归一", "category": "个人", "priority": "medium", "completed": false }]),
      json!([]),
    );
    assert_eq!(data.tasks[0].category, "生活");
    assert_eq!(data.tasks[0].priority, "med");
    assert!(issue_for(&report, "category").unwrap().action.contains("生活"));
    assert!(issue_for(&report, "priority").unwrap().action.contains("med"));
  }

  #[test]
  fn rule6_falls_back_unknown_category_without_losing_task() {
    let (data, report) = migrate(
      json!([{ "id": "t1", "title": "怪分类", "category": "宇宙", "completed": false }]),
      json!([]),
    );
    assert_eq!(data.tasks[0].category, "生活");
    assert!(issue_for(&report, "category").unwrap().action.contains("回落"));
  }

  #[test]
  fn rule7_carries_overdue_open_task_to_today_once() {
    let past = (today() - chrono::Duration::days(3))
      .format("%Y-%m-%d")
      .to_string();
    let (data, _report) = migrate(
      json!([{ "id": "t1", "title": "拖了三天", "due_date": past, "completed": false }]),
      json!([]),
    );
    let task = &data.tasks[0];
    assert_eq!(
      task.planned_date.as_deref(),
      Some(fmt_day(&today()).as_str()),
      "逾期未完成要一次性粘到今天"
    );
    assert_eq!(task.carried_from, 3, "拖留天数一次算清，后续不再自算第二套口径");
  }

  #[test]
  fn rule7_does_not_touch_done_or_trashed_tasks() {
    let past = (today() - chrono::Duration::days(5))
      .format("%Y-%m-%d")
      .to_string();
    let (data, _report) = migrate(
      json!([
        { "id": "t1", "title": "已完成", "due_date": past, "completed": true, "done_at": "2026-01-01T10:00" },
        { "id": "t2", "title": "在回收站", "due_date": past, "completed": false, "deleted_at": "2026-01-02T10:00" }
      ]),
      json!([]),
    );
    assert_eq!(data.tasks[0].planned_date, None, "已完成的不该被塞进今天");
    assert_eq!(data.tasks[1].planned_date, None, "回收站里的不该被塞进今天");
  }

  #[test]
  fn rule8_keeps_recurring_and_multi_clock_times_verbatim() {
    let (data, report) = migrate(
      json!([]),
      json!([
        { "id": "r1", "title": "单次", "time": "2026-09-05 08:30", "completed": false },
        { "id": "r2", "title": "每天", "time": "09:00", "completed": false },
        { "id": "r3", "title": "多时刻", "time": "10:00/14:00/16:00", "completed": true }
      ]),
    );
    assert_eq!(data.reminders[0].time, "2026-09-05T08:30", "空格制式仍要归一为 T 制式");
    assert_eq!(data.reminders[1].time, "09:00");
    assert_eq!(
      data.reminders[2].time, "10:00/14:00/16:00",
      "v1 多时刻必须原样保留 —— 降级为单时刻等于改用户数据"
    );
    assert_eq!(clocks_of(&data.reminders[2].time).len(), 3);
    assert!(!data.reminders[2].enabled, "completed → enabled 取反");
    assert!(!data.reminders[2].completed, "v1 的 completed 不映射为 v2 completed");
    assert!(data.reminders[2].legacy, "迁移来的提醒一律标 legacy");
    assert_eq!(report.reminder_count, 3);
  }

  #[test]
  fn rule9_v1_theme_string_falls_back_to_float_with_trail() {
    let (data, report) = migrate(json!([]), json!([]));
    assert_eq!(data.settings.theme, "float");
    let issue = issue_for(&report, "theme").expect("主题语义变更必须留痕");
    assert_eq!(issue.raw, "纸白");
  }

  #[test]
  fn rule9_carries_over_v1_hotkey_and_clamps_cap() {
    let mut raw = v1_doc(json!([]), json!([]));
    raw["captureHotkey"] = json!("  Ctrl+Alt+K  ");
    raw["remindCapPerHour"] = json!(999);
    raw["onboarded"] = json!(true);
    let (data, _report) = migrate_v1(&raw).unwrap();
    assert_eq!(data.settings.capture_hotkey, "Ctrl+Alt+K", "两端空格要清掉");
    assert_eq!(data.settings.remind_cap_per_hour, 60, "超上限要夹住而不是写花库");
    assert!(data.settings.onboarded);
  }

  /// 便签分离迁移（sticky-separation）：v2 旧形态便签在 JSON 层归一为新模型。
  #[test]
  fn sticky_separation_drops_todo_notes_and_lifts_hidden_to_mini() {
    let mut raw = json!({
      "stickies": [
        { "id": "n1", "kind": "todo", "content": "", "pinned": true, "hidden": false,
          "createdAt": "2026-09-01T10:00:00", "updatedAt": "2026-09-01T10:00:00" },
        { "id": "n2", "kind": "free", "content": "草稿", "pinned": true, "hidden": true,
          "createdAt": "2026-09-01T10:00:00", "updatedAt": "2026-09-01T10:00:00" },
        { "id": "n3", "kind": "free", "content": "常驻", "pinned": false, "hidden": false,
          "createdAt": "2026-09-01T10:00:00", "updatedAt": "2026-09-01T10:00:00" },
        { "id": "n4", "content": "已是新形态", "mini": true,
          "createdAt": "2026-09-01T10:00:00", "updatedAt": "2026-09-01T10:00:00" }
      ]
    });
    assert!(normalize_stickies_v3(&mut raw), "旧形态必须判定为有变化");
    let list = raw["stickies"].as_array().expect("stickies 必须还是数组");
    assert_eq!(list.len(), 3, "todo 镜像便签必须丢弃（内容活在 tasks，零损失）");
    assert_eq!(list[0]["id"], "n2");
    assert_eq!(list[0]["mini"], true, "hidden=true（收起）升格为 mini=true（缩小悬浮条）");
    assert!(
      list[0].get("hidden").is_none()
        && list[0].get("pinned").is_none()
        && list[0].get("kind").is_none(),
      "退役字段必须清干净，避免下次加载再次判定变化"
    );
    assert_eq!(list[0]["content"], "草稿", "正文零损失");
    assert_eq!(list[1]["id"], "n3");
    assert!(list[1].get("mini").is_none(), "展开态不引入 mini 字段（skip 序列化语义）");
    assert_eq!(list[2]["id"], "n4", "新形态便签原样保留");
    assert!(!normalize_stickies_v3(&mut raw), "迁移必须幂等（重复加载不再变化）");
  }

  #[test]
  fn rule10_non_list_tasks_must_error_not_silently_empty() {
    let err = migrate_v1(&json!({ "tasks": { "not": "a list" }, "theme": "纸白" }))
      .expect_err("结构不可还原必须走 Err → corrupt 通道");
    assert!(err.contains("不是列表"), "错误要能直接显示给用户：{}", err);
    let err2 = migrate_v1(&json!({ "reminders": 42, "theme": "纸白" }))
      .expect_err("reminders 非列表同样要报错");
    assert!(err2.contains("不是列表"));
  }

  #[test]
  fn zero_loss_dropped_entries_leave_a_trail() {
    let (data, report) = migrate(
      json!(["我是脏数据",
             { "id": "t1", "title": "好任务1", "completed": false },
             { "id": "t2", "title": "好任务2", "completed": false }]),
      json!([]),
    );
    assert_eq!(data.tasks.len(), 2);
    assert_eq!(report.task_count, 2);
    assert!(
      report.issues.iter().any(|item| item.field == "tasks"),
      "丢弃的每一条都要留痕，否则「零丢失」不可核对"
    );
  }

  #[test]
  fn empty_v1_document_migrates_cleanly_without_panicking() {
    let (data, report) = migrate(json!([]), json!([]));
    assert!(data.tasks.is_empty());
    assert!(data.reminders.is_empty());
    assert_eq!(report.task_count, 0);
    assert_eq!(report.invalid_dates, 0);
  }

  #[test]
  fn report_points_at_the_immutable_v1_archive_name() {
    let (_data, report) = migrate(json!([]), json!([]));
    assert_eq!(report.archived_to, ARCHIVE_FILE);
    assert_eq!(ARCHIVE_FILE, "data.v1.json", "原件留档名是对用户的承诺");
  }

  #[test]
  fn fired_keys_survive_migration_as_opaque_strings() {
    let mut raw = v1_doc(json!([]), json!([]));
    raw["fired"] = json!(["t|t1|2026-09-03T09:00", 42, "r|r1|2026-09-03T10:00"]);
    let mut backend = InMemoryBackend::with_data(&serde_json::to_string(&raw).unwrap());
    backend.archive = std::cell::RefCell::new(Some(serde_json::to_string(&raw).unwrap()));
    let store = Store::load_with(Box::new(backend), Box::new(FixedClock::at(
      Local.with_ymd_and_hms(2026, 9, 6, 9, 0, 0).unwrap())));
    assert_eq!(
      store.runtime.fired,
      vec!["t|t1|2026-09-03T09:00".to_string(), "r|r1|2026-09-03T10:00".to_string()],
      "非字符串键丢弃可以，但合法键一个不能少（跨重启不重复响）；键归运行态（ADR-0005）"
    );
    assert!(store.runtime.migration.is_some(), "迁移报告随运行态");
  }

  #[test]
  fn purge_expired_recycles_only_purged_task_keys_and_keeps_the_rest() {
    let mut store = mem_store();
    // 固定时钟 = 2026-09-05 09:00；31 天前已超 30 天保留期，1 天前未超
    let old = "2026-07-05T09:00".to_string();
    let fresh = "2026-09-04T09:00".to_string();
    store.data.tasks.push(Task {
      id: "t-old".to_string(),
      deleted_at: Some(old),
      ..Default::default()
    });
    store.data.tasks.push(Task {
      id: "t-keep".to_string(),
      deleted_at: Some(fresh),
      ..Default::default()
    });
    store.runtime.fired = vec![
      task_key("t-old", "2026-09-01T09:00"),
      task_key("t-keep", "2026-09-01T09:00"),
      reminder_key("r-1", "2026-09-01T09:00"),
    ];

    let dropped = store.purge_expired(TRASH_RETENTION_DAYS);

    assert_eq!(dropped, 1, "只应清掉过期的软删任务");
    assert_eq!(
      store.runtime.fired,
      vec![
        task_key("t-keep", "2026-09-01T09:00"),
        reminder_key("r-1", "2026-09-01T09:00"),
      ],
      "只回收被清任务自己的键；在世任务与提醒的键一个不能少，否则会重复补发提醒"
    );
  }

  // ---- ADR-0005 schema 拆分：纯函数 fixture + 编排顺序（先测后写迁移） ----

  /// 一份"旧 v2"文档：运行态（fired/migration/capturePos）还混在 data.json
  fn legacy_doc() -> Value {
    json!({
      "tasks": [{ "id": "t1", "title": "示例", "done": false }],
      "reminders": [],
      "fired": ["r|r9|2026-09-01T09:00"],
      "migration": { "taskCount": 1, "reminderCount": 0, "invalidDates": 0, "legacyDone": 0, "issues": [], "archivedTo": "data.v1.json" },
      "settings": { "theme": "float", "capturePos": [12, 34] }
    })
  }

  #[test]
  fn split_moves_runtime_fields_out_of_data() {
    let (data, runtime) = split_schema(&legacy_doc(), None);
    assert!(data.get("fired").is_none(), "fired 必须搬出 data.json");
    assert!(data.get("migration").is_none(), "迁移报告必须搬出 data.json");
    assert!(
      data.get("settings").and_then(|s| s.get("capturePos")).is_none(),
      "capturePos 必须搬出 settings"
    );
    assert_eq!(data["tasks"][0]["id"], "t1", "用户数据原样保留");
    assert_eq!(runtime["fired"][0], "r|r9|2026-09-01T09:00");
    assert_eq!(runtime["capturePos"][0], 12);
    assert!(runtime["migration"].is_object(), "迁移报告进 runtime");
  }

  #[test]
  fn split_is_idempotent_on_already_split_document() {
    let (data, runtime) = split_schema(&legacy_doc(), None);
    let (data2, runtime2) = split_schema(&data, Some(&runtime));
    assert_eq!(data, data2, "拆分幂等：再跑一次 data 不变");
    assert_eq!(runtime2["fired"], runtime["fired"], "fired 并集不重复");
  }

  #[test]
  fn split_merges_fired_with_existing_runtime_as_union() {
    let existing = json!({ "fired": ["r|r9|2026-09-01T09:00", "r|r8|2026-09-02T09:00"] });
    let (_, runtime) = split_schema(&legacy_doc(), Some(&existing));
    let fired = runtime["fired"].as_array().unwrap();
    assert_eq!(fired.len(), 2, "交集去重、并集保留");
    assert!(fired.contains(&json!("r|r8|2026-09-02T09:00")));
  }

  #[test]
  fn split_keeps_existing_runtime_migration_over_data_side() {
    let mut raw = legacy_doc();
    raw["migration"] = json!({ "taskCount": 9 });
    let already_there = json!({ "fired": [], "migration": null });
    let (_, runtime) = split_schema(&raw, Some(&already_there));
    assert!(
      runtime.get("migration").map(|value| value.is_null()).unwrap_or(false),
      "runtime 已有 migration 键时不被 data 侧覆盖"
    );
  }

  #[test]
  fn split_keeps_settings_other_keys_intact() {
    let (data, _) = split_schema(&legacy_doc(), None);
    assert_eq!(data["settings"]["theme"], "float", "只摘 capturePos，其余不动");
  }

  #[test]
  fn load_legacy_data_runs_the_split_end_to_end() {
    let raw = legacy_doc();
    let store = Store::load_with(
      Box::new(InMemoryBackend::with_data(&serde_json::to_string(&raw).unwrap())),
      Box::new(FixedClock::at(Local.with_ymd_and_hms(2026, 9, 6, 9, 0, 0).unwrap())),
    );
    assert!(store.runtime.fired.contains(&"r|r9|2026-09-01T09:00".to_string()));
    assert_eq!(store.data.tasks.len(), 1, "用户数据完整");
    assert!(store.last_error.is_none(), "拆分成功不报错");
  }

  #[test]
  fn split_persists_in_order_and_archives_original() {
    let (data, runtime) = split_schema(&legacy_doc(), None);
    let data_json = serde_json::to_string_pretty(&data).unwrap();
    let runtime_json = serde_json::to_string_pretty(&runtime).unwrap();

    let backend = InMemoryBackend::with_data(&serde_json::to_string(&legacy_doc()).unwrap());
    backend.archive_pre_split().unwrap();
    backend.write_runtime(&runtime_json).unwrap();
    backend.rotate_backups().unwrap();
    backend.write_data(&data_json).unwrap();

    let events: Vec<String> = backend.log.borrow().clone();
    let pos = |name: &str| events.iter().position(|item| item == name).unwrap();
    assert!(pos("archive_pre_split") < pos("write_runtime"), "留档先于 runtime");
    assert!(pos("write_runtime") < pos("rotate_backups"), "runtime 先于 data 轮转");
    assert!(pos("rotate_backups") < pos("write_data"), "轮转后原子写 data");
    let pre = backend.pre_split.borrow().clone().unwrap();
    assert!(pre.contains("fired"), "pre-split 原件是拆分前的完整旧档");
    assert!(
      backend.backups.borrow().first().unwrap().contains("fired"),
      "bak-1 = 拆分前完整旧档，用户手里始终有一份什么都没丢的历史"
    );
  }

  #[test]
  fn split_failure_keeps_originals_and_session_continues() {
    let raw_json = serde_json::to_string(&legacy_doc()).unwrap();
    let backend = InMemoryBackend::with_data(&raw_json);
    backend.set_fail_writes(true);
    let store = Store::load_with(
      Box::new(backend),
      Box::new(FixedClock::at(Local.with_ymd_and_hms(2026, 9, 6, 9, 0, 0).unwrap())),
    );
    assert_eq!(store.health, HEALTH_OK, "拆分失败不进 corrupt、不冻结");
    assert!(
      store.last_error.as_deref().unwrap_or("").contains("拆分"),
      "失败要可解释：{}",
      store.last_error.as_deref().unwrap_or("")
    );
    assert_eq!(store.data.tasks.len(), 1, "本会话按拆分结果在内存中继续，用户可看");
  }

  #[test]
  fn load_tolerates_corrupt_runtime_json_by_rebuilding() {
    let raw = json!({ "tasks": [], "reminders": [], "settings": { "theme": "float" } });
    let backend = InMemoryBackend::with_data(&serde_json::to_string(&raw).unwrap());
    *backend.runtime.borrow_mut() = Some("{截断的垃圾".to_string());
    let store = Store::load_with(
      Box::new(backend),
      Box::new(FixedClock::at(Local.with_ymd_and_hms(2026, 9, 6, 9, 0, 0).unwrap())),
    );
    assert_eq!(store.health, HEALTH_OK, "运行态损坏绝不传染用户数据");
    assert!(store.runtime.fired.is_empty(), "静默重建为空");
  }

  #[test]
  fn purge_task_recycles_its_own_fired_keys() {
    let mut store = mem_store();
    store.data.tasks.push(Task {
      id: "t-gone".to_string(),
      deleted_at: Some("2026-09-01T10:00".to_string()),
      ..Default::default()
    });
    store.runtime.fired = vec![
      task_key("t-gone", "2026-09-01T09:00"),
      reminder_key("r-1", "2026-09-01T09:00"),
    ];

    let removed = store.purge_task("t-gone").expect("回收站里的任务应可彻底清除");

    assert_eq!(removed.id, "t-gone");
    assert_eq!(
      store.runtime.fired,
      vec![reminder_key("r-1", "2026-09-01T09:00")],
      "彻底清除后该任务的 fired 键必须回收"
    );
  }

  // ---- 批次 4：知识库文件夹（D6）+ notes.json 轮转恢复（D8） ----

  fn kb_item(id: &str) -> KbItem {
    KbItem {
      id: id.to_string(),
      title: id.to_string(),
      ..Default::default()
    }
  }

  /// 一份可正常加载的 v2 data.json（知识库测试只关心 notes/folders）
  fn app_data_json() -> String {
    serde_json::to_string(&json!({
      "tasks": [],
      "reminders": [],
      "settings": { "theme": "float" }
    }))
    .unwrap()
  }

  fn fixed_clock() -> FixedClock {
    FixedClock::at(Local.with_ymd_and_hms(2026, 9, 5, 9, 0, 0).unwrap())
  }

  #[test]
  fn folder_crud_validates_names_parents_and_emptiness() {
    let mut store = mem_store();
    assert_eq!(
      store.create_folder("   ", None).unwrap_err(),
      "文件夹名称不能为空"
    );
    assert_eq!(
      store.create_folder(&"字".repeat(41), None).unwrap_err(),
      "文件夹名称最多 40 个字"
    );
    let root = store.create_folder(" 工作 ", None).unwrap();
    assert_eq!(root.name, "工作", "名称 trim 后保存");
    assert_eq!(root.parent_id, None);
    assert_eq!(root.order, 0, "根目录首个 order = 0");
    assert!(root.id.starts_with('f'), "id 沿用 new_id 生成风格");
    assert_eq!(
      store.create_folder(&"字".repeat(40), None).unwrap().order,
      1,
      "同级 dense 追加 order = 同级数量（40 字名合法）"
    );
    let second = store.create_folder("学习", None).unwrap();
    assert_eq!(second.order, 2);
    let via_empty_parent = store.create_folder("空父级当根", Some("")).unwrap();
    assert_eq!(via_empty_parent.parent_id, None, "空串父级按根处理");
    let child = store.create_folder("周报", Some(&root.id)).unwrap();
    assert_eq!(child.parent_id.as_deref(), Some(root.id.as_str()));
    assert_eq!(child.order, 0, "子级 order 独立从 0 开始");
    assert_eq!(
      store.create_folder("子项", Some("missing")).unwrap_err(),
      "文件夹不存在"
    );
    let child2 = store.create_folder("子项2", Some(&second.id)).unwrap();
    assert_eq!(child2.order, 0);

    let renamed = store
      .update_folder(&root.id, Some("  工作区 ".to_string()), None, None, None)
      .unwrap();
    assert_eq!(renamed.name, "工作区");
    assert_eq!(
      store
        .update_folder("missing", Some("x".to_string()), None, None, None)
        .unwrap_err(),
      "文件夹不存在"
    );
    assert!(store
      .update_folder(&root.id, Some("  ".to_string()), None, None, None)
      .is_err());

    // 有子文件夹 → 拒绝删除
    assert_eq!(
      store.delete_folder(&root.id).unwrap_err(),
      "文件夹非空，请先移走其中的条目或子文件夹"
    );
    // 有条目引用 → 拒绝删除
    store.kb.push(kb_item("k1"));
    store
      .place_kb_item("k1", Some(Some(second.id.clone())), None, None)
      .unwrap();
    assert_eq!(
      store.delete_folder(&second.id).unwrap_err(),
      "文件夹非空，请先移走其中的条目或子文件夹"
    );
    // 移到未分类后，空文件夹可删
    store.place_kb_item("k1", Some(None), None, None).unwrap();
    store.delete_folder(&child2.id).unwrap();
    store.delete_folder(&second.id).unwrap();
    assert!(!store.folder_exists(&second.id));
    assert_eq!(store.delete_folder("missing").unwrap_err(), "文件夹不存在");
    // 删空子文件夹后父文件夹也可删
    assert!(store.delete_folder(&child.id).is_ok());
    assert!(store.delete_folder(&root.id).is_ok());
  }

  #[test]
  fn kb_payload_folder_id_is_trivalent() {
    let untouched: KbPayload = serde_json::from_value(json!({ "title": "t" })).unwrap();
    assert_eq!(untouched.folder_id, None, "缺省 folderId = 不改文件夹");
    let cleared: KbPayload = serde_json::from_value(json!({ "folderId": "" })).unwrap();
    assert_eq!(cleared.folder_id.as_deref(), Some(""), "空串 = 移到未分类");
    let moved: KbPayload = serde_json::from_value(json!({ "folderId": "f1" })).unwrap();
    assert_eq!(moved.folder_id.as_deref(), Some("f1"));

    let mut store = mem_store();
    store.kb.push(kb_item("k1"));
    let folder = store.create_folder("工作", None).unwrap();
    assert_eq!(store.folder_target(None).unwrap(), None);
    assert_eq!(
      store.folder_target(Some("")).unwrap(),
      None,
      "Some(\"\") → 未分类（存 None）"
    );
    let target = store.folder_target(Some(&folder.id)).unwrap();
    assert_eq!(target, Some(folder.id.clone()));
    let item = store.place_kb_item("k1", Some(target), None, None).unwrap();
    assert_eq!(item.folder_id.as_deref(), Some(folder.id.as_str()));
    assert_eq!(
      store.folder_target(Some("missing")).unwrap_err(),
      "文件夹不存在"
    );
    assert_eq!(
      store
        .place_kb_item("missing", Some(None), None, None)
        .unwrap_err(),
      "找不到这条资料，可能已经被删除"
    );
  }

  // ---- kb-file-tree D4/D6：同级 dense 排序 / 移动环校验 / 置顶 ----

  fn folder_order(store: &Store, id: &str) -> i64 {
    store
      .folders
      .iter()
      .find(|folder| folder.id == id)
      .unwrap()
      .order
  }

  fn item_order(store: &Store, id: &str) -> Option<i64> {
    store.kb.iter().find(|item| item.id == id).unwrap().order
  }

  fn folder_by(store: &Store, id: &str) -> KbFolder {
    store
      .folders
      .iter()
      .find(|folder| folder.id == id)
      .unwrap()
      .clone()
  }

  #[test]
  fn legacy_notes_without_order_backfill_dense_and_defaults_pinned() {
    // 旧 notes.json：无 order / pinned；旧 folders.json：无 pinned
    let notes = json!([
      { "id": "k-old", "title": "旧", "bodyMd": "", "tags": [], "createdAt": "2026-09-01T09:00", "updatedAt": "2026-09-01T09:00" },
      { "id": "k-new", "title": "新", "bodyMd": "", "tags": [], "createdAt": "2026-09-03T09:00", "updatedAt": "2026-09-03T09:00" },
      { "id": "k-mid", "title": "中", "bodyMd": "", "tags": [], "createdAt": "2026-09-02T09:00", "updatedAt": "2026-09-02T09:00" },
      { "id": "k-in-folder", "title": "夹内", "bodyMd": "", "tags": [], "createdAt": "2026-09-04T09:00", "updatedAt": "2026-09-04T09:00", "folderId": "f1" }
    ]);
    let folders = json!([
      { "id": "f1", "name": "工作", "parentId": null, "order": 0 }
    ]);
    let backend = InMemoryBackend::new();
    *backend.data.borrow_mut() = Some(app_data_json());
    *backend.notes.borrow_mut() = Some(serde_json::to_string(&notes).unwrap());
    *backend.folders.borrow_mut() = Some(serde_json::to_string(&folders).unwrap());
    let store = Store::load_with(Box::new(backend), Box::new(fixed_clock()));
    assert_eq!(store.kb.len(), 4, "旧 notes 缺 order 仍能加载");
    assert!(store.ensure_kb_ok().is_ok(), "旧数据不是损坏");
    assert_eq!(item_order(&store, "k-new"), Some(0), "同级按 updated_at 倒序生成");
    assert_eq!(item_order(&store, "k-mid"), Some(1));
    assert_eq!(item_order(&store, "k-old"), Some(2));
    assert_eq!(item_order(&store, "k-in-folder"), Some(0), "各父级独立从 0 开始");
    assert!(store.kb.iter().all(|item| !item.pinned), "旧条目缺 pinned → false");
    assert_eq!(store.folders[0].pinned, false, "旧文件夹缺 pinned → false");
    // 内存补齐后序列化带 order 数字（下一次保存自然落盘；读路径不改文件）
    let value = serde_json::to_value(&store.kb[0]).unwrap();
    assert!(
      value.get("order").map(|order| order.is_i64()).unwrap_or(false),
      "order 序列化为数字：{value}"
    );
  }

  #[test]
  fn create_folder_appends_dense_order_and_starts_unpinned() {
    let mut store = mem_store();
    let first = store.create_folder("甲", None).unwrap();
    let second = store.create_folder("乙", None).unwrap();
    let third = store.create_folder("丙", None).unwrap();
    assert_eq!(
      vec![first.order, second.order, third.order],
      vec![0, 1, 2],
      "新文件夹同级 dense 追加"
    );
    assert!(store.folders.iter().all(|folder| !folder.pinned), "新建默认不置顶");
    let child = store.create_folder("甲子", Some(&first.id)).unwrap();
    assert_eq!(child.order, 0, "子级 order 独立从 0 开始");
  }

  #[test]
  fn update_folder_rename_pin_and_dense_reorder() {
    let mut store = mem_store();
    let first = store.create_folder("甲", None).unwrap();
    let second = store.create_folder("乙", None).unwrap();
    let third = store.create_folder("丙", None).unwrap();

    let renamed = store
      .update_folder(&first.id, Some("  甲组 ".to_string()), None, None, Some(true))
      .unwrap();
    assert_eq!(renamed.name, "甲组", "改名 trim 后保存");
    assert!(renamed.pinned, "置顶直接置位");
    assert_eq!(renamed.order, 0, "只改名/置顶不重排");

    // 同父级把丙拖到最前 → dense 0..n-1
    store
      .update_folder(&third.id, None, None, Some(0), None)
      .unwrap();
    assert_eq!(
      (
        folder_order(&store, &third.id),
        folder_order(&store, &first.id),
        folder_order(&store, &second.id)
      ),
      (0, 1, 2),
      "拖动后整体 dense 重排"
    );

    // order 越界 → clamp 到 0..len
    store
      .update_folder(&first.id, None, None, Some(99), None)
      .unwrap();
    assert_eq!(
      (
        folder_order(&store, &third.id),
        folder_order(&store, &second.id),
        folder_order(&store, &first.id)
      ),
      (0, 1, 2),
      "越界 order clamp 到末尾"
    );

    // 取消置顶；名称校验复用 folder_name
    assert!(!store
      .update_folder(&first.id, None, None, None, Some(false))
      .unwrap()
      .pinned);
    assert_eq!(
      store
        .update_folder(&first.id, Some("  ".to_string()), None, None, None)
        .unwrap_err(),
      "文件夹名称不能为空"
    );
    assert_eq!(
      store
        .update_folder(&first.id, Some("字".repeat(41)), None, None, None)
        .unwrap_err(),
      "文件夹名称最多 40 个字"
    );
    assert_eq!(
      store
        .update_folder("missing", None, None, None, None)
        .unwrap_err(),
      "文件夹不存在"
    );
  }

  #[test]
  fn update_folder_move_densifies_old_and_new_parent() {
    let mut store = mem_store();
    let root_a = store.create_folder("甲", None).unwrap();
    let root_b = store.create_folder("乙", None).unwrap();
    let child_a = store.create_folder("甲子", Some(&root_a.id)).unwrap();
    let child_a2 = store.create_folder("甲子二", Some(&root_a.id)).unwrap();
    let child_b = store.create_folder("乙子", Some(&root_b.id)).unwrap();

    // 移到乙的第 0 位：旧父级 dense 收缩，新父级插入并整体重排
    store
      .update_folder(&child_a.id, None, Some(root_b.id.clone()), Some(0), None)
      .unwrap();
    assert_eq!(folder_by(&store, &child_a.id).parent_id, Some(root_b.id.clone()));
    assert_eq!(folder_order(&store, &child_a.id), 0, "插入位 0");
    assert_eq!(folder_order(&store, &child_b.id), 1, "新父级原成员后移");
    assert_eq!(folder_order(&store, &child_a2.id), 0, "旧父级只剩一个 → dense 0");

    // parentId 传 "" = 移到顶层（根），缺省 order + 移动 → 追加根末尾
    store
      .update_folder(&child_a.id, None, Some(String::new()), None, None)
      .unwrap();
    assert_eq!(folder_by(&store, &child_a.id).parent_id, None);
    assert_eq!(folder_order(&store, &child_a.id), 2);
    assert_eq!(folder_order(&store, &root_a.id), 0);
    assert_eq!(folder_order(&store, &root_b.id), 1);
    assert_eq!(folder_order(&store, &child_b.id), 0, "乙只剩一个子级 → dense 0");
  }

  #[test]
  fn update_folder_rejects_cycle_and_missing_target() {
    let mut store = mem_store();
    let root = store.create_folder("甲", None).unwrap();
    let child = store.create_folder("乙", Some(&root.id)).unwrap();
    let grandchild = store.create_folder("丙", Some(&child.id)).unwrap();

    // 自身与后代都拒绝（沿 parentId 链校验）
    assert_eq!(
      store
        .update_folder(&root.id, None, Some(root.id.clone()), None, None)
        .unwrap_err(),
      "不能把文件夹移动到自身或它的子文件夹"
    );
    assert_eq!(
      store
        .update_folder(&root.id, None, Some(grandchild.id.clone()), None, None)
        .unwrap_err(),
      "不能把文件夹移动到自身或它的子文件夹"
    );
    assert_eq!(
      store
        .update_folder(&root.id, None, Some("missing".to_string()), None, None)
        .unwrap_err(),
      "文件夹不存在"
    );
    // 拒绝后树结构不变
    assert_eq!(folder_by(&store, &root.id).parent_id, None);
    assert_eq!(folder_by(&store, &child.id).parent_id, Some(root.id.clone()));
    assert_eq!(folder_by(&store, &grandchild.id).parent_id, Some(child.id.clone()));
  }

  #[test]
  fn place_kb_item_reorders_and_pins_within_folder() {
    let mut store = mem_store();
    let folder = store.create_folder("工作", None).unwrap();
    for (index, item) in [kb_item("k1"), kb_item("k2"), kb_item("k3")]
      .into_iter()
      .enumerate()
    {
      store.kb.push(KbItem {
        folder_id: Some(folder.id.clone()),
        order: Some(index as i64),
        ..item
      });
    }

    // 同夹内把 k3 拖到最前并置顶
    let moved = store.place_kb_item("k3", None, Some(0), Some(true)).unwrap();
    assert_eq!(moved.order, Some(0));
    assert!(moved.pinned);
    assert_eq!(
      (
        item_order(&store, "k3"),
        item_order(&store, "k1"),
        item_order(&store, "k2")
      ),
      (Some(0), Some(1), Some(2))
    );

    // 越界 order clamp 到末尾
    store.place_kb_item("k1", None, Some(99), None).unwrap();
    assert_eq!(
      (
        item_order(&store, "k3"),
        item_order(&store, "k2"),
        item_order(&store, "k1")
      ),
      (Some(0), Some(1), Some(2))
    );

    // 只改 pinned 不动顺序
    store.place_kb_item("k2", None, None, Some(true)).unwrap();
    assert_eq!(item_order(&store, "k2"), Some(1));
  }

  #[test]
  fn place_kb_item_cross_folder_move_appends_and_densifies() {
    let mut store = mem_store();
    let first = store.create_folder("甲", None).unwrap();
    let second = store.create_folder("乙", None).unwrap();
    for (index, item) in [kb_item("a"), kb_item("b")].into_iter().enumerate() {
      store.kb.push(KbItem {
        folder_id: Some(first.id.clone()),
        order: Some(index as i64),
        ..item
      });
    }
    store.kb.push(KbItem {
      folder_id: Some(second.id.clone()),
      order: Some(0),
      ..kb_item("c")
    });

    // 跨夹且 order 缺省 → 追加到新父级末尾，旧父级 dense 收缩
    let moved = store
      .place_kb_item("a", Some(Some(second.id.clone())), None, None)
      .unwrap();
    assert_eq!(moved.folder_id, Some(second.id.clone()));
    assert_eq!(item_order(&store, "c"), Some(0));
    assert_eq!(item_order(&store, "a"), Some(1), "缺省 order 追加到新父级末尾");
    assert_eq!(item_order(&store, "b"), Some(0), "旧父级移除后 dense 0");

    // 移到未分类（Some(None)）
    store.place_kb_item("a", Some(None), None, None).unwrap();
    assert_eq!(
      store
        .kb
        .iter()
        .find(|item| item.id == "a")
        .unwrap()
        .folder_id,
      None
    );
    assert_eq!(item_order(&store, "a"), Some(0), "未分类组首个 order = 0");
    assert_eq!(item_order(&store, "c"), Some(0), "旧父级只剩一个 → dense 0");

    // 显式 order 跨夹插入
    store
      .place_kb_item("a", Some(Some(first.id.clone())), Some(0), None)
      .unwrap();
    assert_eq!(item_order(&store, "a"), Some(0));
    assert_eq!(item_order(&store, "b"), Some(1));
  }

  #[test]
  fn old_notes_without_folder_id_load_as_unclassified() {
    let notes = json!([{
      "id": "k1",
      "title": "旧条目",
      "bodyMd": "",
      "tags": [],
      "createdAt": "2026-09-01T09:00",
      "updatedAt": "2026-09-01T09:00"
    }]);
    let backend = InMemoryBackend::new();
    *backend.data.borrow_mut() = Some(app_data_json());
    *backend.notes.borrow_mut() = Some(serde_json::to_string(&notes).unwrap());
    let store = Store::load_with(Box::new(backend), Box::new(fixed_clock()));
    assert_eq!(store.kb.len(), 1, "旧 notes.json（无 folderId）仍能加载");
    assert_eq!(store.kb[0].folder_id, None, "缺字段 → None（未分类）");
    assert!(store.ensure_kb_ok().is_ok(), "旧数据不是损坏");
    // 写回必须显式 null：前端靠 folderId === null 判未分类，不能省略字段
    let value = serde_json::to_value(&store.kb[0]).unwrap();
    assert!(
      matches!(value.get("folderId"), Some(serde_json::Value::Null)),
      "None 必须序列化为 folderId: null：{value}"
    );
  }

  #[test]
  fn corrupt_notes_or_folders_surface_recoverable_error_not_empty_library() {
    // notes.json 坏
    let backend = InMemoryBackend::new();
    *backend.data.borrow_mut() = Some(app_data_json());
    *backend.notes.borrow_mut() = Some("{ 坏掉的 JSON".to_string());
    let store = Store::load_with(Box::new(backend), Box::new(fixed_clock()));
    assert_eq!(
      store.ensure_kb_ok().unwrap_err(),
      KB_CORRUPT_MESSAGE,
      "损坏时必须返回可恢复文案（前端引导去备份恢复页）"
    );
    assert!(store.kb.is_empty());
    assert_eq!(store.health, HEALTH_OK, "知识库损坏不走 data.json 的 corrupt 通道");

    // folders.json 坏同样进知识库错误态
    let backend = InMemoryBackend::new();
    *backend.data.borrow_mut() = Some(app_data_json());
    *backend.folders.borrow_mut() = Some("不是数组".to_string());
    let store = Store::load_with(Box::new(backend), Box::new(fixed_clock()));
    assert_eq!(store.ensure_kb_ok().unwrap_err(), KB_CORRUPT_MESSAGE);

    // 空文件 / 文件不存在 = 空库，不算损坏
    let backend = InMemoryBackend::new();
    *backend.data.borrow_mut() = Some(app_data_json());
    *backend.notes.borrow_mut() = Some("   ".to_string());
    *backend.folders.borrow_mut() = Some(String::new());
    let store = Store::load_with(Box::new(backend), Box::new(fixed_clock()));
    assert!(store.ensure_kb_ok().is_ok(), "空文件不是损坏");
    assert!(store.kb.is_empty() && store.folders.is_empty());

    // 损坏时写入必须被拒绝（save_notes 防御：不允许把空库覆盖回去）
    let backend = InMemoryBackend::new();
    *backend.data.borrow_mut() = Some(app_data_json());
    *backend.notes.borrow_mut() = Some("{ 坏".to_string());
    let mut store = Store::load_with(Box::new(backend), Box::new(fixed_clock()));
    store.kb = vec![kb_item("k1")];
    assert_eq!(store.save_notes().unwrap_err(), KB_CORRUPT_MESSAGE);
  }

  #[test]
  fn notes_rotation_and_kb_backup_restore_via_inmemory() {
    let mut store = mem_store();
    store.kb = vec![kb_item("k-a")];
    store.save_notes().unwrap();
    store.kb = vec![kb_item("k-b")];
    store.save_notes().unwrap();
    store.kb = vec![kb_item("k-c")];
    store.save_notes().unwrap();
    let backups = store.list_kb_backups();
    assert_eq!(backups.len(), 2, "三次写入留下两份历史（bak-1/bak-2）");
    assert_eq!(backups[0].slot, "1");
    assert_eq!(backups[0].name, "notes.json.bak-1");
    assert_eq!(backups[0].note_count, 1);
    assert!(backups[0].readable);
    // slot 2 = 再上一次写入（k-a）
    assert_eq!(store.restore_kb_backup("2").unwrap(), 1);
    assert_eq!(store.kb[0].id, "k-a");
    assert!(store.ensure_kb_ok().is_ok(), "恢复成功解除错误态");
    let backups = store.list_kb_backups();
    assert_eq!(backups.len(), 3, "恢复前先把当前 notes.json 轮转进 bak-1");
    assert_eq!(backups[0].note_count, 1);
    assert_eq!(
      store.restore_kb_backup("0").unwrap_err(),
      "备份只有 1 到 5 号，请重新选择"
    );
    assert_eq!(
      store.restore_kb_backup("9").unwrap_err(),
      "备份只有 1 到 5 号，请重新选择"
    );
    assert!(store.restore_kb_backup("abc").is_err(), "非法编号不落盘");
  }

  #[test]
  fn kb_restore_rejects_unreadable_or_malformed_backup() {
    // 没有备份
    let mut store = Store::with_backend(
      Box::new(InMemoryBackend::new()),
      Box::new(fixed_clock()),
    );
    assert_eq!(
      store.restore_kb_backup("1").unwrap_err(),
      "这个备份也读不了，换一个试试"
    );
    // 备份存在但不是合法知识库数组
    let backend = InMemoryBackend::new();
    backend
      .notes_backups
      .borrow_mut()
      .push("{ 坏掉的备份".to_string());
    let mut store = Store::with_backend(Box::new(backend), Box::new(fixed_clock()));
    assert_eq!(
      store.restore_kb_backup("1").unwrap_err(),
      "这个备份无法还原成可用的知识库"
    );
    assert!(store.kb.is_empty(), "解析失败绝不覆盖当前知识库");
  }

  /// 测试用 scratch 目录：先试系统 temp，建不出来（受限令牌 / 杀软瞬时拦截 /
  /// TEMP 指向受保护目录）再退到 `<crate>/target/test-tmp`，每个候选重试三次仍败才 panic。
  /// 只给必须落真实文件系统的测试用，其余测试一律走 InMemoryBackend。
  fn scratch_dir(tag: &str) -> std::path::PathBuf {
    let name = format!("kettd-{tag}-{}", new_id("t"));
    let candidates = [
      std::env::temp_dir().join(&name),
      std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-tmp")
        .join(&name),
    ];
    let mut failure = None;
    for candidate in candidates {
      for attempt in 0..3 {
        match std::fs::create_dir_all(&candidate) {
          Ok(()) => return candidate,
          Err(err) => {
            failure = Some((candidate.clone(), err));
            if attempt < 2 {
              std::thread::sleep(std::time::Duration::from_millis(50));
            }
          }
        }
      }
    }
    let (path, err) = failure.expect("候选目录不为空");
    panic!("测试临时目录建不出来：{}（{err}）", path.display());
  }

  #[test]
  fn fs_backend_persists_folders_and_rotates_notes_files() {
    let dir = scratch_dir("kb-test");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      let mut store = Store::with_backend(
        Box::new(FsBackend::new(dir.clone())),
        Box::new(fixed_clock()),
      );
      let folder = store.create_folder("工作", None).unwrap();
      store.save_folders().unwrap();
      let raw = std::fs::read_to_string(dir.join("folders.json")).unwrap();
      let folded: Value = serde_json::from_str(&raw).unwrap();
      assert!(folded.is_array(), "folders.json 根必须是裸数组");
      assert_eq!(folded[0]["id"], folder.id);
      assert_eq!(folded[0]["name"], "工作");
      assert_eq!(folded[0]["parentId"], Value::Null);

      store.kb = vec![kb_item("k1")];
      store.save_notes().unwrap();
      store.kb = vec![kb_item("k2")];
      store.save_notes().unwrap();
      store.kb = vec![kb_item("k3")];
      store.save_notes().unwrap();
      assert!(dir.join("notes.json.bak-1").exists());
      assert!(dir.join("notes.json.bak-2").exists());
      let bak1: Vec<crate::models::KbItemMeta> =
        serde_json::from_str(&std::fs::read_to_string(dir.join("notes.json.bak-1")).unwrap())
          .unwrap();
      assert_eq!(bak1[0].id, "k2", "bak-1 = 上一次写入");
      let restored = store.restore_kb_backup("2").unwrap();
      assert_eq!(restored, 1);
      let current: Vec<crate::models::KbItemMeta> =
        serde_json::from_str(&std::fs::read_to_string(dir.join("notes.json")).unwrap()).unwrap();
      assert_eq!(current[0].id, "k1");
      let raw_index: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("notes.json")).unwrap()).unwrap();
      for entry in raw_index.as_array().unwrap() {
        assert!(
          entry.get("bodyMd").is_none() && entry.get("body_md").is_none(),
          "notes.json 必须是纯索引（不含正文）：{entry}"
        );
      }
      let backups = store.list_kb_backups();
      assert!(backups.iter().all(|item| item.readable));
    }));
    let _ = std::fs::remove_dir_all(&dir);
    if let Err(panic) = result {
      std::panic::resume_unwind(panic);
    }
  }

  // ---- kb-storage-backup 批 2：索引/正文分离（D1）+ 旧格式迁移（D7） ----

  fn kb_hash(body: &str) -> String {
    crate::store_notes::content_hash(body)
  }

  /// 旧格式（内联 bodyMd）→ 分文件结构：条目数/标题/正文一致；幂等且 notes.v1.json 只写一次。
  #[test]
  fn legacy_notes_migrate_to_split_storage_idempotently() {
    let legacy = json!([
      {
        "id": "k1",
        "title": "  旧标题 ",
        "bodyMd": "\n第一行\n正文 \n",
        "tags": [" rust ", "", "rust"],
        "folderId": "f1",
        "order": 0,
        "pinned": true,
        "createdAt": "2026-09-01T09:00",
        "updatedAt": "2026-09-02T09:00"
      },
      {
        "id": "k2",
        "title": "空正文",
        "bodyMd": "",
        "tags": [],
        "createdAt": "2026-09-03T09:00",
        "updatedAt": "2026-09-03T09:00"
      }
    ]);
    let legacy_text = serde_json::to_string(&legacy).unwrap();
    let backend = InMemoryBackend::new();
    *backend.data.borrow_mut() = Some(app_data_json());
    *backend.notes.borrow_mut() = Some(legacy_text.clone());
    *backend.folders.borrow_mut() = Some(
      serde_json::to_string(&json!([
        { "id": "f1", "name": "工作", "parentId": null, "order": 0 }
      ]))
      .unwrap(),
    );
    let mut store = Store::with_backend(Box::new(backend), Box::new(fixed_clock()));
    store.load_kb();
    assert!(store.ensure_kb_ok().is_ok(), "旧格式迁移成功不是损坏态");
    assert_eq!(store.kb.len(), 2, "迁移后条目数一致");
    assert_eq!(store.kb[0].title, "旧标题", "标题迁移时 trim");
    assert_eq!(store.kb[0].body_md, "第一行\n正文 ", "正文只裁首尾换行");
    assert_eq!(store.kb[0].tags, vec!["rust"], "标签去空去重");
    assert_eq!(store.kb[0].folder_id.as_deref(), Some("f1"), "文件夹归属保留");
    assert!(store.kb[0].pinned);
    let expected_hash = kb_hash("第一行\n正文 ");
    assert_eq!(store.kb[0].content_hash.as_deref(), Some(expected_hash.as_str()));
    assert_eq!(store.kb[1].body_md, "");
    // ① 永久存档 + ② 轮转：bak-1 = 旧 notes.json
    assert_eq!(
      store.backend.read_notes_v1_archive().unwrap(),
      Some(legacy_text.clone()),
      "notes.v1.json = 旧格式原文"
    );
    assert_eq!(store.list_kb_backups().len(), 1, "迁移前先把旧 notes.json 轮转进 bak-1");
    // ③ notes.json 变纯索引（不含正文）
    let index: Value =
      serde_json::from_str(&store.backend.read_notes().unwrap().unwrap()).unwrap();
    assert!(
      index[0].get("bodyMd").is_none() && index[0].get("body_md").is_none(),
      "索引 MUST NOT 内联正文：{index}"
    );
    assert_eq!(index[0]["contentHash"], expected_hash);
    // ④ 正文逐条落盘
    assert_eq!(
      store.backend.read_note_body("k1").unwrap().as_deref(),
      Some("第一行\n正文 ")
    );
    assert_eq!(store.backend.read_note_body("k2").unwrap().as_deref(), Some(""));
    // 幂等：回滚后再次启动重跑迁移 → 正文覆盖、存档不重写、不产生重复条目
    store.backend.write_note_body("k1", "被篡改的正文").unwrap();
    store.backend.write_notes(&legacy_text).unwrap();
    store.load_kb();
    assert_eq!(
      store.backend.read_notes_v1_archive().unwrap(),
      Some(legacy_text.clone()),
      "notes.v1.json 只写一次（保留最早原文）"
    );
    assert_eq!(
      store.backend.read_note_body("k1").unwrap().as_deref(),
      Some("第一行\n正文 "),
      "重跑覆盖同 id 正文"
    );
    assert_eq!(store.kb.len(), 2, "重跑不产生重复条目");
    assert_eq!(store.list_kb_backups().len(), 2, "重跑再轮转一次");
    let index: Value =
      serde_json::from_str(&store.backend.read_notes().unwrap().unwrap()).unwrap();
    assert_eq!(index.as_array().unwrap().len(), 2);
  }

  /// 迁移失败（只读盘/空间不足）：KB_UPGRADE_FAILED、旧 notes.json 原样保留、下次启动可重试。
  #[test]
  fn legacy_migration_failure_keeps_original_notes_and_reports_upgrade_error() {
    let legacy_text = serde_json::to_string(&json!([
      { "id": "k1", "title": "旧", "bodyMd": "正文", "tags": [], "createdAt": "2026-09-01T09:00", "updatedAt": "2026-09-01T09:00" }
    ]))
    .unwrap();
    let backend = InMemoryBackend::new();
    *backend.data.borrow_mut() = Some(app_data_json());
    *backend.notes.borrow_mut() = Some(legacy_text.clone());
    backend.set_fail_writes(true);
    let mut store = Store::with_backend(Box::new(backend), Box::new(fixed_clock()));
    store.load_kb();
    assert_eq!(
      store.ensure_kb_ok().unwrap_err(),
      crate::kb_errors::KB_UPGRADE_FAILED,
      "迁移失败是专属「升级失败」错误态（不是损坏、不是空库）"
    );
    assert_eq!(
      store.backend.read_notes().unwrap(),
      Some(legacy_text.clone()),
      "失败时旧 notes.json 原样保留"
    );
    assert_eq!(
      store.backend.read_notes_v1_archive().unwrap(),
      None,
      "失败时不留下半截新索引/存档之外的副作用"
    );
    // 下次启动重试：同一份旧 notes.json + 可写盘 → 迁移成功
    let retry = InMemoryBackend::new();
    *retry.data.borrow_mut() = Some(app_data_json());
    *retry.notes.borrow_mut() = Some(legacy_text);
    let mut store = Store::with_backend(Box::new(retry), Box::new(fixed_clock()));
    store.load_kb();
    assert!(store.ensure_kb_ok().is_ok(), "可写后下次启动重试成功");
    assert_eq!(store.kb.len(), 1);
    assert_eq!(store.kb[0].body_md, "正文");
  }

  /// 新格式索引 + 正文文件缺失：空串加载、条目不丢、不判损坏；hash 顺手修复并落盘。
  #[test]
  fn new_index_missing_body_loads_empty_and_repairs_hash() {
    let index = json!([{
      "id": "k1",
      "title": "标题",
      "tags": [],
      "folderId": null,
      "order": 0,
      "pinned": false,
      "createdAt": "2026-09-01T09:00",
      "updatedAt": "2026-09-01T09:00",
      "contentHash": "stale"
    }]);
    let backend = InMemoryBackend::new();
    *backend.data.borrow_mut() = Some(app_data_json());
    *backend.notes.borrow_mut() = Some(serde_json::to_string(&index).unwrap());
    let mut store = Store::with_backend(Box::new(backend), Box::new(fixed_clock()));
    store.load_kb();
    assert!(store.ensure_kb_ok().is_ok(), "正文缺失按空串，不判损坏");
    assert_eq!(store.kb.len(), 1, "条目不丢");
    assert_eq!(store.kb[0].body_md, "");
    let empty_hash = kb_hash("");
    assert_eq!(store.kb[0].content_hash.as_deref(), Some(empty_hash.as_str()));
    let repaired: Value =
      serde_json::from_str(&store.backend.read_notes().unwrap().unwrap()).unwrap();
    assert_eq!(repaired[0]["contentHash"], empty_hash, "可修复差异顺手落盘修正索引");
  }

  /// contentHash 被篡改：以正文为准重算（D1 可修复，不判损坏）并落盘修正。
  #[test]
  fn new_index_tampered_hash_is_corrected_from_body() {
    let index = json!([{
      "id": "k1",
      "title": "标题",
      "tags": [],
      "folderId": null,
      "order": 0,
      "pinned": false,
      "createdAt": "2026-09-01T09:00",
      "updatedAt": "2026-09-01T09:00",
      "contentHash": "tampered"
    }]);
    let backend = InMemoryBackend::new();
    *backend.data.borrow_mut() = Some(app_data_json());
    *backend.notes.borrow_mut() = Some(serde_json::to_string(&index).unwrap());
    backend
      .note_bodies
      .borrow_mut()
      .insert("k1".to_string(), "真实正文".to_string());
    let mut store = Store::with_backend(Box::new(backend), Box::new(fixed_clock()));
    store.load_kb();
    assert!(store.ensure_kb_ok().is_ok(), "hash 不一致不是损坏");
    assert_eq!(store.kb[0].body_md, "真实正文", "正文是唯一真相");
    let actual_hash = kb_hash("真实正文");
    assert_eq!(store.kb[0].content_hash.as_deref(), Some(actual_hash.as_str()));
    let repaired: Value =
      serde_json::from_str(&store.backend.read_notes().unwrap().unwrap()).unwrap();
    assert_eq!(repaired[0]["contentHash"], actual_hash);
  }

  /// folderId 指向已删除文件夹：回退未分类、条目绝不丢（storage spec）。
  #[test]
  fn missing_folder_id_falls_back_to_unclassified_without_dropping_items() {
    let index = json!([
      { "id": "k1", "title": "孤儿", "tags": [], "folderId": "f-missing", "order": null, "pinned": false, "createdAt": "2026-09-01T09:00", "updatedAt": "2026-09-01T09:00", "contentHash": "x" },
      { "id": "k2", "title": "正常", "tags": [], "folderId": "f1", "order": 0, "pinned": false, "createdAt": "2026-09-02T09:00", "updatedAt": "2026-09-02T09:00", "contentHash": "y" }
    ]);
    let backend = InMemoryBackend::new();
    *backend.data.borrow_mut() = Some(app_data_json());
    *backend.notes.borrow_mut() = Some(serde_json::to_string(&index).unwrap());
    *backend.folders.borrow_mut() = Some(
      serde_json::to_string(&json!([
        { "id": "f1", "name": "工作", "parentId": null, "order": 0 }
      ]))
      .unwrap(),
    );
    let mut store = Store::with_backend(Box::new(backend), Box::new(fixed_clock()));
    store.load_kb();
    assert!(store.ensure_kb_ok().is_ok());
    assert_eq!(store.kb.len(), 2, "条目绝不因 folderId 失效丢弃");
    assert_eq!(store.kb[0].id, "k1");
    assert_eq!(store.kb[0].folder_id, None, "失效 folderId 回退未分类");
    assert_eq!(store.kb[1].folder_id.as_deref(), Some("f1"), "有效归属保持不变");
    let repaired: Value =
      serde_json::from_str(&store.backend.read_notes().unwrap().unwrap()).unwrap();
    assert!(matches!(repaired[0].get("folderId"), Some(Value::Null)), "回退结果落盘");
  }

  /// D1 写删顺序：先正文后索引；删除时先删正文再写索引（fail 时索引不指回缺失正文）。
  #[test]
  fn save_and_delete_body_keep_index_as_second_step() {
    let mut store = mem_store();
    store.kb.push(kb_item("k1"));
    store.save_kb_item_body("k1", "\n正文\n").unwrap();
    assert_eq!(
      store.backend.read_note_body("k1").unwrap().as_deref(),
      Some("\n正文\n"),
      "命令层第一步：正文落盘"
    );
    assert_eq!(
      store.backend.read_notes().unwrap(),
      None,
      "先正文后索引：save_kb_item_body 不写索引"
    );
    let body_hash = kb_hash("\n正文\n");
    assert_eq!(store.kb[0].content_hash.as_deref(), Some(body_hash.as_str()));
    assert_eq!(
      store.save_kb_item_body("missing", "x").unwrap_err(),
      "找不到这条资料，可能已经被删除"
    );
    store.save_notes().unwrap();
    let index: Value =
      serde_json::from_str(&store.backend.read_notes().unwrap().unwrap()).unwrap();
    assert!(index[0].get("bodyMd").is_none(), "索引只存元数据");
    // 删除顺序：先删正文（此刻旧索引仍在盘上）、再 save_notes 收敛索引
    store.kb.clear();
    store.delete_kb_item_body("k1").unwrap();
    assert_eq!(store.backend.read_note_body("k1").unwrap(), None);
    assert!(
      store.backend.read_notes().unwrap().is_some(),
      "正文先删、索引后写：此刻盘上仍是旧索引"
    );
    store.save_notes().unwrap();
    let index: Value =
      serde_json::from_str(&store.backend.read_notes().unwrap().unwrap()).unwrap();
    assert_eq!(index, json!([]), "save_notes 后索引不再指向已删条目");
  }

  /// 恢复旧格式备份（含 bodyMd）：走同一迁移 → 分文件结构，且写 notes.v1.json 永久存档。
  #[test]
  fn restore_legacy_backup_migrates_through_same_path() {
    let legacy_text = serde_json::to_string(&json!([
      {
        "id": "k-old",
        "title": "旧备份条目",
        "bodyMd": "备份正文",
        "tags": ["a"],
        "createdAt": "2026-09-01T09:00",
        "updatedAt": "2026-09-01T09:00"
      }
    ]))
    .unwrap();
    let backend = InMemoryBackend::new();
    *backend.data.borrow_mut() = Some(app_data_json());
    backend.notes_backups.borrow_mut().push(legacy_text.clone());
    let mut store = Store::with_backend(Box::new(backend), Box::new(fixed_clock()));
    let restored = store.restore_kb_backup("1").unwrap();
    assert_eq!(restored, 1);
    assert!(store.ensure_kb_ok().is_ok());
    assert_eq!(store.kb[0].id, "k-old");
    assert_eq!(store.kb[0].body_md, "备份正文");
    assert_eq!(
      store.backend.read_note_body("k-old").unwrap().as_deref(),
      Some("备份正文"),
      "恢复旧备份也落成分文件正文"
    );
    assert_eq!(
      store.backend.read_notes_v1_archive().unwrap(),
      Some(legacy_text),
      "旧备份也写永久存档（回滚兜底）"
    );
    let index: Value =
      serde_json::from_str(&store.backend.read_notes().unwrap().unwrap()).unwrap();
    assert!(index[0].get("bodyMd").is_none(), "恢复后直接是分文件索引");
  }

  /// 旧格式解析失败（缺 id / 顶层不是数组）：仍走 KB_CORRUPT_MESSAGE（损坏通道，不是升级失败）。
  #[test]
  fn legacy_notes_parse_failure_stays_corrupt_message() {
    // 旧格式但条目缺 id → 迁移解析失败 = 损坏
    let backend = InMemoryBackend::new();
    *backend.data.borrow_mut() = Some(app_data_json());
    *backend.notes.borrow_mut() =
      Some(serde_json::to_string(&json!([{ "bodyMd": "正文" }])).unwrap());
    let mut store = Store::with_backend(Box::new(backend), Box::new(fixed_clock()));
    store.load_kb();
    assert_eq!(store.ensure_kb_ok().unwrap_err(), KB_CORRUPT_MESSAGE);
    assert!(store.kb.is_empty());

    // 顶层不是数组（非旧格式）→ 索引解析失败 = 损坏
    let backend = InMemoryBackend::new();
    *backend.data.borrow_mut() = Some(app_data_json());
    *backend.notes.borrow_mut() = Some("\"字符串\"".to_string());
    let mut store = Store::with_backend(Box::new(backend), Box::new(fixed_clock()));
    store.load_kb();
    assert_eq!(store.ensure_kb_ok().unwrap_err(), KB_CORRUPT_MESSAGE);
  }

  /// 真实文件系统端到端：旧 notes.json → notes/<id>.md + notes.v1.json + 纯索引；重跑幂等。
  #[test]
  fn fs_backend_migrates_legacy_notes_into_split_files() {
    let dir = scratch_dir("kb-migrate");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      let legacy = json!([
        { "id": "k1", "title": "旧标题", "bodyMd": "第一行\n第二行", "tags": ["x"], "createdAt": "2026-09-01T09:00", "updatedAt": "2026-09-01T09:00" }
      ]);
      let legacy_text = serde_json::to_string(&legacy).unwrap();
      std::fs::write(dir.join("data.json"), app_data_json()).unwrap();
      std::fs::write(dir.join("notes.json"), &legacy_text).unwrap();
      std::fs::write(dir.join("folders.json"), "[]").unwrap();
      let mut store = Store::with_backend(
        Box::new(FsBackend::new(dir.clone())),
        Box::new(fixed_clock()),
      );
      store.load_kb();
      assert!(store.ensure_kb_ok().is_ok(), "旧格式文件迁移成功");
      assert_eq!(store.kb.len(), 1);
      assert_eq!(store.kb[0].body_md, "第一行\n第二行");
      assert_eq!(
        std::fs::read_to_string(dir.join("notes").join("k1.md")).unwrap(),
        "第一行\n第二行",
        "正文落 notes/<id>.md"
      );
      let index: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("notes.json")).unwrap()).unwrap();
      assert!(index[0].get("bodyMd").is_none(), "notes.json 变纯索引");
      assert_eq!(index[0]["contentHash"], kb_hash("第一行\n第二行"));
      assert_eq!(
        std::fs::read_to_string(dir.join("notes.v1.json")).unwrap(),
        legacy_text,
        "notes.v1.json = 旧格式原文"
      );
      assert_eq!(
        std::fs::read_to_string(dir.join("notes.json.bak-1")).unwrap(),
        legacy_text,
        "bak-1 = 迁移前旧 notes.json"
      );
      // 模拟「回滚旧版本后再启动」：旧 notes.json 放回 → 重跑迁移，正文覆盖、存档不重写
      std::fs::write(dir.join("notes.json"), &legacy_text).unwrap();
      let v1_before = std::fs::read_to_string(dir.join("notes.v1.json")).unwrap();
      let mut store = Store::with_backend(
        Box::new(FsBackend::new(dir.clone())),
        Box::new(fixed_clock()),
      );
      store.load_kb();
      assert_eq!(store.kb.len(), 1, "重跑幂等：不重复条目");
      assert_eq!(
        std::fs::read_to_string(dir.join("notes.v1.json")).unwrap(),
        v1_before,
        "notes.v1.json 只写一次"
      );
      assert!(
        std::fs::read_to_string(dir.join("notes.json"))
          .unwrap()
          .contains("contentHash"),
        "重跑后 notes.json 仍是新索引"
      );
    }));
    let _ = std::fs::remove_dir_all(&dir);
    if let Err(panic) = result {
      std::panic::resume_unwind(panic);
    }
  }

  // ---- kb-storage-backup 批 3：图片外置（D2）+ Bootstrap.assetsDir ----

  fn image_data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{};base64,{}", mime, crate::digest::base64_encode(bytes))
  }

  /// 保存正文：内联图片先写 assets（经 StoreBackend），正文只留相对引用；内存索引同步。
  #[test]
  fn save_body_extracts_inline_images_to_assets() {
    let mut store = mem_store();
    store.kb.push(kb_item("k1"));
    let payload = b"store-png-bytes".to_vec();
    let url = image_data_url("image/png", &payload);
    let sha = crate::digest::sha256_hex(&payload);
    store
      .save_kb_item_body("k1", &format!("看图\n![截图]({url})"))
      .unwrap();
    let expected = format!("看图\n![截图](assets/{sha}.png)");
    assert_eq!(store.kb[0].body_md, expected, "内存正文同步成相对引用");
    assert_eq!(
      store.kb[0].content_hash.as_deref(),
      Some(kb_hash(&expected).as_str()),
      "contentHash 按改写后正文重算"
    );
    assert_eq!(
      store.backend.read_note_body("k1").unwrap().as_deref(),
      Some(expected.as_str()),
      "先写资源再写正文"
    );
    assert_eq!(
      store.backend.read_asset(&sha, "png").unwrap(),
      payload,
      "资源落到 assets/<sha>.png"
    );
  }

  /// 旧格式迁移：正文里的内联图一并外置，索引 hash 按改写后正文重算，旧原文仍永久存档。
  #[test]
  fn legacy_notes_migration_extracts_inline_images() {
    let payload = b"legacy-image-bytes".to_vec();
    let url = image_data_url("image/jpeg", &payload);
    let sha = crate::digest::sha256_hex(&payload);
    let legacy = json!([{
      "id": "k1",
      "title": "旧图",
      "bodyMd": format!("旧文\n![照片]({url})"),
      "tags": [],
      "createdAt": "2026-09-01T09:00",
      "updatedAt": "2026-09-01T09:00"
    }]);
    let legacy_text = serde_json::to_string(&legacy).unwrap();
    let backend = InMemoryBackend::new();
    *backend.data.borrow_mut() = Some(app_data_json());
    *backend.notes.borrow_mut() = Some(legacy_text.clone());
    let mut store = Store::with_backend(Box::new(backend), Box::new(fixed_clock()));
    store.load_kb();
    assert!(store.ensure_kb_ok().is_ok(), "带图旧库迁移成功");
    let expected = format!("旧文\n![照片](assets/{sha}.jpg)");
    assert_eq!(store.kb[0].body_md, expected, "内存正文已改写");
    assert_eq!(
      store.backend.read_note_body("k1").unwrap().as_deref(),
      Some(expected.as_str()),
      "notes/k1.md 落的是改写后正文"
    );
    assert_eq!(
      store.backend.read_asset(&sha, "jpg").unwrap(),
      payload,
      "image/jpeg 落成 .jpg"
    );
    let index: Value =
      serde_json::from_str(&store.backend.read_notes().unwrap().unwrap()).unwrap();
    assert_eq!(index[0]["contentHash"], kb_hash(&expected), "索引 hash 按改写后正文");
    assert_eq!(
      store.backend.read_notes_v1_archive().unwrap(),
      Some(legacy_text),
      "旧原文仍进 notes.v1.json（回滚兜底）"
    );
  }

  /// 已迁移但正文仍内联的库：load_kb best-effort 回填（幂等），索引 hash 顺手收敛。
  #[test]
  fn load_backfills_inline_images_in_migrated_library() {
    let payload = b"backfill-bytes".to_vec();
    let url = image_data_url("image/webp", &payload);
    let sha = crate::digest::sha256_hex(&payload);
    let inline_body = format!("正文\n![图]({url})");
    let index = json!([{
      "id": "k1",
      "title": "带图",
      "tags": [],
      "folderId": null,
      "order": 0,
      "pinned": false,
      "createdAt": "2026-09-01T09:00",
      "updatedAt": "2026-09-01T09:00",
      "contentHash": kb_hash(&inline_body)
    }]);
    let backend = InMemoryBackend::new();
    *backend.data.borrow_mut() = Some(app_data_json());
    *backend.notes.borrow_mut() = Some(serde_json::to_string(&index).unwrap());
    backend
      .note_bodies
      .borrow_mut()
      .insert("k1".to_string(), inline_body.clone());
    let mut store = Store::with_backend(Box::new(backend), Box::new(fixed_clock()));
    store.load_kb();
    assert!(store.ensure_kb_ok().is_ok(), "回填失败不算损坏");
    let expected = format!("正文\n![图](assets/{sha}.webp)");
    assert_eq!(store.kb[0].body_md, expected);
    assert_eq!(
      store.backend.read_note_body("k1").unwrap().as_deref(),
      Some(expected.as_str())
    );
    assert_eq!(store.backend.read_asset(&sha, "webp").unwrap(), payload);
    let repaired: Value =
      serde_json::from_str(&store.backend.read_notes().unwrap().unwrap()).unwrap();
    assert_eq!(repaired[0]["contentHash"], kb_hash(&expected), "回填后索引落盘");
    // 幂等：再加载一遍不再有 data: 内含图，正文/资源都不变
    store.load_kb();
    assert_eq!(store.kb[0].body_md, expected);
    assert_eq!(store.backend.read_asset(&sha, "webp").unwrap(), payload);
  }

  /// 回填写盘失败：该条保留内联（绝不丢图/丢条目），不把库判成损坏。
  #[test]
  fn backfill_failure_keeps_inline_body() {
    let payload = b"fail-backfill".to_vec();
    let url = image_data_url("image/png", &payload);
    let sha = crate::digest::sha256_hex(&payload);
    let inline_body = format!("正文\n![图]({url})");
    let index = json!([{
      "id": "k1",
      "title": "带图",
      "tags": [],
      "folderId": null,
      "order": 0,
      "pinned": false,
      "createdAt": "2026-09-01T09:00",
      "updatedAt": "2026-09-01T09:00",
      "contentHash": kb_hash(&inline_body)
    }]);
    let backend = InMemoryBackend::new();
    *backend.data.borrow_mut() = Some(app_data_json());
    *backend.notes.borrow_mut() = Some(serde_json::to_string(&index).unwrap());
    backend
      .note_bodies
      .borrow_mut()
      .insert("k1".to_string(), inline_body.clone());
    backend.set_fail_writes(true);
    let mut store = Store::with_backend(Box::new(backend), Box::new(fixed_clock()));
    store.load_kb();
    assert!(store.ensure_kb_ok().is_ok(), "写不进不是损坏");
    assert_eq!(store.kb[0].body_md, inline_body, "正文保留内联，绝不丢图");
    assert!(
      store.backend.read_asset(&sha, "png").is_err(),
      "失败时没有半截资源落盘"
    );
  }

  /// Bootstrap.assetsDir：生产/文件后端 = 数据目录下 assets 的绝对路径；内存后端给临时占位。
  #[test]
  fn assets_dir_comes_from_backend() {
    let mem = mem_store();
    let placeholder = mem.assets_dir();
    assert!(
      placeholder.contains("kettd-assets"),
      "内存后端给临时占位，不冒充真实目录：{placeholder}"
    );

    let dir = scratch_dir("kb-assets-dir");
    let store = Store::with_backend(Box::new(FsBackend::new(dir.clone())), Box::new(fixed_clock()));
    assert_eq!(
      store.assets_dir(),
      dir.join("assets").to_string_lossy(),
      "文件后端给 <数据目录>/assets 的绝对路径"
    );
    assert!(std::path::Path::new(&store.assets_dir()).is_absolute());
    let _ = std::fs::remove_dir_all(&dir);
  }

  // ---- kb-storage-backup 批 3：手动导出 / 导入（D3 / D4）往返演练 ----

  use crate::models::StickyNote;
  use std::collections::BTreeMap;

  /// 备份源（导出方）与导入目标共用的两个资源：一个目标目录里已有（reused），
  /// 一个没有（added）；另有一条正文引用指向缺失资源（导出跳过、导入不阻断）。
  struct BackupFixture {
    result: backup::ExportResult,
    payload: String,
    missing_sha: String,
    reuse_sha: String,
    new_sha: String,
  }

  fn sha_hex(bytes: &[u8]) -> String {
    crate::digest::sha256_hex(bytes)
  }

  fn backup_assets() -> (Vec<u8>, String, Vec<u8>, String) {
    let reuse = b"kettd-backup-reuse-png".to_vec();
    let new = b"kettd-backup-new-webp".to_vec();
    let reuse_sha = sha_hex(&reuse);
    let new_sha = sha_hex(&new);
    (reuse, reuse_sha, new, new_sha)
  }

  fn task_fixture(id: &str, title: &str, note: &str, updated_at: &str) -> Task {
    Task {
      id: id.to_string(),
      title: title.to_string(),
      note: note.to_string(),
      created_at: "2026-09-01T08:00".to_string(),
      updated_at: updated_at.to_string(),
      ..Default::default()
    }
  }

  fn reminder_fixture(id: &str, title: &str, time: &str) -> Reminder {
    Reminder {
      id: id.to_string(),
      title: title.to_string(),
      time: time.to_string(),
      ..Default::default()
    }
  }

  fn kb_fixture(id: &str, title: &str, body: &str, updated_at: &str) -> KbItem {
    KbItem {
      id: id.to_string(),
      title: title.to_string(),
      body_md: body.to_string(),
      created_at: "2026-09-01T08:00".to_string(),
      updated_at: updated_at.to_string(),
      content_hash: Some(kb_hash(body)),
      ..Default::default()
    }
  }

  fn folder_fixture(id: &str, name: &str, order: i64) -> KbFolder {
    KbFolder {
      id: id.to_string(),
      name: name.to_string(),
      parent_id: None,
      order,
      pinned: false,
    }
  }

  /// 备份源：在 `dir` 上搭 Store 并 `export_backup_to_file`。
  /// 备份时间线设计（越新越靠后）：本地较旧 / 备份较新 / 同刻不同内容 / 本地独有 / 本地已删。
  fn seed_backup_source(dir: &std::path::Path) -> BackupFixture {
    let (reuse_bytes, reuse_sha, new_bytes, new_sha) = backup_assets();
    let mut source = Store::with_backend(
      Box::new(FsBackend::new(dir.to_path_buf())),
      Box::new(fixed_clock()),
    );
    source
      .backend
      .write_asset(&reuse_sha, "png", &reuse_bytes)
      .unwrap();
    source
      .backend
      .write_asset(&new_sha, "webp", &new_bytes)
      .unwrap();
    let missing_sha = "a".repeat(64);
    let body_assets = format!(
      "配图 ![a](assets/{reuse_sha}.png)、![b](assets/{new_sha}.webp)、![m](assets/{missing_sha}.png)"
    );
    source.kb = vec![
      kb_fixture("k-upd", "备份版本", "备份正文", "2026-09-05T10:00"),
      kb_fixture("k-skip", "备份旧版", "备份旧正文", "2026-09-01T08:00"),
      kb_fixture("k-conflict", "冲突条目", "备份冲突正文", "2026-09-02T08:00"),
      kb_fixture("k-deleted", "本地已删", "备份里保留", "2026-09-04T08:00"),
      kb_fixture("k-assets", "带图条目", &body_assets, "2026-09-03T08:00"),
    ];
    source.folders = vec![
      folder_fixture("f-new", "新文件夹", 0),
      folder_fixture("f-same", "相同文件夹", 0),
      folder_fixture("f-conflict", "备份文件夹名", 0),
    ];
    source.data.tasks = vec![
      task_fixture("t-upd", "备份标题", "备份备注", "2026-09-05T10:00"),
      task_fixture("t-skip", "备份旧标题", "备份旧备注", "2026-09-01T08:00"),
      task_fixture("t-conflict", "冲突任务", "备份备注", "2026-09-02T08:00"),
    ];
    source.data.reminders = vec![
      reminder_fixture("r-new", "新提醒", "08:00"),
      reminder_fixture("r-same", "相同提醒", "09:00"),
      reminder_fixture("r-conflict", "备份提醒版本", "07:00"),
    ];
    let mut settings = Settings::default();
    settings.theme = "dark".to_string();
    source.data.settings = settings;
    source.data.stickies = vec![StickyNote {
      id: "s-backup".to_string(),
      content: "备份便签".to_string(),
      ..Default::default()
    }];
    let result = source
      .export_backup_to_file("9.9.9-test", "2026-09-05T09:00")
      .expect("导出不应失败");
    let payload = std::fs::read_to_string(&result.path).expect("备份文件应可读");
    BackupFixture {
      result,
      payload,
      missing_sha,
      reuse_sha,
      new_sha,
    }
  }

  /// 导入目标：本地状态完整落盘（data.json / notes.json / notes/<id>.md / folders.json / assets）。
  fn seed_backup_target(dir: &std::path::Path) -> Store {
    let (reuse_bytes, reuse_sha, _, _) = backup_assets();
    let mut target = Store::with_backend(
      Box::new(FsBackend::new(dir.to_path_buf())),
      Box::new(fixed_clock()),
    );
    // 目标已存在其中一个资源：导入时计 asset_reused，不重写。
    target
      .backend
      .write_asset(&reuse_sha, "png", &reuse_bytes)
      .unwrap();
    target.data.tasks = vec![
      task_fixture("t-upd", "本地旧标题", "本地旧备注", "2026-09-01T08:00"),
      task_fixture("t-skip", "本地较新", "本地较新备注", "2026-09-04T08:00"),
      task_fixture("t-conflict", "冲突任务", "本地备注", "2026-09-02T08:00"),
      task_fixture("t-local", "本地独有", "本地独有备注", "2026-09-03T08:00"),
    ];
    target.data.reminders = vec![
      reminder_fixture("r-same", "相同提醒", "09:00"),
      reminder_fixture("r-conflict", "本地提醒", "07:30"),
    ];
    target.data.stickies = vec![StickyNote {
      id: "s-local".to_string(),
      content: "本地便签".to_string(),
      ..Default::default()
    }];
    target.kb = vec![
      kb_fixture("k-upd", "本地旧版", "本地旧正文", "2026-09-01T08:00"),
      kb_fixture("k-skip", "本地较新", "本地正文", "2026-09-03T08:00"),
      kb_fixture("k-conflict", "冲突条目", "本地冲突正文", "2026-09-02T08:00"),
      kb_fixture("k-local", "本地独有", "本地独有正文", "2026-09-04T08:00"),
    ];
    target.folders = vec![
      folder_fixture("f-same", "相同文件夹", 0),
      folder_fixture("f-conflict", "本地文件夹名", 0),
      folder_fixture("f-local", "本地独有文件夹", 1),
    ];
    let entries: Vec<(String, String)> = target
      .kb
      .iter()
      .map(|item| (item.id.clone(), item.body_md.clone()))
      .collect();
    for (id, body) in entries {
      target.save_kb_item_body(&id, &body).unwrap();
    }
    target.save_notes().unwrap();
    target.save_folders().unwrap();
    target.save().unwrap();
    target
  }

  /// 目录内容 + 大小 + mtime 快照：dry-run / 幂等断言「一个字节都没写、mtime 都没变」。
  fn snapshot_dir(
    dir: &std::path::Path,
  ) -> BTreeMap<std::path::PathBuf, (u128, u64, Vec<u8>)> {
    fn walk(
      base: &std::path::Path,
      current: &std::path::Path,
      out: &mut BTreeMap<std::path::PathBuf, (u128, u64, Vec<u8>)>,
    ) {
      let mut entries: Vec<_> = std::fs::read_dir(current)
        .unwrap()
        .map(|entry| entry.unwrap())
        .collect();
      entries.sort_by_key(|entry| entry.path());
      for entry in entries {
        let path = entry.path();
        if path.is_dir() {
          walk(base, &path, out);
        } else {
          let metadata = std::fs::metadata(&path).unwrap();
          let modified = metadata
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
          let bytes = std::fs::read(&path).unwrap();
          out.insert(
            path.strip_prefix(base).unwrap().to_path_buf(),
            (modified, metadata.len(), bytes),
          );
        }
      }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
  }

  #[test]
  fn backup_export_writes_single_file_and_counts_skipped_assets() {
    let dir = scratch_dir("backup-export");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      let fixture = seed_backup_source(&dir);
      // 单文件：<数据目录>/backups/kettd-backup-<本地时间戳>.kettdbackup
      let backup_dir = dir.join("backups");
      let names: Vec<String> = std::fs::read_dir(&backup_dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
      assert_eq!(names, vec!["kettd-backup-20260905-090000.kettdbackup"]);
      let path = backup_dir.join(&names[0]);
      assert!(path.is_absolute());
      assert_eq!(fixture.result.path, path.to_string_lossy());
      assert_eq!(fixture.result.bytes, fixture.payload.len());
      assert_eq!(std::fs::read(&path).unwrap().len(), fixture.result.bytes);
      // 组装计数：任务 3 + 提醒 3 + 便签 1 + 知识条目 5 = 12；文件夹不计数
      assert_eq!(fixture.result.item_count, 12);
      assert_eq!(fixture.result.asset_count, 2);
      assert_eq!(fixture.result.skipped, 1, "缺失资源只计 skipped，不阻断导出");

      // 备份包可被 parse_backup 回读，结构 / 头字段 / 内容齐全
      let parsed = backup::parse_backup(&fixture.payload).expect("导出的备份应能回读");
      assert_eq!(parsed.file.format, "kettd-backup");
      assert_eq!(parsed.file.format_version, 1);
      assert_eq!(parsed.file.app_version, "9.9.9-test");
      assert_eq!(parsed.file.exported_at, "2026-09-05T09:00");
      assert_eq!(parsed.file.data.tasks.len(), 3);
      assert_eq!(parsed.file.data.reminders.len(), 3);
      assert_eq!(parsed.file.data.stickies.len(), 1);
      assert_eq!(parsed.file.data.settings["theme"], "dark");
      assert_eq!(parsed.file.kb.items.len(), 5);
      assert_eq!(parsed.file.kb.folders.len(), 3);
      // 只导出正文引用到的资源；缺失引用不出现、也不算坏包
      assert_eq!(parsed.asset_count(), 2);
      assert_eq!(parsed.reusable_asset_count(), 2);
      assert!(parsed.asset(&fixture.reuse_sha, "png").is_some());
      assert!(parsed.asset(&fixture.new_sha, "webp").is_some());
      assert!(parsed.asset(&fixture.missing_sha, "png").is_none());
      // 导出是纯读：不写 data.json / notes.json，也不轮转
      assert!(!dir.join("data.json").exists());
      assert!(!dir.join("notes.json").exists());
      assert!(!dir.join("notes").exists());
      assert!(!dir.join("data.json.bak-1").exists());
    }));
    let _ = std::fs::remove_dir_all(&dir);
    if let Err(panic) = result {
      std::panic::resume_unwind(panic);
    }
  }

  #[test]
  fn backup_import_dry_run_reports_without_writing() {
    let source_dir = scratch_dir("backup-dry-src");
    let target_dir = scratch_dir("backup-dry-dst");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      let fixture = seed_backup_source(&source_dir);
      let mut target = seed_backup_target(&target_dir);
      let files_before = snapshot_dir(&target_dir);
      let tasks_before = target.data.tasks.clone();
      let reminders_before = target.data.reminders.clone();
      let kb_before = target.kb.clone();
      let folders_before = target.folders.clone();
      let settings_before = target.data.settings.clone();
      let stickies_before = target.data.stickies.clone();

      let report = target
        .import_backup_from_payload(&fixture.payload, true, "2026-09-05T09:00")
        .expect("dry-run 不应失败");
      assert!(report.dry_run, "dry-run 必须在报告里标记");
      assert_eq!(
        report.added, 6,
        "新增 = 四类 Add（提醒 1 + 知识 2 + 文件夹 1）+ 2 个冲突副本"
      );
      assert_eq!(report.updated, 2, "备份较新的任务 / 知识条目各一条覆盖");
      assert_eq!(report.conflicts, 2, "同刻不同内容 = 任务 1 + 知识条目 1");
      assert_eq!(report.skipped, 6, "本地较新 / 指纹相同 / 提醒与文件夹冲突降级");
      assert_eq!(report.asset_added, 1);
      assert_eq!(report.asset_reused, 1);
      assert_eq!(
        report.conflict_titles,
        vec!["冲突任务".to_string(), "冲突条目".to_string()],
        "冲突标题按本地顺序：任务在前、知识条目在后"
      );

      // dry-run 一个文件都不许写（内容 + 大小 + mtime 全等），内存也不许动
      assert_eq!(
        snapshot_dir(&target_dir),
        files_before,
        "dry-run 必须零写盘（含 mtime）"
      );
      assert_eq!(target.data.tasks, tasks_before);
      assert_eq!(target.data.reminders, reminders_before);
      assert_eq!(target.kb, kb_before);
      assert_eq!(target.folders, folders_before);
      assert_eq!(target.data.settings, settings_before);
      assert_eq!(target.data.stickies, stickies_before);
    }));
    let _ = std::fs::remove_dir_all(&source_dir);
    let _ = std::fs::remove_dir_all(&target_dir);
    if let Err(panic) = result {
      std::panic::resume_unwind(panic);
    }
  }

  #[test]
  fn backup_import_applies_merge_matrix_on_disk() {
    let source_dir = scratch_dir("backup-apply-src");
    let target_dir = scratch_dir("backup-apply-dst");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      let fixture = seed_backup_source(&source_dir);
      let mut target = seed_backup_target(&target_dir);
      let settings_before = target.data.settings.clone();
      let stickies_before = target.data.stickies.clone();

      let report = target
        .import_backup_from_payload(&fixture.payload, false, "2026-09-05T09:00")
        .expect("导入不应失败");
      assert!(!report.dry_run);
      assert_eq!(
        (report.added, report.updated, report.conflicts, report.skipped),
        (6, 2, 2, 6)
      );
      assert_eq!((report.asset_added, report.asset_reused), (1, 1));

      // 任务：较新覆盖 / 本地较新跳过 / 冲突保留本地并生成副本 / 本地独有不删
      let task = |id: &str| target.data.tasks.iter().find(|task| task.id == id).unwrap();
      assert_eq!(task("t-upd").title, "备份标题", "备份较新 → 覆盖");
      assert_eq!(task("t-skip").title, "本地较新", "本地较新 → 跳过");
      assert_eq!(task("t-conflict").title, "冲突任务", "冲突保留本地");
      assert_eq!(task("t-conflict").note, "本地备注");
      assert_eq!(task("t-local").title, "本地独有", "本地独有不删");
      let task_copy = target
        .data
        .tasks
        .iter()
        .find(|task| task.title == "冲突任务（导入冲突）")
        .expect("冲突任务应生成副本");
      assert_ne!(task_copy.id, "t-conflict");
      assert_eq!(task_copy.updated_at, "2026-09-05T09:00");
      assert_eq!(target.data.tasks.len(), 5);

      // 提醒：新增；提醒冲突只保留本地（skip）
      assert_eq!(target.data.reminders.len(), 3);
      assert!(target.data.reminders.iter().any(|item| item.id == "r-new"));
      let reminder_conflict = target
        .data
        .reminders
        .iter()
        .find(|item| item.id == "r-conflict")
        .unwrap();
      assert_eq!(reminder_conflict.title, "本地提醒", "提醒冲突保留本地");

      // 知识条目：覆盖 / 跳过 / 冲突副本 / 本地已删补回 / 带图条目新增
      let kb = |id: &str| target.kb.iter().find(|item| item.id == id).unwrap();
      assert_eq!(kb("k-upd").title, "备份版本");
      assert_eq!(kb("k-upd").body_md, "备份正文");
      assert_eq!(
        kb("k-upd").content_hash.as_deref(),
        Some(kb_hash("备份正文").as_str())
      );
      assert_eq!(kb("k-skip").title, "本地较新");
      assert_eq!(kb("k-conflict").body_md, "本地冲突正文");
      assert_eq!(kb("k-local").title, "本地独有");
      assert_eq!(kb("k-deleted").title, "本地已删");
      assert_eq!(kb("k-assets").title, "带图条目");
      let kb_copy = target
        .kb
        .iter()
        .find(|item| item.title == "冲突条目（导入冲突）")
        .expect("冲突条目应生成副本");
      assert_eq!(kb_copy.body_md, "备份冲突正文");
      assert_eq!(target.kb.len(), 7);

      // 文件夹：新增 / 相同跳过 / 冲突降级 Skip 保留本地 / 本地独有不删
      let folder = |id: &str| target.folders.iter().find(|folder| folder.id == id).unwrap();
      assert_eq!(folder("f-new").name, "新文件夹");
      assert_eq!(folder("f-same").name, "相同文件夹");
      assert_eq!(folder("f-conflict").name, "本地文件夹名");
      assert_eq!(folder("f-local").name, "本地独有文件夹");
      assert_eq!(target.folders.len(), 4);

      // 设置与便签不参与合并，导入一律不覆盖
      assert_eq!(target.data.settings, settings_before, "设置不覆盖");
      assert_eq!(target.data.stickies, stickies_before, "便签不覆盖");

      // 落盘顺序可观察结果：正文文件、索引（无内联正文）、资源、data.json 都在
      let body = std::fs::read_to_string(target_dir.join("notes").join("k-upd.md")).unwrap();
      assert_eq!(body, "备份正文");
      assert!(target_dir.join("notes").join("k-deleted.md").exists());
      let index: Value =
        serde_json::from_str(&std::fs::read_to_string(target_dir.join("notes.json")).unwrap())
          .unwrap();
      let entry = index
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == "k-upd")
        .unwrap();
      assert_eq!(entry["title"], "备份版本");
      assert_eq!(entry["contentHash"].as_str(), Some(kb_hash("备份正文").as_str()));
      assert!(
        entry.get("bodyMd").is_none() && entry.get("body_md").is_none(),
        "索引不得内联正文"
      );
      assert!(target_dir
        .join("assets")
        .join(format!("{}.webp", fixture.new_sha))
        .exists());
      assert!(target_dir
        .join("assets")
        .join(format!("{}.png", fixture.reuse_sha))
        .exists());
      assert!(target_dir.join("data.json").exists());
      // 导入不产生新的备份包（备份文件只在导出一侧）
      let extra_backups = std::fs::read_dir(target_dir.join("backups"))
        .map(|entries| entries.count())
        .unwrap_or(0);
      assert_eq!(extra_backups, 0);
    }));
    let _ = std::fs::remove_dir_all(&source_dir);
    let _ = std::fs::remove_dir_all(&target_dir);
    if let Err(panic) = result {
      std::panic::resume_unwind(panic);
    }
  }

  #[test]
  fn backup_import_is_idempotent_on_second_run() {
    let source_dir = scratch_dir("backup-idem-src");
    let target_dir = scratch_dir("backup-idem-dst");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      let fixture = seed_backup_source(&source_dir);
      let mut target = seed_backup_target(&target_dir);
      let first = target
        .import_backup_from_payload(&fixture.payload, false, "2026-09-05T09:00")
        .expect("首次导入不应失败");
      assert_eq!((first.added, first.updated, first.conflicts), (6, 2, 2));
      let files_after_first = snapshot_dir(&target_dir);
      let tasks_after_first = target.data.tasks.clone();
      let kb_after_first = target.kb.clone();

      let second = target
        .import_backup_from_payload(&fixture.payload, false, "2026-09-05T09:00")
        .expect("第二次导入不应失败");
      assert_eq!(
        (second.added, second.updated, second.conflicts),
        (0, 0, 0),
        "同 payload 第二次导入全部 Skip（冲突副本不再生成）"
      );
      assert_eq!(second.skipped, 14, "备份内 14 条条目第二次全部计入 skipped");
      assert_eq!((second.asset_added, second.asset_reused), (0, 2));
      assert!(!second.dry_run);

      // 零写盘：目录内容 / 大小 / mtime 与首次应用后完全一致
      assert_eq!(
        snapshot_dir(&target_dir),
        files_after_first,
        "第二次导入不得产生二次副作用"
      );
      assert_eq!(target.data.tasks, tasks_after_first);
      assert_eq!(target.kb, kb_after_first);
      assert_eq!(
        target
          .data
          .tasks
          .iter()
          .filter(|task| task.title == "冲突任务（导入冲突）")
          .count(),
        1,
        "冲突副本不得重复生成"
      );
      assert_eq!(
        target
          .kb
          .iter()
          .filter(|item| item.title == "冲突条目（导入冲突）")
          .count(),
        1,
        "知识条目冲突副本不得重复生成"
      );
    }));
    let _ = std::fs::remove_dir_all(&source_dir);
    let _ = std::fs::remove_dir_all(&target_dir);
    if let Err(panic) = result {
      std::panic::resume_unwind(panic);
    }
  }
}


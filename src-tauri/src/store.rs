//! 存储安全层（PRD 6.6）
//!
//! 根治 v1 三个病灶：
//! 1. `unwrap_or_default()` 静默清空 → 解析失败一律进 corrupt 通道，写命令全部拒绝
//! 2. 裸 `fs::write` 全量覆盖 → 轮转备份 + tmp + rename 原子替换
//! 3. 时间漂移 / snake_case → 本地语义归一 + camelCase 序列化 + 迁移前原件永久留档
//!
//! 数据目录沿用 v1 的 `%APPDATA%/todo-list`（兼容发现）。

use crate::infra::fs_store::{default_dir, FsBackend, ARCHIVE_FILE};
use crate::models::{
  carry_days, clocks_of, fmt_day, fmt_dt, new_id, normalize_date_text, normalize_datetime, now_text, today,
  patch_datetime, patch_text, parse_wall, to_local, AppData, BackupInfo, CATEGORIES,
  DataHealthV2, KbItem, MigrationIssue, MigrationReport, Note, PRIORITIES, Reminder, ReminderPayload,
  RuntimeState,
  Repeat, Settings, SettingsPayload, Source, STICKY_PAPERS, Subtask, Task, TaskPayload,
  TRASH_RETENTION_DAYS,
};
use crate::ports::clock::{Clock, SystemClock};
use crate::ports::store_backend::StoreBackend;
use chrono::{DateTime, Duration, Local};
use serde_json::{json, Value};

pub use crate::infra::fs_store::BACKUP_SLOTS;

pub const HEALTH_OK: &str = "ok";
pub const HEALTH_CORRUPT: &str = "corrupt";
pub const HEALTH_WRITE_FAILED: &str = "writeFailed";

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
  /// AI 向量索引（kb_index.json，Phase 3）：衍生数据，可全量重建
  pub kb_index: Vec<crate::ai::embedding::ChunkIndex>,
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
      kb_index: Vec::new(),
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
          Ok(value) => store.from_v1(&value, true),
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
      return store.from_v1(&raw, false);
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
        // 知识库：缺失/损坏 → 静默空库（KB 可整体降级，绝不传染用户数据）
        store.kb = store
          .backend
          .read_notes()
          .ok()
          .flatten()
          .and_then(|text| parse_value(&text).ok())
          .and_then(|value| serde_json::from_value::<Vec<KbItem>>(value).ok())
          .unwrap_or_default();
        // AI 向量索引（kb_index.json，衍生数据）：缺失/损坏 → 静默空（下次搜索触发重建）
        store.kb_index = store
          .backend
          .read_kb_index()
          .ok()
          .flatten()
          .and_then(|text| parse_value(&text).ok())
          .and_then(|value| serde_json::from_value::<Vec<crate::ai::embedding::ChunkIndex>>(value).ok())
          .unwrap_or_default();
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
  /// 只写知识库（notes.json，原子写；v1 不轮转，H2 成熟化）
  pub fn save_notes(&mut self) -> Result<(), String> {
    self.ensure_writable()?;
    let json = serde_json::to_string_pretty(&self.kb)
      .map_err(|error| format!("知识条目序列化失败：{}", brief(&error.to_string())))?;
    self.backend.write_notes(&json)
  }

  /// 只写 AI 向量索引（kb_index.json，衍生数据；损坏可重建，无需轮转）
  pub fn save_kb_index(&mut self) -> Result<(), String> {
    self.ensure_writable()?;
    let json = serde_json::to_string_pretty(&self.kb_index)
      .map_err(|error| format!("向量索引序列化失败：{}", brief(&error.to_string())))?;
    self.backend.write_kb_index(&json)
  }

  pub fn save_runtime_only(&mut self) -> Result<(), String> {
    let runtime_json = self.serialized_runtime()?;
    self.backend.write_runtime(&runtime_json)
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
      if let Some(dnd) = &patch.dnd {
        if let Some(value) = dnd.enabled {
          settings.dnd.enabled = value;
        }
        if let Some(value) = &dnd.from {
          if crate::models::parse_clock(value).is_none() {
            return Err("免打扰开始时间格式不对，请用 HH:MM".to_string());
          }
          settings.dnd.from = value.trim().to_string();
        }
        if let Some(value) = &dnd.to {
          if crate::models::parse_clock(value).is_none() {
            return Err("免打扰结束时间格式不对，请用 HH:MM".to_string());
          }
          settings.dnd.to = value.trim().to_string();
        }
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
}


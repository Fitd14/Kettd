//! 存储安全层（PRD 6.6）
//!
//! 根治 v1 三个病灶：
//! 1. `unwrap_or_default()` 静默清空 → 解析失败一律进 corrupt 通道，写命令全部拒绝
//! 2. 裸 `fs::write` 全量覆盖 → 轮转备份 + tmp + rename 原子替换
//! 3. 时间漂移 / snake_case → 本地语义归一 + camelCase 序列化 + 迁移前原件永久留档
//!
//! 数据目录沿用 v1 的 `%APPDATA%/todo-list`（兼容发现）。

use crate::models::{
  carry_days, clocks_of, fmt_day, fmt_dt, new_id, normalize_date_text, normalize_datetime,
  patch_datetime, patch_text, parse_wall, to_local, today, AppData, BackupInfo, CATEGORIES,
  DataHealthV2, MigrationIssue, MigrationReport, Note, PRIORITIES, Reminder, ReminderPayload,
  Repeat, Settings, SettingsPayload, Source, Subtask, Task, TaskPayload, FLOAT_FORMS,
  TRASH_RETENTION_DAYS,
};
use chrono::{DateTime, Duration, Local};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub const BACKUP_SLOTS: usize = 5;
pub const HEALTH_OK: &str = "ok";
pub const HEALTH_CORRUPT: &str = "corrupt";
pub const HEALTH_WRITE_FAILED: &str = "writeFailed";

const DATA_FILE: &str = "data.json";
const TMP_FILE: &str = "data.json.tmp";
const V1_ARCHIVE: &str = "data.v1.json";

// ---------------------------------------------------------------- 路径

/// v1 同款目录：Windows 下 `dirs::data_dir()` == `%APPDATA%`
pub fn data_dir() -> PathBuf {
  dirs::data_dir()
    .unwrap_or_else(|| PathBuf::from("."))
    .join("todo-list")
}

pub fn data_path(dir: &Path) -> PathBuf {
  dir.join(DATA_FILE)
}

fn backup_path(dir: &Path, slot: usize) -> PathBuf {
  dir.join(format!("{}.bak-{}", DATA_FILE, slot))
}

fn stamp_text(value: &DateTime<Local>) -> String {
  value.format("%Y-%m-%dT%H-%M-%S").to_string()
}

/// 把不可用文件另存为 data.corrupt.<ISO时间>（永不删除原件）
fn quarantine(dir: &Path, source: &Path) -> Option<String> {
  let name = format!("data.corrupt.{}", stamp_text(&Local::now()));
  let target = dir.join(&name);
  if fs::rename(source, &target).is_ok() {
    return Some(name);
  }
  if fs::copy(source, &target).is_ok() {
    let _ = fs::remove_file(source);
    return Some(name);
  }
  None
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
  if let Some(value) = raw.get("floatForm").and_then(|item| item.as_str()) {
    if FLOAT_FORMS.contains(&value) {
      settings.float_form = value.to_string();
    }
  }
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
  if let Some(value) = raw.get("fired") {
    if let Some(items) = value.as_array() {
      data.fired = items
        .iter()
        .filter_map(|item| item.as_str().map(|text| text.to_string()))
        .collect();
    }
  }
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
  report.archived_to = V1_ARCHIVE.to_string();
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

// ---------------------------------------------------------------- Store

/// 撤销栈条目
#[derive(Debug, Clone)]
pub enum Undoable {
  Task(Box<Task>),
  Subtask { task_id: String, subtask: Subtask },
  Note { task_id: String, note: Note },
}

pub struct Store {
  pub dir: PathBuf,
  pub data: AppData,
  pub health: String,
  pub corrupt_file: Option<String>,
  pub last_error: Option<String>,
  pub undo: Vec<Undoable>,
}

impl Store {
  fn new(dir: &Path) -> Self {
    Self {
      dir: dir.to_path_buf(),
      data: AppData::default(),
      health: HEALTH_OK.to_string(),
      corrupt_file: None,
      last_error: None,
      undo: Vec::new(),
    }
  }

  /// 启动加载：任何失败都进 corrupt 通道，绝不返回空集合
  pub fn load() -> Store {
    let dir = data_dir();
    let mut store = Store::new(&dir);
    if let Err(error) = fs::create_dir_all(&dir) {
      store.health = HEALTH_WRITE_FAILED.to_string();
      store.last_error = Some(file_error("无法创建数据文件夹", &error));
      return store;
    }
    let path = data_path(&dir);
    if !path.exists() {
      let archive = dir.join(V1_ARCHIVE);
      if archive.exists() {
        // 上次迁移中途失败：从永久留档的 v1 原件重来
        return match read_value(&archive) {
          Ok(raw) => store.from_v1(&raw, true),
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
    let raw = match read_value(&path) {
      Ok(value) => value,
      Err(error) => {
        store.corrupt_file = quarantine(&dir, &path);
        store.health = HEALTH_CORRUPT.to_string();
        store.last_error = Some(error);
        return store;
      }
    };
    if looks_like_v1(&raw) {
      return store.from_v1(&raw, false);
    }
    match serde_json::from_value::<AppData>(raw.clone()) {
      Ok(mut data) => {
        data.settings.coerce();
        for task in data.tasks.iter_mut() {
          if !task.done && !task.in_trash() {
            task.carried_from = task.carried_from.max(carry_days(&task.due_at.clone()));
          }
        }
        store.data = data;
        store
      }
      Err(error) => {
        store.corrupt_file = quarantine(&dir, &path);
        store.health = HEALTH_CORRUPT.to_string();
        store.last_error = Some(format!(
          "数据文件格式不对，已停止写入：{}",
          brief(&error.to_string())
        ));
        store
      }
    }
  }

  fn from_v1(mut self, raw: &Value, from_archive: bool) -> Store {
    let path = data_path(&self.dir);
    let archive = self.dir.join(V1_ARCHIVE);
    if !from_archive && !archive.exists() {
      // 迁移前原件永久留档（已存在则保留最早那份，绝不覆盖）
      if let Err(error) = fs::copy(&path, &archive) {
        self.health = HEALTH_CORRUPT.to_string();
        self.corrupt_file = quarantine(&self.dir, &path);
        self.last_error = Some(file_error("无法留存 v1 原件", &error));
        return self;
      }
    }
    match migrate_v1(raw) {
      Ok((mut data, report)) => {
        data.settings.coerce();
        data.migration = Some(report);
        self.data = data;
        if let Err(error) = self.save() {
          self.last_error = Some(error);
          self.health = HEALTH_WRITE_FAILED.to_string();
        }
        self
      }
      Err(error) => {
        if !from_archive {
          self.corrupt_file = quarantine(&self.dir, &path);
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
    let mut out: Vec<BackupInfo> = Vec::new();
    for slot in 1..=BACKUP_SLOTS {
      let path = backup_path(&self.dir, slot);
      if !path.exists() {
        continue;
      }
      let meta = fs::metadata(&path).ok();
      let created_at = meta
        .as_ref()
        .and_then(|value| value.modified().ok())
        .map(|time| {
          let local: DateTime<Local> = time.into();
          fmt_dt(local)
        })
        .unwrap_or_else(|| "未知时间".to_string());
      let size_kb = meta.map(|value| (value.len() + 1023) / 1024).unwrap_or(0);
      match read_value(&path) {
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
    let dir = self.dir.clone();
    let json = serde_json::to_string_pretty(&self.data)
      .map_err(|error| format!("数据序列化失败，本次没有保存：{}", brief(&error.to_string())))?;
    if let Err(error) = rotate_backups(&dir) {
      return Err(file_error("备份轮转失败，本次没有保存", &error));
    }
    write_json(&dir, TMP_FILE, DATA_FILE, &json)
  }

  /// 只写不轮转（回滚备份时使用，避免挤掉刚选中的备份）
  fn write_in_place(&mut self) -> Result<(), String> {
    let json = serde_json::to_string_pretty(&self.data)
      .map_err(|error| format!("数据序列化失败：{}", brief(&error.to_string())))?;
    let dir = self.dir.clone();
    write_json(&dir, TMP_FILE, DATA_FILE, &json)?;
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
    let path = backup_path(&self.dir, number);
    if !path.exists() {
      return Err("这个备份不存在，换一个试试".to_string());
    }
    let raw = read_value(&path).map_err(|_| "这个备份也读不了，换一个试试".to_string())?;
    let mut restored = if looks_like_v1(&raw) {
      let (mut data, report) = migrate_v1(&raw).map_err(|_| "这个备份无法还原成可用数据".to_string())?;
      data.migration = Some(report);
      data
    } else {
      serde_json::from_value::<AppData>(raw).map_err(|_| "这个备份无法还原成可用数据".to_string())?
    };
    restored.settings.coerce();
    if restored.migration.is_none() {
      restored.migration = self.data.migration.clone();
    }
    self.data = restored;
    self.undo.clear();
    // 损坏原件早已另存为 data.corrupt.*，这里覆盖写但不轮转
    self.write_in_place()?;
    Ok(self.tasks(true).into_iter().filter(|task| !task.in_trash()).count())
  }

  /// 前端读过迁移报告后清掉，避免每次启动重复弹提示
  pub fn clear_migration(&mut self) {
    self.data.migration = None;
  }

  /// 启动时物理清理超过保留期的软删任务（含其子任务与备注）
  pub fn purge_expired(&mut self, days: i64) -> usize {
    let limit = now() - Duration::days(days);
    let before = self.data.tasks.len();
    self.data.tasks.retain(|task| match &task.deleted_at {
      None => true,
      Some(stamp) => match parse_wall(stamp).and_then(|wall| to_local(&wall)) {
        Some(when) => when > limit,
        None => true,
      },
    });
    let dropped = before - self.data.tasks.len();
    if dropped > 0 {
      let alive: Vec<String> = self.data.tasks.iter().map(|task| task.id.clone()).collect();
      self.data.fired.retain(|key| match key.split('|').next() {
        Some(id) => alive.iter().any(|value| value == id),
        None => false,
      });
    }
    dropped
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
    let stamp = now_text();
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
        let stamp = now_text();
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
        self.data.tasks[index].updated_at = now_text();
        Ok(Some(self.data.tasks[index].clone()))
      }
      Undoable::Note { task_id, note } => {
        let index = self
          .find_index(&task_id)
          .ok_or_else(|| "原任务已经不在了，备注无法放回".to_string())?;
        if !self.data.tasks[index].notes.iter().any(|item| item.id == note.id) {
          self.data.tasks[index].notes.push(note);
        }
        self.data.tasks[index].updated_at = now_text();
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
    self.data.tasks[index].updated_at = now_text();
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
    let prefix = format!("{}|", removed.id);
    self.data.fired.retain(|key| !key.starts_with(&prefix));
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
    let stamp = now_text();
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
    self.data.tasks[index].updated_at = now_text();
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
      if let Some(value) = &patch.float_form {
        if !FLOAT_FORMS.contains(&value.as_str()) {
          return Err("悬浮形态只能是 topmost / desktop / mini".to_string());
        }
        settings.float_form = value.clone();
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
      self.data.tasks[index].updated_at = now_text();
    }
    self.remember_fired(key);
  }

  pub fn remember_fired(&mut self, key: &str) {
    if !self.data.fired.iter().any(|value| value == key) {
      self.data.fired.push(key.to_string());
    }
    if self.data.fired.len() > 400 {
      let excess = self.data.fired.len() - 400;
      self.data.fired.drain(0..excess);
    }
  }

  pub fn has_fired(&self, key: &str) -> bool {
    self.data.fired.iter().any(|value| value == key)
  }

  /// 触发/落库后统一走这里：写盘 + 事件由各命令层负责
  pub fn touch_task_time(&mut self, id: &str) {
    if let Some(index) = self.find_index(id) {
      let stamp = now_text();
      self.data.tasks[index].updated_at = stamp;
    }
  }

  pub fn retention_days(&self) -> i64 {
    TRASH_RETENTION_DAYS
  }
}

// ---------------------------------------------------------------- 工具

pub fn now() -> DateTime<Local> {
  Local::now()
}

fn now_text() -> String {
  fmt_dt(Local::now())
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

fn io_kind_text(error: &std::io::Error) -> &'static str {
  match error.kind() {
    std::io::ErrorKind::PermissionDenied => "没有写入权限",
    std::io::ErrorKind::NotFound => "找不到文件",
    std::io::ErrorKind::ReadOnlyFilesystem => "目录是只读的",
    _ => "磁盘或路径不可用",
  }
}

/// 面向 UI 的中文短句（不含路径与堆栈）
fn file_error(prefix: &str, error: &std::io::Error) -> String {
  format!("{}：{}", prefix, io_kind_text(error))
}

fn write_error(detail: Option<&str>) -> String {
  match detail {
    Some(text) if !text.trim().is_empty() => text.to_string(),
    _ => "本地数据保存失败，请检查磁盘空间或目录权限".to_string(),
  }
}

fn write_json(dir: &Path, tmp_name: &str, final_name: &str, json: &str) -> Result<(), String> {
  let tmp = dir.join(tmp_name);
  let target = dir.join(final_name);
  if let Err(error) = fs::write(&tmp, json.as_bytes()) {
    let _ = fs::remove_file(&tmp);
    return Err(file_error("临时文件写入失败，本次没有保存", &error));
  }
  if let Err(error) = fs::rename(&tmp, &target) {
    let _ = fs::remove_file(&tmp);
    return Err(file_error("替换数据文件失败，本次没有保存", &error));
  }
  Ok(())
}

/// data.json.bak-1 ← 当前 data.json，旧档顺移，保留 5 份
fn rotate_backups(dir: &Path) -> Result<(), std::io::Error> {
  let mut slot = BACKUP_SLOTS;
  while slot > 1 {
    let from = backup_path(dir, slot - 1);
    let to = backup_path(dir, slot);
    if from.exists() {
      if to.exists() {
        let _ = fs::remove_file(&to);
      }
      fs::rename(&from, &to)?;
    }
    slot -= 1;
  }
  let current = data_path(dir);
  if current.exists() {
    let first = backup_path(dir, 1);
    if first.exists() {
      let _ = fs::remove_file(&first);
    }
    fs::copy(&current, &first)?;
  }
  Ok(())
}

pub fn read_value(path: &Path) -> Result<Value, String> {
  let raw = fs::read_to_string(path).map_err(|error| file_error("读不到数据文件", &error))?;
  serde_json::from_str::<Value>(&raw).map_err(|error| {
    let text = error.to_string();
    if text.contains("EOF") || text.contains("expected") {
      "数据文件被截断或格式不对，已停止写入".to_string()
    } else {
      "数据文件读不了，已停止写入".to_string()
    }
  })
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


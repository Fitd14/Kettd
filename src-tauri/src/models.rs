//! v2 领域模型（camelCase 序列化 / 本地时区语义）
//!
//! 时间约定：所有用户可见时间字段都是**本地语义**字符串
//! - 完整时刻：`YYYY-MM-DDTHH:mm`
//! - 仅日期：`YYYY-MM-DD`
//! - 提醒的循环时刻：`HH:MM`，或多时刻 `HH:MM/HH:MM`（兼容 v1 数据）
//! 全链路禁止 UTC 换算（对治 v1 audit-6 时间漂移）。

use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone};
use serde::{Deserialize, Serialize};

pub const CATEGORIES: [&str; 3] = ["工作", "学习", "生活"];
pub const PRIORITIES: [&str; 3] = ["high", "med", "low"];
pub const FLOAT_FORMS: [&str; 3] = ["topmost", "desktop", "mini"];
pub const DEFAULT_HOTKEY: &str = "Alt+Shift+A";
pub const DEFAULT_DND_FROM: &str = "23:00";
pub const DEFAULT_DND_TO: &str = "07:30";
pub const REMINDER_CAP_DEFAULT: u32 = 3;
/// 本地时刻（全链路不用 UTC）
pub type DateTimeLocal = chrono::DateTime<Local>;

/// 软删除保留天数（超期在启动时物理清理）
pub const TRASH_RETENTION_DAYS: i64 = 30;
/// 错过多久不再补发通知
pub const MISSED_GRACE_HOURS: i64 = 24;

// ---------------------------------------------------------------- 通用辅助

fn is_absent<T>(value: &Option<T>) -> bool {
  value.is_none()
}

fn is_false(value: &bool) -> bool {
  !*value
}

fn bool_true() -> bool {
  true
}

fn default_category() -> String {
  "生活".to_string()
}

fn default_priority() -> String {
  "med".to_string()
}

// ---------------------------------------------------------------- 基础类型

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subtask {
  pub id: String,
  pub title: String,
  #[serde(default)]
  pub done: bool,
}

impl Default for Subtask {
  fn default() -> Self {
    Self {
      id: String::new(),
      title: String::new(),
      done: false,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
  pub id: String,
  #[serde(default)]
  pub author: String,
  #[serde(default)]
  pub content: String,
  #[serde(default)]
  pub created_at: String,
}

impl Default for Note {
  fn default() -> Self {
    Self {
      id: String::new(),
      author: String::new(),
      content: String::new(),
      created_at: String::new(),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
  Capture,
  Manual,
  Seed,
}

impl Default for Source {
  fn default() -> Self {
    Source::Manual
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
  None,
  Daily,
  Workdays,
  Weekly,
}

impl Default for Repeat {
  fn default() -> Self {
    Repeat::None
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Task {
  pub id: String,
  pub title: String,
  /// 任务备注正文（v1 的 description）
  pub note: String,
  /// 工作 | 学习 | 生活
  #[serde(default = "default_category")]
  pub category: String,
  /// high | med | low
  #[serde(default = "default_priority")]
  pub priority: String,
  #[serde(skip_serializing_if = "is_absent")]
  pub due_at: Option<String>,
  #[serde(skip_serializing_if = "is_absent")]
  pub remind_at: Option<String>,
  /// 加入今天的日期（YYYY-MM-DD），null = 未计划
  #[serde(skip_serializing_if = "is_absent")]
  pub planned_date: Option<String>,
  /// 逾期粘留天数
  pub carried_from: u32,
  pub done: bool,
  #[serde(skip_serializing_if = "is_absent")]
  pub done_at: Option<String>,
  /// 软删除时刻；非空 = 在回收站
  #[serde(skip_serializing_if = "is_absent")]
  pub deleted_at: Option<String>,
  /// v1 迁移而来且缺少真实完成时刻
  #[serde(default, skip_serializing_if = "is_false")]
  pub legacy: bool,
  pub subtasks: Vec<Subtask>,
  pub notes: Vec<Note>,
  pub source: Source,
  pub created_at: String,
  pub updated_at: String,
}

impl Default for Task {
  fn default() -> Self {
    Self {
      id: String::new(),
      title: String::new(),
      note: String::new(),
      category: default_category(),
      priority: default_priority(),
      due_at: None,
      remind_at: None,
      planned_date: None,
      carried_from: 0,
      done: false,
      done_at: None,
      deleted_at: None,
      legacy: false,
      subtasks: Vec::new(),
      notes: Vec::new(),
      source: Source::Manual,
      created_at: String::new(),
      updated_at: String::new(),
    }
  }
}

impl Task {
  pub fn blank(title: &str) -> Self {
    let stamp = now_text();
    Self {
      id: new_id("t"),
      title: title.to_string(),
      created_at: stamp.clone(),
      updated_at: stamp,
      ..Default::default()
    }
  }

  pub fn in_trash(&self) -> bool {
    self.deleted_at.is_some()
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Reminder {
  pub id: String,
  pub title: String,
  /// `HH:MM` / `HH:MM/HH:MM`（循环）或 `YYYY-MM-DDTHH:mm`（单次）
  pub time: String,
  #[serde(default = "default_category")]
  pub category: String,
  #[serde(default = "bool_true")]
  pub enabled: bool,
  #[serde(default, skip_serializing_if = "is_false")]
  pub completed: bool,
  pub repeat: Repeat,
  #[serde(skip_serializing_if = "is_absent")]
  pub last_fired: Option<String>,
  #[serde(skip_serializing_if = "is_absent")]
  pub snoozed_until: Option<String>,
  #[serde(default, skip_serializing_if = "is_false")]
  pub legacy: bool,
}

impl Default for Reminder {
  fn default() -> Self {
    Self {
      id: String::new(),
      title: String::new(),
      time: String::new(),
      category: default_category(),
      enabled: true,
      completed: false,
      repeat: Repeat::None,
      last_fired: None,
      snoozed_until: None,
      legacy: false,
    }
  }
}

impl Reminder {
  pub fn blank(title: &str, time: &str) -> Self {
    Self {
      id: new_id("r"),
      title: title.to_string(),
      time: time.to_string(),
      ..Default::default()
    }
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Dnd {
  #[serde(default = "bool_true")]
  pub enabled: bool,
  pub from: String,
  pub to: String,
}

impl Default for Dnd {
  fn default() -> Self {
    Self {
      enabled: true,
      from: DEFAULT_DND_FROM.to_string(),
      to: DEFAULT_DND_TO.to_string(),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
  /// 主题（float / dark / light …由前端决定，后端只存不解释）
  pub theme: String,
  /// topmost | desktop | mini
  pub float_form: String,
  pub capture_hotkey: String,
  pub dnd: Dnd,
  pub remind_cap_per_hour: u32,
  pub onboarded: bool,
  #[serde(skip_serializing_if = "is_absent")]
  pub export_dir: Option<String>,
}

impl Default for Settings {
  fn default() -> Self {
    Self {
      theme: "float".to_string(),
      float_form: "topmost".to_string(),
      capture_hotkey: DEFAULT_HOTKEY.to_string(),
      dnd: Dnd::default(),
      remind_cap_per_hour: REMINDER_CAP_DEFAULT,
      onboarded: false,
      export_dir: None,
    }
  }
}

impl Settings {
  /// 兜底纠正非法值，保证调度器 / 窗口层拿到的永远是可用配置
  pub fn coerce(&mut self) {
    if self.theme.trim().is_empty() {
      self.theme = "float".to_string();
    }
    if !FLOAT_FORMS.contains(&self.float_form.as_str()) {
      self.float_form = "topmost".to_string();
    }
    if self.capture_hotkey.trim().is_empty() {
      self.capture_hotkey = DEFAULT_HOTKEY.to_string();
    }
    if self.remind_cap_per_hour == 0 || self.remind_cap_per_hour > 60 {
      self.remind_cap_per_hour = REMINDER_CAP_DEFAULT;
    }
    if parse_clock(&self.dnd.from).is_none() {
      self.dnd.from = DEFAULT_DND_FROM.to_string();
    }
    if parse_clock(&self.dnd.to).is_none() {
      self.dnd.to = DEFAULT_DND_TO.to_string();
    }
  }
}

/// 落盘的数据文件（data.json 的根结构）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppData {
  pub tasks: Vec<Task>,
  pub reminders: Vec<Reminder>,
  pub settings: Settings,
  /// 已处理提醒的稳定键（重启后不重复补发）
  pub fired: Vec<String>,
  /// 首次加载执行的 v1→v2 迁移报告，前端读取后可清除
  #[serde(skip_serializing_if = "is_absent")]
  pub migration: Option<MigrationReport>,
}

impl Default for AppData {
  fn default() -> Self {
    Self {
      tasks: Vec::new(),
      reminders: Vec::new(),
      settings: Settings::default(),
      fired: Vec::new(),
      migration: None,
    }
  }
}

// ---------------------------------------------------------------- 命令入参（patch）

/// `add_task` / `update_task` 的载荷：缺省字段沿用原值（update）或默认值（add）
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TaskPayload {
  pub title: Option<String>,
  pub note: Option<String>,
  pub category: Option<String>,
  pub priority: Option<String>,
  pub due_at: Option<serde_json::Value>,
  pub remind_at: Option<serde_json::Value>,
  pub planned_date: Option<serde_json::Value>,
  pub carried_from: Option<u32>,
  pub done: Option<bool>,
  pub done_at: Option<serde_json::Value>,
  pub deleted_at: Option<serde_json::Value>,
  pub legacy: Option<bool>,
  pub subtasks: Option<Vec<Subtask>>,
  pub notes: Option<Vec<Note>>,
  pub source: Option<Source>,
  pub created_at: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SubtaskPayload {
  pub title: Option<String>,
  pub done: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NotePayload {
  pub author: Option<String>,
  pub content: Option<String>,
  pub created_at: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ReminderPayload {
  pub title: Option<String>,
  pub time: Option<String>,
  pub category: Option<String>,
  pub enabled: Option<bool>,
  pub completed: Option<bool>,
  pub repeat: Option<Repeat>,
  pub snoozed_until: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DndPayload {
  pub enabled: Option<bool>,
  pub from: Option<String>,
  pub to: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsPayload {
  pub theme: Option<String>,
  pub float_form: Option<String>,
  pub capture_hotkey: Option<String>,
  pub dnd: Option<DndPayload>,
  pub remind_cap_per_hour: Option<u32>,
  pub onboarded: Option<bool>,
  pub export_dir: Option<String>,
}

/// patch 字符串字段：null / 空串 → None（清空）
pub fn patch_text(value: &serde_json::Value) -> Option<String> {
  match value {
    serde_json::Value::Null => None,
    serde_json::Value::String(text) => {
      if text.trim().is_empty() || is_nullish(text) {
        None
      } else {
        Some(text.trim().to_string())
      }
    }
    other => Some(other.to_string()),
  }
}

// ---------------------------------------------------------------- 健康度 / 备份 / 迁移

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
  /// "1".."5"
  pub slot: String,
  /// data.json.bak-1 .. data.json.bak-5
  pub name: String,
  pub created_at: String,
  pub size_kb: u64,
  pub task_count: u32,
  pub readable: bool,
  #[serde(skip_serializing_if = "is_absent")]
  pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationIssue {
  pub collection: String,
  pub id: String,
  pub field: String,
  pub raw: String,
  pub action: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationReport {
  pub task_count: u32,
  pub reminder_count: u32,
  /// 非法时间被置空的条数
  pub invalid_dates: u32,
  /// completed 但无完成时刻的条数
  pub legacy_done: u32,
  pub archived_to: String,
  pub issues: Vec<MigrationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataHealthV2 {
  /// ok | corrupt | writeFailed
  pub health: String,
  pub backups: Vec<BackupInfo>,
  #[serde(skip_serializing_if = "is_absent")]
  pub corrupt_file: Option<String>,
  #[serde(skip_serializing_if = "is_absent")]
  pub last_error: Option<String>,
  pub writable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResult {
  /// 回滚后未删除的任务数
  pub restored: usize,
  pub health: DataHealthV2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
  pub tasks: Vec<Task>,
  pub reminders: Vec<Reminder>,
  pub settings: Settings,
  pub health: DataHealthV2,
  pub migration: Option<MigrationReport>,
}

/// `store-changed` 事件负载
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreEvent {
  pub id: String,
  /// changed | fired | missed
  pub kind: String,
}

impl StoreEvent {
  pub fn changed(id: &str) -> Self {
    Self {
      id: id.to_string(),
      kind: "changed".to_string(),
    }
  }

  pub fn fired(id: &str) -> Self {
    Self {
      id: id.to_string(),
      kind: "fired".to_string(),
    }
  }

  pub fn missed(id: &str) -> Self {
    Self {
      id: id.to_string(),
      kind: "missed".to_string(),
    }
  }
}

// ---------------------------------------------------------------- 时间工具

/// 稳定键前缀：ID 单调（毫秒时间戳 + 进程号 + 进程内计数）
pub fn new_id(prefix: &str) -> String {
  use std::sync::atomic::{AtomicU32, Ordering};
  static SEQ: AtomicU32 = AtomicU32::new(0);
  let ms = Local::now().timestamp_millis();
  let n = SEQ.fetch_add(1, Ordering::Relaxed);
  format!("{}{}{:04}", prefix, ms, n)
}

pub fn now() -> chrono::DateTime<Local> {
  Local::now()
}

pub fn today() -> NaiveDate {
  Local::now().date_naive()
}

/// 用户可见完整时刻文本（本地语义）
pub fn fmt_dt(value: chrono::DateTime<Local>) -> String {
  value.format("%Y-%m-%dT%H:%M").to_string()
}

pub fn fmt_wall(value: &NaiveDateTime) -> String {
  value.format("%Y-%m-%dT%H:%M").to_string()
}

pub fn fmt_day(value: &NaiveDate) -> String {
  value.format("%Y-%m-%d").to_string()
}

pub fn now_text() -> String {
  fmt_dt(now())
}

fn is_nullish(text: &str) -> bool {
  let lower = text.trim().to_lowercase();
  lower == "null" || lower == "none" || lower == "undefined" || lower == "nan"
}

/// 枚举字段校验（分类 / 优先级 / 悬浮形态 / 重复规则）
pub fn is_valid_choice(allowed: &[&str], value: &str) -> bool {
  allowed.contains(&value.trim())
}

/// `HH:MM` → NaiveTime
pub fn parse_clock(raw: &str) -> Option<NaiveTime> {
  let text = raw.trim();
  if let Ok(time) = NaiveTime::parse_from_str(text, "%H:%M") {
    return Some(time);
  }
  if let Ok(full) = NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S") {
    return Some(full.time());
  }
  if let Ok(full) = NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M") {
    return Some(full.time());
  }
  None
}

/// 提醒 `time` 可承载多个循环时刻（v1 遗留："10:00/14:00/16:00"）
pub fn clocks_of(raw: &str) -> Vec<NaiveTime> {
  let mut out: Vec<NaiveTime> = Vec::new();
  for part in raw.split('/') {
    if let Some(time) = parse_clock(part) {
      if !out.contains(&time) {
        out.push(time);
      }
    }
  }
  out
}

/// 归一化用户可见时间为本地语义字符串；无法解析返回 None。
/// 接受 `YYYY-MM-DD`、`YYYY-MM-DD HH:MM[:SS]`、`YYYY-MM-DDTHH:mm[:SS][Z|±hh:mm]`、`HH:MM`。
/// 带时区后缀时**只做截断**（绝不换算），避免把 UTC 漂移写进用户可见值。
pub fn normalize_datetime(raw: &str) -> Option<String> {
  let text = raw.trim();
  if text.is_empty() || is_nullish(text) {
    return None;
  }
  let chars: Vec<char> = text.chars().collect();
  if chars.len() == 5 && chars[2] == ':' {
    if parse_clock(text).is_some() {
      return Some(text.to_string());
    }
    return None;
  }
  if chars.len() == 10 {
    if NaiveDate::parse_from_str(text, "%Y-%m-%d").is_ok() {
      return Some(text.to_string());
    }
    return None;
  }
  if chars.len() < 16 {
    return None;
  }
  // 空格制式 → T 制式（v1 的 "2026-08-29 18:00"）
  let mut head: String = chars.iter().collect();
  if chars[10] == ' ' {
    let date_part: String = chars.iter().take(10).collect();
    let time_part: String = chars.iter().skip(11).collect();
    head = format!("{}T{}", date_part, time_part);
  }
  let prefix16: String = head.chars().take(16).collect();
  if NaiveDateTime::parse_from_str(&prefix16, "%Y-%m-%dT%H:%M").is_ok() {
    return Some(prefix16);
  }
  if NaiveDateTime::parse_from_str(&head, "%Y-%m-%dT%H:%M:%S").is_ok() {
    return Some(head.chars().take(16).collect::<String>());
  }
  None
}

/// patch 时间字段：null / 空 / 非法 → None
pub fn patch_datetime(value: &serde_json::Value) -> Option<String> {
  match value {
    serde_json::Value::Null => None,
    serde_json::Value::String(text) => normalize_datetime(text),
    other => normalize_datetime(&other.to_string()),
  }
}

/// 仅日期归一化（plannedDate / 日期比较用）
pub fn normalize_date_text(raw: &str) -> Option<String> {
  let normalized = normalize_datetime(raw)?;
  let chars: Vec<char> = normalized.chars().collect();
  if chars.len() < 10 {
    return None;
  }
  Some(chars.iter().take(10).collect::<String>())
}

/// 用户可见字符串 → 本地墙上时刻（date-only 视为 00:00，HH:MM 视为今天）
pub fn parse_wall(value: &str) -> Option<NaiveDateTime> {
  let text = value.trim();
  let chars: Vec<char> = text.chars().collect();
  if chars.len() == 10 {
    let date = NaiveDate::parse_from_str(text, "%Y-%m-%d").ok()?;
    return date.and_hms_opt(0, 0, 0);
  }
  if chars.len() == 5 {
    let clock = parse_clock(text)?;
    return Some(today().and_time(clock));
  }
  let head: String = chars.iter().take(16).collect();
  if let Ok(parsed) = NaiveDateTime::parse_from_str(&head, "%Y-%m-%dT%H:%M") {
    return Some(parsed);
  }
  if let Ok(parsed) = NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S") {
    return Some(parsed);
  }
  None
}

/// 本地墙上时间 → 可比较时刻（DST 边界取早值，不做换算）
pub fn to_local(wall: &NaiveDateTime) -> Option<chrono::DateTime<Local>> {
  Local.from_local_datetime(wall).earliest()
}

/// 逾期粘留天数：dueAt 到今天（date-only 与完整时刻同日不算拖）
pub fn carry_days(due_at: &Option<String>) -> u32 {
  let raw = match due_at {
    Some(value) => value,
    None => return 0,
  };
  let wall = match parse_wall(raw) {
    Some(value) => value,
    None => return 0,
  };
  let due_date = wall.date();
  let now_date = today();
  if due_date >= now_date {
    return 0;
  }
  let days = (now_date - due_date).num_days();
  if days > 9999 {
    return 9999;
  }
  days as u32
}

/// 多时刻提醒的星期判定（Workdays=周一到周五；Weekly=与上次触发同星期）
pub fn repeat_allows(reminder: &Reminder, wall: &NaiveDateTime) -> bool {
  match reminder.repeat {
    Repeat::None | Repeat::Daily => true,
    Repeat::Workdays => wall.weekday().number_from_monday() <= 5,
    Repeat::Weekly => match &reminder.last_fired {
      None => true,
      Some(text) => match parse_wall(text) {
        Some(previous) => previous.weekday() == wall.weekday(),
        None => true,
      },
    },
  }
}

/// 提醒是否为循环（可跨天复用）
pub fn is_recurring(reminder: &Reminder) -> bool {
  !clocks_of(&reminder.time).is_empty() || reminder.repeat != Repeat::None
}

// ---------------------------------------------------------------- ISO 周

pub fn iso_week_label(date: &NaiveDate) -> String {
  format!("{}-W{:02}", date.iso_week().year(), date.iso_week().week())
}

/// `YYYY-Www` / `YYYYWww` → 该 ISO 周的周一
pub fn week_start(week: &str) -> Option<NaiveDate> {
  let text = week.trim().to_uppercase().replace('-', "");
  let chars: Vec<char> = text.chars().collect();
  if chars.len() < 6 || !chars[0].is_ascii_digit() {
    return None;
  }
  let year: i32 = chars.iter().take(4).collect::<String>().parse().ok()?;
  let rest: String = chars.iter().skip(4).collect();
  let number: u32 = match rest.strip_prefix('W') {
    Some(value) => value.parse().ok()?,
    None => rest.parse().ok()?,
  };
  if number == 0 || number > 53 {
    return None;
  }
  // ISO 8601：包含本年第一个周四的那周为第 1 周
  let jan4 = NaiveDate::from_ymd_opt(year, 1, 4)?;
  let back = jan4.weekday().num_days_from_monday() as i64;
  let week1_monday = jan4.checked_sub_signed(chrono::Duration::days(back))?;
  week1_monday.checked_add_signed(chrono::Duration::weeks(number as i64 - 1))
}

pub fn week_range(monday: &NaiveDate) -> Option<(NaiveDate, NaiveDate)> {
  let sunday = monday.checked_add_signed(chrono::Duration::days(6))?;
  Some((*monday, sunday))
}

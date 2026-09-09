//! v2 领域模型（camelCase 序列化 / 本地时区语义）
//!
//! 时间约定：所有用户可见时间字段都是**本地语义**字符串
//! - 完整时刻：`YYYY-MM-DDTHH:mm`
//! - 仅日期：`YYYY-MM-DD`
//! - 提醒的循环时刻：`HH:MM`，或多时刻 `HH:MM/HH:MM`（兼容 v1 数据）
//! 全链路禁止 UTC 换算（对治 v1 audit-6 时间漂移）。

use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, Timelike, TimeZone};
use serde::{Deserialize, Serialize};

pub const CATEGORIES: [&str; 3] = ["工作", "学习", "生活"];
pub const PRIORITIES: [&str; 3] = ["high", "med", "low"];
pub const DEFAULT_HOTKEY: &str = "Alt+Shift+A";
pub const DEFAULT_MAIN_HOTKEY: &str = "Alt+Shift+O";
pub const DEFAULT_STICKY_HOTKEY: &str = "Alt+Shift+S";
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
  /// 挂载的知识条目 id（**单向** task→KB，H2 验证后才有反向/活引用）
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub kb_refs: Vec<String>,
  /// 同列表内手动排序位（便签规格 §12.2）；None = 未手动排过，按 created_at 兜底
  #[serde(skip_serializing_if = "is_absent")]
  pub sort_order: Option<i64>,
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
      kb_refs: Vec::new(),
      sort_order: None,
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
  /// 便签纸色（便签规格 §12.1：warm 暖白纸 | kraft 牛皮纸 | cyan 淡青 | ink 暗墨，预设四选一非取色器）
  #[serde(default = "default_sticky_paper")]
  pub sticky_paper: String,
  /// 移出淡化开关（planned-settings-ui-spec B.1★：默认开，关则便签移出鼠标不淡化）
  #[serde(default = "bool_true")]
  pub sticky_fade: bool,
  /// 移出淡化后的不透明度（百分比 10-100，默认 38≈融入桌面仍可扫读；100 等于不淡）
  #[serde(default = "default_sticky_fade_opacity")]
  pub sticky_fade_opacity: u32,
  /// 纸面花纹（sticky-background-pattern.md：none 无 | bamboo 墨竹 | mountain 远山，选后两者时朱印伴随）
  #[serde(default = "default_sticky_pattern")]
  pub sticky_pattern: String,
  pub capture_hotkey: String,
  /// None = 未绑定全局快捷键
  pub main_hotkey: Option<String>,
  /// 便签全局热键（sticky-separation）：新建一张自由便签；None = 未绑定（可在设置解绑）
  #[serde(default = "default_sticky_hotkey")]
  pub sticky_hotkey: Option<String>,
  pub dnd: Dnd,
  pub remind_cap_per_hour: u32,
  pub onboarded: bool,
  #[serde(skip_serializing_if = "is_absent")]
  pub export_dir: Option<String>,
  /// 本地度量埋点开关（默认开；纯本机、绝不出网）
  #[serde(default = "telemetry_default")]
  pub telemetry_enabled: bool,
}

fn telemetry_default() -> bool {
  true
}

fn default_sticky_hotkey() -> Option<String> {
  Some(DEFAULT_STICKY_HOTKEY.to_string())
}

impl Default for Settings {
  fn default() -> Self {
    Self {
      theme: "float".to_string(),
      sticky_paper: "warm".to_string(),
      sticky_fade: true,
      sticky_fade_opacity: 38,
      sticky_pattern: "none".to_string(),
      capture_hotkey: DEFAULT_HOTKEY.to_string(),
      main_hotkey: Some(DEFAULT_MAIN_HOTKEY.to_string()),
      sticky_hotkey: Some(DEFAULT_STICKY_HOTKEY.to_string()),
      dnd: Dnd::default(),
      remind_cap_per_hour: REMINDER_CAP_DEFAULT,
      onboarded: false,
      export_dir: None,
      telemetry_enabled: true,
    }
  }
}

impl Settings {
  /// 兜底纠正非法值，保证调度器 / 窗口层拿到的永远是可用配置
  pub fn coerce(&mut self) {
    if self.theme.trim().is_empty() {
      self.theme = "float".to_string();
    }
    if self.capture_hotkey.trim().is_empty() {
      self.capture_hotkey = DEFAULT_HOTKEY.to_string();
    }
    if !STICKY_PAPERS.contains(&self.sticky_paper.as_str()) {
      self.sticky_paper = "warm".to_string();
    }
    if !STICKY_PATTERNS.contains(&self.sticky_pattern.as_str()) {
      self.sticky_pattern = "none".to_string();
    }
    if !(10..=100).contains(&self.sticky_fade_opacity) {
      self.sticky_fade_opacity = 38;
    }
    if let Some(value) = self.main_hotkey.as_deref() {
      if value.trim().is_empty() {
        self.main_hotkey = None;
      }
    }
    if let Some(value) = self.sticky_hotkey.as_deref() {
      if value.trim().is_empty() {
        self.sticky_hotkey = None;
      }
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

/// 便签（sticky-separation 定稿）：纯自由便签，≤500 字纯文本，多开（≤ STICKY_CAP）。
/// 三态生命周期：展开 ↔ 缩小（置顶悬浮文本条）↔ 关闭（销毁）；常驻置顶无开关。
/// 内容归 data.json（用户数据，ADR-0005）；位置归 runtime.json note_pos（ADR-0007）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StickyNote {
  pub id: String,
  /// 自由便签正文（纯文本 ≤500 字）。
  /// 空 content 也必须序列化：前端类型契约 content:string 必填，字段缺省会
  /// 让设置页清单 st.content.split 抛 TypeError、整树卸载黑屏（真机复盘 2026-09-09）
  pub content: String,
  /// 缩小态：置顶悬浮文本条（显示正文第一行，点击展开）
  #[serde(default, skip_serializing_if = "is_false")]
  pub mini: bool,
  pub created_at: String,
  pub updated_at: String,
}

impl Default for StickyNote {
  fn default() -> Self {
    let stamp = now_text();
    Self {
      id: String::new(),
      content: String::new(),
      mini: false,
      created_at: stamp.clone(),
      updated_at: stamp,
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
  /// 额外便签（sticky-separation）：纯自由便签，全部为动态 note:* 窗（float 已退役）
  #[serde(default)]
  pub stickies: Vec<StickyNote>,
}

impl Default for AppData {
  fn default() -> Self {
    Self {
      tasks: Vec::new(),
      reminders: Vec::new(),
      settings: Settings::default(),
      stickies: Vec::new(),
    }
  }
}

/// add/update_kb_item 载荷：缺省字段沿用原值（update）或默认值（add）
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct KbPayload {
  pub title: Option<String>,
  pub body_md: Option<String>,
  pub tags: Option<Vec<String>>,
}

pub const STICKY_PAPERS: [&str; 4] = ["warm", "kraft", "cyan", "ink"];
/// 纸面花纹（sticky-background-pattern.md 定稿：无 / 墨竹 / 远山；选竹或山时朱印自动伴随）
pub const STICKY_PATTERNS: [&str; 3] = ["none", "bamboo", "mountain"];
/// 便签总数上限（含 float 主便签）——防「贴满废纸」，对应 metric Counter 壁纸化
pub const STICKY_CAP: usize = 6;

fn default_sticky_fade_opacity() -> u32 {
  38
}

fn default_sticky_pattern() -> String {
  "none".to_string()
}

fn default_sticky_paper() -> String {
  "warm".to_string()
}

/// 知识条目（frame H1b 最小地基）：flomo 式片段，Markdown 正文，不做长文档编辑器。
/// 独立 notes.json；任务经 `Task.kbRefs` **单向**引用本表（ACL：KB 不反向持有 Task，
/// 架构 §3 —— 假设失败时 KB 可整体降级而不伤 Task 聚合）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct KbItem {
  pub id: String,
  pub title: String,
  pub body_md: String,
  pub tags: Vec<String>,
  pub created_at: String,
  pub updated_at: String,
}

impl Default for KbItem {
  fn default() -> Self {
    Self {
      id: String::new(),
      title: String::new(),
      body_md: String::new(),
      tags: Vec::new(),
      created_at: String::new(),
      updated_at: String::new(),
    }
  }
}

/// 运行态（ADR-0005）：住在 runtime.json，与用户数据物理隔离。
/// 丢了只是体验退化（个别提醒重响一次、窗口回默认位），绝不值得冻结用户数据，
/// 因此损坏时静默重建、**绝不**进 corrupt 通道；单文件覆盖写、不轮转。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RuntimeState {
  /// 已处理提醒的稳定键（t|id|stamp / r|id|stamp），上限 400，重启后不重复补发
  pub fired: Vec<String>,
  /// 快录条窗口位置
  pub capture_pos: Option<[i32; 2]>,
  /// 便签位置 map（ADR-0007 预留：noteId → [x,y]）
  pub note_pos: std::collections::HashMap<String, [i32; 2]>,
  /// v1→v2 迁移报告（前端读过即清）
  #[serde(skip_serializing_if = "is_absent")]
  pub migration: Option<MigrationReport>,
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
  pub kb_refs: Option<Vec<String>>,
  pub sticky_paper: Option<String>,
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
  pub sticky_paper: Option<String>,
  pub sticky_fade: Option<bool>,
  pub sticky_fade_opacity: Option<u32>,
  pub sticky_pattern: Option<String>,
  pub telemetry_enabled: Option<bool>,
  pub capture_hotkey: Option<String>,
  pub main_hotkey: Option<String>,
  pub sticky_hotkey: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationIssue {
  pub collection: String,
  pub id: String,
  pub field: String,
  pub raw: String,
  pub action: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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

/// 热键的**实际注册**快照（运行期事实，不落盘）：
/// 前端拿它与 `settings.captureHotkey / mainHotkey` 比对，不一致即「未生效」（qa-1）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyStatus {
  pub capture: Option<String>,
  pub main: Option<String>,
  pub sticky: Option<String>,
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
  /// 应用版本（设置页「版本 vN」展示；取自 Cargo.toml）
  pub version: String,
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

/// `HH:MM` → NaiveTime（**只保留时分**：调度器按整分匹配 tick，
/// 带秒的时刻永远命中不了，会表现为「到点不响」，故一律归零）
pub fn parse_clock(raw: &str) -> Option<NaiveTime> {
  let text = raw.trim();
  let parsed = NaiveTime::parse_from_str(text, "%H:%M")
    .or_else(|_| NaiveTime::parse_from_str(text, "%H:%M:%S"))
    .or_else(|_| NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S").map(|v| v.time()))
    .or_else(|_| NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M").map(|v| v.time()))
    .ok()?;
  NaiveTime::from_hms_opt(parsed.hour(), parsed.minute(), 0)
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
  carry_days_from(due_at, today())
}

/// 纯核心：「今天」由调用方注入（ADR-0002），规则可固定日期测试
pub fn carry_days_from(due_at: &Option<String>, now_date: NaiveDate) -> u32 {
  let raw = match due_at {
    Some(value) => value,
    None => return 0,
  };
  let wall = match parse_wall(raw) {
    Some(value) => value,
    None => return 0,
  };
  let due_date = wall.date();
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

// ---------------------------------------------------------------- 契约单测
// 全部为纯函数断言：不读写 %APPDATA%、不创建窗口，`cargo test` 可离线跑。
// 覆盖 V2-API §0 时间约定 与 §7 迁移规则的时间部分。

#[cfg(test)]
mod tests {
  use super::*;
  use serde_json::json;

  fn clock(h: u32, m: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(h, m, 0).unwrap()
  }

  #[test]
  fn parse_clock_accepts_clock_and_datetime_forms() {
    assert_eq!(parse_clock("09:00"), Some(clock(9, 0)));
    assert_eq!(parse_clock(" 23:59 "), Some(clock(23, 59)));
    // 带日期的串取其时刻部分
    assert_eq!(parse_clock("2026-08-29T18:00"), Some(clock(18, 0)));
    // 带秒必须归零：调度器按整分匹配，18:00:30 永远不会命中 tick
    assert_eq!(parse_clock("2026-08-29T18:00:30"), Some(clock(18, 0)));
    assert_eq!(parse_clock("18:00:30"), Some(clock(18, 0)));
    // chrono 的 %H 接受单位数小时，对 v1 脏数据应当宽松收下
    assert_eq!(parse_clock("9:00"), Some(clock(9, 0)));
  }

  #[test]
  fn parse_clock_rejects_garbage_and_out_of_range() {
    assert_eq!(parse_clock(""), None);
    assert_eq!(parse_clock("24:00"), None);
    assert_eq!(parse_clock("12:60"), None);
    assert_eq!(parse_clock("明天"), None);
    assert_eq!(parse_clock("null"), None);
  }

  #[test]
  fn clocks_of_keeps_multi_and_dedupes() {
    assert_eq!(clocks_of("10:00/14:00/16:00").len(), 3);
    assert_eq!(clocks_of("09:00"), vec![clock(9, 0)]);
    // 重复时刻去重，否则一次到点会响多遍
    assert_eq!(clocks_of("09:00/09:00"), vec![clock(9, 0)]);
    // 脏片段被跳过而不是整串作废
    assert_eq!(clocks_of("09:00/坏值/14:00"), vec![clock(9, 0), clock(14, 0)]);
    assert!(clocks_of("").is_empty());
  }

  #[test]
  fn normalize_datetime_converts_v1_space_form_to_T_form() {
    assert_eq!(
      normalize_datetime("2026-08-29 18:00").as_deref(),
      Some("2026-08-29T18:00")
    );
  }

  #[test]
  fn normalize_datetime_truncates_seconds_without_converting() {
    assert_eq!(
      normalize_datetime("2026-08-29T18:00:30").as_deref(),
      Some("2026-08-29T18:00")
    );
    assert_eq!(
      normalize_datetime("2026-08-29 18:00:30").as_deref(),
      Some("2026-08-29T18:00")
    );
  }

  /// 对治 v1 audit-6 时间漂移的硬约束：带时区后缀只做截断，绝不做换算
  #[test]
  fn normalize_datetime_truncates_tz_suffix_and_never_shifts() {
    assert_eq!(
      normalize_datetime("2026-08-29T18:00:00Z").as_deref(),
      Some("2026-08-29T18:00"),
      "若按 +08:00 换算会变成 26 日 02:00 —— 契约禁止"
    );
    assert_eq!(
      normalize_datetime("2026-08-29T18:00+08:00").as_deref(),
      Some("2026-08-29T18:00")
    );
    assert_eq!(
      normalize_datetime("2026-08-29T18:00:00-05:00").as_deref(),
      Some("2026-08-29T18:00")
    );
  }

  #[test]
  fn normalize_datetime_passes_through_date_only_and_clock() {
    assert_eq!(
      normalize_datetime("2026-08-29").as_deref(),
      Some("2026-08-29")
    );
    assert_eq!(normalize_datetime("09:00").as_deref(), Some("09:00"));
    assert_eq!(
      normalize_datetime("2026-08-29T18:00").as_deref(),
      Some("2026-08-29T18:00")
    );
  }

  #[test]
  fn normalize_datetime_nullish_and_invalid_become_none() {
    for raw in ["", "   ", "null", "None", "undefined", "NaN"] {
      assert_eq!(normalize_datetime(raw), None, "未置空：{:?}", raw);
    }
    assert_eq!(normalize_datetime("2026-13-45T10:00"), None);
    assert_eq!(normalize_datetime("2026-08-2"), None);
    assert_eq!(normalize_datetime("2026-08-29T9"), None);
  }

  #[test]
  fn normalize_date_text_needs_a_full_date() {
    assert_eq!(
      normalize_date_text("2026-08-29T18:00").as_deref(),
      Some("2026-08-29")
    );
    assert_eq!(
      normalize_date_text("2026-08-29").as_deref(),
      Some("2026-08-29")
    );
    assert_eq!(normalize_date_text("09:00"), None); // 仅时刻不构成日期
    assert_eq!(normalize_date_text("坏值"), None);
  }

  #[test]
  fn patch_datetime_handles_json_value_shapes() {
    assert_eq!(patch_datetime(&serde_json::Value::Null), None);
    assert_eq!(
      patch_datetime(&json!("2026-08-29 18:00")),
      Some("2026-08-29T18:00".to_string())
    );
    assert_eq!(patch_datetime(&json!(12345)), None); // 数字不是时间
    assert_eq!(patch_datetime(&json!("")), None);
  }

  #[test]
  fn parse_wall_treats_date_only_as_midnight_and_clock_as_today() {
    let date_only = parse_wall("2026-08-29").unwrap();
    assert_eq!(date_only.time(), NaiveTime::from_hms_opt(0, 0, 0).unwrap());
    assert_eq!(parse_wall("2026-08-29T18:00").unwrap().time(), clock(18, 0));
    assert_eq!(parse_wall("09:00").unwrap().date(), today()); // 仅时刻挂今天
    assert_eq!(parse_wall("坏值"), None);
  }

  #[test]
  fn carry_days_counts_overdue_and_zeroes_today_and_future() {
    assert_eq!(carry_days(&None), 0);
    assert_eq!(carry_days(&Some("坏值".to_string())), 0);
    let past = (today() - chrono::Duration::days(3))
      .format("%Y-%m-%d")
      .to_string();
    assert_eq!(carry_days(&Some(past)), 3);
    let future = (today() + chrono::Duration::days(3))
      .format("%Y-%m-%d")
      .to_string();
    assert_eq!(carry_days(&Some(future)), 0);
    assert_eq!(carry_days(&Some(fmt_day(&today()))), 0); // 今天不算逾期
  }

  #[test]
  fn week_start_and_range_cover_monday_to_sunday() {
    let monday = week_start("2026-W36").unwrap();
    assert_eq!(monday.weekday().to_string(), "Mon");
    let (start, end) = week_range(&monday).unwrap();
    assert_eq!(start, monday);
    assert_eq!((end - start).num_days(), 6, "周区间必须是周一到周日共 7 天");
    // 紧凑写法等价
    assert_eq!(week_start("2026W36"), Some(monday));
  }

  #[test]
  fn week_start_rejects_bad_numbers_and_shapes() {
    assert!(week_start("2026-W0").is_none());
    assert!(week_start("2026-W54").is_none());
    assert!(week_start("2026-Wabc").is_none());
    assert!(week_start("not-a-week").is_none());
  }
}

#[cfg(test)]
mod sticky_serialization_tests {
  use super::*;

  /// 真机黑屏根因（2026-09-09）：skip_serializing_if 曾把空 content 整个吞掉，
  /// 前端 StickyNoteItem.content 是必填 string，设置页清单 st.content.split 遇
  /// undefined 抛 TypeError → React 整树卸载 → 整窗黑屏。空串是合法态
  /// （清单本就有「（空）」占位），契约字段必须始终出现在 JSON 里。
  #[test]
  fn sticky_note_empty_content_still_serializes() {
    let note = StickyNote::default();
    let value = serde_json::to_value(&note).expect("StickyNote 必须可序列化");
    assert_eq!(
      value["content"], "",
      "空 content 也必须序列化：字段缺省 = 破坏前端类型契约（黑屏根因）"
    );
  }
}

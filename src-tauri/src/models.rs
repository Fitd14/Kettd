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
pub const DEFAULT_TODO_HOTKEY: &str = "Alt+Shift+T";
/// 直达知识库全局热键（kb-storage-backup S4）；允许清空（None = 不注册）
pub const DEFAULT_KB_HOTKEY: &str = "Alt+Shift+K";
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

fn is_true(value: &bool) -> bool {
  *value
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
  /// 待办悬浮窗置顶开关（todo-float）：窗内 Pin 钮与设置同源
  #[serde(default = "bool_true")]
  pub todo_float_pinned: bool,
  /// 待办悬浮窗呼出/隐藏全局热键；None = 未绑定
  #[serde(default = "default_todo_hotkey")]
  pub todo_hotkey: Option<String>,
  /// 直达知识库全局热键（kb-storage-backup S4）：呼出主窗并导航到 /kb；None = 未绑定
  #[serde(default = "default_kb_hotkey")]
  pub kb_hotkey: Option<String>,
  pub remind_cap_per_hour: u32,
  pub onboarded: bool,
  #[serde(skip_serializing_if = "is_absent")]
  pub export_dir: Option<String>,
  /// 本地度量埋点开关（默认开；纯本机、绝不出网）
  #[serde(default = "telemetry_default")]
  pub telemetry_enabled: bool,
  /// 阅读面字族："" = 跟随正文；`system:<键>` = 系统字体；`file:<文件名>` = 用户上传
  #[serde(default)]
  pub reading_font: String,
  /// 阅读面字号（px，白名单 8/10/12/14/16/18/20；越界回落 16）
  #[serde(default = "default_reading_size")]
  pub reading_size: u8,
  /// 阅读面字距档位（0..=6，对应 0–0.09em 七档；越界回落 0）
  #[serde(default)]
  pub reading_tracking: u8,
  /// 正文列宽上限档位（comfortable 96ch | wide 112ch；越界回落 comfortable）
  #[serde(default = "default_reading_measure")]
  pub reading_measure: String,
  /// 阅读主题："" = 默认外观；`builtin:<id>` = 内置主题；`file:<文件名>` = 用户导入
  #[serde(default)]
  pub reading_theme: String,
}

fn telemetry_default() -> bool {
  true
}

fn default_sticky_hotkey() -> Option<String> {
  Some(DEFAULT_STICKY_HOTKEY.to_string())
}

fn default_todo_hotkey() -> Option<String> {
  Some(DEFAULT_TODO_HOTKEY.to_string())
}

fn default_kb_hotkey() -> Option<String> {
  Some(DEFAULT_KB_HOTKEY.to_string())
}

fn todo_float_visible_default() -> bool {
  true
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
      todo_float_pinned: true,
      todo_hotkey: Some(DEFAULT_TODO_HOTKEY.to_string()),
      kb_hotkey: Some(DEFAULT_KB_HOTKEY.to_string()),
      remind_cap_per_hour: REMINDER_CAP_DEFAULT,
      onboarded: false,
      export_dir: None,
      telemetry_enabled: true,
      reading_font: String::new(),
      reading_size: default_reading_size(),
      reading_tracking: 0,
      reading_measure: default_reading_measure(),
      reading_theme: String::new(),
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
    if let Some(value) = self.todo_hotkey.as_deref() {
      if value.trim().is_empty() {
        self.todo_hotkey = None;
      }
    }
    if let Some(value) = self.kb_hotkey.as_deref() {
      if value.trim().is_empty() {
        self.kb_hotkey = None;
      }
    }
    if self.remind_cap_per_hour == 0 || self.remind_cap_per_hour > 60 {
      self.remind_cap_per_hour = REMINDER_CAP_DEFAULT;
    }
    self.reading_size = normalize_reading_size(self.reading_size);
    self.reading_tracking = normalize_reading_tracking(self.reading_tracking);
    self.reading_measure = normalize_reading_measure(&self.reading_measure);
    self.reading_theme = normalize_reading_theme(&self.reading_theme);
  }
}

/// 便签（sticky-separation 定稿）：纯自由便签，≤500 字纯文本，多开。
/// 生命周期：展开 ↔ 缩小（置顶悬浮文本条）↔ 隐藏（✕/Alt+F4 只关窗，数据保留）；
/// 显式 delete_sticky 才真删除。可见（visible=true）便签合计上限 = STICKY_CAP。
/// 常驻置顶无开关。内容归 data.json（用户数据，ADR-0005）；位置归 runtime.json
/// note_pos（ADR-0007，隐藏不清位置）。
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
  /// Phase 2：知识便签视口指针（指向 KbItem.id）。
  /// None = 自由便签；Some = 知识便签（正文不落便签，由 KB 条目持有）。
  /// skip_serializing_if = None：旧数据无此字段→默认 None→零迁移。
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub kb_ref: Option<String>,
  /// 可见性（fix-known-defects D9）：关闭（✕ / Alt+F4）只置 false 并隐藏窗口，
  /// 不删数据、不清位置；可从便签列表 open_sticky 重新打开。隐藏便签不计入上限。
  /// 旧数据缺字段 → true（默认可见，零迁移，不写迁移脚本）；
  /// visible=true 时省略字段，旧数据形状不变（前端按 `!== false` 解读可见）。
  #[serde(default = "bool_true", skip_serializing_if = "is_true")]
  pub visible: bool,
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
      kb_ref: None,
      visible: true,
      created_at: stamp.clone(),
      updated_at: stamp,
    }
  }
}

/// 可见便签数（free + 知识便签合计，隐藏不计）：上限判定与单测共用纯函数，
/// 不依赖 AppHandle、不碰锁，便于单测与托盘/命令多处复用。
pub fn visible_sticky_count(stickies: &[StickyNote]) -> usize {
  stickies.iter().filter(|note| note.visible).count()
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
  /// 三态：None = 不改；Some("") = 移到未分类（存 None）；Some(id) = 归到该文件夹（校验存在）
  pub folder_id: Option<String>,
  /// 同级排序目标索引（kb-file-tree D4）：None = 不改；Some(i) = 插到该父级索引位后 dense 重排
  pub order: Option<i64>,
  /// 置顶（kb-file-tree D6）：None = 不改；Some(true/false) = 置位/取消
  pub pinned: Option<bool>,
}

pub const STICKY_PAPERS: [&str; 4] = ["warm", "kraft", "cyan", "ink"];
/// 纸面花纹（sticky-background-pattern.md 定稿：无 / 墨竹 / 远山；选竹或山时朱印自动伴随）
pub const STICKY_PATTERNS: [&str; 3] = ["none", "bamboo", "mountain"];
/// 同时可见便签总数上限（自由 + 知识合计，隐藏不计）——防「贴满废纸」，
/// 对应 metric Counter 壁纸化。分池已取消（fix-known-defects 4.2）。
pub const STICKY_CAP: usize = 5;

// ---------------------------------------------------------------- 阅读面字体（reading-font）

/// 设置里可选的系统字族：`readingFont = "system:<键>"`，键即 CSS 族名，
/// 前端用同一张表拼回退栈，后端只做白名单校验。
pub const READING_FONT_SYSTEMS: [(&str, &str); 6] = [
  ("SimSun", "宋体"),
  ("KaiTi", "楷体"),
  ("FangSong", "仿宋"),
  ("Microsoft YaHei", "微软雅黑"),
  // 思源（Noto CJK）随 Windows 11 中文版预装；族名必须与系统注册名逐字一致，否则前端解析不到
  ("Noto Serif SC", "思源宋体"),
  ("Noto Sans SC", "思源黑体"),
];

const FONT_EXTENSIONS: [&str; 4] = ["ttf", "otf", "woff", "woff2"];

/// 上传字体的文件名白名单：不许带路径、不许控制字符、不许点开头、扩展名要认识。
/// 真正的落盘路径拼接在 `infra::fonts`，这里只回答「这个名字能不能用」。
pub fn is_safe_font_file_name(name: &str) -> bool {
  let trimmed = name.trim();
  if trimmed.is_empty() || trimmed.chars().count() > 80 || trimmed.starts_with('.') {
    return false;
  }
  if trimmed
    .chars()
    .any(|ch| ch.is_control() || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
  {
    return false;
  }
  let lower = trimmed.to_ascii_lowercase();
  match lower.rsplit_once('.') {
    Some((stem, ext)) => !stem.is_empty() && FONT_EXTENSIONS.contains(&ext),
    None => false,
  }
}

/// 设置值归一化：空串 = 跟随正文；`system:<键>`；`file:<安全文件名>`。
pub fn normalize_reading_font(value: &str) -> Result<String, String> {
  let trimmed = value.trim();
  if trimmed.is_empty() {
    return Ok(String::new());
  }
  if let Some(key) = trimmed.strip_prefix("system:") {
    if READING_FONT_SYSTEMS.iter().any(|(name, _)| *name == key) {
      return Ok(format!("system:{key}"));
    }
    return Err("这个系统字体不在可选项里".to_string());
  }
  if let Some(name) = trimmed.strip_prefix("file:") {
    if is_safe_font_file_name(name) {
      return Ok(format!("file:{name}"));
    }
    return Err("字体文件名不合法".to_string());
  }
  Err("字体设置不认识，已忽略".to_string())
}

// ---------------------------------------------------------------- 阅读主题（reading-themes）

/// 内置阅读主题 id：与 `src-react/src/assets/themes/<id>.css` 一一对应。
/// 前端另有一份同样的表（`src-react/src/lib/reading-theme.js`），一致性由
/// `test/reading-theme-builtins.test.mjs` 把关；发布后 id 不再改名。
pub const READING_THEME_BUILTINS: [&str; 2] = ["paperglow", "mdmdt"];

const THEME_EXTENSION: &str = "css";

/// 导入主题的文件名白名单：不许带路径、不许控制字符、不许点开头、必须是 `.css`。
/// 真正的落盘路径拼接在 `infra::themes`，这里只回答「这个名字能不能用」。
pub fn is_safe_theme_file_name(name: &str) -> bool {
  let trimmed = name.trim();
  if trimmed.is_empty() || trimmed.chars().count() > 64 || trimmed.starts_with('.') {
    return false;
  }
  if trimmed
    .chars()
    .any(|ch| ch.is_control() || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
  {
    return false;
  }
  match trimmed.rsplit_once('.') {
    Some((stem, ext)) => !stem.is_empty() && ext.eq_ignore_ascii_case(THEME_EXTENSION),
    None => false,
  }
}

/// 设置值归一化：空串 = 默认外观；`builtin:<白名单 id>`；`file:<安全文件名>`。
/// 不认识的值一律回空串（不报错）：主题只影响观感，坏值退回默认比拒绝保存更稳。
pub fn normalize_reading_theme(value: &str) -> String {
  let trimmed = value.trim();
  if trimmed.is_empty() {
    return String::new();
  }
  if let Some(id) = trimmed.strip_prefix("builtin:") {
    if READING_THEME_BUILTINS.contains(&id) {
      return format!("builtin:{id}");
    }
    return String::new();
  }
  if let Some(name) = trimmed.strip_prefix("file:") {
    if is_safe_theme_file_name(name) {
      return format!("file:{name}");
    }
    return String::new();
  }
  String::new()
}

// ---------------------------------------------------------------- 阅读偏好档位（reading-prefs）

/// 可选阅读字号（px）：12–24 步长 1，白名单而非区间，避免 0–11 这类不可读值。
pub const READING_SIZES: [u8; 13] = [12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24];

/// 字距三档（0..=2）：常规 0 / 宽松 0.03em / 较宽 0.06em，换算在前端 `reading-prefs.ts`。
pub const READING_TRACKING_MAX: u8 = 2;

/// 列宽上限两档：舒适 96ch / 宽敞 112ch
pub const READING_MEASURES: [&str; 2] = ["comfortable", "wide"];

fn default_reading_size() -> u8 {
  15
}

fn default_reading_measure() -> String {
  "comfortable".to_string()
}

/// 字号越界一律回落到 15px（既不取最近档，也不夹紧，保证可复现）。
pub fn normalize_reading_size(value: u8) -> u8 {
  if READING_SIZES.contains(&value) {
    value
  } else {
    default_reading_size()
  }
}

/// 字距档位越界回落到 0（无额外字距）。
pub fn normalize_reading_tracking(value: u8) -> u8 {
  if value <= READING_TRACKING_MAX {
    value
  } else {
    0
  }
}

/// 列宽档位名字越界回落到 comfortable。
pub fn normalize_reading_measure(value: &str) -> String {
  let trimmed = value.trim();
  if READING_MEASURES.contains(&trimmed) {
    trimmed.to_string()
  } else {
    default_reading_measure()
  }
}

#[cfg(test)]
mod reading_pref_tests {
  use super::*;

  #[test]
  fn size_whitelist_accepts_every_step() {
    for size in READING_SIZES {
      assert_eq!(normalize_reading_size(size), size);
    }
  }

  #[test]
  fn rejects_off_step_size_and_tracking() {
    assert_eq!(normalize_reading_size(11), 15);
    assert_eq!(normalize_reading_size(0), 15);
    assert_eq!(normalize_reading_tracking(3), 0);
    assert_eq!(normalize_reading_tracking(2), 2);
  }

  #[test]
  fn missing_fields_fall_back_to_defaults() {
    // 旧 data.json 没有这三个字段；容器级 #[serde(default)] 必须把它们补成默认值
    let settings: Settings = serde_json::from_str("{}").unwrap();
    assert_eq!(settings.reading_size, 15);
    assert_eq!(settings.reading_tracking, 0);
    assert_eq!(settings.reading_measure, "comfortable");
  }

  #[test]
  fn measure_falls_back_to_comfortable() {
    assert_eq!(normalize_reading_measure("wide"), "wide");
    assert_eq!(normalize_reading_measure(" comfortable "), "comfortable");
    assert_eq!(normalize_reading_measure("wide-ish"), "comfortable");
    assert_eq!(normalize_reading_measure(""), "comfortable");
  }
}

#[cfg(test)]
mod reading_font_tests {
  use super::*;

  #[test]
  fn accepts_plain_and_prefixed_values() {
    assert_eq!(normalize_reading_font("").unwrap(), "");
    assert_eq!(normalize_reading_font("system:KaiTi").unwrap(), "system:KaiTi");
    assert_eq!(normalize_reading_font("  file:文楷.ttf ").unwrap(), "file:文楷.ttf");
  }

  #[test]
  fn rejects_unknown_system_face_and_loose_value() {
    assert!(normalize_reading_font("system:Comic Sans").is_err());
    assert!(normalize_reading_font("KaiTi").is_err());
    assert!(normalize_reading_font("file:../../evil.ttf").is_err());
  }

  #[test]
  fn accepts_noto_faces_registered_on_windows() {
    assert_eq!(
      normalize_reading_font("system:Noto Serif SC").unwrap(),
      "system:Noto Serif SC"
    );
    assert_eq!(
      normalize_reading_font("system:Noto Sans SC").unwrap(),
      "system:Noto Sans SC"
    );
    assert!(normalize_reading_font("system:Noto Serif").is_err());
  }

  #[test]
  fn filename_whitelist_covers_edges() {
    assert!(is_safe_font_file_name("a.ttf"));
    assert!(is_safe_font_file_name("LXGWWenKai-Regular.woff2"));
    assert!(!is_safe_font_file_name("a.exe"));
    assert!(!is_safe_font_file_name(".hidden.ttf"));
    assert!(!is_safe_font_file_name("dir/a.ttf"));
    assert!(!is_safe_font_file_name("a b.txt"));
    assert!(!is_safe_font_file_name(""));
  }
}

#[cfg(test)]
mod reading_theme_tests {
  use super::*;

  #[test]
  fn accepts_empty_builtin_and_file_values() {
    assert_eq!(normalize_reading_theme(""), "");
    assert_eq!(normalize_reading_theme("  "), "");
    assert_eq!(normalize_reading_theme("builtin:paperglow"), "builtin:paperglow");
    assert_eq!(normalize_reading_theme("builtin:mdmdt"), "builtin:mdmdt");
    assert_eq!(normalize_reading_theme(" file:我的主题.css "), "file:我的主题.css");
  }

  #[test]
  fn unknown_or_unsafe_values_fall_back_to_empty() {
    assert_eq!(normalize_reading_theme("builtin:unknown"), "");
    assert_eq!(normalize_reading_theme("builtin:sujian"), "");
    assert_eq!(normalize_reading_theme("file:../evil.css"), "");
    assert_eq!(normalize_reading_theme("file:evil.txt"), "");
    assert_eq!(normalize_reading_theme("paperglow"), "");
    assert_eq!(normalize_reading_theme("file:"), "");
  }

  #[test]
  fn theme_filename_whitelist_covers_edges() {
    assert!(is_safe_theme_file_name("a.css"));
    assert!(is_safe_theme_file_name("我的主题.CSS"));
    assert!(!is_safe_theme_file_name("a.txt"));
    assert!(!is_safe_theme_file_name(".hidden.css"));
    assert!(!is_safe_theme_file_name("dir/a.css"));
    assert!(!is_safe_theme_file_name(""));
    assert!(!is_safe_theme_file_name(&"a".repeat(60) /* 无扩展名 */));
    assert!(!is_safe_theme_file_name(&format!("{}.css", "a".repeat(62))));
  }

  #[test]
  fn settings_coerce_keeps_legal_theme_and_drops_legal_looking_junk() {
    let mut settings = Settings::default();
    settings.reading_theme = "builtin:mdmdt".to_string();
    settings.coerce();
    assert_eq!(settings.reading_theme, "builtin:mdmdt");

    settings.reading_theme = "file:../../evil.css".to_string();
    settings.coerce();
    assert_eq!(settings.reading_theme, "");
  }

  #[test]
  fn old_data_json_without_reading_theme_deserializes_to_default() {
    let settings: Settings = serde_json::from_str("{}").expect("空对象应能反序列化");
    assert_eq!(settings.reading_theme, "");
  }
}

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
  /// 所属文件夹（None = 未分类）。旧 notes.json 无该字段靠 default 兼容；
  /// 序列化必须保留 `folderId: null`（不加 skip_serializing_if），前端按 null 判未分类。
  #[serde(default)]
  pub folder_id: Option<String>,
  /// 同级排序（kb-file-tree D4）：dense 整数，越小越靠前。旧 notes.json 无该字段 → None，
  /// 加载后按 updated_at 倒序在同级生成 0..n-1（只在内存生成，下次保存落盘）。
  /// 序列化必须保留 `order: null`（不加 skip_serializing_if），前端类型为 `number | null`。
  #[serde(default)]
  pub order: Option<i64>,
  /// 置顶（kb-file-tree D6）：置顶项在树顶部快捷区展示且原位置保留。旧 JSON 缺字段 → false。
  #[serde(default)]
  pub pinned: bool,
  /// 正文内容指纹（kb-storage-backup D1）：`sha256(规范化正文)` 的 64 位小写十六进制。
  /// 旧 IPC / 旧 notes.json 缺该字段 → None（前端旧类型可缺省）；序列化保留
  /// `contentHash: null`（与 folderId/order 同风格，不加 skip_serializing_if）。
  #[serde(default)]
  pub content_hash: Option<String>,
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
      folder_id: None,
      order: None,
      pinned: false,
      content_hash: None,
    }
  }
}

/// 知识库元数据索引（kb-storage-backup D1）：`notes.json` 数组的唯一元素形状。
/// 正文在 `notes/<id>.md`，图片在 `assets/<sha256>.<ext>`；索引 MUST NOT 内联正文。
/// 与 `KbItem` 同一套 camelCase IPC 契约（folderId/order 保留 null，前端按 null 判未分类）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct KbItemMeta {
  pub id: String,
  pub title: String,
  pub tags: Vec<String>,
  /// None = 未分类；序列化保留 `folderId: null`
  #[serde(default)]
  pub folder_id: Option<String>,
  /// 同级 dense 排序；序列化保留 `order: null`
  #[serde(default)]
  pub order: Option<i64>,
  #[serde(default)]
  pub pinned: bool,
  pub created_at: String,
  pub updated_at: String,
  /// 正文 SHA-256（64 位小写十六进制），由 `digest::sha256_hex` 计算
  pub content_hash: String,
}

impl Default for KbItemMeta {
  fn default() -> Self {
    Self {
      id: String::new(),
      title: String::new(),
      tags: Vec::new(),
      folder_id: None,
      order: None,
      pinned: false,
      created_at: String::new(),
      updated_at: String::new(),
      content_hash: String::new(),
    }
  }
}

/// 知识库文件夹（D6：独立 folders.json，KbItem 仅持引用）。
/// `name` 由命令层校验（trim 非空、≤40 字）；`order` 为同级 dense 排序（0..n-1，
/// 移动/排序时整体重排；旧数据允许空洞，下一次结构操作收敛）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KbFolder {
  pub id: String,
  pub name: String,
  pub parent_id: Option<String>,
  pub order: i64,
  /// 置顶（kb-file-tree D6）：文件夹同样可置顶，快捷区展示且原位置保留。旧 JSON 缺字段 → false。
  #[serde(default)]
  pub pinned: bool,
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
  /// 待办悬浮窗上次显隐（todo-float）：重启恢复；损坏重建时默认显示
  #[serde(default = "todo_float_visible_default")]
  pub todo_float_visible: bool,
  /// 待办悬浮窗最小化（fix-known-defects 4.7）：true = 380×40 单行条；
  /// false（缺省）= 380×456 完整面板。旧 runtime.json 缺字段 → false，零迁移。
  #[serde(default)]
  pub todo_float_mini: bool,
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
  pub todo_float_pinned: Option<bool>,
  pub todo_hotkey: Option<String>,
  pub remind_cap_per_hour: Option<u32>,
  pub onboarded: Option<bool>,
  pub export_dir: Option<String>,
  pub reading_font: Option<String>,
  pub reading_size: Option<u8>,
  pub reading_tracking: Option<u8>,
  pub reading_measure: Option<String>,
  pub reading_theme: Option<String>,
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

/// 知识库备份（notes.json.bak-1..5），与 BackupInfo 同构但不含 taskCount。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KbBackupInfo {
  /// "1".."5"
  pub slot: String,
  /// notes.json.bak-1 .. notes.json.bak-5
  pub name: String,
  pub created_at: String,
  pub size_kb: u64,
  pub note_count: u32,
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
  pub todo: Option<String>,
  /// 直达知识库热键（kb-storage-backup S4 第五槽）
  pub kb: Option<String>,
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
pub struct RestoreKbResult {
  /// 恢复后知识条目数
  pub restored: usize,
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
  /// 待办悬浮窗最小化态（fix-known-defects 4.7）：前端启动时据此决定窗口初始形态；
  /// 真实窗口尺寸由 Rust 建窗时按 runtime.todo_float_mini 落定，前端只读不写。
  pub todo_float_mini: bool,
  /// 知识库图片的绝对 assets 目录（kb-storage-backup D2）。
  /// 前端用 `convertFileSrc(<assets_dir>/<sha>.<ext>)` 显示正文里的 `assets/<sha>.<ext>`。
  pub assets_dir: String,
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
  fn settings_kb_hotkey_defaults_for_old_config_and_coerces_blank() {
    // 旧配置缺 kbHotkey：serde default 必须补 Alt+Shift+K 且不报错（kb-storage-backup S4）
    let old: Settings = serde_json::from_value(json!({ "theme": "float" })).unwrap();
    assert_eq!(old.kb_hotkey.as_deref(), Some(DEFAULT_KB_HOTKEY));
    // 用户主动清空：空串 / 纯空白 coerce 成 None（表示不注册）
    let mut settings = Settings::default();
    settings.kb_hotkey = Some("   ".to_string());
    settings.coerce();
    assert_eq!(settings.kb_hotkey, None);
    // 非空值保持原样（与 main/sticky/todo 同形态）
    settings.kb_hotkey = Some(" Ctrl+Shift+K ".to_string());
    settings.coerce();
    assert_eq!(settings.kb_hotkey.as_deref(), Some(" Ctrl+Shift+K "));
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

#[cfg(test)]
mod sticky_visibility_tests {
  use super::*;

  /// 旧数据（fix-known-defects 之前落盘）没有 visible 字段：必须默认 true（可见），
  /// 靠 serde default 兼容，不写迁移脚本、不改 data.json 形状。
  #[test]
  fn legacy_sticky_without_visible_deserializes_as_visible() {
    let raw = serde_json::json!({
      "id": "n1",
      "content": "旧便签",
      "createdAt": "2026-09-01T10:00",
      "updatedAt": "2026-09-01T10:00"
    });
    let note: StickyNote = serde_json::from_value(raw).expect("旧 JSON 必须可反序列化");
    assert!(note.visible, "旧数据缺 visible 必须默认 true（可见）");
    // 反向：显式 false 必须读回 false（隐藏态可持久化）
    let raw_hidden = serde_json::json!({
      "id": "n2",
      "content": "已隐藏",
      "visible": false,
      "createdAt": "2026-09-01T10:00",
      "updatedAt": "2026-09-01T10:00"
    });
    let hidden: StickyNote = serde_json::from_value(raw_hidden).expect("隐藏便签可反序列化");
    assert!(!hidden.visible);
  }

  /// 上限判定只看 visible：隐藏便签不计入（fix-known-defects 4.2 合计上限 5）。
  #[test]
  fn visible_sticky_count_only_counts_visible() {
    let mut a = StickyNote::default();
    let mut b = StickyNote::default();
    let mut c = StickyNote::default();
    a.visible = true;
    b.visible = false;
    c.visible = true;
    assert_eq!(visible_sticky_count(&[a, b, c]), 2);
    assert_eq!(visible_sticky_count(&[]), 0);
    assert_eq!(STICKY_CAP, 5, "可见便签合计上限固定为 5");
  }

  /// visible=true 省略字段（旧数据形状不变）；visible=false 必须落盘，
  /// 否则重启读回默认 true 会把用户关掉的便签全部重开。
  #[test]
  fn visible_serialization_skips_true_but_keeps_false() {
    let visible = serde_json::to_value(StickyNote::default()).expect("可序列化");
    assert!(
      visible.get("visible").is_none(),
      "visible=true 必须省略字段：旧数据形状不变"
    );
    let hidden = serde_json::to_value(StickyNote {
      visible: false,
      ..Default::default()
    })
    .expect("可序列化");
    assert_eq!(hidden["visible"], serde_json::json!(false));
  }
}

#[cfg(test)]
mod kb_tree_serialization_tests {
  use super::*;

  /// 旧 notes.json / folders.json 缺 order、pinned：必须靠 serde default 无损读取
  /// （kb-file-tree D4/D6：不写迁移脚本、不改文件形状）。
  #[test]
  fn kb_legacy_json_defaults_order_and_pinned() {
    let legacy_item: KbItem = serde_json::from_value(serde_json::json!({
      "id": "k1",
      "title": "旧条目",
      "bodyMd": "",
      "tags": [],
      "createdAt": "2026-09-01T09:00",
      "updatedAt": "2026-09-01T09:00"
    }))
    .expect("旧条目 JSON 必须可读");
    assert_eq!(legacy_item.order, None, "旧 notes 缺 order → None（加载后内存补齐）");
    assert!(!legacy_item.pinned, "旧 notes 缺 pinned → false");
    assert_eq!(legacy_item.folder_id, None, "旧 notes 缺 folderId → 未分类");

    let legacy_folder: KbFolder = serde_json::from_value(serde_json::json!({
      "id": "f1",
      "name": "旧文件夹",
      "parentId": null,
      "order": 3
    }))
    .expect("旧文件夹 JSON 必须可读");
    assert!(!legacy_folder.pinned, "旧 folders 缺 pinned → false");

    let payload: KbPayload = serde_json::from_value(serde_json::json!({ "title": "t" }))
      .expect("旧 payload 必须可读");
    assert_eq!(payload.order, None, "缺省 order = 不改排序");
    assert_eq!(payload.pinned, None, "缺省 pinned = 不改置顶");
    let full: KbPayload =
      serde_json::from_value(serde_json::json!({ "order": 2, "pinned": true }))
        .expect("新 payload 必须可读");
    assert_eq!(full.order, Some(2));
    assert_eq!(full.pinned, Some(true));
  }

  /// order 必须序列化为数字 / null（前端类型 number | null），不能 skip 省略字段
  #[test]
  fn kb_order_serializes_as_null_not_omitted() {
    let item = KbItem::default();
    let value = serde_json::to_value(&item).expect("KbItem 必须可序列化");
    assert!(
      value.get("order").is_some(),
      "order 字段必须始终出现（null 也是合法值）：{value}"
    );
    assert!(value["order"].is_null(), "None → order: null");
    assert_eq!(value["pinned"], serde_json::json!(false), "pinned 必须序列化");

    let folder = KbFolder {
      id: "f1".to_string(),
      name: "工作".to_string(),
      parent_id: None,
      order: 1,
      pinned: true,
    };
    let value = serde_json::to_value(&folder).expect("KbFolder 必须可序列化");
    assert_eq!(value["pinned"], serde_json::json!(true));
    assert_eq!(value["order"], serde_json::json!(1));
  }
}

#[cfg(test)]
mod kb_storage_serialization_tests {
  use super::*;

  /// KbItemMeta 是 notes.json 的唯一元素形状：camelCase 字段、null 必须保留
  /// （与 KbItem 同一 IPC 契约），且绝不能出现 bodyMd。
  #[test]
  fn kb_item_meta_serializes_camel_case_and_keeps_nulls() {
    let meta = KbItemMeta {
      id: "k1".to_string(),
      title: "标题".to_string(),
      tags: vec!["rust".to_string()],
      folder_id: None,
      order: None,
      pinned: false,
      created_at: "2026-09-01T09:00".to_string(),
      updated_at: "2026-09-02T09:00".to_string(),
      content_hash: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        .to_string(),
    };
    let value = serde_json::to_value(&meta).expect("KbItemMeta 必须可序列化");
    assert_eq!(value["id"], "k1");
    assert_eq!(value["title"], "标题");
    assert_eq!(value["createdAt"], "2026-09-01T09:00");
    assert_eq!(value["updatedAt"], "2026-09-02T09:00");
    assert_eq!(value["contentHash"], meta.content_hash);
    assert!(
      value.get("created_at").is_none() && value.get("content_hash").is_none(),
      "索引字段必须是 camelCase：{value}"
    );
    assert!(
      matches!(value.get("folderId"), Some(serde_json::Value::Null)),
      "folderId:null 必须保留（前端按 null 判未分类）：{value}"
    );
    assert!(matches!(value["order"], serde_json::Value::Null), "order:null 必须保留");
    assert_eq!(value["pinned"], serde_json::json!(false));
    assert!(
      value.get("bodyMd").is_none() && value.get("body_md").is_none(),
      "索引 MUST NOT 内联正文：{value}"
    );
    let back: KbItemMeta = serde_json::from_value(value).expect("索引必须可反序列化");
    assert_eq!(back, meta);
  }

  /// 旧 IPC / 旧索引缺字段：serde default 兜底，不因此报错。
  #[test]
  fn kb_item_meta_tolerates_missing_optional_fields() {
    let meta: KbItemMeta =
      serde_json::from_value(serde_json::json!({ "id": "k1", "contentHash": "abc" }))
        .expect("缺省字段的索引必须可读");
    assert_eq!(meta.folder_id, None);
    assert_eq!(meta.order, None);
    assert!(!meta.pinned);
    assert_eq!(meta.tags, Vec::<String>::new());
  }

  /// KbItem 的 contentHash 对旧前端可选：缺省 → None，序列化保留 null。
  #[test]
  fn kb_item_content_hash_is_optional_for_legacy_ipc() {
    let legacy: KbItem = serde_json::from_value(serde_json::json!({
      "id": "k1",
      "title": "旧条目",
      "bodyMd": "正文",
      "tags": [],
      "createdAt": "2026-09-01T09:00",
      "updatedAt": "2026-09-01T09:00"
    }))
    .expect("旧 KbItem JSON 必须可读");
    assert_eq!(legacy.content_hash, None, "旧 IPC 缺 contentHash → None");
    let value = serde_json::to_value(&legacy).expect("KbItem 必须可序列化");
    assert!(
      matches!(value.get("contentHash"), Some(serde_json::Value::Null)),
      "contentHash 必须序列化为 null（不省略字段）：{value}"
    );

    let with_hash: KbItem = serde_json::from_value(serde_json::json!({
      "id": "k2",
      "contentHash": "abc"
    }))
    .expect("带 contentHash 的 JSON 必须可读");
    assert_eq!(with_hash.content_hash.as_deref(), Some("abc"));
  }
}

#[cfg(test)]
mod legacy_settings_compat_tests {
  use super::*;

  /// 免打扰已从产品移除（sticky-shell-polish quiet-mode-removal）：老 data.json 的
  /// `settings.dnd` 是嵌套对象，加载时必须被静默忽略，不能报错进恢复页。
  /// Settings 故意不设 deny_unknown_fields —— 兼容旧字段正是本契约的根。
  #[test]
  fn legacy_settings_with_dnd_loads_and_ignores_it() {
    let raw = serde_json::json!({
      "settings": {
        "theme": "dark",
        "dnd": { "enabled": true, "from": "23:00", "to": "07:30" }
      }
    });
    let data: AppData = serde_json::from_value(raw).expect("旧 data.json 带 settings.dnd 必须可加载");
    assert_eq!(data.settings.theme, "dark");
    // 保存一次即自然消失：序列化输出不得再出现 dnd（无需迁移脚本）
    let saved = serde_json::to_value(&data).expect("AppData 必须可序列化");
    assert!(
      saved["settings"].get("dnd").is_none(),
      "新写入不得再出现 settings.dnd：{saved}"
    );
  }

  /// 旧前端仍可能带 dnd patch：SettingsPayload 同样忽略未知字段，不拒绝整次保存。
  #[test]
  fn legacy_settings_payload_with_dnd_is_accepted() {
    let payload: SettingsPayload = serde_json::from_value(serde_json::json!({
      "theme": "light",
      "dnd": { "enabled": false, "from": "22:00", "to": "08:00" }
    }))
    .expect("旧 patch 带 dnd 必须可反序列化");
    assert_eq!(payload.theme.as_deref(), Some("light"));
  }
}

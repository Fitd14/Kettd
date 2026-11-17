//! 所有对前端的命令（invoke 参数 camelCase；返回 Result<_, String>，Err 为可显示的中文短句）

use crate::backup;
use crate::export;
use crate::models::{
  clocks_of, new_id, normalize_datetime, now_text,
  Bootstrap, CATEGORIES, DataHealthV2, DEFAULT_HOTKEY, KbBackupInfo, KbFolder, KbItem, KbPayload,
  HotkeyStatus, MigrationReport, Note, NotePayload, PRIORITIES, REMINDER_CAP_DEFAULT, Reminder, ReminderPayload,
  RestoreKbResult, RestoreResult, Settings, SettingsPayload, Source, StickyNote, StoreEvent, Subtask, Task,
  TaskPayload, TRASH_RETENTION_DAYS, today,
};
use crate::runtime;
use crate::store::{Store, Undoable};
use std::sync::{Mutex, MutexGuard};
use tauri::{AppHandle, Manager, State};

type Shared = Mutex<Store>;

/// 取存储守卫；中毒锁直接复用内部状态（不 panic、不静默丢数据）
fn lock<'m>(mutex: &'m Shared) -> MutexGuard<'m, Store> {
  match mutex.lock() {
    Ok(guard) => guard,
    Err(poisoned) => poisoned.into_inner(),
  }
}

fn emit(app: &AppHandle, event: StoreEvent) {
  let _ = app.emit_all("store-changed", event);
}

fn save_and_emit(store: &mut Store, app: &AppHandle, id: &str) -> Result<(), String> {
  store.save()?;
  emit(app, StoreEvent::changed(id));
  Ok(())
}

fn task_error() -> String {
  "找不到这条待办，可能已经被删除".to_string()
}

// ---------------------------------------------------------------- 引导 / 读取

#[tauri::command]
pub fn get_bootstrap(state: State<'_, Shared>) -> Result<Bootstrap, String> {
  let store = lock(&state);
  Ok(Bootstrap {
    tasks: store.tasks(false),
    reminders: store.data.reminders.clone(),
    settings: store.data.settings.clone(),
    health: store.health_view(),
    migration: store.runtime.migration.clone(),
    version: env!("CARGO_PKG_VERSION").to_string(),
    todo_float_mini: store.runtime.todo_float_mini,
    assets_dir: store.assets_dir(),
  })
}

#[tauri::command]
pub fn get_tasks(
  state: State<'_, Shared>,
  include_deleted: Option<bool>,
) -> Result<Vec<Task>, String> {
  let store = lock(&state);
  Ok(store.tasks(include_deleted.unwrap_or(false)))
}

#[tauri::command]
pub fn get_task(state: State<'_, Shared>, id: String) -> Result<Option<Task>, String> {
  let store = lock(&state);
  Ok(store.find(&id).cloned())
}

#[tauri::command]
pub fn get_settings(state: State<'_, Shared>) -> Result<Settings, String> {
  let store = lock(&state);
  Ok(store.data.settings.clone())
}

#[tauri::command]
pub fn get_reminders(state: State<'_, Shared>) -> Result<Vec<Reminder>, String> {
  let store = lock(&state);
  Ok(store.data.reminders.clone())
}

#[tauri::command]
pub fn get_data_health(state: State<'_, Shared>) -> Result<DataHealthV2, String> {
  let store = lock(&state);
  let view = store.health_view();
  if view.health != "ok" {
    crate::telemetry::record_str("data_health_fail", &[("kind", view.health.as_str())]);
  }
  Ok(view)
}

/// 热键实际注册状态：设置页据此判断「当前绑定」是否真的生效（qa-1）
#[tauri::command]
pub fn get_hotkey_status(app: AppHandle) -> Result<HotkeyStatus, String> {
  Ok(runtime::hotkey_status(&app))
}

#[tauri::command]
pub fn get_backups(state: State<'_, Shared>) -> Result<Vec<crate::models::BackupInfo>, String> {
  let store = lock(&state);
  Ok(store.backups())
}

// ---------------------------------------------------------------- 待办写入

#[tauri::command]
pub fn add_task(
  app: AppHandle,
  state: State<'_, Shared>,
  args: TaskPayload,
) -> Result<Task, String> {
  let mut store = lock(&state);
  store.ensure_writable()?;
  let from_capture = matches!(args.source, Some(Source::Capture));
  // 规则在 app/tasks：标题非空、默认值、拖留初值
  let task = match crate::app::tasks::add(&mut store, args, &now_text(), today()) {
    Ok(task) => task,
    Err(e) => {
      if from_capture {
        // 捕获提交失败（Health 指标：落库失败率 ≤1%）
        let kind: String = e.chars().take(48).collect();
        crate::telemetry::record_str("capture_commit_fail", &[("err", kind.as_str())]);
      }
      return Err(e);
    }
  };
  save_and_emit(&mut store, &app, &task.id.clone())?;
  if matches!(task.source, Source::Capture) {
    crate::telemetry::capture_commit();
  }
  Ok(task)
}

#[tauri::command]
pub fn update_task(
  app: AppHandle,
  state: State<'_, Shared>,
  id: String,
  patch: TaskPayload,
) -> Result<Task, String> {
  let mut store = lock(&state);
  store.ensure_writable()?;
  // 规则在 app/tasks：拖留只增不减
  let next = crate::app::tasks::update(&mut store, &id, patch, &now_text(), today())?;
  save_and_emit(&mut store, &app, &id)?;
  Ok(next)
}

#[tauri::command]
pub fn toggle_task(
  app: AppHandle,
  state: State<'_, Shared>,
  id: String,
) -> Result<Task, String> {
  let mut store = lock(&state);
  store.ensure_writable()?;
  let updated = store.toggle_task(&id)?;
  save_and_emit(&mut store, &app, &id)?;
  Ok(updated)
}

/// 软删除：只写 deletedAt，返回被删的完整对象；10s 内可 undo_delete
#[tauri::command]
pub fn delete_task(
  app: AppHandle,
  state: State<'_, Shared>,
  id: String,
) -> Result<Task, String> {
  let mut store = lock(&state);
  store.ensure_writable()?;
  let snapshot = store.soft_delete_task(&id)?;
  save_and_emit(&mut store, &app, &id)?;
  Ok(snapshot)
}

#[tauri::command]
pub fn undo_delete(app: AppHandle, state: State<'_, Shared>) -> Result<Option<Task>, String> {
  let mut store = lock(&state);
  store.ensure_writable()?;
  let restored = store.undo_last()?;
  let id = restored.clone().map(|task| task.id.clone()).unwrap_or_default();
  save_and_emit(&mut store, &app, &id)?;
  Ok(restored)
}

#[tauri::command]
pub fn restore_task(
  app: AppHandle,
  state: State<'_, Shared>,
  id: String,
) -> Result<Task, String> {
  let mut store = lock(&state);
  store.ensure_writable()?;
  let restored = store.restore_task(&id)?;
  save_and_emit(&mut store, &app, &id)?;
  Ok(restored)
}

/// 彻底清除回收站里的任务（不可撤销）
#[tauri::command]
pub fn purge_task(
  app: AppHandle,
  state: State<'_, Shared>,
  id: String,
) -> Result<Task, String> {
  let mut store = lock(&state);
  store.ensure_writable()?;
  let removed = store.purge_task(&id)?;
  save_and_emit(&mut store, &app, &id)?;
  Ok(removed)
}

// ---------------------------------------------------------------- 子任务

#[tauri::command]
pub fn add_subtask(
  app: AppHandle,
  state: State<'_, Shared>,
  task_id: String,
  title: String,
) -> Result<Task, String> {
  if title.trim().is_empty() {
    return Err("子任务名不能为空".to_string());
  }
  let mut store = lock(&state);
  store.ensure_writable()?;
  let index = store.find_index(&task_id).ok_or_else(task_error)?;
  store.data.tasks[index].subtasks.push(Subtask {
    id: new_id("s"),
    title: title.trim().to_string(),
    done: false,
  });
  store.data.tasks[index].updated_at = now_text();
  let updated = store.data.tasks[index].clone();
  save_and_emit(&mut store, &app, &task_id)?;
  Ok(updated)
}

#[tauri::command]
pub fn toggle_subtask(
  app: AppHandle,
  state: State<'_, Shared>,
  task_id: String,
  subtask_id: String,
) -> Result<Task, String> {
  let mut store = lock(&state);
  store.ensure_writable()?;
  let updated = store.toggle_subtask(&task_id, &subtask_id)?;
  save_and_emit(&mut store, &app, &task_id)?;
  Ok(updated)
}

#[tauri::command]
pub fn delete_subtask(
  app: AppHandle,
  state: State<'_, Shared>,
  task_id: String,
  subtask_id: String,
) -> Result<Task, String> {
  let mut store = lock(&state);
  store.ensure_writable()?;
  let index = store.find_index(&task_id).ok_or_else(task_error)?;
  let position = store.data.tasks[index]
    .subtasks
    .iter()
    .position(|item| item.id == subtask_id)
    .ok_or_else(|| "找不到这个子任务".to_string())?;
  let removed = store.data.tasks[index].subtasks.remove(position);
  store.data.tasks[index].updated_at = now_text();
  store.push_undo(Undoable::Subtask {
    task_id: task_id.clone(),
    subtask: removed,
  });
  let updated = store.data.tasks[index].clone();
  save_and_emit(&mut store, &app, &task_id)?;
  Ok(updated)
}

// ---------------------------------------------------------------- 协作备注

#[tauri::command]
pub fn add_note(
  app: AppHandle,
  state: State<'_, Shared>,
  task_id: String,
  note: NotePayload,
) -> Result<Task, String> {
  let content = note.content.unwrap_or_default();
  if content.trim().is_empty() {
    return Err("备注内容不能为空".to_string());
  }
  let mut store = lock(&state);
  store.ensure_writable()?;
  let index = store.find_index(&task_id).ok_or_else(task_error)?;
  let author = note.author.unwrap_or_else(|| "我".to_string());
  let created_at = note
    .created_at
    .unwrap_or_default()
    .as_str()
    .to_string();
  let stamp = match normalize_datetime(&created_at) {
    Some(value) => value,
    None => now_text(),
  };
  store.data.tasks[index].notes.push(Note {
    id: new_id("n"),
    author: if author.trim().is_empty() {
      "我".to_string()
    } else {
      author.trim().to_string()
    },
    content: content.trim().to_string(),
    created_at: stamp,
  });
  store.data.tasks[index].updated_at = now_text();
  let updated = store.data.tasks[index].clone();
  save_and_emit(&mut store, &app, &task_id)?;
  Ok(updated)
}

#[tauri::command]
pub fn delete_note(
  app: AppHandle,
  state: State<'_, Shared>,
  task_id: String,
  note_id: String,
) -> Result<Task, String> {
  let mut store = lock(&state);
  store.ensure_writable()?;
  let index = store.find_index(&task_id).ok_or_else(task_error)?;
  let position = store.data.tasks[index]
    .notes
    .iter()
    .position(|item| item.id == note_id)
    .ok_or_else(|| "找不到这条备注".to_string())?;
  let removed = store.data.tasks[index].notes.remove(position);
  store.data.tasks[index].updated_at = now_text();
  store.push_undo(Undoable::Note {
    task_id: task_id.clone(),
    note: removed,
  });
  let updated = store.data.tasks[index].clone();
  save_and_emit(&mut store, &app, &task_id)?;
  Ok(updated)
}

// ---------------------------------------------------------------- 提醒

#[tauri::command]
pub fn add_reminder(
  app: AppHandle,
  state: State<'_, Shared>,
  reminder: ReminderPayload,
) -> Result<Reminder, String> {
  let title = reminder.title.clone().unwrap_or_default();
  if title.trim().is_empty() {
    return Err("提醒名不能为空".to_string());
  }
  let raw_time = reminder.time.clone().unwrap_or_default();
  if normalize_datetime(&raw_time).is_none() && clocks_of(&raw_time).is_empty() {
    return Err("提醒时间没设好，请重新选一次时间".to_string());
  }
  let mut store = lock(&state);
  store.ensure_writable()?;
  let mut item = Reminder {
    id: new_id("r"),
    title: String::new(),
    time: String::new(),
    ..Default::default()
  };
  Store::apply_reminder_patch(&mut item, &reminder)?;
  store.data.reminders.push(item.clone());
  save_and_emit(&mut store, &app, &item.id.clone())?;
  Ok(item)
}

#[tauri::command]
pub fn update_reminder(
  app: AppHandle,
  state: State<'_, Shared>,
  id: String,
  patch: ReminderPayload,
) -> Result<Reminder, String> {
  let mut store = lock(&state);
  store.ensure_writable()?;
  // 重新启用自动置未完成的互逆规则在 app/reminders
  let next = crate::app::reminders::apply_patch(&mut store, &id, &patch)?;
  save_and_emit(&mut store, &app, &id)?;
  Ok(next)
}

#[tauri::command]
pub fn delete_reminder(
  app: AppHandle,
  state: State<'_, Shared>,
  id: String,
) -> Result<(), String> {
  let mut store = lock(&state);
  store.ensure_writable()?;
  // 删除并回收自己的 fired 键（键格式唯一生产者，规则在 app/reminders）
  crate::app::reminders::delete(&mut store, &id)?;
  save_and_emit(&mut store, &app, &id)?;
  Ok(())
}

#[tauri::command]
pub fn toggle_reminder(
  app: AppHandle,
  state: State<'_, Shared>,
  id: String,
) -> Result<Reminder, String> {
  let mut store = lock(&state);
  store.ensure_writable()?;
  // enabled/completed 互逆规则在 app/reminders
  let updated = crate::app::reminders::toggle(&mut store, &id)?;
  save_and_emit(&mut store, &app, &id)?;
  Ok(updated)
}

#[tauri::command]
pub fn snooze_reminder(
  app: AppHandle,
  state: State<'_, Shared>,
  id: String,
  minutes: u32,
) -> Result<Reminder, String> {
  let mut store = lock(&state);
  store.ensure_writable()?;
  // 1..=720 边界规则在 app/reminders；时刻 = 当前 + minutes
  let updated = crate::app::reminders::snooze(&mut store, &id, minutes, crate::models::now())?;
  save_and_emit(&mut store, &app, &id)?;
  Ok(updated)
}

/// 同列表手动排序（便签规格 §12.2）：按传入顺序整表落 sort_order。
#[tauri::command]
pub fn reorder_tasks(
  app: AppHandle,
  state: State<'_, Shared>,
  ids_in_order: Vec<String>,
) -> Result<usize, String> {
  let mut store = lock(&state);
  store.ensure_writable()?;
  let moved = store.reorder_tasks(&ids_in_order)?;
  save_and_emit(&mut store, &app, "order")?;
  Ok(moved)
}

// ---------------------------------------------------------------- 设置

#[tauri::command]
pub fn set_settings(
  app: AppHandle,
  state: State<'_, Shared>,
  patch: SettingsPayload,
) -> Result<Settings, String> {
  let mut store = lock(&state);
  store.ensure_writable()?;
  let old_theme = store.data.settings.theme.clone();
  let old_reading_theme = store.data.settings.reading_theme.clone();
  // 四槽位热键事务：任一换绑失败，主题/键位整批回滚（规则在 app/settings）
  let mut hotkeys = crate::infra::tauri_hotkeys::TauriHotkeys::new(&app);
  crate::app::settings::apply_with_hotkeys(&mut store, &patch, &mut hotkeys)?;
  if let Some(on) = patch.telemetry_enabled {
    // 埋点开关即时生效（此前只在启动时读一次——补 planned-settings B.5★ 的实效性）
    crate::telemetry::set_enabled(on);
  }
  let next_theme = store.data.settings.theme.clone();
  let next_pinned = store.data.settings.todo_float_pinned;
  store.save()?;
  if next_theme != old_theme {
    let _ = app.emit_all("theme-changed", next_theme.clone());
  }
  let next_reading_theme = store.data.settings.reading_theme.clone();
  if patch.reading_theme.is_some() && next_reading_theme != old_reading_theme {
    // 阅读主题换装：KB 阅读区与本窗设置页各自重放（同一窗口内也可靠 store-changed 兜底）
    let _ = app.emit_all("reading-theme-changed", next_reading_theme.clone());
  }
  if patch.todo_float_pinned.is_some() {
    // 待办悬浮窗置顶即时同步到窗口（todo-float）
    runtime::apply_todo_float_pinned(&app, next_pinned);
  }
  emit(&app, StoreEvent::changed("settings"));
  Ok(store.data.settings.clone())
}

/// 清空本地统计（planned-settings-ui-spec B.5★，二次确认在前端）：重置 events.jsonl。
#[tauri::command]
pub fn clear_events() -> Result<(), String> {
  crate::telemetry::clear_events()
}

// ---------------------------------------------------------------- 阅读面字体（reading-font）

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingFontInfo {
  pub name: String,
  pub size_kb: u32,
}

fn reading_fonts() -> Vec<ReadingFontInfo> {
  crate::infra::fonts::list()
    .into_iter()
    .map(|(name, size_kb)| ReadingFontInfo { name, size_kb })
    .collect()
}

/// 列出数据目录 `<fonts>/` 下已安装的阅读字体。
#[tauri::command]
pub fn list_reading_fonts() -> Vec<ReadingFontInfo> {
  reading_fonts()
}

/// 安装用户上传的阅读字体（前端 `<input type=file>` 读成 base64 传来）。
/// 校验扩展名与文件头，先写临时文件再 rename，半个文件不会被读到。
#[tauri::command]
pub fn save_reading_font(name: String, data: String) -> Result<Vec<ReadingFontInfo>, String> {
  let bytes = crate::infra::text_codec::decode(&data)?;
  crate::infra::fonts::install(&name, &bytes)?;
  Ok(reading_fonts())
}

/// 删除一个阅读字体文件（前端负责先切回默认再调）。
#[tauri::command]
pub fn delete_reading_font(name: String) -> Result<Vec<ReadingFontInfo>, String> {
  crate::infra::fonts::remove(&name)?;
  Ok(reading_fonts())
}

/// 读一个字体文件（base64）；启动时前端据此用 `FontFace` 注册当前阅读字体。
#[tauri::command]
pub fn read_reading_font(name: String) -> Result<String, String> {
  crate::infra::fonts::read_base64(&name)
}

// ---------------------------------------------------------------- 阅读主题（reading-themes）

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingThemeInfo {
  pub name: String,
  pub size_kb: u32,
}

fn reading_themes() -> Vec<ReadingThemeInfo> {
  crate::infra::themes::list()
    .into_iter()
    .map(|(name, size_kb)| ReadingThemeInfo { name, size_kb })
    .collect()
}

/// 列出数据目录 `<themes>/` 下已安装的阅读主题文件。
#[tauri::command]
pub fn list_reading_themes() -> Vec<ReadingThemeInfo> {
  reading_themes()
}

/// 安装用户导入的主题（前端读成 UTF-8 文本传来）。
/// 校验文件名与外部资源，先写临时文件再 rename，半个文件不会被读到。
#[tauri::command]
pub fn save_reading_theme(name: String, data: String) -> Result<Vec<ReadingThemeInfo>, String> {
  crate::infra::themes::install(&name, &data)?;
  Ok(reading_themes())
}

/// 删除一个主题文件（前端负责先切回默认再调）。
#[tauri::command]
pub fn delete_reading_theme(name: String) -> Result<Vec<ReadingThemeInfo>, String> {
  crate::infra::themes::remove(&name)?;
  Ok(reading_themes())
}

/// 读一个主题文件的文本；前端解析后应用到阅读区。
#[tauri::command]
pub fn read_reading_theme(name: String) -> Result<String, String> {
  crate::infra::themes::read(&name)
}

// ---------------------------------------------------------------- 便签（多便签 H1 · multi-sticky-spec）

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StickySelf {
  pub id: String,
  pub content: String,
  pub mini: bool,
  /// Phase 2：知识便签视口指针（None = 自由便签）
  #[serde(skip_serializing_if = "Option::is_none")]
  pub kb_ref: Option<String>,
}

/// 新建一张自由便签（sticky-separation：便签只有这一种）。
/// async：建窗可能被 WebView2 层无限挂起（真机复盘 2026-09-08），绝不能在事件循环
/// 线程上执行；建窗逻辑全在 runtime::create_note，与全局热键共用同一入口。
#[tauri::command]
pub async fn create_sticky(
  app: AppHandle,
  kb_ref: Option<String>,
) -> Result<StickyNote, String> {
  runtime::create_note(&app, kb_ref)
}

#[tauri::command]
pub fn list_stickies(state: State<'_, Shared>) -> Result<Vec<StickyNote>, String> {
  let store = lock(&state);
  Ok(store.stickies())
}

/// 便签自述：前端窗启动时调一次，拿到 身份/内容/形态（缩小悬浮条 or 展开纸片）。
#[tauri::command]
pub fn sticky_self(
  window: tauri::Window,
  state: State<'_, Shared>,
) -> Result<StickySelf, String> {
  let label = window.label().to_string();
  let id = label
    .strip_prefix("note:")
    .ok_or_else(|| "不是便签窗口".to_string())?
    .to_string();
  let store = lock(&state);
  let note = store
    .find_sticky(&id)
    .ok_or_else(|| "找不到这张便签".to_string())?;
  Ok(StickySelf {
    id,
    content: note.content,
    mini: note.mini,
    kb_ref: note.kb_ref,
  })
}

/// 更新便签：内容（≤500 字）/ 形态（缩小悬浮条 ↔ 展开纸片，联动窗口 set_size）/
/// 可见性（fix-known-defects 4.2–4.4）。
/// `visible = Some(false)` = 关闭窗（✕ 同款）：只置 false + 落盘 + 藏窗，不删数据、不清位置；
/// `visible = Some(true)` = 重新打开：与 open_sticky 同款上限逻辑（可见合计 ≥5 拒绝）。
#[tauri::command]
pub fn update_sticky(
  app: AppHandle,
  state: State<'_, Shared>,
  id: String,
  content: Option<String>,
  mini: Option<bool>,
  visible: Option<bool>,
) -> Result<StickyNote, String> {
  let mut store = lock(&state);
  store.ensure_writable()?;
  let was_visible = store
    .find_sticky(&id)
    .map(|note| note.visible)
    .ok_or_else(|| "找不到这张便签".to_string())?;
  if visible == Some(true) && !was_visible {
    // 重新打开：可见合计上限（隐藏项不计；升级首启的 6 张旧数据不强制隐藏，
    // 但已达 5 张可见时不允许再把隐藏项开回来）
    if crate::models::visible_sticky_count(&store.data.stickies) >= crate::models::STICKY_CAP {
      return Err(format!(
        "便签最多同时打开 {} 张，请先关闭一张",
        crate::models::STICKY_CAP
      ));
    }
  }
  {
    let note = store
      .find_sticky_mut(&id)
      .ok_or_else(|| "找不到这张便签，可能已经被关闭".to_string())?;
    if let Some(c) = &content {
      let c = c.trim();
      if c.chars().count() > 500 {
        return Err("便签最多 500 字".to_string());
      }
      note.content = c.to_string();
    }
    if let Some(m) = mini {
      note.mini = m;
    }
    if let Some(v) = visible {
      note.visible = v;
    }
    note.updated_at = now_text();
  }
  let stored_pos = store.runtime.note_pos.get(&id).copied();
  let note = store
    .find_sticky(&id)
    .ok_or_else(|| "找不到这张便签".to_string())?;
  store.save()?;
  // 窗口操作前放锁（v2.2 死锁修复的铁律）
  drop(store);

  if visible == Some(true) {
    // 打开同款逻辑：先确保窗口存在并显示；建窗失败回滚 visible=false
    if let Err(error) = runtime::open_note_window(&app, &id, note.mini, stored_pos) {
      if let Ok(mut store) = state.lock() {
        if let Some(note) = store.find_sticky_mut(&id) {
          note.visible = false;
        }
        let _ = store.save();
      }
      return Err(error);
    }
    if let Some(window) = app.get_window(&runtime::note_label(&id)) {
      let _ = window.show();
      let _ = window.set_focus();
    }
    // 同一次调用里既重开又改形态：窗口可能已存在，补一次 set_size
    if let Some(m) = mini {
      runtime::apply_note_shape(&app, &id, m)?;
    }
  } else if !note.visible {
    // 关闭即隐藏：窗口只 hide，不销毁、不删数据、不清 note_pos
    runtime::hide_note_window(&app, &id);
    if was_visible {
      crate::telemetry::record_str("sticky_hide", &[]);
    }
  } else if let Some(m) = mini {
    runtime::apply_note_shape(&app, &id, m)?;
  }
  emit(&app, StoreEvent::changed("stickies"));
  Ok(note)
}

/// 重新打开一张便签（fix-known-defects 4.4，设置页便签列表入口）。
/// 已可见：确保窗口显示并聚焦；已隐藏：校验可见合计上限 → visible=true 落盘 → 建窗，
/// 建窗失败回滚 visible=false。async：WebView2 建窗可能挂起，不能占事件循环线程。
#[tauri::command]
pub async fn open_sticky(app: AppHandle, id: String) -> Result<StickyNote, String> {
  runtime::open_note(&app, &id)
}

/// 显式删除（sticky-separation）：便签数据、窗口与位置记录一并回收，无撤销；
/// 误删靠写前轮转备份兜底。普通关闭（✕/Alt+F4）已改为隐藏，不再走这里。
#[tauri::command]
pub fn delete_sticky(app: AppHandle, state: State<'_, Shared>, id: String) -> Result<(), String> {
  let mut store = lock(&state);
  store.ensure_writable()?;
  store
    .find_sticky(&id)
    .ok_or_else(|| "找不到这张便签，可能已经被关闭".to_string())?;
  store.delete_sticky(&id)?;
  store.save()?;
  store.save_runtime_only()?;
  if let Some(w) = app.get_window(&runtime::note_label(&id)) {
    let _ = w.close();
  }
  crate::telemetry::record_str("sticky_delete", &[]);
  emit(&app, StoreEvent::changed("stickies"));
  Ok(())
}

/// 通用前端埋点通道（metric 蓝图：weekly_open 等 UI 侧事件）。仅落本机 events.jsonl，绝不出网。
#[tauri::command]
pub fn track_event(name: String, props: Option<String>) -> Result<(), String> {
  let name = name.trim();
  if name.is_empty() || name.len() > 48 {
    return Err("事件名不合法".to_string());
  }
  match props {
    Some(json) if !json.trim().is_empty() => match serde_json::from_str::<serde_json::Value>(json.trim()) {
      Ok(v) => crate::telemetry::record(name, v),
      Err(_) => return Err("props 不是合法 JSON".to_string()),
    },
    _ => crate::telemetry::record_str(name, &[]),
  }
  Ok(())
}

// ---------------------------------------------------------------- 窗口 / 托盘

/// 打开主界面；route 可选（前端内部路由，如 /kb），缺省只显示不导航
#[tauri::command]
pub fn open_main_window(app: AppHandle, route: Option<String>) -> Result<(), String> {
  runtime::show_main(&app, normalize_route(route).as_deref())
}

/// 前端可选路由归一：trim，空串 / 纯空白 = 不导航
fn normalize_route(route: Option<String>) -> Option<String> {
  route
    .map(|value| value.trim().to_string())
    .filter(|value| !value.is_empty())
}

#[tauri::command]
pub fn open_capture_overlay(app: AppHandle) -> Result<(), String> {
  let r = runtime::open_capture_overlay(&app);
  if r.is_ok() {
    crate::telemetry::capture_open();
  }
  r
}

#[tauri::command]
pub fn close_capture_overlay(app: AppHandle) -> Result<(), String> {
  runtime::close_capture_overlay(&app)
}

/// 前端拖拽把手 mousedown 时调用，移动无边框快速记录条
#[tauri::command]
pub fn capture_start_drag(app: AppHandle) -> Result<(), String> {
  runtime::capture_start_drag(&app)
}

/// 前端 ResizeObserver 上报内容高度，动态改快速记录条窗口高度（顶部锚定向下伸缩）
#[tauri::command]
pub fn capture_resize(app: AppHandle, height: f64) -> Result<(), String> {
  runtime::capture_resize(&app, height)
}

/// 换绑捕获热键；失败返回「快捷键被占用，请用备用入口」并保持旧值
#[tauri::command]
pub fn register_capture_hotkey(
  app: AppHandle,
  state: State<'_, Shared>,
  combo: String,
) -> Result<Settings, String> {
  let trimmed = combo.trim().to_string();
  if trimmed.is_empty() {
    return Err("快捷键不能为空".to_string());
  }
  let mut store = lock(&state);
  let previous = store.data.settings.capture_hotkey.clone();
  runtime::register_capture_hotkey(&app, &trimmed)?;
  if previous == trimmed {
    return Ok(store.data.settings.clone());
  }
  store.ensure_writable()?;
  store.data.settings.capture_hotkey = trimmed.clone();
  if let Err(error) = store.save() {
    store.data.settings.capture_hotkey = previous.clone();
    let _ = runtime::register_capture_hotkey(&app, &previous);
    return Err(error);
  }
  emit(&app, StoreEvent::changed("settings"));
  Ok(store.data.settings.clone())
}

/// 换绑/解绑「打开主界面」热键；空串 = 解绑；失败保持旧值
#[tauri::command]
pub fn register_main_hotkey(
  app: AppHandle,
  state: State<'_, Shared>,
  combo: String,
) -> Result<Settings, String> {
  let trimmed = combo.trim().to_string();
  let next = if trimmed.is_empty() {
    None
  } else {
    Some(trimmed)
  };
  let mut store = lock(&state);
  let previous = store.data.settings.main_hotkey.clone();
  runtime::register_main_hotkey(&app, &next)?;
  if previous == next {
    return Ok(store.data.settings.clone());
  }
  store.ensure_writable()?;
  store.data.settings.main_hotkey = next.clone();
  if let Err(error) = store.save() {
    store.data.settings.main_hotkey = previous.clone();
    let _ = runtime::register_main_hotkey(&app, &previous);
    return Err(error);
  }
  emit(&app, StoreEvent::changed("settings"));
  Ok(store.data.settings.clone())
}

/// 换绑/解绑「新建便签」热键；空串 = 解绑；失败保持旧值（sticky-separation）
#[tauri::command]
pub fn register_sticky_hotkey(
  app: AppHandle,
  state: State<'_, Shared>,
  combo: String,
) -> Result<Settings, String> {
  let trimmed = combo.trim().to_string();
  let next = if trimmed.is_empty() {
    None
  } else {
    Some(trimmed)
  };
  let mut store = lock(&state);
  let previous = store.data.settings.sticky_hotkey.clone();
  runtime::register_sticky_hotkey(&app, &next)?;
  if previous == next {
    return Ok(store.data.settings.clone());
  }
  emit(&app, StoreEvent::changed("settings"));
  Ok(store.data.settings.clone())
}

/// 换绑/解绑「待办悬浮窗」呼出热键；空串 = 解绑；失败保持旧值（todo-float）
#[tauri::command]
pub fn register_todo_hotkey(
  app: AppHandle,
  state: State<'_, Shared>,
  combo: String,
) -> Result<Settings, String> {
  let trimmed = combo.trim().to_string();
  let next = if trimmed.is_empty() {
    None
  } else {
    Some(trimmed)
  };
  let mut store = lock(&state);
  let previous = store.data.settings.todo_hotkey.clone();
  runtime::register_todo_hotkey(&app, &next)?;
  if previous == next {
    return Ok(store.data.settings.clone());
  }
  store.ensure_writable()?;
  store.data.settings.todo_hotkey = next.clone();
  if let Err(error) = store.save() {
    store.data.settings.todo_hotkey = previous.clone();
    let _ = runtime::register_todo_hotkey(&app, &previous);
    return Err(error);
  }
  emit(&app, StoreEvent::changed("settings"));
  Ok(store.data.settings.clone())
}

/// 换绑/解绑「直达知识库」热键；空串 = 解绑；失败保持旧值（kb-storage-backup S4）
#[tauri::command]
pub fn register_kb_hotkey(
  app: AppHandle,
  state: State<'_, Shared>,
  combo: String,
) -> Result<Settings, String> {
  let trimmed = combo.trim().to_string();
  let next = if trimmed.is_empty() {
    None
  } else {
    Some(trimmed)
  };
  let mut store = lock(&state);
  let previous = store.data.settings.kb_hotkey.clone();
  runtime::register_kb_hotkey(&app, &next)?;
  if previous == next {
    return Ok(store.data.settings.clone());
  }
  store.ensure_writable()?;
  store.data.settings.kb_hotkey = next.clone();
  if let Err(error) = store.save() {
    store.data.settings.kb_hotkey = previous.clone();
    let _ = runtime::register_kb_hotkey(&app, &previous);
    return Err(error);
  }
  emit(&app, StoreEvent::changed("settings"));
  Ok(store.data.settings.clone())
}

/// 隐藏待办悬浮窗（✕ 同款）：只藏窗，显隐状态落盘，重启按此恢复
#[tauri::command]
pub fn hide_todo_float(app: AppHandle) -> Result<(), String> {
  runtime::hide_todo_float(&app)
}

/// 待办悬浮窗最小化（fix-known-defects 4.7）：写 runtime.json 后切窗尺寸
/// （最小化 380×40 / 完整 380×456）。sync：只 set_size，不建窗。
#[tauri::command]
pub fn set_todo_float_mini(
  app: AppHandle,
  _state: State<'_, Shared>,
  mini: bool,
) -> Result<(), String> {
  runtime::set_todo_float_mini(&app, mini)
}

/// 主窗 ✕ = 收进托盘（A3 托盘常驻）。必须是 hide：前端 appWindow.close() 在 tauri 1.8.3 走
/// `on_window_close` → 丢弃 tao Window → DestroyWindow，全程不产生 CloseRequested，
/// main.rs 里的 prevent_close 无从拦截 —— 主窗一旦被销毁，这个进程就再也呼不出它了。
#[tauri::command]
pub fn hide_main_window(app: AppHandle) -> Result<(), String> {
  runtime::hide_window(&app, runtime::MAIN_LABEL)
}

#[tauri::command]
pub fn open_data_folder() -> Result<(), String> {
  runtime::open_in_folder(&crate::store::data_dir())
}

// ---------------------------------------------------------------- 恢复 / 导出 / 迁移报告

#[tauri::command]
pub fn restore_backup(
  app: AppHandle,
  state: State<'_, Shared>,
  slot: String,
) -> Result<RestoreResult, String> {
  let mut store = lock(&state);
  let restored = store.restore_backup(&slot)?;
  let health = store.health_view();
  let hotkey = store.data.settings.capture_hotkey.clone();
  let main_hotkey = store.data.settings.main_hotkey.clone();
  drop(store);
  let _ = runtime::register_capture_hotkey(&app, &hotkey);
  let _ = runtime::register_main_hotkey(&app, &main_hotkey);
  emit(&app, StoreEvent::changed("data"));
  Ok(RestoreResult { restored, health })
}

/// 前端展示完迁移报告后清账，避免每次启动重复提示
#[tauri::command]
pub fn clear_migration_report(
  app: AppHandle,
  state: State<'_, Shared>,
) -> Result<Option<MigrationReport>, String> {
  let mut store = lock(&state);
  let report = store.runtime.migration.clone();
  store.clear_migration();
  if store.ensure_writable().is_ok() && store.save().is_ok() {
    emit(&app, StoreEvent::changed("data"));
  }
  Ok(report)
}

// ---------------------------------------------------------------- 手动备份（D3 / D4）

/// 手动导出单文件备份到 `<数据目录>/backups/*.kettdbackup`（design D3）。
///
/// **偏离原设计**：design D3 要求 async + spawn_blocking；本批沿用仓库现有同步命令风格，
/// 锁在文件 I/O 期间短暂持有（单文件写盘，资源仅正文引用到的图片）。真机若出现主线程卡顿，
/// 再改 async + spawn_blocking（TODO 见交付报告）。
#[tauri::command]
pub fn export_backup(
  _app: AppHandle,
  state: State<'_, Shared>,
) -> Result<backup::ExportResult, String> {
  let store = lock(&state);
  store.export_backup_to_file(env!("CARGO_PKG_VERSION"), &now_text())
}

/// 从备份文本导入（design D4）；`dry_run = true` 只解析 + 预览，绝不写盘。
/// 同步命令与锁内文件 I/O 的偏离说明同 `export_backup`。
#[tauri::command]
pub fn import_backup(
  app: AppHandle,
  state: State<'_, Shared>,
  payload: String,
  dry_run: bool,
) -> Result<backup::ImportReport, String> {
  let mut store = lock(&state);
  let report = store.import_backup_from_payload(&payload, dry_run, &now_text())?;
  drop(store);
  if !dry_run {
    emit(&app, StoreEvent::changed("data"));
    emit(&app, StoreEvent::changed("kb"));
  }
  Ok(report)
}

// ---------------------------------------------------------------- 知识库（frame H1b：库 + 搜索 + 任务单向引用）

/// 知识条目列表（D1）：缺省只回元数据（bodyMd 置空串，其余字段完整，降低 IPC 体积）；
/// 传 `ids` 时按给定顺序批量返回完整条目（含正文），不存在的 id 忽略。
#[tauri::command]
pub fn get_kb_items(
  state: State<'_, Shared>,
  ids: Option<Vec<String>>,
) -> Result<Vec<KbItem>, String> {
  let store = lock(&state);
  store.ensure_kb_ok()?;
  match ids {
    None => Ok(store.kb.iter().map(kb_metadata_only).collect()),
    Some(ids) => {
      let mut out: Vec<KbItem> = Vec::with_capacity(ids.len());
      for id in ids {
        if let Some(item) = store.kb.iter().find(|item| item.id == id) {
          out.push(item.clone());
        }
      }
      Ok(out)
    }
  }
}

/// 列表 IPC 的元数据形态：正文置空串，其余字段完整（正文经 get_kb_items(ids) 按需补取）
fn kb_metadata_only(item: &KbItem) -> KbItem {
  KbItem {
    body_md: String::new(),
    ..item.clone()
  }
}

#[tauri::command]
pub fn add_kb_item(
  app: AppHandle,
  state: State<'_, Shared>,
  args: KbPayload,
) -> Result<KbItem, String> {
  let mut store = lock(&state);
  store.ensure_writable()?;
  store.ensure_kb_ok()?;
  // 先校验 folderId（失败时内存没有半截新建条目）
  let folder = store.folder_target(args.folder_id.as_deref())?;
  let created = crate::kb::create(
    &mut store.kb,
    new_id("k"),
    args.title.as_deref().unwrap_or(""),
    args.body_md.as_deref().unwrap_or(""),
    args.tags.unwrap_or_default(),
    &now_text(),
  )?;
  // D1：先写正文（notes/<id>.md），再 place + 写索引；正文失败则命令整体失败
  store.save_kb_item_body(&created.id, &created.body_md)?;
  // 新条目追加到目标父级末尾（i64::MAX 由 place_kb_item clamp 成同级数量）
  let item = store.place_kb_item(&created.id, Some(folder), Some(i64::MAX), None)?;
  store.save_notes()?;
  drop(store);
  emit(&app, StoreEvent::changed("kb"));
  Ok(item)
}

#[tauri::command]
pub fn update_kb_item(
  app: AppHandle,
  state: State<'_, Shared>,
  id: String,
  patch: KbPayload,
) -> Result<KbItem, String> {
  let mut store = lock(&state);
  store.ensure_writable()?;
  store.ensure_kb_ok()?;
  // None = 不改；先校验目标文件夹，避免 kb::update 已改内存后才发现 folderId 非法
  let folder = if patch.folder_id.is_some() {
    Some(store.folder_target(patch.folder_id.as_deref())?)
  } else {
    None
  };
  crate::kb::update(
    &mut store.kb,
    &id,
    patch.title.as_deref(),
    patch.body_md.as_deref(),
    patch.tags,
    &now_text(),
  )?;
  // D1：先写正文再落索引（update 只改内存；正文文件是 IPC 正文的唯一来源）
  let body = store
    .kb
    .iter()
    .find(|item| item.id == id)
    .map(|item| item.body_md.clone())
    .ok_or_else(|| "找不到这条资料，可能已经被删除".to_string())?;
  store.save_kb_item_body(&id, &body)?;
  // order/pinned 与 folderId 一并落位：改变归属或顺序时对旧/新父级 dense 重排
  let item = store.place_kb_item(&id, folder, patch.order, patch.pinned)?;
  store.save_notes()?;
  drop(store);
  emit(&app, StoreEvent::changed("kb"));
  Ok(item)
}

/// 删除条目；任务侧的 kbRefs 会悬空 —— 前端据此展示「已失效」
#[tauri::command]
pub fn delete_kb_item(
  app: AppHandle,
  state: State<'_, Shared>,
  id: String,
) -> Result<(), String> {
  let mut store = lock(&state);
  store.ensure_writable()?;
  store.ensure_kb_ok()?;
  crate::kb::delete(&mut store.kb, &id)?;
  // D1：先删正文再写索引（顺序保证索引不会指回缺失正文）
  store.delete_kb_item_body(&id)?;
  store.save_notes()?;
  emit(&app, StoreEvent::changed("kb"));
  Ok(())
}

/// 内存检索（kb-storage-backup S4 收窄）：只匹配标题 + 全部祖先文件夹名，
/// 返回按相关度排序的 id 数组；空查询 = 全量按更新时间倒序。
/// 正文 / 标签不参与匹配（标签筛选由前端独立过滤，二者互不影响）。
#[tauri::command]
pub fn search_kb(state: State<'_, Shared>, query: Option<String>) -> Result<Vec<String>, String> {
  let store = lock(&state);
  store.ensure_kb_ok()?;
  Ok(crate::kb::search(
    &store.kb,
    &store.folders,
    query.as_deref().unwrap_or(""),
  ))
}

// ---------------------------------------------------------------- 知识库文件夹（D6）/ notes 备份恢复（D8）

/// 文件夹全量（前端按 parentId 组树；顺序按后端先后，前端可再用 order 排序）
#[tauri::command]
pub fn list_folders(state: State<'_, Shared>) -> Result<Vec<KbFolder>, String> {
  let store = lock(&state);
  store.ensure_kb_ok()?;
  Ok(store.list_folders())
}

#[tauri::command]
pub fn create_folder(
  app: AppHandle,
  state: State<'_, Shared>,
  name: String,
  parent_id: Option<String>,
) -> Result<KbFolder, String> {
  let mut store = lock(&state);
  store.ensure_writable()?;
  store.ensure_kb_ok()?;
  let folder = store.create_folder(&name, parent_id.as_deref())?;
  store.save_folders()?;
  drop(store);
  emit(&app, StoreEvent::changed("kb"));
  Ok(folder)
}

/// 文件夹更新（D4）：改名 / 移动 / 同级排序 / 置顶统一入口。
/// parentId 传 "" 表示移到顶层（根），缺省表示不改；order 缺省且发生移动时追加末尾。
#[tauri::command]
pub fn update_folder(
  app: AppHandle,
  state: State<'_, Shared>,
  id: String,
  name: Option<String>,
  parent_id: Option<String>,
  order: Option<i64>,
  pinned: Option<bool>,
) -> Result<KbFolder, String> {
  let mut store = lock(&state);
  store.ensure_writable()?;
  store.ensure_kb_ok()?;
  let folder = store.update_folder(&id, name, parent_id, order, pinned)?;
  store.save_folders()?;
  drop(store);
  emit(&app, StoreEvent::changed("kb"));
  Ok(folder)
}

#[tauri::command]
pub fn delete_folder(
  app: AppHandle,
  state: State<'_, Shared>,
  id: String,
) -> Result<(), String> {
  let mut store = lock(&state);
  store.ensure_writable()?;
  store.ensure_kb_ok()?;
  store.delete_folder(&id)?;
  store.save_folders()?;
  drop(store);
  emit(&app, StoreEvent::changed("kb"));
  Ok(())
}

/// notes.json.bak-1..5 备份列表（读取不做损坏封锁：恢复页必须能列出来）
#[tauri::command]
pub fn list_kb_backups(state: State<'_, Shared>) -> Result<Vec<KbBackupInfo>, String> {
  let store = lock(&state);
  Ok(store.list_kb_backups())
}

/// 从 notes 备份恢复；成功后返回恢复的条目数
#[tauri::command]
pub fn restore_kb_backup(
  app: AppHandle,
  state: State<'_, Shared>,
  slot: String,
) -> Result<RestoreKbResult, String> {
  let mut store = lock(&state);
  let restored = store.restore_kb_backup(&slot)?;
  drop(store);
  emit(&app, StoreEvent::changed("kb"));
  Ok(RestoreKbResult { restored })
}

/// 调试入口（不进 UI）：回滚 schema 拆分，重启后生效
#[tauri::command]
pub fn rollback_schema_split(state: State<'_, Shared>) -> Result<String, String> {
  let mut store = lock(&state);
  store.rollback_schema_split()?;
  Ok("已回滚到拆分前的单文件存储，重启后生效".to_string())
}

/// 一键导出本周汇总：week = current | last | YYYY-Www；format = md | csv
#[tauri::command]
pub fn export_weekly(
  state: State<'_, Shared>,
  week: Option<String>,
  format: Option<String>,
  dir: Option<String>,
) -> Result<String, String> {
  let store = lock(&state);
  store.ensure_writable()?;
  let settings = store.data.settings.clone();
  let tasks = store.data.tasks.clone();
  drop(store);
  let fmt = format.clone();
  let out = export::write_weekly(&settings, &tasks, week, format, dir);
  if let Ok(path) = &out {
    // 仅 md 末尾追加本地度量快照（csv 不污染格式）；纯本机读 events.jsonl 现算
    if path.ends_with(".md") {
      let snap = crate::telemetry::render_snapshot(&crate::telemetry::load_recent(
        &crate::store::data_dir(),
        8000,
      ));
      if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(path) {
        use std::io::Write as _;
        let _ = f.write_all(snap.as_bytes());
      }
    }
    crate::telemetry::record_str("weekly_export", &[("fmt", fmt.as_deref().unwrap_or("md"))]);
  }
  out
}

// ---------------------------------------------------------------- patch 内时间字段再归一

/// 表单常量（分类 / 优先级 / 悬浮形态 / 保留天数 / 默认热键），避免前端硬编码
#[tauri::command]
pub fn get_form_hints() -> Result<serde_json::Value, String> {
  Ok(serde_json::json!({
    "categories": CATEGORIES,
    "priorities": PRIORITIES,
    "sources": ["capture", "manual", "seed"],
    "repeat": ["none", "daily", "workdays", "weekly"],
    "defaultHotkey": DEFAULT_HOTKEY,
    "defaultCap": REMINDER_CAP_DEFAULT,
    "retentionDays": TRASH_RETENTION_DAYS,
    "today": today_text(),
  }))
}

fn today_text() -> String {
  crate::models::fmt_day(&today())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn normalize_route_keeps_path_and_drops_blank() {
    assert_eq!(
      normalize_route(Some("/kb".to_string())).as_deref(),
      Some("/kb"),
      "open_main_window 的 route 透传给主窗（kb-storage-backup S4）"
    );
    assert_eq!(
      normalize_route(Some("  /kb  ".to_string())).as_deref(),
      Some("/kb"),
      "首尾空白要裁掉"
    );
    assert_eq!(normalize_route(Some("   ".to_string())), None);
    assert_eq!(normalize_route(None), None);
  }
}

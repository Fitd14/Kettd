//! 所有对前端的命令（invoke 参数 camelCase；返回 Result<_, String>，Err 为可显示的中文短句）

use crate::export;
use crate::models::{
  carry_days, clocks_of, fmt_dt, is_valid_choice, new_id, normalize_datetime, now_text,
  Bootstrap, CATEGORIES, DataHealthV2, DEFAULT_HOTKEY, FLOAT_FORMS,
  HotkeyStatus, MigrationReport, Note, NotePayload, PRIORITIES, REMINDER_CAP_DEFAULT, Reminder, ReminderPayload,
  RestoreResult, Settings, SettingsPayload, Source, StoreEvent, Subtask, Task, TaskPayload,
  TRASH_RETENTION_DAYS, today,
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

fn reminder_error() -> String {
  "找不到这条提醒，可能已经被删除".to_string()
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
    migration: store.data.migration.clone(),
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
  Ok(store.health_view())
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
  if args.title.as_deref().unwrap_or("").trim().is_empty() {
    return Err("标题不能为空".to_string());
  }
  let mut store = lock(&state);
  store.ensure_writable()?;
  let stamp = now_text();
  let mut task = Task {
    id: new_id("t"),
    created_at: stamp.clone(),
    updated_at: stamp,
    source: args.source.unwrap_or(Source::Manual),
    ..Default::default()
  };
  Store::apply_task_patch(&mut task, &args)?;
  task.carried_from = if task.done {
    0
  } else {
    carry_days(&task.due_at.clone())
  };
  store.data.tasks.push(task.clone());
  save_and_emit(&mut store, &app, &task.id.clone())?;
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
  let index = store.find_index(&id).ok_or_else(task_error)?;
  let mut next = store.data.tasks[index].clone();
  Store::apply_task_patch(&mut next, &patch)?;
  next.updated_at = now_text();
  next.carried_from = if next.done || next.in_trash() {
    next.carried_from
  } else {
    next.carried_from.max(carry_days(&next.due_at.clone()))
  };
  store.data.tasks[index] = next.clone();
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
  let index = store
    .find_reminder_index(&id)
    .ok_or_else(reminder_error)?;
  let mut next = store.data.reminders[index].clone();
  Store::apply_reminder_patch(&mut next, &patch)?;
  if Some(true) == patch.enabled {
    next.completed = false;
  }
  store.data.reminders[index] = next.clone();
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
  let index = store
    .find_reminder_index(&id)
    .ok_or_else(reminder_error)?;
  store.data.reminders.remove(index);
  let prefix = format!("r|{}|", id);
  store.data.fired.retain(|key| !key.starts_with(&prefix));
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
  let index = store
    .find_reminder_index(&id)
    .ok_or_else(reminder_error)?;
  let completed = store.data.reminders[index].completed;
  {
    let item = &mut store.data.reminders[index];
    item.completed = !completed;
    item.enabled = completed;
    if item.enabled {
      item.snoozed_until = None;
    }
  }
  let updated = store.data.reminders[index].clone();
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
  if minutes == 0 || minutes > 720 {
    return Err("稍后提醒只能设 1 到 720 分钟".to_string());
  }
  let mut store = lock(&state);
  store.ensure_writable()?;
  let index = store
    .find_reminder_index(&id)
    .ok_or_else(reminder_error)?;
  let target = fmt_dt(crate::models::now() + chrono::Duration::minutes(minutes as i64));
  {
    let item = &mut store.data.reminders[index];
    item.snoozed_until = Some(target);
    item.completed = false;
    item.enabled = true;
  }
  let updated = store.data.reminders[index].clone();
  save_and_emit(&mut store, &app, &id)?;
  Ok(updated)
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
  let old_form = store.data.settings.float_form.clone();
  let old_hotkey = store.data.settings.capture_hotkey.clone();
  let old_main_hotkey = store.data.settings.main_hotkey.clone();
  store.apply_settings_patch(&patch)?;
  let next_hotkey = store.data.settings.capture_hotkey.clone();
  if next_hotkey != old_hotkey && runtime::register_capture_hotkey(&app, &next_hotkey).is_err() {
    // 注册失败：整次回滚，避免界面热键与实际绑定不一致
    store.data.settings.capture_hotkey = old_hotkey;
    store.data.settings.main_hotkey = old_main_hotkey;
    store.data.settings.float_form = old_form;
    store.data.settings.theme = old_theme;
    return Err("快捷键被占用，请用备用入口".to_string());
  }
  let next_main_hotkey = store.data.settings.main_hotkey.clone();
  if next_main_hotkey != old_main_hotkey
    && runtime::register_main_hotkey(&app, &next_main_hotkey).is_err()
  {
    store.data.settings.capture_hotkey = old_hotkey.clone();
    store.data.settings.main_hotkey = old_main_hotkey;
    store.data.settings.float_form = old_form;
    store.data.settings.theme = old_theme;
    let _ = runtime::register_capture_hotkey(&app, &old_hotkey);
    return Err("快捷键被占用，请用备用入口".to_string());
  }
  let next_form = store.data.settings.float_form.clone();
  let next_theme = store.data.settings.theme.clone();
  store.save()?;
  if next_form != old_form {
    let _ = runtime::set_float_form(&app, &next_form);
    runtime::sync_tray_selection(&app, &next_form);
  }
  if next_theme != old_theme {
    let _ = app.emit_all("theme-changed", next_theme.clone());
  }
  emit(&app, StoreEvent::changed("settings"));
  Ok(store.data.settings.clone())
}

// ---------------------------------------------------------------- 窗口 / 托盘

#[tauri::command]
pub fn open_main_window(app: AppHandle) -> Result<(), String> {
  runtime::show_main(&app, None)
}

#[tauri::command]
pub fn show_float(app: AppHandle) -> Result<(), String> {
  runtime::show_window(&app, runtime::FLOAT_LABEL)
}

#[tauri::command]
pub fn hide_float(app: AppHandle) -> Result<(), String> {
  runtime::hide_window(&app, runtime::FLOAT_LABEL)
}

#[tauri::command]
pub fn set_float_form(
  app: AppHandle,
  state: State<'_, Shared>,
  form: String,
) -> Result<(), String> {
  if !is_valid_choice(&FLOAT_FORMS, &form) {
    return Err("悬浮形态只能是 topmost / desktop / mini".to_string());
  }
  runtime::set_float_form(&app, &form)?;
  let mut store = lock(&state);
  if store.data.settings.float_form != form {
    store.ensure_writable()?;
    store.data.settings.float_form = form.clone();
    store.data.settings.coerce();
    store.save()?;
    emit(&app, StoreEvent::changed("settings"));
  }
  runtime::sync_tray_selection(&app, &form);
  Ok(())
}

#[tauri::command]
pub fn open_capture_overlay(app: AppHandle) -> Result<(), String> {
  runtime::open_capture_overlay(&app)
}

#[tauri::command]
pub fn close_capture_overlay(app: AppHandle) -> Result<(), String> {
  runtime::close_capture_overlay(&app)
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
  let report = store.data.migration.clone();
  store.clear_migration();
  if store.ensure_writable().is_ok() && store.save().is_ok() {
    emit(&app, StoreEvent::changed("data"));
  }
  Ok(report)
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
  export::write_weekly(&settings, &tasks, week, format, dir)
}

// ---------------------------------------------------------------- patch 内时间字段再归一

/// 表单常量（分类 / 优先级 / 悬浮形态 / 保留天数 / 默认热键），避免前端硬编码
#[tauri::command]
pub fn get_form_hints() -> Result<serde_json::Value, String> {
  Ok(serde_json::json!({
    "categories": CATEGORIES,
    "priorities": PRIORITIES,
    "floatForms": FLOAT_FORMS,
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

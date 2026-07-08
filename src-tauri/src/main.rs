#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use log::{info, error};
use std::sync::{Arc, RwLock};
use tauri::State;

pub mod lib;
use lib::*;

struct AppState {
    db: RwLock<SqliteDatabase>,
}

#[tauri::command]
fn get_tasks(state: State<'_, Arc<AppState>>) -> Result<Vec<Task>, String> {
    let db = state.db.read().map_err(|e| format!("{}", e))?;
    TaskService::get_tasks(&*db)
}

#[tauri::command]
fn get_task(state: State<'_, Arc<AppState>>, id: &str) -> Result<Option<Task>, String> {
    let db = state.db.read().map_err(|e| format!("{}", e))?;
    TaskService::get_task(&*db, id)
}

#[tauri::command]
fn add_task(state: State<'_, Arc<AppState>>, task: Task) -> Result<(), String> {
    let mut db = state.db.write().map_err(|e| format!("{}", e))?;
    TaskService::add_task(&mut *db, task)
}

#[tauri::command]
fn update_task(state: State<'_, Arc<AppState>>, id: &str, task: Task) -> Result<(), String> {
    let mut db = state.db.write().map_err(|e| format!("{}", e))?;
    TaskService::update_task(&mut *db, id, task)
}

#[tauri::command]
fn delete_task(state: State<'_, Arc<AppState>>, id: &str) -> Result<(), String> {
    let mut db = state.db.write().map_err(|e| format!("{}", e))?;
    TaskService::delete_task(&mut *db, id)
}

#[tauri::command]
fn toggle_task(state: State<'_, Arc<AppState>>, id: &str) -> Result<(), String> {
    let mut db = state.db.write().map_err(|e| format!("{}", e))?;
    TaskService::toggle_task(&mut *db, id)
}

#[tauri::command]
fn toggle_subtask(state: State<'_, Arc<AppState>>, task_id: &str, subtask_id: &str) -> Result<(), String> {
    let mut db = state.db.write().map_err(|e| format!("{}", e))?;
    TaskService::toggle_subtask(&mut *db, task_id, subtask_id)
}

#[tauri::command]
fn add_subtask(state: State<'_, Arc<AppState>>, task_id: &str, title: &str) -> Result<(), String> {
    let mut db = state.db.write().map_err(|e| format!("{}", e))?;
    TaskService::add_subtask(&mut *db, task_id, title)
}

#[tauri::command]
fn delete_subtask(state: State<'_, Arc<AppState>>, task_id: &str, subtask_id: &str) -> Result<(), String> {
    let mut db = state.db.write().map_err(|e| format!("{}", e))?;
    TaskService::delete_subtask(&mut *db, task_id, subtask_id)
}

#[tauri::command]
fn add_note(state: State<'_, Arc<AppState>>, task_id: &str, note: Note) -> Result<(), String> {
    let mut db = state.db.write().map_err(|e| format!("{}", e))?;
    TaskService::add_note(&mut *db, task_id, note)
}

#[tauri::command]
fn delete_note(state: State<'_, Arc<AppState>>, task_id: &str, note_id: &str) -> Result<(), String> {
    let mut db = state.db.write().map_err(|e| format!("{}", e))?;
    TaskService::delete_note(&mut *db, task_id, note_id)
}

#[tauri::command]
fn get_reminders(state: State<'_, Arc<AppState>>) -> Result<Vec<Reminder>, String> {
    let db = state.db.read().map_err(|e| format!("{}", e))?;
    ReminderService::get_reminders(&*db)
}

#[tauri::command]
fn add_reminder(state: State<'_, Arc<AppState>>, reminder: Reminder) -> Result<(), String> {
    let mut db = state.db.write().map_err(|e| format!("{}", e))?;
    ReminderService::add_reminder(&mut *db, reminder)
}

#[tauri::command]
fn update_reminder(state: State<'_, Arc<AppState>>, id: &str, reminder: Reminder) -> Result<(), String> {
    let mut db = state.db.write().map_err(|e| format!("{}", e))?;
    ReminderService::update_reminder(&mut *db, id, reminder)
}

#[tauri::command]
fn delete_reminder(state: State<'_, Arc<AppState>>, id: &str) -> Result<(), String> {
    let mut db = state.db.write().map_err(|e| format!("{}", e))?;
    ReminderService::delete_reminder(&mut *db, id)
}

#[tauri::command]
fn toggle_reminder(state: State<'_, Arc<AppState>>, id: &str) -> Result<(), String> {
    let mut db = state.db.write().map_err(|e| format!("{}", e))?;
    ReminderService::toggle_reminder(&mut *db, id)
}

#[tauri::command]
fn get_theme(state: State<'_, Arc<AppState>>) -> Result<String, String> {
    let db = state.db.read().map_err(|e| format!("{}", e))?;
    ConfigService::get_theme(&*db)
}

#[tauri::command]
fn set_theme(state: State<'_, Arc<AppState>>, theme: &str) -> Result<(), String> {
    let mut db = state.db.write().map_err(|e| format!("{}", e))?;
    ConfigService::set_theme(&mut *db, theme)
}

#[tauri::command]
fn get_task_stats(state: State<'_, Arc<AppState>>) -> Result<TaskStats, String> {
    let db = state.db.read().map_err(|e| format!("{}", e))?;
    TaskService::get_task_stats(&*db)
}

#[tauri::command]
fn search(state: State<'_, Arc<AppState>>, keyword: &str) -> Result<SearchResult, String> {
    let db = state.db.read().map_err(|e| format!("{}", e))?;
    TaskService::search(&*db, keyword)
}

#[tauri::command]
fn generate_id() -> Result<String, String> {
    Ok(service::generate_id())
}

#[tauri::command]
fn open_main_window(window: tauri::Window) {
    if let Some(main_window) = window.get_window("main") {
        let _ = main_window.show();
        let _ = main_window.set_focus();
    }
}

#[tauri::command]
fn close_main_window(window: tauri::Window) {
    if let Some(main_window) = window.get_window("main") {
        let _ = main_window.hide();
    }
}

#[tauri::command]
fn export_data(state: State<'_, Arc<AppState>>) -> Result<String, String> {
    let db = state.db.read().map_err(|e| format!("{}", e))?;
    db.export_data()
}

#[tauri::command]
fn import_data(state: State<'_, Arc<AppState>>, json_content: &str) -> Result<(), String> {
    let mut db = state.db.write().map_err(|e| format!("{}", e))?;
    db.import_data(json_content)
}

#[tauri::command]
fn get_history(state: State<'_, Arc<AppState>>) -> Result<Vec<HistoryItem>, String> {
    let db = state.db.read().map_err(|e| format!("{}", e))?;
    HistoryService::get_history(&*db)
}

#[tauri::command]
fn get_history_by_date(state: State<'_, Arc<AppState>>, date: &str) -> Result<Vec<HistoryItem>, String> {
    let db = state.db.read().map_err(|e| format!("{}", e))?;
    HistoryService::get_history_by_date(&*db, date)
}

#[tauri::command]
fn get_history_by_type(state: State<'_, Arc<AppState>>, target_type: &str) -> Result<Vec<HistoryItem>, String> {
    let db = state.db.read().map_err(|e| format!("{}", e))?;
    HistoryService::get_history_by_type(&*db, target_type)
}

#[tauri::command]
fn get_history_by_action(state: State<'_, Arc<AppState>>, action: &str) -> Result<Vec<HistoryItem>, String> {
    let db = state.db.read().map_err(|e| format!("{}", e))?;
    HistoryService::get_history_by_action(&*db, action)
}

#[tauri::command]
fn get_history_stats(state: State<'_, Arc<AppState>>) -> Result<HashMap<String, usize>, String> {
    let db = state.db.read().map_err(|e| format!("{}", e))?;
    HistoryService::get_history_stats(&*db)
}

#[tauri::command]
fn clear_history(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut db = state.db.write().map_err(|e| format!("{}", e))?;
    HistoryService::clear_history(&mut *db)
}

#[tauri::command]
fn delete_history_item(state: State<'_, Arc<AppState>>, id: &str) -> Result<(), String> {
    let mut db = state.db.write().map_err(|e| format!("{}", e))?;
    HistoryService::delete_history_item(&mut *db, id)
}

fn main() {
    env_logger::init();
    
    info!("Starting kettd application...");
    
    let db = match SqliteDatabase::new() {
        Ok(db) => {
            info!("Database initialized successfully");
            db
        }
        Err(e) => {
            error!("Failed to initialize database: {}", e);
            panic!("Database initialization failed: {}", e);
        }
    };
    
    let app_state = Arc::new(AppState {
        db: RwLock::new(db),
    });

    info!("Application state initialized");
    
    let context = tauri::generate_context!();
    
    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            get_tasks,
            get_task,
            add_task,
            update_task,
            delete_task,
            toggle_task,
            toggle_subtask,
            add_subtask,
            delete_subtask,
            add_note,
            delete_note,
            get_reminders,
            add_reminder,
            update_reminder,
            delete_reminder,
            toggle_reminder,
            get_theme,
            set_theme,
            get_task_stats,
            search,
            generate_id,
            open_main_window,
            close_main_window,
            export_data,
            import_data,
            get_history,
            get_history_by_date,
            get_history_by_type,
            get_history_by_action,
            get_history_stats,
            clear_history,
            delete_history_item,
        ])
        .system_tray(tauri::SystemTray::new()
            .with_menu(tauri::SystemTrayMenu::new()
                .add_item(tauri::CustomMenuItem::new("show_main", "打开主界面"))
                .add_item(tauri::CustomMenuItem::new("show_float", "显示悬浮面板"))
                .add_item(tauri::CustomMenuItem::new("hide_float", "隐藏悬浮面板"))
                .add_native_item(tauri::SystemTrayMenuItem::Separator)
                .add_item(tauri::CustomMenuItem::new("quit", "退出"))
            )
        )
        .on_system_tray_event(|app, event| match event {
            tauri::SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
                "show_main" => {
                    if let Some(main_window) = app.get_window("main") {
                        let _ = main_window.show();
                        let _ = main_window.set_focus();
                    }
                }
                "show_float" => {
                    if let Some(float_window) = app.get_window("float") {
                        let _ = float_window.show();
                        let _ = float_window.set_focus();
                    }
                }
                "hide_float" => {
                    if let Some(float_window) = app.get_window("float") {
                        let _ = float_window.hide();
                    }
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            },
            _ => {}
        })
        .run(context)
        .expect("error while running tauri application");
}

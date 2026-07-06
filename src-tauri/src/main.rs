#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
struct Task {
    id: String,
    title: String,
    description: String,
    category: String,
    priority: String,
    due_date: String,
    completed: bool,
    subtasks: Vec<Subtask>,
    notes: Vec<Note>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Subtask {
    id: String,
    title: String,
    completed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Note {
    id: String,
    author: String,
    content: String,
    created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Reminder {
    id: String,
    title: String,
    time: String,
    category: String,
    completed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct AppData {
    tasks: Vec<Task>,
    reminders: Vec<Reminder>,
    theme: String,
}

fn get_data_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("todo-list")
}

fn load_data() -> AppData {
    let path = get_data_path().join("data.json");
    if path.exists() {
        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => AppData {
                tasks: vec![],
                reminders: vec![],
                theme: "light".to_string(),
            },
        }
    } else {
        let default_data = AppData {
            tasks: vec![
                Task {
                    id: "1".to_string(),
                    title: "完成季度报告并提交审核".to_string(),
                    description: "整理第三季度的销售数据和市场分析报告，需要包含竞品对比和增长趋势图表。".to_string(),
                    category: "工作".to_string(),
                    priority: "high".to_string(),
                    due_date: "2026-07-05".to_string(),
                    completed: false,
                    subtasks: vec![
                        Subtask { id: "s1".to_string(), title: "收集各部门销售数据".to_string(), completed: true },
                        Subtask { id: "s2".to_string(), title: "制作竞品对比分析表".to_string(), completed: true },
                        Subtask { id: "s3".to_string(), title: "绘制增长趋势图表".to_string(), completed: true },
                        Subtask { id: "s4".to_string(), title: "与产品团队确认发布时间".to_string(), completed: false },
                        Subtask { id: "s5".to_string(), title: "整合报告并提交审核".to_string(), completed: false },
                    ],
                    notes: vec![
                        Note { id: "n1".to_string(), author: "我".to_string(), content: "销售数据已经从财务部门拿到了，竞品数据还需要从市场部获取。".to_string(), created_at: "2026-07-06 10:30".to_string() },
                        Note { id: "n2".to_string(), author: "李经理".to_string(), content: "报告模板已发到共享文件夹，请按照新模板格式填写。".to_string(), created_at: "2026-07-05 16:45".to_string() },
                    ],
                    created_at: "2026-07-01".to_string(),
                    updated_at: "2026-07-06".to_string(),
                },
                Task {
                    id: "2".to_string(),
                    title: "准备下午三点项目评审会议".to_string(),
                    description: "准备演示材料和PPT，确保会议顺利进行。".to_string(),
                    category: "工作".to_string(),
                    priority: "high".to_string(),
                    due_date: "2026-07-06 15:00".to_string(),
                    completed: false,
                    subtasks: vec![],
                    notes: vec![],
                    created_at: "2026-07-06".to_string(),
                    updated_at: "2026-07-06".to_string(),
                },
                Task {
                    id: "3".to_string(),
                    title: "阅读《设计模式》第五章并做笔记".to_string(),
                    description: "深入理解单例模式、工厂模式等常用设计模式。".to_string(),
                    category: "学习".to_string(),
                    priority: "medium".to_string(),
                    due_date: "2026-07-06".to_string(),
                    completed: false,
                    subtasks: vec![],
                    notes: vec![],
                    created_at: "2026-07-05".to_string(),
                    updated_at: "2026-07-06".to_string(),
                },
                Task {
                    id: "4".to_string(),
                    title: "整理书桌和文件归档".to_string(),
                    description: "清理桌面文件，将重要文件归档到相应文件夹。".to_string(),
                    category: "个人".to_string(),
                    priority: "low".to_string(),
                    due_date: "2026-07-06".to_string(),
                    completed: true,
                    subtasks: vec![],
                    notes: vec![],
                    created_at: "2026-07-06".to_string(),
                    updated_at: "2026-07-06".to_string(),
                },
                Task {
                    id: "5".to_string(),
                    title: "编写前端组件单元测试".to_string(),
                    description: "为核心组件编写完整的单元测试用例。".to_string(),
                    category: "工作".to_string(),
                    priority: "medium".to_string(),
                    due_date: "2026-07-07".to_string(),
                    completed: false,
                    subtasks: vec![],
                    notes: vec![],
                    created_at: "2026-07-06".to_string(),
                    updated_at: "2026-07-06".to_string(),
                },
                Task {
                    id: "6".to_string(),
                    title: "预约牙科检查".to_string(),
                    description: "联系牙科诊所预约检查时间。".to_string(),
                    category: "个人".to_string(),
                    priority: "low".to_string(),
                    due_date: "2026-07-07".to_string(),
                    completed: false,
                    subtasks: vec![],
                    notes: vec![],
                    created_at: "2026-07-06".to_string(),
                    updated_at: "2026-07-06".to_string(),
                },
                Task {
                    id: "7".to_string(),
                    title: "完成 TypeScript 高级类型练习".to_string(),
                    description: "练习泛型、条件类型、映射类型等高级特性。".to_string(),
                    category: "学习".to_string(),
                    priority: "high".to_string(),
                    due_date: "2026-07-06".to_string(),
                    completed: false,
                    subtasks: vec![],
                    notes: vec![],
                    created_at: "2026-07-06".to_string(),
                    updated_at: "2026-07-06".to_string(),
                },
                Task {
                    id: "8".to_string(),
                    title: "回复团队邮件".to_string(),
                    description: "回复本周团队周报收集邮件。".to_string(),
                    category: "工作".to_string(),
                    priority: "medium".to_string(),
                    due_date: "2026-07-06".to_string(),
                    completed: true,
                    subtasks: vec![],
                    notes: vec![],
                    created_at: "2026-07-06".to_string(),
                    updated_at: "2026-07-06".to_string(),
                },
            ],
            reminders: vec![
                Reminder { id: "r1".to_string(), title: "每日站会".to_string(), time: "09:00".to_string(), category: "工作".to_string(), completed: false },
                Reminder { id: "r2".to_string(), title: "周报提交提醒".to_string(), time: "17:00".to_string(), category: "工作".to_string(), completed: false },
                Reminder { id: "r3".to_string(), title: "季度复盘".to_string(), time: "09:00".to_string(), category: "工作".to_string(), completed: false },
                Reminder { id: "r4".to_string(), title: "喝水提醒".to_string(), time: "10:00/14:00/16:00".to_string(), category: "个人".to_string(), completed: false },
                Reminder { id: "r5".to_string(), title: "午休结束".to_string(), time: "13:30".to_string(), category: "个人".to_string(), completed: false },
            ],
            theme: "light".to_string(),
        };
        let _ = save_data(&default_data);
        default_data
    }
}

fn save_data(data: &AppData) -> Result<(), String> {
    let path = get_data_path();
    if let Err(e) = fs::create_dir_all(&path) {
        return Err(format!("Failed to create directory: {}", e));
    }
    let file_path = path.join("data.json");
    match serde_json::to_string_pretty(data) {
        Ok(content) => match fs::write(&file_path, content) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to write file: {}", e)),
        },
        Err(e) => Err(format!("Failed to serialize data: {}", e)),
    }
}

#[tauri::command]
fn get_tasks() -> Result<Vec<Task>, String> {
    Ok(load_data().tasks)
}

#[tauri::command]
fn get_task(id: &str) -> Result<Option<Task>, String> {
    Ok(load_data().tasks.into_iter().find(|t| t.id == id))
}

#[tauri::command]
fn add_task(task: Task) -> Result<(), String> {
    let mut data = load_data();
    data.tasks.push(task);
    save_data(&data)
}

#[tauri::command]
fn update_task(id: &str, updated_task: Task) -> Result<(), String> {
    let mut data = load_data();
    if let Some(index) = data.tasks.iter_mut().position(|t| t.id == id) {
        data.tasks[index] = updated_task;
        save_data(&data)
    } else {
        Err("Task not found".to_string())
    }
}

#[tauri::command]
fn delete_task(id: &str) -> Result<(), String> {
    let mut data = load_data();
    data.tasks.retain(|t| t.id != id);
    save_data(&data)
}

#[tauri::command]
fn toggle_task(id: &str) -> Result<(), String> {
    let mut data = load_data();
    if let Some(task) = data.tasks.iter_mut().find(|t| t.id == id) {
        task.completed = !task.completed;
        save_data(&data)
    } else {
        Err("Task not found".to_string())
    }
}

#[tauri::command]
fn toggle_subtask(task_id: &str, subtask_id: &str) -> Result<(), String> {
    let mut data = load_data();
    if let Some(task) = data.tasks.iter_mut().find(|t| t.id == task_id) {
        if let Some(subtask) = task.subtasks.iter_mut().find(|s| s.id == subtask_id) {
            subtask.completed = !subtask.completed;
            return save_data(&data);
        }
    }
    Err("Subtask not found".to_string())
}

#[tauri::command]
fn add_note(task_id: &str, note: Note) -> Result<(), String> {
    let mut data = load_data();
    if let Some(task) = data.tasks.iter_mut().find(|t| t.id == task_id) {
        task.notes.push(note);
        save_data(&data)
    } else {
        Err("Task not found".to_string())
    }
}

#[tauri::command]
fn get_reminders() -> Result<Vec<Reminder>, String> {
    Ok(load_data().reminders)
}

#[tauri::command]
fn add_reminder(reminder: Reminder) -> Result<(), String> {
    let mut data = load_data();
    data.reminders.push(reminder);
    save_data(&data)
}

#[tauri::command]
fn update_reminder(id: &str, updated: Reminder) -> Result<(), String> {
    let mut data = load_data();
    if let Some(index) = data.reminders.iter_mut().position(|r| r.id == id) {
        data.reminders[index] = updated;
        save_data(&data)
    } else {
        Err("Reminder not found".to_string())
    }
}

#[tauri::command]
fn delete_reminder(id: &str) -> Result<(), String> {
    let mut data = load_data();
    data.reminders.retain(|r| r.id != id);
    save_data(&data)
}

#[tauri::command]
fn toggle_reminder(id: &str) -> Result<(), String> {
    let mut data = load_data();
    if let Some(reminder) = data.reminders.iter_mut().find(|r| r.id == id) {
        reminder.completed = !reminder.completed;
        save_data(&data)
    } else {
        Err("Reminder not found".to_string())
    }
}

#[tauri::command]
fn get_theme() -> Result<String, String> {
    Ok(load_data().theme)
}

#[tauri::command]
fn set_theme(theme: &str) -> Result<(), String> {
    let mut data = load_data();
    data.theme = theme.to_string();
    save_data(&data)
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

fn main() {
    let context = tauri::generate_context!();
    
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_tasks,
            get_task,
            add_task,
            update_task,
            delete_task,
            toggle_task,
            toggle_subtask,
            add_note,
            get_reminders,
            add_reminder,
            update_reminder,
            delete_reminder,
            toggle_reminder,
            get_theme,
            set_theme,
            open_main_window,
            close_main_window,
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

use crate::models::{Task, Subtask, Note, Reminder, HistoryItem, TaskStats, SearchResult};
use crate::persistence::Database;
use crate::validation::{validate_task, validate_reminder};
use chrono::Local;
use log::{info, warn, error};
use std::collections::HashMap;
use uuid::Uuid;

fn generate_uuid() -> String {
    Uuid::new_v4().to_string()
}

fn get_current_time() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn get_current_date() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

fn record_history(db: &mut dyn Database, action: &str, target_type: &str, target_id: &str, target_title: &str, detail: &str) -> Result<(), String> {
    let item = HistoryItem {
        id: generate_uuid(),
        action: action.to_string(),
        target_type: target_type.to_string(),
        target_id: target_id.to_string(),
        target_title: target_title.to_string(),
        detail: detail.to_string(),
        timestamp: get_current_time(),
    };
    db.add_history_item(item)
}

pub struct TaskService;

impl TaskService {
    pub fn get_tasks(db: &dyn Database) -> Result<Vec<Task>, String> {
        db.get_tasks()
    }

    pub fn get_task(db: &dyn Database, id: &str) -> Result<Option<Task>, String> {
        db.get_task(id)
    }

    pub fn add_task(db: &mut dyn Database, mut task: Task) -> Result<(), String> {
        info!("Adding task: {}", task.title);
        if let Err(e) = validate_task(&task) {
            warn!("Task validation failed: {}", e);
            return Err(e);
        }
        if task.id.is_empty() {
            task.id = generate_uuid();
        }
        if task.created_at.is_empty() {
            task.created_at = get_current_date();
        }
        if task.updated_at.is_empty() {
            task.updated_at = get_current_date();
        }
        let title = task.title.clone();
        let task_id = task.id.clone();
        match db.add_task(task) {
            Ok(_) => {
                info!("Task added successfully: {} ({})", title, task_id);
                record_history(db, "创建", "任务", &task_id, &title, "")
            }
            Err(e) => {
                error!("Failed to add task: {}", e);
                Err(e)
            }
        }
    }

    pub fn update_task(db: &mut dyn Database, id: &str, mut task: Task) -> Result<(), String> {
        info!("Updating task: {}", id);
        if let Err(e) = validate_task(&task) {
            warn!("Task validation failed: {}", e);
            return Err(e);
        }
        let old_task = db.get_task(id)?.ok_or_else(|| "任务不存在".to_string())?;
        let old_title = old_task.title;
        task.id = id.to_string();
        task.updated_at = get_current_date();
        match db.update_task(id, task.clone()) {
            Ok(_) => {
                info!("Task updated successfully: {} → {}", old_title, task.title);
                let detail = if old_title != task.title { format!("标题: {} → {}", old_title, task.title) } else { "更新任务信息".to_string() };
                record_history(db, "更新", "任务", id, &task.title, &detail)
            }
            Err(e) => {
                error!("Failed to update task: {}", e);
                Err(e)
            }
        }
    }

    pub fn delete_task(db: &mut dyn Database, id: &str) -> Result<(), String> {
        info!("Deleting task: {}", id);
        let task = db.get_task(id)?.ok_or_else(|| "任务不存在".to_string())?;
        let title = task.title;
        match db.delete_task(id) {
            Ok(_) => {
                info!("Task deleted successfully: {}", title);
                record_history(db, "删除", "任务", id, &title, "")
            }
            Err(e) => {
                error!("Failed to delete task: {}", e);
                Err(e)
            }
        }
    }

    pub fn toggle_task(db: &mut dyn Database, id: &str) -> Result<(), String> {
        let mut task = db.get_task(id)?.ok_or_else(|| "任务不存在".to_string())?;
        let old_status = task.completed;
        task.completed = !task.completed;
        task.updated_at = get_current_date();
        db.update_task(id, task.clone())?;
        let detail = if task.completed { "状态: 未完成 → 已完成" } else { "状态: 已完成 → 未完成" };
        record_history(db, "完成", "任务", id, &task.title, detail)
    }

    pub fn toggle_subtask(db: &mut dyn Database, task_id: &str, subtask_id: &str) -> Result<(), String> {
        let mut task = db.get_task(task_id)?.ok_or_else(|| "任务不存在".to_string())?;
        if let Some(subtask) = task.subtasks.iter_mut().find(|s| s.id == subtask_id) {
            subtask.completed = !subtask.completed;
            task.updated_at = get_current_date();
            db.update_task(task_id, task)
        } else {
            Err("子任务不存在".to_string())
        }
    }

    pub fn add_subtask(db: &mut dyn Database, task_id: &str, title: &str) -> Result<(), String> {
        if title.trim().is_empty() {
            return Err("子任务标题不能为空".to_string());
        }
        if title.len() > 100 {
            return Err("子任务标题长度不能超过100个字符".to_string());
        }
        let mut task = db.get_task(task_id)?.ok_or_else(|| "任务不存在".to_string())?;
        task.subtasks.push(Subtask {
            id: generate_uuid(),
            title: title.trim().to_string(),
            completed: false,
        });
        task.updated_at = get_current_date();
        db.update_task(task_id, task)
    }

    pub fn delete_subtask(db: &mut dyn Database, task_id: &str, subtask_id: &str) -> Result<(), String> {
        let mut task = db.get_task(task_id)?.ok_or_else(|| "任务不存在".to_string())?;
        let original_len = task.subtasks.len();
        task.subtasks.retain(|s| s.id != subtask_id);
        if task.subtasks.len() == original_len {
            return Err("子任务不存在".to_string());
        }
        task.updated_at = get_current_date();
        db.update_task(task_id, task)
    }

    pub fn add_note(db: &mut dyn Database, task_id: &str, mut note: Note) -> Result<(), String> {
        if note.content.trim().is_empty() {
            return Err("备注内容不能为空".to_string());
        }
        if note.content.len() > 500 {
            return Err("备注内容长度不能超过500个字符".to_string());
        }
        let mut task = db.get_task(task_id)?.ok_or_else(|| "任务不存在".to_string())?;
        note.id = generate_uuid();
        note.created_at = get_current_time();
        task.notes.push(note);
        task.updated_at = get_current_date();
        db.update_task(task_id, task)
    }

    pub fn delete_note(db: &mut dyn Database, task_id: &str, note_id: &str) -> Result<(), String> {
        let mut task = db.get_task(task_id)?.ok_or_else(|| "任务不存在".to_string())?;
        let original_len = task.notes.len();
        task.notes.retain(|n| n.id != note_id);
        if task.notes.len() == original_len {
            return Err("备注不存在".to_string());
        }
        task.updated_at = get_current_date();
        db.update_task(task_id, task)
    }

    pub fn get_task_stats(db: &dyn Database) -> Result<TaskStats, String> {
        let tasks = db.get_tasks()?;
        let today_str = get_current_date();
        
        let mut stats = TaskStats {
            total: tasks.len(),
            completed: tasks.iter().filter(|t| t.completed).count(),
            pending: tasks.iter().filter(|t| !t.completed).count(),
            overdue: 0,
            today: 0,
            by_category: HashMap::new(),
            by_priority: HashMap::new(),
        };
        
        for task in &tasks {
            if !task.completed {
                if let Ok(due_date) = chrono::NaiveDate::parse_from_str(&task.due_date.split(' ').next().unwrap_or(&task.due_date), "%Y-%m-%d") {
                    if let Ok(today) = chrono::NaiveDate::parse_from_str(&today_str, "%Y-%m-%d") {
                        if due_date < today {
                            stats.overdue += 1;
                        }
                        if due_date == today {
                            stats.today += 1;
                        }
                    }
                }
            }
            
            *stats.by_category.entry(task.category.clone()).or_insert(0) += 1;
            *stats.by_priority.entry(task.priority.clone()).or_insert(0) += 1;
        }
        
        Ok(stats)
    }

    pub fn search(db: &dyn Database, keyword: &str) -> Result<SearchResult, String> {
        if keyword.trim().is_empty() {
            return Err("搜索关键词不能为空".to_string());
        }
        
        let tasks = db.get_tasks()?;
        let reminders = db.get_reminders()?;
        let keyword_lower = keyword.to_lowercase();
        
        let filtered_tasks: Vec<Task> = tasks
            .into_iter()
            .filter(|t| t.title.to_lowercase().contains(&keyword_lower) || 
                        t.description.to_lowercase().contains(&keyword_lower))
            .collect();
        
        let filtered_reminders: Vec<Reminder> = reminders
            .into_iter()
            .filter(|r| r.title.to_lowercase().contains(&keyword_lower))
            .collect();
        
        Ok(SearchResult { tasks: filtered_tasks, reminders: filtered_reminders })
    }
}

pub struct ReminderService;

impl ReminderService {
    pub fn get_reminders(db: &dyn Database) -> Result<Vec<Reminder>, String> {
        db.get_reminders()
    }

    pub fn add_reminder(db: &mut dyn Database, mut reminder: Reminder) -> Result<(), String> {
        if let Err(e) = validate_reminder(&reminder) {
            return Err(e);
        }
        reminder.id = generate_uuid();
        let title = reminder.title.clone();
        let reminder_id = reminder.id.clone();
        db.add_reminder(reminder)?;
        record_history(db, "创建", "提醒", &reminder_id, &title, "")
    }

    pub fn update_reminder(db: &mut dyn Database, id: &str, mut reminder: Reminder) -> Result<(), String> {
        if let Err(e) = validate_reminder(&reminder) {
            return Err(e);
        }
        let old_reminder = db.get_reminder(id)?.ok_or_else(|| "提醒不存在".to_string())?;
        let old_title = old_reminder.title;
        reminder.id = id.to_string();
        db.update_reminder(id, reminder.clone())?;
        let detail = if old_title != reminder.title { format!("标题: {} → {}", old_title, reminder.title) } else { "更新提醒信息".to_string() };
        record_history(db, "更新", "提醒", id, &reminder.title, &detail)
    }

    pub fn delete_reminder(db: &mut dyn Database, id: &str) -> Result<(), String> {
        let reminder = db.get_reminder(id)?.ok_or_else(|| "提醒不存在".to_string())?;
        let title = reminder.title;
        db.delete_reminder(id)?;
        record_history(db, "删除", "提醒", id, &title, "")
    }

    pub fn toggle_reminder(db: &mut dyn Database, id: &str) -> Result<(), String> {
        let mut reminder = db.get_reminder(id)?.ok_or_else(|| "提醒不存在".to_string())?;
        reminder.completed = !reminder.completed;
        db.update_reminder(id, reminder)
    }
}

pub struct HistoryService;

impl HistoryService {
    pub fn get_history(db: &dyn Database) -> Result<Vec<HistoryItem>, String> {
        db.get_history()
    }

    pub fn get_history_by_date(db: &dyn Database, date: &str) -> Result<Vec<HistoryItem>, String> {
        db.get_history_by_date(date)
    }

    pub fn get_history_by_type(db: &dyn Database, target_type: &str) -> Result<Vec<HistoryItem>, String> {
        db.get_history_by_type(target_type)
    }

    pub fn get_history_by_action(db: &dyn Database, action: &str) -> Result<Vec<HistoryItem>, String> {
        db.get_history_by_action(action)
    }

    pub fn get_history_stats(db: &dyn Database) -> Result<HashMap<String, usize>, String> {
        let history = db.get_history()?;
        let mut stats = HashMap::new();
        
        for item in &history {
            *stats.entry(item.action.clone()).or_insert(0) += 1;
            *stats.entry(item.target_type.clone()).or_insert(0) += 1;
        }
        
        *stats.entry("total".to_string()).or_insert(0) = history.len();
        
        Ok(stats)
    }

    pub fn clear_history(db: &mut dyn Database) -> Result<(), String> {
        db.clear_history()
    }

    pub fn delete_history_item(db: &mut dyn Database, id: &str) -> Result<(), String> {
        db.delete_history_item(id)
    }
}

pub struct ConfigService;

impl ConfigService {
    pub fn get_theme(db: &dyn Database) -> Result<String, String> {
        db.get_theme()
    }

    pub fn set_theme(db: &mut dyn Database, theme: &str) -> Result<(), String> {
        if !["light", "dark"].contains(&theme) {
            return Err("无效的主题设置".to_string());
        }
        db.set_theme(theme)
    }
}

pub fn generate_id() -> String {
    generate_uuid()
}

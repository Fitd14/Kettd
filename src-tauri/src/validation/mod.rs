use crate::models::{Task, Reminder};

pub fn validate_task(task: &Task) -> Result<(), String> {
    if task.title.trim().is_empty() {
        return Err("任务标题不能为空".to_string());
    }
    if task.title.len() > 200 {
        return Err("任务标题长度不能超过200个字符".to_string());
    }
    if task.description.len() > 2000 {
        return Err("任务描述长度不能超过2000个字符".to_string());
    }
    if !["工作", "个人", "学习"].contains(&task.category.as_str()) {
        return Err("无效的任务分类".to_string());
    }
    if !["high", "medium", "low"].contains(&task.priority.as_str()) {
        return Err("无效的优先级".to_string());
    }
    Ok(())
}

pub fn validate_reminder(reminder: &Reminder) -> Result<(), String> {
    if reminder.title.trim().is_empty() {
        return Err("提醒标题不能为空".to_string());
    }
    if reminder.title.len() > 200 {
        return Err("提醒标题长度不能超过200个字符".to_string());
    }
    if !["工作", "个人", "学习"].contains(&reminder.category.as_str()) {
        return Err("无效的提醒分类".to_string());
    }
    Ok(())
}

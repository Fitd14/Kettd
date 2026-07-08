use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub priority: String,
    pub due_date: String,
    pub completed: bool,
    pub subtasks: Vec<Subtask>,
    pub notes: Vec<Note>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Subtask {
    pub id: String,
    pub title: String,
    pub completed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Note {
    pub id: String,
    pub author: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Reminder {
    pub id: String,
    pub title: String,
    pub time: String,
    pub category: String,
    pub completed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HistoryItem {
    pub id: String,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub target_title: String,
    pub detail: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppData {
    pub tasks: Vec<Task>,
    pub reminders: Vec<Reminder>,
    pub theme: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub history: Vec<HistoryItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskStats {
    pub total: usize,
    pub completed: usize,
    pub pending: usize,
    pub overdue: usize,
    pub today: usize,
    pub by_category: HashMap<String, usize>,
    pub by_priority: HashMap<String, usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub tasks: Vec<Task>,
    pub reminders: Vec<Reminder>,
}

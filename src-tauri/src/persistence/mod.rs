pub mod migration;

use crate::models::{Task, Subtask, Note, Reminder, HistoryItem, AppData};
use rusqlite::{Connection, params};
use serde_json;
use std::fs;
use std::path::PathBuf;

pub trait Database {
    fn get_tasks(&self) -> Result<Vec<Task>, String>;
    fn get_task(&self, id: &str) -> Result<Option<Task>, String>;
    fn add_task(&mut self, task: Task) -> Result<(), String>;
    fn update_task(&mut self, id: &str, task: Task) -> Result<(), String>;
    fn delete_task(&mut self, id: &str) -> Result<(), String>;
    
    fn get_reminders(&self) -> Result<Vec<Reminder>, String>;
    fn get_reminder(&self, id: &str) -> Result<Option<Reminder>, String>;
    fn add_reminder(&mut self, reminder: Reminder) -> Result<(), String>;
    fn update_reminder(&mut self, id: &str, reminder: Reminder) -> Result<(), String>;
    fn delete_reminder(&mut self, id: &str) -> Result<(), String>;
    
    fn get_history(&self) -> Result<Vec<HistoryItem>, String>;
    fn get_history_by_date(&self, date: &str) -> Result<Vec<HistoryItem>, String>;
    fn get_history_by_type(&self, target_type: &str) -> Result<Vec<HistoryItem>, String>;
    fn get_history_by_action(&self, action: &str) -> Result<Vec<HistoryItem>, String>;
    fn add_history_item(&mut self, item: HistoryItem) -> Result<(), String>;
    fn clear_history(&mut self) -> Result<(), String>;
    fn delete_history_item(&mut self, id: &str) -> Result<(), String>;
    
    fn get_theme(&self) -> Result<String, String>;
    fn set_theme(&mut self, theme: &str) -> Result<(), String>;
    
    fn export_data(&self) -> Result<String, String>;
    fn import_data(&mut self, json_content: &str) -> Result<(), String>;
}

pub struct SqliteDatabase {
    conn: Connection,
}

impl SqliteDatabase {
    pub fn new() -> Result<Self, String> {
        let db_path = Self::get_db_path();
        let parent_dir = db_path.parent().unwrap();
        if let Err(e) = fs::create_dir_all(parent_dir) {
            return Err(format!("Failed to create directory: {}", e));
        }
        
        let mut conn = Connection::open(&db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;
        
        Self::create_tables(&conn)?;
        Self::ensure_default_config(&mut conn)?;
        Self::migrate_from_json_if_needed(&mut conn)?;
        migration::run_migrations(&mut conn)?;
        
        Ok(Self { conn })
    }
    
    fn get_db_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("kettd")
            .join("kettd.db")
    }
    
    fn create_tables(conn: &Connection) -> Result<(), String> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT,
                category TEXT NOT NULL,
                priority TEXT NOT NULL,
                due_date TEXT,
                completed INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        ).map_err(|e| format!("Failed to create tasks table: {}", e))?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS subtasks (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                title TEXT NOT NULL,
                completed INTEGER DEFAULT 0,
                FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
            )",
            [],
        ).map_err(|e| format!("Failed to create subtasks table: {}", e))?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS notes (
                id TEXT PRIMARY KEY,
                task_id TEXT NOT NULL,
                author TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
            )",
            [],
        ).map_err(|e| format!("Failed to create notes table: {}", e))?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS reminders (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                time TEXT NOT NULL,
                category TEXT NOT NULL,
                completed INTEGER DEFAULT 0
            )",
            [],
        ).map_err(|e| format!("Failed to create reminders table: {}", e))?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS history (
                id TEXT PRIMARY KEY,
                action TEXT NOT NULL,
                target_type TEXT NOT NULL,
                target_id TEXT NOT NULL,
                target_title TEXT NOT NULL,
                detail TEXT,
                timestamp TEXT NOT NULL
            )",
            [],
        ).map_err(|e| format!("Failed to create history table: {}", e))?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS app_config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        ).map_err(|e| format!("Failed to create app_config table: {}", e))?;
        
        Ok(())
    }
    
    fn migrate_from_json_if_needed(conn: &mut Connection) -> Result<(), String> {
        let json_path = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("kettd")
            .join("data.json");
        
        if !json_path.exists() {
            return Ok(());
        }
        
        let content = fs::read_to_string(&json_path)
            .map_err(|e| format!("Failed to read JSON file: {}", e))?;
        
        let app_data: AppData = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse JSON: {}", e))?;
        
        let tx = conn.transaction()
            .map_err(|e| format!("Failed to start transaction: {}", e))?;
        
        for task in &app_data.tasks {
            tx.execute(
                "INSERT OR IGNORE INTO tasks (id, title, description, category, priority, due_date, completed, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    task.id,
                    task.title,
                    task.description,
                    task.category,
                    task.priority,
                    task.due_date,
                    task.completed as i32,
                    task.created_at,
                    task.updated_at,
                ],
            ).map_err(|e| format!("Failed to insert task: {}", e))?;
            
            for subtask in &task.subtasks {
                tx.execute(
                    "INSERT OR IGNORE INTO subtasks (id, task_id, title, completed)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![subtask.id, task.id, subtask.title, subtask.completed as i32],
                ).map_err(|e| format!("Failed to insert subtask: {}", e))?;
            }
            
            for note in &task.notes {
                tx.execute(
                    "INSERT OR IGNORE INTO notes (id, task_id, author, content, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![note.id, task.id, note.author, note.content, note.created_at],
                ).map_err(|e| format!("Failed to insert note: {}", e))?;
            }
        }
        
        for reminder in &app_data.reminders {
            tx.execute(
                "INSERT OR IGNORE INTO reminders (id, title, time, category, completed)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![reminder.id, reminder.title, reminder.time, reminder.category, reminder.completed as i32],
            ).map_err(|e| format!("Failed to insert reminder: {}", e))?;
        }
        
        for item in &app_data.history {
            tx.execute(
                "INSERT OR IGNORE INTO history (id, action, target_type, target_id, target_title, detail, timestamp)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![item.id, item.action, item.target_type, item.target_id, item.target_title, item.detail, item.timestamp],
            ).map_err(|e| format!("Failed to insert history: {}", e))?;
        }
        
        tx.execute(
            "INSERT OR IGNORE INTO app_config (key, value) VALUES ('theme', ?1)",
            params![app_data.theme],
        ).map_err(|e| format!("Failed to insert theme: {}", e))?;
        
        tx.execute(
            "INSERT OR IGNORE INTO app_config (key, value) VALUES ('version', ?1)",
            params![app_data.version],
        ).map_err(|e| format!("Failed to insert version: {}", e))?;
        
        tx.commit()
            .map_err(|e| format!("Failed to commit transaction: {}", e))?;
        
        if let Err(e) = fs::rename(&json_path, json_path.with_extension("json.bak")) {
            return Err(format!("Failed to backup JSON file: {}", e));
        }
        
        Ok(())
    }
    
    fn ensure_default_config(conn: &mut Connection) -> Result<(), String> {
        conn.execute(
            "INSERT OR IGNORE INTO app_config (key, value) VALUES ('theme', 'light')",
            [],
        ).map_err(|e| format!("Failed to insert default theme: {}", e))?;
        
        conn.execute(
            "INSERT OR IGNORE INTO app_config (key, value) VALUES ('version', '1.0.0')",
            [],
        ).map_err(|e| format!("Failed to insert default version: {}", e))?;
        
        Ok(())
    }
    
    fn load_task_with_relations(&self, row: &rusqlite::Row) -> Result<Task, String> {
        let id: String = row.get(0).map_err(|e| format!("{}", e))?;
        let title: String = row.get(1).map_err(|e| format!("{}", e))?;
        let description: String = row.get(2).map_err(|e| format!("{}", e))?;
        let category: String = row.get(3).map_err(|e| format!("{}", e))?;
        let priority: String = row.get(4).map_err(|e| format!("{}", e))?;
        let due_date: String = row.get(5).map_err(|e| format!("{}", e))?;
        let completed: i32 = row.get(6).map_err(|e| format!("{}", e))?;
        let created_at: String = row.get(7).map_err(|e| format!("{}", e))?;
        let updated_at: String = row.get(8).map_err(|e| format!("{}", e))?;
        
        let subtasks: Vec<Subtask> = self.conn.prepare("SELECT id, title, completed FROM subtasks WHERE task_id = ?")
            .map_err(|e| format!("{}", e))?
            .query_map(params![&id], |row| {
                Ok(Subtask {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    completed: row.get(2)? == 1,
                })
            })
            .map_err(|e| format!("{}", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("{}", e))?;
        
        let notes: Vec<Note> = self.conn.prepare("SELECT id, author, content, created_at FROM notes WHERE task_id = ?")
            .map_err(|e| format!("{}", e))?
            .query_map(params![&id], |row| {
                Ok(Note {
                    id: row.get(0)?,
                    author: row.get(1)?,
                    content: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })
            .map_err(|e| format!("{}", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("{}", e))?;
        
        Ok(Task {
            id,
            title,
            description,
            category,
            priority,
            due_date,
            completed: completed == 1,
            subtasks,
            notes,
            created_at,
            updated_at,
        })
    }
}

impl Database for SqliteDatabase {
    fn get_tasks(&self) -> Result<Vec<Task>, String> {
        let mut stmt = self.conn.prepare("SELECT id, title, description, category, priority, due_date, completed, created_at, updated_at FROM tasks")
            .map_err(|e| format!("{}", e))?;
        
        let tasks: Vec<Task> = stmt.query_map([], |row| {
            self.load_task_with_relations(row)
        })
        .map_err(|e| format!("{}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{}", e))?;
        
        Ok(tasks)
    }
    
    fn get_task(&self, id: &str) -> Result<Option<Task>, String> {
        let mut stmt = self.conn.prepare("SELECT id, title, description, category, priority, due_date, completed, created_at, updated_at FROM tasks WHERE id = ?")
            .map_err(|e| format!("{}", e))?;
        
        let task: Option<Task> = stmt.query_row(params![id], |row| {
            self.load_task_with_relations(row)
        })
        .ok();
        
        Ok(task)
    }
    
    fn add_task(&mut self, task: Task) -> Result<(), String> {
        let tx = self.conn.transaction()
            .map_err(|e| format!("{}", e))?;
        
        tx.execute(
            "INSERT INTO tasks (id, title, description, category, priority, due_date, completed, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                task.id,
                task.title,
                task.description,
                task.category,
                task.priority,
                task.due_date,
                task.completed as i32,
                task.created_at,
                task.updated_at,
            ],
        ).map_err(|e| format!("{}", e))?;
        
        for subtask in &task.subtasks {
            tx.execute(
                "INSERT INTO subtasks (id, task_id, title, completed) VALUES (?1, ?2, ?3, ?4)",
                params![subtask.id, task.id, subtask.title, subtask.completed as i32],
            ).map_err(|e| format!("{}", e))?;
        }
        
        for note in &task.notes {
            tx.execute(
                "INSERT INTO notes (id, task_id, author, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![note.id, task.id, note.author, note.content, note.created_at],
            ).map_err(|e| format!("{}", e))?;
        }
        
        tx.commit()
            .map_err(|e| format!("{}", e))?;
        
        Ok(())
    }
    
    fn update_task(&mut self, id: &str, task: Task) -> Result<(), String> {
        let tx = self.conn.transaction()
            .map_err(|e| format!("{}", e))?;
        
        tx.execute(
            "UPDATE tasks SET title = ?1, description = ?2, category = ?3, priority = ?4, due_date = ?5, completed = ?6, updated_at = ?7 WHERE id = ?8",
            params![task.title, task.description, task.category, task.priority, task.due_date, task.completed as i32, task.updated_at, id],
        ).map_err(|e| format!("{}", e))?;
        
        tx.execute("DELETE FROM subtasks WHERE task_id = ?", params![id])
            .map_err(|e| format!("{}", e))?;
        
        for subtask in &task.subtasks {
            tx.execute(
                "INSERT INTO subtasks (id, task_id, title, completed) VALUES (?1, ?2, ?3, ?4)",
                params![subtask.id, id, subtask.title, subtask.completed as i32],
            ).map_err(|e| format!("{}", e))?;
        }
        
        tx.execute("DELETE FROM notes WHERE task_id = ?", params![id])
            .map_err(|e| format!("{}", e))?;
        
        for note in &task.notes {
            tx.execute(
                "INSERT INTO notes (id, task_id, author, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![note.id, id, note.author, note.content, note.created_at],
            ).map_err(|e| format!("{}", e))?;
        }
        
        tx.commit()
            .map_err(|e| format!("{}", e))?;
        
        Ok(())
    }
    
    fn delete_task(&mut self, id: &str) -> Result<(), String> {
        self.conn.execute("DELETE FROM tasks WHERE id = ?", params![id])
            .map_err(|e| format!("{}", e))?;
        
        Ok(())
    }
    
    fn get_reminders(&self) -> Result<Vec<Reminder>, String> {
        let mut stmt = self.conn.prepare("SELECT id, title, time, category, completed FROM reminders")
            .map_err(|e| format!("{}", e))?;
        
        let reminders: Vec<Reminder> = stmt.query_map([], |row| {
            Ok(Reminder {
                id: row.get(0)?,
                title: row.get(1)?,
                time: row.get(2)?,
                category: row.get(3)?,
                completed: row.get(4)? == 1,
            })
        })
        .map_err(|e| format!("{}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{}", e))?;
        
        Ok(reminders)
    }
    
    fn get_reminder(&self, id: &str) -> Result<Option<Reminder>, String> {
        let mut stmt = self.conn.prepare("SELECT id, title, time, category, completed FROM reminders WHERE id = ?")
            .map_err(|e| format!("{}", e))?;
        
        let reminder: Option<Reminder> = stmt.query_row(params![id], |row| {
            Ok(Reminder {
                id: row.get(0)?,
                title: row.get(1)?,
                time: row.get(2)?,
                category: row.get(3)?,
                completed: row.get(4)? == 1,
            })
        })
        .ok();
        
        Ok(reminder)
    }
    
    fn add_reminder(&mut self, reminder: Reminder) -> Result<(), String> {
        self.conn.execute(
            "INSERT INTO reminders (id, title, time, category, completed) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![reminder.id, reminder.title, reminder.time, reminder.category, reminder.completed as i32],
        ).map_err(|e| format!("{}", e))?;
        
        Ok(())
    }
    
    fn update_reminder(&mut self, id: &str, reminder: Reminder) -> Result<(), String> {
        self.conn.execute(
            "UPDATE reminders SET title = ?1, time = ?2, category = ?3, completed = ?4 WHERE id = ?5",
            params![reminder.title, reminder.time, reminder.category, reminder.completed as i32, id],
        ).map_err(|e| format!("{}", e))?;
        
        Ok(())
    }
    
    fn delete_reminder(&mut self, id: &str) -> Result<(), String> {
        self.conn.execute("DELETE FROM reminders WHERE id = ?", params![id])
            .map_err(|e| format!("{}", e))?;
        
        Ok(())
    }
    
    fn get_history(&self) -> Result<Vec<HistoryItem>, String> {
        let mut stmt = self.conn.prepare("SELECT id, action, target_type, target_id, target_title, detail, timestamp FROM history ORDER BY timestamp DESC LIMIT 500")
            .map_err(|e| format!("{}", e))?;
        
        let history: Vec<HistoryItem> = stmt.query_map([], |row| {
            Ok(HistoryItem {
                id: row.get(0)?,
                action: row.get(1)?,
                target_type: row.get(2)?,
                target_id: row.get(3)?,
                target_title: row.get(4)?,
                detail: row.get(5)?,
                timestamp: row.get(6)?,
            })
        })
        .map_err(|e| format!("{}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{}", e))?;
        
        Ok(history)
    }
    
    fn get_history_by_date(&self, date: &str) -> Result<Vec<HistoryItem>, String> {
        let mut stmt = self.conn.prepare("SELECT id, action, target_type, target_id, target_title, detail, timestamp FROM history WHERE timestamp LIKE ? ORDER BY timestamp DESC")
            .map_err(|e| format!("{}", e))?;
        
        let pattern = format!("{}%", date);
        let history: Vec<HistoryItem> = stmt.query_map(params![pattern], |row| {
            Ok(HistoryItem {
                id: row.get(0)?,
                action: row.get(1)?,
                target_type: row.get(2)?,
                target_id: row.get(3)?,
                target_title: row.get(4)?,
                detail: row.get(5)?,
                timestamp: row.get(6)?,
            })
        })
        .map_err(|e| format!("{}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{}", e))?;
        
        Ok(history)
    }
    
    fn get_history_by_type(&self, target_type: &str) -> Result<Vec<HistoryItem>, String> {
        let mut stmt = self.conn.prepare("SELECT id, action, target_type, target_id, target_title, detail, timestamp FROM history WHERE target_type = ? ORDER BY timestamp DESC LIMIT 500")
            .map_err(|e| format!("{}", e))?;
        
        let history: Vec<HistoryItem> = stmt.query_map(params![target_type], |row| {
            Ok(HistoryItem {
                id: row.get(0)?,
                action: row.get(1)?,
                target_type: row.get(2)?,
                target_id: row.get(3)?,
                target_title: row.get(4)?,
                detail: row.get(5)?,
                timestamp: row.get(6)?,
            })
        })
        .map_err(|e| format!("{}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{}", e))?;
        
        Ok(history)
    }
    
    fn get_history_by_action(&self, action: &str) -> Result<Vec<HistoryItem>, String> {
        let mut stmt = self.conn.prepare("SELECT id, action, target_type, target_id, target_title, detail, timestamp FROM history WHERE action = ? ORDER BY timestamp DESC LIMIT 500")
            .map_err(|e| format!("{}", e))?;
        
        let history: Vec<HistoryItem> = stmt.query_map(params![action], |row| {
            Ok(HistoryItem {
                id: row.get(0)?,
                action: row.get(1)?,
                target_type: row.get(2)?,
                target_id: row.get(3)?,
                target_title: row.get(4)?,
                detail: row.get(5)?,
                timestamp: row.get(6)?,
            })
        })
        .map_err(|e| format!("{}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("{}", e))?;
        
        Ok(history)
    }
    
    fn add_history_item(&mut self, item: HistoryItem) -> Result<(), String> {
        self.conn.execute(
            "INSERT INTO history (id, action, target_type, target_id, target_title, detail, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![item.id, item.action, item.target_type, item.target_id, item.target_title, item.detail, item.timestamp],
        ).map_err(|e| format!("{}", e))?;
        
        self.conn.execute("DELETE FROM history WHERE id NOT IN (SELECT id FROM history ORDER BY timestamp DESC LIMIT 500)", [])
            .map_err(|e| format!("{}", e))?;
        
        Ok(())
    }
    
    fn clear_history(&mut self) -> Result<(), String> {
        self.conn.execute("DELETE FROM history", [])
            .map_err(|e| format!("{}", e))?;
        
        Ok(())
    }
    
    fn delete_history_item(&mut self, id: &str) -> Result<(), String> {
        self.conn.execute("DELETE FROM history WHERE id = ?", params![id])
            .map_err(|e| format!("{}", e))?;
        
        Ok(())
    }
    
    fn get_theme(&self) -> Result<String, String> {
        let theme: String = self.conn.query_row(
            "SELECT value FROM app_config WHERE key = 'theme'",
            [],
            |row| row.get(0)
        ).unwrap_or_else(|_| "light".to_string());
        
        Ok(theme)
    }
    
    fn set_theme(&mut self, theme: &str) -> Result<(), String> {
        self.conn.execute(
            "REPLACE INTO app_config (key, value) VALUES ('theme', ?)",
            params![theme],
        ).map_err(|e| format!("{}", e))?;
        
        Ok(())
    }
    
    fn export_data(&self) -> Result<String, String> {
        let tasks = self.get_tasks()?;
        let reminders = self.get_reminders()?;
        let theme = self.get_theme()?;
        let history = self.get_history()?;
        
        let app_data = AppData {
            tasks,
            reminders,
            theme,
            version: "1.0.0".to_string(),
            history,
        };
        
        serde_json::to_string_pretty(&app_data)
            .map_err(|e| format!("{}", e))
    }
    
    fn import_data(&mut self, json_content: &str) -> Result<(), String> {
        let app_data: AppData = serde_json::from_str(json_content)
            .map_err(|e| format!("{}", e))?;
        
        let tx = self.conn.transaction()
            .map_err(|e| format!("{}", e))?;
        
        tx.execute("DELETE FROM tasks", []).map_err(|e| format!("{}", e))?;
        tx.execute("DELETE FROM subtasks", []).map_err(|e| format!("{}", e))?;
        tx.execute("DELETE FROM notes", []).map_err(|e| format!("{}", e))?;
        tx.execute("DELETE FROM reminders", []).map_err(|e| format!("{}", e))?;
        tx.execute("DELETE FROM history", []).map_err(|e| format!("{}", e))?;
        
        for task in &app_data.tasks {
            tx.execute(
                "INSERT INTO tasks (id, title, description, category, priority, due_date, completed, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![task.id, task.title, task.description, task.category, task.priority, task.due_date, task.completed as i32, task.created_at, task.updated_at],
            ).map_err(|e| format!("{}", e))?;
            
            for subtask in &task.subtasks {
                tx.execute(
                    "INSERT INTO subtasks (id, task_id, title, completed) VALUES (?1, ?2, ?3, ?4)",
                    params![subtask.id, task.id, subtask.title, subtask.completed as i32],
                ).map_err(|e| format!("{}", e))?;
            }
            
            for note in &task.notes {
                tx.execute(
                    "INSERT INTO notes (id, task_id, author, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![note.id, task.id, note.author, note.content, note.created_at],
                ).map_err(|e| format!("{}", e))?;
            }
        }
        
        for reminder in &app_data.reminders {
            tx.execute(
                "INSERT INTO reminders (id, title, time, category, completed) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![reminder.id, reminder.title, reminder.time, reminder.category, reminder.completed as i32],
            ).map_err(|e| format!("{}", e))?;
        }
        
        for item in &app_data.history {
            tx.execute(
                "INSERT INTO history (id, action, target_type, target_id, target_title, detail, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![item.id, item.action, item.target_type, item.target_id, item.target_title, item.detail, item.timestamp],
            ).map_err(|e| format!("{}", e))?;
        }
        
        tx.execute("REPLACE INTO app_config (key, value) VALUES ('theme', ?)", params![app_data.theme])
            .map_err(|e| format!("{}", e))?;
        
        tx.execute("REPLACE INTO app_config (key, value) VALUES ('version', ?)", params![app_data.version])
            .map_err(|e| format!("{}", e))?;
        
        tx.commit()
            .map_err(|e| format!("{}", e))?;
        
        Ok(())
    }
}

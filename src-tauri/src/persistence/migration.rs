use rusqlite::Connection;

pub struct Migration {
    pub version: &'static str,
    pub apply: fn(&mut Connection) -> Result<(), String>,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: "1.0.1",
        apply: |conn| {
            conn.execute(
                "CREATE TABLE IF NOT EXISTS subtasks (
                    id TEXT PRIMARY KEY,
                    task_id TEXT NOT NULL,
                    title TEXT NOT NULL,
                    completed INTEGER DEFAULT 0,
                    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
                )",
                [],
            ).map_err(|e| format!("Failed to create subtasks table: {}", e))
        },
    },
    Migration {
        version: "1.0.2",
        apply: |conn| {
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
            ).map_err(|e| format!("Failed to create notes table: {}", e))
        },
    },
    Migration {
        version: "1.0.3",
        apply: |conn| {
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
            ).map_err(|e| format!("Failed to create history table: {}", e))
        },
    },
];

pub fn run_migrations(conn: &mut Connection) -> Result<String, String> {
    let current_version = get_current_version(conn);
    
    for migration in MIGRATIONS {
        if migration.version > current_version {
            if let Err(e) = (migration.apply)(conn) {
                return Err(format!("Migration {} failed: {}", migration.version, e));
            }
            set_version(conn, migration.version);
        }
    }
    
    Ok(get_current_version(conn))
}

fn get_current_version(conn: &Connection) -> String {
    conn.query_row(
        "SELECT value FROM app_config WHERE key = 'version'",
        [],
        |row| row.get(0)
    ).unwrap_or_else(|_| "1.0.0".to_string())
}

fn set_version(conn: &Connection, version: &str) {
    let _ = conn.execute(
        "REPLACE INTO app_config (key, value) VALUES ('version', ?)",
        [version],
    );
}

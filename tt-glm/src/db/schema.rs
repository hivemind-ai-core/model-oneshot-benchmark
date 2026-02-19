//! Database schema and row types.

use crate::db::Connection as DbConnection;
use crate::error::Result;
use rusqlite::Row;

/// Schema version and management.
pub struct Schema;

impl Schema {
    /// Current schema version.
    pub const VERSION: i32 = 1;

    /// Initialize the database schema.
    ///
    /// Creates all tables, indexes, and constraints.
    /// Returns an error if the database is already initialized.
    pub fn init(conn: &mut DbConnection) -> Result<()> {
        // Check if already initialized by looking for the tasks table
        {
            let mut check =
                conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='tasks'")?;
            let exists = check.exists(())?;
            drop(check); // Explicitly drop to release the borrow

            if exists {
                return Err(crate::error::Error::AlreadyInitialized);
            }
        }

        conn.execute_pragma("PRAGMA foreign_keys = ON", &[])?;
        conn.execute_pragma("PRAGMA journal_mode = WAL", &[])?;

        conn.execute(
            "CREATE TABLE tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                description TEXT,
                dod TEXT,
                status TEXT NOT NULL DEFAULT 'pending'
                    CHECK(status IN ('pending', 'in_progress', 'completed', 'blocked')),
                manual_order REAL NOT NULL DEFAULT 0.0,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now')),
                started_at TEXT,
                completed_at TEXT,
                last_touched_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now'))
            )",
            &[],
        )?;

        conn.execute(
            "CREATE TABLE dependencies (
                task_id INTEGER NOT NULL,
                depends_on INTEGER NOT NULL,
                PRIMARY KEY (task_id, depends_on),
                FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE,
                FOREIGN KEY (depends_on) REFERENCES tasks(id) ON DELETE CASCADE,
                CHECK(task_id != depends_on)
            )",
            &[],
        )?;

        conn.execute(
            "CREATE TABLE artifacts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                file_path TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now')),
                FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
            )",
            &[],
        )?;

        conn.execute(
            "CREATE TABLE config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            &[],
        )?;

        conn.execute("CREATE INDEX idx_tasks_status ON tasks(status)", &[])?;
        conn.execute(
            "CREATE INDEX idx_tasks_manual_order ON tasks(manual_order)",
            &[],
        )?;
        conn.execute(
            "CREATE INDEX idx_dependencies_task_id ON dependencies(task_id)",
            &[],
        )?;
        conn.execute(
            "CREATE INDEX idx_dependencies_depends_on ON dependencies(depends_on)",
            &[],
        )?;
        conn.execute(
            "CREATE INDEX idx_artifacts_task_id ON artifacts(task_id)",
            &[],
        )?;

        Ok(())
    }

    /// Check if the database schema is valid (already initialized).
    pub fn is_initialized(conn: &mut DbConnection) -> bool {
        conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='tasks'")
            .and_then(|mut stmt| Ok(stmt.exists(())?))
            .unwrap_or(false)
    }
}

/// Row representation of a task from the database.
#[derive(Debug, Clone)]
pub struct TaskRow {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub dod: Option<String>,
    pub status: String,
    pub manual_order: f64,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub last_touched_at: String,
}

impl TaskRow {
    /// Create a TaskRow from a SQLite row.
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            title: row.get("title")?,
            description: row.get("description")?,
            dod: row.get("dod")?,
            status: row.get("status")?,
            manual_order: row.get("manual_order")?,
            created_at: row.get("created_at")?,
            started_at: row.get("started_at")?,
            completed_at: row.get("completed_at")?,
            last_touched_at: row.get("last_touched_at")?,
        })
    }
}

/// Row representation of a dependency from the database.
#[derive(Debug, Clone)]
pub struct DependencyRow {
    pub task_id: i64,
    pub depends_on: i64,
}

impl DependencyRow {
    /// Create a DependencyRow from a SQLite row.
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            task_id: row.get("task_id")?,
            depends_on: row.get("depends_on")?,
        })
    }
}

/// Row representation of an artifact from the database.
#[derive(Debug, Clone)]
pub struct ArtifactRow {
    pub id: i64,
    pub task_id: i64,
    pub name: String,
    pub file_path: String,
    pub created_at: String,
}

impl ArtifactRow {
    /// Create an ArtifactRow from a SQLite row.
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            task_id: row.get("task_id")?,
            name: row.get("name")?,
            file_path: row.get("file_path")?,
            created_at: row.get("created_at")?,
        })
    }
}

/// Row representation of a config value from the database.
#[derive(Debug, Clone)]
pub struct ConfigRow {
    pub key: String,
    pub value: String,
}

impl ConfigRow {
    /// Create a ConfigRow from a SQLite row.
    pub fn from_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            key: row.get("key")?,
            value: row.get("value")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_temp_db() -> DbConnection {
        DbConnection::open_in_memory().unwrap()
    }

    #[test]
    fn test_schema_init_creates_tables() {
        let mut conn = create_temp_db();
        Schema::init(&mut conn).unwrap();

        // Check tasks table exists
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='tasks'")
            .unwrap();
        assert!(stmt.exists(()).unwrap());
        drop(stmt);

        // Check dependencies table exists
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='dependencies'")
            .unwrap();
        assert!(stmt.exists(()).unwrap());
        drop(stmt);

        // Check artifacts table exists
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='artifacts'")
            .unwrap();
        assert!(stmt.exists(()).unwrap());
        drop(stmt);

        // Check config table exists
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='config'")
            .unwrap();
        assert!(stmt.exists(()).unwrap());
    }

    #[test]
    fn test_schema_init_fails_if_already_initialized() {
        let mut conn = create_temp_db();
        Schema::init(&mut conn).unwrap();
        assert!(matches!(
            Schema::init(&mut conn).unwrap_err(),
            crate::error::Error::AlreadyInitialized
        ));
    }

    #[test]
    fn test_schema_enables_foreign_keys() {
        let mut conn = create_temp_db();
        Schema::init(&mut conn).unwrap();

        // PRAGMA foreign_keys returns an integer
        let fk_status: i64 = conn
            .query_row("PRAGMA foreign_keys", &[], |row| row.get(0))
            .unwrap();
        assert_eq!(fk_status, 1);
    }

    #[test]
    fn test_schema_enables_wal_mode() {
        let mut conn = create_temp_db();
        Schema::init(&mut conn).unwrap();

        // In-memory databases may not support WAL, but the setting should be accepted
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", &[], |row| row.get(0))
            .unwrap();
        // For in-memory databases, this might be "memory" instead of "wal"
        assert!(journal_mode.to_lowercase() == "wal" || journal_mode.to_lowercase() == "memory");
    }

    #[test]
    fn test_task_status_check_constraint() {
        let mut conn = create_temp_db();
        Schema::init(&mut conn).unwrap();

        // Valid status should work
        conn.execute(
            "INSERT INTO tasks (title, status) VALUES (?, ?)",
            &[
                &"Test Task" as &dyn rusqlite::ToSql,
                &"pending" as &dyn rusqlite::ToSql,
            ],
        )
        .unwrap();

        // Invalid status should fail
        let result = conn.execute(
            "INSERT INTO tasks (title, status) VALUES (?, ?)",
            &[
                &"Bad Task" as &dyn rusqlite::ToSql,
                &"invalid_status" as &dyn rusqlite::ToSql,
            ],
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_dependency_no_self_reference() {
        let mut conn = create_temp_db();
        Schema::init(&mut conn).unwrap();

        // Create a task
        conn.execute(
            "INSERT INTO tasks (title) VALUES (?)",
            &[&"Task 1" as &dyn rusqlite::ToSql],
        )
        .unwrap();

        // Try to create self-dependency
        let result = conn.execute(
            "INSERT INTO dependencies (task_id, depends_on) VALUES (?, ?)",
            &[&1i64 as &dyn rusqlite::ToSql, &1i64 as &dyn rusqlite::ToSql],
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_is_initialized() {
        let mut conn = create_temp_db();
        assert!(!Schema::is_initialized(&mut conn));

        Schema::init(&mut conn).unwrap();
        assert!(Schema::is_initialized(&mut conn));
    }

    #[test]
    fn test_task_row_from_row() {
        let mut conn = create_temp_db();
        Schema::init(&mut conn).unwrap();

        conn.execute(
            "INSERT INTO tasks (title, description, dod, status, manual_order) VALUES (?, ?, ?, ?, ?)",
            &[&"Test Task" as &dyn rusqlite::ToSql, &"desc" as &dyn rusqlite::ToSql, &"dod" as &dyn rusqlite::ToSql, &"pending" as &dyn rusqlite::ToSql, &10.0f64 as &dyn rusqlite::ToSql],
        )
        .unwrap();

        let row = conn
            .query_row("SELECT * FROM tasks WHERE id = 1", &[], |r| {
                Ok(TaskRow::from_row(r))
            })
            .unwrap()
            .unwrap();

        assert_eq!(row.id, 1);
        assert_eq!(row.title, "Test Task");
        assert_eq!(row.description.as_deref(), Some("desc"));
        assert_eq!(row.dod.as_deref(), Some("dod"));
        assert_eq!(row.status, "pending");
        assert_eq!(row.manual_order, 10.0);
    }
}

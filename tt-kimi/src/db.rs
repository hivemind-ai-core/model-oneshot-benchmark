use crate::error::{Result, TaskError};
use crate::models::{Artifact, Dependency, Status, Task};
use chrono::{DateTime, NaiveDateTime, Utc};
use rusqlite::{Connection, OptionalExtension, Row};
use std::path::Path;

/// Database handle
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open database connection
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;

        // Enable foreign keys
        conn.pragma_update(None, "foreign_keys", "ON")?;

        // Enable WAL mode for better concurrency
        conn.pragma_update(None, "journal_mode", "WAL")?;

        Ok(Database { conn })
    }

    /// Open database in current directory (tt.db)
    pub fn open_current_dir() -> Result<Self> {
        Self::open("tt.db")
    }

    /// Initialize the database schema
    pub fn init(&self) -> Result<()> {
        self.create_tables()?;
        self.create_indexes()?;
        Ok(())
    }

    fn create_tables(&self) -> Result<()> {
        // Tasks table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                description TEXT,
                dod TEXT,
                status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'in_progress', 'completed', 'blocked')),
                manual_order REAL NOT NULL DEFAULT 0.0,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now')),
                started_at TEXT,
                completed_at TEXT,
                last_touched_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now'))
            )",
            [],
        )?;

        // Dependencies table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS dependencies (
                task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                depends_on INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                PRIMARY KEY (task_id, depends_on),
                CHECK (task_id != depends_on)
            )",
            [],
        )?;

        // Artifacts table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS artifacts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                file_path TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now'))
            )",
            [],
        )?;

        // Config table
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )?;

        Ok(())
    }

    fn create_indexes(&self) -> Result<()> {
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_tasks_manual_order ON tasks(manual_order)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_dependencies_task_id ON dependencies(task_id)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_dependencies_depends_on ON dependencies(depends_on)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_artifacts_task_id ON artifacts(task_id)",
            [],
        )?;
        Ok(())
    }

    /// Check if database is initialized
    pub fn is_initialized(&self) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='tasks'",
            [],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    // ==================== Task Operations ====================

    pub fn create_task(
        &self,
        title: &str,
        description: Option<&str>,
        dod: Option<&str>,
        manual_order: f64,
    ) -> Result<Task> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        self.conn.execute(
            "INSERT INTO tasks (title, description, dod, manual_order, created_at, last_touched_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            (title, description, dod, manual_order, &now_str),
        )?;

        let id = self.conn.last_insert_rowid();
        self.get_task(id).map(|t| t.unwrap())
    }

    pub fn get_task(&self, id: i64) -> Result<Option<Task>> {
        self.conn
            .query_row(
                "SELECT id, title, description, dod, status, manual_order, 
                        created_at, started_at, completed_at, last_touched_at
                 FROM tasks WHERE id = ?1",
                [id],
                task_from_row,
            )
            .optional()
            .map_err(|e| e.into())
    }

    pub fn get_all_tasks(&self) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, dod, status, manual_order,
                    created_at, started_at, completed_at, last_touched_at
             FROM tasks
             ORDER BY manual_order",
        )?;

        let tasks = stmt.query_map([], task_from_row)?;
        tasks
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    pub fn update_task(
        &self,
        id: i64,
        title: Option<&str>,
        description: Option<Option<&str>>,
        dod: Option<Option<&str>>,
    ) -> Result<Task> {
        let now = Utc::now().to_rfc3339();

        if let Some(t) = title {
            self.conn.execute(
                "UPDATE tasks SET title = ?1, last_touched_at = ?2 WHERE id = ?3",
                (t, &now, id),
            )?;
        }

        if let Some(d) = description {
            self.conn.execute(
                "UPDATE tasks SET description = ?1, last_touched_at = ?2 WHERE id = ?3",
                (d, &now, id),
            )?;
        }

        if let Some(d) = dod {
            self.conn.execute(
                "UPDATE tasks SET dod = ?1, last_touched_at = ?2 WHERE id = ?3",
                (d, &now, id),
            )?;
        }

        self.get_task(id)?.ok_or(TaskError::TaskNotFound(id))
    }

    pub fn set_task_status(&self, id: i64, status: Status) -> Result<Task> {
        let now = Utc::now().to_rfc3339();

        match status {
            Status::InProgress => {
                self.conn.execute(
                    "UPDATE tasks SET status = 'in_progress', started_at = ?1, last_touched_at = ?1 WHERE id = ?2",
                    (&now, id),
                )?;
            }
            Status::Completed => {
                self.conn.execute(
                    "UPDATE tasks SET status = 'completed', completed_at = ?1, last_touched_at = ?1 WHERE id = ?2",
                    (&now, id),
                )?;
            }
            _ => {
                self.conn.execute(
                    "UPDATE tasks SET status = ?1, last_touched_at = ?2 WHERE id = ?3",
                    (status.as_str(), &now, id),
                )?;
            }
        }

        self.get_task(id)?.ok_or(TaskError::TaskNotFound(id))
    }

    pub fn get_active_task(&self) -> Result<Option<Task>> {
        self.conn
            .query_row(
                "SELECT id, title, description, dod, status, manual_order,
                        created_at, started_at, completed_at, last_touched_at
                 FROM tasks WHERE status = 'in_progress'
                 LIMIT 1",
                [],
                task_from_row,
            )
            .optional()
            .map_err(|e| e.into())
    }

    pub fn clear_active_task(&self) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE tasks SET status = 'blocked', last_touched_at = ?1 WHERE status = 'in_progress'",
            [&now],
        )?;
        Ok(())
    }

    pub fn update_manual_order(&self, id: i64, order: f64) -> Result<Task> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE tasks SET manual_order = ?1, last_touched_at = ?2 WHERE id = ?3",
            (order, &now, id),
        )?;
        self.get_task(id)?.ok_or(TaskError::TaskNotFound(id))
    }

    pub fn get_max_manual_order(&self) -> Result<f64> {
        let result: Option<f64> = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(manual_order), 0.0) FROM tasks",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(result.unwrap_or(0.0))
    }

    pub fn get_all_tasks_with_order(&self) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, dod, status, manual_order,
                    created_at, started_at, completed_at, last_touched_at
             FROM tasks
             ORDER BY manual_order, id",
        )?;

        let tasks = stmt.query_map([], task_from_row)?;
        tasks
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    // ==================== Dependency Operations ====================

    pub fn add_dependency(&self, task_id: i64, depends_on: i64) -> Result<()> {
        if task_id == depends_on {
            return Err(TaskError::SelfDependency);
        }
        self.conn.execute(
            "INSERT INTO dependencies (task_id, depends_on) VALUES (?1, ?2)",
            (task_id, depends_on),
        )?;
        Ok(())
    }

    pub fn remove_dependency(&self, task_id: i64, depends_on: i64) -> Result<()> {
        let rows = self.conn.execute(
            "DELETE FROM dependencies WHERE task_id = ?1 AND depends_on = ?2",
            (task_id, depends_on),
        )?;
        if rows == 0 {
            return Err(TaskError::DependencyNotFound);
        }
        Ok(())
    }

    pub fn get_dependencies(&self, task_id: i64) -> Result<Vec<Dependency>> {
        let mut stmt = self
            .conn
            .prepare("SELECT task_id, depends_on FROM dependencies WHERE task_id = ?1")?;

        let deps = stmt.query_map([task_id], |row| {
            Ok(Dependency {
                task_id: row.get(0)?,
                depends_on: row.get(1)?,
            })
        })?;

        deps.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    pub fn get_dependency_statuses(&self, task_id: i64) -> Result<Vec<(i64, String, Status)>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.title, t.status 
             FROM tasks t
             JOIN dependencies d ON t.id = d.depends_on
             WHERE d.task_id = ?1",
        )?;

        let results = stmt.query_map([task_id], |row| {
            let status_str: String = row.get(2)?;
            let status = Status::try_from(status_str.as_str()).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
                )
            })?;
            Ok((row.get(0)?, row.get(1)?, status))
        })?;

        results
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    pub fn get_dependents(&self, task_id: i64) -> Result<Vec<i64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT task_id FROM dependencies WHERE depends_on = ?1")?;

        let ids = stmt.query_map([task_id], |row| row.get::<_, i64>(0))?;

        ids.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    pub fn get_all_dependencies(&self) -> Result<Vec<Dependency>> {
        let mut stmt = self
            .conn
            .prepare("SELECT task_id, depends_on FROM dependencies")?;

        let deps = stmt.query_map([], |row| {
            Ok(Dependency {
                task_id: row.get(0)?,
                depends_on: row.get(1)?,
            })
        })?;

        deps.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    // ==================== Artifact Operations ====================

    pub fn create_artifact(&self, task_id: i64, name: &str, file_path: &str) -> Result<Artifact> {
        self.conn.execute(
            "INSERT INTO artifacts (task_id, name, file_path) VALUES (?1, ?2, ?3)",
            (task_id, name, file_path),
        )?;

        let id = self.conn.last_insert_rowid();
        self.get_artifact(id).map(|a| a.unwrap())
    }

    pub fn get_artifact(&self, id: i64) -> Result<Option<Artifact>> {
        self.conn
            .query_row(
                "SELECT id, task_id, name, file_path, created_at FROM artifacts WHERE id = ?1",
                [id],
                artifact_from_row,
            )
            .optional()
            .map_err(|e| e.into())
    }

    pub fn get_artifacts_for_task(&self, task_id: i64) -> Result<Vec<Artifact>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, task_id, name, file_path, created_at 
             FROM artifacts 
             WHERE task_id = ?1
             ORDER BY created_at",
        )?;

        let artifacts = stmt.query_map([task_id], artifact_from_row)?;
        artifacts
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    // ==================== Config Operations ====================

    pub fn get_config(&self, key: &str) -> Result<Option<String>> {
        self.conn
            .query_row("SELECT value FROM config WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|e| e.into())
    }

    pub fn set_config(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO config (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            (key, value),
        )?;
        Ok(())
    }

    pub fn delete_config(&self, key: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM config WHERE key = ?1", [key])?;
        Ok(())
    }

    // ==================== Target Walk ====================

    /// Get the transitive dependencies of a target (the active subgraph)
    /// Note: This includes ALL tasks in the subgraph (including completed)
    /// The caller should filter as needed
    pub fn get_target_subgraph(&self, target_id: i64) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "WITH RECURSIVE subgraph(id) AS (
                SELECT ?1
                UNION
                SELECT d.depends_on
                FROM dependencies d
                JOIN subgraph s ON d.task_id = s.id
            )
            SELECT t.id, t.title, t.description, t.dod, t.status, t.manual_order,
                   t.created_at, t.started_at, t.completed_at, t.last_touched_at
            FROM tasks t
            JOIN subgraph s ON t.id = s.id",
        )?;

        let tasks = stmt.query_map([target_id], task_from_row)?;
        tasks
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }

    pub fn get_target_subgraph_with_deps(&self, target_id: i64) -> Result<Vec<(Task, Vec<i64>)>> {
        let tasks = self.get_target_subgraph(target_id)?;
        let mut result = Vec::new();

        for task in tasks {
            let deps = self.get_dependencies(task.id)?;
            let dep_ids: Vec<i64> = deps.into_iter().map(|d| d.depends_on).collect();
            result.push((task, dep_ids));
        }

        Ok(result)
    }

    /// Get all tasks in the target subgraph including completed ones
    pub fn get_full_target_subgraph(&self, target_id: i64) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "WITH RECURSIVE subgraph(id) AS (
                SELECT ?1
                UNION
                SELECT d.depends_on
                FROM dependencies d
                JOIN subgraph s ON d.task_id = s.id
            )
            SELECT t.id, t.title, t.description, t.dod, t.status, t.manual_order,
                   t.created_at, t.started_at, t.completed_at, t.last_touched_at
            FROM tasks t
            JOIN subgraph s ON t.id = s.id",
        )?;

        let tasks = stmt.query_map([target_id], task_from_row)?;
        tasks
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.into())
    }
}

// ==================== Row Parsers ====================

fn task_from_row(row: &Row) -> std::result::Result<Task, rusqlite::Error> {
    let status_str: String = row.get(4)?;
    let status = Status::try_from(status_str.as_str()).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            4,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
        )
    })?;

    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        dod: row.get(3)?,
        status,
        manual_order: row.get(5)?,
        created_at: parse_datetime(row.get(6)?)?,
        started_at: row
            .get::<_, Option<String>>(7)?
            .map(parse_datetime)
            .transpose()?,
        completed_at: row
            .get::<_, Option<String>>(8)?
            .map(parse_datetime)
            .transpose()?,
        last_touched_at: parse_datetime(row.get(9)?)?,
    })
}

fn artifact_from_row(row: &Row) -> std::result::Result<Artifact, rusqlite::Error> {
    Ok(Artifact {
        id: row.get(0)?,
        task_id: row.get(1)?,
        name: row.get(2)?,
        file_path: row.get(3)?,
        created_at: parse_datetime(row.get(4)?)?,
    })
}

fn parse_datetime(s: String) -> std::result::Result<DateTime<Utc>, rusqlite::Error> {
    // Try formats with timezone first
    if let Ok(dt) = DateTime::parse_from_rfc3339(&s) {
        return Ok(dt.with_timezone(&Utc));
    }
    if let Ok(dt) = DateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S%.f%:z") {
        return Ok(dt.with_timezone(&Utc));
    }
    if let Ok(dt) = DateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S%:z") {
        return Ok(dt.with_timezone(&Utc));
    }
    // Then try naive datetime formats (assume UTC)
    if let Ok(ndt) = NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S%.f") {
        return Ok(DateTime::from_naive_utc_and_offset(ndt, Utc));
    }
    if let Ok(ndt) = NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S") {
        return Ok(DateTime::from_naive_utc_and_offset(ndt, Utc));
    }
    if let Ok(ndt) = NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S") {
        return Ok(DateTime::from_naive_utc_and_offset(ndt, Utc));
    }
    Err(rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Cannot parse datetime: {s}"),
        )),
    ))
}

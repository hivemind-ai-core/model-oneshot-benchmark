use crate::core::error::{TTError, TTResult};
use crate::core::models::{Artifact, Dependency, Task, TaskStatus};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};

use std::path::Path;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new<P: AsRef<Path>>(path: P) -> TTResult<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;",
        )?;
        Ok(Self { conn })
    }

    pub fn init_schema(&self) -> TTResult<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                description TEXT,
                dod TEXT,
                status TEXT NOT NULL CHECK(status IN ('pending', 'in_progress', 'completed', 'blocked')),
                manual_order REAL NOT NULL,
                created_at TEXT NOT NULL,
                started_at TEXT,
                completed_at TEXT,
                last_touched_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS dependencies (
                task_id INTEGER NOT NULL,
                depends_on INTEGER NOT NULL,
                PRIMARY KEY (task_id, depends_on),
                CHECK (task_id != depends_on),
                FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE,
                FOREIGN KEY (depends_on) REFERENCES tasks(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS artifacts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                file_path TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
            CREATE INDEX IF NOT EXISTS idx_tasks_manual_order ON tasks(manual_order);
            CREATE INDEX IF NOT EXISTS idx_deps_task_id ON dependencies(task_id);
            CREATE INDEX IF NOT EXISTS idx_deps_depends_on ON dependencies(depends_on);
            CREATE INDEX IF NOT EXISTS idx_artifacts_task_id ON artifacts(task_id);
            
            CREATE INDEX IF NOT EXISTS idx_deps_composite ON dependencies(task_id, depends_on);
            
            CREATE TRIGGER IF NOT EXISTS update_last_touched
            AFTER UPDATE ON tasks
            BEGIN
                UPDATE tasks SET last_touched_at = strftime('%Y-%m-%dT%H:%M:%S', 'now')
                WHERE id = NEW.id;
            END;",
        )?;
        Ok(())
    }

    // Task operations

    pub fn create_task(
        &self,
        title: &str,
        description: Option<&str>,
        dod: Option<&str>,
        manual_order: f64,
    ) -> TTResult<Task> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();

        self.conn.execute(
            "INSERT INTO tasks (title, description, dod, status, manual_order, created_at, last_touched_at)
             VALUES (?1, ?2, ?3, 'pending', ?4, ?5, ?5)",
            params![title, description, dod, manual_order, now_str],
        )?;

        let id = self.conn.last_insert_rowid();
        self.get_task(id)?.ok_or(TTError::TaskNotFound(id))
    }

    pub fn get_task(&self, id: i64) -> TTResult<Option<Task>> {
        self.conn
            .query_row(
                "SELECT id, title, description, dod, status, manual_order, 
                        created_at, started_at, completed_at, last_touched_at
                 FROM tasks WHERE id = ?1",
                [id],
                |row| self.row_to_task(row),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn get_all_tasks(&self) -> TTResult<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, dod, status, manual_order, 
                    created_at, started_at, completed_at, last_touched_at
             FROM tasks ORDER BY manual_order",
        )?;

        let tasks: Result<Vec<_>, _> = stmt.query_map([], |row| self.row_to_task(row))?.collect();

        Ok(tasks?)
    }

    pub fn get_tasks_by_status(&self, status: TaskStatus) -> TTResult<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, dod, status, manual_order, 
                    created_at, started_at, completed_at, last_touched_at
             FROM tasks WHERE status = ?1 ORDER BY manual_order",
        )?;

        let tasks: Result<Vec<_>, _> = stmt
            .query_map([status.as_str()], |row| self.row_to_task(row))?
            .collect();

        Ok(tasks?)
    }

    pub fn get_active_task(&self) -> TTResult<Option<Task>> {
        self.conn
            .query_row(
                "SELECT id, title, description, dod, status, manual_order, 
                        created_at, started_at, completed_at, last_touched_at
                 FROM tasks WHERE status = 'in_progress' LIMIT 1",
                [],
                |row| self.row_to_task(row),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn update_task_status(&self, id: i64, status: TaskStatus) -> TTResult<()> {
        let now = Utc::now().to_rfc3339();

        let (started_at, completed_at) = match status {
            TaskStatus::InProgress => (Some(now.clone()), None),
            TaskStatus::Completed => {
                let started: Option<String> = self
                    .conn
                    .query_row("SELECT started_at FROM tasks WHERE id = ?1", [id], |row| {
                        row.get(0)
                    })
                    .optional()?
                    .flatten();
                (started, Some(now.clone()))
            }
            _ => (None, None),
        };

        self.conn.execute(
            "UPDATE tasks 
             SET status = ?1, 
                 started_at = COALESCE(?2, started_at),
                 completed_at = ?3,
                 last_touched_at = ?4
             WHERE id = ?5",
            params![status.as_str(), started_at, completed_at, now, id],
        )?;

        Ok(())
    }

    pub fn update_task_fields(
        &self,
        id: i64,
        title: Option<&str>,
        description: Option<&str>,
        dod: Option<&str>,
    ) -> TTResult<()> {
        let now = Utc::now().to_rfc3339();

        if let Some(title) = title {
            self.conn.execute(
                "UPDATE tasks SET title = ?1, last_touched_at = ?2 WHERE id = ?3",
                params![title, now, id],
            )?;
        }

        if let Some(desc) = description {
            self.conn.execute(
                "UPDATE tasks SET description = ?1, last_touched_at = ?2 WHERE id = ?3",
                params![desc, now, id],
            )?;
        }

        if let Some(dod) = dod {
            self.conn.execute(
                "UPDATE tasks SET dod = ?1, last_touched_at = ?2 WHERE id = ?3",
                params![dod, now, id],
            )?;
        }

        Ok(())
    }

    pub fn update_manual_order(&self, id: i64, order: f64) -> TTResult<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE tasks SET manual_order = ?1, last_touched_at = ?2 WHERE id = ?3",
            params![order, now, id],
        )?;
        Ok(())
    }

    pub fn get_max_manual_order(&self) -> TTResult<f64> {
        let max: Option<f64> =
            self.conn
                .query_row("SELECT MAX(manual_order) FROM tasks", [], |row| {
                    let val: Option<f64> = row.get(0)?;
                    Ok(val)
                })?;
        Ok(max.unwrap_or(0.0))
    }

    // Dependency operations

    pub fn add_dependency(&self, task_id: i64, depends_on: i64) -> TTResult<()> {
        self.conn.execute(
            "INSERT INTO dependencies (task_id, depends_on) VALUES (?1, ?2)",
            params![task_id, depends_on],
        )?;
        Ok(())
    }

    pub fn remove_dependency(&self, task_id: i64, depends_on: i64) -> TTResult<()> {
        self.conn.execute(
            "DELETE FROM dependencies WHERE task_id = ?1 AND depends_on = ?2",
            params![task_id, depends_on],
        )?;
        Ok(())
    }

    pub fn get_dependencies(&self, task_id: i64) -> TTResult<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.title, t.description, t.dod, t.status, t.manual_order, 
                    t.created_at, t.started_at, t.completed_at, t.last_touched_at
             FROM tasks t
             JOIN dependencies d ON t.id = d.depends_on
             WHERE d.task_id = ?1
             ORDER BY t.manual_order",
        )?;

        let tasks: Result<Vec<_>, _> = stmt
            .query_map([task_id], |row| self.row_to_task(row))?
            .collect();

        Ok(tasks?)
    }

    pub fn get_dependency_ids(&self, task_id: i64) -> TTResult<Vec<i64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT depends_on FROM dependencies WHERE task_id = ?1")?;

        let ids: Result<Vec<_>, _> = stmt
            .query_map([task_id], |row| row.get::<_, i64>(0))?
            .collect();

        Ok(ids?)
    }

    pub fn get_dependents(&self, task_id: i64) -> TTResult<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT t.id, t.title, t.description, t.dod, t.status, t.manual_order, 
                    t.created_at, t.started_at, t.completed_at, t.last_touched_at
             FROM tasks t
             JOIN dependencies d ON t.id = d.task_id
             WHERE d.depends_on = ?1
             ORDER BY t.manual_order",
        )?;

        let tasks: Result<Vec<_>, _> = stmt
            .query_map([task_id], |row| self.row_to_task(row))?
            .collect();

        Ok(tasks?)
    }

    pub fn get_dependent_ids(&self, task_id: i64) -> TTResult<Vec<i64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT task_id FROM dependencies WHERE depends_on = ?1")?;

        let ids: Result<Vec<_>, _> = stmt
            .query_map([task_id], |row| row.get::<_, i64>(0))?
            .collect();

        Ok(ids?)
    }

    pub fn get_all_dependencies(&self) -> TTResult<Vec<Dependency>> {
        let mut stmt = self
            .conn
            .prepare("SELECT task_id, depends_on FROM dependencies")?;

        let deps: Result<Vec<_>, _> = stmt
            .query_map([], |row| {
                Ok(Dependency {
                    task_id: row.get(0)?,
                    depends_on: row.get(1)?,
                })
            })?
            .collect();

        Ok(deps?)
    }

    // Artifact operations

    pub fn create_artifact(&self, task_id: i64, name: &str, file_path: &str) -> TTResult<Artifact> {
        let now = Utc::now().to_rfc3339();

        self.conn.execute(
            "INSERT INTO artifacts (task_id, name, file_path, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![task_id, name, file_path, now],
        )?;

        let id = self.conn.last_insert_rowid();
        self.get_artifact(id)?.ok_or(TTError::TaskNotFound(id))
    }

    pub fn get_artifact(&self, id: i64) -> TTResult<Option<Artifact>> {
        self.conn
            .query_row(
                "SELECT id, task_id, name, file_path, created_at FROM artifacts WHERE id = ?1",
                [id],
                |row| self.row_to_artifact(row),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn get_artifacts_for_task(&self, task_id: i64) -> TTResult<Vec<Artifact>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, task_id, name, file_path, created_at 
             FROM artifacts WHERE task_id = ?1 ORDER BY created_at",
        )?;

        let artifacts: Result<Vec<_>, _> = stmt
            .query_map([task_id], |row| self.row_to_artifact(row))?
            .collect();

        Ok(artifacts?)
    }

    // Config operations

    pub fn set_config(&self, key: &str, value: &str) -> TTResult<()> {
        self.conn.execute(
            "INSERT INTO config (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_config(&self, key: &str) -> TTResult<Option<String>> {
        self.conn
            .query_row("SELECT value FROM config WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()
            .map_err(Into::into)
    }

    // Target operations

    pub fn set_target(&self, task_id: i64) -> TTResult<()> {
        self.set_config("target_id", &task_id.to_string())
    }

    pub fn get_target(&self) -> TTResult<Option<i64>> {
        match self.get_config("target_id")? {
            Some(val) => Ok(Some(val.parse().map_err(|_| {
                TTError::InvalidStatus("Invalid target_id format".to_string())
            })?)),
            None => Ok(None),
        }
    }

    // Subgraph query using recursive CTE

    pub fn get_subgraph_tasks(&self, target_id: i64) -> TTResult<Vec<Task>> {
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
            JOIN subgraph s ON t.id = s.id
            WHERE t.status != 'completed'
            ORDER BY t.manual_order",
        )?;

        let tasks: Result<Vec<_>, _> = stmt
            .query_map([target_id], |row| self.row_to_task(row))?
            .collect();

        Ok(tasks?)
    }

    // Helper methods

    fn row_to_task(&self, row: &Row) -> rusqlite::Result<Task> {
        let status_str: String = row.get(4)?;
        let status = std::str::FromStr::from_str(&status_str).map_err(|e: String| {
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
            created_at: parse_datetime(&row.get::<_, String>(6)?),
            started_at: row
                .get::<_, Option<String>>(7)?
                .as_deref()
                .map(parse_datetime),
            completed_at: row
                .get::<_, Option<String>>(8)?
                .as_deref()
                .map(parse_datetime),
            last_touched_at: parse_datetime(&row.get::<_, String>(9)?),
        })
    }

    fn row_to_artifact(&self, row: &Row) -> rusqlite::Result<Artifact> {
        Ok(Artifact {
            id: row.get(0)?,
            task_id: row.get(1)?,
            name: row.get(2)?,
            file_path: row.get(3)?,
            created_at: parse_datetime(&row.get::<_, String>(4)?),
        })
    }
}

fn parse_datetime(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> Database {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                description TEXT,
                dod TEXT,
                status TEXT NOT NULL CHECK(status IN ('pending', 'in_progress', 'completed', 'blocked')),
                manual_order REAL NOT NULL,
                created_at TEXT NOT NULL,
                started_at TEXT,
                completed_at TEXT,
                last_touched_at TEXT NOT NULL
            );
            CREATE TABLE dependencies (
                task_id INTEGER NOT NULL,
                depends_on INTEGER NOT NULL,
                PRIMARY KEY (task_id, depends_on),
                CHECK (task_id != depends_on),
                FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE,
                FOREIGN KEY (depends_on) REFERENCES tasks(id) ON DELETE CASCADE
            );
            CREATE TABLE artifacts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                file_path TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
            );
            CREATE TABLE config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )
        .unwrap();
        Database { conn }
    }

    #[test]
    fn test_create_and_get_task() {
        let db = setup_test_db();
        let task = db
            .create_task("Test Task", Some("Desc"), Some("DoD"), 10.0)
            .unwrap();

        assert_eq!(task.title, "Test Task");
        assert_eq!(task.description, Some("Desc".to_string()));
        assert_eq!(task.dod, Some("DoD".to_string()));
        assert!(matches!(task.status, TaskStatus::Pending));

        let fetched = db.get_task(task.id).unwrap().unwrap();
        assert_eq!(fetched.title, "Test Task");
    }

    #[test]
    fn test_task_status_transitions() {
        let db = setup_test_db();
        let task = db.create_task("Test", None, None, 10.0).unwrap();

        db.update_task_status(task.id, TaskStatus::InProgress)
            .unwrap();
        let updated = db.get_task(task.id).unwrap().unwrap();
        assert!(matches!(updated.status, TaskStatus::InProgress));
        assert!(updated.started_at.is_some());

        db.update_task_status(task.id, TaskStatus::Completed)
            .unwrap();
        let completed = db.get_task(task.id).unwrap().unwrap();
        assert!(matches!(completed.status, TaskStatus::Completed));
        assert!(completed.completed_at.is_some());
    }

    #[test]
    fn test_dependencies() {
        let db = setup_test_db();
        let t1 = db.create_task("Task 1", None, None, 10.0).unwrap();
        let t2 = db.create_task("Task 2", None, None, 20.0).unwrap();

        db.add_dependency(t2.id, t1.id).unwrap();

        let deps = db.get_dependencies(t2.id).unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].id, t1.id);

        let dependents = db.get_dependents(t1.id).unwrap();
        assert_eq!(dependents.len(), 1);
        assert_eq!(dependents[0].id, t2.id);
    }

    #[test]
    fn test_target_config() {
        let db = setup_test_db();
        assert!(db.get_target().unwrap().is_none());

        db.set_target(42).unwrap();
        assert_eq!(db.get_target().unwrap(), Some(42));
    }
}

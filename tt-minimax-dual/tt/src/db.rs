use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Mutex;

use crate::error::{Error, Result};
use crate::models::{Artifact, Status, Task};

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;

        let db = Self {
            conn: Mutex::new(conn),
        };

        db.init_schema()?;

        Ok(db)
    }

    pub fn exists<P: AsRef<Path>>(path: P) -> bool {
        path.as_ref().exists()
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                description TEXT,
                dod TEXT,
                status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'in_progress', 'completed', 'blocked')),
                manual_order REAL NOT NULL DEFAULT 10.0,
                created_at TEXT NOT NULL,
                started_at TEXT,
                completed_at TEXT,
                last_touched_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS dependencies (
                task_id INTEGER NOT NULL,
                depends_on INTEGER NOT NULL,
                PRIMARY KEY (task_id, depends_on),
                FOREIGN KEY (task_id) REFERENCES tasks(id),
                FOREIGN KEY (depends_on) REFERENCES tasks(id),
                CHECK (task_id != depends_on)
            );

            CREATE TABLE IF NOT EXISTS artifacts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                file_path TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (task_id) REFERENCES tasks(id)
            );

            CREATE TABLE IF NOT EXISTS config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
            CREATE INDEX IF NOT EXISTS idx_tasks_manual_order ON tasks(manual_order);
            CREATE INDEX IF NOT EXISTS idx_dependencies_task_id ON dependencies(task_id);
            CREATE INDEX IF NOT EXISTS idx_dependencies_depends_on ON dependencies(depends_on);
            CREATE INDEX IF NOT EXISTS idx_artifacts_task_id ON artifacts(task_id);
        "#)?;

        Ok(())
    }

    fn now() -> String {
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string()
    }

    pub fn create_task(
        &self,
        title: &str,
        description: Option<&str>,
        dod: Option<&str>,
        after_id: Option<i64>,
        before_id: Option<i64>,
    ) -> Result<Task> {
        let conn = self.conn.lock().unwrap();

        let manual_order = match (after_id, before_id) {
            (Some(after), None) => {
                let order: f64 = conn.query_row(
                    "SELECT manual_order FROM tasks WHERE id = ?",
                    [after],
                    |row| row.get(0),
                )?;
                order + 10.0
            }
            (None, Some(before)) => {
                let order: f64 = conn.query_row(
                    "SELECT manual_order FROM tasks WHERE id = ?",
                    [before],
                    |row| row.get(0),
                )?;
                order - 10.0
            }
            (Some(after), Some(before)) => {
                let order_after: f64 = conn.query_row(
                    "SELECT manual_order FROM tasks WHERE id = ?",
                    [after],
                    |row| row.get(0),
                )?;
                let order_before: f64 = conn.query_row(
                    "SELECT manual_order FROM tasks WHERE id = ?",
                    [before],
                    |row| row.get(0),
                )?;
                let mid = (order_after + order_before) / 2.0;
                if mid == order_after || mid == order_before {
                    return Err(Error::ReorderError(
                        "Precision exhausted. Run `tt reindex` first.".to_string(),
                    ));
                }
                mid
            }
            (None, None) => {
                let max_order: Option<f64> = conn
                    .query_row("SELECT MAX(manual_order) FROM tasks", [], |row| row.get(0))
                    .ok();
                max_order.unwrap_or(0.0) + 10.0
            }
        };

        let now = Self::now();

        conn.execute(
            "INSERT INTO tasks (title, description, dod, status, manual_order, created_at, last_touched_at) VALUES (?, ?, ?, 'pending', ?, ?, ?)",
            params![title, description, dod, manual_order, now, now]
        )?;

        let id = conn.last_insert_rowid();

        let task = conn.query_row(
            "SELECT id, title, description, dod, status, manual_order, created_at, started_at, completed_at, last_touched_at FROM tasks WHERE id = ?",
            [id],
            |row| {
                Ok(Task {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    description: row.get(2)?,
                    dod: row.get(3)?,
                    status: Status::from_db_str(&row.get::<_, String>(4)?).unwrap(),
                    manual_order: row.get(5)?,
                    created_at: row.get(6)?,
                    started_at: row.get(7)?,
                    completed_at: row.get(8)?,
                    last_touched_at: row.get(9)?,
                })
            }
        )?;

        Ok(task)
    }

    pub fn get_task(&self, id: i64) -> Result<Task> {
        let conn = self.conn.lock().unwrap();

        conn.query_row(
            "SELECT id, title, description, dod, status, manual_order, created_at, started_at, completed_at, last_touched_at FROM tasks WHERE id = ?",
            [id],
            |row| {
                Ok(Task {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    description: row.get(2)?,
                    dod: row.get(3)?,
                    status: Status::from_db_str(&row.get::<_, String>(4)?).unwrap(),
                    manual_order: row.get(5)?,
                    created_at: row.get(6)?,
                    started_at: row.get(7)?,
                    completed_at: row.get(8)?,
                    last_touched_at: row.get(9)?,
                })
            }
        ).map_err(|_| Error::TaskNotFound(id))
    }

    pub fn update_task(
        &self,
        id: i64,
        title: Option<&str>,
        description: Option<&str>,
        dod: Option<&str>,
    ) -> Result<Task> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now();

        let mut updates = vec!["last_touched_at = ?".to_string()];
        let mut values: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];

        if let Some(t) = title {
            updates.push("title = ?".to_string());
            values.push(Box::new(t.to_string()));
        }
        if let Some(d) = description {
            updates.push("description = ?".to_string());
            values.push(Box::new(d.to_string()));
        }
        if let Some(d) = dod {
            updates.push("dod = ?".to_string());
            values.push(Box::new(d.to_string()));
        }

        values.push(Box::new(id));

        let sql = format!("UPDATE tasks SET {} WHERE id = ?", updates.join(", "));

        let params: Vec<&dyn rusqlite::ToSql> = values.iter().map(|b| b.as_ref()).collect();

        conn.execute(&sql, params.as_slice())?;

        drop(conn);
        self.get_task(id)
    }

    pub fn update_task_status(
        &self,
        id: i64,
        status: Status,
        set_started: bool,
        set_completed: bool,
    ) -> Result<Task> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now();
        let now_for_started = if set_started { Some(now.clone()) } else { None };
        let now_for_completed = if set_completed {
            Some(now.clone())
        } else {
            None
        };

        let mut updates = vec!["status = ?".to_string(), "last_touched_at = ?".to_string()];
        let mut values: Vec<Box<dyn rusqlite::ToSql>> =
            vec![Box::new(status.as_str().to_string()), Box::new(now)];

        if let Some(ref started) = now_for_started {
            updates.push("started_at = ?".to_string());
            values.push(Box::new(started.clone()));
        }

        if let Some(ref completed) = now_for_completed {
            updates.push("completed_at = ?".to_string());
            values.push(Box::new(completed.clone()));
        }

        values.push(Box::new(id));

        let sql = format!("UPDATE tasks SET {} WHERE id = ?", updates.join(", "));

        let params: Vec<&dyn rusqlite::ToSql> = values.iter().map(|b| b.as_ref()).collect();

        conn.execute(&sql, params.as_slice())?;

        drop(conn);
        self.get_task(id)
    }

    pub fn get_active_task(&self) -> Result<Task> {
        let conn = self.conn.lock().unwrap();

        conn.query_row(
            "SELECT id, title, description, dod, status, manual_order, created_at, started_at, completed_at, last_touched_at FROM tasks WHERE status = 'in_progress'",
            [],
            |row| {
                Ok(Task {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    description: row.get(2)?,
                    dod: row.get(3)?,
                    status: Status::from_db_str(&row.get::<_, String>(4)?).unwrap(),
                    manual_order: row.get(5)?,
                    created_at: row.get(6)?,
                    started_at: row.get(7)?,
                    completed_at: row.get(8)?,
                    last_touched_at: row.get(9)?,
                })
            }
        ).map_err(|_| Error::NoActiveTask)
    }

    pub fn get_all_tasks(&self, filter: crate::models::TaskFilter) -> Result<Vec<Task>> {
        let conn = self.conn.lock().unwrap();

        let mut conditions = vec![];
        if !filter.pending {
            conditions.push("status != 'pending'");
        }
        if !filter.in_progress {
            conditions.push("status != 'in_progress'");
        }
        if !filter.completed {
            conditions.push("status != 'completed'");
        }
        if !filter.blocked {
            conditions.push("status != 'blocked'");
        }

        let sql = if conditions.is_empty() {
            "SELECT id, title, description, dod, status, manual_order, created_at, started_at, completed_at, last_touched_at FROM tasks ORDER BY manual_order".to_string()
        } else {
            format!(
                "SELECT id, title, description, dod, status, manual_order, created_at, started_at, completed_at, last_touched_at FROM tasks WHERE {} ORDER BY manual_order",
                conditions.join(" AND ")
            )
        };

        let mut stmt = conn.prepare(&sql)?;

        let tasks = stmt
            .query_map([], |row| {
                Ok(Task {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    description: row.get(2)?,
                    dod: row.get(3)?,
                    status: Status::from_db_str(&row.get::<_, String>(4)?).unwrap(),
                    manual_order: row.get(5)?,
                    created_at: row.get(6)?,
                    started_at: row.get(7)?,
                    completed_at: row.get(8)?,
                    last_touched_at: row.get(9)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(tasks)
    }

    pub fn get_target_subgraph(&self, target_id: i64) -> Result<Vec<Task>> {
        let conn = self.conn.lock().unwrap();

        conn.query_row("SELECT id FROM tasks WHERE id = ?", [target_id], |_| Ok(()))
            .map_err(|_| Error::TaskNotFound(target_id))?;

        let mut stmt = conn.prepare(
            r#"
            WITH RECURSIVE deps AS (
                SELECT id FROM tasks WHERE id = ?
                UNION ALL
                SELECT d.depends_on FROM dependencies d
                INNER JOIN deps p ON d.task_id = p.id
            )
            SELECT DISTINCT id FROM deps
        "#,
        )?;

        let task_ids: Vec<i64> = stmt
            .query_map([target_id], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        if task_ids.is_empty() {
            return Ok(vec![]);
        }

        let placeholders: Vec<String> = task_ids.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "SELECT id, title, description, dod, status, manual_order, created_at, started_at, completed_at, last_touched_at FROM tasks WHERE id IN ({}) AND status != 'completed' ORDER BY manual_order",
            placeholders.join(", ")
        );

        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> = task_ids
            .iter()
            .map(|id| id as &dyn rusqlite::ToSql)
            .collect();

        let tasks = stmt
            .query_map(params.as_slice(), |row| {
                Ok(Task {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    description: row.get(2)?,
                    dod: row.get(3)?,
                    status: Status::from_db_str(&row.get::<_, String>(4)?).unwrap(),
                    manual_order: row.get(5)?,
                    created_at: row.get(6)?,
                    started_at: row.get(7)?,
                    completed_at: row.get(8)?,
                    last_touched_at: row.get(9)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(tasks)
    }

    pub fn get_target(&self) -> Result<Option<i64>> {
        let conn = self.conn.lock().unwrap();

        let result: std::result::Result<String, _> = conn.query_row(
            "SELECT value FROM config WHERE key = 'target_id'",
            [],
            |row| row.get(0),
        );

        match result {
            Ok(v) => Ok(Some(v.parse().unwrap_or(0))),
            Err(_) => Ok(None),
        }
    }

    pub fn set_target(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.query_row("SELECT id FROM tasks WHERE id = ?", [id], |_| Ok(()))
            .map_err(|_| Error::TaskNotFound(id))?;

        conn.execute(
            "INSERT OR REPLACE INTO config (key, value) VALUES ('target_id', ?)",
            [id.to_string()],
        )?;

        Ok(())
    }

    pub fn get_dependencies(&self, task_id: i64) -> Result<Vec<i64>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare("SELECT depends_on FROM dependencies WHERE task_id = ?")?;
        let deps = stmt
            .query_map([task_id], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(deps)
    }

    pub fn get_dependents(&self, task_id: i64) -> Result<Vec<i64>> {
        let conn = self.conn.lock().unwrap();

        let mut stmt = conn.prepare("SELECT task_id FROM dependencies WHERE depends_on = ?")?;
        let deps = stmt
            .query_map([task_id], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(deps)
    }

    pub fn add_dependency(&self, task_id: i64, depends_on: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "INSERT OR IGNORE INTO dependencies (task_id, depends_on) VALUES (?, ?)",
            [task_id, depends_on],
        )?;

        Ok(())
    }

    pub fn remove_dependency(&self, task_id: i64, depends_on: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "DELETE FROM dependencies WHERE task_id = ? AND depends_on = ?",
            [task_id, depends_on],
        )?;

        Ok(())
    }

    pub fn check_cycle(&self, from: i64, to: i64) -> Result<Option<Vec<i64>>> {
        let conn = self.conn.lock().unwrap();

        let mut visited = std::collections::HashSet::new();
        let mut path = vec![];

        fn dfs(
            conn: &Connection,
            current: i64,
            target: i64,
            visited: &mut std::collections::HashSet<i64>,
            path: &mut Vec<i64>,
        ) -> bool {
            if current == target {
                path.push(target);
                return true;
            }

            if visited.contains(&current) {
                return false;
            }

            visited.insert(current);
            path.push(current);

            let mut stmt = conn
                .prepare("SELECT task_id FROM dependencies WHERE depends_on = ?")
                .unwrap();
            let dependents: Vec<i64> = stmt
                .query_map([current], |row| row.get(0))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect();

            for dep in dependents {
                if dfs(conn, dep, target, visited, path) {
                    return true;
                }
            }

            path.pop();
            false
        }

        if dfs(&conn, from, to, &mut visited, &mut path) {
            path.push(to);
            Ok(Some(path))
        } else {
            Ok(None)
        }
    }

    pub fn create_artifact(&self, task_id: i64, name: &str, file_path: &str) -> Result<Artifact> {
        let conn = self.conn.lock().unwrap();
        let now = Self::now();

        conn.execute(
            "INSERT INTO artifacts (task_id, name, file_path, created_at) VALUES (?, ?, ?, ?)",
            params![task_id, name, file_path, now],
        )?;

        let id = conn.last_insert_rowid();

        Ok(Artifact {
            id,
            task_id,
            name: name.to_string(),
            file_path: file_path.to_string(),
            created_at: now,
        })
    }

    pub fn get_artifacts(&self, task_id: Option<i64>) -> Result<Vec<Artifact>> {
        let conn = self.conn.lock().unwrap();

        let sql = match task_id {
            Some(_) => {
                "SELECT id, task_id, name, file_path, created_at FROM artifacts WHERE task_id = ? ORDER BY created_at"
            }
            None => {
                "SELECT id, task_id, name, file_path, created_at FROM artifacts ORDER BY created_at"
            }
        };

        let mut stmt = conn.prepare(sql)?;

        let artifacts = match task_id {
            Some(id) => stmt
                .query_map([id], |row| {
                    Ok(Artifact {
                        id: row.get(0)?,
                        task_id: row.get(1)?,
                        name: row.get(2)?,
                        file_path: row.get(3)?,
                        created_at: row.get(4)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?,
            None => stmt
                .query_map([], |row| {
                    Ok(Artifact {
                        id: row.get(0)?,
                        task_id: row.get(1)?,
                        name: row.get(2)?,
                        file_path: row.get(3)?,
                        created_at: row.get(4)?,
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?,
        };

        Ok(artifacts)
    }

    pub fn reindex(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "UPDATE tasks SET manual_order = (SELECT COUNT(*) * 10.0 FROM tasks t2 WHERE t2.manual_order < tasks.manual_order OR t2.id < tasks.id) + 10.0",
            []
        )?;

        Ok(())
    }

    pub fn reorder_task(
        &self,
        id: i64,
        after_id: Option<i64>,
        before_id: Option<i64>,
    ) -> Result<f64> {
        let conn = self.conn.lock().unwrap();

        let new_order = match (after_id, before_id) {
            (Some(after), None) => {
                let order: f64 = conn.query_row(
                    "SELECT manual_order FROM tasks WHERE id = ?",
                    [after],
                    |row| row.get(0),
                )?;
                order + 10.0
            }
            (None, Some(before)) => {
                let order: f64 = conn.query_row(
                    "SELECT manual_order FROM tasks WHERE id = ?",
                    [before],
                    |row| row.get(0),
                )?;
                order - 10.0
            }
            (Some(after), Some(before)) => {
                let order_after: f64 = conn.query_row(
                    "SELECT manual_order FROM tasks WHERE id = ?",
                    [after],
                    |row| row.get(0),
                )?;
                let order_before: f64 = conn.query_row(
                    "SELECT manual_order FROM tasks WHERE id = ?",
                    [before],
                    |row| row.get(0),
                )?;
                let mid = (order_after + order_before) / 2.0;
                if mid == order_after || mid == order_before {
                    return Err(Error::ReorderError(
                        "Precision exhausted. Run `tt reindex` first.".to_string(),
                    ));
                }
                mid
            }
            (None, None) => {
                return Err(Error::ReorderError(
                    "Either --after or --before must be specified".to_string(),
                ));
            }
        };

        let now = Self::now();

        conn.execute(
            "UPDATE tasks SET manual_order = ?, last_touched_at = ? WHERE id = ?",
            params![new_order, now, id],
        )?;

        Ok(new_order)
    }
}

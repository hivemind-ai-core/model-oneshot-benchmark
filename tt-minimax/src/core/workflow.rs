use crate::db::{Db, Task, TaskStatus};
use crate::error::{Error, Result};
use crate::graph;

/// Get the currently active task (in_progress status)
pub fn get_current_task(db: &Db) -> Result<Task> {
    let mut stmt = db.conn.prepare(
        "SELECT id, title, description, dod, status, manual_order,
                created_at, started_at, completed_at, last_touched_at
         FROM tasks WHERE status = 'in_progress' LIMIT 1",
    )?;

    let task = stmt
        .query_row([], |row| {
            let status_str: String = row.get(4)?;
            let status = TaskStatus::from_str(&status_str)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            Ok(Task {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                dod: row.get(3)?,
                status,
                manual_order: row.get(5)?,
                created_at: row.get(6)?,
                started_at: row.get(7)?,
                completed_at: row.get(8)?,
                last_touched_at: row.get(9)?,
            })
        })
        .map_err(|_| Error::NoActiveTask)?;

    Ok(task)
}

/// Start a task (move to in_progress)
pub fn start_task(db: &mut Db, id: i64) -> Result<Task> {
    let task = super::get_task(db, id)?;

    // Idempotent: if already in_progress, return the task
    if task.status == TaskStatus::InProgress {
        return Ok(task);
    }

    // Check if another task is already active
    let active = get_active_task_id(db)?;
    if let Some(active_id) = active {
        if active_id != id {
            let active_task = super::get_task(db, active_id)?;
            return Err(Error::AnotherTaskActive {
                id: active_id,
                title: active_task.title,
            });
        }
    }

    // Check if task is in the right state to start
    if task.status != TaskStatus::Pending {
        return Err(Error::InvalidTaskStatus {
            id,
            current: task.status.as_str().to_string(),
            expected: "pending".to_string(),
            action: "start".to_string(),
        });
    }

    // Check if all dependencies are completed
    let unmet_deps = get_unmet_dependencies(db, id)?;
    if !unmet_deps.is_empty() {
        return Err(Error::UnmetDependencies {
            id,
            unmet_ids: unmet_deps,
        });
    }

    // Update task status
    let mut tx = db.transaction()?;
    let now = Db::now();

    tx.execute(
        "UPDATE tasks SET status = 'in_progress', started_at = ?1, last_touched_at = ?1 WHERE id = ?2",
        (&now, id),
    )?;

    tx.commit()?;

    super::get_task(db, id)
}

/// Get the ID of the currently active task, if any
fn get_active_task_id(db: &Db) -> Result<Option<i64>> {
    let mut stmt = db
        .conn
        .prepare("SELECT id FROM tasks WHERE status = 'in_progress' LIMIT 1")?;
    let result = stmt.query_row([], |row| row.get::<_, i64>(0));

    match result {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::Db(e)),
    }
}

/// Get unmet dependencies for a task
fn get_unmet_dependencies(db: &Db, task_id: i64) -> Result<Vec<i64>> {
    let mut stmt = db.conn.prepare(
        "SELECT d.depends_on
         FROM dependencies d
         JOIN tasks t ON t.id = d.depends_on
         WHERE d.task_id = ?1 AND t.status != 'completed'",
    )?;

    let unmet = stmt
        .query_map([task_id], |row| row.get(0))?
        .collect::<std::result::Result<Vec<i64>, _>>()?;

    Ok(unmet)
}

/// Stop the current task (move back to pending)
pub fn stop_task(db: &mut Db) -> Result<Task> {
    let task = get_current_task(db)?;

    let mut tx = db.transaction()?;
    let now = Db::now();

    tx.execute(
        "UPDATE tasks SET status = 'pending', last_touched_at = ?1 WHERE id = ?2",
        (&now, task.id),
    )?;

    tx.commit()?;

    super::get_task(db, task.id)
}

/// Complete the current task (move to completed)
pub fn complete_task(db: &mut Db) -> Result<Task> {
    let task = get_current_task(db)?;

    // Check DoD
    if task.dod.is_none() || task.dod.as_ref().map(|s| s.trim()).unwrap_or("").is_empty() {
        return Err(Error::NoDod { id: task.id });
    }

    let mut tx = db.transaction()?;
    let now = Db::now();

    tx.execute(
        "UPDATE tasks SET status = 'completed', completed_at = ?1, last_touched_at = ?1 WHERE id = ?2",
        (&now, task.id),
    )?;

    tx.commit()?;

    super::get_task(db, task.id)
}

/// Block a task (move to blocked status)
pub fn block_task(db: &mut Db, id: i64) -> Result<Task> {
    let task = super::get_task(db, id)?;

    let new_status = match task.status {
        TaskStatus::Pending | TaskStatus::InProgress => TaskStatus::Blocked,
        s => {
            return Err(Error::InvalidTaskStatus {
                id,
                current: s.as_str().to_string(),
                expected: "pending or in_progress".to_string(),
                action: "block".to_string(),
            });
        }
    };

    let mut tx = db.transaction()?;
    let now = Db::now();

    tx.execute(
        "UPDATE tasks SET status = ?1, last_touched_at = ?2 WHERE id = ?3",
        (new_status.as_str(), &now, id),
    )?;

    tx.commit()?;

    super::get_task(db, id)
}

/// Unblock a task (move from blocked to pending)
pub fn unblock_task(db: &mut Db, id: i64) -> Result<Task> {
    let task = super::get_task(db, id)?;

    if task.status != TaskStatus::Blocked {
        return Err(Error::InvalidTaskStatus {
            id,
            current: task.status.as_str().to_string(),
            expected: "blocked".to_string(),
            action: "unblock".to_string(),
        });
    }

    let mut tx = db.transaction()?;
    let now = Db::now();

    tx.execute(
        "UPDATE tasks SET status = 'pending', last_touched_at = ?1 WHERE id = ?2",
        (&now, id),
    )?;

    tx.commit()?;

    super::get_task(db, id)
}

/// Get the next task to work on
pub fn get_next(db: &Db, target_id: Option<i64>) -> Result<Task> {
    let tid = if let Some(t) = target_id {
        t
    } else {
        super::get_current_target(db)?.ok_or(Error::NoTarget)?
    };

    graph::get_next_task(db, tid)
}

/// Add a dependency between tasks
pub fn add_dependency(db: &mut Db, task_id: i64, depends_on: i64) -> Result<()> {
    // Verify both tasks exist
    if !db.task_exists(task_id)? {
        return Err(Error::TaskNotFound { id: task_id });
    }
    if !db.task_exists(depends_on)? {
        return Err(Error::TaskNotFound { id: depends_on });
    }

    // Check for cycles
    graph::check_cycle(db, task_id, depends_on)?;

    let mut tx = db.transaction()?;

    tx.execute(
        "INSERT INTO dependencies (task_id, depends_on) VALUES (?1, ?2)",
        (task_id, depends_on),
    )?;

    // Update last_touched_at
    tx.execute(
        "UPDATE tasks SET last_touched_at = ?1 WHERE id = ?2",
        (Db::now(), task_id),
    )?;

    tx.commit()?;

    Ok(())
}

/// Remove a dependency between tasks
pub fn remove_dependency(db: &mut Db, task_id: i64, depends_on: i64) -> Result<()> {
    let mut tx = db.transaction()?;

    let rows_affected = tx.execute(
        "DELETE FROM dependencies WHERE task_id = ?1 AND depends_on = ?2",
        (task_id, depends_on),
    )?;

    if rows_affected == 0 {
        tx.rollback()?;
        // Might not exist, but that's ok - just consider it successful
        return Ok(());
    }

    // Update last_touched_at
    tx.execute(
        "UPDATE tasks SET last_touched_at = ?1 WHERE id = ?2",
        (Db::now(), task_id),
    )?;

    tx.commit()?;

    Ok(())
}

/// Log an artifact for the active task
pub fn log_artifact(db: &mut Db, name: String, file_path: String) -> Result<crate::db::Artifact> {
    let task = get_current_task(db)?;

    let mut tx = db.transaction()?;
    let now = Db::now();

    tx.execute(
        "INSERT INTO artifacts (task_id, name, file_path, created_at) VALUES (?1, ?2, ?3, ?4)",
        (&task.id, &name, &file_path, &now),
    )?;

    let artifact_id = tx.last_insert_rowid();

    // Update last_touched_at
    tx.execute(
        "UPDATE tasks SET last_touched_at = ?1 WHERE id = ?2",
        (&now, task.id),
    )?;

    tx.commit()?;

    Ok(crate::db::Artifact {
        id: artifact_id,
        task_id: task.id,
        name,
        file_path,
        created_at: now,
    })
}

/// Get artifacts for a task (or the active task if no ID specified)
pub fn get_artifacts(db: &Db, task_id: Option<i64>) -> Result<Vec<crate::db::Artifact>> {
    let tid = if let Some(t) = task_id {
        t
    } else {
        get_current_task(db)?.id
    };

    super::get_artifacts_for_task(db, tid)
}

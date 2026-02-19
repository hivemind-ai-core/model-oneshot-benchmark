use crate::db::{Artifact, Db, Task, TaskDependencyInfo, TaskDetail, TaskStatus};
use crate::error::{Error, Result};
use crate::graph::{self, calculate_midpoint};

/// Add a new task
pub fn add_task(
    db: &mut Db,
    title: String,
    description: Option<String>,
    dod: Option<String>,
    after_id: Option<i64>,
    before_id: Option<i64>,
) -> Result<Task> {
    if after_id.is_some() && before_id.is_some() {
        return Err(Error::ReorderConflict);
    }

    let manual_order = calculate_new_order(db, after_id, before_id)?;

    let mut tx = db.transaction()?;
    let now = Db::now();

    let task_id = tx.execute(
        "INSERT INTO tasks (title, description, dod, status, manual_order, created_at, last_touched_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        (
            &title,
            &description,
            &dod,
            TaskStatus::Pending.as_str(),
            manual_order,
            &now,
            &now,
        ),
    )?;

    let task_id = task_id as i64;

    tx.commit()?;

    get_task(db, task_id)
}

/// Calculate the manual_order for a new task based on positioning hints
fn calculate_new_order(db: &Db, after_id: Option<i64>, before_id: Option<i64>) -> Result<f64> {
    match (after_id, before_id) {
        (Some(after), Some(before)) => {
            // Insert between two tasks
            let after_order = get_task_order(db, after)?;
            let before_order = get_task_order(db, before)?;
            calculate_midpoint(after_order, before_order)
        }
        (Some(after), None) => {
            // Insert after a specific task
            let after_order = get_task_order(db, after)?;
            Ok(after_order + 10.0)
        }
        (None, Some(before)) => {
            // Insert before a specific task
            let before_order = get_task_order(db, before)?;
            Ok(before_order - 10.0)
        }
        (None, None) => {
            // Add at the end
            let max = db.max_manual_order()?;
            Ok(max + 10.0)
        }
    }
}

/// Get the manual_order of a task
fn get_task_order(db: &Db, id: i64) -> Result<f64> {
    let mut stmt = db
        .conn
        .prepare("SELECT manual_order FROM tasks WHERE id = ?1")?;
    let order: f64 = stmt.query_row([id], |row| row.get(0))?;
    Ok(order)
}

/// Edit a task
pub fn edit_task(
    db: &mut Db,
    id: i64,
    title: Option<String>,
    description: Option<String>,
    dod: Option<String>,
) -> Result<Task> {
    if !db.task_exists(id)? {
        return Err(Error::TaskNotFound { id });
    }

    let mut tx = db.transaction()?;

    // Build the update query dynamically
    if title.is_some() || description.is_some() || dod.is_some() {
        let mut updates = Vec::new();

        if let Some(t) = &title {
            updates.push(format!("title = '{}'", t.replace("'", "''")));
        }
        if let Some(ref d) = description {
            updates.push(format!("description = '{}'", d.replace("'", "''")));
        }
        if let Some(ref d) = dod {
            updates.push(format!("dod = '{}'", d.replace("'", "''")));
        }

        let now = Db::now();
        updates.push(format!("last_touched_at = '{}'", now));

        let query = format!("UPDATE tasks SET {} WHERE id = {}", updates.join(", "), id);

        tx.execute(&query, [])?;
    }

    tx.commit()?;

    get_task(db, id)
}

/// Get a task by ID
pub fn get_task(db: &Db, id: i64) -> Result<Task> {
    let mut stmt = db.conn.prepare(
        "SELECT id, title, description, dod, status, manual_order,
                created_at, started_at, completed_at, last_touched_at
         FROM tasks WHERE id = ?1",
    )?;

    let task = stmt.query_row([id], |row| {
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
    })?;

    Ok(task)
}

/// Get detailed information about a task including dependencies, dependents, and artifacts
pub fn show_task(db: &Db, id: i64) -> Result<TaskDetail> {
    let task = get_task(db, id)?;

    // Get dependencies
    let mut deps_stmt = db.conn.prepare(
        "SELECT d.depends_on, t.title, t.status
         FROM dependencies d
         JOIN tasks t ON t.id = d.depends_on
         WHERE d.task_id = ?1",
    )?;

    let dependencies = deps_stmt
        .query_map([id], |row| {
            let status_str: String = row.get(2)?;
            let status = TaskStatus::from_str(&status_str)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            Ok(TaskDependencyInfo {
                id: row.get(0)?,
                title: row.get(1)?,
                status,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // Get dependents
    let mut dep_stmt = db
        .conn
        .prepare("SELECT task_id FROM dependencies WHERE depends_on = ?1")?;

    let dependents = dep_stmt
        .query_map([id], |row| row.get(0))?
        .collect::<std::result::Result<Vec<i64>, _>>()?;

    // Get artifacts
    let artifacts = get_artifacts_for_task(db, id)?;

    Ok(TaskDetail {
        task,
        dependencies,
        dependents,
        artifacts,
    })
}

/// List tasks, optionally filtered by target
pub fn list_tasks(db: &Db, target_id: Option<i64>, show_all: bool) -> Result<Vec<Task>> {
    let task_ids = if show_all {
        // Get all tasks
        let mut stmt = db
            .conn
            .prepare("SELECT id FROM tasks ORDER BY manual_order")?;
        let ids = stmt
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<Vec<i64>, _>>()?;
        drop(stmt); // Explicitly drop the statement
        ids
    } else if let Some(tid) = target_id {
        // Get target subgraph
        graph::get_target_subgraph(db, tid)?
    } else {
        // No target and not showing all - check if there's a target in config
        let current_target = get_current_target(db)?;
        if let Some(tid) = current_target {
            graph::get_target_subgraph(db, tid)?
        } else {
            return Err(Error::NoTarget);
        }
    };

    // Sort topologically
    let sorted = graph::topological_sort(db, &task_ids)?;

    // Check for order conflicts and emit warnings
    let conflicts = graph::check_order_conflicts(db, &sorted)?;
    for conflict in conflicts {
        eprintln!(
            "Warning: task #{} (order {}) depends on #{} (order {}) which has higher manual_order",
            conflict.0, conflict.1, conflict.2, conflict.3
        );
    }

    // Return tasks in sorted order
    sorted.into_iter().map(|id| get_task(db, id)).collect()
}

/// Get the current target from config
pub fn get_current_target(db: &Db) -> Result<Option<i64>> {
    let mut stmt = db
        .conn
        .prepare("SELECT value FROM config WHERE key = 'target_id'")?;
    let result = stmt.query_row([], |row| row.get::<_, String>(0));

    match result {
        Ok(s) => Ok(Some(
            s.parse::<i64>()
                .map_err(|_| Error::InvalidStatus { status: s })?,
        )),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::Db(e)),
    }
}

/// Get artifacts for a task
pub fn get_artifacts_for_task(db: &Db, task_id: i64) -> Result<Vec<Artifact>> {
    let mut stmt = db.conn.prepare(
        "SELECT id, task_id, name, file_path, created_at
         FROM artifacts WHERE task_id = ?1 ORDER BY created_at",
    )?;

    let artifacts = stmt
        .query_map([task_id], |row| {
            Ok(Artifact {
                id: row.get(0)?,
                task_id: row.get(1)?,
                name: row.get(2)?,
                file_path: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    Ok(artifacts)
}

/// Reorder a task
pub fn reorder_task(
    db: &mut Db,
    id: i64,
    after_id: Option<i64>,
    before_id: Option<i64>,
) -> Result<f64> {
    if after_id.is_some() && before_id.is_some() {
        return Err(Error::ReorderConflict);
    }

    if !db.task_exists(id)? {
        return Err(Error::TaskNotFound { id });
    }

    let new_order = calculate_new_order(db, after_id, before_id)?;

    db.conn.execute(
        "UPDATE tasks SET manual_order = ?1, last_touched_at = ?2 WHERE id = ?3",
        (new_order, Db::now(), id),
    )?;

    Ok(new_order)
}

/// Reindex all manual_order values
pub fn reindex(db: &mut Db) -> Result<usize> {
    // Get all task IDs sorted by current manual_order
    let mut stmt = db
        .conn
        .prepare("SELECT id FROM tasks ORDER BY manual_order")?;

    let task_ids: Vec<i64> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    drop(stmt); // Explicitly drop the statement

    // Assign new orders: 10, 20, 30, ...
    for (i, id) in task_ids.iter().enumerate() {
        let new_order = (i as i64 + 1) * 10;
        db.conn.execute(
            "UPDATE tasks SET manual_order = ?1, last_touched_at = ?2 WHERE id = ?3",
            (new_order, Db::now(), id),
        )?;
    }

    Ok(task_ids.len())
}

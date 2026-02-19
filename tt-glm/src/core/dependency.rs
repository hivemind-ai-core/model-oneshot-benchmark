//! Dependency operations.

use crate::db::Connection;
use crate::error::{Error, Result};
use crate::graph::cycle::detect_cycle;
use std::collections::HashMap;

/// Add a dependency: task_id depends on depends_on.
pub fn add_dependency(conn: &mut Connection, task_id: i64, depends_on: i64) -> Result<()> {
    // First, load all dependencies for cycle detection
    let deps = load_all_dependencies(conn)?;

    // Check if adding this dependency would create a cycle
    if let Some(cycle) = detect_cycle(task_id, depends_on, &deps)? {
        return Err(Error::CycleDetected(task_id, depends_on, cycle.format()));
    }

    // Check if the dependency already exists
    let existing: Option<(i64, i64)> = conn
        .query_row(
            "SELECT task_id, depends_on FROM dependencies WHERE task_id = ? AND depends_on = ?",
            &[
                &task_id as &dyn rusqlite::ToSql,
                &depends_on as &dyn rusqlite::ToSql,
            ],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();

    if existing.is_some() {
        return Err(Error::DuplicateDependency(task_id, depends_on));
    }

    // Add the dependency
    conn.execute(
        "INSERT INTO dependencies (task_id, depends_on) VALUES (?, ?)",
        &[
            &task_id as &dyn rusqlite::ToSql,
            &depends_on as &dyn rusqlite::ToSql,
        ],
    )?;

    // Update last_touched_at for the dependent task
    conn.update_last_touched(task_id)?;
    Ok(())
}

/// Remove a dependency.
pub fn remove_dependency(conn: &mut Connection, task_id: i64, depends_on: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM dependencies WHERE task_id = ? AND depends_on = ?",
        &[
            &task_id as &dyn rusqlite::ToSql,
            &depends_on as &dyn rusqlite::ToSql,
        ],
    )?;

    // Update last_touched_at
    conn.update_last_touched(task_id)?;
    Ok(())
}

/// Get all dependencies for a task.
pub fn get_dependencies(conn: &mut Connection, task_id: i64) -> Result<Vec<i64>> {
    let rows = conn.query(
        "SELECT depends_on FROM dependencies WHERE task_id = ? ORDER BY depends_on",
        &[&task_id as &dyn rusqlite::ToSql],
        |row| row.get(0),
    )?;
    Ok(rows)
}

/// Get all tasks that depend on this task (dependents).
pub fn get_dependents(conn: &mut Connection, task_id: i64) -> Result<Vec<i64>> {
    let rows = conn.query(
        "SELECT task_id FROM dependencies WHERE depends_on = ? ORDER BY task_id",
        &[&task_id as &dyn rusqlite::ToSql],
        |row| row.get(0),
    )?;
    Ok(rows)
}

/// Load all dependencies into a HashMap for cycle detection.
fn load_all_dependencies(conn: &mut Connection) -> Result<HashMap<i64, Vec<i64>>> {
    let rows = conn.query("SELECT task_id, depends_on FROM dependencies", &[], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?;

    let mut deps = HashMap::new();
    for (task_id, depends_on) in rows {
        deps.entry(task_id)
            .or_insert_with(Vec::new)
            .push(depends_on);
    }
    Ok(deps)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    use crate::db::schema::Schema;

    fn setup_db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        Schema::init(&mut conn).unwrap();

        // Create some test tasks
        conn.as_conn_mut()
            .execute(
                "INSERT INTO tasks (id, title, status) VALUES (?, ?, ?)",
                [
                    &1i64 as &dyn rusqlite::ToSql,
                    &"Task 1" as &dyn rusqlite::ToSql,
                    &"pending" as &dyn rusqlite::ToSql,
                ],
            )
            .unwrap();
        conn.as_conn_mut()
            .execute(
                "INSERT INTO tasks (id, title, status) VALUES (?, ?, ?)",
                [
                    &2i64 as &dyn rusqlite::ToSql,
                    &"Task 2" as &dyn rusqlite::ToSql,
                    &"pending" as &dyn rusqlite::ToSql,
                ],
            )
            .unwrap();
        conn.as_conn_mut()
            .execute(
                "INSERT INTO tasks (id, title, status) VALUES (?, ?, ?)",
                [
                    &3i64 as &dyn rusqlite::ToSql,
                    &"Task 3" as &dyn rusqlite::ToSql,
                    &"pending" as &dyn rusqlite::ToSql,
                ],
            )
            .unwrap();

        conn
    }

    #[test]
    fn test_add_dependency() {
        let mut conn = setup_db();
        add_dependency(&mut conn, 2, 1).unwrap();

        let deps = get_dependencies(&mut conn, 2).unwrap();
        assert_eq!(deps, vec![1]);
    }

    #[test]
    fn test_add_dependency_cycle_detection() {
        let mut conn = setup_db();

        // Add: 2 depends on 1, 3 depends on 2
        add_dependency(&mut conn, 2, 1).unwrap();
        add_dependency(&mut conn, 3, 2).unwrap();

        // Try to add: 1 depends on 3 (would create cycle: 1 -> 2 -> 3 -> 1)
        let result = add_dependency(&mut conn, 1, 3);
        assert!(matches!(result, Err(Error::CycleDetected(1, 3, _))));
    }

    #[test]
    fn test_add_dependency_duplicate() {
        let mut conn = setup_db();
        add_dependency(&mut conn, 2, 1).unwrap();

        let result = add_dependency(&mut conn, 2, 1);
        assert!(matches!(result, Err(Error::DuplicateDependency(2, 1))));
    }

    #[test]
    fn test_remove_dependency() {
        let mut conn = setup_db();
        add_dependency(&mut conn, 2, 1).unwrap();

        remove_dependency(&mut conn, 2, 1).unwrap();

        let deps = get_dependencies(&mut conn, 2).unwrap();
        assert!(deps.is_empty());
    }

    #[test]
    fn test_get_dependents() {
        let mut conn = setup_db();
        add_dependency(&mut conn, 2, 1).unwrap();
        add_dependency(&mut conn, 3, 1).unwrap();

        let dependents = get_dependents(&mut conn, 1).unwrap();
        assert_eq!(dependents, vec![2, 3]);
    }
}

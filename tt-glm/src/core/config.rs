//! Configuration and target management.

use crate::db::Connection;
use crate::error::{Error, Result};

/// Key used for storing the target task ID.
const TARGET_KEY: &str = "target_id";

/// Get the current target task ID.
pub fn get_target(conn: &mut Connection) -> Result<Option<i64>> {
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM config WHERE key = ?",
            &[&TARGET_KEY as &dyn rusqlite::ToSql],
            |row| row.get(0),
        )
        .ok();

    match value {
        Some(v) => v
            .parse::<i64>()
            .map(Some)
            .map_err(|_| Error::InvalidStatus(v)),
        None => Ok(None),
    }
}

/// Set the target task ID.
pub fn set_target(conn: &mut Connection, task_id: i64) -> Result<()> {
    // Verify the task exists
    let exists: Option<i64> = conn
        .query_row(
            "SELECT id FROM tasks WHERE id = ?",
            &[&task_id as &dyn rusqlite::ToSql],
            |row| row.get(0),
        )
        .ok();

    if exists.is_none() {
        return Err(Error::TaskNotFound(task_id));
    }

    conn.execute(
        "INSERT OR REPLACE INTO config (key, value) VALUES (?, ?)",
        &[
            &TARGET_KEY as &dyn rusqlite::ToSql,
            &task_id.to_string() as &dyn rusqlite::ToSql,
        ],
    )?;
    Ok(())
}

/// Clear the target.
pub fn clear_target(conn: &mut Connection) -> Result<()> {
    conn.execute(
        "DELETE FROM config WHERE key = ?",
        &[&TARGET_KEY as &dyn rusqlite::ToSql],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::Schema;

    #[test]
    fn test_get_target_none() {
        let mut conn = Connection::open_in_memory().unwrap();
        Schema::init(&mut conn).unwrap();

        assert!(get_target(&mut conn).unwrap().is_none());
    }

    #[test]
    fn test_set_get_target() {
        let mut conn = Connection::open_in_memory().unwrap();
        Schema::init(&mut conn).unwrap();

        // Create a task first
        conn.as_conn_mut()
            .execute("INSERT INTO tasks (title) VALUES (?)", ["Test Task"])
            .unwrap();

        set_target(&mut conn, 1).unwrap();
        assert_eq!(get_target(&mut conn).unwrap(), Some(1));
    }

    #[test]
    fn test_set_target_task_not_found() {
        let mut conn = Connection::open_in_memory().unwrap();
        Schema::init(&mut conn).unwrap();

        let result = set_target(&mut conn, 999);
        assert!(matches!(result, Err(Error::TaskNotFound(999))));
    }

    #[test]
    fn test_clear_target() {
        let mut conn = Connection::open_in_memory().unwrap();
        Schema::init(&mut conn).unwrap();

        conn.as_conn_mut()
            .execute("INSERT INTO tasks (title) VALUES (?)", ["Test Task"])
            .unwrap();

        set_target(&mut conn, 1).unwrap();
        clear_target(&mut conn).unwrap();
        assert!(get_target(&mut conn).unwrap().is_none());
    }

    #[test]
    fn test_set_target_replace() {
        let mut conn = Connection::open_in_memory().unwrap();
        Schema::init(&mut conn).unwrap();

        conn.as_conn_mut()
            .execute(
                "INSERT INTO tasks (id, title) VALUES (?, ?)",
                [
                    &1i64 as &dyn rusqlite::ToSql,
                    &"Task 1" as &dyn rusqlite::ToSql,
                ],
            )
            .unwrap();
        conn.as_conn_mut()
            .execute(
                "INSERT INTO tasks (id, title) VALUES (?, ?)",
                [
                    &2i64 as &dyn rusqlite::ToSql,
                    &"Task 2" as &dyn rusqlite::ToSql,
                ],
            )
            .unwrap();

        set_target(&mut conn, 1).unwrap();
        set_target(&mut conn, 2).unwrap();
        assert_eq!(get_target(&mut conn).unwrap(), Some(2));
    }
}

//! Artifact model and operations.

use crate::db::{schema::ArtifactRow, Connection};
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// An artifact linked to a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: i64,
    pub task_id: i64,
    pub name: String,
    pub file_path: String,
    pub created_at: String,
}

impl Artifact {
    /// Convert an ArtifactRow to an Artifact.
    pub fn from_row(row: ArtifactRow) -> Self {
        Self {
            id: row.id,
            task_id: row.task_id,
            name: row.name,
            file_path: row.file_path,
            created_at: row.created_at,
        }
    }
}

/// Add an artifact to a task.
pub fn add_artifact(
    conn: &mut Connection,
    task_id: i64,
    name: String,
    file_path: String,
) -> Result<Artifact> {
    conn.execute(
        "INSERT INTO artifacts (task_id, name, file_path) VALUES (?, ?, ?)",
        &[
            &task_id as &dyn rusqlite::ToSql,
            &name as &dyn rusqlite::ToSql,
            &file_path as &dyn rusqlite::ToSql,
        ],
    )?;

    let id = conn.last_insert_rowid();

    // Get the created artifact
    let row = conn.query_row(
        "SELECT * FROM artifacts WHERE id = ?",
        &[&id as &dyn rusqlite::ToSql],
        ArtifactRow::from_row,
    )?;

    Ok(Artifact::from_row(row))
}

/// Get all artifacts for a task.
pub fn get_artifacts(conn: &mut Connection, task_id: i64) -> Result<Vec<Artifact>> {
    let rows = conn.query(
        "SELECT * FROM artifacts WHERE task_id = ? ORDER BY created_at",
        &[&task_id as &dyn rusqlite::ToSql],
        ArtifactRow::from_row,
    )?;

    Ok(rows.into_iter().map(Artifact::from_row).collect())
}

/// Get the active task and its artifacts.
pub fn get_active_task_artifacts(conn: &mut Connection) -> Result<(i64, Vec<Artifact>)> {
    let task_id: i64 = conn
        .query_row(
            "SELECT id FROM tasks WHERE status = 'in_progress' LIMIT 1",
            &[],
            |row| row.get(0),
        )
        .map_err(|_| crate::error::Error::NoActiveTask)?;

    let artifacts = get_artifacts(conn, task_id)?;
    Ok((task_id, artifacts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::Schema;

    fn setup_db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        Schema::init(&mut conn).unwrap();

        // Create a test task
        conn.as_conn_mut()
            .execute(
                "INSERT INTO tasks (title, status) VALUES (?, ?)",
                ["Test Task", "in_progress"],
            )
            .unwrap();

        conn
    }

    #[test]
    fn test_add_artifact() {
        let mut conn = setup_db();

        let artifact = add_artifact(
            &mut conn,
            1,
            "research".to_string(),
            ".tt/artifacts/1-research.md".to_string(),
        )
        .unwrap();

        assert_eq!(artifact.task_id, 1);
        assert_eq!(artifact.name, "research");
        assert_eq!(artifact.file_path, ".tt/artifacts/1-research.md");
    }

    #[test]
    fn test_get_artifacts() {
        let mut conn = setup_db();

        add_artifact(
            &mut conn,
            1,
            "research".to_string(),
            ".tt/artifacts/1-research.md".to_string(),
        )
        .unwrap();
        add_artifact(
            &mut conn,
            1,
            "plan".to_string(),
            ".tt/artifacts/1-plan.md".to_string(),
        )
        .unwrap();

        let artifacts = get_artifacts(&mut conn, 1).unwrap();
        assert_eq!(artifacts.len(), 2);
        assert_eq!(artifacts[0].name, "research");
        assert_eq!(artifacts[1].name, "plan");
    }

    #[test]
    fn test_get_active_task_artifacts() {
        let mut conn = setup_db();

        add_artifact(
            &mut conn,
            1,
            "research".to_string(),
            ".tt/artifacts/1-research.md".to_string(),
        )
        .unwrap();

        let (task_id, artifacts) = get_active_task_artifacts(&mut conn).unwrap();
        assert_eq!(task_id, 1);
        assert_eq!(artifacts.len(), 1);
    }

    #[test]
    fn test_get_active_task_artifacts_no_active() {
        let mut conn = Connection::open_in_memory().unwrap();
        Schema::init(&mut conn).unwrap();

        let result = get_active_task_artifacts(&mut conn);
        assert!(matches!(result, Err(crate::error::Error::NoActiveTask)));
    }
}

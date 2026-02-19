use crate::db::Db;
use crate::error::{Error, Result};

/// Set the target task
pub fn set_target(db: &mut Db, id: i64) -> Result<()> {
    // Verify the task exists
    if !db.task_exists(id)? {
        return Err(Error::TaskNotFound { id });
    }

    // Insert or replace the target in config
    let mut tx = db.transaction()?;

    tx.execute(
        "INSERT OR REPLACE INTO config (key, value) VALUES ('target_id', ?1)",
        (id.to_string(),),
    )?;

    tx.commit()?;

    Ok(())
}

/// Get the current target
pub fn get_target(db: &Db) -> Result<Option<i64>> {
    let mut stmt = db
        .conn
        .prepare("SELECT value FROM config WHERE key = 'target_id'")?;

    match stmt.query_row([], |row| row.get::<_, String>(0)) {
        Ok(s) => {
            let id = s
                .parse::<i64>()
                .map_err(|_| Error::InvalidStatus { status: s })?;
            Ok(Some(id))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(Error::Db(e)),
    }
}

/// Clear the target
pub fn clear_target(db: &mut Db) -> Result<()> {
    db.conn
        .execute("DELETE FROM config WHERE key = 'target_id'", [])?;
    Ok(())
}

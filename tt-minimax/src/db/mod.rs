pub mod models;
pub mod schema;

use crate::error::Result;
use rusqlite::{params, Connection};
use std::path::Path;

pub use models::*;

/// Database connection wrapper
pub struct Db {
    pub conn: Connection,
}

impl Db {
    /// Open a connection to the database at the given path
    /// Creates the database if it doesn't exist
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let exists = path.exists();

        let conn = Connection::open(path)?;

        // Enable WAL mode for better concurrency
        let _ = conn.execute("PRAGMA journal_mode=WAL", []);

        // Enable foreign keys
        conn.execute("PRAGMA foreign_keys=ON", [])?;

        let db = Db { conn };

        // Create schema if new database
        if !exists {
            schema::create_schema(&db.conn)?;
        }

        Ok(db)
    }

    /// Open an in-memory database for testing
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;

        // Enable WAL mode and foreign keys
        let _ = conn.execute("PRAGMA journal_mode=WAL", []);
        conn.execute("PRAGMA foreign_keys=ON", [])?;

        schema::create_schema(&conn)?;

        Ok(Db { conn })
    }

    /// Begin a transaction
    pub fn transaction(&mut self) -> Result<Transaction> {
        Ok(Transaction {
            tx: self.conn.transaction()?,
        })
    }

    /// Get the current time as ISO 8601 string
    pub fn now() -> String {
        chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    }

    /// Update the last_touched_at timestamp for a task
    pub fn touch_task(&mut self, id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE tasks SET last_touched_at = ?1 WHERE id = ?2",
            params![Self::now(), id],
        )?;
        Ok(())
    }

    /// Check if a task exists
    pub fn task_exists(&self, id: i64) -> Result<bool> {
        let mut stmt = self
            .conn
            .prepare("SELECT COUNT(*) FROM tasks WHERE id = ?1")?;
        let count: i64 = stmt.query_row(params![id], |row| row.get(0))?;
        Ok(count > 0)
    }

    /// Get the maximum manual_order value
    pub fn max_manual_order(&self) -> Result<f64> {
        let mut stmt = self
            .conn
            .prepare("SELECT COALESCE(MAX(manual_order), 0) FROM tasks")?;
        let max: f64 = stmt.query_row([], |row| row.get(0))?;
        Ok(max)
    }
}

/// Transaction wrapper
pub struct Transaction<'a> {
    pub tx: rusqlite::Transaction<'a>,
}

impl<'a> Transaction<'a> {
    /// Commit the transaction
    pub fn commit(self) -> Result<()> {
        self.tx.commit()?;
        Ok(())
    }

    /// Rollback the transaction
    pub fn rollback(self) -> Result<()> {
        self.tx.rollback()?;
        Ok(())
    }

    /// Execute a SQL statement within the transaction
    pub fn execute(&mut self, sql: &str, params: impl rusqlite::Params) -> Result<usize> {
        Ok(self.tx.execute(sql, params)?)
    }

    /// Get last insert rowid
    pub fn last_insert_rowid(&self) -> i64 {
        self.tx.last_insert_rowid()
    }
}

/// Deref to the underlying transaction for direct access
impl<'a> std::ops::Deref for Transaction<'a> {
    type Target = rusqlite::Transaction<'a>;

    fn deref(&self) -> &Self::Target {
        &self.tx
    }
}

impl<'a> std::ops::DerefMut for Transaction<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tx
    }
}

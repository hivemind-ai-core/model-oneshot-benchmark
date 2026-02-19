//! Database connection management.

use crate::error::{Error, Result};
use rusqlite::{Connection as SqliteConnection, Transaction};
use std::path::{Path, PathBuf};

/// Path to the task tracker database file.
#[derive(Debug, Clone)]
pub struct DbPath {
    path: PathBuf,
}

impl DbPath {
    /// Create a new DbPath with the default filename "tt.db".
    pub fn default_path() -> Self {
        Self {
            path: PathBuf::from("tt.db"),
        }
    }

    /// Create a DbPath from a string path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Get the path as a reference.
    pub fn as_path(&self) -> &Path {
        &self.path
    }

    /// Check if the database file exists.
    pub fn exists(&self) -> bool {
        self.path.exists()
    }
}

impl Default for DbPath {
    fn default() -> Self {
        Self::default_path()
    }
}

/// Database connection wrapper.
pub struct Connection {
    conn: SqliteConnection,
}

impl Connection {
    /// Open a connection to the database at the given path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = SqliteConnection::open(path)?;
        // Enable foreign keys
        conn.execute("PRAGMA foreign_keys = ON", [])?;
        Ok(Self { conn })
    }

    /// Open a connection to the default tt.db file.
    pub fn open_default() -> Result<Self> {
        Self::open("tt.db")
    }

    /// Open an in-memory database for testing.
    pub fn open_in_memory() -> Result<Self> {
        let conn = SqliteConnection::open_in_memory()?;
        conn.execute("PRAGMA foreign_keys = ON", [])?;
        Ok(Self { conn })
    }

    /// Begin a new transaction.
    pub fn transaction(&mut self) -> Result<Transaction> {
        self.conn.transaction().map_err(Error::from)
    }

    /// Get a reference to the underlying SqliteConnection.
    pub fn as_conn(&self) -> &SqliteConnection {
        &self.conn
    }

    /// Get a mutable reference to the underlying SqliteConnection.
    pub fn as_conn_mut(&mut self) -> &mut SqliteConnection {
        &mut self.conn
    }

    /// Execute a statement and return the number of rows affected.
    pub fn execute(&mut self, sql: &str, params: &[&dyn rusqlite::ToSql]) -> Result<usize> {
        self.conn.execute(sql, params).map_err(Error::from)
    }

    /// Prepare a statement for execution.
    pub fn prepare(&mut self, sql: &str) -> Result<rusqlite::Statement> {
        self.conn.prepare(sql).map_err(Error::from)
    }

    /// Query a single row.
    pub fn query_row<T, F>(&mut self, sql: &str, params: &[&dyn rusqlite::ToSql], f: F) -> Result<T>
    where
        F: FnOnce(&rusqlite::Row) -> rusqlite::Result<T>,
    {
        self.conn.query_row(sql, params, f).map_err(Error::from)
    }

    /// Query multiple rows.
    pub fn query<T, F>(
        &mut self,
        sql: &str,
        params: &[&dyn rusqlite::ToSql],
        f: F,
    ) -> Result<Vec<T>>
    where
        T: Send + 'static,
        F: FnMut(&rusqlite::Row) -> rusqlite::Result<T>,
    {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt
            .query_map(params, f)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Check if a table exists.
    pub fn table_exists(&mut self, table_name: &str) -> Result<bool> {
        let exists = self.conn.query_row(
            "SELECT name FROM sqlite_master WHERE type='table' AND name=?",
            [table_name],
            |_| Ok(true),
        );
        match exists {
            Ok(true) => Ok(true),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(e) => Err(Error::from(e)),
            _ => Ok(false),
        }
    }

    /// Update last_touched_at timestamp for a task.
    pub fn update_last_touched(&mut self, task_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE tasks SET last_touched_at = strftime('%Y-%m-%dT%H:%M:%S', 'now') WHERE id = ?",
            [task_id],
        )?;
        Ok(())
    }

    /// Get the last inserted row id.
    pub fn last_insert_rowid(&self) -> i64 {
        self.conn.last_insert_rowid()
    }

    /// Execute a PRAGMA statement (which may return results).
    pub fn execute_pragma(&mut self, sql: &str, params: &[&dyn rusqlite::ToSql]) -> Result<()> {
        // PRAGMA statements may return results, so we need to handle them specially
        let mut stmt = self.conn.prepare(sql)?;
        // Just execute and ignore any results
        let _ = stmt
            .query_map(params, |_: &rusqlite::Row| Ok(()))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::Schema;

    #[test]
    fn test_db_path_default() {
        let path = DbPath::default_path();
        assert_eq!(path.as_path(), Path::new("tt.db"));
    }

    #[test]
    fn test_db_path_new() {
        let path = DbPath::new("custom.db");
        assert_eq!(path.as_path(), Path::new("custom.db"));
    }

    #[test]
    fn test_db_path_exists() {
        let path = DbPath::new("nonexistent.db");
        assert!(!path.exists());

        // Create a temp file
        let temp = tempfile::NamedTempFile::new().unwrap();
        let existing = DbPath::new(temp.path());
        assert!(existing.exists());
    }

    #[test]
    fn test_connection_open_in_memory() {
        let mut conn = Connection::open_in_memory().unwrap();
        Schema::init(&mut conn).unwrap();
        assert!(conn.table_exists("tasks").unwrap());
    }

    #[test]
    fn test_transaction() {
        let mut conn = Connection::open_in_memory().unwrap();
        Schema::init(&mut conn).unwrap();

        {
            let tx = conn.transaction().unwrap();
            tx.execute(
                "INSERT INTO tasks (title) VALUES (?)",
                rusqlite::params!("Test Task"),
            )
            .unwrap();
            tx.commit().unwrap();
        }

        // Verify task was committed
        let count: i64 = conn
            .as_conn()
            .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_transaction_rollback() {
        let mut conn = Connection::open_in_memory().unwrap();
        Schema::init(&mut conn).unwrap();

        {
            let tx = conn.transaction().unwrap();
            tx.execute(
                "INSERT INTO tasks (title) VALUES (?)",
                rusqlite::params!("Test Task"),
            )
            .unwrap();
            drop(tx); // Rollback by dropping
        }

        // Verify task was rolled back
        let count: i64 = conn
            .as_conn()
            .query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_update_last_touched() {
        let mut conn = Connection::open_in_memory().unwrap();
        Schema::init(&mut conn).unwrap();

        // Create a task
        conn.as_conn_mut()
            .execute(
                "INSERT INTO tasks (title) VALUES (?)",
                rusqlite::params!("Test"),
            )
            .unwrap();

        let id = conn.last_insert_rowid();

        // Update last_touched
        conn.update_last_touched(id).unwrap();

        // Verify it was updated
        let last_touched: String = conn
            .as_conn()
            .query_row(
                "SELECT last_touched_at FROM tasks WHERE id = ?",
                [id],
                |r| r.get(0),
            )
            .unwrap();

        assert!(!last_touched.is_empty());
    }

    #[test]
    fn test_last_insert_rowid() {
        let mut conn = Connection::open_in_memory().unwrap();
        Schema::init(&mut conn).unwrap();

        conn.as_conn_mut()
            .execute(
                "INSERT INTO tasks (title) VALUES (?)",
                rusqlite::params!("Test"),
            )
            .unwrap();

        let id = conn.last_insert_rowid();
        assert_eq!(id, 1);
    }
}

//! Database layer for tt task tracker.
//!
//! Handles SQLite database connection, schema creation, and low-level queries.

mod connection;
pub mod schema;

pub use connection::{Connection, DbPath};
pub use schema::{ArtifactRow, ConfigRow, DependencyRow, Schema, TaskRow};

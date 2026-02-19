//! # tt - DAG-based Task Tracker
//!
//! A task tracker that manages tasks as nodes in a Directed Acyclic Graph (DAG),
//! stored in SQLite. Designed for AI coding agents with human-set high-level goals.

pub mod cli;
pub mod core;
pub mod db;
pub mod error;
pub mod graph;

// Re-export commonly used types
pub use core::{Artifact, Task, TaskStatus};
pub use error::{Error, Result};

pub use db::Connection;

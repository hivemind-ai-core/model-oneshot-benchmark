//! Core business logic for the tt task tracker.
//!
//! This module contains all the business logic that is shared between
//! the CLI and MCP interfaces.

pub mod db;
pub mod error;
pub mod graph;
pub mod task;

pub use error::{Result, TTError};

//! Error types for the tt task tracker.
//!
//! All errors that can occur in the application are defined here.

use std::io;
use thiserror::Error;

/// Result type alias for convenience.
pub type Result<T> = std::result::Result<T, TTError>;

/// All errors that can occur in the tt application.
#[derive(Error, Debug)]
pub enum TTError {
    /// Task with the given ID was not found.
    #[error("Task #{0} not found")]
    TaskNotFound(i64),

    /// Task is not in the pending state when it needs to be.
    #[error("Task #{0} is not pending, cannot start")]
    TaskNotPending(i64),

    /// Another task is already in progress.
    #[error("Task #{0} is already in progress. Finish or stop it first.")]
    AnotherTaskActive(i64),

    /// No task is currently in progress.
    #[error("No task is currently in progress")]
    NoActiveTask,

    /// Task has unmet dependencies that must be completed first.
    #[error("Cannot start #{0}: dependencies not completed: {1:?}")]
    UnmetDependencies(i64, Vec<i64>),

    /// Adding a dependency would create a cycle.
    #[error("Adding #{0} → #{1} would create a cycle: {2:?}")]
    CycleDetected(i64, i64, Vec<i64>),

    /// No target has been set.
    #[error("No target set. Use `tt target <id>` first.")]
    NoTarget,

    /// All tasks for the target have been completed.
    #[error("Target reached. All tasks for #{0} are completed.")]
    TargetReached(i64),

    /// Task has no definition of done.
    #[error("Task #{0} has no definition of done. Set one with `tt edit {0} --dod`")]
    NoDod(i64),

    /// Order conflict warning - a task has lower manual_order than its prerequisite.
    #[error("Warning: #{0} (order {1}) depends on #{2} (order {3}) which has higher manual_order")]
    OrderConflict(i64, f64, i64, f64),

    /// Invalid status string.
    #[error("Invalid status: {0}")]
    InvalidStatus(String),

    /// All remaining tasks are blocked.
    #[error("All remaining tasks are blocked: {0:?}")]
    AllBlocked(Vec<i64>),

    /// Task deletion is not supported.
    #[error("Task deletion is not supported in v1")]
    DeletionNotSupported,

    /// Float precision exhausted - need to reindex.
    #[error("Float precision exhausted. Run `tt reindex` to fix.")]
    FloatPrecisionExhausted,

    /// At least one of --after or --before is required.
    #[error("At least one of --after or --before is required")]
    AfterOrBeforeRequired,

    /// Database error.
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Invalid state transition.
    #[error("Invalid state transition from {0} to {1}")]
    InvalidTransition(String, String),

    /// MCP error.
    #[error("MCP error: {0}")]
    Mcp(String),
}

impl TTError {
    /// Returns the error code for MCP responses.
    pub fn error_code(&self) -> &'static str {
        match self {
            TTError::TaskNotFound(_) => "TaskNotFound",
            TTError::TaskNotPending(_) => "TaskNotPending",
            TTError::AnotherTaskActive(_) => "AnotherTaskActive",
            TTError::NoActiveTask => "NoActiveTask",
            TTError::UnmetDependencies(_, _) => "UnmetDependencies",
            TTError::CycleDetected(_, _, _) => "CycleDetected",
            TTError::NoTarget => "NoTarget",
            TTError::TargetReached(_) => "TargetReached",
            TTError::NoDod(_) => "NoDod",
            TTError::OrderConflict(_, _, _, _) => "OrderConflict",
            TTError::InvalidStatus(_) => "InvalidStatus",
            TTError::AllBlocked(_) => "AllBlocked",
            TTError::DeletionNotSupported => "DeletionNotSupported",
            TTError::FloatPrecisionExhausted => "FloatPrecisionExhausted",
            TTError::AfterOrBeforeRequired => "AfterOrBeforeRequired",
            TTError::InvalidTransition(_, _) => "InvalidTransition",
            TTError::Db(_) => "DatabaseError",
            TTError::Io(_) => "IoError",
            TTError::Json(_) => "JsonError",
            TTError::Mcp(_) => "McpError",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        let err = TTError::TaskNotFound(123);
        assert_eq!(err.error_code(), "TaskNotFound");

        let err = TTError::AnotherTaskActive(456);
        assert_eq!(err.error_code(), "AnotherTaskActive");

        let err = TTError::NoTarget;
        assert_eq!(err.error_code(), "NoTarget");
    }
}

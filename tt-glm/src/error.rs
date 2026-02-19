//! Error types for the tt task tracker.

use std::io;

/// Result type alias for tt operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Main error enum for the tt task tracker.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Database error.
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Task not found.
    #[error("Task #{0} not found")]
    TaskNotFound(i64),

    /// Task is not in pending status.
    #[error("Task #{0} is not pending, cannot start")]
    TaskNotPending(i64),

    /// Another task is already active.
    #[error("Task #{0} is already in progress. Finish or stop it first.")]
    AnotherTaskActive(i64),

    /// No task is currently active.
    #[error("No task is currently in progress")]
    NoActiveTask,

    /// Task has unmet dependencies.
    #[error("Cannot start #{0}: dependencies not completed: {1}")]
    UnmetDependencies(i64, String),

    /// Cycle detected in dependency graph.
    #[error("Adding #{0} -> #{1} would create a cycle: {2}")]
    CycleDetected(i64, i64, String),

    /// No target set.
    #[error("No target set. Use `tt target <id>` first.")]
    NoTarget,

    /// Target reached - all tasks completed.
    #[error("Target reached. All tasks for #{0} are completed.")]
    TargetReached(i64),

    /// Task has no Definition of Done.
    #[error("Task #{0} has no definition of done. Set one with `tt edit {0} --dod`")]
    NoDod(i64),

    /// Invalid status string.
    #[error("Invalid status: {0}")]
    InvalidStatus(String),

    /// All remaining tasks are blocked.
    #[error("All remaining tasks are blocked: {0}")]
    AllBlocked(String),

    /// Order conflict warning (not an error, but informational).
    #[error("Warning: task ordering issue detected")]
    OrderConflictWarning,

    /// Already initialized.
    #[error("Already initialized in this directory")]
    AlreadyInitialized,

    /// Not initialized.
    #[error("Not initialized. Run `tt init` first")]
    NotInitialized,

    /// Deleting tasks is not supported.
    #[error("Task deletion is not supported in v1")]
    DeleteNotSupported,

    /// Float precision exhausted for ordering.
    #[error("Cannot calculate midpoint: precision exhausted. Run `tt reindex`")]
    PrecisionExhausted,

    /// Invalid transition.
    #[error("Invalid status transition from {0} to {1}")]
    InvalidTransition(String, String),

    /// Cannot modify completed task.
    #[error("Task #{0} is completed and cannot be modified")]
    TaskCompleted(i64),

    /// Task must be blocked to unblock.
    #[error("Task #{0} is not blocked")]
    TaskNotBlocked(i64),

    /// JSON serialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Task already has this dependency.
    #[error("Task #{0} already depends on #{1}")]
    DuplicateDependency(i64, i64),

    /// Both after and before specified for reorder.
    #[error("Cannot specify both --after and --before")]
    BothAfterAndBefore,

    /// Reorder needs at least one of after or before.
    #[error("Must specify at least one of --after or --before")]
    NeedAfterOrBefore,
}

/// Format a list of task IDs as a comma-separated string with # prefix.
pub fn format_task_ids(ids: &[i64]) -> String {
    ids.iter()
        .map(|id| format!("#{id}"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_task_ids() {
        assert_eq!(format_task_ids(&[1, 2, 3]), "#1, #2, #3");
        assert_eq!(format_task_ids(&[42]), "#42");
        assert_eq!(format_task_ids(&[]), "");
    }
}

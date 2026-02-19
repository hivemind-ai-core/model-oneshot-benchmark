use std::io;
use thiserror::Error;

/// Error type for the tt task tracker
#[derive(Error, Debug)]
pub enum Error {
    /// Task with the given ID was not found
    #[error("Task #{id} not found")]
    TaskNotFound { id: i64 },

    /// Task is not in the required status for the operation
    #[error("Task #{id} is not {expected}, cannot {action}")]
    InvalidTaskStatus {
        id: i64,
        current: String,
        expected: String,
        action: String,
    },

    /// Another task is already in progress
    #[error("Task #{id} is already in progress. Finish or stop it first.")]
    AnotherTaskActive { id: i64, title: String },

    /// No task is currently in progress
    #[error("No task is currently in progress")]
    NoActiveTask,

    /// Dependencies not satisfied for starting a task
    #[error("Cannot start task #{id}: dependencies not completed: {unmet_ids:?}")]
    UnmetDependencies { id: i64, unmet_ids: Vec<i64> },

    /// Adding a dependency would create a cycle
    #[error("Adding #{from_id} → #{to_id} would create a cycle: {cycle_path:?}")]
    CycleDetected {
        from_id: i64,
        to_id: i64,
        cycle_path: Vec<i64>,
    },

    /// No target has been set
    #[error("No target set. Use `tt target <id>` first.")]
    NoTarget,

    /// All tasks for the target are completed
    #[error("Target reached. All tasks for #{id} ({title}) are completed.")]
    TargetReached { id: i64, title: String },

    /// Task has no Definition of Done
    #[error(
        "Task #{id} has no definition of done. Set one with `tt edit {id} --dod <definition>`"
    )]
    NoDod { id: i64 },

    /// Manual order conflict warning
    #[error(
        "Warning: task #{id} (order {task_order}) depends on #{dep_id} (order {dep_order}) which has higher manual_order"
    )]
    OrderConflict {
        id: i64,
        task_order: f64,
        dep_id: i64,
        dep_order: f64,
    },

    /// Invalid status string
    #[error("Invalid status: {status}")]
    InvalidStatus { status: String },

    /// All remaining tasks are blocked
    #[error("All remaining tasks are blocked: {blocked_ids:?}")]
    AllBlocked { blocked_ids: Vec<i64> },

    /// Float precision exhausted for ordering
    #[error("Float precision exhausted between orders {a} and {b}. Run `tt reindex` to fix.")]
    FloatPrecisionExhausted { a: f64, b: f64 },

    /// Task deletion not supported
    #[error("Task deletion is not supported in v1")]
    DeletionNotSupported,

    /// Database error
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Invalid transition
    #[error("Invalid status transition from {from} to {to}")]
    InvalidTransition { from: String, to: String },

    /// Both after and before specified for reorder
    #[error("Cannot specify both --after and --before for reorder")]
    ReorderConflict,

    /// Task already has the target status (idempotent operation)
    #[error("Task #{id} is already {status}")]
    AlreadyInState { id: i64, status: String },

    /// Artifact not found
    #[error("Artifact #{id} not found")]
    ArtifactNotFound { id: i64 },
}

/// Result type alias for tt operations
pub type Result<T> = std::result::Result<T, Error>;

/// Helper to convert rusqlite errors that indicate a task not found
impl Error {
    pub fn task_not_found(id: i64) -> Self {
        Error::TaskNotFound { id }
    }

    pub fn invalid_status<S: Into<String>>(status: S) -> Self {
        Error::InvalidStatus {
            status: status.into(),
        }
    }

    pub fn another_task_active(id: i64, title: String) -> Self {
        Error::AnotherTaskActive { id, title }
    }

    pub fn unmet_dependencies(id: i64, unmet_ids: Vec<i64>) -> Self {
        Error::UnmetDependencies { id, unmet_ids }
    }

    pub fn cycle_detected(from_id: i64, to_id: i64, cycle_path: Vec<i64>) -> Self {
        Error::CycleDetected {
            from_id,
            to_id,
            cycle_path,
        }
    }

    pub fn no_target() -> Self {
        Error::NoTarget
    }

    pub fn target_reached(id: i64, title: String) -> Self {
        Error::TargetReached { id, title }
    }

    pub fn no_dod(id: i64) -> Self {
        Error::NoDod { id }
    }

    pub fn all_blocked(blocked_ids: Vec<i64>) -> Self {
        Error::AllBlocked { blocked_ids }
    }
}

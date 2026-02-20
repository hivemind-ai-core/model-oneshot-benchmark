use thiserror::Error;

#[derive(Error, Debug)]
pub enum TTError {
    #[error("Task #{0} not found")]
    TaskNotFound(i64),

    #[error("Task #{0} is not pending, cannot start")]
    TaskNotPending(i64),

    #[error("Task #{0} is already in progress. Finish or stop it first.")]
    AnotherTaskActive(i64),

    #[error("No task is currently in progress")]
    NoActiveTask,

    #[error("Cannot start #{0}: dependencies not completed: {1:?}")]
    UnmetDependencies(i64, Vec<i64>),

    #[error("Adding #{0} → #{1} would create a cycle: {2:?}")]
    CycleDetected(i64, i64, Vec<i64>),

    #[error("No target set. Use `tt target <id>` first.")]
    NoTarget,

    #[error("Target reached. All tasks for #{0} are completed.")]
    TargetReached(i64),

    #[error("Task #{0} has no definition of done. Set one with `tt edit {0} --dod`")]
    NoDod(i64),

    #[error("Warning: Task #{id} (order {order}) depends on #{dep_id} (order {dep_order}) which has higher manual_order")]
    OrderConflict {
        id: i64,
        order: f64,
        dep_id: i64,
        dep_order: f64,
    },

    #[error("Invalid status: {0}")]
    InvalidStatus(String),

    #[error("All remaining tasks are blocked: {0:?}")]
    AllBlocked(Vec<i64>),

    #[error("Cannot calculate midpoint: float precision exhausted. Run `tt reindex` to fix.")]
    FloatPrecisionExhausted,

    #[error("Not initialized. Run `tt init` first.")]
    NotInitialized,

    #[error("Already initialized")]
    AlreadyInitialized,

    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type TTResult<T> = Result<T, TTError>;

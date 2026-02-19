use thiserror::Error;

/// All possible errors in the task tracker
#[derive(Error, Debug)]
pub enum TaskError {
    #[error("Task #{0} not found")]
    TaskNotFound(i64),

    #[error("Task #{0} is not pending, cannot start")]
    TaskNotPending(i64),

    #[error("Task #{0} is already in progress. Finish or stop it first.")]
    AnotherTaskActive(i64),

    #[error("No task is currently in progress")]
    NoActiveTask,

    #[error("Cannot start #{id}: dependencies not completed: {deps}", deps = format_deps(deps))]
    UnmetDependencies { id: i64, deps: Vec<i64> },

    #[error("Adding #{from} -> #{to} would create a cycle: {path}", path = format_cycle(path))]
    CycleDetected { from: i64, to: i64, path: Vec<i64> },

    #[error("No target set. Use `tt target <id>` first.")]
    NoTarget,

    #[error("Target reached. All tasks for #{0} are completed.")]
    TargetReached(i64),

    #[error("Task #{0} has no definition of done. Set one with `tt edit {0} --dod`")]
    NoDod(i64),

    #[error(
        "Warning: #{id} (order {task_order}) depends on #{dep_id} (order {dep_order}) which has higher manual_order"
    )]
    OrderConflict {
        id: i64,
        task_order: f64,
        dep_id: i64,
        dep_order: f64,
    },

    #[error("Invalid status: {0}")]
    InvalidStatus(String),

    #[error("All remaining tasks are blocked: {tasks}", tasks = .0.iter().map(|t| format!("#{} {}", t.id, t.title)).collect::<Vec<_>>().join(", "))]
    AllBlocked(Vec<BlockedTaskSummary>),

    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Project not initialized. Run `tt init` first.")]
    NotInitialized,

    #[error("Project already initialized")]
    AlreadyInitialized,

    #[error("Float precision exhausted. Run `tt reindex` to clean up ordering.")]
    FloatPrecisionExhausted,

    #[error("Task #{0} is not blocked")]
    TaskNotBlocked(i64),

    #[error("Task #{0} is blocked")]
    TaskIsBlocked(i64),

    #[error("Task #{0} is already completed")]
    TaskAlreadyCompleted(i64),

    #[error("Dependency already exists")]
    DependencyAlreadyExists,

    #[error("Dependency not found")]
    DependencyNotFound,

    #[error("Cannot modify manual order: need at least one of --after or --before")]
    MissingPositionHint,

    #[error("Cannot depend on self")]
    SelfDependency,

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("MCP error: {0}")]
    Mcp(String),
}

/// Summary of a blocked task for error messages
#[derive(Debug, Clone)]
pub struct BlockedTaskSummary {
    pub id: i64,
    pub title: String,
}

fn format_cycle(path: &[i64]) -> String {
    path.iter()
        .map(|id| format!("#{id}"))
        .collect::<Vec<_>>()
        .join(" → ")
}

fn format_deps(deps: &[i64]) -> String {
    deps.iter()
        .map(|id| format!("#{id}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Result type alias
pub type Result<T> = std::result::Result<T, TaskError>;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Task #{0} not found")]
    TaskNotFound(i64),

    #[error("Task #{0} is not pending, cannot start")]
    TaskNotPending(i64),

    #[error("Task #{0} is already in progress. Finish or stop it first.")]
    AnotherTaskActive(i64, String),

    #[error("No task is currently in progress")]
    NoActiveTask,

    #[error("Cannot start #{0}: dependencies not completed: #{1}")]
    UnmetDependencies(i64, i64),

    #[error("Adding #{0} → #{1} would create a cycle: {2}")]
    CycleDetected(i64, i64, String),

    #[error("No target set. Use `tt target <id>` first.")]
    NoTarget,

    #[error("Target reached. All tasks for #{0} are completed.")]
    TargetReached(i64),

    #[error("Task #{0} has no definition of done. Set one with `tt edit {0} --dod`")]
    NoDod(i64),

    #[error("Invalid status: {0}")]
    InvalidStatus(String),

    #[error("All remaining tasks are blocked: #{0}")]
    AllBlocked(String),

    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Not supported: {0}")]
    NotSupported(String),

    #[error("Task #{0} is not blocked")]
    TaskNotBlocked(i64),

    #[error("Task #{0} is already completed")]
    TaskAlreadyCompleted(i64),

    #[error("Cannot reorder: {0}")]
    ReorderError(String),
}

impl Error {
    pub fn error_code(&self) -> &'static str {
        match self {
            Error::TaskNotFound(_) => "TaskNotFound",
            Error::TaskNotPending(_) => "TaskNotPending",
            Error::AnotherTaskActive(_, _) => "AnotherTaskActive",
            Error::NoActiveTask => "NoActiveTask",
            Error::UnmetDependencies(_, _) => "UnmetDependencies",
            Error::CycleDetected(_, _, _) => "CycleDetected",
            Error::NoTarget => "NoTarget",
            Error::TargetReached(_) => "TargetReached",
            Error::NoDod(_) => "NoDod",
            Error::InvalidStatus(_) => "InvalidStatus",
            Error::AllBlocked(_) => "AllBlocked",
            Error::Db(_) => "Db",
            Error::Io(_) => "Io",
            Error::Serde(_) => "Serde",
            Error::NotSupported(_) => "NotSupported",
            Error::TaskNotBlocked(_) => "TaskNotBlocked",
            Error::TaskAlreadyCompleted(_) => "TaskAlreadyCompleted",
            Error::ReorderError(_) => "ReorderError",
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

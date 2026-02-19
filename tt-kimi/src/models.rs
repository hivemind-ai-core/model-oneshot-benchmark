use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Task status in the state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pending,
    InProgress,
    Completed,
    Blocked,
}

impl Status {
    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Pending => "pending",
            Status::InProgress => "in_progress",
            Status::Completed => "completed",
            Status::Blocked => "blocked",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Status::Completed => "✓",
            Status::InProgress => "●",
            Status::Pending => "○",
            Status::Blocked => "✗",
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl TryFrom<&str> for Status {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "pending" => Ok(Status::Pending),
            "in_progress" => Ok(Status::InProgress),
            "completed" => Ok(Status::Completed),
            "blocked" => Ok(Status::Blocked),
            _ => Err(format!("Invalid status: {s}")),
        }
    }
}

/// A task in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub dod: Option<String>, // Definition of Done
    pub status: Status,
    pub manual_order: f64,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub last_touched_at: DateTime<Utc>,
}

/// An artifact linked to a task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: i64,
    pub task_id: i64,
    pub name: String,
    pub file_path: String,
    pub created_at: DateTime<Utc>,
}

/// A dependency edge in the graph
#[derive(Debug, Clone)]
pub struct Dependency {
    pub task_id: i64,
    pub depends_on: i64,
}

/// Full task details including dependencies and artifacts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDetail {
    #[serde(flatten)]
    pub task: Task,
    pub dependencies: Vec<DependencyInfo>,
    pub dependents: Vec<i64>,
    pub artifacts: Vec<Artifact>,
}

/// Dependency info with task status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyInfo {
    pub id: i64,
    pub title: String,
    pub status: Status,
}

/// Task with its dependency IDs for graph operations
#[derive(Debug, Clone)]
pub struct TaskWithDeps {
    pub task: Task,
    pub dependency_ids: Vec<i64>,
}

/// Configuration key-value pair
#[derive(Debug, Clone)]
pub struct ConfigEntry {
    pub key: String,
    pub value: String,
}

/// New task input
#[derive(Debug, Clone, Default)]
pub struct NewTask {
    pub title: String,
    pub description: Option<String>,
    pub dod: Option<String>,
    pub manual_order: Option<f64>,
}

/// Task update input
#[derive(Debug, Clone, Default)]
pub struct TaskUpdate {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub dod: Option<Option<String>>,
}

/// Next task result
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum NextTaskResult {
    Task { task: TaskDetail },
    TargetReached { target_id: i64 },
    AllBlocked { tasks: Vec<BlockedTaskInfo> },
}

/// Blocked task info for error reporting
#[derive(Debug, Clone, Serialize)]
pub struct BlockedTaskInfo {
    pub id: i64,
    pub title: String,
    pub waiting_on: Vec<WaitingOnInfo>,
}

/// What a blocked task is waiting on
#[derive(Debug, Clone, Serialize)]
pub struct WaitingOnInfo {
    pub id: i64,
    pub title: String,
    pub status: Status,
}

/// Order conflict warning
#[derive(Debug, Clone, Serialize)]
pub struct OrderConflict {
    pub task_id: i64,
    pub task_order: f64,
    pub dep_id: i64,
    pub dep_order: f64,
}

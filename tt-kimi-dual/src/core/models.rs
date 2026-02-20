use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Blocked,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Completed => "completed",
            TaskStatus::Blocked => "blocked",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "○",
            TaskStatus::InProgress => "●",
            TaskStatus::Completed => "✓",
            TaskStatus::Blocked => "✗",
        }
    }
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for TaskStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(TaskStatus::Pending),
            "in_progress" => Ok(TaskStatus::InProgress),
            "completed" => Ok(TaskStatus::Completed),
            "blocked" => Ok(TaskStatus::Blocked),
            _ => Err(format!("Invalid status: {s}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub dod: Option<String>,
    pub status: TaskStatus,
    pub manual_order: f64,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub last_touched_at: DateTime<Utc>,
}

impl Task {
    pub fn is_pending(&self) -> bool {
        matches!(self.status, TaskStatus::Pending)
    }

    pub fn is_in_progress(&self) -> bool {
        matches!(self.status, TaskStatus::InProgress)
    }

    pub fn is_completed(&self) -> bool {
        matches!(self.status, TaskStatus::Completed)
    }

    pub fn is_blocked(&self) -> bool {
        matches!(self.status, TaskStatus::Blocked)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: i64,
    pub task_id: i64,
    pub name: String,
    pub file_path: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub task_id: i64,
    pub depends_on: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskWithDependencies {
    #[serde(flatten)]
    pub task: Task,
    pub dependencies: Vec<Task>,
    pub dependents: Vec<Task>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDetail {
    #[serde(flatten)]
    pub task: Task,
    pub dependencies: Vec<Task>,
    pub dependents: Vec<i64>,
    pub artifacts: Vec<Artifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderConflict {
    pub task_id: i64,
    pub task_order: f64,
    pub dep_id: i64,
    pub dep_order: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct NextTaskResult {
    pub task: Task,
    pub dependencies_met: bool,
    pub unmet_dependencies: Vec<i64>,
}

use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Task status enum
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

    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "pending" => Ok(TaskStatus::Pending),
            "in_progress" => Ok(TaskStatus::InProgress),
            "completed" => Ok(TaskStatus::Completed),
            "blocked" => Ok(TaskStatus::Blocked),
            _ => Err(crate::error::Error::invalid_status(s)),
        }
    }

    /// Returns the display character for this status
    pub fn display_char(&self) -> char {
        match self {
            TaskStatus::Pending => '○',
            TaskStatus::InProgress => '●',
            TaskStatus::Completed => '✓',
            TaskStatus::Blocked => '✗',
        }
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = crate::error::Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::from_str(s)
    }
}

/// A task in the DAG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dod: Option<String>,
    pub status: TaskStatus,
    pub manual_order: f64,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    pub last_touched_at: String,
}

impl Task {
    /// Parse a datetime string to DateTime<Utc>
    pub fn parse_datetime(s: &str) -> Result<DateTime<Utc>> {
        Ok(s.parse::<DateTime<Utc>>()
            .map_err(|_| crate::error::Error::InvalidStatus {
                status: format!("Invalid datetime: {}", s),
            })?)
    }

    /// Get the display time for a datetime string
    pub fn format_datetime(s: &str) -> String {
        match DateTime::parse_from_rfc3339(s) {
            Ok(dt) => dt.format("%Y-%m-%d %H:%M").to_string(),
            Err(_) => s.to_string(),
        }
    }
}

/// A dependency relationship between tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub task_id: i64,
    pub depends_on: i64,
}

/// An artifact (file) linked to a task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: i64,
    pub task_id: i64,
    pub name: String,
    pub file_path: String,
    pub created_at: String,
}

/// Config key-value pair
#[derive(Debug, Clone)]
pub struct Config {
    pub key: String,
    pub value: String,
}

/// Task with additional context (dependencies, dependents, artifacts)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDetail {
    pub task: Task,
    pub dependencies: Vec<TaskDependencyInfo>,
    pub dependents: Vec<i64>,
    pub artifacts: Vec<Artifact>,
}

/// Information about a dependency task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDependencyInfo {
    pub id: i64,
    pub title: String,
    pub status: TaskStatus,
}

/// Response for next task operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NextTaskResponse {
    Next(Task),
    TargetReached { id: i64, title: String },
    AllBlocked { blocked_ids: Vec<(i64, String)> },
}

/// Summary info about a task for display
#[derive(Debug, Clone)]
pub struct TaskSummary {
    pub id: i64,
    pub title: String,
    pub status: TaskStatus,
    pub manual_order: f64,
    pub dependency_ids: Vec<i64>,
    pub all_deps_completed: bool,
}

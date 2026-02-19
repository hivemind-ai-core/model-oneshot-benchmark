//! Task model and operations.

use serde::{Deserialize, Serialize};

/// Task status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Blocked,
}

impl TaskStatus {
    /// Parse a string into a TaskStatus.
    pub fn parse(s: &str) -> crate::Result<Self> {
        match s {
            "pending" => Ok(TaskStatus::Pending),
            "in_progress" => Ok(TaskStatus::InProgress),
            "completed" => Ok(TaskStatus::Completed),
            "blocked" => Ok(TaskStatus::Blocked),
            _ => Err(crate::error::Error::InvalidStatus(s.to_string())),
        }
    }

    /// Convert to string for database storage.
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Completed => "completed",
            TaskStatus::Blocked => "blocked",
        }
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A task in the DAG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub dod: Option<String>,
    pub status: TaskStatus,
    pub manual_order: f64,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub last_touched_at: String,
}

impl Task {
    /// Convert a TaskRow to a Task.
    pub fn from_row(row: crate::db::schema::TaskRow) -> crate::Result<Self> {
        Ok(Self {
            id: row.id,
            title: row.title,
            description: row.description,
            dod: row.dod,
            status: TaskStatus::parse(&row.status)?,
            manual_order: row.manual_order,
            created_at: row.created_at,
            started_at: row.started_at,
            completed_at: row.completed_at,
            last_touched_at: row.last_touched_at,
        })
    }

    /// Get the status character for display.
    pub fn status_char(&self) -> char {
        match self.status {
            TaskStatus::Pending => '○',
            TaskStatus::InProgress => '●',
            TaskStatus::Completed => '✓',
            TaskStatus::Blocked => '✗',
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_status_parse() {
        assert_eq!(TaskStatus::parse("pending").unwrap(), TaskStatus::Pending);
        assert_eq!(
            TaskStatus::parse("in_progress").unwrap(),
            TaskStatus::InProgress
        );
        assert_eq!(
            TaskStatus::parse("completed").unwrap(),
            TaskStatus::Completed
        );
        assert_eq!(TaskStatus::parse("blocked").unwrap(), TaskStatus::Blocked);
        assert!(TaskStatus::parse("invalid").is_err());
    }

    #[test]
    fn test_task_status_as_str() {
        assert_eq!(TaskStatus::Pending.as_str(), "pending");
        assert_eq!(TaskStatus::InProgress.as_str(), "in_progress");
        assert_eq!(TaskStatus::Completed.as_str(), "completed");
        assert_eq!(TaskStatus::Blocked.as_str(), "blocked");
    }

    #[test]
    fn test_task_status_display() {
        assert_eq!(format!("{}", TaskStatus::Pending), "pending");
        assert_eq!(format!("{}", TaskStatus::InProgress), "in_progress");
    }

    #[test]
    fn test_task_status_char() {
        let task = Task {
            id: 1,
            title: "Test".to_string(),
            description: None,
            dod: None,
            status: TaskStatus::Pending,
            manual_order: 10.0,
            created_at: String::new(),
            started_at: None,
            completed_at: None,
            last_touched_at: String::new(),
        };
        assert_eq!(task.status_char(), '○');

        let task = Task {
            status: TaskStatus::InProgress,
            ..task
        };
        assert_eq!(task.status_char(), '●');

        let task = Task {
            status: TaskStatus::Completed,
            ..task
        };
        assert_eq!(task.status_char(), '✓');

        let task = Task {
            status: TaskStatus::Blocked,
            ..task
        };
        assert_eq!(task.status_char(), '✗');
    }
}

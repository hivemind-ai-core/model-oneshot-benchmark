use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Pending,
    InProgress,
    Completed,
    Blocked,
}

impl Status {
    pub fn from_db_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "pending" => Some(Status::Pending),
            "in_progress" => Some(Status::InProgress),
            "completed" => Some(Status::Completed),
            "blocked" => Some(Status::Blocked),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Pending => "pending",
            Status::InProgress => "in_progress",
            Status::Completed => "completed",
            Status::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub dod: Option<String>,
    pub status: Status,
    pub manual_order: f64,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub last_touched_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskWithDeps {
    pub task: Task,
    pub dependencies: Vec<DependencyInfo>,
    pub dependents: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyInfo {
    pub id: i64,
    pub title: String,
    pub status: Status,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: i64,
    pub task_id: i64,
    pub name: String,
    pub file_path: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListOptions {
    pub all: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskFilter {
    pub pending: bool,
    pub in_progress: bool,
    pub completed: bool,
    pub blocked: bool,
}

impl TaskFilter {
    pub fn all() -> Self {
        Self {
            pending: true,
            in_progress: true,
            completed: true,
            blocked: true,
        }
    }

    pub fn active() -> Self {
        Self {
            pending: true,
            in_progress: true,
            completed: false,
            blocked: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskOptions {
    pub title: String,
    pub description: Option<String>,
    pub dod: Option<String>,
    pub after_id: Option<i64>,
    pub before_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditTaskOptions {
    pub title: Option<String>,
    pub description: Option<String>,
    pub dod: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReorderOptions {
    pub after_id: Option<i64>,
    pub before_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    pub status: String,
    pub data: Option<serde_json::Value>,
    pub error_code: Option<String>,
    pub message: Option<String>,
}

impl McpResponse {
    pub fn ok<T: Serialize>(data: T) -> Self {
        Self {
            status: "ok".to_string(),
            data: Some(serde_json::to_value(data).unwrap_or(serde_json::Value::Null)),
            error_code: None,
            message: None,
        }
    }

    pub fn error<T: Into<String>>(error_code: &str, message: T) -> Self {
        Self {
            status: "error".to_string(),
            data: None,
            error_code: Some(error_code.to_string()),
            message: Some(message.into()),
        }
    }
}

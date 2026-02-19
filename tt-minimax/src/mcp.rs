use crate::core::*;
use crate::db::Db;
use crate::error::{Error, Result};

/// Start the MCP server over stdio
pub async fn run_mcp() -> Result<()> {
    // TODO: Implement full MCP server using rmcp
    // For now, just return an error indicating it's not yet implemented
    Err(Error::Io(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "MCP server not yet implemented. The rmcp crate requires async runtime setup.",
    )))
}

/// MCP tool definitions and handlers
/// This is a placeholder for future implementation
pub struct McpServer {
    db: Db,
}

impl McpServer {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// Get the next task to work on
    pub fn get_next_task(&self) -> Result<McpResponse> {
        match get_next(&self.db, None) {
            Ok(task) => Ok(McpResponse::Task(task.into())),
            Err(Error::TargetReached { id, title }) => Ok(McpResponse::TargetReached { id, title }),
            Err(Error::AllBlocked { blocked_ids }) => Ok(McpResponse::AllBlocked { blocked_ids }),
            Err(e) => Err(e),
        }
    }

    /// Get the current task
    pub fn get_current_task(&self) -> Result<McpResponse> {
        let task = workflow::get_current_task(&self.db)?;
        let artifacts = get_artifacts_for_task(&self.db, task.id)?;
        Ok(McpResponse::CurrentTask {
            task: task.into(),
            artifacts: artifacts.into_iter().map(|a| a.into()).collect(),
        })
    }

    /// Start a task
    pub fn start_task(&mut self, id: i64) -> Result<McpResponse> {
        let task = workflow::start_task(&mut self.db, id)?;
        Ok(McpResponse::Task(task.into()))
    }

    /// Complete the current task
    pub fn complete_task(&mut self) -> Result<McpResponse> {
        let task = workflow::complete_task(&mut self.db)?;
        Ok(McpResponse::Task(task.into()))
    }

    /// Stop the current task
    pub fn stop_task(&mut self) -> Result<McpResponse> {
        let task = workflow::stop_task(&mut self.db)?;
        Ok(McpResponse::Task(task.into()))
    }

    /// Create a new task
    pub fn create_task(
        &mut self,
        title: String,
        description: Option<String>,
        dod: Option<String>,
        after_id: Option<i64>,
        before_id: Option<i64>,
    ) -> Result<McpResponse> {
        let task = add_task(&mut self.db, title, description, dod, after_id, before_id)?;
        Ok(McpResponse::Task(task.into()))
    }

    /// Edit a task
    pub fn edit_task(
        &mut self,
        id: i64,
        title: Option<String>,
        description: Option<String>,
        dod: Option<String>,
    ) -> Result<McpResponse> {
        let task = edit_task(&mut self.db, id, title, description, dod)?;
        Ok(McpResponse::Task(task.into()))
    }

    /// Show a task
    pub fn show_task(&self, id: i64) -> Result<McpResponse> {
        let detail = show_task(&self.db, id)?;
        Ok(McpResponse::TaskDetail(detail.into()))
    }

    /// List tasks
    pub fn list_tasks(&self, all: bool) -> Result<McpResponse> {
        let tasks = list_tasks(&self.db, None, all)?;
        Ok(McpResponse::TaskList(
            tasks.into_iter().map(|t| t.into()).collect(),
        ))
    }

    /// Add a dependency
    pub fn add_dependency(&mut self, task_id: i64, depends_on: i64) -> Result<McpResponse> {
        workflow::add_dependency(&mut self.db, task_id, depends_on)?;
        Ok(McpResponse::Success {
            message: format!("Task #{} now depends on #{}", task_id, depends_on),
        })
    }

    /// Remove a dependency
    pub fn remove_dependency(&mut self, task_id: i64, depends_on: i64) -> Result<McpResponse> {
        workflow::remove_dependency(&mut self.db, task_id, depends_on)?;
        Ok(McpResponse::Success {
            message: format!("Task #{} no longer depends on #{}", task_id, depends_on),
        })
    }

    /// Block a task
    pub fn block_task(&mut self, id: i64) -> Result<McpResponse> {
        let task = workflow::block_task(&mut self.db, id)?;
        Ok(McpResponse::Task(task.into()))
    }

    /// Unblock a task
    pub fn unblock_task(&mut self, id: i64) -> Result<McpResponse> {
        let task = workflow::unblock_task(&mut self.db, id)?;
        Ok(McpResponse::Task(task.into()))
    }

    /// Log an artifact
    pub fn log_artifact(&mut self, name: String, file_path: String) -> Result<McpResponse> {
        let artifact = workflow::log_artifact(&mut self.db, name, file_path)?;
        Ok(McpResponse::Artifact(artifact.into()))
    }

    /// Get artifacts
    pub fn get_artifacts(&self, task_id: Option<i64>) -> Result<McpResponse> {
        let artifacts = workflow::get_artifacts(&self.db, task_id)?;
        Ok(McpResponse::ArtifactList(
            artifacts.into_iter().map(|a| a.into()).collect(),
        ))
    }

    /// Set target
    pub fn set_target(&mut self, id: i64) -> Result<McpResponse> {
        target::set_target(&mut self.db, id)?;
        Ok(McpResponse::Success {
            message: format!("Set target to #{}", id),
        })
    }

    /// Reorder a task
    pub fn reorder_task(
        &mut self,
        id: i64,
        after_id: Option<i64>,
        before_id: Option<i64>,
    ) -> Result<McpResponse> {
        let new_order = reorder_task(&mut self.db, id, after_id, before_id)?;
        Ok(McpResponse::Reorder { id, new_order })
    }
}

/// MCP response types
#[derive(Debug, Clone, serde::Serialize)]
#[serde(untagged)]
pub enum McpResponse {
    Task(McpTask),
    TaskDetail(McpTaskDetail),
    TaskList(Vec<McpTask>),
    Artifact(McpArtifact),
    ArtifactList(Vec<McpArtifact>),
    CurrentTask {
        task: McpTask,
        artifacts: Vec<McpArtifact>,
    },
    TargetReached {
        id: i64,
        title: String,
    },
    AllBlocked {
        blocked_ids: Vec<i64>,
    },
    Success {
        message: String,
    },
    Reorder {
        id: i64,
        new_order: f64,
    },
    Error {
        error_code: String,
        message: String,
    },
}

/// Task representation for MCP
#[derive(Debug, Clone, serde::Serialize)]
pub struct McpTask {
    pub id: i64,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dod: Option<String>,
    pub status: String,
    pub manual_order: f64,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    pub last_touched_at: String,
}

impl From<crate::db::Task> for McpTask {
    fn from(task: crate::db::Task) -> Self {
        Self {
            id: task.id,
            title: task.title,
            description: task.description,
            dod: task.dod,
            status: task.status.as_str().to_string(),
            manual_order: task.manual_order,
            created_at: task.created_at,
            started_at: task.started_at,
            completed_at: task.completed_at,
            last_touched_at: task.last_touched_at,
        }
    }
}

/// Task detail representation for MCP
#[derive(Debug, Clone, serde::Serialize)]
pub struct McpTaskDetail {
    pub task: McpTask,
    pub dependencies: Vec<McpDependencyInfo>,
    pub dependents: Vec<i64>,
    pub artifacts: Vec<McpArtifact>,
}

impl From<crate::db::TaskDetail> for McpTaskDetail {
    fn from(detail: crate::db::TaskDetail) -> Self {
        Self {
            task: detail.task.into(),
            dependencies: detail
                .dependencies
                .into_iter()
                .map(|d| McpDependencyInfo {
                    id: d.id,
                    title: d.title,
                    status: d.status.as_str().to_string(),
                })
                .collect(),
            dependents: detail.dependents,
            artifacts: detail.artifacts.into_iter().map(|a| a.into()).collect(),
        }
    }
}

/// Dependency info for MCP
#[derive(Debug, Clone, serde::Serialize)]
pub struct McpDependencyInfo {
    pub id: i64,
    pub title: String,
    pub status: String,
}

/// Artifact representation for MCP
#[derive(Debug, Clone, serde::Serialize)]
pub struct McpArtifact {
    pub id: i64,
    pub task_id: i64,
    pub name: String,
    pub file_path: String,
    pub created_at: String,
}

impl From<crate::db::Artifact> for McpArtifact {
    fn from(artifact: crate::db::Artifact) -> Self {
        Self {
            id: artifact.id,
            task_id: artifact.task_id,
            name: artifact.name,
            file_path: artifact.file_path,
            created_at: artifact.created_at,
        }
    }
}

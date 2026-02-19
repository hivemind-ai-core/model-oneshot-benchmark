use crate::core::TaskTracker;
use crate::error::TaskError;
use crate::models::NextTaskResult;
use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt, handler::server::tool::ToolRouter,
    handler::server::wrapper::Parameters, model::*, schemars, tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Task tracker MCP server
#[derive(Clone)]
pub struct TaskTrackerMcp {
    tracker: Arc<Mutex<TaskTracker>>,
    tool_router: ToolRouter<Self>,
}

// Input/Output types for tools
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreateTaskInput {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dod: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_id: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskIdInput {
    pub id: i64,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EditTaskInput {
    pub id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dod: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DependencyInput {
    pub task_id: i64,
    pub depends_on: i64,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LogArtifactInput {
    pub name: String,
    pub file_path: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListTasksInput {
    #[serde(default)]
    pub all: bool,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReorderInput {
    pub id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_id: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetTargetInput {
    pub id: i64,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetArtifactsInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<i64>,
}

// Response type
#[derive(Debug, Serialize)]
pub struct McpResponse<T: Serialize> {
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl<T: Serialize> McpResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            status: "ok",
            data: Some(data),
            error_code: None,
            message: None,
        }
    }

    pub fn error(error_code: &str, message: &str) -> Self {
        Self {
            status: "error",
            data: None,
            error_code: Some(error_code.to_string()),
            message: Some(message.to_string()),
        }
    }
}

fn to_json<T: Serialize>(response: McpResponse<T>) -> Result<CallToolResult, McpError> {
    let json = serde_json::to_string(&response)
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
    Ok(CallToolResult::success(vec![Content::text(json)]))
}

fn error_to_response(e: TaskError) -> McpResponse<serde_json::Value> {
    let error_code = format!("{e:?}");
    let message = e.to_string();
    McpResponse::error(&error_code, &message)
}

#[tool_router]
impl TaskTrackerMcp {
    pub fn new() -> Result<Self, TaskError> {
        let tracker = TaskTracker::open()?;
        Ok(Self {
            tracker: Arc::new(Mutex::new(tracker)),
            tool_router: Self::tool_router(),
        })
    }

    #[tool(
        description = "Returns the next task to work on toward the current target. Call this after completing a task. If the response is TargetReached, stop working and report to the user."
    )]
    async fn get_next_task(&self) -> Result<CallToolResult, McpError> {
        let tracker = self.tracker.lock().await;

        let result = tracker.get_next_task(false);

        let response = match result {
            Ok(NextTaskResult::Task { task }) => {
                to_json(McpResponse::success(serde_json::to_value(task).unwrap()))?
            }
            Ok(NextTaskResult::TargetReached { target_id }) => {
                to_json(McpResponse::success(serde_json::json!({
                    "type": "TargetReached",
                    "target_id": target_id
                })))?
            }
            Ok(NextTaskResult::AllBlocked { tasks }) => {
                to_json(McpResponse::success(serde_json::json!({
                    "type": "AllBlocked",
                    "tasks": tasks
                })))?
            }
            Err(e) => to_json(error_to_response(e))?,
        };

        Ok(response)
    }

    #[tool(
        description = "Returns the currently active task being worked on, including its artifacts."
    )]
    async fn get_current_task(&self) -> Result<CallToolResult, McpError> {
        let tracker = self.tracker.lock().await;

        match tracker.get_current_task() {
            Ok(detail) => to_json(McpResponse::success(serde_json::to_value(detail).unwrap())),
            Err(e) => to_json(error_to_response(e)),
        }
    }

    #[tool(
        description = "Start working on a specific task. This will fail if another task is already active or if dependencies are not yet completed."
    )]
    async fn start_task(
        &self,
        params: Parameters<TaskIdInput>,
    ) -> Result<CallToolResult, McpError> {
        let tracker = self.tracker.lock().await;
        let id = params.0.id;

        match tracker.start_task(id) {
            Ok(detail) => to_json(McpResponse::success(serde_json::to_value(detail).unwrap())),
            Err(e) => to_json(error_to_response(e)),
        }
    }

    #[tool(
        description = "Complete the currently active task. The task must have a Definition of Done set. Call this when you've finished the work."
    )]
    async fn complete_task(&self) -> Result<CallToolResult, McpError> {
        let tracker = self.tracker.lock().await;

        match tracker.complete_task() {
            Ok(detail) => to_json(McpResponse::success(serde_json::to_value(detail).unwrap())),
            Err(e) => to_json(error_to_response(e)),
        }
    }

    #[tool(
        description = "Stop the currently active task and return it to pending status. Use this if you need to pause work on the current task."
    )]
    async fn stop_task(&self) -> Result<CallToolResult, McpError> {
        let tracker = self.tracker.lock().await;

        match tracker.stop_task() {
            Ok(detail) => to_json(McpResponse::success(serde_json::to_value(detail).unwrap())),
            Err(e) => to_json(error_to_response(e)),
        }
    }

    #[tool(
        description = "Create a new task. If you discover during implementation that a task needs to be broken into smaller pieces, create subtasks and add dependencies."
    )]
    async fn create_task(
        &self,
        params: Parameters<CreateTaskInput>,
    ) -> Result<CallToolResult, McpError> {
        let tracker = self.tracker.lock().await;
        let p = params.0;

        match tracker.create_task(
            &p.title,
            p.description.as_deref(),
            p.dod.as_deref(),
            p.after_id,
            p.before_id,
        ) {
            Ok(task) => to_json(McpResponse::success(serde_json::to_value(task).unwrap())),
            Err(e) => to_json(error_to_response(e)),
        }
    }

    #[tool(
        description = "Edit an existing task's title, description, or Definition of Done. Only the fields you provide will be changed."
    )]
    async fn edit_task(
        &self,
        params: Parameters<EditTaskInput>,
    ) -> Result<CallToolResult, McpError> {
        let tracker = self.tracker.lock().await;
        let p = params.0;

        let desc = p.description.as_ref().map(|d| Some(d.as_str()));
        let dod = p.dod.as_ref().map(|d| Some(d.as_str()));

        match tracker.update_task(p.id, p.title.as_deref(), desc, dod) {
            Ok(detail) => to_json(McpResponse::success(serde_json::to_value(detail).unwrap())),
            Err(e) => to_json(error_to_response(e)),
        }
    }

    #[tool(
        description = "Show full details of a specific task including its dependencies, dependents, and artifacts."
    )]
    async fn show_task(&self, params: Parameters<TaskIdInput>) -> Result<CallToolResult, McpError> {
        let tracker = self.tracker.lock().await;
        let id = params.0.id;

        match tracker.get_task(id) {
            Ok(detail) => to_json(McpResponse::success(serde_json::to_value(detail).unwrap())),
            Err(e) => to_json(error_to_response(e)),
        }
    }

    #[tool(
        description = "List all tasks. By default shows only tasks in the target subgraph. Use all=true to see every task in the system."
    )]
    async fn list_tasks(
        &self,
        params: Parameters<ListTasksInput>,
    ) -> Result<CallToolResult, McpError> {
        let tracker = self.tracker.lock().await;
        let all = params.0.all;

        match tracker.list_tasks(all) {
            Ok((tasks, conflicts)) => to_json(McpResponse::success(serde_json::json!({
                "tasks": tasks,
                "order_conflicts": conflicts
            }))),
            Err(e) => to_json(error_to_response(e)),
        }
    }

    #[tool(
        description = "Add a dependency: task_id will depend on depends_on being completed first. This will fail if it would create a cycle in the dependency graph."
    )]
    async fn add_dependency(
        &self,
        params: Parameters<DependencyInput>,
    ) -> Result<CallToolResult, McpError> {
        let tracker = self.tracker.lock().await;
        let p = params.0;

        match tracker.add_dependency(p.task_id, p.depends_on) {
            Ok(()) => to_json(McpResponse::success(serde_json::json!({
                "message": format!("Task #{} now depends on task #{}", p.task_id, p.depends_on)
            }))),
            Err(e) => to_json(error_to_response(e)),
        }
    }

    #[tool(description = "Remove a dependency relationship between two tasks.")]
    async fn remove_dependency(
        &self,
        params: Parameters<DependencyInput>,
    ) -> Result<CallToolResult, McpError> {
        let tracker = self.tracker.lock().await;
        let p = params.0;

        match tracker.remove_dependency(p.task_id, p.depends_on) {
            Ok(()) => to_json(McpResponse::success(serde_json::json!({
                "message": format!("Removed dependency: #{} no longer depends on #{}", p.task_id, p.depends_on)
            }))),
            Err(e) => to_json(error_to_response(e)),
        }
    }

    #[tool(
        description = "Block a task, preventing it from being started. Use this when a task is waiting on external factors or has issues that need resolution."
    )]
    async fn block_task(
        &self,
        params: Parameters<TaskIdInput>,
    ) -> Result<CallToolResult, McpError> {
        let tracker = self.tracker.lock().await;
        let id = params.0.id;

        match tracker.block_task(id) {
            Ok(detail) => to_json(McpResponse::success(serde_json::to_value(detail).unwrap())),
            Err(e) => to_json(error_to_response(e)),
        }
    }

    #[tool(description = "Unblock a task, returning it to pending status so it can be worked on.")]
    async fn unblock_task(
        &self,
        params: Parameters<TaskIdInput>,
    ) -> Result<CallToolResult, McpError> {
        let tracker = self.tracker.lock().await;
        let id = params.0.id;

        match tracker.unblock_task(id) {
            Ok(detail) => to_json(McpResponse::success(serde_json::to_value(detail).unwrap())),
            Err(e) => to_json(error_to_response(e)),
        }
    }

    #[tool(
        description = "Records a file you have created as an artifact of the current task. Create the file first, then call this. Use descriptive names like 'research', 'plan', 'implementation-notes', 'test-report'."
    )]
    async fn log_artifact(
        &self,
        params: Parameters<LogArtifactInput>,
    ) -> Result<CallToolResult, McpError> {
        let tracker = self.tracker.lock().await;
        let p = params.0;

        match tracker.log_artifact(&p.name, &p.file_path) {
            Ok(artifact) => to_json(McpResponse::success(
                serde_json::to_value(artifact).unwrap(),
            )),
            Err(e) => to_json(error_to_response(e)),
        }
    }

    #[tool(
        description = "Get all artifacts for a specific task, or for the currently active task if no task_id is provided."
    )]
    async fn get_artifacts(
        &self,
        params: Parameters<GetArtifactsInput>,
    ) -> Result<CallToolResult, McpError> {
        let tracker = self.tracker.lock().await;

        match tracker.get_artifacts(params.0.task_id) {
            Ok(artifacts) => to_json(McpResponse::success(
                serde_json::to_value(artifacts).unwrap(),
            )),
            Err(e) => to_json(error_to_response(e)),
        }
    }

    #[tool(
        description = "Set the target task. When a target is set, get_next_task and list_tasks will only consider the subgraph of tasks that are transitive dependencies of the target."
    )]
    async fn set_target(
        &self,
        params: Parameters<SetTargetInput>,
    ) -> Result<CallToolResult, McpError> {
        let tracker = self.tracker.lock().await;
        let id = params.0.id;

        match tracker.set_target(id) {
            Ok(()) => to_json(McpResponse::success(serde_json::json!({
                "message": format!("Set target to task #{}", id)
            }))),
            Err(e) => to_json(error_to_response(e)),
        }
    }

    #[tool(description = "Get the current target task ID, if one is set.")]
    async fn get_target(&self) -> Result<CallToolResult, McpError> {
        let tracker = self.tracker.lock().await;

        match tracker.get_target() {
            Ok(Some(id)) => to_json(McpResponse::success(serde_json::json!({
                "target_id": id
            }))),
            Ok(None) => to_json(McpResponse::success(serde_json::json!({
                "target_id": None::<i64>
            }))),
            Err(e) => to_json(error_to_response(e)),
        }
    }

    #[tool(
        description = "Reorder a task by specifying its new position. Provide either after_id, before_id, or both."
    )]
    async fn reorder_task(
        &self,
        params: Parameters<ReorderInput>,
    ) -> Result<CallToolResult, McpError> {
        let tracker = self.tracker.lock().await;
        let p = params.0;

        match tracker.reorder_task(p.id, p.after_id, p.before_id) {
            Ok(task) => to_json(McpResponse::success(serde_json::to_value(task).unwrap())),
            Err(e) => to_json(error_to_response(e)),
        }
    }

    #[tool(
        description = "Reindex all task orders to clean integers (10.0, 20.0, 30.0, etc.) preserving current sorted order."
    )]
    async fn reindex(&self) -> Result<CallToolResult, McpError> {
        let tracker = self.tracker.lock().await;

        match tracker.reindex() {
            Ok(tasks) => to_json(McpResponse::success(serde_json::json!({
                "reindexed_count": tasks.len()
            }))),
            Err(e) => to_json(error_to_response(e)),
        }
    }
}

#[tool_handler]
impl ServerHandler for TaskTrackerMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Task Tracker - A DAG-based task management system. Use this to manage software development tasks with dependencies. \
                 Key workflow: 1) Set a target with set_target, 2) Call get_next_task to find what to work on, 3) Call start_task to begin work, \
                 4) Create artifacts and log them with log_artifact, 5) Call complete_task when done, 6) Repeat from step 2.".to_string()
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

pub async fn run_mcp_server() -> Result<(), Box<dyn std::error::Error>> {
    let mcp = TaskTrackerMcp::new().map_err(|e| {
        eprintln!("Failed to initialize MCP server: {e}");
        e
    })?;

    let service = mcp.serve(stdio()).await.inspect_err(|e| {
        eprintln!("Error starting MCP server: {e}");
    })?;

    service.waiting().await?;
    Ok(())
}

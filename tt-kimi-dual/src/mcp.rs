//! MCP Server implementation using JSON-RPC over stdio
//!
//! This implements the Model Context Protocol manually since the rmcp crate
//! API has changed significantly between versions.

use crate::core::error::TTError;
use crate::core::AppCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

const DB_FILE: &str = "tt.db";

/// JSON-RPC request
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

/// JSON-RPC response
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

impl JsonRpcResponse {
    fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Option<Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
        }
    }
}

/// JSON response wrapper for tool results
#[derive(Serialize)]
struct McpResponse<T: Serialize> {
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

impl<T: Serialize> McpResponse<T> {
    fn success(data: T) -> Self {
        Self {
            status: "ok",
            data: Some(data),
            error_code: None,
            message: None,
        }
    }

    fn error(error_code: String, message: String) -> Self {
        Self {
            status: "error",
            data: None,
            error_code: Some(error_code),
            message: Some(message),
        }
    }
}

fn error_to_response(e: TTError) -> Value {
    let error_code = format!("{:?}", std::mem::discriminant(&e))
        .split("::")
        .last()
        .unwrap_or("Unknown")
        .to_string();

    json!(McpResponse::<()>::error(error_code, e.to_string()))
}

fn tt_result_to_json<T: Serialize>(result: Result<T, TTError>) -> Value {
    match result {
        Ok(data) => json!(McpResponse::success(data)),
        Err(e) => error_to_response(e),
    }
}

/// MCP Server
pub struct McpServer {
    core: AppCore,
}

impl McpServer {
    fn new() -> Result<Self, TTError> {
        let db_path = PathBuf::from(DB_FILE);
        if !db_path.exists() {
            return Err(TTError::NotInitialized);
        }
        let core = AppCore::open(db_path)?;
        Ok(Self { core })
    }

    fn handle_request(&mut self, request: JsonRpcRequest) -> JsonRpcResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request.id),
            "tools/list" => self.handle_list_tools(request.id),
            "tools/call" => self.handle_call_tool(request.id, request.params),
            _ => JsonRpcResponse::error(
                request.id,
                -32601,
                format!("Method not found: {}", request.method),
            ),
        }
    }

    fn handle_initialize(&self, id: Option<Value>) -> JsonRpcResponse {
        let result = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "tt",
                "version": "0.1.0"
            }
        });
        JsonRpcResponse::success(id, result)
    }

    fn handle_list_tools(&self, id: Option<Value>) -> JsonRpcResponse {
        let tools = json!([
            {
                "name": "get_next_task",
                "description": "Returns the next task to work on toward the current target. Call this after completing a task. If the response is TargetReached, stop working and report to the user.",
                "inputSchema": {"type": "object", "properties": {}, "required": []}
            },
            {
                "name": "get_current_task",
                "description": "Returns the currently active task and its artifacts. Use this to check what you're working on.",
                "inputSchema": {"type": "object", "properties": {}, "required": []}
            },
            {
                "name": "start_task",
                "description": "Start working on a specific task by ID. This marks the task as in_progress. Only one task can be active at a time.",
                "inputSchema": {"type": "object", "properties": {"id": {"type": "integer", "description": "Task ID to start"}}, "required": ["id"]}
            },
            {
                "name": "complete_task",
                "description": "Complete the currently active task. The task must have a Definition of Done (DoD) set. This marks the task as completed.",
                "inputSchema": {"type": "object", "properties": {}, "required": []}
            },
            {
                "name": "stop_task",
                "description": "Stop working on the currently active task. This moves the task back to pending status. Call this if you need to switch tasks.",
                "inputSchema": {"type": "object", "properties": {}, "required": []}
            },
            {
                "name": "create_task",
                "description": "Create a new task with a title. Optionally provide description, definition of done (dod), and positioning hints (after_id or before_id).",
                "inputSchema": {"type": "object", "properties": {"title": {"type": "string", "description": "Task title"}, "description": {"type": "string", "description": "Optional task description"}, "dod": {"type": "string", "description": "Optional Definition of Done"}, "after_id": {"type": "integer", "description": "Optional task ID to insert after"}, "before_id": {"type": "integer", "description": "Optional task ID to insert before"}}, "required": ["title"]}
            },
            {
                "name": "edit_task",
                "description": "Edit an existing task. Only provided fields are updated. Use this to add or update the Definition of Done (DoD).",
                "inputSchema": {"type": "object", "properties": {"id": {"type": "integer", "description": "Task ID to edit"}, "title": {"type": "string", "description": "New title"}, "description": {"type": "string", "description": "New description"}, "dod": {"type": "string", "description": "New Definition of Done"}}, "required": ["id"]}
            },
            {
                "name": "show_task",
                "description": "Show detailed information about a specific task including its dependencies and dependents.",
                "inputSchema": {"type": "object", "properties": {"id": {"type": "integer", "description": "Task ID to show"}}, "required": ["id"]}
            },
            {
                "name": "list_tasks",
                "description": "List all tasks in the target subgraph, sorted in topological order. Set all=true to show all tasks regardless of target.",
                "inputSchema": {"type": "object", "properties": {"all": {"type": "boolean", "description": "Show all tasks, not just target subgraph"}}, "required": []}
            },
            {
                "name": "add_dependency",
                "description": "Add a dependency between two tasks. The task 'task_id' will depend on 'depends_on' task. This enforces that 'depends_on' must be completed before 'task_id' can start. Fails if adding this dependency would create a cycle.",
                "inputSchema": {"type": "object", "properties": {"task_id": {"type": "integer", "description": "The dependent task ID"}, "depends_on": {"type": "integer", "description": "The prerequisite task ID"}}, "required": ["task_id", "depends_on"]}
            },
            {
                "name": "remove_dependency",
                "description": "Remove a dependency between two tasks.",
                "inputSchema": {"type": "object", "properties": {"task_id": {"type": "integer", "description": "The dependent task ID"}, "depends_on": {"type": "integer", "description": "The prerequisite task ID"}}, "required": ["task_id", "depends_on"]}
            },
            {
                "name": "block_task",
                "description": "Block a task. Blocked tasks cannot be started until they are unblocked. Use this when a task is waiting on external factors.",
                "inputSchema": {"type": "object", "properties": {"id": {"type": "integer", "description": "Task ID to block"}}, "required": ["id"]}
            },
            {
                "name": "unblock_task",
                "description": "Unblock a previously blocked task, moving it back to pending status.",
                "inputSchema": {"type": "object", "properties": {"id": {"type": "integer", "description": "Task ID to unblock"}}, "required": ["id"]}
            },
            {
                "name": "log_artifact",
                "description": "Record a file you have created as an artifact of the current task. Create the file first, then call this. Use descriptive names like 'research', 'plan', 'implementation-notes', 'test-report'.",
                "inputSchema": {"type": "object", "properties": {"name": {"type": "string", "description": "Artifact name (e.g., 'research', 'plan')"}, "file_path": {"type": "string", "description": "Path to the artifact file"}}, "required": ["name", "file_path"]}
            },
            {
                "name": "get_artifacts",
                "description": "Get all artifacts for a specific task. If task_id is not provided, returns artifacts for the current active task.",
                "inputSchema": {"type": "object", "properties": {"task_id": {"type": "integer", "description": "Optional task ID"}}, "required": []}
            },
            {
                "name": "set_target",
                "description": "Set the target task. All subsequent list and next operations will focus on the subgraph leading to this target.",
                "inputSchema": {"type": "object", "properties": {"id": {"type": "integer", "description": "Task ID to set as target"}}, "required": ["id"]}
            },
            {
                "name": "reorder_task",
                "description": "Reorder a task by specifying its position relative to other tasks. Use after_id to place after a specific task, before_id to place before, or both to insert between.",
                "inputSchema": {"type": "object", "properties": {"id": {"type": "integer", "description": "Task ID to reorder"}, "after_id": {"type": "integer", "description": "Optional task ID to place after"}, "before_id": {"type": "integer", "description": "Optional task ID to place before"}}, "required": ["id"]}
            }
        ]);
        JsonRpcResponse::success(id, tools)
    }

    fn handle_call_tool(&mut self, id: Option<Value>, params: Option<Value>) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => return JsonRpcResponse::error(id, -32602, "Missing params".to_string()),
        };

        let name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => return JsonRpcResponse::error(id, -32602, "Missing tool name".to_string()),
        };

        let args = params.get("arguments").cloned().unwrap_or(json!({}));

        let result = self.execute_tool(name, args);
        JsonRpcResponse::success(id, result)
    }

    fn execute_tool(&mut self, name: &str, args: Value) -> Value {
        match name {
            "get_next_task" => tt_result_to_json(self.core.next_task()),
            "get_current_task" => tt_result_to_json(self.core.get_active_task()),
            "start_task" => {
                let id = args.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                tt_result_to_json(self.core.start_task(id))
            }
            "complete_task" => tt_result_to_json(self.core.complete_task()),
            "stop_task" => tt_result_to_json(self.core.stop_task()),
            "create_task" => {
                let title = args
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let description = args
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let dod = args
                    .get("dod")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let after_id = args.get("after_id").and_then(|v| v.as_i64());
                let before_id = args.get("before_id").and_then(|v| v.as_i64());
                tt_result_to_json(self.core.add_task(
                    &title,
                    description.as_deref(),
                    dod.as_deref(),
                    after_id,
                    before_id,
                ))
            }
            "edit_task" => {
                let task_id = args.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                let title = args
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let description = args
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let dod = args
                    .get("dod")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                tt_result_to_json(self.core.edit_task(
                    task_id,
                    title.as_deref(),
                    description.as_deref(),
                    dod.as_deref(),
                ))
            }
            "show_task" => {
                let id = args.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                tt_result_to_json(self.core.get_task_detail(id))
            }
            "list_tasks" => {
                let all = args.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
                tt_result_to_json(self.core.list_tasks(all))
            }
            "add_dependency" => {
                let task_id = args.get("task_id").and_then(|v| v.as_i64()).unwrap_or(0);
                let depends_on = args.get("depends_on").and_then(|v| v.as_i64()).unwrap_or(0);
                tt_result_to_json(self.core.add_dependency(task_id, depends_on))
            }
            "remove_dependency" => {
                let task_id = args.get("task_id").and_then(|v| v.as_i64()).unwrap_or(0);
                let depends_on = args.get("depends_on").and_then(|v| v.as_i64()).unwrap_or(0);
                tt_result_to_json(self.core.remove_dependency(task_id, depends_on))
            }
            "block_task" => {
                let id = args.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                tt_result_to_json(self.core.block_task(id))
            }
            "unblock_task" => {
                let id = args.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                tt_result_to_json(self.core.unblock_task(id))
            }
            "log_artifact" => {
                let name = args
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let file_path = args
                    .get("file_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                tt_result_to_json(self.core.log_artifact(&name, &file_path))
            }
            "get_artifacts" => {
                let task_id = args.get("task_id").and_then(|v| v.as_i64());
                tt_result_to_json(self.core.get_artifacts(task_id))
            }
            "set_target" => {
                let id = args.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                tt_result_to_json(self.core.set_target(id))
            }
            "reorder_task" => {
                let id = args.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                let after_id = args.get("after_id").and_then(|v| v.as_i64());
                let before_id = args.get("before_id").and_then(|v| v.as_i64());
                tt_result_to_json(self.core.reorder_task(id, after_id, before_id))
            }
            _ => json!(McpResponse::<()>::error(
                "UnknownTool".to_string(),
                format!("Unknown tool: {name}")
            )),
        }
    }

    fn run(&mut self) -> io::Result<()> {
        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut stdout_lock = stdout.lock();

        for line in stdin.lock().lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<JsonRpcRequest>(&line) {
                Ok(request) => {
                    let response = self.handle_request(request);
                    let response_json = serde_json::to_string(&response)?;
                    writeln!(stdout_lock, "{response_json}")?;
                    stdout_lock.flush()?;
                }
                Err(e) => {
                    let response =
                        JsonRpcResponse::error(None, -32700, format!("Parse error: {e}"));
                    let response_json = serde_json::to_string(&response)?;
                    writeln!(stdout_lock, "{response_json}")?;
                    stdout_lock.flush()?;
                }
            }
        }

        Ok(())
    }
}

pub fn run_mcp_server() -> Result<(), TTError> {
    let mut server = McpServer::new()?;
    server.run().map_err(TTError::Io)
}

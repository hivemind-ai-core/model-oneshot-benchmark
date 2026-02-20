use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, Write};

use crate::core::CoreImpl;
use crate::db::Database;
use crate::error::{Error, Result};
use crate::models::McpResponse;

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Value,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    result: Option<Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

fn get_core() -> Result<CoreImpl> {
    let db = Database::new("tt.db")?;
    Ok(CoreImpl::new(db))
}

fn handle_tool(name: &str, params: Value) -> McpResponse {
    let core = match get_core() {
        Ok(c) => c,
        Err(e) => return McpResponse::error("DbError", e.to_string()),
    };

    match name {
        "get_next_task" => match core.next_task() {
            Ok((Some(task), _)) => McpResponse::ok(serde_json::json!({
                "task": task,
                "message": "Next task available"
            })),
            Ok((None, _)) => McpResponse::ok(serde_json::json!({
                "message": "Target reached",
                "target_reached": true
            })),
            Err(Error::AllBlocked(ids)) => {
                McpResponse::error("AllBlocked", format!("Blocked tasks: {ids}"))
            }
            Err(Error::NoTarget) => McpResponse::error("NoTarget", "No target set"),
            Err(e) => McpResponse::error("Error", e.to_string()),
        },

        "get_current_task" => match core.current_task() {
            Ok(task_with_deps) => McpResponse::ok(task_with_deps),
            Err(Error::NoActiveTask) => {
                McpResponse::error("NoActiveTask", "No task is currently in progress")
            }
            Err(e) => McpResponse::error("Error", e.to_string()),
        },

        "start_task" => {
            let id = params.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
            match core.start_task(id) {
                Ok(task) => McpResponse::ok(task),
                Err(Error::TaskNotFound(id)) => {
                    McpResponse::error("TaskNotFound", format!("Task #{id} not found"))
                }
                Err(Error::AnotherTaskActive(id, title)) => McpResponse::error(
                    "AnotherTaskActive",
                    format!("Task #{id} ({title}) is already in progress"),
                ),
                Err(Error::UnmetDependencies(id, deps)) => McpResponse::error(
                    "UnmetDependencies",
                    format!("Cannot start #{id}: dependencies not completed: {deps:?}"),
                ),
                Err(e) => McpResponse::error("Error", e.to_string()),
            }
        }

        "complete_task" => match core.complete_task() {
            Ok(task) => McpResponse::ok(task),
            Err(Error::NoActiveTask) => {
                McpResponse::error("NoActiveTask", "No task is currently in progress")
            }
            Err(Error::NoDod(id)) => {
                McpResponse::error("NoDod", format!("Task #{id} has no definition of done"))
            }
            Err(e) => McpResponse::error("Error", e.to_string()),
        },

        "stop_task" => match core.stop_task() {
            Ok(task) => McpResponse::ok(task),
            Err(Error::NoActiveTask) => {
                McpResponse::error("NoActiveTask", "No task is currently in progress")
            }
            Err(e) => McpResponse::error("Error", e.to_string()),
        },

        "create_task" => {
            let title = params.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let description = params.get("description").and_then(|v| v.as_str());
            let dod = params.get("dod").and_then(|v| v.as_str());
            let after_id = params.get("after_id").and_then(|v| v.as_i64());
            let before_id = params.get("before_id").and_then(|v| v.as_i64());

            match core.add_task(title, description, dod, after_id, before_id) {
                Ok(task) => McpResponse::ok(task),
                Err(e) => McpResponse::error("Error", e.to_string()),
            }
        }

        "edit_task" => {
            let id = params.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
            let title = params.get("title").and_then(|v| v.as_str());
            let description = params.get("description").and_then(|v| v.as_str());
            let dod = params.get("dod").and_then(|v| v.as_str());

            match core.edit_task(id, title, description, dod) {
                Ok(task) => McpResponse::ok(task),
                Err(Error::TaskNotFound(id)) => {
                    McpResponse::error("TaskNotFound", format!("Task #{id} not found"))
                }
                Err(e) => McpResponse::error("Error", e.to_string()),
            }
        }

        "add_dependency" => {
            let task_id = params.get("task_id").and_then(|v| v.as_i64()).unwrap_or(0);
            let depends_on = params
                .get("depends_on")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            match core.add_dependency(task_id, depends_on) {
                Ok(_) => McpResponse::ok(serde_json::json!({"message": "Dependency added"})),
                Err(Error::TaskNotFound(id)) => {
                    McpResponse::error("TaskNotFound", format!("Task #{id} not found"))
                }
                Err(Error::CycleDetected(_, _, cycle)) => {
                    McpResponse::error("CycleDetected", cycle)
                }
                Err(e) => McpResponse::error("Error", e.to_string()),
            }
        }

        "remove_dependency" => {
            let task_id = params.get("task_id").and_then(|v| v.as_i64()).unwrap_or(0);
            let depends_on = params
                .get("depends_on")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            match core.remove_dependency(task_id, depends_on) {
                Ok(_) => McpResponse::ok(serde_json::json!({"message": "Dependency removed"})),
                Err(e) => McpResponse::error("Error", e.to_string()),
            }
        }

        "block_task" => {
            let id = params.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
            match core.block_task(id) {
                Ok(task) => McpResponse::ok(task),
                Err(Error::TaskNotFound(id)) => {
                    McpResponse::error("TaskNotFound", format!("Task #{id} not found"))
                }
                Err(e) => McpResponse::error("Error", e.to_string()),
            }
        }

        "unblock_task" => {
            let id = params.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
            match core.unblock_task(id) {
                Ok(task) => McpResponse::ok(task),
                Err(Error::TaskNotFound(id)) => {
                    McpResponse::error("TaskNotFound", format!("Task #{id} not found"))
                }
                Err(Error::TaskNotBlocked(id)) => {
                    McpResponse::error("TaskNotBlocked", format!("Task #{id} is not blocked"))
                }
                Err(e) => McpResponse::error("Error", e.to_string()),
            }
        }

        "list_tasks" => {
            let all = params.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
            match core.list_tasks(all) {
                Ok((tasks, target_id, warnings)) => McpResponse::ok(serde_json::json!({
                    "tasks": tasks,
                    "target_id": target_id,
                    "warnings": warnings
                })),
                Err(Error::NoTarget) => McpResponse::error("NoTarget", "No target set"),
                Err(e) => McpResponse::error("Error", e.to_string()),
            }
        }

        "show_task" => {
            let id = params.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
            match core.show_task(id) {
                Ok(task) => McpResponse::ok(task),
                Err(Error::TaskNotFound(id)) => {
                    McpResponse::error("TaskNotFound", format!("Task #{id} not found"))
                }
                Err(e) => McpResponse::error("Error", e.to_string()),
            }
        }

        "log_artifact" => {
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let file_path = params
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            match core.log_artifact(name, file_path) {
                Ok(artifact) => McpResponse::ok(artifact),
                Err(Error::NoActiveTask) => {
                    McpResponse::error("NoActiveTask", "No task is currently in progress")
                }
                Err(e) => McpResponse::error("Error", e.to_string()),
            }
        }

        "get_artifacts" => {
            let task_id = params.get("task_id").and_then(|v| v.as_i64());
            match core.get_artifacts(task_id) {
                Ok(artifacts) => McpResponse::ok(artifacts),
                Err(e) => McpResponse::error("Error", e.to_string()),
            }
        }

        "set_target" => {
            let id = params.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
            match core.set_target(id) {
                Ok(_) => McpResponse::ok(serde_json::json!({"message": "Target set"})),
                Err(Error::TaskNotFound(id)) => {
                    McpResponse::error("TaskNotFound", format!("Task #{id} not found"))
                }
                Err(e) => McpResponse::error("Error", e.to_string()),
            }
        }

        "reorder_task" => {
            let id = params.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
            let after_id = params.get("after_id").and_then(|v| v.as_i64());
            let before_id = params.get("before_id").and_then(|v| v.as_i64());

            match core.reorder_task(id, after_id, before_id) {
                Ok(order) => McpResponse::ok(serde_json::json!({"manual_order": order})),
                Err(e) => McpResponse::error("Error", e.to_string()),
            }
        }

        _ => McpResponse::error("UnknownTool", format!("Unknown tool: {name}")),
    }
}

fn handle_initialize() -> Value {
    serde_json::json!({
        "protocolVersion": "2024-11-05",
        "serverInfo": {
            "name": "tt",
            "version": "0.1.0"
        },
        "capabilities": {
            "tools": {}
        }
    })
}

fn handle_tools_list() -> Value {
    let tools = vec![
        serde_json::json!({
            "name": "get_next_task",
            "description": "Returns the next task to work on toward the current target. Call this after completing a task. If the response is TargetReached, stop working and report to the user.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        serde_json::json!({
            "name": "get_current_task",
            "description": "Returns the currently active task with its artifacts.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        serde_json::json!({
            "name": "start_task",
            "description": "Starts a task by moving it to in_progress status.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {"type": "integer", "description": "Task ID to start"}
                },
                "required": ["id"]
            }
        }),
        serde_json::json!({
            "name": "complete_task",
            "description": "Marks the active task as completed. Requires a DoD to be set.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        serde_json::json!({
            "name": "stop_task",
            "description": "Stops the active task, moving it back to pending.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        serde_json::json!({
            "name": "create_task",
            "description": "Creates a new task. If you discover during implementation that a task needs to be broken into smaller pieces, create subtasks and add dependencies.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title": {"type": "string", "description": "Task title"},
                    "description": {"type": "string", "description": "Optional description"},
                    "dod": {"type": "string", "description": "Definition of Done"},
                    "after_id": {"type": "integer", "description": "Insert after task ID"},
                    "before_id": {"type": "integer", "description": "Insert before task ID"}
                },
                "required": ["title"]
            }
        }),
        serde_json::json!({
            "name": "edit_task",
            "description": "Updates task fields.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {"type": "integer", "description": "Task ID"},
                    "title": {"type": "string", "description": "New title"},
                    "description": {"type": "string", "description": "New description"},
                    "dod": {"type": "string", "description": "New Definition of Done"}
                },
                "required": ["id"]
            }
        }),
        serde_json::json!({
            "name": "add_dependency",
            "description": "Adds a dependency: task_id depends on depends_on.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "integer", "description": "The dependent task"},
                    "depends_on": {"type": "integer", "description": "The prerequisite task"}
                },
                "required": ["task_id", "depends_on"]
            }
        }),
        serde_json::json!({
            "name": "remove_dependency",
            "description": "Removes a dependency.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "integer", "description": "The dependent task"},
                    "depends_on": {"type": "integer", "description": "The prerequisite task"}
                },
                "required": ["task_id", "depends_on"]
            }
        }),
        serde_json::json!({
            "name": "block_task",
            "description": "Blocks a task, moving it to blocked status.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {"type": "integer", "description": "Task ID to block"}
                },
                "required": ["id"]
            }
        }),
        serde_json::json!({
            "name": "unblock_task",
            "description": "Unblocks a blocked task, moving it to pending.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {"type": "integer", "description": "Task ID to unblock"}
                },
                "required": ["id"]
            }
        }),
        serde_json::json!({
            "name": "list_tasks",
            "description": "Lists tasks in topological order.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "all": {"type": "boolean", "description": "Show all tasks, not just target subgraph"}
                }
            }
        }),
        serde_json::json!({
            "name": "show_task",
            "description": "Shows full details of a task.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {"type": "integer", "description": "Task ID"}
                },
                "required": ["id"]
            }
        }),
        serde_json::json!({
            "name": "log_artifact",
            "description": "Records a file you have created as an artifact of the current task. Create the file first, then call this. Use descriptive names like 'research', 'plan', 'implementation-notes', 'test-report'.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {"type": "string", "description": "Artifact name"},
                    "file_path": {"type": "string", "description": "Path to the file"}
                },
                "required": ["name", "file_path"]
            }
        }),
        serde_json::json!({
            "name": "get_artifacts",
            "description": "Gets artifacts for a task or the current task.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "task_id": {"type": "integer", "description": "Task ID (optional, defaults to current)"}
                }
            }
        }),
        serde_json::json!({
            "name": "set_target",
            "description": "Sets the target task for the current session.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {"type": "integer", "description": "Target task ID"}
                },
                "required": ["id"]
            }
        }),
        serde_json::json!({
            "name": "reorder_task",
            "description": "Reorders a task by moving it before or after another task.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {"type": "integer", "description": "Task ID to reorder"},
                    "after_id": {"type": "integer", "description": "Move after this task"},
                    "before_id": {"type": "integer", "description": "Move before this task"}
                },
                "required": ["id"]
            }
        }),
    ];

    serde_json::json!({
        "tools": tools
    })
}

pub fn run_mcp_server() -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        let mut input = String::new();
        if stdin.read_line(&mut input).unwrap() == 0 {
            break;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&input) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let response = if request.method == "initialize" {
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(handle_initialize()),
                error: None,
            }
        } else if request.method == "tools/list" {
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(handle_tools_list()),
                error: None,
            }
        } else if request.method == "tools/call" {
            let params = request.params.unwrap_or(serde_json::Value::Null);
            let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let tool_args = params
                .get("arguments")
                .cloned()
                .unwrap_or(serde_json::Value::Null);

            let result = handle_tool(tool_name, tool_args);

            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(serde_json::to_value(result).unwrap_or(serde_json::Value::Null)),
                error: None,
            }
        } else if request.method == "notifications/initialized" {
            continue;
        } else {
            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: format!("Method not found: {}", request.method),
                }),
            }
        };

        let output = serde_json::to_string(&response).unwrap();
        writeln!(stdout, "{output}").unwrap();
        stdout.flush().unwrap();
    }

    Ok(())
}

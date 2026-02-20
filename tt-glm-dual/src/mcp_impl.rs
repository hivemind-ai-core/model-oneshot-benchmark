//! MCP server for the tt task tracker.
//!
//! Provides a stdio-based MCP server exposing all tt operations as tools.

use crate::core::error::Result;
use crate::core::task::TaskManager;
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// Run the MCP server over stdio.
pub fn run_mcp() -> Result<()> {
    eprintln!("tt MCP server starting...");

    // For now, we'll use a simple manual JSON-RPC implementation
    // since rmcp API is still evolving
    run_simple_mcp()
}

/// Simple MCP server using manual JSON-RPC over stdio.
fn run_simple_mcp() -> Result<()> {
    use std::io::{self, BufRead, Write};

    let db_path = std::path::PathBuf::from("tt.db");
    if !db_path.exists() {
        return Err(crate::core::error::TTError::Io(std::io::Error::new(
            io::ErrorKind::NotFound,
            "tt.db not found",
        )));
    }

    let db = crate::core::db::Db::open(&db_path)?;
    let mgr = Arc::new(Mutex::new(TaskManager::new(db)));

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    // MCP protocol: read JSON-RPC messages line by line
    let mut buffer = String::new();

    loop {
        buffer.clear();
        match reader.read_line(&mut buffer) {
            Ok(0) => break, // EOF
            Ok(_) => {
                let response = match handle_mcp_request(buffer.trim(), mgr.clone()) {
                    Ok(resp) => resp,
                    Err(e) => format_mcp_error(&e.to_string(), e.error_code()),
                };

                if let Err(e) = writeln!(writer, "{}", response) {
                    eprintln!("Error writing response: {}", e);
                    break;
                }
                let _ = writer.flush();
            }
            Err(e) => {
                eprintln!("Error reading request: {}", e);
                break;
            }
        }
    }

    Ok(())
}

/// Handle an MCP request.
fn handle_mcp_request(request: &str, mgr: Arc<Mutex<TaskManager>>) -> Result<String> {
    let json: Value = serde_json::from_str(request)
        .map_err(|e| crate::core::error::TTError::Mcp(format!("Invalid JSON: {}", e)))?;

    let method = json
        .get("method")
        .and_then(|m| m.as_str())
        .ok_or_else(|| crate::core::error::TTError::Mcp("Missing method".to_string()))?;

    let params = json.get("params").and_then(|p| p.as_object());
    let id = json.get("id").cloned().unwrap_or(Value::Null);

    let (status, data) = match handle_method(method, params, mgr) {
        Ok(result) => ("ok", result),
        Err(e) => {
            return Ok(format_mcp_error_response(
                &e.to_string(),
                e.error_code(),
                &id,
            ))
        }
    };

    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "status": status,
            "data": data
        }
    });

    Ok(serde_json::to_string(&response)?)
}

/// Handle a specific method call.
fn handle_method(
    method: &str,
    params: Option<&serde_json::Map<String, Value>>,
    mgr: Arc<Mutex<TaskManager>>,
) -> Result<Value> {
    match method {
        "get_next_task" => {
            let mgr = mgr.lock().unwrap();
            match mgr.next_task() {
                Ok(task) => Ok(serde_json::to_value(task_with_deps_to_json(&task))?),
                Err(e) => match &e {
                    crate::core::error::TTError::TargetReached(id) => {
                        Ok(serde_json::json!({"type": "target_reached", "id": id}))
                    }
                    crate::core::error::TTError::AllBlocked(ids) => {
                        Ok(serde_json::json!({"type": "all_blocked", "ids": ids}))
                    }
                    _ => Err(e),
                },
            }
        }

        "get_current_task" => {
            let mgr = mgr.lock().unwrap();
            let task = mgr.get_current_task()?;
            Ok(serde_json::to_value(task_with_deps_to_json(&task))?)
        }

        "start_task" => {
            let id = get_param(params, "id")?;
            let mut mgr = mgr.lock().unwrap();
            let task = mgr.start_task(id)?;
            Ok(serde_json::to_value(task_to_json(&task))?)
        }

        "complete_task" => {
            let mut mgr = mgr.lock().unwrap();
            let task = mgr.complete_task()?;
            Ok(serde_json::to_value(task_to_json(&task))?)
        }

        "stop_task" => {
            let mut mgr = mgr.lock().unwrap();
            let task = mgr.stop_task()?;
            Ok(serde_json::to_value(task_to_json(&task))?)
        }

        "create_task" => {
            let title = get_param_str(params, "title")?;
            let description = params
                .and_then(|p| p.get("description"))
                .and_then(|v| v.as_str());
            let dod = params.and_then(|p| p.get("dod")).and_then(|v| v.as_str());
            let after_id = params
                .and_then(|p| p.get("after_id"))
                .and_then(|v| v.as_i64());
            let before_id = params
                .and_then(|p| p.get("before_id"))
                .and_then(|v| v.as_i64());

            let mut mgr = mgr.lock().unwrap();
            let id = mgr.add_task(&title, description, dod, after_id, before_id)?;
            let task = mgr.show_task(id)?;
            Ok(serde_json::to_value(task_with_deps_to_json(&task))?)
        }

        "edit_task" => {
            let id = get_param(params, "id")?;
            let title = params.and_then(|p| p.get("title")).and_then(|v| v.as_str());
            let description = params
                .and_then(|p| p.get("description"))
                .and_then(|v| v.as_str());
            let dod = params.and_then(|p| p.get("dod")).and_then(|v| v.as_str());

            let mut mgr = mgr.lock().unwrap();
            mgr.edit_task(id, title, description, dod)?;
            let task = mgr.show_task(id)?;
            Ok(serde_json::to_value(task_with_deps_to_json(&task))?)
        }

        "add_dependency" => {
            let task_id = get_param(params, "task_id")?;
            let depends_on = get_param(params, "depends_on")?;
            let mut mgr = mgr.lock().unwrap();
            mgr.add_dependency(task_id, depends_on)?;
            Ok(serde_json::json!({"success": true}))
        }

        "remove_dependency" => {
            let task_id = get_param(params, "task_id")?;
            let depends_on = get_param(params, "depends_on")?;
            let mut mgr = mgr.lock().unwrap();
            mgr.remove_dependency(task_id, depends_on)?;
            Ok(serde_json::json!({"success": true}))
        }

        "block_task" => {
            let id = get_param(params, "id")?;
            let mut mgr = mgr.lock().unwrap();
            let task = mgr.block_task(id)?;
            Ok(serde_json::to_value(task_to_json(&task))?)
        }

        "unblock_task" => {
            let id = get_param(params, "id")?;
            let mut mgr = mgr.lock().unwrap();
            let task = mgr.unblock_task(id)?;
            Ok(serde_json::to_value(task_to_json(&task))?)
        }

        "list_tasks" => {
            let all = params
                .and_then(|p| p.get("all"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let mgr = mgr.lock().unwrap();
            let tasks = mgr.list_tasks(all)?;
            let json: Vec<Value> = tasks.iter().map(task_with_deps_to_json).collect();
            Ok(serde_json::to_value(json)?)
        }

        "show_task" => {
            let id = get_param(params, "id")?;
            let mgr = mgr.lock().unwrap();
            let task = mgr.show_task(id)?;
            Ok(serde_json::to_value(task_with_deps_to_json(&task))?)
        }

        "log_artifact" => {
            let name = get_param_str(params, "name")?;
            let file_path = get_param_str(params, "file_path")?;
            let mut mgr = mgr.lock().unwrap();
            let artifact = mgr.log_artifact(&name, &file_path)?;
            Ok(serde_json::to_value(artifact_to_json(&artifact))?)
        }

        "get_artifacts" => {
            let task_id = params
                .and_then(|p| p.get("task_id"))
                .and_then(|v| v.as_i64());
            let mgr = mgr.lock().unwrap();
            let artifacts = mgr.get_artifacts(task_id)?;
            let json: Vec<Value> = artifacts.iter().map(artifact_to_json).collect();
            Ok(serde_json::to_value(json)?)
        }

        "set_target" => {
            let id = get_param(params, "id")?;
            let mut mgr = mgr.lock().unwrap();
            mgr.set_target(id)?;
            Ok(serde_json::json!({"success": true, "target_id": id}))
        }

        "reorder_task" => {
            let id = get_param(params, "id")?;
            let after_id = params
                .and_then(|p| p.get("after_id"))
                .and_then(|v| v.as_i64());
            let before_id = params
                .and_then(|p| p.get("before_id"))
                .and_then(|v| v.as_i64());
            let mut mgr = mgr.lock().unwrap();
            mgr.reorder_task(id, after_id, before_id)?;
            Ok(serde_json::json!({"success": true}))
        }

        _ => Err(crate::core::error::TTError::Mcp(format!(
            "Unknown method: {}",
            method
        ))),
    }
}

/// Get a parameter as i64.
fn get_param(params: Option<&serde_json::Map<String, Value>>, key: &str) -> Result<i64> {
    params
        .and_then(|p| p.get(key))
        .and_then(|v| v.as_i64())
        .ok_or_else(|| {
            crate::core::error::TTError::Mcp(format!("Missing or invalid parameter: {}", key))
        })
}

/// Get a parameter as String.
fn get_param_str(params: Option<&serde_json::Map<String, Value>>, key: &str) -> Result<String> {
    params
        .and_then(|p| p.get(key))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            crate::core::error::TTError::Mcp(format!("Missing or invalid parameter: {}", key))
        })
}

/// Convert TaskWithDeps to JSON.
fn task_with_deps_to_json(t: &crate::core::db::TaskWithDeps) -> Value {
    serde_json::json!({
        "id": t.task.id,
        "title": t.task.title,
        "description": t.task.description,
        "dod": t.task.dod,
        "status": t.task.status,
        "manual_order": t.task.manual_order,
        "created_at": t.task.created_at,
        "started_at": t.task.started_at,
        "completed_at": t.task.completed_at,
        "last_touched_at": t.task.last_touched_at,
        "dependencies": t.dependencies,
        "dependents": t.dependents,
        "artifacts": t.artifacts.iter().map(artifact_to_json).collect::<Vec<_>>(),
    })
}

/// Convert Task to JSON.
fn task_to_json(t: &crate::core::db::Task) -> Value {
    serde_json::json!({
        "id": t.id,
        "title": t.title,
        "description": t.description,
        "dod": t.dod,
        "status": t.status,
        "manual_order": t.manual_order,
        "created_at": t.created_at,
        "started_at": t.started_at,
        "completed_at": t.completed_at,
        "last_touched_at": t.last_touched_at,
    })
}

/// Convert Artifact to JSON.
fn artifact_to_json(a: &crate::core::db::Artifact) -> Value {
    serde_json::json!({
        "id": a.id,
        "task_id": a.task_id,
        "name": a.name,
        "file_path": a.file_path,
        "created_at": a.created_at,
    })
}

/// Format an MCP error response.
fn format_mcp_error_response(message: &str, code: &str, id: &Value) -> String {
    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    });

    serde_json::to_string(&response).unwrap_or_else(|_| {
        format!(
            r#"{{"jsonrpc":"2.0","id":{},"error":{{"code":"{}","message":"{}"}}}}"#,
            id, code, message
        )
    })
}

/// Format an MCP error (without request id).
fn format_mcp_error(message: &str, code: &str) -> String {
    format_mcp_error_response(message, code, &Value::Null)
}

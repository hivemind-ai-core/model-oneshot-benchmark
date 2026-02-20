//! Output formatting for the CLI.

use crate::core::db::{Artifact, Db, Task, TaskWithDeps};
use crate::core::error::TTError;

/// Format a single task for display.
fn format_task_id(id: i64) -> String {
    format!("{id}")
}

/// Format status indicator.
fn format_status(status: &str) -> &'static str {
    match status {
        "completed" => "✓",
        "in_progress" => "●",
        "pending" => "○",
        "blocked" => "✗",
        _ => "?",
    }
}

/// Format the list output.
pub fn format_list(tasks: &[TaskWithDeps], target_id: i64) {
    let target_task = match tasks.iter().find(|t| t.task.id == target_id) {
        Some(t) => t,
        None => {
            println!("No tasks in target subgraph");
            return;
        }
    };

    println!("Target: {}", format_task_with_status(&target_task.task));
    println!();

    for task_with_deps in tasks {
        let task = &task_with_deps.task;
        let deps_str = if !task_with_deps.dependencies.is_empty() {
            let parts: Vec<String> = task_with_deps
                .dependencies
                .iter()
                .map(|&id| match tasks.iter().find(|t| t.task.id == id) {
                    Some(t) => format!("{id} {}", format_status(&t.task.status)),
                    None => format!("{id} ?"),
                })
                .collect();
            format!("(deps: {})", parts.join(", "))
        } else {
            String::new()
        };

        println!(
            "  [{}] {} {} {deps_str:<30}",
            format_task_id(task.id),
            format_status(&task.status),
            task.title,
        );
    }

    println!();
    println!("Legend: ✓ completed  ● in_progress  ○ pending  ✗ blocked");
}

/// Format the list all output.
pub fn format_list_all(tasks: &[TaskWithDeps]) {
    for task_with_deps in tasks {
        let task = &task_with_deps.task;
        let deps_str = if !task_with_deps.dependencies.is_empty() {
            let parts: Vec<String> = task_with_deps
                .dependencies
                .iter()
                .map(|&id| format!("{id}"))
                .collect();
            format!("(deps: {})", parts.join(", "))
        } else {
            String::new()
        };

        println!(
            "[{}] {} {} {deps_str:<30}",
            format_task_id(task.id),
            format_status(&task.status),
            task.title,
        );
    }

    println!();
    println!("Legend: ✓ completed  ● in_progress  ○ pending  ✗ blocked");
}

/// Format task with status.
fn format_task_with_status(task: &Task) -> String {
    format!(
        "#{} ({}{})",
        task.id,
        format_status(&task.status),
        task.status
    )
}

/// Format show output.
pub fn format_show(task_with_deps: &TaskWithDeps) {
    let task = &task_with_deps.task;

    println!("[{}] {}", format_task_id(task.id), task.title);
    println!("Status:       {}", task.status);
    println!("Order:        {}", task.manual_order);
    println!("Created:      {}", format_timestamp(&task.created_at));

    if let Some(ref started) = task.started_at {
        println!("Started:      {}", format_timestamp(started));
    }

    if let Some(ref completed) = task.completed_at {
        println!("Completed:    {}", format_timestamp(completed));
    }

    if let Some(ref desc) = task.description {
        println!();
        println!("Description:");
        println!("  {}", desc);
    }

    if let Some(ref dod) = task.dod {
        println!();
        println!("DoD:          {}", dod);
    }

    println!();
    println!(
        "Dependencies: {}",
        if task_with_deps.dependencies.is_empty() {
            "(none)".to_string()
        } else {
            let parts: Vec<String> = task_with_deps
                .dependencies
                .iter()
                .map(
                    |&id| match task_with_deps.dependencies.iter().find(|&&x| x == id) {
                        Some(_) => format!("#{}", id),
                        None => format!("#{}", id),
                    },
                )
                .collect();
            parts.join(", ")
        }
    );

    println!(
        "Dependents:   {}",
        if task_with_deps.dependents.is_empty() {
            "(none)".to_string()
        } else {
            let parts: Vec<String> = task_with_deps
                .dependents
                .iter()
                .map(|&id| format!("#{}", id))
                .collect();
            parts.join(", ")
        }
    );

    println!(
        "Artifacts:    {}",
        if task_with_deps.artifacts.is_empty() {
            "(none)".to_string()
        } else {
            let parts: Vec<String> = task_with_deps
                .artifacts
                .iter()
                .map(|a| format!("{}: {}", a.name, a.file_path))
                .collect();
            parts.join(", ")
        }
    );
}

/// Format next output.
pub fn format_next(task_with_deps: &TaskWithDeps, db: &Db) -> std::result::Result<(), TTError> {
    let task = &task_with_deps.task;

    println!("Next: [{}] {}", format_task_id(task.id), task.title);

    // Check dependency status
    if !task_with_deps.dependencies.is_empty() {
        let all_completed =
            task_with_deps
                .dependencies
                .iter()
                .all(|&dep_id| match db.get_task(dep_id) {
                    Ok(dep_task) => dep_task.status == "completed",
                    Err(_) => false,
                });

        if all_completed {
            let dep_ids: Vec<String> = task_with_deps
                .dependencies
                .iter()
                .map(|&id| format!("#{} ✓", id))
                .collect();
            println!("  Dependencies: {} (all met)", dep_ids.join(", "));
        } else {
            let dep_parts: Vec<String> = task_with_deps
                .dependencies
                .iter()
                .map(|&dep_id| match db.get_task(dep_id) {
                    Ok(dep_task) => {
                        format!("#{} {}", dep_id, format_status(&dep_task.status))
                    }
                    Err(_) => format!("#{} ?", dep_id),
                })
                .collect();
            println!("  Dependencies: {}", dep_parts.join(", "));
        }
    }

    if let Some(ref dod) = task.dod {
        println!("  DoD:       {}", dod);
    }

    Ok(())
}

/// Format next error output.
pub fn format_next_error(err: TTError) {
    match &err {
        TTError::TargetReached(id) => {
            println!("Target Reached: all tasks for #{} are completed.", id);
        }
        TTError::AllBlocked(ids) => {
            println!("All remaining tasks are blocked:");
            for &id in ids {
                println!("  [{}] ✗ Blocked", id);
            }
        }
        TTError::NoTarget => {
            eprintln!("Error: No target set. Use `tt target <id>` first.");
        }
        _ => {
            eprintln!("Error: {}", err);
        }
    }
}

/// Format current output.
pub fn format_current(task_with_deps: &TaskWithDeps) {
    let task = &task_with_deps.task;

    println!("Active: [{}] {}", format_task_id(task.id), task.title);
    println!("  Status:    {}", task.status);

    if let Some(ref started) = task.started_at {
        println!("  Started:   {}", format_timestamp(started));
    }

    if let Some(ref dod) = task.dod {
        println!("  DoD:       {}", dod);
    }

    if !task_with_deps.artifacts.is_empty() {
        println!("  Artifacts:");
        for artifact in &task_with_deps.artifacts {
            println!("    - {}: {}", artifact.name, artifact.file_path);
        }
    } else {
        println!("  Artifacts: (none)");
    }
}

/// Format artifacts output.
pub fn format_artifacts(artifacts: &[Artifact]) {
    if artifacts.is_empty() {
        println!("No artifacts");
        return;
    }

    for artifact in artifacts {
        println!(
            "  [{}] {}: {}",
            artifact.id, artifact.name, artifact.file_path
        );
    }
}

/// Format timestamp for display.
fn format_timestamp(ts: &str) -> String {
    // Parse ISO 8601 and format nicely
    match ts.parse::<chrono::DateTime<chrono::Utc>>() {
        Ok(dt) => dt.format("%Y-%m-%d %H:%M").to_string(),
        Err(_) => ts.to_string(),
    }
}

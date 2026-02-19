use crate::core::TaskTracker;
use crate::error::TaskError;
use crate::models::{NextTaskResult, Status};
use std::fs;
use std::path::Path;

/// Handle the init command
pub fn handle_init() -> Result<(), TaskError> {
    // Check if already initialized
    if Path::new("tt.db").exists() {
        return Err(TaskError::AlreadyInitialized);
    }

    let tracker = TaskTracker::open()?;
    tracker.init()?;

    // Create .tt/artifacts directory
    fs::create_dir_all(".tt/artifacts")?;

    println!("Initialized task tracker in current directory");
    println!("  - Created: tt.db");
    println!("  - Created: .tt/artifacts/");

    Ok(())
}

/// Handle the add command
pub fn handle_add(
    title: &str,
    desc: Option<&str>,
    dod: Option<&str>,
    after: Option<i64>,
    before: Option<i64>,
) -> Result<(), TaskError> {
    let tracker = TaskTracker::open()?;
    check_initialized(&tracker)?;

    let task = tracker.create_task(title, desc, dod, after, before)?;

    println!("Created task #{}: {}", task.id, task.title);
    println!("  Order: {}", task.manual_order);

    Ok(())
}

/// Handle the edit command
pub fn handle_edit(
    id: i64,
    title: Option<&str>,
    desc: Option<&str>,
    no_desc: bool,
    dod: Option<&str>,
    no_dod: bool,
) -> Result<(), TaskError> {
    let tracker = TaskTracker::open()?;
    check_initialized(&tracker)?;

    let description = if no_desc { Some(None) } else { desc.map(Some) };

    let dod_value = if no_dod { Some(None) } else { dod.map(Some) };

    let detail = tracker.update_task(id, title, description, dod_value)?;

    println!("Updated task #{}: {}", detail.task.id, detail.task.title);

    Ok(())
}

/// Handle the show command
pub fn handle_show(id: i64) -> Result<(), TaskError> {
    let tracker = TaskTracker::open()?;
    check_initialized(&tracker)?;

    let detail = tracker.get_task(id)?;
    let task = &detail.task;

    println!("[#{id}] {title}", id = task.id, title = task.title);
    println!("Status:       {}", task.status);
    println!("Order:        {}", task.manual_order);
    println!("Created:      {}", task.created_at.format("%Y-%m-%d %H:%M"));

    if let Some(ref dod) = task.dod {
        println!("DoD:          {dod}");
    } else {
        println!("DoD:          (none)");
    }

    if let Some(ref desc) = task.description {
        println!("Description:  {desc}");
    }

    if !detail.dependencies.is_empty() {
        let deps_str = detail
            .dependencies
            .iter()
            .map(|d| format!("#{} ({})", d.id, d.status.icon()))
            .collect::<Vec<_>>()
            .join(", ");
        println!("Dependencies: {deps_str}");
    }

    if !detail.dependents.is_empty() {
        let deps_str = detail
            .dependents
            .iter()
            .map(|id| format!("#{id}"))
            .collect::<Vec<_>>()
            .join(", ");
        println!("Dependents:   {deps_str}");
    }

    if detail.artifacts.is_empty() {
        println!("Artifacts:    (none)");
    } else {
        println!("Artifacts:");
        for artifact in &detail.artifacts {
            println!("  - {}: {}", artifact.name, artifact.file_path);
        }
    }

    Ok(())
}

/// Handle the list command
pub fn handle_list(all: bool) -> Result<(), TaskError> {
    let tracker = TaskTracker::open()?;
    check_initialized(&tracker)?;

    // Show target info
    if !all {
        match tracker.get_target()? {
            Some(target_id) => {
                let target = tracker.get_task(target_id)?;
                println!("Target: #{} ({})", target.task.id, target.task.title);
            }
            None => {
                return Err(TaskError::NoTarget);
            }
        }
    }

    let (tasks, conflicts) = tracker.list_tasks(all)?;

    if tasks.is_empty() {
        println!("No tasks found.");
        return Ok(());
    }

    // Build a map of task statuses for dependency display
    let status_map: std::collections::HashMap<i64, Status> =
        tasks.iter().map(|d| (d.task.id, d.task.status)).collect();

    for detail in &tasks {
        let task = &detail.task;
        let icon = task.status.icon();

        let dep_info = if detail.dependencies.is_empty() {
            String::new()
        } else {
            let deps = detail
                .dependencies
                .iter()
                .map(|d| {
                    let status = status_map.get(&d.id).copied().unwrap_or(Status::Completed);
                    format!("#{} {}", d.id, status.icon())
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(" (deps: {deps})")
        };

        println!("  [#{:>3}] {} {}{}", task.id, icon, task.title, dep_info);
    }

    // Print legend
    println!();
    println!(
        "Legend: {} completed  {} in_progress  {} pending  {} blocked",
        Status::Completed.icon(),
        Status::InProgress.icon(),
        Status::Pending.icon(),
        Status::Blocked.icon()
    );

    // Print order conflicts as warnings
    for conflict in conflicts {
        eprintln!(
            "Warning: #{} (order {:.1}) depends on #{} (order {:.1}) which has higher manual_order",
            conflict.task_id, conflict.task_order, conflict.dep_id, conflict.dep_order
        );
    }

    Ok(())
}

/// Handle the target command
pub fn handle_target(id: Option<i64>) -> Result<(), TaskError> {
    let tracker = TaskTracker::open()?;
    check_initialized(&tracker)?;

    if let Some(target_id) = id {
        tracker.set_target(target_id)?;
        let target = tracker.get_task(target_id)?;
        println!("Set target to #{}: {}", target.task.id, target.task.title);
    } else {
        match tracker.get_target()? {
            Some(target_id) => {
                let target = tracker.get_task(target_id)?;
                println!(
                    "Current target: #{} ({})",
                    target.task.id, target.task.title
                );
            }
            None => {
                println!("No target set.");
            }
        }
    }

    Ok(())
}

/// Handle the next command
pub fn handle_next() -> Result<(), TaskError> {
    let tracker = TaskTracker::open()?;
    check_initialized(&tracker)?;

    match tracker.get_next_task(false)? {
        NextTaskResult::Task { task } => {
            println!("Next: [#{}] {}", task.task.id, task.task.title);

            if !task.dependencies.is_empty() {
                let all_met = task
                    .dependencies
                    .iter()
                    .all(|d| d.status == Status::Completed);
                let deps_str = task
                    .dependencies
                    .iter()
                    .map(|d| format!("#{} {}", d.id, d.status.icon()))
                    .collect::<Vec<_>>()
                    .join(", ");

                if all_met {
                    println!("  Dependencies: {deps_str} (all met)");
                } else {
                    println!("  Dependencies: {deps_str}");
                }
            }

            if let Some(ref dod) = task.task.dod {
                println!("  DoD: {dod}");
            }
        }
        NextTaskResult::TargetReached { target_id } => {
            let target = tracker.get_task(target_id)?;
            println!(
                "Target Reached: all tasks for #{} ({}) are completed.",
                target_id, target.task.title
            );
        }
        NextTaskResult::AllBlocked { tasks } => {
            println!("All remaining tasks are blocked:");
            for task in tasks {
                let waiting = task
                    .waiting_on
                    .iter()
                    .map(|w| format!("#{} ({})", w.id, w.status.icon()))
                    .collect::<Vec<_>>()
                    .join(", ");
                println!(
                    "  [#{}] {} {} — waiting on: {}",
                    task.id,
                    Status::Blocked.icon(),
                    task.title,
                    waiting
                );
            }
        }
    }

    Ok(())
}

/// Handle the start command
pub fn handle_start(id: i64) -> Result<(), TaskError> {
    let tracker = TaskTracker::open()?;
    check_initialized(&tracker)?;

    let detail = tracker.start_task(id)?;
    println!("Started task #{}: {}", detail.task.id, detail.task.title);

    Ok(())
}

/// Handle the stop command
pub fn handle_stop() -> Result<(), TaskError> {
    let tracker = TaskTracker::open()?;
    check_initialized(&tracker)?;

    let detail = tracker.stop_task()?;
    println!("Stopped task #{}: {}", detail.task.id, detail.task.title);

    Ok(())
}

/// Handle the done command
pub fn handle_done() -> Result<(), TaskError> {
    let tracker = TaskTracker::open()?;
    check_initialized(&tracker)?;

    let detail = tracker.complete_task()?;
    println!("Completed task #{}: {}", detail.task.id, detail.task.title);

    Ok(())
}

/// Handle the block command
pub fn handle_block(id: i64) -> Result<(), TaskError> {
    let tracker = TaskTracker::open()?;
    check_initialized(&tracker)?;

    let detail = tracker.block_task(id)?;
    println!("Blocked task #{}: {}", detail.task.id, detail.task.title);

    Ok(())
}

/// Handle the unblock command
pub fn handle_unblock(id: i64) -> Result<(), TaskError> {
    let tracker = TaskTracker::open()?;
    check_initialized(&tracker)?;

    let detail = tracker.unblock_task(id)?;
    println!("Unblocked task #{}: {}", detail.task.id, detail.task.title);

    Ok(())
}

/// Handle the current command
pub fn handle_current() -> Result<(), TaskError> {
    let tracker = TaskTracker::open()?;
    check_initialized(&tracker)?;

    let detail = tracker.get_current_task()?;
    let task = &detail.task;

    println!("Active: [#{}] {}", task.id, task.title);
    println!("  Status:    {}", task.status);
    if let Some(started) = task.started_at {
        println!("  Started:   {}", started.format("%Y-%m-%d %H:%M"));
    }
    if let Some(ref dod) = task.dod {
        println!("  DoD:       {dod}");
    }

    if detail.artifacts.is_empty() {
        println!("  Artifacts: (none)");
    } else {
        println!("  Artifacts:");
        for artifact in &detail.artifacts {
            println!("    - {}: {}", artifact.name, artifact.file_path);
        }
    }

    Ok(())
}

/// Handle the depend command
pub fn handle_depend(id: i64, on_id: i64) -> Result<(), TaskError> {
    let tracker = TaskTracker::open()?;
    check_initialized(&tracker)?;

    tracker.add_dependency(id, on_id)?;
    println!("Task #{id} now depends on task #{on_id}");

    Ok(())
}

/// Handle the undepend command
pub fn handle_undepend(id: i64, on_id: i64) -> Result<(), TaskError> {
    let tracker = TaskTracker::open()?;
    check_initialized(&tracker)?;

    tracker.remove_dependency(id, on_id)?;
    println!("Removed dependency: #{id} no longer depends on #{on_id}");

    Ok(())
}

/// Handle the log command
pub fn handle_log(name: &str, file: &str) -> Result<(), TaskError> {
    let tracker = TaskTracker::open()?;
    check_initialized(&tracker)?;

    let artifact = tracker.log_artifact(name, file)?;
    println!(
        "Logged artifact '{}' for task #{}: {}",
        artifact.name, artifact.task_id, artifact.file_path
    );

    Ok(())
}

/// Handle the artifacts command
pub fn handle_artifacts(task_id: Option<i64>) -> Result<(), TaskError> {
    let tracker = TaskTracker::open()?;
    check_initialized(&tracker)?;

    let artifacts = tracker.get_artifacts(task_id)?;

    if artifacts.is_empty() {
        println!("No artifacts found.");
    } else {
        println!("Artifacts:");
        for artifact in artifacts {
            println!(
                "  - {}: {} (task #{})",
                artifact.name, artifact.file_path, artifact.task_id
            );
        }
    }

    Ok(())
}

/// Handle the reorder command
pub fn handle_reorder(id: i64, after: Option<i64>, before: Option<i64>) -> Result<(), TaskError> {
    let tracker = TaskTracker::open()?;
    check_initialized(&tracker)?;

    let task = tracker.reorder_task(id, after, before)?;
    println!(
        "Reordered task #{} to position {}",
        task.id, task.manual_order
    );

    Ok(())
}

/// Handle the reindex command
pub fn handle_reindex() -> Result<(), TaskError> {
    let tracker = TaskTracker::open()?;
    check_initialized(&tracker)?;

    let tasks = tracker.reindex()?;
    println!("Reindexed {} tasks", tasks.len());

    Ok(())
}

// Helper function
fn check_initialized(tracker: &TaskTracker) -> Result<(), TaskError> {
    if !tracker.is_initialized()? {
        return Err(TaskError::NotInitialized);
    }
    Ok(())
}

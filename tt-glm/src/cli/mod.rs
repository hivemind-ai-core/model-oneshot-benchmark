//! CLI commands for tt.

use crate::core::artifact as artifact_ops;
use crate::core::dependency;
use crate::core::{Task, TaskRepository};
use crate::db::schema::Schema;
use crate::db::Connection;
use crate::error::Result;
use std::fs;
use std::path::Path;

/// Run the CLI application.
pub fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_help();
        return Ok(());
    }

    let command = &args[1];

    match command.as_str() {
        "init" => cmd_init(),
        "add" => cmd_add(&args[2..]),
        "edit" => cmd_edit(&args[2..]),
        "show" => cmd_show(&args[2..]),
        "list" => cmd_list(&args[2..]),
        "target" => cmd_target(&args[2..]),
        "next" => cmd_next(),
        "start" => cmd_start(&args[2..]),
        "stop" => cmd_stop(),
        "done" => cmd_done(),
        "block" => cmd_block(&args[2..]),
        "unblock" => cmd_unblock(&args[2..]),
        "current" => cmd_current(),
        "depend" => cmd_depend(&args[2..]),
        "undepend" => cmd_undepend(&args[2..]),
        "log" => cmd_log(&args[2..]),
        "artifacts" => cmd_artifacts(&args[2..]),
        "reorder" => cmd_reorder(&args[2..]),
        "reindex" => cmd_reindex(),
        "mcp" => {
            eprintln!("Error: Use 'tt mcp' in a context where MCP stdio is available");
            Err(crate::error::Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "mcp command not available",
            )))
        }
        _ => {
            eprintln!("Unknown command: {command}");
            print_help();
            Err(crate::error::Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "unknown command",
            )))
        }
    }
}

fn print_help() {
    println!("tt - DAG-based Task Tracker");
    println!();
    println!("Commands:");
    println!("  init                            Initialize a new task tracker");
    println!("  add \"<title>\"                  Create a new task");
    println!("    --desc <description>           Set description");
    println!("    --dod <definition>             Set definition of done");
    println!("    --after <id>                   Insert after task");
    println!("    --before <id>                  Insert before task");
    println!("  edit <id>                       Update a task");
    println!("    --title <title>                New title");
    println!("    --desc <description>           New description");
    println!("    --dod <definition>             New DoD");
    println!("  show <id>                       Show task details");
    println!("  list [--all]                    List tasks in target subgraph");
    println!("  target <id>                     Set the target task");
    println!("  next                            Show next task to work on");
    println!("  start <id>                      Start working on a task");
    println!("  stop                            Stop current task");
    println!("  done                            Complete current task");
    println!("  block <id>                      Mark a task as blocked");
    println!("  unblock <id>                    Unblock a task");
    println!("  current                         Show current task");
    println!("  depend <id> <on_id>             Add a dependency");
    println!("  undepend <id> <on_id>           Remove a dependency");
    println!("  log <name> --file <path>         Link an artifact to current task");
    println!("  artifacts [--task <id>]         Show artifacts");
    println!("  reorder <id>                    Reorder a task");
    println!("    --after <id>                  Move after task");
    println!("    --before <id>                 Move before task");
    println!("  reindex                         Reindex all manual_order values");
}

fn ensure_initialized() -> Result<TaskRepository> {
    let db_path = "tt.db";
    if !Path::new(db_path).exists() {
        return Err(crate::error::Error::NotInitialized);
    }
    TaskRepository::open()
}

fn cmd_init() -> Result<()> {
    if Path::new("tt.db").exists() {
        return Err(crate::error::Error::AlreadyInitialized);
    }

    let mut conn = Connection::open_default()?;
    Schema::init(&mut conn)?;

    // Create .tt directory
    fs::create_dir_all(".tt/artifacts")?;

    println!("Initialized task tracker in current directory");
    Ok(())
}

fn get_required_id(args: &[String], name: &str) -> Result<i64> {
    args.first().and_then(|s| s.parse().ok()).ok_or_else(|| {
        crate::error::Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{name} id required"),
        ))
    })
}

fn cmd_add(args: &[String]) -> Result<()> {
    let mut repo = ensure_initialized()?;

    let title = args.first().cloned().ok_or_else(|| {
        crate::error::Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "title required",
        ))
    })?;

    let mut description = None;
    let mut dod = None;
    let mut after = None;
    let mut before = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--desc" => {
                description = args.get(i + 1).cloned();
                i += 2;
            }
            "--dod" => {
                dod = args.get(i + 1).cloned();
                i += 2;
            }
            "--after" => {
                after = args.get(i + 1).and_then(|s| s.parse().ok());
                i += 2;
            }
            "--before" => {
                before = args.get(i + 1).and_then(|s| s.parse().ok());
                i += 2;
            }
            _ => {
                eprintln!("Unknown flag: {}", args[i]);
                i += 1;
            }
        }
    }

    // Calculate manual_order
    let manual_order = calculate_manual_order(&mut repo, after, before)?;

    let task = repo.create_task(title, description, dod, manual_order)?;
    println!("#{}", task.id);
    Ok(())
}

fn calculate_manual_order(
    repo: &mut TaskRepository,
    after: Option<i64>,
    before: Option<i64>,
) -> Result<f64> {
    use crate::graph::order::{calculate_order, OrderPosition};

    let all_tasks = repo.get_all_tasks()?;
    let max_order = all_tasks.iter().map(|t| t.manual_order).reduce(f64::max);

    let position = match (after, before) {
        (None, None) => OrderPosition::End,
        (Some(after_id), None) => {
            let order = repo.get_task(after_id)?.manual_order as i64;
            OrderPosition::After(order)
        }
        (None, Some(before_id)) => {
            let order = repo.get_task(before_id)?.manual_order as i64;
            OrderPosition::Before(order)
        }
        (Some(after_id), Some(before_id)) => {
            let after_order = repo.get_task(after_id)?.manual_order as i64;
            let before_order = repo.get_task(before_id)?.manual_order as i64;
            OrderPosition::Between(after_order, before_order)
        }
    };

    calculate_order(position, None, None, max_order)
}

fn cmd_edit(args: &[String]) -> Result<()> {
    let mut repo = ensure_initialized()?;

    let id = get_required_id(args, "edit")?;

    let mut title = None;
    let mut description = None;
    let mut dod = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--title" => {
                title = args.get(i + 1).cloned();
                i += 2;
            }
            "--desc" => {
                description = args.get(i + 1).cloned();
                i += 2;
            }
            "--dod" => {
                dod = args.get(i + 1).cloned();
                i += 2;
            }
            _ => {
                eprintln!("Unknown flag: {}", args[i]);
                i += 1;
            }
        }
    }

    repo.update_task(id, title, description, dod)?;
    println!("Updated task #{id}");
    Ok(())
}

fn cmd_show(args: &[String]) -> Result<()> {
    let mut repo = ensure_initialized()?;

    let id = get_required_id(args, "show")?;

    let task = repo.get_task(id)?;
    print_task_detail(&mut repo, &task)?;
    Ok(())
}

fn print_task_detail(repo: &mut TaskRepository, task: &Task) -> Result<()> {
    println!("[#{}] {}", task.id, task.title);
    println!("Status:       {}", task.status.as_str());
    println!("Order:        {}", task.manual_order);
    println!("Created:      {}", format_datetime(&task.created_at));
    if let Some(dod) = &task.dod {
        println!("DoD:          {dod}");
    }

    let deps = dependency::get_dependencies(repo.conn(), task.id)?;
    if !deps.is_empty() {
        let dep_str: Vec<String> = deps
            .iter()
            .map(|id| {
                repo.get_task(*id)
                    .map(|t| format!("#{} ({})", id, t.status_char()))
                    .unwrap_or_else(|_| format!("#{id}"))
            })
            .collect();
        println!("Dependencies: {}", dep_str.join(", "));
    }

    let dependents = dependency::get_dependents(repo.conn(), task.id)?;
    if !dependents.is_empty() {
        println!(
            "Dependents:   {}",
            dependents
                .iter()
                .map(|id| format!("#{id}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    let artifacts = artifact_ops::get_artifacts(repo.conn(), task.id)?;
    if artifacts.is_empty() {
        println!("Artifacts:    (none)");
    } else {
        println!("Artifacts:");
        for artifact in artifacts {
            println!("  - {}: {}", artifact.name, artifact.file_path);
        }
    }

    Ok(())
}

fn cmd_list(args: &[String]) -> Result<()> {
    let mut repo = ensure_initialized()?;

    let all = args.iter().any(|a| a == "--all");
    let tasks = repo.list_tasks(all)?;

    if let Ok(target_id) = repo.get_target() {
        if let Some(tid) = target_id {
            println!("Target: #{tid}");
        }
    }

    for task in tasks {
        println!("  [#{}] {} {}", task.id, task.status_char(), task.title);

        let deps = dependency::get_dependencies(repo.conn(), task.id)?;
        if !deps.is_empty() {
            let dep_str: Vec<String> = deps
                .iter()
                .map(|id| {
                    repo.get_task(*id)
                        .map(|t| format!("#{} ({})", id, t.status_char()))
                        .unwrap_or_else(|_| format!("#{id}"))
                })
                .collect();
            println!("      (deps: {})", dep_str.join(", "));
        }
    }

    println!();
    println!("Legend: ✓ completed  ● in_progress  ○ pending  ✗ blocked");
    Ok(())
}

fn cmd_target(args: &[String]) -> Result<()> {
    let mut repo = ensure_initialized()?;

    let id = get_required_id(args, "target")?;

    repo.set_target(id)?;
    println!("Set target to task #{id}");
    Ok(())
}

fn cmd_next() -> Result<()> {
    let mut repo = ensure_initialized()?;

    let task = repo.get_next_task()?;

    println!("Next: [#{}] {}", task.id, task.title);

    let deps = dependency::get_dependencies(repo.conn(), task.id)?;
    if deps.is_empty() {
        println!("  Dependencies: (none)");
    } else {
        println!(
            "  Dependencies: {}",
            deps.iter()
                .map(|id| format!("#{id}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    if let Some(dod) = &task.dod {
        println!("  DoD: {dod}");
    }

    Ok(())
}

fn cmd_start(args: &[String]) -> Result<()> {
    let mut repo = ensure_initialized()?;

    let id = get_required_id(args, "start")?;

    let task = repo.start_task(id)?;
    println!("Started: [#{}] {}", task.id, task.title);
    Ok(())
}

fn cmd_stop() -> Result<()> {
    let mut repo = ensure_initialized()?;

    let task = repo.stop_task()?;
    println!("Stopped: [#{}] {}", task.id, task.title);
    Ok(())
}

fn cmd_done() -> Result<()> {
    let mut repo = ensure_initialized()?;

    let task = repo.complete_task()?;
    println!("Completed: [#{}] {}", task.id, task.title);
    Ok(())
}

fn cmd_block(args: &[String]) -> Result<()> {
    let mut repo = ensure_initialized()?;

    let id = get_required_id(args, "block")?;

    let task = repo.block_task(id)?;
    println!("Blocked: [#{}] {}", task.id, task.title);
    Ok(())
}

fn cmd_unblock(args: &[String]) -> Result<()> {
    let mut repo = ensure_initialized()?;

    let id = get_required_id(args, "unblock")?;

    let task = repo.unblock_task(id)?;
    println!("Unblocked: [#{}] {}", task.id, task.title);
    Ok(())
}

fn cmd_current() -> Result<()> {
    let mut repo = ensure_initialized()?;

    let task = repo.get_active_task()?;

    println!("Active: [#{}] {}", task.id, task.title);
    println!("  Status:    {}", task.status.as_str());

    if let Some(started) = &task.started_at {
        println!("  Started:   {}", format_datetime(started));
    }

    if let Some(dod) = &task.dod {
        println!("  DoD:       {dod}");
    }

    let (_task_id, artifacts) = artifact_ops::get_active_task_artifacts(repo.conn())?;
    if artifacts.is_empty() {
        println!("  Artifacts: (none)");
    } else {
        println!("  Artifacts:");
        for artifact in artifacts {
            println!("    - {}: {}", artifact.name, artifact.file_path);
        }
    }

    Ok(())
}

fn cmd_depend(args: &[String]) -> Result<()> {
    let mut repo = ensure_initialized()?;

    let id = get_required_id(args, "depend")?;

    let on_id = get_required_id(&args[1..], "depends_on")?;

    dependency::add_dependency(repo.conn(), id, on_id)?;
    println!("Added dependency: #{id} depends on #{on_id}");
    Ok(())
}

fn cmd_undepend(args: &[String]) -> Result<()> {
    let mut repo = ensure_initialized()?;

    let id = get_required_id(args, "undepend")?;

    let on_id = get_required_id(&args[1..], "depends_on")?;

    dependency::remove_dependency(repo.conn(), id, on_id)?;
    println!(
        "Removed dependency: #{id} no longer depends on #{on_id}"
    );
    Ok(())
}

fn cmd_log(args: &[String]) -> Result<()> {
    let mut repo = ensure_initialized()?;

    let mut name = None;
    let mut file_path = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--file" => {
                file_path = args.get(i + 1).cloned();
                i += 2;
            }
            _ => {
                if name.is_none() {
                    name = Some(args[i].clone());
                }
                i += 1;
            }
        }
    }

    let name = name.ok_or_else(|| {
        crate::error::Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "artifact name required",
        ))
    })?;

    let file_path = file_path.ok_or_else(|| {
        crate::error::Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "--file <path> required",
        ))
    })?;

    let (task_id, _) = artifact_ops::get_active_task_artifacts(repo.conn())?;
    artifact_ops::add_artifact(repo.conn(), task_id, name.clone(), file_path.clone())?;

    println!("Logged artifact '{name}' for task #{task_id}");
    Ok(())
}

fn cmd_artifacts(args: &[String]) -> Result<()> {
    let mut repo = ensure_initialized()?;

    let task_id = if let Some(pos) = args.iter().position(|a| a == "--task") {
        args.get(pos + 1)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| {
                crate::error::Error::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "--task requires an id",
                ))
            })?
    } else {
        let task = repo.get_active_task()?;
        task.id
    };

    let artifacts = artifact_ops::get_artifacts(repo.conn(), task_id)?;

    if artifacts.is_empty() {
        println!("No artifacts for task #{task_id}");
    } else {
        println!("Artifacts for task #{task_id}:");
        for artifact in artifacts {
            println!(
                "  [{}] {} - {}",
                artifact.id, artifact.name, artifact.file_path
            );
        }
    }

    Ok(())
}

fn cmd_reorder(args: &[String]) -> Result<()> {
    let mut repo = ensure_initialized()?;

    let id = get_required_id(args, "reorder")?;

    let mut after = None;
    let mut before = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--after" => {
                after = args.get(i + 1).and_then(|s| s.parse().ok());
                i += 2;
            }
            "--before" => {
                before = args.get(i + 1).and_then(|s| s.parse().ok());
                i += 2;
            }
            _ => {
                i += 1;
            }
        }
    }

    if after.is_none() && before.is_none() {
        return Err(crate::error::Error::NeedAfterOrBefore);
    }

    let new_order = calculate_manual_order(&mut repo, after, before)?;

    repo.conn().execute(
        "UPDATE tasks SET manual_order = ?, last_touched_at = strftime('%Y-%m-%dT%H:%M:%S', 'now') WHERE id = ?",
        &[&new_order as &dyn rusqlite::ToSql, &id as &dyn rusqlite::ToSql],
    )?;

    println!("Reordered task #{id} to order {new_order}");
    Ok(())
}

fn cmd_reindex() -> Result<()> {
    let mut repo = ensure_initialized()?;

    repo.reindex()?;
    println!("Reindexed all tasks");
    Ok(())
}

fn format_datetime(dt: &str) -> String {
    // Basic formatting - just return the first 19 characters (YYYY-MM-DD HH:MM:SS)
    dt.chars().take(19).collect()
}

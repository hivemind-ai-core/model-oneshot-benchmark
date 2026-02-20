use clap::{Parser, Subcommand};
use std::fs;

use crate::core::CoreImpl;
use crate::db::Database;
use crate::error::{Error, Result};
use crate::models::Status;

#[derive(Parser)]
#[command(name = "tt")]
#[command(about = "DAG-Based Task Tracker", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Init,
    Add {
        title: String,
        #[arg(long)]
        desc: Option<String>,
        #[arg(long)]
        dod: Option<String>,
        #[arg(long)]
        after: Option<i64>,
        #[arg(long)]
        before: Option<i64>,
    },
    Edit {
        id: i64,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        desc: Option<String>,
        #[arg(long)]
        dod: Option<String>,
    },
    Show {
        id: i64,
    },
    List {
        #[arg(long)]
        all: bool,
    },
    Target {
        id: i64,
    },
    Next,
    Start {
        id: i64,
    },
    Stop,
    Done,
    Block {
        id: i64,
    },
    Unblock {
        id: i64,
    },
    Current,
    Depend {
        id: i64,
        on_id: i64,
    },
    Undepend {
        id: i64,
        on_id: i64,
    },
    Log {
        name: String,
        #[arg(long)]
        file: String,
    },
    Artifacts {
        #[arg(long)]
        task: Option<i64>,
    },
    Reorder {
        id: i64,
        #[arg(long)]
        after: Option<i64>,
        #[arg(long)]
        before: Option<i64>,
    },
    Reindex,
    Mcp,
}

fn get_db() -> Result<Database> {
    Database::new("tt.db")
}

fn get_core() -> Result<CoreImpl> {
    let db = get_db()?;
    Ok(CoreImpl::new(db))
}

fn status_icon(status: &Status) -> &'static str {
    match status {
        Status::Pending => "○",
        Status::InProgress => "●",
        Status::Completed => "✓",
        Status::Blocked => "✗",
    }
}

pub fn run() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => cmd_init()?,
        Commands::Add {
            title,
            desc,
            dod,
            after,
            before,
        } => cmd_add(&title, desc.as_deref(), dod.as_deref(), after, before)?,
        Commands::Edit {
            id,
            title,
            desc,
            dod,
        } => cmd_edit(id, title.as_deref(), desc.as_deref(), dod.as_deref())?,
        Commands::Show { id } => cmd_show(id)?,
        Commands::List { all } => cmd_list(all)?,
        Commands::Target { id } => cmd_target(id)?,
        Commands::Next => cmd_next()?,
        Commands::Start { id } => cmd_start(id)?,
        Commands::Stop => cmd_stop()?,
        Commands::Done => cmd_done()?,
        Commands::Block { id } => cmd_block(id)?,
        Commands::Unblock { id } => cmd_unblock(id)?,
        Commands::Current => cmd_current()?,
        Commands::Depend { id, on_id } => cmd_depend(id, on_id)?,
        Commands::Undepend { id, on_id } => cmd_undepend(id, on_id)?,
        Commands::Log { name, file } => cmd_log(&name, &file)?,
        Commands::Artifacts { task } => cmd_artifacts(task)?,
        Commands::Reorder { id, after, before } => cmd_reorder(id, after, before)?,
        Commands::Reindex => cmd_reindex()?,
        Commands::Mcp => cmd_mcp()?,
    }

    Ok(())
}

fn cmd_init() -> std::result::Result<(), Box<dyn std::error::Error>> {
    if Database::exists("tt.db") {
        return Err("Already initialized. Delete tt.db to reinitialize".into());
    }

    fs::create_dir_all(".tt/artifacts")?;
    let _db = Database::new("tt.db")?;

    println!("Initialized: tt.db and .tt/artifacts/");

    Ok(())
}

fn cmd_add(
    title: &str,
    description: Option<&str>,
    dod: Option<&str>,
    after: Option<i64>,
    before: Option<i64>,
) -> Result<()> {
    let core = get_core()?;
    let task = core.add_task(title, description, dod, after, before)?;
    println!("{}", task.id);
    Ok(())
}

fn cmd_edit(
    id: i64,
    title: Option<&str>,
    description: Option<&str>,
    dod: Option<&str>,
) -> Result<()> {
    let core = get_core()?;
    let _task = core.edit_task(id, title, description, dod)?;
    println!("Task #{id} updated");
    Ok(())
}

fn cmd_show(id: i64) -> Result<()> {
    let core = get_core()?;
    let task_with_deps = core.show_task(id)?;

    let task = &task_with_deps.task;
    let deps = &task_with_deps.dependencies;
    let dependents = &task_with_deps.dependents;

    println!("[#{}] {}", task.id, task.title);
    println!("Status:       {}", task.status.as_str());
    println!("Order:        {}", task.manual_order);
    println!("Created:      {}", task.created_at);
    if let Some(dod) = &task.dod {
        println!("DoD:          {dod}");
    }

    print!("Dependencies: ");
    if deps.is_empty() {
        println!("(none)");
    } else {
        for (i, dep) in deps.iter().enumerate() {
            if i > 0 {
                print!(", ");
            }
            print!("#{} ({})", dep.id, status_icon(&dep.status));
        }
        println!();
    }

    print!("Dependents:   ");
    if dependents.is_empty() {
        println!("(none)");
    } else {
        for (i, dep) in dependents.iter().enumerate() {
            if i > 0 {
                print!(", ");
            }
            print!("#{dep}");
        }
        println!();
    }

    let artifacts = core.get_artifacts(Some(id))?;
    print!("Artifacts:    ");
    if artifacts.is_empty() {
        println!("(none)");
    } else {
        println!("{} file(s)", artifacts.len());
    }

    Ok(())
}

fn cmd_list(all: bool) -> Result<()> {
    let core = get_core()?;
    let (tasks, target_id, warnings) = core.list_tasks(all)?;

    if tasks.is_empty() {
        println!("(no tasks)");
        return Ok(());
    }

    if let Some(tid) = target_id {
        if let Ok(target_task) = get_core()?.show_task(tid) {
            println!("Target: #{} ({})", tid, target_task.task.title);
        }
    }

    for task in &tasks {
        let deps = core.db.get_dependencies(task.id).unwrap_or_default();

        print!(
            "  [#{:>2}] {} {}",
            task.id,
            status_icon(&task.status),
            task.title
        );

        if !deps.is_empty() {
            print!(" (deps: ");
            for (i, dep_id) in deps.iter().enumerate() {
                if i > 0 {
                    print!(", ");
                }
                if let Ok(dep) = get_core()?.db.get_task(*dep_id) {
                    print!("#{} {}", dep.id, status_icon(&dep.status));
                }
            }
            print!(")");
        }
        println!();
    }

    println!();
    println!("Legend: ✓ completed  ● in_progress  ○ pending  ✗ blocked");

    for warning in warnings {
        eprintln!("{warning}");
    }

    Ok(())
}

fn cmd_target(id: i64) -> Result<()> {
    let core = get_core()?;
    core.set_target(id)?;
    println!("Target set to #{id}");
    Ok(())
}

fn cmd_next() -> Result<()> {
    let core = get_core()?;

    match core.next_task() {
        Ok((Some(task), _)) => {
            let deps = core.db.get_dependencies(task.id).unwrap_or_default();
            let dep_status: Vec<(i64, String)> = deps
                .iter()
                .filter_map(|d| {
                    core.db
                        .get_task(*d)
                        .ok()
                        .map(|t| (t.id, t.status.as_str().to_string()))
                })
                .collect();

            println!("Next: [#{}] {}", task.id, task.title);
            print!("  Dependencies: ");
            for (i, (id, status)) in dep_status.iter().enumerate() {
                if i > 0 {
                    print!(", ");
                }
                print!("#{id} {status}");
            }
            println!(" (all met)");

            if let Some(dod) = &task.dod {
                println!("  DoD: {dod}");
            }
        }
        Ok((None, _)) => {
            if let Some(target_id) = core.get_target()? {
                println!("Target Reached: all tasks for #{target_id} are completed.");
            }
        }
        Err(Error::AllBlocked(ids)) => {
            println!("All remaining tasks are blocked:");
            for id in ids.split(", ") {
                println!("  #{} ✗ blocked", id.trim_start_matches('#'));
            }
        }
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }

    Ok(())
}

fn cmd_start(id: i64) -> Result<()> {
    let core = get_core()?;
    let task = core.start_task(id)?;
    println!("Started: [#{}] {}", task.id, task.title);
    Ok(())
}

fn cmd_stop() -> Result<()> {
    let core = get_core()?;
    let task = core.stop_task()?;
    println!("Stopped: [#{}] {}", task.id, task.title);
    Ok(())
}

fn cmd_done() -> Result<()> {
    let core = get_core()?;
    let task = core.complete_task()?;
    println!("Completed: [#{}] {}", task.id, task.title);
    Ok(())
}

fn cmd_block(id: i64) -> Result<()> {
    let core = get_core()?;
    let task = core.block_task(id)?;
    println!("Blocked: [#{}] {}", task.id, task.title);
    Ok(())
}

fn cmd_unblock(id: i64) -> Result<()> {
    let core = get_core()?;
    let task = core.unblock_task(id)?;
    println!("Unblocked: [#{}] {}", task.id, task.title);
    Ok(())
}

fn cmd_current() -> Result<()> {
    let core = get_core()?;
    let task_with_deps = core.current_task()?;

    let task = &task_with_deps.task;

    println!("Active: [#{}] {}", task.id, task.title);
    println!("  Status:    {}", task.status.as_str());
    if let Some(started) = &task.started_at {
        println!("  Started:   {started}");
    }
    if let Some(dod) = &task.dod {
        println!("  DoD:       {dod}");
    }

    let artifacts = core.get_artifacts(Some(task.id))?;
    println!("  Artifacts:");
    if artifacts.is_empty() {
        println!("    (none)");
    } else {
        for artifact in artifacts {
            println!("    - {}: {}", artifact.name, artifact.file_path);
        }
    }

    Ok(())
}

fn cmd_depend(id: i64, on_id: i64) -> Result<()> {
    let core = get_core()?;
    core.add_dependency(id, on_id)?;
    println!("#{id} now depends on #{on_id}");
    Ok(())
}

fn cmd_undepend(id: i64, on_id: i64) -> Result<()> {
    let core = get_core()?;
    core.remove_dependency(id, on_id)?;
    println!("Removed dependency: #{id} no longer depends on #{on_id}");
    Ok(())
}

fn cmd_log(name: &str, file: &str) -> Result<()> {
    let core = get_core()?;
    let artifact = core.log_artifact(name, file)?;
    println!("Logged: {} -> {}", artifact.name, artifact.file_path);
    Ok(())
}

fn cmd_artifacts(task_id: Option<i64>) -> Result<()> {
    let core = get_core()?;

    let task_id = match task_id {
        Some(id) => id,
        None => {
            let active = core.current_task()?;
            active.task.id
        }
    };

    let artifacts = core.get_artifacts(Some(task_id))?;

    if artifacts.is_empty() {
        println!("No artifacts for task #{task_id}");
    } else {
        for artifact in artifacts {
            println!("  {}: {}", artifact.name, artifact.file_path);
        }
    }

    Ok(())
}

fn cmd_reorder(id: i64, after: Option<i64>, before: Option<i64>) -> Result<()> {
    let core = get_core()?;
    let new_order = core.reorder_task(id, after, before)?;
    println!("Task #{id} moved to order {new_order}");
    Ok(())
}

fn cmd_reindex() -> Result<()> {
    let core = get_core()?;
    core.reindex()?;
    println!("Reindexed all tasks");
    Ok(())
}

fn cmd_mcp() -> Result<()> {
    use crate::mcp::run_mcp_server;
    run_mcp_server()
}

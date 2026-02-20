use crate::core::error::{TTError, TTResult};
use crate::core::AppCore;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

const DB_FILE: &str = "tt.db";
const ARTIFACTS_DIR: &str = ".tt/artifacts";

#[derive(Parser)]
#[command(name = "tt")]
#[command(about = "DAG-Based Task Tracker")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new tt project
    Init,

    /// Add a new task
    Add {
        /// Task title
        title: String,
        /// Task description
        #[arg(long)]
        desc: Option<String>,
        /// Definition of Done
        #[arg(long)]
        dod: Option<String>,
        /// Insert after this task ID
        #[arg(long)]
        after: Option<i64>,
        /// Insert before this task ID
        #[arg(long)]
        before: Option<i64>,
    },

    /// Edit a task
    Edit {
        /// Task ID
        id: i64,
        /// New title
        #[arg(long)]
        title: Option<String>,
        /// New description
        #[arg(long)]
        desc: Option<String>,
        /// New Definition of Done
        #[arg(long)]
        dod: Option<String>,
    },

    /// Show task details
    Show {
        /// Task ID
        id: i64,
    },

    /// List tasks
    List {
        /// Show all tasks (not just target subgraph)
        #[arg(long)]
        all: bool,
    },

    /// Set target task
    Target {
        /// Task ID
        id: i64,
    },

    /// Get next task to work on
    Next,

    /// Start a task
    Start {
        /// Task ID
        id: i64,
    },

    /// Stop the active task
    Stop,

    /// Complete the active task
    Done,

    /// Block a task
    Block {
        /// Task ID
        id: i64,
    },

    /// Unblock a task
    Unblock {
        /// Task ID
        id: i64,
    },

    /// Show current active task
    Current,

    /// Add a dependency
    Depend {
        /// Task ID (the dependent)
        id: i64,
        /// Task ID to depend on
        on_id: i64,
    },

    /// Remove a dependency
    Undepend {
        /// Task ID (the dependent)
        id: i64,
        /// Task ID to remove dependency on
        on_id: i64,
    },

    /// Log an artifact for the active task
    Log {
        /// Artifact name
        name: String,
        /// File path
        #[arg(long)]
        file: String,
    },

    /// List artifacts
    Artifacts {
        /// Task ID (defaults to active task)
        #[arg(long)]
        task: Option<i64>,
    },

    /// Reorder a task
    Reorder {
        /// Task ID
        id: i64,
        /// Insert after this task
        #[arg(long)]
        after: Option<i64>,
        /// Insert before this task
        #[arg(long)]
        before: Option<i64>,
    },

    /// Reindex all task orders
    Reindex,

    /// Start MCP server
    Mcp,
}

pub fn run() -> TTResult<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => cmd_init(),
        Commands::Add {
            title,
            desc,
            dod,
            after,
            before,
        } => cmd_add(title, desc, dod, after, before),
        Commands::Edit {
            id,
            title,
            desc,
            dod,
        } => cmd_edit(id, title, desc, dod),
        Commands::Show { id } => cmd_show(id),
        Commands::List { all } => cmd_list(all),
        Commands::Target { id } => cmd_target(id),
        Commands::Next => cmd_next(),
        Commands::Start { id } => cmd_start(id),
        Commands::Stop => cmd_stop(),
        Commands::Done => cmd_done(),
        Commands::Block { id } => cmd_block(id),
        Commands::Unblock { id } => cmd_unblock(id),
        Commands::Current => cmd_current(),
        Commands::Depend { id, on_id } => cmd_depend(id, on_id),
        Commands::Undepend { id, on_id } => cmd_undepend(id, on_id),
        Commands::Log { name, file } => cmd_log(name, file),
        Commands::Artifacts { task } => cmd_artifacts(task),
        Commands::Reorder { id, after, before } => cmd_reorder(id, after, before),
        Commands::Reindex => cmd_reindex(),
        Commands::Mcp => crate::mcp::run_mcp_server(),
    }
}

fn get_core() -> TTResult<AppCore> {
    let db_path = PathBuf::from(DB_FILE);
    if !db_path.exists() {
        return Err(TTError::NotInitialized);
    }
    AppCore::open(db_path)
}

fn cmd_init() -> TTResult<()> {
    let db_path = PathBuf::from(DB_FILE);
    let artifacts_path = PathBuf::from(ARTIFACTS_DIR);

    AppCore::init(&db_path, &artifacts_path)?;
    println!("Initialized tt project in current directory");
    println!("  Database: {}", db_path.display());
    println!("  Artifacts: {}", artifacts_path.display());
    Ok(())
}

fn cmd_add(
    title: String,
    desc: Option<String>,
    dod: Option<String>,
    after: Option<i64>,
    before: Option<i64>,
) -> TTResult<()> {
    let core = get_core()?;
    let task = core.add_task(&title, desc.as_deref(), dod.as_deref(), after, before)?;
    println!("Created task #{}: {}", task.id, task.title);
    Ok(())
}

fn cmd_edit(
    id: i64,
    title: Option<String>,
    desc: Option<String>,
    dod: Option<String>,
) -> TTResult<()> {
    let core = get_core()?;
    let task = core.edit_task(id, title.as_deref(), desc.as_deref(), dod.as_deref())?;
    println!("Updated task #{}: {}", task.id, task.title);
    Ok(())
}

fn cmd_show(id: i64) -> TTResult<()> {
    let core = get_core()?;
    let detail = core.get_task_detail(id)?;
    let task = &detail.task;

    println!("[#{id}] {}", task.title);
    println!("Status:       {}", task.status);
    println!("Order:        {}", task.manual_order);
    println!("Created:      {}", task.created_at.format("%Y-%m-%d %H:%M"));

    if let Some(ref dod) = task.dod {
        println!("DoD:          {dod}");
    }

    if !detail.dependencies.is_empty() {
        let deps_str: Vec<String> = detail
            .dependencies
            .iter()
            .map(|d| format!("#{} ({})", d.id, d.status.icon()))
            .collect();
        println!("Dependencies: {}", deps_str.join(", "));
    }

    if !detail.dependents.is_empty() {
        let dep_str: Vec<String> = detail
            .dependents
            .iter()
            .map(|id| format!("#{id}"))
            .collect();
        println!("Dependents:   {}", dep_str.join(", "));
    }

    if !detail.artifacts.is_empty() {
        println!("Artifacts:");
        for artifact in &detail.artifacts {
            println!("  - {}: {}", artifact.name, artifact.file_path);
        }
    } else {
        println!("Artifacts:    (none)");
    }

    Ok(())
}

fn cmd_list(all: bool) -> TTResult<()> {
    let core = get_core()?;

    if !all {
        match core.get_target()? {
            Some(target_id) => {
                let target = core.get_task(target_id)?;
                println!("Target: #{} ({})", target_id, target.title);
            }
            None => {
                return Err(TTError::NoTarget);
            }
        }
    }

    let (tasks, conflicts) = core.list_tasks(all)?;

    for task in &tasks {
        let deps = core.db.get_dependencies(task.id)?;
        let dep_str = if deps.is_empty() {
            String::new()
        } else {
            let dep_parts: Vec<String> = deps
                .iter()
                .map(|d| format!("#{} {}", d.id, d.status.icon()))
                .collect();
            format!("  (deps: {})", dep_parts.join(", "))
        };

        println!(
            "  [#{:>3}] {} {:<25}{}",
            task.id,
            task.status.icon(),
            task.title,
            dep_str
        );
    }

    if !conflicts.is_empty() {
        eprintln!("\nWarnings:");
        for conflict in conflicts {
            eprintln!(
                "  Task #{} (order {}) depends on #{} (order {}) - manual order conflict",
                conflict.task_id, conflict.task_order, conflict.dep_id, conflict.dep_order
            );
        }
    }

    if !all {
        println!("\nLegend: ✓ completed  ● in_progress  ○ pending  ✗ blocked");
    }

    Ok(())
}

fn cmd_target(id: i64) -> TTResult<()> {
    let core = get_core()?;
    core.set_target(id)?;
    let task = core.get_task(id)?;
    println!("Target set to #{}: {}", id, task.title);
    Ok(())
}

fn cmd_next() -> TTResult<()> {
    let core = get_core()?;

    match core.next_task()? {
        Some(task) => {
            println!("Next: [#{}] {}", task.id, task.title);

            let deps = core.db.get_dependencies(task.id)?;
            if !deps.is_empty() {
                let all_completed = deps.iter().all(|d| d.is_completed());
                let dep_status: Vec<String> = deps
                    .iter()
                    .map(|d| format!("#{} {}", d.id, d.status.icon()))
                    .collect();

                if all_completed {
                    println!("  Dependencies: {} (all met)", dep_status.join(", "));
                } else {
                    println!("  Dependencies: {}", dep_status.join(", "));
                }
            }

            if let Some(ref dod) = task.dod {
                println!("  DoD: {dod}");
            }
        }
        None => {
            println!("No tasks ready. All remaining tasks have unmet dependencies.");
        }
    }

    Ok(())
}

fn cmd_start(id: i64) -> TTResult<()> {
    let core = get_core()?;
    let task = core.start_task(id)?;
    println!("Started task #{}: {}", task.id, task.title);
    Ok(())
}

fn cmd_stop() -> TTResult<()> {
    let core = get_core()?;
    let task = core.stop_task()?;
    println!("Stopped task #{}: {}", task.id, task.title);
    Ok(())
}

fn cmd_done() -> TTResult<()> {
    let core = get_core()?;
    let task = core.complete_task()?;
    println!("Completed task #{}: {}", task.id, task.title);
    Ok(())
}

fn cmd_block(id: i64) -> TTResult<()> {
    let core = get_core()?;
    let task = core.block_task(id)?;
    println!("Blocked task #{}: {}", task.id, task.title);
    Ok(())
}

fn cmd_unblock(id: i64) -> TTResult<()> {
    let core = get_core()?;
    let task = core.unblock_task(id)?;
    println!("Unblocked task #{}: {}", task.id, task.title);
    Ok(())
}

fn cmd_current() -> TTResult<()> {
    let core = get_core()?;

    match core.get_active_task()? {
        Some(task) => {
            println!("Active: [#{}] {}", task.id, task.title);
            println!("  Status:    {}", task.status);

            if let Some(ref started) = task.started_at {
                println!("  Started:   {}", started.format("%Y-%m-%d %H:%M"));
            }

            if let Some(ref dod) = task.dod {
                println!("  DoD:       {dod}");
            }

            let artifacts = core.get_artifacts(None)?;
            if !artifacts.is_empty() {
                println!("  Artifacts:");
                for artifact in &artifacts {
                    println!("    - {}: {}", artifact.name, artifact.file_path);
                }
            }
        }
        None => {
            return Err(TTError::NoActiveTask);
        }
    }

    Ok(())
}

fn cmd_depend(id: i64, on_id: i64) -> TTResult<()> {
    let core = get_core()?;
    core.add_dependency(id, on_id)?;
    println!("Task #{id} now depends on #{on_id}");
    Ok(())
}

fn cmd_undepend(id: i64, on_id: i64) -> TTResult<()> {
    let core = get_core()?;
    core.remove_dependency(id, on_id)?;
    println!("Removed dependency: #{id} no longer depends on #{on_id}");
    Ok(())
}

fn cmd_log(name: String, file: String) -> TTResult<()> {
    let core = get_core()?;
    let artifact = core.log_artifact(&name, &file)?;
    println!(
        "Logged artifact '{}' for task #{}: {}",
        artifact.name, artifact.task_id, artifact.file_path
    );
    Ok(())
}

fn cmd_artifacts(task: Option<i64>) -> TTResult<()> {
    let core = get_core()?;
    let artifacts = core.get_artifacts(task)?;

    if artifacts.is_empty() {
        println!("No artifacts found");
        return Ok(());
    }

    for artifact in artifacts {
        println!(
            "  - {}: {} (task #{})",
            artifact.name, artifact.file_path, artifact.task_id
        );
    }

    Ok(())
}

fn cmd_reorder(id: i64, after: Option<i64>, before: Option<i64>) -> TTResult<()> {
    let core = get_core()?;
    let order = core.reorder_task(id, after, before)?;
    println!("Reordered task #{id} to position {order}");
    Ok(())
}

fn cmd_reindex() -> TTResult<()> {
    let core = get_core()?;
    core.reindex()?;
    println!("Reindexed all tasks");
    Ok(())
}

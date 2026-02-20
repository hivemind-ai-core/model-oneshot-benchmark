//! CLI interface for the tt task tracker.

pub mod output;

use crate::core::error::Result;
use crate::core::task::TaskManager;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// tt - DAG-based task tracker
#[derive(Parser, Debug)]
#[command(name = "tt")]
#[command(about = "A DAG-based task tracker for AI-driven development", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Initialize a new tt database
    Init,

    /// Add a new task
    Add {
        /// Task title
        title: String,
        /// Task description
        #[arg(short, long)]
        desc: Option<String>,
        /// Definition of Done
        #[arg(long)]
        dod: Option<String>,
        /// Position after this task ID
        #[arg(long)]
        after: Option<i64>,
        /// Position before this task ID
        #[arg(long)]
        before: Option<i64>,
    },

    /// Edit a task
    Edit {
        /// Task ID
        id: i64,
        /// New title
        #[arg(short, long)]
        title: Option<String>,
        /// New description
        #[arg(short, long)]
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
        /// Show all tasks, not just target subgraph
        #[arg(long)]
        all: bool,
    },

    /// Set the target task
    Target {
        /// Task ID
        id: i64,
    },

    /// Show the next task to work on
    Next,

    /// Start working on a task
    Start {
        /// Task ID
        id: i64,
    },

    /// Stop working on the current task
    Stop,

    /// Mark the current task as done
    Done,

    /// Mark a task as blocked
    Block {
        /// Task ID
        id: i64,
    },

    /// Mark a task as unblocked
    Unblock {
        /// Task ID
        id: i64,
    },

    /// Show the current task
    Current,

    /// Add a dependency
    Depend {
        /// Task ID
        id: i64,
        /// Dependency task ID
        on_id: i64,
    },

    /// Remove a dependency
    Undepend {
        /// Task ID
        id: i64,
        /// Dependency task ID
        on_id: i64,
    },

    /// Log an artifact for the current task
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
        /// Position after this task ID
        #[arg(long)]
        after: Option<i64>,
        /// Position before this task ID
        #[arg(long)]
        before: Option<i64>,
    },

    /// Reindex all task orders
    Reindex,

    /// Start MCP server
    Mcp,
}

/// Get the database path.
fn db_path() -> PathBuf {
    PathBuf::from("tt.db")
}

/// Get the artifacts directory path.
fn artifacts_dir() -> PathBuf {
    PathBuf::from(".tt/artifacts")
}

/// Initialize the database.
fn init() -> Result<()> {
    let db_path = db_path();

    if db_path.exists() {
        eprintln!("Error: tt.db already exists in this directory");
        std::process::exit(1);
    }

    let db = crate::core::db::Db::open(&db_path)?;
    db.init_schema()?;

    // Create artifacts directory
    let artifacts = artifacts_dir();
    std::fs::create_dir_all(&artifacts)?;

    println!("Initialized tt database at {}", db_path.display());
    Ok(())
}

/// Open the database and create a task manager.
fn open_manager() -> Result<TaskManager> {
    let db_path = db_path();

    if !db_path.exists() {
        eprintln!("Error: tt.db not found. Run `tt init` first.");
        std::process::exit(1);
    }

    let db = crate::core::db::Db::open(&db_path)?;
    Ok(TaskManager::new(db))
}

/// Run the CLI.
pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init => init()?,

        Command::Add {
            title,
            desc,
            dod,
            after,
            before,
        } => {
            let mut mgr = open_manager()?;
            let id = mgr.add_task(&title, desc.as_deref(), dod.as_deref(), after, before)?;
            println!("{}", id);
        }

        Command::Edit {
            id,
            title,
            desc,
            dod,
        } => {
            let mut mgr = open_manager()?;
            mgr.edit_task(id, title.as_deref(), desc.as_deref(), dod.as_deref())?;
            println!("Task #{} updated", id);
        }

        Command::Show { id } => {
            let mgr = open_manager()?;
            let task_with_deps = mgr.show_task(id)?;
            output::format_show(&task_with_deps);
        }

        Command::List { all } => {
            let mgr = open_manager()?;
            let tasks = mgr.list_tasks(all)?;

            if all {
                output::format_list_all(&tasks);
            } else {
                let target_id = mgr.get_target()?;
                output::format_list(&tasks, target_id);
            }
        }

        Command::Target { id } => {
            let mut mgr = open_manager()?;
            mgr.set_target(id)?;
            println!("Target set to task #{}", id);
        }

        Command::Next => {
            let mgr = open_manager()?;
            match mgr.next_task() {
                Ok(task) => output::format_next(&task, mgr.db())?,
                Err(e) => output::format_next_error(e),
            }
        }

        Command::Start { id } => {
            let mut mgr = open_manager()?;
            let task = mgr.start_task(id)?;
            println!("Started [#{}] {}", task.id, task.title);
        }

        Command::Stop => {
            let mut mgr = open_manager()?;
            let task = mgr.stop_task()?;
            println!("Stopped [#{}] {}", task.id, task.title);
        }

        Command::Done => {
            let mut mgr = open_manager()?;
            let task = mgr.complete_task()?;
            println!("Completed [#{}] {}", task.id, task.title);
        }

        Command::Block { id } => {
            let mut mgr = open_manager()?;
            let task = mgr.block_task(id)?;
            println!("Blocked [#{}] {}", task.id, task.title);
        }

        Command::Unblock { id } => {
            let mut mgr = open_manager()?;
            let task = mgr.unblock_task(id)?;
            println!("Unblocked [#{}] {}", task.id, task.title);
        }

        Command::Current => {
            let mgr = open_manager()?;
            let task_with_deps = mgr.get_current_task()?;
            output::format_current(&task_with_deps);
        }

        Command::Depend { id, on_id } => {
            let mut mgr = open_manager()?;
            mgr.add_dependency(id, on_id)?;
            println!("Added dependency: #{} depends on #{}", id, on_id);
        }

        Command::Undepend { id, on_id } => {
            let mut mgr = open_manager()?;
            mgr.remove_dependency(id, on_id)?;
            println!(
                "Removed dependency: #{} no longer depends on #{}",
                id, on_id
            );
        }

        Command::Log { name, file } => {
            let mut mgr = open_manager()?;
            let artifact = mgr.log_artifact(&name, &file)?;
            println!(
                "Logged artifact: {} -> {}",
                artifact.name, artifact.file_path
            );
        }

        Command::Artifacts { task } => {
            let mgr = open_manager()?;
            let artifacts = mgr.get_artifacts(task)?;
            output::format_artifacts(&artifacts);
        }

        Command::Reorder { id, after, before } => {
            let mut mgr = open_manager()?;
            mgr.reorder_task(id, after, before)?;
            println!("Reordered task #{}", id);
        }

        Command::Reindex => {
            let mut mgr = open_manager()?;
            mgr.reindex()?;
            println!("Reindexed all task orders");
        }

        Command::Mcp => {
            return crate::run_mcp();
        }
    }

    Ok(())
}

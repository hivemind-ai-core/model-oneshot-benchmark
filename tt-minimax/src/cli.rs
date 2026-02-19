use crate::core::*;
use crate::db::{Artifact, Db, Task, TaskDetail, TaskStatus};
use crate::error::{Error, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;

/// tt — DAG-Based Task Tracker
#[derive(Parser)]
#[command(name = "tt")]
#[command(about = "A DAG-based task tracker for AI-driven development", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new task tracker in the current directory
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
        #[arg(long)]
        title: Option<String>,
        /// New description
        #[arg(long)]
        desc: Option<String>,
        /// New Definition of Done
        #[arg(long)]
        dod: Option<String>,
    },

    /// Show detailed information about a task
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
        /// Target task ID
        id: i64,
    },

    /// Get the next task to work on
    Next,

    /// Start working on a task
    Start {
        /// Task ID
        id: i64,
    },

    /// Stop the current task
    Stop,

    /// Complete the current task
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

    /// Show the current task
    Current,

    /// Add a dependency
    Depend {
        /// Task ID (the dependent)
        id: i64,
        /// Task ID to depend on (the prerequisite)
        on_id: i64,
    },

    /// Remove a dependency
    Undepend {
        /// Task ID (the dependent)
        id: i64,
        /// Task ID to stop depending on
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
        /// Task ID (if not specified, uses active task)
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

    /// Start the MCP server
    Mcp,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => {
            let db_path = PathBuf::from("tt.db");
            let artifacts_dir = PathBuf::from(".tt/artifacts");

            if db_path.exists() {
                eprintln!("Error: Already initialized in this directory");
                std::process::exit(1);
            }

            // Create .tt/artifacts directory
            fs::create_dir_all(&artifacts_dir).map_err(|e| Error::Io(e))?;

            // Create database
            let mut db = Db::open(&db_path)?;
            println!(
                "Initialized task tracker in {}",
                std::env::current_dir().unwrap().display()
            );
            Ok(())
        }

        Commands::Add {
            title,
            desc,
            dod,
            after,
            before,
        } => {
            let mut db = open_db()?;
            let task = add_task(&mut db, title, desc, dod, after, before)?;
            println!("Created task #{}", task.id);
            Ok(())
        }

        Commands::Edit {
            id,
            title,
            desc,
            dod,
        } => {
            let mut db = open_db()?;
            let task = edit_task(&mut db, id, title, desc, dod)?;
            println!("Updated task #{}", task.id);
            Ok(())
        }

        Commands::Show { id } => {
            let db = open_db()?;
            let detail = show_task(&db, id)?;
            print_task_detail(&detail);
            Ok(())
        }

        Commands::List { all } => {
            let db = open_db()?;
            let tasks = list_tasks(&db, None, all)?;

            if all {
                println!("All tasks:");
            } else {
                let target = get_current_target(&db)?;
                if let Some(tid) = target {
                    let target_task = crate::core::task::get_task(&db, tid)?;
                    println!("Target: #{} ({})", tid, target_task.title);
                }
            }

            for task in tasks {
                print_task_compact(&task, &db);
            }

            println!("\nLegend: ✓ completed  ● in_progress  ○ pending  ✗ blocked");
            Ok(())
        }

        Commands::Target { id } => {
            let mut db = open_db()?;
            set_target(&mut db, id)?;
            println!("Set target to #{}", id);
            Ok(())
        }

        Commands::Next => {
            let db = open_db()?;
            match get_next(&db, None) {
                Ok(task) => {
                    print_next_task(&task, &db)?;
                }
                Err(Error::TargetReached { id, title }) => {
                    println!(
                        "Target Reached: all tasks for #{} ({}) are completed.",
                        id, title
                    );
                }
                Err(Error::AllBlocked { blocked_ids }) => {
                    println!("All remaining tasks are blocked:");
                    for id in blocked_ids {
                        let task = crate::core::task::get_task(&db, id)?;
                        println!("  [#{}] ✗ {} — blocked", id, task.title);
                    }
                }
                Err(e) => return Err(e),
            }
            Ok(())
        }

        Commands::Start { id } => {
            let mut db = open_db()?;
            let task = start_task(&mut db, id)?;
            println!("Started task #{}: {}", task.id, task.title);
            Ok(())
        }

        Commands::Stop => {
            let mut db = open_db()?;
            let task = stop_task(&mut db)?;
            println!("Stopped task #{}: {}", task.id, task.title);
            Ok(())
        }

        Commands::Done => {
            let mut db = open_db()?;
            let task = complete_task(&mut db)?;
            println!("Completed task #{}: {}", task.id, task.title);
            Ok(())
        }

        Commands::Block { id } => {
            let mut db = open_db()?;
            let task = block_task(&mut db, id)?;
            println!("Blocked task #{}: {}", task.id, task.title);
            Ok(())
        }

        Commands::Unblock { id } => {
            let mut db = open_db()?;
            let task = unblock_task(&mut db, id)?;
            println!("Unblocked task #{}: {}", task.id, task.title);
            Ok(())
        }

        Commands::Current => {
            let db = open_db()?;
            let task = get_current_task(&db)?;
            let artifacts = get_artifacts_for_task(&db, task.id)?;

            println!("Active: [#{}] {}", task.id, task.title);
            println!("  Status:    {}", task.status.as_str());
            if let Some(started) = &task.started_at {
                println!("  Started:   {}", Task::format_datetime(started));
            }
            if let Some(dod) = &task.dod {
                println!("  DoD:       {}", dod);
            }

            if !artifacts.is_empty() {
                println!("  Artifacts:");
                for artifact in artifacts {
                    println!("    - {}: {}", artifact.name, artifact.file_path);
                }
            } else {
                println!("  Artifacts: (none)");
            }

            Ok(())
        }

        Commands::Depend { id, on_id } => {
            let mut db = open_db()?;
            add_dependency(&mut db, id, on_id)?;
            println!("Task #{} now depends on #{}", id, on_id);
            Ok(())
        }

        Commands::Undepend { id, on_id } => {
            let mut db = open_db()?;
            remove_dependency(&mut db, id, on_id)?;
            println!("Task #{} no longer depends on #{}", id, on_id);
            Ok(())
        }

        Commands::Log { name, file } => {
            let mut db = open_db()?;
            let artifact = log_artifact(&mut db, name, file)?;
            println!(
                "Logged artifact {} for task #{}: {}",
                artifact.name, artifact.task_id, artifact.file_path
            );
            Ok(())
        }

        Commands::Artifacts { task } => {
            let db = open_db()?;
            let artifacts = get_artifacts(&db, task)?;

            if artifacts.is_empty() {
                println!("No artifacts found.");
            } else {
                for artifact in artifacts {
                    println!(
                        "  [{}] {}: {}",
                        artifact.id, artifact.name, artifact.file_path
                    );
                }
            }

            Ok(())
        }

        Commands::Reorder { id, after, before } => {
            let mut db = open_db()?;
            let new_order = reorder_task(&mut db, id, after, before)?;
            println!("Reordered task #{} to order {}", id, new_order);
            Ok(())
        }

        Commands::Reindex => {
            let mut db = open_db()?;
            let count = reindex(&mut db)?;
            println!("Reindexed {} tasks", count);
            Ok(())
        }

        Commands::Mcp => {
            // MCP is handled in main.rs
            Ok(())
        }
    }
}

/// Open the database, handling the case where it doesn't exist
fn open_db() -> Result<Db> {
    let db_path = PathBuf::from("tt.db");
    if !db_path.exists() {
        eprintln!("Error: Not initialized. Run `tt init` first.");
        std::process::exit(1);
    }
    Db::open(&db_path)
}

/// Print a compact task listing
fn print_task_compact(task: &Task, db: &Db) {
    // Get dependencies
    let mut stmt = db
        .conn
        .prepare(
            "SELECT d.depends_on, t.status
         FROM dependencies d
         JOIN tasks t ON t.id = d.depends_on
         WHERE d.task_id = ?1",
        )
        .unwrap();

    let deps = stmt
        .query_map([task.id], |row| {
            let id: i64 = row.get(0)?;
            let status: String = row.get(1)?;
            Ok((id, status))
        })
        .unwrap()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap();

    let status_char = task.status.display_char();
    let deps_str = if deps.is_empty() {
        String::new()
    } else {
        let dep_list: Vec<String> = deps
            .iter()
            .map(|(id, status)| {
                let ch = match status.as_str() {
                    "completed" => "✓",
                    "in_progress" => "●",
                    "pending" => "○",
                    "blocked" => "✗",
                    _ => "?",
                };
                format!("{} {}", ch, id)
            })
            .collect();
        format!(" (deps: {})", dep_list.join(", "))
    };

    println!(
        "  [#{:2}] {} {}{}",
        task.id, status_char, task.title, deps_str
    );
}

/// Print detailed task information
fn print_task_detail(detail: &TaskDetail) {
    let task = &detail.task;
    println!("[#{}] {}", task.id, task.title);
    println!("Status:       {}", task.status.as_str());
    println!("Order:        {}", task.manual_order);
    println!("Created:      {}", Task::format_datetime(&task.created_at));

    if let Some(desc) = &task.description {
        println!("\nDescription:\n  {}", desc);
    }

    if let Some(dod) = &task.dod {
        println!("DoD:          {}", dod);
    }

    if let Some(started) = &task.started_at {
        println!("Started:      {}", Task::format_datetime(started));
    }

    if let Some(completed) = &task.completed_at {
        println!("Completed:    {}", Task::format_datetime(completed));
    }

    if !detail.dependencies.is_empty() {
        let deps: Vec<String> = detail
            .dependencies
            .iter()
            .map(|d| format!("{} ({})", d.id, d.status.display_char()))
            .collect();
        println!("Dependencies: {}", deps.join(", "));
    }

    if !detail.dependents.is_empty() {
        let dependents: Vec<String> = detail
            .dependents
            .iter()
            .map(|id| format!("#{}", id))
            .collect();
        println!("Dependents:   {}", dependents.join(", "));
    }

    if !detail.artifacts.is_empty() {
        println!("Artifacts:");
        for artifact in &detail.artifacts {
            println!("  - {}: {}", artifact.name, artifact.file_path);
        }
    } else {
        println!("Artifacts:    (none)");
    }
}

/// Print the next task to work on
fn print_next_task(task: &Task, db: &Db) -> Result<()> {
    println!("Next: [#{}] {}", task.id, task.title);

    // Show dependencies
    let mut stmt = db.conn.prepare(
        "SELECT d.depends_on, t.status
         FROM dependencies d
         JOIN tasks t ON t.id = d.depends_on
         WHERE d.task_id = ?1",
    )?;

    let deps: Vec<(i64, String)> = stmt
        .query_map([task.id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    if !deps.is_empty() {
        let all_completed = deps.iter().all(|(_, status)| status == "completed");
        let status_str = if all_completed {
            "all met".to_string()
        } else {
            let unmet: Vec<String> = deps
                .iter()
                .filter(|(_, s)| s != "completed")
                .map(|(id, _)| format!("#{}", id))
                .collect();
            format!("unmet: {}", unmet.join(", "))
        };

        println!("  Dependencies: {}", status_str);
    }

    if let Some(dod) = &task.dod {
        println!("  DoD: {}", dod);
    }

    Ok(())
}

/// Check if tt is initialized
pub fn is_initialized() -> bool {
    PathBuf::from("tt.db").exists()
}

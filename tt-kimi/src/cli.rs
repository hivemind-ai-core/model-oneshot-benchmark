use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "tt")]
#[command(about = "DAG-Based Task Tracker")]
#[command(version = "0.1.0")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new task tracker project
    Init,

    /// Add a new task
    Add {
        /// Task title
        title: String,
        /// Optional description
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

    /// Edit an existing task
    Edit {
        /// Task ID
        id: i64,
        /// New title
        #[arg(long)]
        title: Option<String>,
        /// New description
        #[arg(long)]
        desc: Option<String>,
        /// Clear description
        #[arg(long)]
        no_desc: bool,
        /// New DoD
        #[arg(long)]
        dod: Option<String>,
        /// Clear DoD
        #[arg(long)]
        no_dod: bool,
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

    /// Set or view the target task
    Target {
        /// Task ID (omit to view current target)
        id: Option<i64>,
    },

    /// Get the next task to work on
    Next,

    /// Start working on a task
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

    /// Show the current active task
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
        /// Task ID
        id: i64,
        /// Task ID to remove dependency on
        on_id: i64,
    },

    /// Log an artifact for the current task
    Log {
        /// Artifact name (e.g., "research", "plan")
        name: String,
        /// Path to the artifact file
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
        /// Insert after this task ID
        #[arg(long)]
        after: Option<i64>,
        /// Insert before this task ID
        #[arg(long)]
        before: Option<i64>,
    },

    /// Reindex all task orders
    Reindex,

    /// Start MCP server
    Mcp,
}

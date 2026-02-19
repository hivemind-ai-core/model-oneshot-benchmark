use clap::Parser;
use std::process;
use tt::cli::{Cli, Commands};
use tt::cli_handlers;
use tt::mcp::run_mcp_server;

#[tokio::main]
async fn main() {
    // Initialize tracing for MCP
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init => cli_handlers::handle_init(),
        Commands::Add {
            title,
            desc,
            dod,
            after,
            before,
        } => cli_handlers::handle_add(&title, desc.as_deref(), dod.as_deref(), after, before),
        Commands::Edit {
            id,
            title,
            desc,
            no_desc,
            dod,
            no_dod,
        } => cli_handlers::handle_edit(
            id,
            title.as_deref(),
            desc.as_deref(),
            no_desc,
            dod.as_deref(),
            no_dod,
        ),
        Commands::Show { id } => cli_handlers::handle_show(id),
        Commands::List { all } => cli_handlers::handle_list(all),
        Commands::Target { id } => cli_handlers::handle_target(id),
        Commands::Next => cli_handlers::handle_next(),
        Commands::Start { id } => cli_handlers::handle_start(id),
        Commands::Stop => cli_handlers::handle_stop(),
        Commands::Done => cli_handlers::handle_done(),
        Commands::Block { id } => cli_handlers::handle_block(id),
        Commands::Unblock { id } => cli_handlers::handle_unblock(id),
        Commands::Current => cli_handlers::handle_current(),
        Commands::Depend { id, on_id } => cli_handlers::handle_depend(id, on_id),
        Commands::Undepend { id, on_id } => cli_handlers::handle_undepend(id, on_id),
        Commands::Log { name, file } => cli_handlers::handle_log(&name, &file),
        Commands::Artifacts { task } => cli_handlers::handle_artifacts(task),
        Commands::Reorder { id, after, before } => cli_handlers::handle_reorder(id, after, before),
        Commands::Reindex => cli_handlers::handle_reindex(),
        Commands::Mcp => {
            if let Err(e) = run_mcp_server().await {
                eprintln!("MCP server error: {e}");
                process::exit(1);
            }
            return;
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}

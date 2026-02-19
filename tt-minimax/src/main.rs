mod cli;
mod core;
mod db;
mod error;
mod graph;
mod mcp;

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    // Check for MCP mode
    if args.len() > 1 && args[1] == "mcp" {
        // Run MCP server
        if let Err(e) = mcp::run_mcp().await {
            eprintln!("Error: {}", e);
            return ExitCode::from(1);
        }
        return ExitCode::SUCCESS;
    }

    // Run CLI
    if let Err(e) = cli::run() {
        eprintln!("Error: {e}");
        return ExitCode::from(1);
    }

    ExitCode::SUCCESS
}

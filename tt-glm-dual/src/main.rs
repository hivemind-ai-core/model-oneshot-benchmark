//! tt - DAG-based task tracker
//!
//! A single-binary CLI tool and MCP server for managing tasks as nodes in a DAG.

fn main() {
    if let Err(e) = tt::cli::run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

//! tt - DAG-based task tracker library.
//!
//! This library contains the core business logic for the tt task tracker.

pub mod cli;
pub mod core;

// MCP server module (private to library, used by main)
mod mcp_impl;
pub use mcp_impl::run_mcp;

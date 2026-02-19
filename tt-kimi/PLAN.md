# Implementation Plan for `tt` - Task Tracker

## Overview
A DAG-based task tracker CLI and MCP server in Rust, using SQLite for persistence.

## Architecture

```
src/
├── main.rs           # CLI entry point
├── lib.rs            # Library exports
├── error.rs          # Error enum definitions
├── models.rs         # Data structures (Task, Artifact, etc.)
├── db.rs             # Database connection and initialization
├── graph.rs          # Graph algorithms (topological sort, cycle detection)
├── core.rs           # Business logic layer
├── cli.rs            # CLI argument parsing (clap)
├── cli_handlers.rs   # CLI command implementations
└── mcp.rs            # MCP server implementation
```

## Phase 1: Foundation
1. **Setup**: Initialize Cargo project with dependencies
   - clap v4 (derive feature)
   - rusqlite (bundled feature)
   - chrono (serde feature)
   - serde, serde_json
   - thiserror
   - rmcp (for MCP server)

2. **Error Types** (`error.rs`)
   - Define `TaskError` enum with all variants from Section 15
   - Implement `From<rusqlite::Error>` and `From<std::io::Error>`

3. **Models** (`models.rs`)
   - `Task` struct with all fields
   - `Artifact` struct
   - `Status` enum (pending, in_progress, completed, blocked)
   - Serialization support for MCP responses

4. **Database Layer** (`db.rs`)
   - Initialize connection with WAL mode and foreign keys
   - Schema creation (tasks, dependencies, artifacts, config tables)
   - Migration/index setup

## Phase 2: Core Logic
1. **Graph Engine** (`graph.rs`)
   - Kahn's algorithm with min-heap for topological sort
   - Cycle detection using DFS
   - Order conflict detection
   - Target subgraph computation via recursive CTE

2. **Core Operations** (`core.rs`)
   - Task CRUD operations
   - Dependency management (with cycle check)
   - State transitions (start, stop, done, block, unblock)
   - Ordering operations (reorder, reindex)
   - Artifact logging
   - Target management

## Phase 3: CLI Interface
1. **Argument Parsing** (`cli.rs`)
   - All commands from Section 11
   - Proper flags and options

2. **CLI Handlers** (`cli_handlers.rs`)
   - Format human-readable output
   - Handle errors gracefully
   - Colorized status indicators

## Phase 4: MCP Server
1. **MCP Implementation** (`mcp.rs`)
   - stdio transport
   - Tool registration for all operations
   - JSON response formatting

## Phase 5: Testing
1. **Unit Tests**
   - Graph algorithm tests
   - State machine tests
   - Ordering calculation tests

2. **Integration Tests**
   - Full CLI workflow tests
   - MCP server tests

## Implementation Order
1. Create project skeleton and dependencies
2. Implement error types and models
3. Implement database layer
4. Implement graph engine with tests
5. Implement core operations with tests
6. Implement CLI handlers
7. Implement MCP server
8. Integration tests
9. Refactoring and optimization

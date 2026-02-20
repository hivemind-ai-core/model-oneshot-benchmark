# Plan: tt — DAG-Based Task Tracker

## Phase 1: Project Setup
1. Create Rust project with `cargo new tt`
2. Add dependencies to Cargo.toml:
   - rusqlite (with bundled feature)
   - clap v4 with derive macros
   - thiserror
   - chrono with serde
   - serde and serde_json
   - clap-mcp (if mature enough) or manual MCP implementation
3. Set up logging and error handling structure

## Phase 2: Database Layer
1. Implement schema in `src/db/mod.rs`
2. Create tables: tasks, dependencies, artifacts, config
3. Add indexes and constraints
4. Implement CRUD operations for each table
5. WAL mode and foreign keys setup

## Phase 3: Core Business Logic
1. Task management (create, update, status transitions)
2. Dependency management (add, remove, cycle detection)
3. Topological sorting with Kahn's algorithm
4. Target system with recursive CTE
5. Ordering system with manual_order float
6. Artifact management

## Phase 4: CLI Interface
1. Implement all commands from spec:
   - init, add, edit, show, list
   - target, next, start, stop, done, block, unblock, current
   - depend, undepend
   - log, artifacts
   - reorder, reindex
   - mcp
2. Format output per spec

## Phase 5: MCP Server
1. Implement stdio transport
2. Expose all CLI commands as MCP tools
3. JSON-RPC handling

## Phase 6: Testing & Refinement
1. Unit tests for core logic
2. Integration tests for CLI
3. Edge case handling
4. Format and lint checks

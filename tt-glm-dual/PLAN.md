# Implementation Plan for `tt` - DAG-Based Task Tracker

## Project Overview
A single-binary CLI tool and MCP server in Rust that manages tasks as nodes in a DAG stored in SQLite.

## Phase 1: Project Setup and Foundation

### 1.1 Project Initialization
- Create Rust project with `cargo init`
- Add required dependencies to `Cargo.toml`:
  - `clap` v4 with derive feature
  - `rusqlite` with bundled feature
  - `thiserror`
  - `chrono` with serde feature
  - `serde` and `serde_json`
  - MCP library (will evaluate `clap-mcp`, `rmcp`, or manual implementation)

### 1.2 Directory Structure
```
src/
├── main.rs          # CLI entry point
├── mcp.rs           # MCP server entry point
├── core/
│   ├── mod.rs       # Core module exports
│   ├── db.rs        # Database layer, schema, migrations
│   ├── graph.rs     # Graph operations (sort, cycle detection)
│   ├── task.rs      # Task operations (add, edit, show, etc)
│   └── error.rs     # Error types with thiserror
└── cli/
    ├── mod.rs       # CLI module exports
    └── output.rs    # Output formatting

tests/
├── integration.rs   # End-to-end CLI tests
```

### 1.3 Error Types (`src/core/error.rs`)
Define all error variants from SPEC.md section 15:
- TaskNotFound
- TaskNotPending
- AnotherTaskActive
- NoActiveTask
- UnmetDependencies
- CycleDetected
- NoTarget
- TargetReached
- NoDod
- OrderConflict (warning)
- InvalidStatus
- AllBlocked
- Db (rusqlite::Error)
- Io (std::io::Error)

## Phase 2: Database Layer

### 2.1 Schema (`src/core/db.rs`)
Create tables:
- `tasks`: id, title, description, dod, status, manual_order, created_at, started_at, completed_at, last_touched_at
- `dependencies`: task_id, depends_on (composite PK, CHECK task_id != depends_on)
- `artifacts`: id, task_id, name, file_path, created_at
- `config`: key, value (for target_id)

### 2.2 Database Operations
- `init_db()` - Create database and tables, enable WAL mode, foreign keys
- `create_task()`
- `get_task()`
- `update_task()`
- `update_task_status()`
- `add_dependency()`
- `remove_dependency()`
- `add_artifact()`
- `get_artifacts()`
- `get_active_task()`
- `set_config()` / `get_config()`

## Phase 3: Graph Engine

### 3.1 Topological Sort (`src/core/graph.rs`)
Implement Kahn's algorithm with priority queue:
- Build in-degree map and adjacency list
- Use min-heap (with reversed Ord on manual_order)
- Return sorted task list

### 3.2 Target Subgraph
- Recursive CTE to get transitive dependencies
- Filter out completed tasks
- Return "active subgraph"

### 3.3 Cycle Detection
- DFS from potential dependency
- Return cycle path if detected

### 3.4 Next Task Selection
- First task in sorted order where:
  - Status is pending
  - All direct dependencies are completed
- Return error if all remaining are blocked

## Phase 4: Task Operations (`src/core/task.rs`)

### 4.1 Task Management
- `add_task(title, description, dod, after_id, before_id)`
- `edit_task(id, title, description, dod)`
- `show_task(id)` - with dependencies and dependents
- `list_tasks(all)` - topological sort within target subgraph

### 4.2 Workflow Operations
- `start_task(id)` - enforce single active task, check dependencies
- `stop_task()` - move active back to pending
- `complete_task()` - check DoD
- `block_task(id)`
- `unblock_task(id)`
- `get_current_task()`
- `set_target(id)`

### 4.3 Dependency Management
- `add_dependency(id, depends_on)` - with cycle detection
- `remove_dependency(id, depends_on)`

### 4.4 Artifact Management
- `log_artifact(name, file_path)`
- `get_artifacts(task_id)`

### 4.5 Ordering
- `reorder_task(id, after_id, before_id)`
- `reindex_all()` - reassign to 10, 20, 30...

## Phase 5: CLI Interface (`src/main.rs`)

### 5.1 Command Structure
Using clap derive:
- Subcommands: init, add, edit, show, list, target, next, start, stop, done, block, unblock, current, depend, undepend, log, artifacts, reorder, reindex, mcp

### 5.2 Output Formatting (`src/cli/output.rs`)
- `format_list()` - with status indicators and dependency summary
- `format_show()` - full task detail
- `format_next()` - next task with dependency status
- `format_current()` - active task with artifacts
- `format_error()` - clear error messages

## Phase 6: MCP Server (`src/mcp.rs`)

### 6.1 Implementation
Evaluate and choose:
1. `clap-mcp` if mature enough
2. `rmcp` (official SDK)
3. Manual stdio JSON-RPC implementation

### 6.2 Tool Exposures
Map CLI commands to MCP tools with JSON responses

## Phase 7: Testing (TDD)

### 7.1 Unit Tests
- Graph logic (sort, cycle detection)
- State machine transitions
- Target walk
- Order calculations

### 7.2 Integration Tests
- Full CLI workflow as specified in SPEC.md section 17.4

## Phase 8: Verification and Refactoring

### 8.1 Verification
- All tests pass
- No type errors or warnings
- No format errors (`cargo fmt --check`)
- Clippy clean (`cargo clippy`)
- Manual testing of compiled binary

### 8.2 Refactoring
- Clean up code
- Ensure performance
- Re-run verification

## Implementation Order

1. **Foundation** - Project setup, error types
2. **Database** - Schema and basic CRUD
3. **Graph Engine** - Sort, cycle detection, target subgraph
4. **Task Operations** - All business logic
5. **CLI** - Command interface and output formatting
6. **MCP** - Server implementation
7. **Testing** - Comprehensive test coverage
8. **Verification** - Final checks and manual testing

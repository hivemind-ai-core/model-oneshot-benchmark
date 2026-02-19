# Implementation Plan for `tt` - DAG-Based Task Tracker

## Project Overview

A single-binary CLI tool and MCP server for managing tasks as nodes in a Directed Acyclic Graph (DAG), stored in SQLite.

## Dependencies (Latest Versions)

```toml
[dependencies]
clap = { version = "4.5", features = ["derive"] }
rusqlite = { version = "0.38", features = ["bundled"] }
thiserror = "2.0"
chrono = { version = "0.4", features = ["serde"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
rmcp = "0.15"
```

## Project Structure

```
tt-minimax/
├── Cargo.toml
├── SPEC.md
├── PLAN.md
└── src/
    ├── main.rs              # CLI entry point
    ├── cli.rs               # CLI command definitions
    ├── mcp.rs               # MCP server implementation
    ├── core/
    │   ├── mod.rs           # Core business logic
    │   ├── task.rs          # Task operations
    │   ├── workflow.rs      # State transitions (start, stop, done, etc.)
    │   └── target.rs        # Target system
    ├── db/
    │   ├── mod.rs           # Database layer
    │   ├── schema.rs        # Database schema setup
    │   └── models.rs        # Data models
    ├── graph/
    │   ├── mod.rs           # Graph algorithms
    │   ├── cycle.rs         # Cycle detection
    │   └── sort.rs          # Topological sort
    └── error.rs             # Error types
```

## Implementation Phases (TDD)

### Phase 1: Project Setup & Database Schema
1. Initialize Cargo project
2. Add dependencies
3. Create module structure
4. Define database schema
5. Tests: Database initialization, table creation

### Phase 2: Data Models & Error Types
1. Define `Task` struct with `Status` enum
2. Define `Dependency`, `Artifact`, `Config` structs
3. Define `Error` enum with all variants
4. Tests: Model serialization, error display

### Phase 3: Database Layer
1. Implement database connection management
2. Implement CRUD operations for tasks
3. Implement dependency management
4. Implement artifact management
5. Implement config key-value store
6. Tests: All database operations

### Phase 4: Graph Algorithms
1. Implement DFS-based cycle detection
2. Implement Kahn's algorithm with priority queue
3. Implement recursive CTE for target walk
4. Tests: Linear chain, diamond graph, cycles, manual ordering

### Phase 5: Core Business Logic - Task Management
1. Implement `add_task` with manual ordering
2. Implement `edit_task`
3. Implement `show_task`
4. Implement `list_tasks`
5. Tests: All task management operations

### Phase 6: Core Business Logic - Workflow
1. Implement `start_task` (with guards)
2. Implement `stop_task`
3. Implement `done_task` (with DoD check)
4. Implement `block_task` / `unblock_task`
5. Implement `get_next_task`
6. Implement `get_current_task`
7. Tests: All state transitions, guards, edge cases

### Phase 7: Core Business Logic - Dependencies & Ordering
1. Implement `add_dependency` (with cycle detection)
2. Implement `remove_dependency`
3. Implement `reorder_task`
4. Implement `reindex`
5. Tests: Dependency operations, cycle detection, ordering

### Phase 8: Core Business Logic - Target & Artifacts
1. Implement `set_target`
2. Implement `get_target_subgraph`
3. Implement `log_artifact`
4. Implement `get_artifacts`
5. Tests: Target walk, artifact linking

### Phase 9: CLI Implementation
1. Implement all CLI commands using clap derive
2. Implement human-readable output formatting
3. Implement error output to stderr
4. Tests: CLI integration tests using subprocess

### Phase 10: MCP Server
1. Implement stdio transport
2. Register all tools with descriptions
3. Implement JSON request/response handling
4. Tests: MCP protocol compliance

## Invariants to Enforce

1. **Single active task** - Only one task can be `in_progress` at a time
2. **Dependencies gate starting** - All deps must be `completed` to start
3. **No cycles** - Reject dependency edges that create cycles
4. **DoD required for completion** - Must have non-empty DoD to complete
5. **Topological correctness** - Output must respect dependency order
6. **No deletion** - v1 does not support deleting tasks
7. **Mutation updates timestamp** - Every mutation updates `last_touched_at`

## Database Schema

```sql
CREATE TABLE tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    description TEXT,
    dod TEXT,
    status TEXT NOT NULL CHECK(status IN ('pending', 'in_progress', 'completed', 'blocked')),
    manual_order REAL NOT NULL DEFAULT 10.0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now')),
    started_at TEXT,
    completed_at TEXT,
    last_touched_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now'))
);

CREATE TABLE dependencies (
    task_id INTEGER NOT NULL,
    depends_on INTEGER NOT NULL,
    PRIMARY KEY (task_id, depends_on),
    CHECK (task_id != depends_on),
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE,
    FOREIGN KEY (depends_on) REFERENCES tasks(id) ON DELETE CASCADE
);

CREATE TABLE artifacts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    file_path TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now')),
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE CASCADE
);

CREATE TABLE config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE INDEX idx_tasks_status ON tasks(status);
CREATE INDEX idx_tasks_manual_order ON tasks(manual_order);
CREATE INDEX idx_dependencies_task ON dependencies(task_id);
CREATE INDEX idx_dependencies_depends_on ON dependencies(depends_on);
CREATE INDEX idx_artifacts_task ON artifacts(task_id);
```

## Testing Strategy

1. **Unit Tests**: For graph algorithms, model serialization
2. **Integration Tests**: For database operations, workflow
3. **CLI Tests**: Using `std::process::Command` to test CLI output
4. **In-Memory SQLite**: For fast tests using `:memory:` database

## MCP Tools to Expose

All CLI commands except `init`, `mcp`, `reindex` should be exposed as MCP tools with structured JSON responses.

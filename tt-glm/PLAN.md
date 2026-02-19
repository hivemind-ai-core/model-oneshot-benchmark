# Implementation Plan for `tt` — DAG-Based Task Tracker

## Overview
This plan breaks down the implementation of a Rust-based CLI tool and MCP server for managing tasks in a DAG structure stored in SQLite.

## Phase 1: Project Setup & Infrastructure

### 1.1 Cargo Project Initialization
- Create Cargo.toml with dependencies
- Set up binary targets (`tt` CLI, optional separate MCP binary or single binary with subcommands)
- Configure workspace structure

### 1.2 Dependencies (latest versions as of implementation)
- `rusqlite` with `bundled` feature
- `clap` v4 with derive features
- `thiserror` for error handling
- `chrono` with `serde` feature
- `serde` and `serde_json`
- `rmcp` (Rust MCP SDK) - evaluate available options

### 1.3 Project Structure
```
src/
├── main.rs              # CLI entry point
├── mcp.rs               # MCP server entry
├── lib.rs               # Library exports
├── db/
│   ├── mod.rs
│   ├── schema.rs        # Database schema & migrations
│   └── connection.rs    # SQLite connection management
├── core/
│   ├── mod.rs
│   ├── task.rs          # Task model & operations
│   ├── dependency.rs    # Dependency graph logic
│   ├── artifact.rs      # Artifact management
│   └── config.rs        # Config/target management
├── graph/
│   ├── mod.rs
│   ├── topology.rs      # Kahn's algorithm implementation
│   ├── cycle.rs         # Cycle detection
│   └── order.rs         # Manual order management
├── cli/
│   ├── mod.rs
│   └── commands.rs      # CLI command handlers
└── mcp_server/
    ├── mod.rs
    └── tools.rs         # MCP tool implementations
```

## Phase 2: Database Layer (TDD)

### 2.1 Schema Implementation
- Tasks table with all columns
- Dependencies table with composite PK
- Artifacts table
- Config table
- All indexes and CHECK constraints

### 2.2 Connection Management
- Open/create tt.db in current directory
- Enable WAL mode
- Enable foreign keys
- Transaction helper methods

### 2.3 Tests for Database
- Test schema creation
- Test constraint enforcement (status values, no self-dependency)
- Test foreign key relationships

## Phase 3: Core Models (TDD)

### 3.1 Task Model
- Struct representing a task
- Status enum (pending, in_progress, completed, blocked)
- CRUD operations
- Last_touched_at auto-update

### 3.2 Artifact Model
- Struct for artifacts
- Link/unlink operations
- Query by task

### 3.3 Config Management
- Get/set target_id
- Key-value operations

### 3.4 Tests for Models
- Create/read/update tasks
- Artifact linking
- Config CRUD

## Phase 4: Graph Engine (TDD)

### 4.1 Topological Sort
- Kahn's algorithm implementation
- Manual order as secondary sort key
- Min-heap using BinaryHeap with reversed Ord

### 4.2 Cycle Detection
- DFS from dependency to check for cycles
- Return full cycle path on detection

### 4.3 Target Subgraph
- Recursive CTE to find transitive dependencies
- Filter out completed tasks

### 4.4 Manual Order System
- Midpoint calculation
- Float precision exhaustion detection
- Reindex operation

### 4.5 Tests for Graph
- Linear chain sorting
- Diamond pattern sorting
- Cycle detection scenarios
- Manual order tiebreaking
- Target subgraph computation
- Order conflict detection (warnings)

## Phase 5: Business Logic (TDD)

### 5.1 State Machine
- `start`: pending → in_progress
- `stop`: in_progress → pending
- `done`: in_progress → completed
- `block`: pending/in_progress → blocked
- `unblock`: blocked → pending

### 5.2 Invariants Enforcement
1. Single active task at a time
2. Dependencies gate starting
3. No cycles (from graph layer)
4. DoD required for completion
5. Topological correctness
6. No deletion (v1)
7. last_touched_at updates

### 5.3 Task Operations
- Add task with positioning
- Edit task
- Add/remove dependencies
- Reorder tasks

### 5.4 Tests for Business Logic
- All status transitions
- All invariant violations
- Edge cases (idempotent start, blocking active task)

## Phase 6: CLI Implementation

### 6.1 Command Structure
```rust
#[derive(Subcommand)]
enum Commands {
    Init,
    Add { title: String, ... },
    Edit { id: u32, ... },
    Show { id: u32 },
    List { all: bool },
    Target { id: u32 },
    Next,
    Start { id: u32 },
    Stop,
    Done,
    Block { id: u32 },
    Unblock { id: u32 },
    Current,
    Depend { id: u32, on_id: u32 },
    Undepend { id: u32, on_id: u32 },
    Log { name: String, file: String },
    Artifacts { task: Option<u32> },
    Reorder { id: u32, ... },
    Reindex,
    Mcp,
}
```

### 6.2 Output Formatting
- Human-readable text with status indicators
- Error messages to stderr
- Proper exit codes

### 6.3 Commands Implementation
- Map each CLI command to core logic
- Format results per spec Section 12

## Phase 7: MCP Server

### 7.1 MCP Transport
- Evaluate and choose MCP library
- Set up stdio transport

### 7.2 Tool Registration
- Map all CLI commands (except init, mcp, reindex) to MCP tools
- Define JSON schemas for inputs

### 7.3 Response Format
- Structured JSON responses
- Error codes mapping

### 7.4 Tool Descriptions
- Write AI-friendly descriptions
- Explain when to use each tool

## Phase 8: Testing & Validation

### 8.1 Unit Tests
- Database layer
- Graph algorithms
- State machine transitions

### 8.2 Integration Tests
- CLI command tests
- Full workflow scenarios

### 8.3 End-to-End Tests
- Per spec Section 17.4: 15-step CLI integration test

### 8.4 Manual Testing
- Compile and run all commands
- Verify output formatting
- Test MCP server with an AI client

## Phase 9: Code Quality & Refactoring

### 9.1 Linting
- Run `cargo clippy` - fix all warnings
- Zero compiler warnings

### 9.2 Formatting
- Run `cargo fmt` - consistent code style

### 9.3 Refactoring
- Extract common patterns
- Improve error messages
- Performance optimization if needed

### 9.4 Documentation
- Add module documentation
- Add inline documentation for public APIs

## Phase 10: Final Verification

### 10.1 All Tests Pass
```bash
cargo test --all-features
```

### 10.2 No Warnings
```bash
cargo clippy -- -D warnings
cargo build
```

### 10.3 Formatted
```bash
cargo fmt --check
```

### 10.4 Manual Smoke Test
- Initialize a new project
- Create tasks with dependencies
- Set target and run through workflow
- Verify all commands work

## Implementation Order Priority

1. **Database Layer** - Foundation for everything
2. **Core Models** - Basic CRUD
3. **Graph Engine** - Topological sort and cycle detection
4. **Business Logic** - State machine and invariants
5. **CLI** - Human interface
6. **MCP** - AI interface
7. **Testing** - Continuous TDD throughout
8. **Quality** - Clippy, fmt, refactor

## Notes

- Follow TDD: write failing test first, then implement
- Use latest versions of all dependencies
- Single binary with subcommands approach
- Error messages should be clear and actionable
- All invariants must be enforced
- Manual ordering is secondary to topological ordering

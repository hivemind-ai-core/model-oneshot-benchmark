# tt - DAG-Based Task Tracker Implementation Plan

## Overview
A Rust CLI tool and MCP server for managing tasks as nodes in a Directed Acyclic Graph (DAG), stored in SQLite.

## Architecture

```
src/
├── main.rs          # CLI entry point
├── lib.rs           # Library exports
├── mcp.rs           # MCP server implementation (manual JSON-RPC)
├── cli.rs           # CLI commands (clap)
├── core/
│   ├── mod.rs       # Core module exports
│   ├── db.rs        # Database layer (rusqlite)
│   ├── graph.rs     # Graph algorithms (Kahn's, cycle detection)
│   ├── models.rs    # Data structures (Task, Artifact, etc.)
│   └── error.rs     # Error types (thiserror)
└── tests/
    └── integration_test.rs  # End-to-end CLI tests
```

## Implementation Status: ✅ COMPLETE

### Phase 1: Project Setup & Dependencies ✅
- Created Cargo.toml with latest stable dependencies
- Set up project structure

### Phase 2: Core Data Models ✅
- Task struct with all required fields
- TaskStatus enum with proper serialization
- Artifact struct
- Dependency struct
- OrderConflict detection

### Phase 3: Database Schema ✅
- SQLite with WAL mode and foreign keys
- Tables: tasks, dependencies, artifacts, config
- All required indexes and constraints
- Proper datetime handling with chrono

### Phase 4: Error Handling ✅
- Complete error enum with thiserror
- All required error variants per spec
- Proper error messages

### Phase 5: Graph Engine ✅
- Kahn's Algorithm with priority queue (min-heap)
- Cycle detection using DFS
- Order conflict detection
- Midpoint calculation for manual ordering
- Reindex functionality

### Phase 6: Core Operations ✅
- Task CRUD (add, edit, get, list)
- Status transitions (start, stop, done, block, unblock)
- Dependency management
- Ordering (reorder, reindex)
- Artifacts (log, get)
- Target system
- Next task computation

### Phase 7: CLI Implementation ✅
- All commands per spec section 11
- Human-readable output with status indicators
- Proper error handling and exit codes

### Phase 8: MCP Server Implementation ✅
- Manual JSON-RPC implementation over stdio
- All tools exposed per spec section 13
- Proper request/response handling

### Phase 9: Testing ✅
- Unit tests for graph algorithms
- Unit tests for database operations
- Unit tests for core operations
- Integration test for full CLI workflow
- All tests passing

## Verification Checklist: ✅ ALL PASS

- [x] All unit tests pass (21 tests)
- [x] Integration test passes (1 test)
- [x] No compiler errors
- [x] `cargo fmt` clean
- [x] `cargo clippy` clean (only one expected warning about jsonrpc field)
- [x] CLI binary works end-to-end
- [x] MCP server starts correctly
- [x] Dependencies use latest versions:
  - clap 4.5
  - rusqlite 0.34
  - thiserror 2.0
  - chrono 0.4
  - serde 1.0
  - serde_json 1.0

## End-to-End Test Results

The full scenario from spec section 17.4 passes:
1. `tt init` → creates `tt.db` and `.tt/artifacts/` ✅
2. `tt add "Task A"` → prints ID 1 ✅
3. `tt add "Task B"` → prints ID 2 ✅
4. `tt depend 2 1` → success ✅
5. `tt target 2` → success ✅
6. `tt next` → returns Task 1 ✅
7. `tt start 1` → success ✅
8. `tt done` → error (no DoD) ✅
9. `tt edit 1 --dod "Schema exists"` → success ✅
10. `tt done` → success ✅
11. `tt next` → returns Task 2 ✅
12. `tt edit 2 --dod "Feature works"` → success ✅
13. `tt start 2` → success ✅
14. `tt done` → success ✅
15. `tt next` → "Target Reached" ✅

## Notes

- The MCP implementation uses manual JSON-RPC over stdio as the rmcp crate API changed significantly between versions
- The `jsonrpc` field in JsonRpcRequest is kept for proper deserialization despite the dead_code warning
- Manual ordering uses f64 with NaN handling for robustness
- Topological sort uses Kahn's algorithm with a min-heap for manual order tiebreaking

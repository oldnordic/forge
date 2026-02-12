# Phase 1 Task Breakdown

**Phase**: 01 - Core SDK Foundation
**Status**: 📋 Planned
**Created**: 2026-02-12

---

## Task Summary

| ID | Task | Priority | Complexity | Dependencies | Est. Days |
|-----|-------|------------|----------------|------------|
| 01-01 | Storage Layer Implementation | P0 | Medium | None | 2-3 |
| 01-02 | Graph Module Implementation | P0 | High | 01-01 | 3-4 |
| 01-03 | Search Module Implementation | P0 | Medium | 01-01 | 2-3 |
| 01-04 | CFG Module Implementation | P1 | High | 01-01 | 3-4 |
| 01-05 | Edit Module Implementation | P0 | Very High | 01-01, 01-02 | 4-5 |
| 01-06 | Test Infrastructure | P0 | Low | None | 1 |
| 01-07 | Analysis Module Implementation | P1 | Low | 01-02, 01-04, 01-05 | 1-2 |
| 01-08 | Documentation Completion | P1 | Low | All impl tasks | 2-3 |

**Total Estimated Duration**: 13-17 working days (3-4 weeks)

---

## Task 01-01: Storage Layer Implementation

**Status**: Pending
**Assigned**: Unassigned
**Priority**: P0 (Must Have)

### Subtasks

- [ ] 01-01-01: Add `sqlitegraph::GraphBackend` field to `UnifiedGraphStore`
- [ ] 01-01-02: Implement `async fn query_symbols()` method
- [ ] 01-01-03: Implement `async fn query_references()` method
- [ ] 01-01-04: Implement `async fn query_cfg()` method
- [ ] 01-01-05: Implement transaction management methods
- [ ] 01-01-06: Add error conversion from SQLite to `ForgeError`
- [ ] 01-01-07: Write unit tests (minimum 3 tests)

### Success Criteria
- Direct SQL queries work on symbols table
- Direct SQL queries work on references table
- Transactions begin, commit, and rollback correctly
- All unit tests pass

---

## Task 01-02: Graph Module Implementation

**Status**: Pending
**Assigned**: Unassigned
**Priority**: P0 (Must Have)

### Subtasks

- [ ] 01-02-01: Implement `find_symbol()` with SQL query
- [ ] 01-02-02: Implement `find_symbol_by_id()` with SQL query
- [ ] 01-02-03: Implement `callers_of()` filtering for Call references
- [ ] 01-02-04: Implement `references()` returning all reference types
- [ ] 01-02-05: Implement `reachable_from()` with recursive query
- [ ] 01-02-06: Implement `cycles()` with DFS algorithm
- [ ] 01-02-07: Write unit tests (minimum 6 tests)
- [ ] 01-02-08: (Optional) Split into `graph/query.rs` submodule if >300 LOC

### Success Criteria
- All methods return real data from SQLiteGraph
- Callers are correctly filtered to Call kind only
- Reachability computes transitive closure
- Cycle detection finds actual call graph cycles
- All unit tests pass

---

## Task 01-03: Search Module Implementation

**Status**: Pending
**Assigned**: Unassigned
**Priority**: P0 (Must Have)

### Subtasks

- [ ] 01-03-01: Implement `SearchBuilder::execute()` with SQL generation
- [ ] 01-03-02: Add LIKE filtering for name pattern matching
- [ ] 01-03-03: Add equality filtering for SymbolKind
- [ ] 01-03-04: Add prefix filtering for file path
- [ ] 01-03-05: Add LIMIT clause support
- [ ] 01-03-06: Ensure multiple filters combine with AND logic
- [ ] 01-03-07: Write unit tests (minimum 5 tests)
- [ ] 01-03-08: (Optional) Implement `pattern()` for AST search

### Success Criteria
- Execute returns filtered results from database
- All filters work independently
- Multiple filters combine correctly
- Limit caps result count
- All unit tests pass

---

## Task 01-04: CFG Module Implementation

**Status**: Pending
**Assigned**: Unassigned
**Priority**: P1 (Should Have)

### Subtasks

- [ ] 01-04-01: Implement `PathBuilder::execute()` with path enumeration
- [ ] 01-04-02: Add path kind classification (normal/error/degenerate)
- [ ] 01-04-03: Implement `normal_only()` filtering logic
- [ ] 01-04-04: Implement `error_only()` filtering logic
- [ ] 01-04-05: Implement `max_length()` path truncation
- [ ] 01-04-06: Implement `dominators()` with iterative algorithm
- [ ] 01-04-07: Implement `loops()` with back-edge detection
- [ ] 01-04-08: Write unit tests (minimum 4 tests)
- [ ] 01-04-09: (Optional) Split into submodules if >300 LOC

### Success Criteria
- Path enumeration returns all execution paths
- Normal/error filters correctly categorize paths
- Dominator tree is mathematically correct
- Natural loops are correctly detected
- All unit tests pass

---

## Task 01-05: Edit Module Implementation

**Status**: Pending
**Assigned**: Unassigned
**Priority**: P0 (Must Have)

### Subtasks

#### RenameOperation
- [ ] 01-05-01: Implement `verify()` to find all references
- [ ] 01-05-02: Add name conflict detection in `verify()`
- [ ] 01-05-03: Implement `preview()` to generate diffs
- [ ] 01-05-04: Implement `apply()` to update files
- [ ] 01-05-05: Implement `apply()` to update graph database
- [ ] 01-05-06: Implement `rollback()` from operation log
- [ ] 01-05-07: Add transaction wrapper for atomicity

#### DeleteOperation
- [ ] 01-05-08: Implement `verify()` to check active references
- [ ] 01-05-09: Implement `preview()` showing removal
- [ ] 01-05-10: Implement `apply()` to remove symbol
- [ ] 01-05-11: Implement `apply()` to remove all references
- [ ] 01-05-12: Implement `rollback()` to restore

#### Testing
- [ ] 01-05-13: Write unit tests for rename (minimum 4 tests)
- [ ] 01-05-14: Write unit tests for delete (minimum 2 tests)
- [ ] 01-05-15: (Optional) Split into submodules if >300 LOC

### Success Criteria
- Rename verifies all references before applying
- Rename updates all files and graph
- Rollback restores exact previous state
- Delete prevents unsafe deletions
- Delete removes definition and all references
- All unit tests pass

---

## Task 01-06: Test Infrastructure

**Status**: Pending
**Assigned**: Unassigned
**Priority**: P0 (Must Have)

### Subtasks

- [ ] 01-06-01: Add `tempfile = "3"` to dev-dependencies
- [ ] 01-06-02: Create `tests/common/mod.rs` with utilities
- [ ] 01-06-03: Implement `test_forge()` helper function
- [ ] 01-06-04: Implement `create_test_file()` helper function
- [ ] 01-06-05: Create `tests/integration/` directory
- [ ] 01-06-06: Write 3 integration tests for Graph
- [ ] 01-06-07: Write 2 integration tests for Search
- [ ] 01-06-08: Write 2 integration tests for Edit

### Success Criteria
- `cargo test` runs without tempfile errors
- Integration tests can create temporary codebases
- All integration tests pass
- Test helpers are reusable

---

## Task 01-07: Analysis Module Implementation

**Status**: Pending
**Assigned**: Unassigned
**Priority**: P1 (Should Have)

### Subtasks

- [ ] 01-07-01: Implement `impact_radius()` using reachable_from()
- [ ] 01-07-02: Add file tracking to impact analysis
- [ ] 01-07-03: Implement `unused_functions()` using graph queries
- [ ] 01-07-04: Implement `circular_dependencies()` delegating to cycles()
- [ ] 01-07-05: Write unit tests (minimum 3 tests)

### Success Criteria
- Impact radius includes all affected symbols
- Impact radius lists all affected files
- Unused functions correctly identify dead code
- Circular dependencies return cycle data
- All unit tests pass

---

## Task 01-08: Documentation Completion

**Status**: Pending
**Assigned**: Unassigned
**Priority**: P1 (Should Have)

### Subtasks

- [ ] 01-08-01: Run `cargo doc --no-deps` and fix warnings
- [ ] 01-08-02: Add example to Graph module
- [ ] 01-08-03: Add example to Search module
- [ ] 01-08-04: Add example to CFG module
- [ ] 01-08-05: Add example to Edit module (full workflow)
- [ ] 01-08-06: Add example to Analysis module
- [ ] 01-08-07: Verify all doc examples compile with `cargo test --doc`
- [ ] 01-08-08: Update API.md if needed

### Success Criteria
- `cargo doc` completes with zero warnings
- All public items have documentation
- All doc examples compile and run
- At least one working example per module

---

## Task Dependencies Graph

```
01-06 (Test Infra)     → No dependencies
                         ↓
01-01 (Storage)        → No dependencies
                         ↓
    ┌──────────────────────┼──────────────────────┐
    ↓                      ↓                      ↓
01-02 (Graph)       01-03 (Search)        01-04 (CFG)
    ↓                      ↓                      ↓
    └──────────────────────┼──────────────────────┘
                           ↓
                      01-05 (Edit)
                           ↓
                      01-07 (Analysis)
                           ↓
                      01-08 (Documentation)
```

---

## Tracking Legend

| Status | Meaning |
|--------|----------|
| Pending | Not started, ready to begin |
| In Progress | Active work on this task |
| Blocked | Cannot proceed, dependencies unmet |
| Review | Implementation complete, awaiting review |
| Complete | All subtasks done, tests passing |

---

*Last updated: 2026-02-12*

# Phase 1 Checklist

**Phase**: 01 - Core SDK Foundation
**Goal**: Complete v0.1 Foundation with functional SDK
**Status**: 📋 In Progress

---

## Overview Checklist

### Preparation
- [x] Project workspace initialized
- [x] Core types defined
- [x] Error system defined
- [x] Module stubs created
- [ ] Test infrastructure in place
- [ ] All modules functional

---

## Task 01-01: Storage Layer

**File**: `forge_core/src/storage/mod.rs`

### Implementation
- [ ] Add `sqlitegraph::GraphBackend` to `UnifiedGraphStore`
- [ ] Add `sqlitegraph::Connection` to `UnifiedGraphStore`
- [ ] Implement `async fn query_symbols(name: &str) -> Result<Vec<Symbol>>`
- [ ] Implement `async fn query_references(id: SymbolId) -> Result<Vec<Reference>>`
- [ ] Implement `async fn query_cfg(id: SymbolId) -> Result<CfgData>`
- [ ] Implement `async fn begin_transaction() -> Result<Transaction>`
- [ ] Implement `async fn commit_transaction(tx: Transaction) -> Result<()>`
- [ ] Implement `async fn rollback_transaction(tx: Transaction) -> Result<()>`

### Testing
- [ ] Test: Query returns correct symbols
- [ ] Test: Empty query returns empty vec
- [ ] Test: Transaction commits correctly
- [ ] Test: Transaction rollbacks correctly
- [ ] Test: Errors convert to `ForgeError`

### Verification
- [ ] File size ≤ 200 LOC
- [ ] All methods are async
- [ ] Error handling complete

---

## Task 01-02: Graph Module

**File**: `forge_core/src/graph/mod.rs`

### Implementation
- [ ] `find_symbol(name)` queries symbols table
- [ ] `find_symbol_by_id(id)` queries by primary key
- [ ] `callers_of(name)` filters for Call kind, incoming refs
- [ ] `references(name)` returns all reference types
- [ ] `reachable_from(id)` computes transitive closure
- [ ] `cycles()` detects call graph cycles

### Testing
- [ ] Test: Find symbol returns matches
- [ ] Test: Find by ID returns single symbol
- [ ] Test: Callers includes only calls
- [ ] Test: References include all kinds
- [ ] Test: Reachable includes transitive calls
- [ ] Test: Cycles detect circular calls

### Verification
- [ ] File size ≤ 300 LOC (or split)
- [ ] All methods return `Result<T>`
- [ ] SQL injection protected (prepared statements)

---

## Task 01-03: Search Module

**File**: `forge_core/src/search/mod.rs`

### Implementation
- [ ] `SearchBuilder::execute()` generates SQL query
- [ ] Name filter uses LIKE pattern matching
- [ ] Kind filter uses exact equality
- [ ] File filter uses prefix matching
- [ ] Limit clause applies correctly
- [ ] Multiple filters combine with AND

### Testing
- [ ] Test: Name filter works
- [ ] Test: Kind filter works
- [ ] Test: File filter works
- [ ] Test: Limit works
- [ ] Test: Combined filters work

### Verification
- [ ] File size ≤ 250 LOC
- [ ] Builder is fluent API
- [ ] No SQL injection vulnerabilities

---

## Task 01-04: CFG Module

**File**: `forge_core/src/cfg/mod.rs`

### Implementation
- [ ] `PathBuilder::execute()` enumerates paths
- [ ] `normal_only()` filters success paths
- [ ] `error_only()` filters error paths
- [ ] `max_length()` truncates paths
- [ ] `dominators()` computes dominator tree
- [ ] `loops()` detects natural loops

### Testing
- [ ] Test: Path enumeration returns all paths
- [ ] Test: Normal filter excludes error paths
- [ ] Test: Error filter excludes normal paths
- [ ] Test: Max length limits paths
- [ ] Test: Dominators computed correctly
- [ ] Test: Loops detected correctly

### Verification
- [ ] File size ≤ 300 LOC (or split)
- [ ] Path kinds correctly classified
- [ ] No infinite loops in enumeration

---

## Task 01-05: Edit Module

**File**: `forge_core/src/edit/mod.rs`

### RenameOperation Implementation
- [ ] `verify()` finds all references
- [ ] `verify()` checks for name conflicts
- [ ] `verify()` sets verified flag
- [ ] `preview()` generates unified diff
- [ ] `apply()` updates all files
- [ ] `apply()` updates graph database
- [ ] `rollback()` restores from log

### DeleteOperation Implementation
- [ ] `verify()` checks for active references
- [ ] `preview()` shows removal diff
- [ ] `apply()` removes definition
- [ ] `apply()` removes all references
- [ ] `rollback()` restores deleted code

### Testing
- [ ] Test: Rename verifies references
- [ ] Test: Rename detects conflicts
- [ ] Test: Rename preview shows diff
- [ ] Test: Rename apply updates files
- [ ] Test: Rename rollback works
- [ ] Test: Delete verifies safety
- [ ] Test: Delete apply removes code
- [ ] Test: Delete rollback works

### Verification
- [ ] File size ≤ 300 LOC (or split)
- [ ] All operations follow verify → preview → apply → rollback pattern
- [ ] Rollback always available

---

## Task 01-06: Test Infrastructure

### Dependencies
- [ ] `tempfile = "3"` added to `forge_core/Cargo.toml`

### Common Test Utilities
- [ ] `tests/common/mod.rs` created
- [ ] `test_forge()` helper implemented
- [ ] `create_test_file()` helper implemented

### Integration Tests
- [ ] `tests/integration/graph_tests.rs` created
- [ ] `tests/integration/search_tests.rs` created
- [ ] `tests/integration/edit_tests.rs` created
- [ ] At least 3 graph tests written
- [ ] At least 2 search tests written
- [ ] At least 2 edit tests written

### Verification
- [ ] `cargo test` runs successfully
- [ ] Tests use temp directories
- [ ] Tests clean up after themselves

---

## Task 01-07: Analysis Module

**File**: `forge_core/src/analysis/mod.rs`

### Implementation
- [ ] `impact_radius(symbol)` uses `reachable_from()`
- [ ] `impact_radius(symbol)` collects affected files
- [ ] `unused_functions(entries)` finds unreached symbols
- [ ] `circular_dependencies()` delegates to `cycles()`

### Testing
- [ ] Test: Impact radius includes all affected
- [ ] Test: Unused functions identified correctly
- [ ] Test: Circular dependencies return cycles

### Verification
- [ ] File size ≤ 200 LOC
- [ ] Uses other modules (no direct DB access)

---

## Task 01-08: Documentation

### Module Documentation
- [ ] Graph module has working example
- [ ] Search module has working example
- [ ] CFG module has working example
- [ ] Edit module has working example
- [ ] Analysis module has working example

### API Documentation
- [ ] `cargo doc --no-deps` runs without warnings
- [ ] All public items have `///` docs
- [ ] All examples compile with `cargo test --doc`

### Verification
- [ ] Examples demonstrate real workflows
- [ ] Cross-references are accurate

---

## Phase Exit Criteria

### Build & Test
- [ ] `cargo build --workspace` succeeds
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo fmt --check` passes

### Documentation
- [ ] `cargo doc --no-deps` completes
- [ ] No documentation warnings
- [ ] All examples run

### Code Quality
- [ ] No `#[allow(...)]` without justification
- [ ] No files exceed 300 LOC (core modules)
- [ ] All error paths handled

---

## Progress Tracking

### Overall Phase Progress

| Category | Complete | Total | Percent |
|----------|-----------|---------|----------|
| Tasks | 0 | 8 | 0% |
| Subtasks | 0 | ~85 | 0% |

### Module Progress

| Module | Status | Notes |
|--------|----------|--------|
| Storage | Pending | Foundation for all other modules |
| Graph | Pending | Core queries needed by Edit/Analysis |
| Search | Pending | Standalone, depends on Storage |
| CFG | Pending | Standalone, depends on Storage |
| Edit | Pending | Depends on Graph, Storage |
| Analysis | Pending | Composite, depends on all |
| Tests | Pending | Infrastructure needed for validation |
| Docs | Pending | Final polish after implementation |

---

## Notes

### Blocking Issues
None identified yet.

### Decisions Log
| Date | Decision | Rationale |
|-------|-----------|------------|
| - | - | - |

### Risks
| Risk | Mitigation |
|-------|------------|
| sqlitegraph API may differ from docs | Pin version, test early |
| File size may exceed limits | Plan to split into submodules |

---

*Last updated: 2026-02-12*

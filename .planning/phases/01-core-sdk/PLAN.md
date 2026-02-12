# Plan: Phase 1 - Core SDK Foundation

**Phase**: 01 - Core SDK Foundation
**Milestone**: v0.1 Foundation (Completion)
**Status**: 📋 Planned
**Created**: 2026-02-12
**Estimated Duration**: 2 weeks

---

## Objective

Complete the v0.1 Foundation phase by implementing actual functionality in the Core SDK layer. This phase transitions the project from stub implementations to a working SDK integrated with SQLiteGraph, Magellan, LLMGrep, Mirage, and Splice.

---

## Phase Context

### Current State (v0.1 @ 80%)

| Component | Status | Notes |
|-----------|---------|--------|
| Workspace Structure | ✅ Complete | All three crates compile |
| Core Types | ✅ Complete | All types in `types.rs` |
| Error System | ✅ Complete | `ForgeError` enum defined |
| Module Stubs | ✅ Complete | All 5 modules have stubs |
| Storage Backend | ⚠️ Placeholder | Only path management exists |
| Graph Module | ⚠️ Stub | Returns `BackendNotAvailable` |
| Search Module | ⚠️ Stub | Returns `BackendNotAvailable` |
| CFG Module | ⚠️ Stub | Returns `BackendNotAvailable` |
| Edit Module | ⚠️ Stub | Returns `BackendNotAvailable` |
| Test Infrastructure | ❌ Pending | tempfile not in dev-dependencies |

### Target State (v0.1 @ 100%)

All core modules functional with SQLiteGraph integration. Foundation complete, ready for v0.2 (Runtime Layer).

---

## Task Breakdown

### 1. Storage Layer Implementation

**Priority**: P0 (Must Have)
**Complexity**: Medium
**Dependencies**: None
**Estimated**: 2-3 days

**Objective**: Implement `UnifiedGraphStore` with actual SQLiteGraph backend integration.

#### File: `forge_core/src/storage/mod.rs`

**Current State**: 82 lines, placeholder implementation

**Required Changes**:
```rust
// Add SQLiteGraph backend integration
use sqlitegraph::GraphBackend;

#[derive(Clone)]
pub struct UnifiedGraphStore {
    codebase_path: PathBuf,
    db_path: PathBuf,
    backend: Arc<sqlitegraph::GraphBackend>,  // NEW
    connection: Arc<sqlitegraph::Connection>, // NEW
}
```

**New Methods to Implement**:
- `async fn query_symbols(&self, name: &str) -> Result<Vec<Symbol>>`
- `async fn query_references(&self, symbol_id: SymbolId) -> Result<Vec<Reference>>`
- `async fn query_cfg(&self, symbol_id: SymbolId) -> Result<CfgData>`
- `async fn begin_transaction(&self) -> Result<Transaction>`
- `async fn commit_transaction(&self, tx: Transaction) -> Result<()>`
- `async fn rollback_transaction(&self, tx: Transaction) -> Result<()>`

**Acceptance Criteria**:
- [ ] `UnifiedGraphStore` wraps `sqlitegraph::GraphBackend`
- [ ] Direct SQL queries work for symbols table
- [ ] Direct SQL queries work for references table
- [ ] Transaction management implemented
- [ ] Error handling converts SQLite errors to `ForgeError`
- [ ] Unit tests pass (minimum 3 tests)

**Integration Points**:
- `sqlitegraph` crate v1.6.0 (already in Cargo.toml)
- Uses `.forge/graph.db` as database location

**File Size Target**: ≤ 300 lines

---

### 2. Graph Module Implementation

**Priority**: P0 (Must Have)
**Complexity**: High
**Dependencies**: Task 1 (Storage)
**Estimated**: 3-4 days

**Objective**: Implement symbol and reference queries via Magellan integration.

#### File: `forge_core/src/graph/mod.rs`

**Current State**: 149 lines, all methods return `BackendNotAvailable`

**Methods to Implement**:

| Method | Implementation Strategy |
|--------|---------------------|
| `find_symbol(name)` | Query `symbols` table by name, filter by exact match |
| `find_symbol_by_id(id)` | Query `symbols` table by primary key |
| `callers_of(name)` | Query `references` table, filter by `kind == Call` and direction=in |
| `references(name)` | Query `references` table, return all reference kinds |
| `reachable_from(id)` | Use Magellan's reachable algorithm OR SQL recursive query |
| `cycles()` | Use Magellan's cycles algorithm OR DFS detection |

**Integration Approach**:
```rust
// Option A: Direct SQL (preferred for v0.1)
impl GraphModule {
    pub async fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>> {
        let sql = "SELECT * FROM symbols WHERE name = ?1";
        let stmt = self.store.prepare(sql).await?;
        // ... execute and return
    }
}

// Option B: CLI wrapper (fallback if SQL insufficient)
impl GraphModule {
    pub async fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>> {
        let output = Command::new("magellan")
            .args(["--db", self.store.db_path().to_str().unwrap()])
            .args(["find", "--name", name])
            .args(["--output", "json"])
            .output()
            .await?;
        // ... parse JSON output
    }
}
```

**Acceptance Criteria**:
- [ ] `find_symbol()` returns all symbols matching name
- [ ] `find_symbol_by_id()` returns single symbol or error
- [ ] `callers_of()` returns only Call references
- [ ] `references()` returns all reference types
- [ ] `reachable_from()` returns transitive closure
- [ ] `cycles()` detects actual call graph cycles
- [ ] Unit tests cover all methods (minimum 6 tests)

**File Size Target**: ≤ 300 lines (may need split into `query.rs` submodule)

---

### 3. Search Module Implementation

**Priority**: P0 (Must Have)
**Complexity**: Medium
**Dependencies**: Task 1 (Storage)
**Estimated**: 2-3 days

**Objective**: Implement semantic search via LLMGrep integration.

#### File: `forge_core/src/search/mod.rs`

**Current State**: 154 lines, builder complete but `execute()` is stub

**Methods to Implement**:

| Method | Implementation |
|--------|---------------|
| `SearchBuilder::execute()` | Query with applied filters, return matching symbols |
| `SearchModule::pattern()` | AST-based pattern search (may defer to v0.2) |

**Filter Implementation**:
```rust
impl SearchBuilder {
    pub async fn execute(self) -> Result<Vec<Symbol>> {
        let mut sql = "SELECT * FROM symbols WHERE 1=1".to_string();
        let mut params = vec![];

        if let Some(name) = &self.name_filter {
            sql.push_str(" AND name LIKE ?");
            params.push(format!("%{}%", name));
        }

        if let Some(kind) = &self.kind_filter {
            sql.push_str(" AND kind = ?");
            params.push(kind.to_string());
        }

        if let Some(file) = &self.file_filter {
            sql.push_str(" AND file_path LIKE ?");
            params.push(format!("{}%", file));
        }

        if let Some(limit) = self.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        // ... execute query
    }
}
```

**Acceptance Criteria**:
- [ ] `symbol().execute()` returns filtered results
- [ ] `kind()` filter correctly filters by SymbolKind
- [ ] `file()` filter correctly filters by path prefix
- [ ] `limit()` correctly caps result count
- [ ] Multiple filters work together (AND logic)
- [ ] Unit tests cover filter combinations (minimum 5 tests)

**File Size Target**: ≤ 250 lines

---

### 4. CFG Module Implementation

**Priority**: P1 (Should Have)
**Complexity**: High
**Dependencies**: Task 1 (Storage)
**Estimated**: 3-4 days

**Objective**: Implement control flow analysis via Mirage integration.

#### File: `forge_core/src/cfg/mod.rs`

**Current State**: 215 lines, types defined, methods stubbed

**Methods to Implement**:

| Method | Implementation Strategy |
|--------|---------------------|
| `PathBuilder::execute()` | Enumerate paths from CFG blocks |
| `dominators(function)` | Compute dominator tree using iterative algorithm |
| `loops(function)` | Detect natural loops using back-edge detection |

**Data Structure Notes**:
- CFG stored in SQLite as `blocks` and `edges` tables
- Block ID references symbol from `symbols` table
- Path enumeration needs cycle detection to avoid infinite paths

**Acceptance Criteria**:
- [ ] `paths().execute()` returns all execution paths
- [ ] `normal_only()` filters to success-only paths
- [ ] `error_only()` filters to error-returning paths
- [ ] `max_length()` prevents path explosion
- [ ] `dominators()` returns correct dominator tree
- [ ] `loops()` detects all natural loops
- [ ] Unit tests cover CFG operations (minimum 4 tests)

**File Size Target**: ≤ 300 lines (may need split into `paths.rs`, `dominance.rs`)

---

### 5. Edit Module Implementation

**Priority**: P0 (Must Have)
**Complexity**: Very High
**Dependencies**: Tasks 1, 2 (Storage, Graph)
**Estimated**: 4-5 days

**Objective**: Implement span-safe refactoring via Splice integration.

#### File: `forge_core/src/edit/mod.rs`

**Current State**: 242 lines, trait defined, implementations stubbed

**Methods to Implement**:

| Method | Implementation Strategy |
|--------|---------------------|
| `RenameOperation::verify()` | Query all references, check for conflicts |
| `RenameOperation::preview()` | Generate unified diff using `similar` crate |
| `RenameOperation::apply()` | Use Splice or direct file edit with spans |
| `RenameOperation::rollback()` | Restore from backup/log |
| `DeleteOperation::verify()` | Check symbol exists, no active references |
| `DeleteOperation::preview()` | Generate diff showing removal |
| `DeleteOperation::apply()` | Remove definition, update references |
| `DeleteOperation::rollback()` | Restore deleted code |

**Edit Workflow**:
```rust
impl EditOperation for RenameOperation {
    fn verify(mut self) -> Result<Self> {
        // 1. Find symbol definition
        let symbol = self.module.store.find_symbol(&self.old_name).await?
            .into_iter().next()
            .ok_or(ForgeError::SymbolNotFound(self.old_name.clone()))?;

        // 2. Find all references
        let refs = self.module.store.references(symbol.id).await?;

        // 3. Check for name conflicts
        let conflicts = self.module.store.find_symbol(&self.new_name).await?;
        if !conflicts.is_empty() {
            return Err(ForgeError::EditConflict {
                file: symbol.location.file_path,
                span: symbol.location.span,
            });
        }

        self.verified = true;
        Ok(self)
    }

    fn preview(self) -> Result<Diff> {
        if !self.verified {
            return Err(ForgeError::VerificationFailed(
                "Call verify() first".to_string()
            ));
        }

        // Generate diffs for each file
        // Use tree-sitter to locate exact spans
        // Generate unified diff format
    }

    fn apply(self) -> Result<RenameResult> {
        // 1. Create transaction
        // 2. For each file with references:
        //    - Read file content
        //    - Replace at exact spans
        //    - Write back
        // 3. Update graph database
        // 4. Commit transaction
        // 5. Log operation for rollback
    }
}
```

**Acceptance Criteria**:
- [ ] `rename_symbol().verify()` catches all error conditions
- [ ] `rename_symbol().preview()` shows accurate diffs
- [ ] `rename_symbol().apply()` updates all references
- [ ] `rename_symbol().rollback()` restores state
- [ ] `delete_symbol().verify()` prevents unsafe deletions
- [ ] `delete_symbol().apply()` removes definition and references
- [ ] Unit tests cover edit operations (minimum 6 tests)

**File Size Target**: ≤ 300 lines (may need split into `rename.rs`, `delete.rs`, `diff.rs`)

---

### 6. Test Infrastructure

**Priority**: P0 (Must Have)
**Complexity**: Low
**Dependencies**: None
**Estimated**: 1 day

**Objective**: Establish testing utilities and fix test dependencies.

#### File: `forge_core/Cargo.toml`

**Change Required**:
```toml
[dev-dependencies]
tokio = { version = "1", features = ["test-util", "macros"] }
tempfile = "3"  # ADD THIS
```

#### File: `tests/common/mod.rs` (NEW)

**Purpose**: Shared test utilities

```rust
//! Common test utilities

use tempfile::TempDir;
use crate::Forge;

/// Creates a test Forge instance with temporary storage.
pub async fn test_forge() -> (TempDir, Forge) {
    let temp = TempDir::new().unwrap();
    let forge = Forge::open(temp.path()).await.unwrap();
    (temp, forge)
}

/// Creates a test Rust file in a temp directory.
pub async fn create_test_file(dir: &Path, name: &str, content: &str) -> PathBuf {
    let file_path = dir.join(name);
    tokio::fs::write(&file_path, content).await.unwrap();
    file_path
}
```

#### File: `tests/integration/graph_tests.rs` (NEW)

```rust
//! Integration tests for graph module

#[tokio::test]
async fn test_find_symbol_returns_results() {
    // Create test codebase
    // Index with magellan
    // Query via GraphModule
    // Assert results
}
```

**Acceptance Criteria**:
- [ ] `tempfile` in dev-dependencies
- [ ] `tests/common/mod.rs` created with utilities
- [ ] `tests/integration/` directory created
- [ ] At least 3 integration tests for Graph
- [ ] At least 2 integration tests for Search
- [ ] At least 2 integration tests for Edit
- [ ] All tests pass

---

### 7. Analysis Module Implementation

**Priority**: P1 (Should Have)
**Complexity**: Low
**Dependencies**: Tasks 2, 4, 5 (Graph, CFG, Edit)
**Estimated**: 1-2 days

**Objective**: Implement composite operations using multiple modules.

#### File: `forge_core/src/analysis/mod.rs`

**Current State**: 117 lines, stubbed methods

**Methods to Implement**:

| Method | Implementation |
|--------|---------------|
| `impact_radius(symbol)` | Combine `reachable_from()` + reference counting |
| `unused_functions(entries)` | Use `reachable_from()` to find unreached |
| `circular_dependencies()` | Delegate to `graph.cycles()` |

**Acceptance Criteria**:
- [ ] `impact_radius()` returns affected symbols and files
- [ ] `unused_functions()` finds dead code
- [ ] `circular_dependencies()` returns cycles
- [ ] Unit tests cover analysis operations (minimum 3 tests)

**File Size Target**: ≤ 200 lines

---

### 8. Documentation Completion

**Priority**: P1 (Should Have)
**Complexity**: Low
**Dependencies**: All implementation tasks
**Estimated**: 2-3 days

**Objective**: Complete all API documentation and examples.

#### Tasks

1. **Verify all `#[doc]` comments compile**
   - Run `cargo doc --no-deps`
   - Fix any documentation warnings
   - Ensure all public items have docs

2. **Add working examples to each module**
   - Graph: Find symbol, get callers
   - Search: Filtered search with multiple criteria
   - CFG: Path enumeration with filters
   - Edit: Complete rename workflow
   - Analysis: Impact analysis example

3. **Update API.md** (if exists)
   - Document all public types
   - Document all public methods
   - Include examples

**Acceptance Criteria**:
- [ ] `cargo doc` completes without warnings
- [ ] Each module has at least 1 working example
- [ ] All examples in doc comments compile
- [ ] API.md is complete

---

## File/Module Structure

### Core Module Files

| File | Current LOC | Target LOC | New Files |
|-------|--------------|-------------|------------|
| `storage/mod.rs` | 82 | ≤ 200 | - |
| `graph/mod.rs` | 149 | ≤ 300 | `graph/query.rs`? |
| `search/mod.rs` | 154 | ≤ 250 | - |
| `cfg/mod.rs` | 215 | ≤ 300 | `cfg/paths.rs`, `cfg/domination.rs`? |
| `edit/mod.rs` | 242 | ≤ 300 | `edit/rename.rs`, `edit/delete.rs`, `edit/diff.rs`? |
| `analysis/mod.rs` | 117 | ≤ 200 | - |
| `types.rs` | 255 | - | - |
| `error.rs` | 92 | - | - |
| `lib.rs` | 242 | - | - |

### Test Files

| File | Status | Purpose |
|-------|----------|---------|
| `tests/common/mod.rs` | New | Shared utilities |
| `tests/integration/graph.rs` | New | Graph module tests |
| `tests/integration/search.rs` | New | Search module tests |
| `tests/integration/edit.rs` | New | Edit module tests |
| `tests/integration/cfg.rs` | New | CFG module tests |

---

## Success Criteria

### Phase Complete When:

1. **All Core Modules Functional**
   - [ ] Graph queries return real data from SQLiteGraph
   - [ ] Search filters work correctly
   - [ ] CFG operations execute on actual CFG data
   - [ ] Edit operations can modify files and update graph

2. **Test Coverage**
   - [ ] Unit tests for each module (≥80% coverage target)
   - [ ] Integration tests demonstrate end-to-end workflows
   - [ ] All tests pass with `cargo test --workspace`

3. **Documentation**
   - [ ] All public APIs documented
   - [ ] Code examples compile and run
   - [ ] No `cargo doc` warnings

4. **Code Quality**
   - [ ] No `#[allow(...)]` without justification
   - [ ] `cargo clippy` passes with no warnings
   - [ ] `cargo fmt` applied

5. **Build Status**
   - [ ] `cargo build --workspace` succeeds
   - [ ] `cargo test --workspace` passes
   - [ ] `cargo doc --no-deps` completes

---

## Risk Register

| Risk | Impact | Mitigation |
|-------|---------|------------|
| SQLiteGraph API changes | High | Pin to v1.6.0, track upstream |
| Tool CLI instability | Medium | Prefer direct library integration |
| File size blowup | Low | Split into submodules early |
| Test flakiness | Low | Use tempfile, isolate each test |
| Edit conflicts | High | Verify step mandatory, rollback available |

---

## Dependencies

### External Dependencies

| Crate | Version | Status | Notes |
|--------|---------|--------|-------|
| sqlitegraph | 1.6.0 | In Cargo.toml | Ready for use |
| tokio | 1.49.0 | In Cargo.toml | Full features enabled |
| tempfile | 3.x | ⚠️ MISSING | Add to dev-dependencies |
| similar | 2.x | ⚠️ MISSING | Add for diff generation |

### Internal Dependencies

```
Task 6 (Tests)      → No dependencies (can run in parallel)
Task 1 (Storage)     → No dependencies (foundation)
Task 2 (Graph)       → Task 1
Task 3 (Search)       → Task 1
Task 4 (CFG)          → Task 1
Task 5 (Edit)         → Tasks 1, 2
Task 7 (Analysis)     → Tasks 2, 4, 5
Task 8 (Docs)        → All implementation tasks
```

---

## Estimated Timeline

**Week 1** (Days 1-5):
- Day 1-2: Task 1 (Storage Layer)
- Day 3-4: Task 2 (Graph Module)
- Day 5: Task 3 (Search Module)

**Week 2** (Days 6-10):
- Day 6-7: Task 4 (CFG Module)
- Day 8-10: Task 5 (Edit Module)

**Week 3** (Days 11-13):
- Day 11: Task 6 (Test Infrastructure)
- Day 12: Task 7 (Analysis Module)
- Day 13: Task 8 (Documentation)

**Week 3** (Days 14-15):
- Day 14-15: Integration testing, bug fixes, polish

---

## Next Phase Preparation

Upon completion of Phase 1, the project will be ready for:

**Phase 2: v0.2 - Runtime Layer**
- File watching with `notify` crate
- Incremental indexing
- Query caching layer
- Connection pooling

**Phase 3: v0.3 - Agent Layer**
- Deterministic AI loop implementation
- Policy system
- LLM integration (optional)

---

## GSD Workflow Checklist

This plan follows the GSD (Graph-First Software Development) workflow:

### UNDERSTAND
- [x] Read existing codebase structure
- [x] Identify stub implementations
- [x] Map integration points
- [x] Define success criteria

### PLAN
- [x] Break down into 8 tasks
- [x] Identify dependencies
- [x] Estimate complexity
- [x] Define file structure

### PROVE (Next)
- [ ] Write test cases first (TDD)
- [ ] Verify test harness works
- [ ] Confirm tool availability

### IMPLEMENT (After PROVE)
- [ ] Implement Storage (Task 1)
- [ ] Implement Graph (Task 2)
- [ ] Implement Search (Task 3)
- [ ] Implement CFG (Task 4)
- [ ] Implement Edit (Task 5)
- [ ] Implement Analysis (Task 7)

### VERIFY (After IMPLEMENT)
- [ ] Run all tests
- [ ] Check documentation builds
- [ ] Verify code quality metrics
- [ ] Run integration tests

---

*Last updated: 2026-02-12*

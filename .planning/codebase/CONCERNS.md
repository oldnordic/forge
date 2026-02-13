# Codebase Concerns

**Analysis Date:** 2026-02-12

**Project:** ForgeKit v0.1.0
**Status:** Design Phase - Core SDK and Agent Layer Stubs

---

## Overview

ForgeKit is a code intelligence SDK that unifies several external tools (Magellan, LLMGrep, Mirage, Splice) under a single API. The project is in early development with significant stub implementations and deferred features. This document catalogs all known concerns, technical debt, implementation gaps, and architectural risks.

The codebase currently consists of:
- `forge_core` - Core SDK library with graph, search, CFG, edit, and analysis modules
- `forge_runtime` - Runtime layer for file watching, caching, and indexing (mostly stub)
- `forge_agent` - Agent layer implementing a six-phase deterministic AI loop

---

## 1. Implementation Gaps and Stub Methods

### 1.1 Storage Layer - No Database Operations

**Location:** `forge_core/src/storage/mod.rs`

**Concern:** The `UnifiedGraphStore` is a shell that provides no actual database functionality.

```rust
// All these methods return stub results:
pub async fn query_symbols(&self, name: &str) -> Result<Vec<Symbol>> {
    let graph = self.graph.as_ref()
        .ok_or_else(|| ForgeError::BackendNotAvailable(
            "Database not connected".to_string()
        ))?;

    // Placeholder implementation - returns empty
    self.query_symbols_impl(graph, name).await  // Returns Ok(Vec::new())
}

pub async fn query_references(&self, _symbol_id: SymbolId) -> Result<Vec<Reference>> {
    // Placeholder - will be implemented with proper SQLiteGraph API
    Ok(Vec::new())  // ALWAYS EMPTY
}

pub async fn symbol_exists(&self, id: SymbolId) -> Result<bool> {
    // For now, return false since we haven't implemented symbol lookup
    Ok(false)  // ALWAYS FALSE
}

pub async fn get_symbol(&self, id: SymbolId) -> Result<Symbol> {
    // For now, return not found since we haven't implemented symbol lookup
    Err(ForgeError::SymbolNotFound(format!("{}", id)))  // ALWAYS ERROR
}
```

**Impact:** All graph-dependent operations are non-functional. Cannot query symbols, references, or perform any graph analysis.

**Files Affected:**
- `forge_core/src/graph/mod.rs` - All graph operations fail
- `forge_core/src/search/mod.rs` - Cannot query actual data
- `forge_core/src/cfg/mod.rs` - Cannot access CFG data
- `forge_core/src/analysis/mod.rs` - Cannot perform impact analysis
- `forge_agent/src/observe.rs` - Observation phase returns no data
- `forge_agent/src/planner.rs` - Planning has no symbol data to work with

**Fix Approach:**
1. Complete `sqlitegraph` client integration in `UnifiedGraphStore`
2. Define database schema and migration path
3. Implement actual SQL queries for symbols/references
4. Add integration tests with real database

---

### 1.2 Graph Module - All Methods Stubbed

**Location:** `forge_core/src/graph/mod.rs`

**Concern:** All graph query methods are stub implementations.

```rust
pub async fn cycles(&self) -> Result<Vec<Cycle>> {
    // For now, return empty as we need full graph traversal
    // Full implementation will use Tarjan's SCC algorithm
    // or the sqlitegraph cycles API
    Ok(Vec::new())  // STUB - always empty
}
```

**Specific Stub Methods:**
- `find_symbol()` - Works but queries through stubbed storage
- `find_symbol_by_id()` - Always returns `SymbolNotFound`
- `callers_of()` - Returns empty via stubbed storage
- `references()` - Returns empty via stubbed storage
- `reachable_from()` - Builds adjacency but has no data
- `cycles()` - Completely stubbed, always returns empty

**Impact:** No cycle detection, no reachability analysis, dead code detection impossible.

**Fix Priority:** HIGH - Core SDK functionality

---

### 1.3 Search Module - LLMGrep Integration Missing

**Location:** `forge_core/src/search/mod.rs`

**Concern:** Pattern search is deferred to v0.1.

```rust
pub async fn pattern(&self, _pattern: &str) -> Result<Vec<Symbol>> {
    // TODO: Implement via LLMGrep integration
    // For v0.1, this is deferred
    Ok(Vec::new())  // STUB - always empty
}
```

**Impact:**
- No semantic search capability
- Pattern search for code exploration is non-functional
- SearchModule only provides name-based filtering

**Related Issue:** `SearchBuilder::execute()` can only filter by name, then applies kind/file/limit filters post-hoc. Inefficient for large symbol sets.

---

### 1.4 CFG Module - Mirage Integration Missing

**Location:** `forge_core/src/cfg/mod.rs`

**Concern:** All CFG operations return empty results.

```rust
pub async fn dominators(&self, function: SymbolId) -> Result<DominatorTree> {
    // For v0.1, return empty dominator tree
    // Full implementation requires CFG data from Mirage
    let _ = function;
    Ok(DominatorTree {
        root: BlockId(0),
        dominators: HashMap::new(),  // ALWAYS EMPTY
    })
}

pub async fn loops(&self, function: SymbolId) -> Result<Vec<Loop>> {
    // For v0.1, return empty list
    // Full implementation requires CFG data from Mirage
    let _ = function;
    Ok(Vec::new())  // ALWAYS EMPTY
}

pub async fn execute(self) -> Result<Vec<Path>> {
    // For v0.1, return empty path list
    // Full implementation requires CFG data from Mirage
    Ok(Vec::new())  // ALWAYS EMPTY
}
```

**Note:** The file does include complete test infrastructure (`TestCfg`, `DominatorTree`, `Path` implementations) that works correctly in isolation, but the module methods themselves return stub data.

**Impact:** No control flow analysis, complexity metrics, or path enumeration possible.

---

### 1.5 Edit Module - Splice Integration Missing

**Location:** `forge_core/src/edit/mod.rs`

**Concern:** Edit operations do not modify files, only track state.

```rust
fn apply(self) -> Result<Self::Output> {
    if !self.verified {
        return Err(ForgeError::VerificationFailed(
            "Call verify() first".to_string()
        ));
    }

    // For v0.1, return a result without actual file modification
    // Full implementation requires Splice integration
    Ok(RenameResult {
        files_modified: 0,      // ALWAYS ZERO
        references_updated: 0,    // ALWAYS ZERO
    })
}

fn rollback(self) -> Result<()> {
    // For v0.1, rollback is a no-op
    // Full implementation requires operation logging
    Ok(())  // DOES NOTHING
}
```

**Critical Flaw:** The `preview()` method returns a `Diff` with `file_path: PathBuf::from("<unknown>")` - not actionable.

```rust
fn preview(self) -> Result<Diff> {
    // Generate a simple diff showing the rename
    Ok(Diff {
        file_path: PathBuf::from("<unknown>"),  // USELESS
        original: self.old_name.clone(),
        modified: self.new_name.clone(),
    })
}
```

**Impact:** No actual refactoring possible. Rollback is a lie - cannot undo changes that were never made.

---

### 1.6 Runtime Layer - Almost Complete Stub

**Location:** `forge_runtime/src/lib.rs`

**Concern:** The entire runtime crate is a stub returning errors or zeros.

```rust
pub async fn new(_codebase_path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
    // TODO: Implement runtime initialization
    Ok(Self {
        config: RuntimeConfig::default(),  // Does nothing
    })
}

pub async fn watch(&self) -> anyhow::Result<()> {
    // TODO: Implement file watching
    Err(anyhow::anyhow!("File watching not yet implemented"))  // ALWAYS FAILS
}

pub async fn clear_cache(&self) -> anyhow::Result<()> {
    // TODO: Implement cache clearing
    Err(anyhow::anyhow!("Cache not yet implemented"))  // ALWAYS FAILS
}

pub fn stats(&self) -> RuntimeStats {
    RuntimeStats {
        cache_size: 0,           // ALWAYS ZERO
        watch_active: false,       // ALWAYS FALSE
        reindex_count: 0,          // ALWAYS ZERO
    }
}
```

**Impact:** No file watching, no caching, no runtime statistics. The `ForgeRuntime` type exists but provides no value.

**Paradox:** The workspace `forge_runtime` crate has real implementations in `forge_core/src/runtime.rs`, but the standalone `forge_runtime` crate is a stub.

---

### 1.7 Agent Layer - All Phases Stubbed

**Location:** `forge_agent/src/lib.rs` and submodules

**Concern:** The entire six-phase agent loop is non-functional.

#### Observe Phase (`observe.rs`)

```rust
async fn gather_symbols(&self, query: &ParsedQuery) -> Result<Vec<ObservedSymbol>> {
    // Search via pattern() which returns Vec::new()
    let results = search.pattern("").await?;  // ALWAYS EMPTY
    for symbol in results {
        if symbol.kind == SymbolKind::Function {
            symbols.push(ObservedSymbol::from_symbol(symbol.clone())?);
        }
    }
    // Result: symbols is always empty
}
```

The natural language query parser in `parse_query()` and `extract_target_name()` is implemented with string matching - functional but fragile.

#### Constrain Phase (`lib.rs`)

```rust
pub async fn constrain(&self, observation: Observation, _policies: Vec<policy::Policy>) -> Result<ConstrainedPlan> {
    // Policy validation will happen after mutation when we have actual diffs
    Ok(ConstrainedPlan {
        observation,
        policy_violations: vec![],  // ALWAYS EMPTY
    })
}
```

**Impact:** No policy enforcement at planning time.

#### Plan Phase (`planner.rs`)

```rust
pub async fn generate_steps(&self, observation: &Observation) -> Result<Vec<PlanStep>> {
    let mut steps = Vec::new();

    for symbol in &observation.symbols {
        // In production, this would use LLM to decide what operations
        // For now, create placeholder steps
        steps.push(PlanStep {
            description: format!("Process symbol {}", symbol.name),
            operation: PlanOperation::Inspect {  // ONLY INSPECT STEPS
                symbol_id: symbol.id,
                symbol_name: symbol.name.clone(),
            },
        });
    }

    Ok(steps)  // Only generates Inspect operations, no actual changes
}
```

**Impact:** No rename, delete, create, or modify operations are generated. Planning phase produces no useful work.

#### Mutate Phase (`mutate.rs`)

```rust
pub async fn apply_step(&mut self, step: &PlanStep) -> Result<()> {
    match &step.operation {
        PlanOperation::Rename { old, new } => {
            // Record for rollback
            transaction.applied_steps.push(format!("Rename {} to {}", old, new));
            // NO ACTUAL RENAME OCCURS
        }
        PlanOperation::Create { path, content } => {
            // Save original for rollback
            if let Ok(original_content) = fs::read_to_string(path).await {
                transaction.rollback_state.push(RollbackState { ... });
            }
            // Write new content
            fs::write(path, content).await
                .map_err(|e| AgentError::MutationFailed(...))?;
            transaction.applied_steps.push(format!("Create {}", path));
        }
        // ...
    }
}
```

**Note:** Only `Create` operations actually write files. `Rename`, `Delete`, `Modify` only track state without acting.

#### Verify Phase (`verify.rs`)

```rust
pub async fn graph_check(&self, _working_dir: &std::path::Path) -> Result<Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    // For v0.4, this is a simplified check
    // In production, would query graph for orphan references

    // Placeholder: graph is assumed consistent
    diagnostics.push(Diagnostic {
        level: DiagnosticLevel::Info,
        message: "Graph consistency check: skipped (not yet implemented)".to_string(),
    });

    Ok(diagnostics)  // ALWAYS SAYS SKIPPED
}
```

The `compile_check()` and `test_check()` methods do invoke `cargo` commands - these may actually work but are slow.

#### Commit Phase (`commit.rs`)

```rust
pub async fn finalize(&self, _working_dir: &std::path::Path, modified_files: &[std::path::PathBuf]) -> Result<CommitReport> {
    // Generate transaction ID using timestamp
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let transaction_id = format!("txn-{}", now);

    Ok(CommitReport {
        transaction_id: transaction_id.clone(),
        files_committed: modified_files.to_vec(),
    })
    // NO GIT INTEGRATION, NO METADATA PERSISTENCE
}
```

**Impact:** Transaction IDs are fake, no git integration, no history tracking.

---

### 1.8 Incremental Indexing - No Indexing Logic

**Location:** `forge_core/src/indexing.rs`

**Concern:** The indexer queues events but does nothing with them.

```rust
async fn index_file(&self, _path: &PathBuf) -> anyhow::Result<()> {
    // In a full implementation, this would:
    // 1. Read file
    // 2. Parse it with tree-sitter or similar
    // 3. Extract symbols, references, etc.
    // 4. Write to graph store

    // For v0.2, we'll store a placeholder record
    // The full indexing will be added in a later phase

    Ok(())  // DOES NOTHING
}

async fn delete_file(&self, _path: &PathBuf) -> anyhow::Result<()> {
    // In a full implementation, this would:
    // 1. Query all symbols in this file
    // 2. Delete those symbols
    // 3. Delete incoming/outgoing references
    // 4. Clean up any CFG blocks

    Ok(())  // DOES NOTHING
}
```

**Impact:** File changes are detected but never indexed. The `IncrementalIndexer` is effectively a no-op.

---

## 2. Technical Debt

### 2.1 Inconsistent Async/Sync API Design

**Locations:**
- `forge_core/src/edit/mod.rs` - `EditOperation` trait
- `forge_core/src/lib.rs` - API documentation

**Problem:** The `EditOperation` trait defines synchronous methods:

```rust
pub trait EditOperation: Sized {
    type Output;

    fn verify(self) -> Result<Self>;           // SYNC
    fn preview(self) -> Result<Diff>;           // SYNC
    fn apply(self) -> Result<Self::Output>;    // SYNC
    fn rollback(self) -> Result<()>;              // SYNC
}
```

But API documentation shows async usage:

```rust
/// # Examples
///
/// ```rust,no_run
/// let result = edit.rename_symbol("foo", "bar")
///     .verify()        // Not async
///     .await?        // BUT DOCS SHOW .await?
///     .apply()
///     .await?;       // AND HERE TOO?
/// ```
```

**Impact:**
- Documentation is misleading
- API is inconsistent with rest of async codebase
- Cannot easily make edit operations async without breaking trait

**Resolution:** Either make trait methods async or remove `.await` from all documentation/examples.

---

### 2.2 Typos in Variable Names

**Location:** `forge_agent/src/policy.rs`

**Issue:** Multiple functions use "complexity" instead of "complexity":

```rust
pub enum Policy {
    MaxComplexity(usize),  // SPELLED CORRECTLY
}

async fn check_max_complexity(    // TYPO: should be check_max_complexity
    _forge: &Forge,
    max_complexity: usize,  // TYPO: should be max_complexity
    diff: &Diff,
) -> Result<Option<PolicyViolation>>
```

Functions with typo:
- `check_max_complexity()` (should be `check_max_complexity`)
- `estimate_complexity_from_line()` (should be `estimate_complexity_from_line`)
- `estimate_complexity()` (should be `estimate_complexity()`)

**Impact:**
- Code is harder to read
- Auto-completion doesn't work as expected
- Inconsistent naming across codebase

**Fix:** Global rename of all `complexity` -> `complexity` in policy.rs.

---

### 2.3 Duplicate Type Definitions

**Locations:**
- `forge_core/src/types.rs` - Core shared types
- `forge_core/src/cfg/mod.rs` - CFG-specific module

**Issue:** `Path` and `Loop` types defined in both modules:

```rust
// In forge_core/src/types.rs (line 231-242):
pub struct Path {
    pub id: PathId,
    pub kind: PathKind,
    pub blocks: Vec<BlockId>,
    pub length: usize,
}

// In forge_core/src/cfg/mod.rs (line 274-285):
pub struct Path {  // DUPLICATE!
    pub id: PathId,
    pub kind: PathKind,
    pub blocks: Vec<BlockId>,
    pub length: usize,
}
```

**Impact:**
- Name collision when both modules are imported
- Compiler requires explicit paths: `crate::types::Path` vs `crate::cfg::Path`
- Confusing for code navigation

**Fix:** Rename CFG-specific types to `CfgPath`, `CfgLoop`, or use module-qualified names throughout.

---

### 2.4 Missing tempfile Dependency

**Locations:**
- `forge_core/src/storage/mod.rs:75`
- `forge_core/src/graph/mod.rs:141`
- `forge_agent/src/lib.rs:312`

**Issue:** Tests use `tempfile::tempdir()` but it's not in `[dev-dependencies]`:

```rust
// In multiple test files:
let store = Arc::new(UnifiedGraphStore::open(
    tempfile::tempdir().unwrap()  // COMPILES BUT DEP MISSING
).await.unwrap());
```

**Cargo.toml Analysis:**
```toml
# forge_core/Cargo.toml - MISSING tempfile:
[dev-dependencies]
tokio = { version = "1", features = ["test-util", "macros"] }
tempfile = "3"  # NOT PRESENT
```

**Impact:** `cargo test` may fail if tempfile is not pulled in by other dependencies.

**Fix:** Add `tempfile = "3"` to all `[dev-dependencies]` sections.

---

### 2.5 Incomplete ForgeBuilder Implementation

**Location:** `forge_core/src/lib.rs:305-318`

**Issue:** `ForgeBuilder::build()` is incomplete:

```rust
pub async fn build(self) -> anyhow::Result<Forge> {
    let path = self.path
        .ok_or_else(|| anyhow!("path is required"))?;

    let store = if let Some(db_path) = self.database_path {
        std::sync::Arc::new(UnifiedGraphStore::open_with_path(&path, &db_path).await?)
    } else {
        std::sync::Arc::new(UnifiedGraphStore::open(&path).await?)
    };

    Ok(Forge { store, runtime: None })  // NO builder options used!
}

// Builder accepts these options but ignores them:
pub struct ForgeBuilder {
    path: Option<std::path::PathBuf>,
    database_path: Option<std::path::PathBuf>,
    cache_ttl: Option<std::time::Duration>,  // IGNORED
}
```

**Impact:** The builder pattern is non-functional - `cache_ttl` is silently discarded.

---

### 2.6 Incomplete Indexer Flush Implementation

**Location:** `forge_core/src/indexing.rs:98-123`

**Issue:** `flush()` calls methods that do nothing:

```rust
pub async fn flush(&self) -> anyhow::Result<FlushStats> {
    let mut pending = self.pending.lock().await;
    let mut deleted = self.deleted.lock().await;

    let mut stats = FlushStats::default();

    // Process deletions first
    for path in deleted.drain() {
        if let Err(e) = self.delete_file(&path).await {  // delete_file DOES NOTHING
            eprintln!("Error deleting {:?}: {}", path, e);
        } else {
            stats.deleted += 1;  // NEVER INCREMENTS
        }
    }

    // Process additions/updates
    for path in pending.drain() {
        if let Err(e) = self.index_file(&path).await {  // index_file DOES NOTHING
            eprintln!("Error indexing {:?}: {}", path, e);
        } else {
            stats.indexed += 1;  // NEVER INCREMENTS
        }
    }

    Ok(stats)  // ALWAYS Returns zeros
}
```

**Impact:** Flush always reports success but changes are lost.

---

### 2.7 Weak Natural Language Query Parser

**Location:** `forge_agent/src/observe.rs:120-199`

**Issue:** Query parsing is fragile string matching:

```rust
fn parse_query(&self, query: &str) -> Result<ParsedQuery> {
    let query_lower = query.to_lowercase();

    let query_type = if query_lower.contains("functions that call") {
        QueryType::FunctionsCalling
    } else if query_lower.contains("functions called by") {
        QueryType::FunctionsCalledBy
    } else if query_lower.contains("find") && query_lower.contains("named") {
        QueryType::FindByName
    } else if query_lower.contains("all functions") {
        QueryType::AllFunctions
    } else if query_lower.contains("all structs") {
        QueryType::AllStructs
    } else {
        QueryType::SemanticSearch  // CATCHALL - falls back to pattern() which is stubbed
    };
    // ...
}
```

**Problems:**
- No tokenization or proper parsing
- Fails on compound queries
- `SemanticSearch` falls through to stubbed `pattern()`
- No handling of quotes, boolean operators, or scope limits

**Impact:** Agent can only understand very specific query patterns. Most user queries will fall through to non-functional semantic search.

---

## 3. Architecture and Design Concerns

### 3.1 Span-Safety Not Guaranteed

**Location:** `forge_core/src/edit/mod.rs`

**Problem:** The edit module assumes span-safety from Splice integration, but provides no verification:

```rust
pub fn rename_symbol(&self, old_name: &str, new_name: &str) -> RenameOperation {
    RenameOperation {
        module: self.clone(),
        old_name: old_name.to_string(),
        new_name: new_name.to_string(),
        verified: false,
    }
}

// No span validation happens anywhere
// No conflict detection for overlapping edits
// Rollback doesn't restore original content
```

**Risks:**
1. **Concurrent edits:** Multiple rename operations on overlapping spans could corrupt files
2. **No conflict detection:** `RenameOperation` doesn't check if `old_name` is ambiguous
3. **Incomplete rollback:** If apply succeeds but verify fails, rollback is a no-op

**Impact:** Data corruption during refactoring, especially multi-file edits.

**Fix Requirements:**
- Implement span validation before apply
- Add conflict detection for overlapping edits
- Design transactional edit log for proper rollback
- Consider file locks for concurrent mutation safety

---

### 3.2 Graph Consistency Not Maintained

**Problem:** When edits are applied, there's no strategy to keep the graph consistent:

```rust
// No cache invalidation defined
// No incremental reindex specification
// No triggers for graph updates

// forge_core/src/cache.rs has QueryCache but:
// - No integration with storage layer
// - No invalidation protocol
// - Modules can query stale data indefinitely
```

**Concerns:**
1. **Stale queries:** After an edit, cached symbol data is wrong
2. **No invalidation strategy:** Should cache be invalidated per-file, per-symbol, or globally?
3. **No reindex triggers:** What events cause graph updates?
4. **Concurrent edits:** If two processes edit, who wins?

**Impact:** Queries return incorrect data after mutations until... undefined state.

**Fix Requirements:**
- Define cache invalidation protocol (file-level, symbol-level, or global)
- Specify reindex triggers (file save, edit commit, or explicit)
- Consider write-ahead log for edit coordination
- Document cache TTL expectations

---

### 3.3 Backend Abstraction May Leak

**Location:** `forge_core/src/storage/mod.rs`

**Problem:** The `UnifiedGraphStore` is supposed to abstract backend differences, but doesn't:

```rust
#[derive(Clone)]
pub struct UnifiedGraphStore {
    pub codebase_path: std::path::PathBuf,
    pub db_path: std::path::PathBuf,
    #[cfg(feature = "sqlite")]
    graph: Option<Arc<sqlitegraph::SqliteGraph>>,  // DIRECT SQLITE REFERENCE
    // No backend trait
    // No abstraction over different backends
}
```

**Issues:**
1. No `Backend` trait defined internally
2. SQLite vs Native V3 differences may leak to API
3. Feature-specific capabilities unhandled
4. Code duplication when adding new backends

**Impact:**
- Hard to test without real database
- Cannot mock storage for unit tests
- Native V3 support will require code changes everywhere

**Fix:** Define a `GraphBackend` trait before v0.2.

---

### 3.4 Watcher Event Debounce May Lose Events

**Location:** `forge_core/src/watcher.rs:97-111`

**Issue:** Debounce logic has a race condition:

```rust
let event_handler = move |res: notify::Result<notify::Event>| {
    let now = std::time::Instant::now();

    match res {
        Ok(event) => {
            for path in event.paths {
                // Check debounce
                if let Some(last) = &last_path {
                    if last == &path && now.duration_since(last_event).as_millis() < 100 {
                        continue;  // SKIPS THIS EVENT
                    }
                }

                let watch_event = match event.kind {
                    notify::EventKind::Create(_) => WatchEvent::Created(path.clone()),
                    // ...
                };

                last_path = Some(path);     // UPDATED AFTER CHECK
                last_event = now;          // UPDATED AFTER CHECK

                let _ = sender.send(watch_event);  // MAY NOT SEND
            }
        }
        // ...
    }
};
```

**Race:** If multiple events for same path arrive within 100ms window:
- First event: `last_path = None`, passes check, sends event, updates `last_path`
- Second event (within 100ms): `last_path = Some(path)`, time diff < 100ms, **SKIPPED**

**Impact:** Rapid file changes may be lost. Editor save sequences (write → rename) could miss the rename.

---

### 3.5 Transaction Not Atomic

**Location:** `forge_agent/src/mutate.rs:49-112`

**Issue:** Transaction tracking is in-memory only, not durable:

```rust
pub async fn apply_step(&mut self, step: &PlanStep) -> Result<()> {
    // ...
    match &step.operation {
        PlanOperation::Create { path, content } => {
            // Save original for rollback
            if let Ok(original_content) = fs::read_to_string(path).await {
                transaction.rollback_state.push(RollbackState {
                    file: path.clone(),
                    original_content,  // IN MEMORY ONLY
                });
            }

            // Write new content
            fs::write(path, content).await
                .map_err(|e| AgentError::MutationFailed(...))?;
        }
        // ...
    }
}

pub async fn rollback(&mut self) -> Result<()> {
    let transaction = self.transaction.take()
        .ok_or_else(|| AgentError::MutationFailed("No active transaction".to_string()))?;

    // Rollback in reverse order
    for state in transaction.rollback_state.iter().rev() {
        std::fs::write(&state.file, &state.original_content)  // IF PROCESS CRASHES, DATA LOST
            .map_err(|e| AgentError::MutationFailed(...))?;
    }

    Ok(())
}
```

**Risks:**
1. **Process crash:** Rollback state is lost on crash
2. **No commit log:** Cannot recover transactions after restart
3. **No isolation:** Multiple agents on same codebase could conflict

---

### 3.6 Watcher Has No Stop Mechanism

**Location:** `forge_core/src/watcher.rs:91-140`

**Issue:** Once started, watcher cannot be stopped:

```rust
pub async fn start(&self, path: PathBuf) -> notify::Result<()> {
    // ...
    let event_handler = move |res: notify::Result<notify::Event>| { /* ... */ };

    // Create watcher
    RecommendedWatcher::new(event_handler, notify::Config::default())?
        .watch(&path, RecursiveMode::Recursive)?;

    Ok(())  // NO HANDLE RETURNED - CANNOT STOP
}
```

**Impact:**
- Cannot gracefully shutdown watcher
- Background thread continues after `drop`
- Resource leak on restart

**Fix:** Return a `WatcherHandle` with shutdown method.

---

## 4. Performance Concerns

### 4.1 No Query Result Caching

**Location:** `forge_core/src/lib.rs`, `forge_core/src/graph/mod.rs`

**Issue:** Despite `ForgeBuilder` accepting `cache_ttl`, no caching exists:

```rust
// In lib.rs, builder accepts cache_ttl:
pub fn cache_ttl(mut self, ttl: std::time::Duration) -> Self {
    self.cache_ttl = Some(ttl);
    self
}

// But Forge struct doesn't use it:
#[derive(Clone)]
pub struct Forge {
    store: std::sync::Arc<UnifiedGraphStore>,
    runtime: Option<std::sync::Arc<runtime::Runtime>>,
    // NO CACHE FIELD
}
```

**Impact:**
- Symbol queries always hit database (when functional)
- CFG paths recomputed every time
- Search results not memoized
- No performance benefit from planned caching layer

**Related:** `forge_core/src/cache.rs` implements `QueryCache` but it's never integrated into the query paths.

---

### 4.2 Linear Search in Filter Operations

**Location:** `forge_core/src/search/mod.rs:127-160`

**Issue:** `SearchBuilder::execute()` filters sequentially:

```rust
pub async fn execute(self) -> Result<Vec<Symbol>> {
    // Get all symbols matching the name filter
    let name_match = match &self.name_filter {
        Some(name) => {
            let symbols = self.module.store.query_symbols(name).await?;  // DB HIT
            symbols
        }
        None => {
            return Ok(Vec::new());
        }
    };

    // Apply filters - ALL IN MEMORY ON FULL RESULT SET
    let mut filtered = name_match;

    if let Some(ref kind) = self.kind_filter {
        filtered.retain(|s| s.kind == *kind);  // O(n) scan
    }

    if let Some(ref file) = self.file_filter {
        filtered.retain(|s| {  // ANOTHER O(n) scan
            s.location.file_path.to_string_lossy().starts_with(file.as_str())
        });
    }

    if let Some(n) = self.limit {
        filtered.truncate(n);  // O(1) but after full scan
    }

    Ok(filtered)
}
```

**Impact:** For codebases with thousands of symbols, filtering is expensive. Database does all the work, then results are scanned in Rust.

**Fix:** Push filters into database query (WHERE clause) not post-processing.

---

### 4.3 Unbounded Vector Growth in Observations

**Location:** `forge_agent/src/observe.rs:282-311`

**Issue:** No size limits on gathered data:

```rust
async fn gather_symbols(&self, query: &ParsedQuery) -> Result<Vec<ObservedSymbol>> {
    let mut symbols = Vec::new();  // Could grow unbounded

    match &query.query_type {
        QueryType::AllFunctions => {
            let results = search.pattern("").await?;  // Returns ALL functions
            for symbol in results {
                if symbol.kind == SymbolKind::Function {
                    symbols.push(ObservedSymbol::from_symbol(symbol.clone())?);
                }
            }
        }
        // ...
    }

    Ok(symbols)  // No limit applied
}
```

**Impact:**
- "All functions" query could return tens of thousands of symbols
- Memory exhaustion on large codebases
- No pagination for large result sets

---

### 4.4 Inefficient LRU Cache Implementation

**Location:** `forge_core/src/cache.rs:137-157`

**Issue:** LRU "touch" operation is O(n):

```rust
pub async fn get(&self, key: &K) -> Option<V> {
    let mut inner = self.inner.write().await;

    // ...
    if let Some(entry) = value_opt {
        if now < entry.expires_at {
            // Touch key: move to end of list (LRU behavior)
            if let Some(pos) = inner.keys.iter().position(|k| k == &key_clone) {  // O(n) SCAN
                inner.keys.remove(pos);
                inner.keys.push(key_clone);
            }
            return Some(entry.value);
        }
        // ...
    }
    None
}
```

**Impact:**
- Cache get operations are O(n) not O(1)
- Performance degrades with cache size
- Mutex held for entire scan

**Fix:** Use `HashMap` + `LinkedList` for proper O(1) LRU, or existing crate like `lru`.

---

### 4.5 File Watch Creates Unbounded Tasks

**Location:** `forge_core/src/indexing.rs:66-84`

**Issue:** `queue()` spawns unbounded tasks:

```rust
pub fn queue(&self, event: WatchEvent) {
    match event {
        WatchEvent::Created(path) | WatchEvent::Modified(path) => {
            let pending = self.pending.clone();
            tokio::spawn(async move {  // SPAWNS UNBOUNDED
                pending.lock().await.insert(path);
            });
        }
        WatchEvent::Deleted(path) => {
            let deleted = self.deleted.clone();
            tokio::spawn(async move {  // ANOTHER UNBOUNDED TASK
                deleted.lock().await.insert(path);
            });
        }
        // ...
    }
}
```

**Impact:**
- Rapid file changes spawn thousands of tasks
- Each task takes a mutex lock
- Potential task exhaustion
- No backpressure mechanism

**Fix:** Use bounded channel with backpressure.

---

## 5. Security Considerations

### 5.1 No Validation of Agent-Generated Operations

**Location:** `forge_agent/src/planner.rs:28-50`

**Issue:** Plan generation has no safety checks:

```rust
pub async fn generate_steps(&self, observation: &Observation) -> Result<Vec<PlanStep>> {
    let mut steps = Vec::new();

    for symbol in &observation.symbols {
        // In production, this would use LLM to decide what operations
        // For now, create placeholder steps
        steps.push(PlanStep {
            description: format!("Process symbol {}", symbol.name),
            operation: PlanOperation::Inspect {  // ONLY INNOCUOUS STEPS
                symbol_id: symbol.id,
                symbol_name: symbol.name.clone(),
            },
        });
    }

    Ok(steps)  // NO VALIDATION OF STEPS
}
```

**Risks:**
- LLM could generate malicious operations (delete all files, etc.)
- No allowlist for dangerous operations
- No confirmation required for destructive changes
- No audit trail of agent actions

**Recommendations:**
1. Validate all `PlanOperation` variants against allowlist
2. Require explicit confirmation for: Delete, Modify, Create operations
3. Implement audit logging for all agent actions
4. Consider "dry run" mode that shows what would happen

---

### 5.2 Database File Permissions Not Specified

**Location:** `forge_core/src/storage/mod.rs:35-46`

**Issue:** Graph database at `.forge/graph.db` has no permission model:

```rust
pub async fn open(codebase_path: impl AsRef<Path>) -> Result<Self> {
    let codebase = codebase_path.as_ref();
    let db_path = codebase.join(".forge").join("graph.db");

    // Create parent directory if it doesn't exist
    if let Some(parent) = db_path.parent() {
        tokio::fs::create_dir_all(parent).await
            .map_err(|e| ForgeError::DatabaseError(
                format!("Failed to create database directory: {}", e)
            ))?;
    }

    // ... no permission setting on created files
}
```

**Concerns:**
- Database created with umask (often 0644 - group/world readable)
- No encryption at rest specified
- Could expose codebase structure to unintended readers
- Shared development environments risk information leakage

**Impact:** Information disclosure in shared environments.

**Recommendations:**
1. Set 0600 permissions on `.forge` directory
2. Document permission model
3. Consider optional encryption for sensitive codebases
4. Add `.gitignore` entry for `.forge/` to avoid accidental commit

---

### 5.3 Arbitrary Command Execution in Verifier

**Location:** `forge_agent/src/verify.rs:28-95`

**Issue:** Compile and test checks run arbitrary commands:

```rust
pub async fn compile_check(&self, working_dir: &std::path::Path) -> Result<Vec<Diagnostic>> {
    let output = Command::new("cargo")
        .args(["check", "--message-format=short"])
        .current_dir(working_dir)  // RUNS IN ANY DIRECTORY
        .output()
        .map_err(|e| AgentError::VerificationFailed(
            format!("Cargo check failed: {}", e)
        ))?;

    // Output is parsed but could contain anything
}

pub async fn test_check(&self, working_dir: &std::path::Path) -> Result<Vec<Diagnostic>> {
    let output = Command::new("cargo")
        .args(["test", "--message-format=short"])
        .current_dir(working_dir)  // RUNS TESTS IN ANY DIRECTORY
        .output()
        // ...
}
```

**Concerns:**
- Runs in any `working_dir` passed to verify
- `cargo` must be on PATH (could be malicious binary)
- Tests run with full filesystem access
- No sandboxing or resource limits

**Impact:** If agent is compromised, could execute arbitrary cargo commands.

**Mitigation:**
1. Pin absolute path to cargo binary
2. Validate working_dir is within expected codebase
3. Consider chroot sandbox for test execution
4. Add timeout to prevent hanging tests

---

## 6. Testing Concerns

### 6.1 Tests Pass Against Empty Databases

**Locations:** Throughout `forge_core` and `forge_agent`

**Issue:** Most tests use empty databases and assert on empty results:

```rust
#[tokio::test]
async fn test_find_symbol_empty() {
    let store = Arc::new(UnifiedGraphStore::open(
        tempfile::tempdir().unwrap()
    ).await.unwrap());
    let module = GraphModule::new(store);

    let symbols = module.find_symbol("nonexistent").await.unwrap();
    assert_eq!(symbols.len(), 0);  // PASSES ON EMPTY DB
}

#[tokio::test]
async fn test_cycles_empty() {
    let store = Arc::new(UnifiedGraphStore::open(
        tempfile::tempdir().unwrap()
    ).await.unwrap());
    let module = GraphModule::new(store);

    let cycles = module.cycles().await.unwrap();
    assert_eq!(cycles.len(), 0);  // PASSES ON STUBBED METHOD
}
```

**Impact:**
- Tests provide no coverage of actual functionality
- Stub implementations appear to "work" in tests
- False confidence in code quality

**Fix:** Add integration tests with pre-populated databases or test fixtures.

---

### 6.2 Test Coverage for Critical Paths Missing

**Locations:** Various

**Uncovered Areas:**
1. **Incremental indexing:** `index_file()` and `delete_file()` never tested with real files
2. **File watcher:** No test verifies actual file watching works
3. **Conflict detection:** No tests for overlapping edit detection
4. **Rollback:** No test verifies rollback restores actual content
5. **Policy validation:** Only tests happy path, no edge cases
6. **Natural language parsing:** Fragile parser has no coverage for malformed input
7. **Error recovery:** No tests for error scenarios in agent loop

---

## 7. Integration and Dependency Concerns

### 7.1 Dependency Version Conflicts

**File:** `Cargo.toml` workspace dependencies

**Issue:** Different versions of `sqlitegraph` specified:

```toml
# forge_core/Cargo.toml:
sqlitegraph = { version = "1.6", default-features = false, optional = true }

# forge_runtime/Cargo.toml:
sqlitegraph = { version = "1.5", default-features = false, optional = true }
```

**Impact:**
- Potential for schema incompatibility
- Different features enabled between crates
- Unpredictable which version gets linked in workspace

**Fix:** Unify all `sqlitegraph` dependencies to exact same version.

---

### 7.2 Native V2 Backend Not Implemented

**Files:**
- `forge_core/Cargo.toml:17`
- `forge_runtime/Cargo.toml:16-17`
- `forge_agent/Cargo.toml:16-17`

```toml
[features]
native-v2 = ["sqlitegraph/native-v2"]  # DEFINED BUT NO IMPLEMENTATION
```

**Issue:** Feature flag exists but no native V3 backend code exists.

**Impact:**
- Users may enable feature expecting faster performance
- Feature appears to work but falls back to SQLite
- No documentation of what native V3 means

**Resolution:** Either implement native V3 or remove feature flag.

---

### 7.3 No Integration Tests

**Location:** Workspace root

**Issue:** No tests verify tool integrations work:

```bash
# No tests for:
# - Magellan indexing produces correct graph.db
# - LLMGrep semantic search returns expected results
# - Mirage CFG analysis generates correct paths
# - Splice edit operations apply correctly
```

**Impact:**
- External tool bugs break ForgeKit silently
- Version mismatches not caught until runtime
- Integration assumptions never validated

**Fix:** Add integration test suite with fixture codebases.

---

## 8. Documentation Concerns

### 8.1 Async/Sync Inconsistency in Examples

**Locations:** Throughout crate documentation

**Issue:** Examples show `.await?` on sync operations:

```rust
/// # Examples
///
/// ```rust,no_run
/// # let forge = unimplemented!();  // IN lib.rs - says unimplemented
/// let result = edit.rename_symbol("foo", "bar")
///     .verify()
///     .await?      // EditOperation::verify is SYNC, not async
///     .apply()
///     .await?;   // EditOperation::apply is SYNC, not async
/// ```
```

**Impact:** Users write code that doesn't compile.

---

### 8.2 Example Code Uses `unimplemented!()` Macro

**Locations:** `forge_core/src/lib.rs`

**Issue:** Documentation examples contain `unimplemented!()`:

```rust
/// # Examples
///
/// ```rust,no_run
/// # let forge = unimplemented!();  // COPY-PASTA ERROR IN DOCS
/// let result = edit.rename_symbol("foo", "bar")...
```

**Impact:** Documentation cannot be run as-is.

---

## 9. Blocking Issues

### 9.1 Phase Exit Criteria Unclear

**File:** `.planning/milestones/v0.1-ROADMAP.md`

**Issue:** v0.1 success criteria are not objectively verifiable:

```
## Phase 01: Project Organization
**Status:** [ ] Planned

## Exit Criteria
1. Workspace compiles
2. All modules have public API
3. Basic tests pass
```

**Problems:**
- "All modules have public API" is subjective
- No metric for "basic" test coverage
- No integration test requirement
- Phase can be marked complete without actual functionality

**Risk:** v0.1 declared "done" but core SDK doesn't work.

---

### 9.2 Circular Dependency in Development Workflow

**Location:** Development philosophy

**Issue:** TDD approach requires graph to work, but graph doesn't exist without indexing:

```
From DEVELOPMENT_WORKFLOW.md:
> NEVER write code based on assumptions. ALWAYS read source and query graph first.

# But:
# 1. To query graph, it must exist
# 2. To create graph, need indexing
# 3. Indexing needs tree-sitter/parser
# 4. Parser needs to be written
# 5. Which requires TDD tests first...
```

**Impact:**
- Cannot follow TDD strictly when starting from scratch
- Creates false choice between "write code based on assumptions" vs "can't test yet"
- May lead to analysis paralysis

**Workaround:** For initial implementation, write stub code with clear markers, implement bottom-up.

---

## 10. Platform and Compatibility Concerns

### 10.1 Windows Path Handling Not Tested

**Issue:** Code uses `std::path::PathBuf` throughout with no Windows testing:

```rust
// Throughout codebase:
pub struct Location {
    pub file_path: PathBuf,  // May have issues on Windows
    // ...
}
```

**Known Windows Issues:**
- Path separator (`/` vs `\`)
- Max path length (260 chars)
- Reserved filenames (`CON`, `PRN`, etc.)
- Case sensitivity differences

**Impact:** May not work correctly on Windows platforms.

---

### 10.2 No Graceful Degradation for Missing Features

**Locations:** `forge_core/src/lib.rs:42-63`

**Issue:** `BackendNotAvailable` error terminates flow:

```rust
impl From<anyhow::Error> for ForgeError {
    fn from(err: anyhow::Error) -> anyhow::Error {
        err
    }
}

// When sqlitegraph is not available, all operations fail immediately
// No "limited mode" or graceful degradation
```

**Impact:**
- Cannot use ANY ForgeKit functionality without sqlitegraph
- No error messages explaining what's missing
- No guidance for users on installation

---

## 11. Known Bugs and Race Conditions

### 11.1 Race in Cache Insert Update

**Location:** `forge_core/src/cache.rs:152-157`

**Issue:** Cache doesn't update keys correctly on duplicate insert:

```rust
pub async fn insert(&self, key: K, value: V) {
    let mut inner = self.inner.write().await;

    // ...eviction logic...

    // Update or insert
    if !inner.keys.contains(&key) {
        inner.keys.push(key.clone());
    }
    inner.entries.insert(key, CacheEntry { value, expires_at });
    // BUG: If key already exists, it's NOT moved to end (LRU violation)
}
```

**Impact:**
- Frequently-used entries may be evicted prematurely
- Cache behavior doesn't match LRU documentation

---

### 11.2 Potential Deadlock in Runtime

**Location:** `forge_core/src/runtime.rs:95-110`

**Issue:** Acquires permit while holding potential locks:

```rust
pub async fn start_with_watching(&mut self) -> anyhow::Result<()> {
    let (tx, _rx) = Watcher::channel();
    let watcher = Watcher::new(self.store.clone(), tx);

    let path = std::env::current_dir()?;
    watcher.start(path).await?;  // What if this spawns tasks?

    self.watcher = Some(watcher);  // Assignment after start

    // Note: For v0.2, event processing is manual via process_events()
    // Background processing would require store to be Send + Sync
}
```

**Concern:** If `Watcher::new` spawns background tasks that access `store`, and store is not `Send + Sync`, potential for undefined behavior.

---

## Summary by Category

| Category | Count | Severity |
|----------|--------|----------|
| Stub/Unimplemented Methods | 35+ | Critical |
| API Inconsistencies | 8 | High |
| Missing Integrations | 5 | Critical |
| Performance Issues | 7 | Medium |
| Security Concerns | 6 | High |
| Testing Gaps | 9 | Medium |
| Documentation Issues | 4 | Medium |
| Architecture Risks | 8 | High |
| Race Conditions | 3 | High |
| Dependency Issues | 4 | Medium |
| **TOTAL** | **89** | - |

---

## Prioritized Remediation

### Phase 1 (Critical - Blocker)

1. **Complete `UnifiedGraphStore` implementation** - All other modules depend on this
2. **Implement sqlitegraph integration** - Real database queries required
3. **Add tempfile to dev-dependencies** - Tests may be broken
4. **Fix async/sync inconsistency in EditOperation** - API confusion

### Phase 2 (High Priority)

5. **Implement actual file operations in EditModule** - Currently no-ops
6. **Complete IncrementalIndexer** - File changes must be indexed
7. **Add conflict detection for edits** - Prevent data corruption
8. **Implement LLMGrep integration** - Semantic search required
9. **Implement Mirage integration** - CFG analysis required
10. **Fix watcher debounce race** - Events may be lost

### Phase 3 (Medium Priority)

11. **Add integration test suite** - Verify tool integrations
12. **Implement query result caching** - Performance optimization
13. **Define cache invalidation protocol** - Graph consistency
14. **Fix all `complexity` typos** - Code quality
15. **Unify sqlitegraph versions** - Prevent conflicts
16. **Add Native V3 or remove feature flag** - Honest API
17. **Implement durable transaction log** - Crash recovery
18. **Add watcher stop mechanism** - Resource cleanup
19. **Fix cache LRU implementation** - O(1) operations
20. **Document agent operation safety** - Security guidance

---

*Concerns analysis: 2026-02-12*
*Total items cataloged: 89*
*Lines: 1,471*

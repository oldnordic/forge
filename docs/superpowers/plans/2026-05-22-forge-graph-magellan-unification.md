# forge_core Graph Module — Magellan Unification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace forge_core's broken graph module integration with a clean dependency on `magellan` as the single query engine, using `~/.magellan/<stem>.db` as the shared DB path.

**Architecture:** `UnifiedGraphStore::open` derives the DB path as `~/.magellan/<project_stem>.db`. `ForgeBuilder` gains `db_path`/`db_dir` overrides for tests and non-standard setups. `GraphModule` methods are rewritten to call `magellan::CodeGraph` directly; `graph/queries.rs` and all `#[cfg(not(feature = "magellan"))]` fallback arms are deleted.

**Tech Stack:** Rust, magellan 3.3.x (already in Cargo.toml as default dep), sqlitegraph (already in Cargo.toml), tokio, tempfile (dev-dep)

**Spec:** `docs/superpowers/specs/2026-05-22-forge-graph-magellan-unification-design.md`

---

## File Map

| Action | File | What changes |
|--------|------|-------------|
| Modify | `forge_core/src/storage/mod.rs` | Add `default_db_path`, change `UnifiedGraphStore::open` path |
| Modify | `forge_core/src/lib.rs` | Add `db_path`/`db_dir` to `ForgeBuilder`, update `build()` |
| Modify | `forge_core/src/graph/mod.rs` | Rewrite 5 methods, move `ImpactedSymbol`, add test helper, remove fallbacks |
| Delete | `forge_core/src/graph/queries.rs` | Entire file removed |

---

## Task 1: Add `default_db_path` to storage

**Files:**
- Modify: `forge_core/src/storage/mod.rs`

- [ ] **Step 1: Write the failing test**

Add at the bottom of `forge_core/src/storage/mod.rs`, inside `#[cfg(test)]`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_db_path_uses_home_dot_magellan() {
        let project = std::path::Path::new("/home/user/Projects/my-cool-project");
        let db = default_db_path(project);
        assert!(db.to_string_lossy().contains(".magellan"));
        assert!(db.to_string_lossy().ends_with("my-cool-project.db"));
    }

    #[test]
    fn test_default_db_path_fallback_stem() {
        // Path with no file_name component should not panic
        let project = std::path::Path::new("/");
        let db = default_db_path(project);
        assert!(db.to_string_lossy().ends_with(".magellan/graph.db"));
    }
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cd /home/feanor/Projects/forge
cargo test -p forge-core storage::tests 2>&1 | tail -20
```

Expected: FAIL with `cannot find function 'default_db_path'`

- [ ] **Step 3: Add `default_db_path` to `storage/mod.rs`**

Add this function immediately before the `BackendKind` enum (around line 44):

```rust
/// Resolves the default magellan database path for a project root.
///
/// Returns `~/.magellan/<stem>.db` where `<stem>` is the last component
/// of `project_root`. Falls back to `~/.magellan/graph.db` if the stem
/// cannot be determined.
pub fn default_db_path(project_root: &std::path::Path) -> std::path::PathBuf {
    let stem = project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("graph");
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    std::path::PathBuf::from(home)
        .join(".magellan")
        .join(format!("{}.db", stem))
}
```

- [ ] **Step 4: Run tests to confirm they pass**

```bash
cargo test -p forge-core storage::tests 2>&1 | tail -10
```

Expected: `test storage::tests::test_default_db_path_uses_home_dot_magellan ... ok`

- [ ] **Step 5: Commit**

```bash
git add forge_core/src/storage/mod.rs
git commit -m "feat(forge-core): add default_db_path resolving ~/.magellan/<stem>.db"
```

---

## Task 2: Add `db_path`/`db_dir` overrides to `ForgeBuilder`

**Files:**
- Modify: `forge_core/src/lib.rs`

- [ ] **Step 1: Write the failing test**

In `forge_core/src/lib.rs`, add inside the existing `#[cfg(test)] mod tests` block:

```rust
#[tokio::test]
async fn test_forge_builder_db_path_override() {
    let temp_dir = tempfile::tempdir().unwrap();
    let custom_db = temp_dir.path().join("custom.db");

    let forge = ForgeBuilder::new()
        .path(temp_dir.path())
        .db_path(custom_db.clone())
        .build()
        .await
        .unwrap();

    assert_eq!(forge.store.db_path, custom_db);
}

#[tokio::test]
async fn test_forge_builder_db_dir_override() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_dir = temp_dir.path().join("custom_dir");
    std::fs::create_dir_all(&db_dir).unwrap();

    let project_dir = temp_dir.path().join("my-project");
    std::fs::create_dir_all(&project_dir).unwrap();

    let forge = ForgeBuilder::new()
        .path(&project_dir)
        .db_dir(db_dir.clone())
        .build()
        .await
        .unwrap();

    assert_eq!(forge.store.db_path, db_dir.join("my-project.db"));
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cargo test -p forge-core "test_forge_builder_db_path_override\|test_forge_builder_db_dir_override" 2>&1 | tail -20
```

Expected: FAIL with `no method named 'db_path' found for struct 'ForgeBuilder'`

- [ ] **Step 3: Add fields and methods to `ForgeBuilder`**

Find the `ForgeBuilder` struct definition (around line 187) and replace it:

```rust
#[derive(Clone, Default)]
pub struct ForgeBuilder {
    path: Option<std::path::PathBuf>,
    backend_kind: Option<BackendKind>,
    db_path: Option<std::path::PathBuf>,
    db_dir: Option<std::path::PathBuf>,
}
```

Add these two builder methods after the existing `backend_kind` method:

```rust
/// Sets an explicit database path, overriding the default ~/.magellan/<stem>.db.
pub fn db_path(self, path: std::path::PathBuf) -> Self {
    Self {
        db_path: Some(path),
        ..self
    }
}

/// Sets the database directory; stem is still derived from the project root.
pub fn db_dir(self, dir: std::path::PathBuf) -> Self {
    Self {
        db_dir: Some(dir),
        ..self
    }
}
```

- [ ] **Step 4: Update `ForgeBuilder::build()` to resolve DB path**

Replace the existing `build()` method body:

```rust
pub async fn build(self) -> anyhow::Result<Forge> {
    let path = self.path.ok_or_else(|| anyhow!("path is required"))?;
    let backend = self.backend_kind.unwrap_or_default();

    let resolved_db = if let Some(explicit) = self.db_path {
        explicit
    } else if let Some(dir) = self.db_dir {
        let stem = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("graph");
        dir.join(format!("{}.db", stem))
    } else {
        storage::default_db_path(&path)
    };

    let store = std::sync::Arc::new(
        storage::UnifiedGraphStore::open_with_path(&path, &resolved_db, backend).await?,
    );

    Ok(Forge { store })
}
```

- [ ] **Step 5: Run tests to confirm they pass**

```bash
cargo test -p forge-core "test_forge_builder_db_path_override\|test_forge_builder_db_dir_override" 2>&1 | tail -10
```

Expected: both tests `ok`

- [ ] **Step 6: Commit**

```bash
git add forge_core/src/lib.rs
git commit -m "feat(forge-core): ForgeBuilder db_path/db_dir overrides for unified DB path"
```

---

## Task 3: Fix test isolation — migrate all tests to `ForgeBuilder` with explicit `db_path`

All existing tests call `Forge::open(temp_dir.path())` which after Task 4 will resolve to `~/.magellan/<random-tempdir-stem>.db`, polluting the global directory. Fix all tests now so they use an explicit `db_path` inside the tempdir.

**Files:**
- Modify: `forge_core/src/lib.rs`
- Modify: `forge_core/src/graph/mod.rs`

- [ ] **Step 1: Add `test_forge` helper in `graph/mod.rs` test module**

At the bottom of `forge_core/src/graph/mod.rs`, inside `#[cfg(test)] mod tests`, add:

```rust
/// Creates a Forge instance for tests with the DB inside the given directory.
/// Never writes to ~/.magellan/.
async fn test_forge(dir: &std::path::Path) -> crate::Forge {
    crate::ForgeBuilder::new()
        .path(dir)
        .db_path(dir.join("test-graph.db"))
        .build()
        .await
        .unwrap()
}
```

- [ ] **Step 2: Update all tests in `graph/mod.rs` to use `test_forge`**

Replace every occurrence of:
```rust
let store = Arc::new(
    UnifiedGraphStore::open(temp_dir.path(), BackendKind::SQLite)
        .await
        .unwrap(),
);
let module = GraphModule::new(store);
```

with:
```rust
let forge = test_forge(temp_dir.path()).await;
let module = forge.graph();
```

There are 5 such occurrences (one per test). Also replace `GraphModule::new(Arc::clone(&store))` patterns.

- [ ] **Step 3: Update tests in `lib.rs` to use `ForgeBuilder` with `db_path`**

Replace every occurrence of:
```rust
Forge::open(temp_dir.path()).await.unwrap()
```

with:
```rust
ForgeBuilder::new()
    .path(temp_dir.path())
    .db_path(temp_dir.path().join("test-graph.db"))
    .build()
    .await
    .unwrap()
```

Also replace the `UnifiedGraphStore::open(temp_dir.path(), BackendKind::default())` pattern in `lib.rs` tests:
```rust
let store = std::sync::Arc::new(
    storage::UnifiedGraphStore::open_with_path(
        temp_dir.path(),
        temp_dir.path().join("test-graph.db"),
        BackendKind::default(),
    )
    .await
    .unwrap(),
);
```

- [ ] **Step 4: Run full test suite to confirm all tests pass**

```bash
cargo test -p forge-core --lib 2>&1 | tail -20
```

Expected: all tests pass (same count as before)

- [ ] **Step 5: Commit**

```bash
git add forge_core/src/graph/mod.rs forge_core/src/lib.rs
git commit -m "test(forge-core): migrate tests to ForgeBuilder with explicit db_path"
```

---

## Task 4: Update `UnifiedGraphStore::open` to use `default_db_path`

With tests now using explicit `db_path`, it is safe to change the default resolution.

**Files:**
- Modify: `forge_core/src/storage/mod.rs`

- [ ] **Step 1: Replace the path derivation in `UnifiedGraphStore::open`**

Find this block (around line 150):
```rust
let db_path = codebase
    .join(".forge")
    .join(backend_kind.default_filename());
```

Replace with:
```rust
let db_path = default_db_path(codebase);
```

- [ ] **Step 2: Run tests to confirm nothing broke**

```bash
cargo test -p forge-core --lib 2>&1 | tail -20
```

Expected: same pass count as before (tests use explicit `db_path` so they don't hit this code path)

- [ ] **Step 3: Commit**

```bash
git add forge_core/src/storage/mod.rs
git commit -m "feat(forge-core): UnifiedGraphStore::open now resolves ~/.magellan/<stem>.db"
```

---

## Task 5: Rewrite `find_symbol` with `search_symbols_by_name`

**Files:**
- Modify: `forge_core/src/graph/mod.rs`

- [ ] **Step 1: Write the failing test**

In `graph/mod.rs` test module, add:

```rust
#[tokio::test]
async fn test_find_symbol_returns_empty_on_missing_db() {
    let temp_dir = tempfile::tempdir().unwrap();
    // db_path points to a non-existent file
    let forge = ForgeBuilder::new()
        .path(temp_dir.path())
        .db_path(temp_dir.path().join("nonexistent.db"))
        .build()
        .await
        .unwrap();
    let module = forge.graph();
    let result = module.find_symbol("anything").await.unwrap();
    assert_eq!(result, vec![]);
}
```

- [ ] **Step 2: Run test to confirm it passes (it's a guard test)**

```bash
cargo test -p forge-core test_find_symbol_returns_empty_on_missing_db 2>&1 | tail -5
```

Expected: `ok` (the DB doesn't exist so early return kicks in)

- [ ] **Step 3: Replace `find_symbol` implementation**

In `forge_core/src/graph/mod.rs`, replace the entire `find_symbol` method with:

```rust
pub async fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>> {
    use magellan::CodeGraph;
    use std::sync::Arc;

    let db_path = &self.store.db_path;
    if !db_path.exists() {
        return Ok(Vec::new());
    }

    let graph = CodeGraph::open(db_path).map_err(|e| {
        crate::error::ForgeError::DatabaseError(format!("Failed to open magellan graph: {}", e))
    })?;

    let results = graph.search_symbols_by_name(name).map_err(|e| {
        crate::error::ForgeError::DatabaseError(format!("Symbol search failed: {}", e))
    })?;

    Ok(results
        .into_iter()
        .map(|r| Symbol {
            id: SymbolId(r.entity_id),
            name: Arc::from(r.name.clone()),
            fully_qualified_name: Arc::from(r.name.clone()),
            kind: parse_symbol_kind_str(&r.kind),
            language: map_magellan_language(std::path::Path::new(&r.file_path)),
            location: Location {
                file_path: std::path::PathBuf::from(&r.file_path),
                byte_start: r.byte_start as u32,
                byte_end: r.byte_end as u32,
                line_number: 0,
            },
            parent_id: None,
            metadata: serde_json::Value::Null,
        })
        .collect())
}
```

Also add this private helper at the bottom of `graph/mod.rs` (outside `impl GraphModule`):

```rust
fn parse_symbol_kind_str(kind: &str) -> crate::types::SymbolKind {
    use crate::types::SymbolKind;
    match kind {
        "Function" | "function" | "fn" => SymbolKind::Function,
        "Method" | "method" => SymbolKind::Method,
        "Struct" | "struct" => SymbolKind::Struct,
        "Enum" | "enum" => SymbolKind::Enum,
        "Trait" | "trait" => SymbolKind::Trait,
        "Impl" | "impl" => SymbolKind::Impl,
        "Module" | "module" | "mod" => SymbolKind::Module,
        "TypeAlias" | "type" => SymbolKind::TypeAlias,
        "Constant" | "const" => SymbolKind::Constant,
        "Static" | "static" => SymbolKind::Static,
        "Macro" | "macro" => SymbolKind::Macro,
        _ => SymbolKind::Function,
    }
}
```

- [ ] **Step 4: Remove cfg guards from `find_symbol` and `map_magellan_language`**

The old `find_symbol` had two `cfg` blocks — delete them (new implementation has none).

The `map_magellan_language` helper at the bottom of `graph/mod.rs` is currently wrapped in `#[cfg(feature = "magellan")]`. Remove that gate so the function is always compiled:

```rust
// Before:
#[cfg(feature = "magellan")]
fn map_magellan_language(file_path: &std::path::Path) -> crate::types::Language { ... }

// After (remove the cfg attribute):
fn map_magellan_language(file_path: &std::path::Path) -> crate::types::Language { ... }
```

- [ ] **Step 5: Run tests**

```bash
cargo test -p forge-core --lib 2>&1 | tail -20
```

Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add forge_core/src/graph/mod.rs
git commit -m "feat(forge-core): find_symbol uses magellan search_symbols_by_name (indexed, O(1))"
```

---

## Task 6: Rewrite `references` with `cross_file_references_to`

**Files:**
- Modify: `forge_core/src/graph/mod.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn test_references_returns_empty_on_missing_db() {
    let temp_dir = tempfile::tempdir().unwrap();
    let forge = ForgeBuilder::new()
        .path(temp_dir.path())
        .db_path(temp_dir.path().join("nonexistent.db"))
        .build()
        .await
        .unwrap();
    let module = forge.graph();
    let result = module.references("any_symbol").await.unwrap();
    assert_eq!(result, vec![]);
}
```

- [ ] **Step 2: Run to confirm test passes (guard test)**

```bash
cargo test -p forge-core test_references_returns_empty_on_missing_db 2>&1 | tail -5
```

Expected: `ok`

- [ ] **Step 3: Replace `references` implementation**

Replace the entire `references` method:

```rust
pub async fn references(&self, name: &str) -> Result<Vec<Reference>> {
    use magellan::CodeGraph;

    let db_path = &self.store.db_path;
    if !db_path.exists() {
        return Ok(Vec::new());
    }

    let graph = CodeGraph::open(db_path).map_err(|e| {
        crate::error::ForgeError::DatabaseError(format!("Failed to open magellan graph: {}", e))
    })?;

    let cross_refs = magellan::cross_file_references_to(&graph, name).map_err(|e| {
        crate::error::ForgeError::DatabaseError(format!("Reference query failed: {}", e))
    })?;

    Ok(cross_refs
        .into_iter()
        .map(|r| Reference {
            from: SymbolId(0),
            to: SymbolId(0),
            from_name: Some(r.from_symbol_id),
            to_name: Some(r.to_symbol_id),
            kind: ReferenceKind::TypeReference,
            location: Location {
                file_path: std::path::PathBuf::from(&r.file_path),
                byte_start: r.byte_start as u32,
                byte_end: r.byte_end as u32,
                line_number: r.line_number,
            },
        })
        .collect())
}
```

Note: `magellan::cross_file_references_to` is exported from magellan's crate root (`pub use graph::query::cross_file_references_to`). Call it as a free function.

- [ ] **Step 4: Remove all `#[cfg]` guards and the old fallback arm from `references`**

The old method had `#[cfg(feature = "magellan")]` and `#[cfg(not(...))]` blocks. Delete them — the new method has no cfg guards.

- [ ] **Step 5: Run tests**

```bash
cargo test -p forge-core --lib 2>&1 | tail -20
```

Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add forge_core/src/graph/mod.rs
git commit -m "feat(forge-core): references uses magellan cross_file_references_to"
```

---

## Task 7: Rewrite `callers_of` — remove cfg fallback

The existing `callers_of` already calls `magellan::CodeGraph::callers_of_symbol` inside `#[cfg(feature = "magellan")]`. The work here is to remove the `#[cfg]` guards and unify to a single code path, using the correct DB path.

**Files:**
- Modify: `forge_core/src/graph/mod.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn test_callers_of_returns_empty_on_missing_db() {
    let temp_dir = tempfile::tempdir().unwrap();
    let forge = ForgeBuilder::new()
        .path(temp_dir.path())
        .db_path(temp_dir.path().join("nonexistent.db"))
        .build()
        .await
        .unwrap();
    let module = forge.graph();
    let result = module.callers_of("any_fn").await.unwrap();
    assert_eq!(result, vec![]);
}
```

- [ ] **Step 2: Run to confirm test passes**

```bash
cargo test -p forge-core test_callers_of_returns_empty_on_missing_db 2>&1 | tail -5
```

Expected: `ok`

- [ ] **Step 3: Replace `callers_of` implementation**

Replace the entire `callers_of` method (removes both `cfg` arms and the `GraphQueryEngine` fallback):

```rust
pub async fn callers_of(&self, name: &str) -> Result<Vec<Reference>> {
    use magellan::CodeGraph;

    let db_path = &self.store.db_path;
    if !db_path.exists() {
        return Ok(Vec::new());
    }

    let mut graph = CodeGraph::open(db_path).map_err(|e| {
        crate::error::ForgeError::DatabaseError(format!("Failed to open magellan graph: {}", e))
    })?;

    let file_nodes = graph.all_file_nodes_readonly().map_err(|e| {
        crate::error::ForgeError::DatabaseError(format!("Failed to list files: {}", e))
    })?;

    let mut callers = Vec::new();
    for (file_path, _) in file_nodes {
        if let Ok(call_facts) = graph.callers_of_symbol(&file_path, name) {
            for fact in call_facts {
                callers.push(Reference {
                    from: SymbolId(0),
                    to: SymbolId(0),
                    from_name: Some(fact.caller.clone()),
                    to_name: Some(fact.callee.clone()),
                    kind: ReferenceKind::Call,
                    location: Location {
                        file_path: std::path::PathBuf::from(&fact.file_path),
                        byte_start: fact.byte_start as u32,
                        byte_end: fact.byte_end as u32,
                        line_number: fact.start_line,
                    },
                });
            }
        }
    }

    Ok(callers)
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p forge-core --lib 2>&1 | tail -20
```

Expected: all tests pass

- [ ] **Step 5: Commit**

```bash
git add forge_core/src/graph/mod.rs
git commit -m "feat(forge-core): callers_of unified to single magellan code path"
```

---

## Task 8: Implement `cycles()` with `condense_call_graph`

**Files:**
- Modify: `forge_core/src/graph/mod.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn test_cycles_returns_empty_when_db_absent() {
    let temp_dir = tempfile::tempdir().unwrap();
    let forge = ForgeBuilder::new()
        .path(temp_dir.path())
        .db_path(temp_dir.path().join("nonexistent.db"))
        .build()
        .await
        .unwrap();
    let module = forge.graph();
    let result = module.cycles().await.unwrap();
    assert_eq!(result, vec![]);
}
```

- [ ] **Step 2: Run to confirm test passes**

```bash
cargo test -p forge-core test_cycles_returns_empty_when_db_absent 2>&1 | tail -5
```

Expected: `ok`

- [ ] **Step 3: Replace `cycles()` implementation**

Replace the `cycles()` method (currently returns `Ok(Vec::new())`):

```rust
pub async fn cycles(&self) -> Result<Vec<Cycle>> {
    use magellan::CodeGraph;

    let db_path = &self.store.db_path;
    if !db_path.exists() {
        return Ok(Vec::new());
    }

    let graph = CodeGraph::open(db_path).map_err(|e| {
        crate::error::ForgeError::DatabaseError(format!("Failed to open magellan graph: {}", e))
    })?;

    let condensation = graph.condense_call_graph().map_err(|e| {
        crate::error::ForgeError::DatabaseError(format!("Condensation failed: {}", e))
    })?;

    // Supernodes with >1 member are SCCs — actual cycles in the call graph
    let cycles = condensation
        .graph
        .supernodes
        .into_iter()
        .filter(|sn| sn.members.len() > 1)
        .map(|sn| Cycle {
            members: sn.members.into_iter().map(|m| SymbolId(m.id as i64)).collect(),
        })
        .collect();

    Ok(cycles)
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p forge-core --lib 2>&1 | tail -20
```

Expected: all tests pass

- [ ] **Step 5: Commit**

```bash
git add forge_core/src/graph/mod.rs
git commit -m "feat(forge-core): cycles() now delegates to magellan condense_call_graph"
```

---

## Task 9: Fix `impact_analysis` and move `ImpactedSymbol` inline

**Files:**
- Modify: `forge_core/src/graph/mod.rs`

- [ ] **Step 1: Move `ImpactedSymbol` struct into `graph/mod.rs`**

Add this struct to `graph/mod.rs` (just before `impl GraphModule`):

```rust
/// Symbol impacted by a change, with its hop distance.
#[derive(Debug, Clone)]
pub struct ImpactedSymbol {
    pub symbol_id: i64,
    pub name: String,
    pub kind: String,
    pub file_path: String,
    pub hop_distance: u32,
    pub edge_type: String,
}
```

- [ ] **Step 2: Replace `impact_analysis` implementation**

Replace the `impact_analysis` method (currently calls `GraphQueryEngine::find_impacted_symbols`):

```rust
pub async fn impact_analysis(
    &self,
    symbol_name: &str,
    max_hops: Option<u32>,
) -> Result<Vec<ImpactedSymbol>> {
    use magellan::CodeGraph;
    use std::collections::{HashMap, HashSet, VecDeque};

    let db_path = &self.store.db_path;
    if !db_path.exists() {
        return Ok(Vec::new());
    }

    let mut graph = CodeGraph::open(db_path).map_err(|e| {
        crate::error::ForgeError::DatabaseError(format!("Failed to open magellan graph: {}", e))
    })?;

    let hops = max_hops.unwrap_or(2);

    // Seed: find the starting symbol
    let seeds = graph.search_symbols_by_name(symbol_name).map_err(|e| {
        crate::error::ForgeError::DatabaseError(format!("Symbol search failed: {}", e))
    })?;

    if seeds.is_empty() {
        return Ok(Vec::new());
    }

    // BFS over callers-of to find what would be impacted
    let file_nodes = graph.all_file_nodes_readonly().map_err(|e| {
        crate::error::ForgeError::DatabaseError(format!("Failed to list files: {}", e))
    })?;

    // Build call map: callee_name -> Vec<(caller_name, file, byte_start, byte_end, line)>
    let mut callee_to_callers: HashMap<String, Vec<(String, String, usize, usize, usize)>> =
        HashMap::new();
    for (file_path, _) in &file_nodes {
        if let Ok(calls) = graph.callers_of_symbol(file_path, symbol_name) {
            for c in calls {
                callee_to_callers
                    .entry(c.callee.clone())
                    .or_default()
                    .push((
                        c.caller.clone(),
                        c.file_path.clone(),
                        c.byte_start,
                        c.byte_end,
                        c.start_line,
                    ));
            }
        }
    }

    // BFS from the seed symbol upward through callers
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, u32)> = VecDeque::new();
    let mut results: Vec<ImpactedSymbol> = Vec::new();

    for seed in &seeds {
        queue.push_back((seed.name.clone(), 0));
        visited.insert(seed.name.clone());
    }

    while let Some((sym, depth)) = queue.pop_front() {
        if depth >= hops {
            continue;
        }
        if let Some(callers) = callee_to_callers.get(&sym) {
            for (caller_name, file_path, _bs, _be, _line) in callers {
                if visited.insert(caller_name.clone()) {
                    results.push(ImpactedSymbol {
                        symbol_id: 0,
                        name: caller_name.clone(),
                        kind: "Function".to_string(),
                        file_path: file_path.clone(),
                        hop_distance: depth + 1,
                        edge_type: "call".to_string(),
                    });
                    queue.push_back((caller_name.clone(), depth + 1));
                }
            }
        }
    }

    Ok(results)
}
```

- [ ] **Step 3: Update the import in `graph/mod.rs` to remove `queries::ImpactedSymbol`**

Find `use queries::GraphQueryEngine;` and any `use queries::ImpactedSymbol` — remove both. The struct is now inline.

- [ ] **Step 4: Run tests**

```bash
cargo test -p forge-core --lib 2>&1 | tail -20
```

Expected: all tests pass

- [ ] **Step 5: Commit**

```bash
git add forge_core/src/graph/mod.rs
git commit -m "feat(forge-core): impact_analysis replaced with magellan BFS, ImpactedSymbol moved inline"
```

---

## Task 10: Delete `graph/queries.rs` and clean up all cfg guards

**Files:**
- Delete: `forge_core/src/graph/queries.rs`
- Modify: `forge_core/src/graph/mod.rs`

- [ ] **Step 1: Remove `pub mod queries;` from `graph/mod.rs`**

Find and delete this line near the top of `graph/mod.rs`:
```rust
pub mod queries;
```

- [ ] **Step 2: Delete `graph/queries.rs`**

```bash
rm /home/feanor/Projects/forge/forge_core/src/graph/queries.rs
```

- [ ] **Step 3: Remove remaining `#[cfg]` guards from `graph/mod.rs`**

Search for any remaining `#[cfg(feature = "magellan")]` and `#[cfg(not(feature = "magellan"))]` blocks in `graph/mod.rs`:

```bash
grep -n "cfg(feature" /home/feanor/Projects/forge/forge_core/src/graph/mod.rs
```

For each found:
- If it wraps the `index_references_recursive` helper and `index()` method: keep the `#[cfg(feature = "magellan")]` on `index_references_recursive` since it uses `magellan::CodeGraph` directly — or remove cfg and unconditionally use magellan since the feature is always on.
- Delete all `#[cfg(not(feature = "magellan"))]` blocks entirely.

- [ ] **Step 4: Attempt to build**

```bash
cargo build -p forge-core 2>&1 | head -40
```

Fix any remaining compilation errors (likely unused imports from `queries`).

- [ ] **Step 5: Run full test suite**

```bash
cargo test -p forge-core --lib 2>&1 | tail -30
```

Expected: same or higher pass count as before

- [ ] **Step 6: Run clippy**

```bash
cargo clippy -p forge-core -- -D warnings 2>&1 | head -40
```

Fix any warnings before continuing.

- [ ] **Step 7: Commit**

```bash
git add forge_core/src/graph/mod.rs
git rm forge_core/src/graph/queries.rs
git commit -m "refactor(forge-core): delete GraphQueryEngine, remove all cfg fallback arms"
```

---

## Task 11: Final verification

- [ ] **Step 1: Run full test suite**

```bash
cargo test -p forge-core 2>&1 | tail -30
```

Expected: all tests pass, no failures

- [ ] **Step 2: Run clippy**

```bash
cargo clippy -p forge-core -- -D warnings 2>&1
```

Expected: no warnings

- [ ] **Step 3: Confirm `queries.rs` is gone**

```bash
ls forge_core/src/graph/
```

Expected: `mod.rs` only (no `queries.rs`)

- [ ] **Step 4: Confirm no cfg fallback arms remain in graph module**

```bash
grep -n "cfg(not(feature" forge_core/src/graph/mod.rs
```

Expected: no output

- [ ] **Step 5: Confirm DB path resolution**

```bash
cargo test -p forge-core storage::tests 2>&1 | tail -10
```

Expected: `test_default_db_path_uses_home_dot_magellan ... ok`

- [ ] **Step 6: Commit verification note**

```bash
git commit --allow-empty -m "chore(forge-core): magellan unification complete — queries.rs removed, cfg fallbacks gone"
```

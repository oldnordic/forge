# forge_core Graph Module — Magellan Unification Design

**Date:** 2026-05-22
**Scope:** `forge_core` graph module only (search/cfg/edit follow as separate specs)
**Status:** Approved

---

## Goal

Replace the half-baked graph module integration with a clean, correct dependency on
`magellan` as the single engine for all symbol and reference queries. Remove the
parallel `GraphQueryEngine` implementation. Unify the database path so forge, magellan,
and future cross-project tooling all use the same file.

---

## Context

`forge_core` already lists `magellan` as an optional Cargo dependency (enabled by
default via the `tools` feature). The `graph/mod.rs` module already has
`#[cfg(feature = "magellan")]` code paths. But the integration is incorrect in
several ways:

- DB path is `.forge/graph.db` — not where magellan creates its database
- `find_symbol` does an O(n) scan over all file nodes instead of a name-indexed lookup
- `cycles()` always returns an empty vec
- `impact_analysis` uses `GraphQueryEngine`, a local reimplementation on sqlitegraph
- `GraphQueryEngine` in `graph/queries.rs` is a full parallel implementation of queries
  that magellan already provides correctly
- `#[cfg(not(feature = "magellan"))]` fallback arms exist alongside the magellan paths

---

## Database Path Convention

All project databases live in `~/.magellan/<stem>.db` where `<stem>` is the final
directory component of the project root.

Examples:
- `Forge::open("/home/user/Projects/geographdb-core")` → `~/.magellan/geographdb-core.db`
- `Forge::open("/home/user/Projects/splice")` → `~/.magellan/splice.db`

This matches the convention used by magellan itself and enables the cross-project
registry: any tool that looks in `~/.magellan/` finds all indexed project databases.

### Overrides

`ForgeBuilder` exposes two override methods:

```rust
ForgeBuilder::new()
    .path("./my-project")
    .db_path(PathBuf::from("/custom/path/graph.db"))  // full override
    .build().await?

ForgeBuilder::new()
    .path("./my-project")
    .db_dir(PathBuf::from("/custom/dir/"))            // dir override, stem derived
    .build().await?
```

A `default_db_path(project_root: &Path) -> PathBuf` helper handles path derivation.
It reads `dirs::home_dir()` (or falls back to `$HOME`) and appends `.magellan/<stem>.db`.

---

## `UnifiedGraphStore` Changes

`UnifiedGraphStore` keeps its sqlitegraph connection (still needed by the knowledge
module). The only change is the DB path it resolves to: `~/.magellan/<stem>.db`
instead of `.forge/graph.db`.

Field changes:
- `db_path: PathBuf` — updated to the global path
- `codebase_path: PathBuf` — unchanged (project root, used for file-relative ops)

No other structural changes to `UnifiedGraphStore`.

---

## `GraphModule` Method Rewrites

Each method opens `magellan::CodeGraph` on the resolved DB path. `CodeGraph` manages
its own SQLite connection; forge does not hold a persistent `CodeGraph` handle (keeps
the design stateless, avoids connection lifecycle complexity).

### `find_symbol(name: &str)`

Replace the O(n) file-node scan with a direct name-indexed lookup via
`magellan::CodeGraph`. Convert `magellan::SymbolInfo` → `forge_core::types::Symbol`
using the existing `map_magellan_language` helper.

### `callers_of(name: &str)`

Remove the per-file loop. Use magellan's cross-file caller resolution directly.
Convert results to `forge_core::types::Reference`.

### `references(name: &str)`

Replace per-file loop with `magellan::cross_file_references_to(name)` — already
in magellan's public API (`pub use graph::query::cross_file_references_to`).

### `cycles()`

Remove the `Ok(Vec::new())` stub. Magellan exposes `CondensationResult` and `Cycle`
from its public API (`pub use graph::{CondensationGraph, CondensationResult, Cycle}`).
Open `CodeGraph`, call the condensation/SCC API, and convert results to
`forge_core::types::Cycle`. If magellan's `Cycle` type is compatible enough, consider
re-exporting it directly rather than maintaining a parallel type.

### `impact_analysis(symbol_name, max_hops)`

Remove `GraphQueryEngine` call. Use magellan's context/impact query API.
Return `Vec<ImpactedSymbol>` — `ImpactedSymbol` struct moves from `graph/queries.rs`
to `graph/mod.rs` since it is part of the public API surface.

### `index()`

Fix the DB path from `.forge/graph.db` to the resolved `~/.magellan/<stem>.db`.
No other changes needed.

---

## Deletions

| What | Why |
|------|-----|
| `src/graph/queries.rs` | Entire file — `GraphQueryEngine` and all helpers are replaced by magellan |
| All `#[cfg(not(feature = "magellan"))]` arms in `graph/mod.rs` | Fallback paths go away; magellan is always-on in `default` |
| `graph/mod.rs` import of `queries::GraphQueryEngine` | No longer needed |

The `ImpactedSymbol` struct from `queries.rs` is kept but moved inline into
`graph/mod.rs` (it is a public return type — removing it would be a breaking change).

---

## Cargo.toml

No dependency changes needed. `magellan` is already listed as optional and enabled by
the `default` feature. The `which` dep stays (used elsewhere for tool-not-found errors).

The only possible addition: `dirs = "5"` for `home_dir()` resolution, if not already
present. Check `Cargo.toml` at implementation time; if `dirs` is absent, use
`std::env::var("HOME")` with a documented fallback.

---

## Test Helper

Existing tests use `tempfile::tempdir()` and pass an explicit `db_path` via
`ForgeBuilder`. After this change, `Forge::open(tmpdir)` would try to write to
`~/.magellan/` during tests — wrong. Tests must use `ForgeBuilder` with `db_path`
override pointing inside the tempdir.

Add a test helper `open_test_forge(dir: &Path) -> Forge` that:
1. Resolves `db_path` to `dir.join("graph.db")` (inside tempdir, not global)
2. Creates the magellan schema via `magellan::CodeGraph::open(db_path)`
3. Returns a `Forge` instance ready for assertions

All existing tests in `graph/mod.rs` are updated to call this helper instead of
`Forge::open(temp_dir.path())`.

---

## Out of Scope

- Search module (llmgrep integration) — separate spec
- CFG module (mirage integration) — separate spec
- Edit module (splice integration) — separate spec
- Cross-project registry — separate spec (foundation in `magellan/src/registry_cmd.rs`)
- `knowledge` module — no changes

---

## Success Criteria

- `cargo test -p forge-core` passes with no `#[cfg(not(feature = "magellan"))]` paths
- `cargo clippy -p forge-core -- -D warnings` clean
- `src/graph/queries.rs` does not exist
- `Forge::open("./geographdb-core").graph().find_symbol("main")` returns results from
  `~/.magellan/geographdb-core.db` (if previously indexed by magellan)
- `Forge::open("./geographdb-core").graph().cycles()` returns actual data, not empty vec

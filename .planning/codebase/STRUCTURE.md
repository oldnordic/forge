# Codebase Structure

**Analysis Date:** 2026-02-13

## Directory Layout

```
forge/
├── Cargo.toml                      # Workspace configuration (3 members)
├── README.md                       # Project overview and quick start
├── LICENSE                         # GPL-3.0-or-later
├── CHANGELOG.md                    # Version history
├── AGENTS.md                       # Agent usage documentation
├── .gitignore                      # Git ignore rules
│
├── docs/                          # User-facing documentation
│   ├── ARCHITECTURE.md             # System architecture (design doc)
│   ├── API.md                     # API reference guide
│   ├── PHILOSOPHY.md              # Design philosophy and rationale
│   ├── CONTRIBUTING.md             # Contribution guidelines
│   ├── DEVELOPMENT_WORKFLOW.md      # Development workflow
│   └── ROADMAP.md                 # Project roadmap
│
├── .planning/                     # Project planning documents
│   ├── milestones/
│   │   ├── v0.1-REQUIREMENTS.md   # v0.1 requirements
│   │   └── v0.1-ROADMAP.md       # v0.1 milestone roadmap
│   ├── phases/
│   │   ├── 01-project-organization/
│   │   │   ├── 01-01-PLAN.md
│   │   │   └── 01-02-PLAN.md
│   │   ├── 02-core-sdk/
│   │   │   └── 02-01-PLAN.md
│   │   ├── 03-runtime-layer/      # (planned)
│   │   └── 04-agent-layer/        # (planned)
│   └── codebase/
│       ├── ARCHITECTURE.md         # This file (generated)
│       └── STRUCTURE.md           # This file
│
├── forge_core/                    # Core SDK library (required)
│   ├── Cargo.toml                 # Package manifest
│   └── src/
│
├── forge_runtime/                 # Runtime layer (optional, stub)
│   ├── Cargo.toml
│   └── src/
│
├── forge_agent/                   # Agent layer (optional, stub)
│   ├── Cargo.toml
│   └── src/
│
├── tests/                         # Workspace-level integration tests
│   ├── integration/               # Integration test suites
│   └── fixtures/                 # Test fixtures and data
│
└── target/                        # Cargo build output (gitignored)
```

## Directory Purposes

**docs/**:**
- Purpose: User-facing documentation
- Contains: Architecture docs, API reference, philosophy
- Key files: `ARCHITECTURE.md`, `PHILOSOPHY.md`, `ROADMAP.md`

**.planning/**:**
- Purpose: Project management and phase planning
- Contains: Milestone requirements, phase plans, codebase analysis
- Subdirectories: `milestones/`, `phases/`, `codebase/`

**forge_core/**:**
- Purpose: Core SDK library providing programmatic interface to code intelligence
- Contains: Module implementations, runtime components, type definitions
- Key files: `src/lib.rs` (entry point), `src/types.rs`, `src/error.rs`

**forge_runtime/**:**
- Purpose: Runtime services for hot-reload and performance
- Status: Stub implementation in v0.1
- Contains: `src/lib.rs` with `RuntimeConfig`, `ForgeRuntime`

**forge_agent/**:**
- Purpose: Deterministic six-phase AI loop for automated code operations
- Contains: Agent orchestrator, phase implementations (observe, policy, planner, mutate, verify, commit)
- Key files: `src/lib.rs`, `src/observe.rs`, `src/policy.rs`, `src/planner.rs`

**tests/**:**
- Purpose: Workspace-level integration tests
- Contains: Integration test suites, test fixtures, common test utilities
- Subdirectories: `integration/`, `fixtures/`, `common/`

## Key File Locations

**Entry Points:**
- `forge_core/src/lib.rs`: Main SDK entry point (Forge struct)
- `forge_agent/src/lib.rs`: Agent orchestrator
- `forge_agent/src/cli.rs`: Command-line interface
- `forge_runtime/src/lib.rs`: Runtime entry point (stub)

**Configuration:**
- `Cargo.toml`: Workspace root defining 3 members
- `forge_core/Cargo.toml`: Core SDK package manifest
- `forge_agent/Cargo.toml`: Agent package manifest
- `forge_runtime/Cargo.toml`: Runtime package manifest

**Core Logic:**
- `forge_core/src/types.rs`: Core type definitions (SymbolId, BlockId, PathId, Symbol, Reference, etc.)
- `forge_core/src/error.rs`: Unified error handling (ForgeError enum)
- `forge_core/src/storage/mod.rs`: Storage abstraction (UnifiedGraphStore)
- `forge_core/src/runtime.rs`: Runtime orchestration (combines watcher, indexer, cache, pool)

**Testing:**
- `tests/integration/mod.rs`: Integration test entry point
- `tests/common/mod.rs`: Shared test utilities
- `forge_core/src/lib.rs`: Inline unit tests in each module

**Documentation:**
- `.planning/codebase/ARCHITECTURE.md`: This architecture document
- `.planning/codebase/STRUCTURE.md`: This structure document
- `docs/ARCHITECTURE.md`: User-facing architecture documentation

## Naming Conventions

**Files:**
- Modules: `mod.rs` (e.g., `graph/mod.rs`, `search/mod.rs`)
- Tests: `{module}_tests.rs` (e.g., `accessor_tests.rs`, `builder_tests.rs`)
- Single-file modules: `{name}.rs` (e.g., `types.rs`, `error.rs`, `lib.rs`)

**Directories:**
- Module directories: lowercase singular (e.g., `graph/`, `search/`, `cfg/`)
- Workspace members: lowercase with underscore (e.g., `forge_core`, `forge_agent`)

**Types:**
- Structs: PascalCase (e.g., `Symbol`, `Reference`, `UnifiedGraphStore`)
- Enums: PascalCase (e.g., `SymbolKind`, `ReferenceKind`, `PathKind`)
- Newtypes: PascalCase with tuple struct (e.g., `SymbolId(pub i64)`, `BlockId(pub i64)`)

**Functions:**
- Public: snake_case (e.g., `symbols_in_file`, `find_symbol`, `references_to_symbol`)
- Private: snake_case with leading underscore (e.g., `query_symbols_impl`)

## Where to Add New Code

**New Feature:**
- Primary code: `forge_core/src/{module}.rs` or `forge_core/src/{module}/mod.rs`
- Tests: Inline in module file or `forge_core/src/{module}/mod.rs`

**New Component/Module:**
- Implementation: `forge_core/src/{module_name}/mod.rs`
- If single file: `forge_core/src/{module_name}.rs`

**Utilities:**
- Shared helpers: `forge_core/src/{utility_name}.rs`
- Test utilities: `tests/common/mod.rs`

**Agent Phase:**
- New phase: `forge_agent/src/{phase_name}.rs`
- Updates: Modify `forge_agent/src/lib.rs` to wire in new phase

**Integration Tests:**
- New test suite: `tests/integration/{suite_name}_tests.rs`
- Fixtures: `tests/fixtures/{fixture_name}/`

**Documentation:**
- Phase plan: `.planning/phases/{XX-phase-name}/XX-XX-PLAN.md`
- Design docs: `docs/{document_name}.md`

## Special Directories

**.forge/**
- Purpose: Graph database storage
- Generated: Yes (by UnifiedGraphStore)
- Committed: No (in .gitignore)
- Contains: `graph.db` - SQLiteGraph database file

**target/**
- Purpose: Cargo build output
- Generated: Yes
- Committed: No (in .gitignore)
- Contains: debug/ release builds, dependency tracking

**.planning/codebase/**
- Purpose: Generated codebase documentation
- Generated: Yes (by this mapper agent)
- Committed: Yes
- Contains: ARCHITECTURE.md, STRUCTURE.md, STACK.md, etc.

## Module File Organization

**forge_core/src structure:**

```
forge_core/src/
├── lib.rs              # Public API, Forge entry point
├── types.rs            # Core data types
├── error.rs            # Error types
├── storage/
│   └── mod.rs          # UnifiedGraphStore
├── graph/
│   └── mod.rs          # Graph operations
├── search/
│   └── mod.rs          # Search operations (stub)
├── cfg/
│   └── mod.rs          # CFG analysis
├── edit/
│   └── mod.rs          # Edit operations (stub)
├── analysis/
│   └── mod.rs          # Combined operations (stub)
├── watcher.rs          # File watching
├── indexing.rs         # Incremental indexing
├── cache.rs           # Query caching
├── pool.rs            # Connection pooling
└── runtime.rs          # Runtime orchestration
```

**forge_agent/src structure:**

```
forge_agent/src/
├── lib.rs              # Agent orchestrator
├── observe.rs          # Phase 1: Observation
├── policy.rs          # Phase 2: Policy validation
├── planner.rs          # Phase 3: Plan generation
├── mutate.rs          # Phase 4: Mutation
├── verify.rs          # Phase 5: Verification
├── commit.rs          # Phase 6: Commit
└── cli.rs             # Command-line interface
```

## Test Structure

**Unit Tests:**

Each module includes inline unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_something() {
        // Test code
    }
}
```

**Integration Tests:**

```
tests/
├── integration/
│   ├── accessor_tests.rs       # Module accessor tests
│   ├── builder_tests.rs       # Builder pattern tests
│   └── runtime_tests.rs      # Runtime integration tests
├── fixtures/
│   └── simple_rust_project/   # Sample codebase for testing
└── common/
    └── mod.rs               # Shared test utilities
```

## Public API Organization

**forge_core Re-exports:**

```rust
// Error types
pub use error::{ForgeError, Result};

// Core types
pub use types::{SymbolId, BlockId, CfgBlock, CfgBlockKind, Location, Span};

// Access to modules
pub mod storage;
pub mod graph;
pub mod search;
pub mod cfg;
pub mod edit;
pub mod analysis;
pub mod watcher;
pub mod indexing;
pub mod cache;
pub mod pool;
pub mod runtime;
```

**forge_agent Re-exports:**

```rust
// Re-export policy module
pub use policy::{Policy, PolicyValidator, PolicyReport, PolicyViolation};

// Re-export observation types
pub use observe::Observation;
```

## Database Location

When Forge is used, it creates/uses a graph database at:

```
<codebase>/
└── .forge/
    └── graph.db                 # SQLiteGraph database
```

This is created and managed by `UnifiedGraphStore` in `forge_core/src/storage/mod.rs`.

## Feature Flag Matrix

### forge_core

| Feature | Default | Dependencies | Purpose |
|---------|---------|--------------|---------|
| (none) | - | - | Minimal build (storage stub only) |
| sqlite | Yes | sqlitegraph/sqlite-backend | SQLite database backend |
| native-v3 | No | sqlitegraph/native-v3 | Native V3 binary backend (future) |

### forge_runtime

| Feature | Default | Dependencies | Purpose |
|---------|---------|--------------|---------|
| sqlite | Yes | forge-core/sqlite, sqlitegraph/sqlite-backend | SQLite backend |
| native-v3 | No | forge-core/native-v3, sqlitegraph/native-v3 | Native V3 backend |

### forge_agent

| Feature | Default | Dependencies | Purpose |
|---------|---------|--------------|---------|
| sqlite | Yes | forge-core/sqlite, sqlitegraph/sqlite-backend | SQLite backend |
| native-v3 | No | forge-core/native-v3, sqlitegraph/native-v3 | Native V3 backend |

## File Organization Patterns

### Module Structure

Each forge_core module follows this pattern:

```
module_name/
└── mod.rs                       # Single-file modules (current)
    ├── Module struct (wraps Arc<UnifiedGraphStore>)
    ├── Builder structs (for complex queries)
    ├── Operation structs (for multi-step workflows)
    ├── Trait definitions (for polymorphism)
    └── Unit tests (in #[cfg(test)] module)
```

### Code Organization by Layer

```
1. Types layer (types.rs)
    └── No dependencies on other forge_core modules

2. Error layer (error.rs)
    └── Depends only on types

3. Storage layer (storage/)
    └── Depends on types, error

4. Functional modules (graph/, search/, cfg/, edit/)
    └── Depend on storage, types, error

5. Composition layer (analysis/)
    └── Combines functional modules

6. Entry point (lib.rs)
    └── Exports public API, creates modules
```

## External Dependencies

### Direct Dependencies (forge_core)

| Crate | Version | Feature | Purpose |
|-------|---------|----------|---------|
| sqlitegraph | 1.5 | optional | Graph storage backend |
| tokio | 1 | full | Async runtime |
| anyhow | 1 | - | Error handling at boundaries |
| serde | 1 | derive | Serialization support |
| serde_json | 1 | - | JSON serialization |
| thiserror | 1 | - | Error derives |
| similar | 2 | - | Diff generation |
| blake3 | 1 | - | Hashing for PathId |
| notify | 6 | - | File watching |

### Dev Dependencies

| Crate | Purpose |
|--------|---------|
| tokio/test-util | Async test utilities |
| tempfile | Temporary test directories |

## Testing Structure

### Unit Tests

Each module includes inline unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_something() {
        // Test code
    }
}
```

### Integration Tests (Planned)

```
tests/
├── integration/
│   ├── graph_tests.rs         # Graph module integration tests
│   ├── search_tests.rs       # Search module integration tests
│   ├── cfg_tests.rs          # CFG module integration tests
│   └── edit_tests.rs         # Edit module integration tests
│
└── fixtures/
    ├── simple_rust_project/   # Sample Rust codebase
    ├── multi_file_project/   # Multi-file sample
    └── edge_cases/          # Edge case fixtures
```

## Workspace Members

**forge_core** (`forge_core/Cargo.toml`):
- name: "forge-core"
- version: "0.1.0"
- edition: "2021"
- dependencies: See above dependency matrix
- features: sqlite (default), native-v3 (optional)

**forge_runtime** (`forge_runtime/Cargo.toml`):
- name: "forge-runtime"
- version: "0.1.0"
- edition: "2021"
- dependencies: forge-core, sqlitegraph, tokio, notify, anyhow
- features: sqlite (default), native-v3 (optional)

**forge_agent** (`forge_agent/Cargo.toml`):
- name: "forge-agent"
- version: "0.1.0"
- edition: "2021"
- dependencies: forge-core, sqlitegraph, tokio, anyhow, thiserror, serde, similar
- features: sqlite (default), native-v3 (optional)

## Line Count Summary

| Crate | Module | Lines (approx.) |
|-------|--------|-----------------|
| forge_core | lib.rs | 513 |
| forge_core | types.rs | 716 |
| forge_core | error.rs | 96 |
| forge_core | storage/mod.rs | 348 |
| forge_core | graph/mod.rs | 169 |
| forge_core | search/mod.rs | 22 |
| forge_core | cfg/mod.rs | 872 |
| forge_core | edit/mod.rs | 1 |
| forge_core | analysis/mod.rs | 29 |
| forge_core | watcher.rs | 437 |
| forge_core | indexing.rs | 402 |
| forge_core | cache.rs | 414 |
| forge_core | pool.rs | 373 |
| forge_core | runtime.rs | 396 |
| forge_core | **Total** | **4,788** |
| forge_runtime | lib.rs | 140 |
| forge_agent | lib.rs | 334 |
| forge_agent | observe.rs | 187 |
| forge_agent | policy.rs | 588 |
| forge_agent | planner.rs | 424 |
| forge_agent | mutate.rs | Not created yet |
| forge_agent | verify.rs | Not created yet |
| forge_agent | commit.rs | Not created yet |
| forge_agent | cli.rs | Not created yet |
| forge_agent | **Total** | **1,533+** |
| tests | integration/* | ~450 |
| **Total Codebase** | **~6,800** |

---

*Structure analysis: 2026-02-13*

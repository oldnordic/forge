# Directory Structure

**Version**: 0.1.0 (Design Phase)
**Generated**: 2026-02-12

---

## Workspace Layout

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
│       └── STRUCTURE.md           # This file (generated)
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

---

## forge_core/

The core SDK library providing the unified Forge API.

```
forge_core/
├── Cargo.toml                     # Package manifest
│   ├── name: "forge-core"
│   ├── features: sqlite, native-v2
│   └── dependencies: sqlitegraph, tokio, anyhow, serde, thiserror
│
└── src/
    ├── lib.rs                     # Public API, Forge entry point (242 lines)
    │   └── Exports: Forge, ForgeBuilder, error, types, storage modules
    │
    ├── types.rs                  # Core type definitions (255 lines)
    │   ├── Identifiers: SymbolId, BlockId, PathId
    │   ├── Location: Location, Span
    │   ├── SymbolKind: Function, Method, Struct, Enum, Trait, etc.
    │   ├── ReferenceKind: Call, Use, TypeReference, etc.
    │   ├── PathKind: Normal, Error, Degenerate, Infinite
    │   ├── Language: Rust, Python, C, Cpp, Java, JS, TS, Go
    │   └── Data: Symbol, Reference, Path, Cycle, Loop
    │
    ├── error.rs                  # Error type definitions (92 lines)
    │   └── ForgeError enum with variants for all error cases
    │
    ├── storage/
    │   └── mod.rs               # UnifiedGraphStore placeholder (82 lines)
    │       ├── UnifiedGraphStore struct
    │       ├── Database path management (.forge/graph.db)
    │       └── Stub methods for v0.1
    │
    ├── graph/
    │   └── mod.rs               # Graph operations module (149 lines)
    │       ├── GraphModule struct
    │       ├── Methods: find_symbol, callers_of, references, etc.
    │       └── Status: Stubs returning BackendNotAvailable
    │
    ├── search/
    │   └── mod.rs               # Search operations module (154 lines)
    │       ├── SearchModule struct
    │       ├── SearchBuilder for fluent queries
    │       └── Status: Stubs returning BackendNotAvailable
    │
    ├── cfg/
    │   └── mod.rs               # CFG analysis module (215 lines)
    │       ├── CfgModule struct
    │       ├── PathBuilder for path enumeration
    │       ├── Types: DominatorTree, Loop, Path
    │       └── Status: Stubs returning BackendNotAvailable
    │
    ├── edit/
    │   └── mod.rs               # Edit operations module (242 lines)
    │       ├── EditModule struct
    │       ├── EditOperation trait (verify, preview, apply, rollback)
    │       ├── RenameOperation, DeleteOperation
    │       ├── Types: Diff, RenameResult, DeleteResult
    │       └── Status: Stubs returning BackendNotAvailable
    │
    └── analysis/
        └── mod.rs               # Combined analysis module (117 lines)
            ├── AnalysisModule struct
            ├── Methods: impact_radius, unused_functions, etc.
            ├── Types: ImpactAnalysis
            └── Status: Mostly stubs, delegates to other modules
```

### Module Dependencies (forge_core)

```
lib.rs
    ├─► types.rs      (no dependencies)
    ├─► error.rs      (depends on types.rs)
    ├─► storage/      (depends on types.rs, error.rs)
    ├─► graph/        (depends on storage, types, error)
    ├─► search/       (depends on storage, types, error)
    ├─► cfg/          (depends on storage, types, error)
    ├─► edit/         (depends on storage, types, error)
    └─► analysis/     (depends on graph, cfg, edit)
```

---

## forge_runtime/

Runtime services for indexing and caching (stub implementation).

```
forge_runtime/
├── Cargo.toml                     # Package manifest
│   ├── name: "forge-runtime"
│   ├── features: sqlite, native-v2
│   └── dependencies: forge-core, sqlitegraph, tokio, notify, anyhow
│
└── src/
    └── lib.rs                     # Runtime services (136 lines)
        ├── RuntimeConfig struct
        │   ├── watch_enabled: bool
        │   ├── symbol_cache_ttl: Duration
        │   ├── cfg_cache_ttl: Duration
        │   └── max_cache_size: usize
        │
        ├── ForgeRuntime struct
        │   ├── config: RuntimeConfig
        │   ├── new() - Create with default config
        │   ├── with_config() - Create with custom config
        │   ├── watch() - Start file watcher
        │   ├── clear_cache() - Clear all caches
        │   └── stats() - Get runtime statistics
        │
        └── RuntimeStats struct
            ├── cache_size: usize
            ├── watch_active: bool
            └── reindex_count: u64
```

### Status

- **v0.1**: Stub implementation only
- **v0.3**: Planned full implementation with:
  - File watching (notify crate)
  - Query result caching
  - Incremental reindexing

---

## forge_agent/

Deterministic AI loop orchestration (stub implementation).

```
forge_agent/
├── Cargo.toml                     # Package manifest
│   ├── name: "forge-agent"
│   ├── features: sqlite, native-v2
│   └── dependencies: forge-core, sqlitegraph, tokio, anyhow, thiserror
│
└── src/
    └── lib.rs                     # Agent loop (318 lines)
        ├── AgentError enum
        │   ├── ObservationFailed
        │   ├── PlanningFailed
        │   ├── MutationFailed
        │   ├── VerificationFailed
        │   ├── CommitFailed
        │   └── PolicyViolation
        │
        ├── policy module
        │   ├── Policy enum
        │   │   ├── NoUnsafeInPublicAPI
        │   │   ├── PreserveTests
        │   │   ├── MaxComplexity(usize)
        │   │   └── Custom { name, validate }
        │   └── Policy::validate() method
        │
        ├── Agent struct
        │   ├── observe() - Gather context from graph
        │   ├── constrain() - Apply policy rules
        │   ├── plan() - Generate execution steps
        │   ├── mutate() - Apply changes
        │   ├── verify() - Validate results
        │   └── commit() - Finalize transaction
        │
        └── Data types
            ├── Observation
            ├── ConstrainedPlan
            ├── ExecutionPlan
            ├── PlanStep
            ├── PlanOperation (Rename, Delete, Create)
            ├── ImpactEstimate
            ├── MutationResult
            ├── VerificationResult
            └── CommitResult
```

### Agent Loop Flow

```
1. observe(query)     → Observation
2. constrain(observation, policy) → ConstrainedPlan
3. plan(constrained) → ExecutionPlan
4. mutate(plan)      → MutationResult
5. verify(result)    → VerificationResult
6. commit(result)     → CommitResult
```

### Status

- **v0.1**: Stub implementation only
- **v0.4**: Planned full implementation

---

## Documentation

```
docs/
├── ARCHITECTURE.md              # System architecture (536 lines)
│   ├── Design principles (Graph-First, Deterministic, Backend Agnostic)
│   ├── System architecture diagram
│   ├── Module structure details
│   ├── Data flow diagrams
│   ├── Component interfaces
│   ├── Error handling strategy
│   └── Testing strategy
│
├── API.md                       # API reference (placeholder)
│
├── PHILOSOPHY.md                # Design philosophy (386 lines)
│   ├── The problem with LLMs and text tools
│   ├── Why ForgeKit is different
│   ├── Deterministic stack components
│   ├── Core principles
│   ├── "LLVM for AI Code Agents" vision
│   └── Local-first philosophy
│
├── CONTRIBUTING.md               # Contribution guide (placeholder)
│
├── DEVELOPMENT_WORKFLOW.md       # Development workflow (placeholder)
│
└── ROADMAP.md                   # Project roadmap (placeholder)
```

---

## Planning Documents

```
.planning/
├── milestones/
│   ├── v0.1-REQUIREMENTS.md     # Requirements for v0.1 milestone
│   └── v0.1-ROADMAP.md         # Roadmap items for v0.1
│
├── phases/
│   ├── 01-project-organization/
│   │   ├── 01-01-PLAN.md        # Workspace setup plan
│   │   └── 01-02-PLAN.md        # Project organization plan
│   │
│   ├── 02-core-sdk/
│   │   └── 02-01-PLAN.md        # Core SDK development plan
│   │
│   ├── 03-runtime-layer/         # (planned for v0.3)
│   └── 04-agent-layer/          # (planned for v0.4)
│
└── codebase/
    ├── ARCHITECTURE.md           # Generated architecture doc
    └── STRUCTURE.md             # This file
```

---

## Build Artifacts

```
target/                          # Cargo build output (gitignored)
├── debug/
│   ├── build/                   # Build scripts output
│   ├── deps/                    # Dependency libraries
│   ├── .fingerprint/            # Dependency tracking
│   └── incremental/             # Incremental compilation
│
├── CACHEDIR.TAG                 # Cache directory marker
└── .rustc_info.json            # Rust compiler info
```

---

## Database Location

When Forge is used, it creates/uses a graph database at:

```
<codebase>/
└── .forge/
    └── graph.db                 # SQLiteGraph database
```

This is created and managed by `UnifiedGraphStore` in `forge_core/src/storage/mod.rs`.

---

## Feature Flag Matrix

### forge_core

| Feature | Default | Dependencies | Purpose |
|---------|---------|--------------|---------|
| (none) | - | - | Minimal build (storage stub only) |
| sqlite | Yes | sqlitegraph/sqlite-backend | SQLite database backend |
| native-v2 | No | sqlitegraph/native-v2 | Native V3 binary backend (future) |

### forge_runtime

| Feature | Default | Dependencies | Purpose |
|---------|---------|--------------|---------|
| sqlite | Yes | forge-core/sqlite, sqlitegraph/sqlite-backend | SQLite backend |
| native-v2 | No | forge-core/native-v2, sqlitegraph/native-v2 | Native V3 backend |

### forge_agent

| Feature | Default | Dependencies | Purpose |
|---------|---------|--------------|---------|
| sqlite | Yes | forge-core/sqlite, sqlitegraph/sqlite-backend | SQLite backend |
| native-v2 | No | forge-core/native-v2, sqlitegraph/native-v2 | Native V3 backend |

---

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
   └─ No dependencies on other forge_core modules

2. Error layer (error.rs)
   └─ Depends only on types

3. Storage layer (storage/)
   └─ Depends on types, error

4. Functional modules (graph/, search/, cfg/, edit/)
   └─ Depend on storage, types, error

5. Composition layer (analysis/)
   └─ Combines functional modules

6. Entry point (lib.rs)
   └─ Exports public API, creates modules
```

---

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

### Dev Dependencies

| Crate | Purpose |
|--------|---------|
| tokio/test-util | Async test utilities |
| tempfile | Temporary test directories |

---

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

---

## Line Count Summary

| Crate | Module | Lines (approx.) |
|-------|--------|-----------------|
| forge_core | lib.rs | 242 |
| forge_core | types.rs | 255 |
| forge_core | error.rs | 92 |
| forge_core | storage/mod.rs | 82 |
| forge_core | graph/mod.rs | 149 |
| forge_core | search/mod.rs | 154 |
| forge_core | cfg/mod.rs | 215 |
| forge_core | edit/mod.rs | 242 |
| forge_core | analysis/mod.rs | 117 |
| forge_core | **Total** | **1,548** |
| forge_runtime | lib.rs | 136 |
| forge_agent | lib.rs | 318 |
| **Total Codebase** | | **2,002** |

---

*Generated from codebase analysis on 2026-02-12*

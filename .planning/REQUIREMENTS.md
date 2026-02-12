# ForgeKit Requirements

**Project**: ForgeKit
**Version**: 0.1.0
**Status**: Active Development
**Last Updated**: 2026-02-12

---

## Requirements Overview

This document defines scoped requirements for the ForgeKit project, organized by milestone phase. Requirements are prioritized and include acceptance criteria.

---

## Milestone v0.1: Foundation

**Goal**: Establish project foundation with proper workspace structure, documentation, and basic API design.

**Success Criteria:**
- Workspace compiles with all three crates
- Public API is defined (even if stubbed)
- Documentation is complete and consistent
- Examples demonstrate intended usage
- Integration tests validate basic structure
- No external tool dependencies (yet)

---

### v0.1 Requirements

#### REQ-001: Workspace Structure

**Priority**: P0 (Must Have)

Create a Cargo workspace with three members.

**Acceptance Criteria:**
- [ ] `Cargo.toml` at workspace root with `members = ["forge_core", "forge_runtime", "forge_agent"]`
- [ ] Each crate has its own `Cargo.toml`
- [ ] `cargo build --workspace` succeeds
- [ ] `cargo test --workspace` succeeds

**Status**: Complete - Workspace structure exists and compiles

---

#### REQ-002: Public API Definition

**Priority**: P0 (Must Have)

Define core public API even if implementations are stubs.

**Required Types:**
```rust
pub struct Forge { /* */ }
pub struct ForgeBuilder { /* */ }
pub struct GraphModule { /* */ }
pub struct SearchModule { /* */ }
pub struct CfgModule { /* */ }
pub struct EditModule { /* */ }
pub struct AnalysisModule { /* */ }
```

**Acceptance Criteria:**
- [ ] `use forge::Forge;` compiles
- [ ] `forge.graph()` returns `GraphModule`
- [ ] `forge.search()` returns `SearchModule`
- [ ] `forge.cfg()` returns `CfgModule`
- [ ] `forge.edit()` returns `EditModule`
- [ ] `forge.analysis()` returns `AnalysisModule`
- [ ] All module types are public

**Status**: Complete - All types defined with stub implementations

---

#### REQ-003: Error Types

**Priority**: P0 (Must Have)

Define comprehensive error hierarchy covering all planned operations.

**Required Variants:**
```rust
pub enum ForgeError {
    DatabaseError(String),
    SymbolNotFound(String),
    InvalidQuery(String),
    EditConflict { file: PathBuf, span: Span },
    VerificationFailed(String),
    PolicyViolation(String),
    BackendNotAvailable(String),
    CfgNotAvailable(SymbolId),
    PathOverflow(SymbolId),
    Io(std::io::Error),
    Json(serde_json::Error),
    Graph(anyhow::Error),
}
```

**Acceptance Criteria:**
- [ ] `std::error::Error` implemented
- [ ] Error variants cover all planned operations
- [ ] Error messages are actionable
- [ ] `Result<T>` type alias defined

**Status**: Complete - Error types defined in forge_core/src/error.rs

---

#### REQ-004: Core Type System

**Priority**: P0 (Must Have)

Define fundamental types used across all modules.

**Required Types:**
- `SymbolId(i64)` - Stable symbol identifier
- `BlockId(i64)` - CFG block identifier
- `PathId([u8; 16])` - BLAKE3 hash of execution path
- `Location { file_path, byte_start, byte_end, line_number }`
- `Span { start, end }`
- `SymbolKind { Function, Method, Struct, Enum, ... }`
- `ReferenceKind { Call, Use, TypeReference, ... }`
- `PathKind { Normal, Error, Degenerate, Infinite }`
- `Language { Rust, Python, C, Cpp, Java, ... }`

**Acceptance Criteria:**
- [ ] All types defined in `types.rs`
- [ ] Debug implementations for all types
- [ ] Clone/Copy where appropriate
- [ ] Display implementations for ID types

**Status**: Complete - All types defined in forge_core/src/types.rs (255 lines)

---

#### REQ-005: Documentation

**Priority**: P0 (Must Have)

Complete project documentation set.

**Required Documents:**
- README.md - Project overview
- ARCHITECTURE.md - System design
- API.md - API reference
- PHILOSOPHY.md - Design principles
- DEVELOPMENT_WORKFLOW.md - Process
- CONTRIBUTING.md - Guidelines
- ROADMAP.md - Project roadmap

**Acceptance Criteria:**
- [ ] All documents exist and are complete
- [ ] Cross-references are accurate
- [ ] Code examples compile
- [ ] Diagrams are rendered correctly

**Status**: Complete - All documentation exists

---

#### REQ-006: Storage Abstraction

**Priority**: P1 (Should Have)

Define storage interface even if only SQLite is implemented initially.

**Required Interface:**
```rust
pub struct UnifiedGraphStore {
    codebase_path: PathBuf,
    db_path: PathBuf,
}
```

**Acceptance Criteria:**
- [ ] `UnifiedGraphStore` struct defined
- [ ] `open()` method for initialization
- [ ] Database path management (`.forge/graph.db`)
- [ ] Backend selection capability

**Status**: Partial - Placeholder exists in forge_core/src/storage/mod.rs

---

#### REQ-007: Module Stubs

**Priority**: P1 (Should Have)

Define all module APIs with stub implementations.

**Required Modules:**
- `graph/mod.rs` - GraphModule with find_symbol, callers_of, etc.
- `search/mod.rs` - SearchModule with SearchBuilder
- `cfg/mod.rs` - CfgModule with PathBuilder
- `edit/mod.rs` - EditModule with EditOperation trait
- `analysis/mod.rs` - AnalysisModule combining all modules

**Acceptance Criteria:**
- [ ] All module structs defined
- [ ] Public methods with correct signatures
- [ ] Stub implementations return `BackendNotAvailable`
- [ ] Builder patterns defined where appropriate

**Status**: Complete - All modules stubbed in forge_core/src/

---

## Milestone v0.2: Core SDK Implementation

**Goal**: Implement actual functionality with SQLiteGraph integration and tool bindings.

---

### v0.2 Requirements

#### REQ-101: SQLiteGraph Integration

**Priority**: P0 (Must Have)

Integrate SQLiteGraph as the storage backend.

**Acceptance Criteria:**
- [ ] `UnifiedGraphStore` wraps `sqlitegraph::GraphBackend`
- [ ] Database connection management
- [ ] Prepared statement caching
- [ ] Transaction support
- [ ] Error handling and conversion

**Dependencies:**
- sqlitegraph crate >= 1.6.0
- Database schema compatibility

---

#### REQ-102: Graph Module Implementation

**Priority**: P0 (Must Have)

Implement graph operations via Magellan integration.

**Acceptance Criteria:**
- [ ] `find_symbol()` queries symbols table by name
- [ ] `find_symbol_by_id()` retrieves symbol by ID
- [ ] `callers_of()` finds incoming references
- [ ] `references()` finds all references
- [ ] `reachable_from()` performs reachability analysis
- [ ] `cycles()` detects call graph cycles
- [ ] Unit tests for each operation

**Integration Points:**
- magellan v2.2.1 CLI or library

---

#### REQ-103: Search Module Implementation

**Priority**: P0 (Must Have)

Implement semantic search via LLMGrep integration.

**Acceptance Criteria:**
- [ ] `symbol()` creates SearchBuilder
- [ ] `SearchBuilder::kind()` filters by SymbolKind
- [ ] `SearchBuilder::file()` filters by path
- [ ] `SearchBuilder::limit()` caps results
- [ ] `execute()` returns matching symbols
- [ ] Unit tests for filtering logic

**Integration Points:**
- llmgrep CLI or library

---

#### REQ-104: CFG Module Implementation

**Priority**: P1 (Should Have)

Implement CFG analysis via Mirage integration.

**Acceptance Criteria:**
- [ ] `paths()` creates PathBuilder
- [ ] `PathBuilder::normal_only()` filters success paths
- [ ] `PathBuilder::error_only()` filters error paths
- [ ] `PathBuilder::execute()` enumerates paths
- [ ] `dominators()` computes dominator tree
- [ ] `loops()` detects natural loops
- [ ] Unit tests for each operation

**Integration Points:**
- mirage CLI or library

---

#### REQ-105: Edit Module Implementation

**Priority**: P0 (Must Have)

Implement span-safe editing via Splice integration.

**Acceptance Criteria:**
- [ ] `rename_symbol()` creates RenameOperation
- [ ] `delete_symbol()` creates DeleteOperation
- [ ] `EditOperation::verify()` validates syntax/types
- [ ] `EditOperation::preview()` generates diff
- [ ] `EditOperation::apply()` commits changes
- [ ] `EditOperation::rollback()` reverts changes
- [ ] Unit tests for edit workflow

**Integration Points:**
- splice v2.5.0 CLI or library
- tree-sitter for validation

---

#### REQ-106: Analysis Module Implementation

**Priority**: P1 (Should Have)

Implement combined operations using multiple modules.

**Acceptance Criteria:**
- [ ] `impact_radius()` analyzes change impact
- [ ] `unused_functions()` finds dead code
- [ ] `circular_dependencies()` detects cycles
- [ ] Unit tests for composite operations

---

#### REQ-107: Integration Testing

**Priority**: P0 (Must Have)

Comprehensive integration tests for all modules.

**Acceptance Criteria:**
- [ ] Test fixtures with sample codebases
- [ ] Multi-file operation tests
- [ ] End-to-end workflow tests
- [ ] Error path tests
- [ ] Performance benchmarks (optional)

---

## Milestone v0.3: Runtime Layer

**Goal**: Implement indexing, caching, and file watching.

---

### v0.3 Requirements

#### REQ-201: File Watching

**Priority**: P0 (Must Have)

Implement file system watching for reindexing.

**Acceptance Criteria:**
- [ ] Watch mode via `forge_runtime::watch()`
- [ ] Detects file changes via `notify` crate
- [ ] Triggers incremental reindex
- [ ] Configurable debounce period

---

#### REQ-202: Query Caching

**Priority**: P1 (Should Have)

Implement caching for frequently accessed data.

**Acceptance Criteria:**
- [ ] Symbol query cache
- [ ] CFG path cache
- [ ] Configurable TTL
- [ ] Cache invalidation on edits
- [ ] Cache size limits

---

#### REQ-203: Incremental Indexing

**Priority**: P1 (Should Have)

Re-index only changed files.

**Acceptance Criteria:**
- [ ] Parse changed files only
- [ ] Update affected graph regions
- [ ] Invalidate stale references
- [ ] Maintain graph consistency

---

## Milestone v0.4: Agent Layer

**Goal**: Implement deterministic AI orchestration loop.

---

### v0.4 Requirements

#### REQ-301: Agent Loop

**Priority**: P0 (Must Have)

Implement observe -> constrain -> plan -> mutate -> verify -> commit loop.

**Acceptance Criteria:**
- [ ] `observe()` gathers graph context
- [ ] `constrain()` applies policy rules
- [ ] `plan()` generates execution steps
- [ ] `mutate()` applies changes
- [ ] `verify()` validates results
- [ ] `commit()` finalizes transaction

---

#### REQ-302: Policy System

**Priority**: P0 (Must Have)

Define and enforce policy constraints.

**Acceptance Criteria:**
- [ ] `NoUnsafeInPublicAPI` policy
- [ ] `PreserveTests` policy
- [ ] `MaxComplexity` policy
- [ ] Custom policy support
- [ ] Policy validation at each step

---

#### REQ-303: LLM Integration (Optional)

**Priority**: P2 (Nice to Have)

Integrate LLM for planning phase.

**Acceptance Criteria:**
- [ ] Pluggable LLM backend
- [ ] Structured prompt generation
- [ ] Response validation
- [ ] Fallback to deterministic planning

---

## Performance Requirements

### Query Performance Targets

| Operation | Target | Notes |
|-----------|---------|--------|
| Symbol lookup | < 10ms | By name, cached |
| Reference query | < 50ms | All references |
| CFG enumeration | < 100ms | Up to 1000 paths |
| Rename operation | < 1s | Cross-file with 100 refs |

### Scalability Targets

| Metric | Target | Notes |
|---------|---------|--------|
| Repository size | 100k files | Tested with large projects |
| Symbol count | 1M symbols | Graph database performance |
| Concurrent queries | 100 simultaneous | Connection pooling |

---

## Security Requirements

### Local-First Guarantee

- [ ] No network calls for core operations
- [ ] All data stored locally
- [ ] No telemetry or analytics
- [ ] Audit trail for all operations

### Edit Safety

- [ ] All edits verified before apply
- [ ] Rollback always available
- [ ] No silent failures
- [ ] Explicit confirmation for destructive operations

---

## Code Quality Requirements

### Testing Coverage

| Component | Target | Status |
|-----------|---------|--------|
| Public API | 100% | Required |
| Internal logic | 80%+ | Target |
| Error paths | 100% | Required |
| Edge cases | Explicit | Required |

### Linting Standards

- [ ] Zero clippy warnings
- [ ] Zero `#[allow(...)]` without justification
- [ ] rustfmt compliance
- [ ] 100-character line limit (soft)

### Documentation Standards

- [ ] All public items documented
- [ ] All examples compile
- [ ] Error messages are actionable
- [ ] Cross-references are accurate

---

## Non-Requirements

Explicitly OUT of scope:

### Not in Any Milestone

- **Web interface** - ForgeKit is a library/CLI only
- **Cloud hosting** - Local-only by design
- **Multi-language editing** - Rust first, others later
- **AI training** - Not an AI training platform

### Deferred Post-v1.0

- **Native V3 backend** - External dependency on sqlitegraph
- **Language server protocol** - Separate LSP server project
- **IDE plugins** - Consumers of ForgeKit, not part of it

---

## Requirements Traceability Matrix

| Requirement | Milestone | Priority | Status | Notes |
|--------------|------------|-----------|---------|--------|
| REQ-001: Workspace | v0.1 | P0 | Complete |
| REQ-002: Public API | v0.1 | P0 | Complete |
| REQ-003: Error Types | v0.1 | P0 | Complete |
| REQ-004: Core Types | v0.1 | P0 | Complete |
| REQ-005: Documentation | v0.1 | P0 | Complete |
| REQ-006: Storage | v0.1 | P1 | Partial |
| REQ-007: Module Stubs | v0.1 | P1 | Complete |
| REQ-101: SQLiteGraph | v0.2 | P0 | Pending |
| REQ-102: Graph Module | v0.2 | P0 | Pending |
| REQ-103: Search Module | v0.2 | P0 | Pending |
| REQ-104: CFG Module | v0.2 | P1 | Pending |
| REQ-105: Edit Module | v0.2 | P0 | Pending |
| REQ-106: Analysis Module | v0.2 | P1 | Pending |
| REQ-107: Integration Tests | v0.2 | P0 | Pending |
| REQ-201: File Watch | v0.3 | P0 | Pending |
| REQ-202: Caching | v0.3 | P1 | Pending |
| REQ-203: Incremental Index | v0.3 | P1 | Pending |
| REQ-301: Agent Loop | v0.4 | P0 | Pending |
| REQ-302: Policy System | v0.4 | P0 | Pending |
| REQ-303: LLM Integration | v0.4 | P2 | Pending |

---

*Last updated: 2026-02-12*

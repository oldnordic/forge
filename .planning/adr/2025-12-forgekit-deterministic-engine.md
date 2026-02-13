# ADR: ForgeKit as Deterministic Code Reasoning Engine

**Date**: 2025-12-30
**Status**: Accepted
**Context**: Phase 03 (Test Infrastructure) complete; planning Phase 04+ architecture

---

## Problem Statement

ForgeKit was initially conceived as a "tool aggregator" — wrapping CLI tools (Magellan, LLMGrep, Mirage, Splice) with library APIs. This approach creates tight coupling and makes LLM behavior a critical dependency.

**User insight**: ForgeKit should be a **deterministic code reasoning engine** — a stable, queryable foundation where LLM agents can build reliable tooling without being language-specific.

---

## Decision

**ForgeKit will be a deterministic code reasoning engine with trait-based architecture.**

### Core Principles

1. **No LLM Dependency** — All operations work through structured traits, not token-guessing
2. **Determinism** — Same inputs → same outputs, always
3. **Trait-Based** — Clean abstraction boundaries between components
4. **Provider Pattern** — External tools (Magellan, etc.) are pluggable adapters behind traits
5. **Result Contracts** — Every operation returns structured, queryable data
6. **Patch Safety** — All edits are validated, reversible, and auditable

---

## Architecture

```
forgekit/
├── Cargo.toml (workspace root)
├── forge-core/        (traits + stable types)
├── forge-providers/  (Magellan, LLMGrep, Mirage, Splice adapters)
├── forge-runtime/     (high-level orchestration engine)
├── forge-testing/    (deterministic integration harness)
└── examples/          (minimal usage demos)
```

### Crate: forge-core

**Purpose**: Stable trait definitions and core types.

**Contains**:
- Core Types: `NodeId`, `SymbolId`, `Span`, `FileId`, `GraphEdge`, `CfgBlock`, `PatchSet`, `AnalysisReport`, `ImpactSet`
- Core Traits:
  - `SymbolIndex` — symbol lookup by name/ID
  - `SemanticSearch` — pattern-based code search
  - `ControlFlow` — CFG and path queries
  - `GraphTraversal` — reachability and cycle detection
  - `RefactorEngine` — rename and delete operations
  - `PatchEngine` — apply and rollback of changes
  - `ImpactAnalyzer` — combined analysis operations

**Dependencies**: None (except `serde` for serialization)

**Stability**: This crate must compile in <1s and rarely change.

---

### Crate: forge-providers

**Purpose**: Implement forge-core traits for external tools.

**Contains**:
- `MagellanIndex` — implements `SymbolIndex`, `GraphTraversal`, `ControlFlow`
- `LlmgrepSearch` — implements `SemanticSearch`
- `MirageCfg` — implements `ControlFlow`
- `SplicePatchEngine` — implements `PatchEngine`
- `SQLiteGraphTraversal` — implements `GraphTraversal` for SQLite backend

**Feature flags**:
- `--features magellan` — enable Magellan provider
- `--features llmgrep` — enable LLMGrep provider
- `--features mirage` — enable Mirage provider
- `--features splice` — enable Splice provider
- Default: SQLiteGraph only (no external tools)

**Dependencies**: `forge-core` (traits), `sqlitegraph` (Magellan), `llmgrep`, `mirage`, `splice`

---

### Crate: forge-runtime

**Purpose**: High-level orchestration API composing all traits.

**Key APIs**:
```rust
// Main entry point
pub fn analyze(codebase: &Path) -> Result<AnalysisReport> {
    let engine = ForgeEngine::new(codebase)?;
    let report = engine.analyze()?;
    Ok(report)
}

// High-level operations built from traits
pub fn rename_symbol(repo: &Repository, old: &str, new: &str) -> Result<PatchSet>
pub fn find_impact(symbol: SymbolId) -> Result<ImpactSet>
pub fn extract_function(symbol: SymbolId) -> Result<FunctionInfo>
pub fn dependency_slice(roots: &[SymbolId]) -> Result<DependencyGraph>
pub fn control_dependence_of(symbol: SymbolId) -> Result<CallGraph>
```

**Dependencies**: `forge-core` (traits), `forge-providers` (adapters)

**Does NOT**:
- Call LLMs
- Make "smart guesses"
- Depend on any AI service

---

### Crate: forge-testing

**Purpose**: Deterministic integration testing harness.

**Provides**:
- Snapshot testing — verify state after operations
- Hash verification — ensure graph integrity
- Graph invariant checking — validate structure constraints
- Patch safety validation — ensure edits are reversible
- Cross-provider consistency — verify behavior across different backends
- Deterministic fixtures — repeatable test data

**Dependencies**: `forge-core` (traits), `forge-runtime` (orchestration)

**Purpose**: Not to be used by end users; internal quality assurance.

---

## Migration Strategy

### Phase 1: Add Traits (forge-core expansion)
1. Add new trait definitions to `forge-core/src/traits/`
2. Implement traits in existing modules as default adapters
3. Update `forge-runtime` to use traits instead of concrete implementations
4. Add provider stubs behind feature flags

### Phase 2: Create Providers (new crate)
1. Create `forge-providers/Cargo.toml` with workspace dependency
2. Implement `MagellanIndex` using Magellan library
3. Implement `LlmgrepSearch` using LLMGrep library
4. Implement `MirageCfg` using Mirage library
5. Implement `SplicePatchEngine` using Splice library
6. Add `SQLiteGraphTraversal` for in-memory graphs
7. Add feature flags to disable all providers

### Phase 3: Extract Runtime
1. Create `forge-runtime/Cargo.toml` if not exists
2. Implement `ForgeEngine` orchestrator
3. Implement high-level APIs (`analyze`, `rename_symbol`, etc.)
4. Move runtime-specific code from `forge_agent`

### Phase 4: Create Testing
1. Create `forge-testing/Cargo.toml`
2. Implement deterministic test harness
3. Move integration tests from `forge_core/tests/` to `forge-testing/`
4. Add snapshot and invariant verification

### Phase 5: Cleanup Agent
1. Remove LLM-specific code from `forge_agent`
2. Simplify to pure CLI tool
3. Move planning-specific logic to `forge-runtime`

---

## Rationale

### Why This Architecture?

1. **Separation of Concerns** — Traits, providers, runtime, and testing are distinct crates with clear boundaries
2. **Provider Pattern** — External tools are behind feature-flagged adapters; users can disable them
3. **Stability** — `forge-core` changes rarely; providers can evolve independently
4. **Testing Independence** — `forge-testing` has no dependency on runtime logic
5. **LLM-Optional** — Runtime orchestrates traits; LLM agents use the traits directly
6. **Determinism** — All operations return structured data that can be reasoned about

### Why NOT Tool Aggregation?

1. **Tight Coupling** — Wrapping CLIs creates hidden dependencies
2. **Version Mismatch** — Library API != CLI API (features lag behind)
3. **Process Overhead** — Subprocess spawning is expensive
4. **Debuggability** — Structured return values enable better debugging vs. text parsing
5. **Composability** — Traits enable users to build custom analysis pipelines

---

## Success Criteria

- [ ] `forge-core` compiles in <1s with ~2000 LOC
- [ ] `forge-providers` compiles with feature flags for each tool
- [ ] `forge-runtime` provides high-level API composing all traits
- [ ] All external tools are behind trait boundaries
- [ ] `forge-testing` provides deterministic verification
- [ ] Zero LLM dependencies in critical path
- [ ] Doc examples demonstrate trait-based usage

---

## Consequences

### If Accepted

1. **Phase 04 (Agent Layer) needs re-planning** — Current plan assumes LLM integration; new architecture makes LLM optional
2. **Phase 16 (Tool Integration) becomes obsolete** — External tools are already providers behind traits
3. **New crate structure** — 4 crates instead of 3; update ROADMAP.md
4. **Immediate next phase** — Phase 04.1: Define Core Traits (add to forge-core)
5. **Estimated effort** — 2-3 weeks for foundational restructuring

### If Rejected

1. Continue with current "tool aggregator" approach
2. Accept tight coupling to Magellan/LLMGrep/Mirage/Splice CLIs

---

## Related Decisions

- **Decision 001**: Workspace Structure — Multi-crate workspace
- **Decision 002**: Error Type Pattern — Single `ForgeError` enum
- **Decision 003**: Newtype Pattern for IDs — Type-safe wrappers
- **Decision 004**: Builder Pattern for Queries — Fluent API
- **Decision 005**: Backend Abstraction — `UnifiedGraphStore` with backend trait

---

*Status: Accepted — 2026-02-13*

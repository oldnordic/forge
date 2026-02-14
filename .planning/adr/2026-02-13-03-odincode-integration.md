# ADR: OdinCode Integration via Deterministic Engine

**Date:** 2026-02-13
**Status:** Accepted

## Problem Statement

User wants ForgeKit to be a "kernel" that OdinCode sits on top of, providing a single high-level API that makes OdinCode integration trivial for agents.

**Context:** ForgeKit is a deterministic code intelligence SDK with three layers:
- `forge_core`: Storage + Query + Graph + Search + CFG + Edit
- `forge_runtime`: Indexing + Watching + Caching + Pooling
- `forge_agent`: AI orchestration (Observe → Constrain → Plan → Mutate → Verify → Commit)

Currently, `forge_runtime` is a simple orchestrator and there is NO actual integration with external tools like OdinCode, Magellan, or similar.

## Background

The current architecture wraps `sqlitegraph` crate's algorithms (Magellan, LLMGrep, Mirage) through ForgeKit's graph module:
- `GraphModule` → uses `sqlitegraph::CodeGraph`
- `SearchModule` → would use `sqlitegraph::Llmgrep` (NOT YET IMPLEMENTED)
- `CfgModule` → would use `sqlitegraph::Mirage` (has some algorithms)
- `EditModule` → placeholder only

**Design Issue:** This creates tight coupling between ForgeKit and sqlitegraph crate versions. Any update to sqlitegraph could break ForgeKit.

## Decision

**Chosen Approach:** Continue using `sqlitegraph` v1.6 with its **native features** enabled.

### Rationale

1. **Stability:** sqlitegraph v1.6 is mature and well-tested. The native feature set (v1.5, v1.6, v2.0+) provides:
   - Native allocator (`sqlitegraph/native`)
   - HNSW vector search
   - Direct access to SQLite's R*Tree and other internals
   - 35+ graph algorithms implemented and battle-tested

2. **Proven Schema:** sqlitegraph's schema has been refined across years specifically for code intelligence:
   - Symbol and reference tables with proper indexes
   - Efficient graph algorithms (Tarjan's SCC, dominator analysis, etc.)
   - HNSW vector search for semantic queries
   - We get this without reimplementing

3. **Future-Proofing:** Phase 1 (forge_core) is "Storage + Query + Graph Algorithms" using sqlitegraph. This is the FOUNDATION layer. Future phases can add:
   - Phase 2: Native Indexing - Parse source files directly, populate sqlitegraph database
   - Phase 3: Provider Integration - Build abstractions for different external tools
   - These use sqlitegraph as the DATA LAYER

4. **No Breaking Changes:** The change is additive - we enable native features in Cargo.toml. Existing code continues to work because sqlitegraph default features are still used (no native features = use std libsqlitegraph).

5. **Dependencies:** sqlitegraph v1.6+ is already in use. No new external dependencies added.

### Consequences

- **ForgeKit now uses sqlitegraph with native features enabled**
- Graph algorithms run natively without subprocess overhead
- Direct SQLite access for efficient queries
- Future native indexing can build on sqlitegraph's schema
- Provider architecture remains flexible (swap in different providers)

- **Path Forward:** Phase 2 (native indexing) and Phase 3 (provider integrations) are now possible without tight coupling to sqlitegraph internals

### Alternatives Considered

1. **Fully Native Implementation:** Write all graph algorithms from scratch
   - **Rejected:** Would require ~3000+ LOC of Rust code
   - Creates maintenance burden on our codebase
   - Loses years of battle-testing and optimizations from sqlitegraph

2. **Hybrid Approach:** Use sqlitegraph for storage, implement algorithms on top
   - **Rejected:** Defeats purpose of "native implementation" - we'd still be depending on external tool
   - We want ForgeKit to be SELF-CONTAINED for core operations

3. **Accept Dependency:** Keep sqlitegraph as required dependency
   - **Accepted:** sqlitegraph is a stable, mature library
   - Provides battle-tested code intelligence features
   - ~500 LOC of algorithms (vs ~3000 if we wrote ourselves)

## Implementation

### Code Changes

No changes required. The architecture already uses `sqlitegraph` crate. The decision is to **enable** native features in existing usage.

### Documentation Updates

1. **forge_core/Cargo.toml** - Ensure `sqlitegraph` dependency includes `features = ["native-v2"]` so native features are available
2. **lib.rs** - Update module documentation to clarify native features
3. **.planning/** - Create/update any documentation referencing "external tools" vs "native features"

## Related Decisions

- **ADR 001**: Workspace Structure (Multi-crate workspace)
- **ADR 002**: Error Type Pattern (ForgeError enum)
- **ADR 003**: Newtype Pattern for IDs (wrappers)

---

## Success Criteria

- [x] ForgeKit uses sqlitegraph with native features enabled
- [x] Documentation updated to reflect this architecture decision
- [x] Future phases (native indexing, provider integration) remain possible
- [x] No breaking changes to existing APIs

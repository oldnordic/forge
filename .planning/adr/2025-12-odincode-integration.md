# ADR: OdinCode Integration via Deterministic Engine

**Date**: 2025-12-30
**Status**: Accepted
**Context**: User wants ForgeKit to be the "kernel" that OdinCode sits on top of

---

## Problem Statement

**User Request:** ForgeKit should provide a single, high-level API that makes OdinCode integration trivial:

```rust
// Instead of:
let forge = Forge::open(path)?;
let symbol = forge.graph().find_symbol("main")?;
let callers = forge.graph().callers_of(symbol)?;

// OdinCode can:
let forge = Forge::new();
let symbol = forge.symbol("main");
let callers = forge.callers(symbol);
```

**Benefits:**
1. **Single Entry Point** — `Forge::new()` replaces `Forge::open()`, builders, etc.
2. **Opinionated Kernel** — ForgeKit makes high-level decisions (policy enforcement, transaction safety)
3. **Provider Injection** — Runtime accepts providers; OdinCode can inject custom implementations
4. **Determinism** — All operations go through observe → constrain → mutate → verify → commit cycle

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    OdinCode (User Agent)                  │
│                           ↓                              │
│  ┌─────────────────────────────────────────────────────┐ │
│  │          forge-runtime (Opinionated Kernel)          │ │
│  │  ┌───────────────────────────────────────────────┐ │ │
│  │  │         forge-providers (Adapters)          │ │
│  │  │         ┌─────────┬──────────────┐    │ │
│  │  │         │         │               │       │ │
│  │  │  ┌───────┴────┐      │       │       │ │
│  │  │  │  ┌─────────┴─────┐ │       │       │ │
│  │  │  │  │                │  │       │       │ │
▼  ▼  ▼  ▼               ▼  ▼       ▼       ▼       │
┌───────┴──────┐ ┌─────────┴─────────┐ ┌─────────┴─────────┐ ┌─────────┴───────┐
│  SymbolIndex  │ SemanticSearch │ ControlFlow │   PatchEngine   │  ImpactAnalyzer │
└───────┬──────┘ └───────┬─────────┘ └───────────┬───────┘ └───────────┬───────┘
        │              │           │            │              │           │
        ▼              ▼           ▼            ▼              ▼           ▼
   ┌───────────────────────────────────────────────────────────────────────────────┐
   │              forge-core (Traits & Stable Types)                  │
   └───────────────────────────────────────────────────────────────────────────────┘
```

### Layer: forge-runtime (Opinionated Kernel)

**Purpose:** Orchestrate analysis workflows with policy enforcement and transaction safety.

**Key Methods:**

```rust
impl ForgeRuntime {
    // Main entry — providers injected
    pub fn new(providers: Vec<Box<dyn Provider>>) -> Self;

    // High-level analysis ops
    pub fn analyze(&self, codebase: &Path) -> Result<AnalysisReport>;
    pub fn rename_symbol(&mut self, old: &str, new: &str) -> Result<PatchSet>;
    pub fn extract_function(&mut self, span: Span) -> Result<FunctionInfo>;
    pub fn find_dead_code(&mut self, criterion: DeadCodeCriterion) -> Vec<SymbolId>;

    // Internal: delegates to providers
    fn resolve_provider<T: Provider>(&self) -> T where T: Provider;
}
```

**Core Traits Exposed:**

```rust
pub trait Provider {
    fn name(&self) -> &str;
    fn initialize(&mut self) -> Result<()>;
}

// Provider interfaces (from forge-core traits)
pub trait SymbolIndex {
    fn find_by_name(&self, name: &str) -> Option<NodeId>;
    fn callers_of(&self, symbol: NodeId) -> Vec<NodeId>;
}

pub trait SemanticSearch {
    fn pattern_search(&self, pattern: &str) -> Vec<Symbol>;
}

pub trait ControlFlow {
    fn cfg_for(&self, function: NodeId) -> Cfg;
    fn paths(&self, function: NodeId) -> Vec<GraphPath>;
}

pub trait PatchEngine {
    fn compute_patch(&mut self, change: &Change) -> Result<PatchSet>;
    fn apply(&mut self, patch: &PatchSet) -> Result<()>;
}
```

**Workflow: Observe → Constrain → Mutate → Verify → Commit**

1. **Observe:** Gather data through SymbolIndex, SemanticSearch, ControlFlow
2. **Constrain:** Apply policy (read-only? safety rules?) to determine allowed operations
3. **Mutate:** Compute PatchSet via PatchEngine (proves constraints before mutating)
4. **Verify:** Re-query to ensure postconditions hold
5. **Commit:** Return structured PatchSet with proof metadata

### Safety Model

**PatchSet Structure:**
```rust
pub struct PatchSet {
    operations: Vec<PatchOperation>,  // What to apply
    preconditions: Vec<Invariant>,      // What must be true
    postconditions: Vec<Invariant>,     // What must be true after
    proof: QueryProof,                  // What was queried/proven
    hash: [u8; 32],                 // Repo state fingerprint
}
```

**Every mutation:**
- Checks preconditions before applying
- Runs postcondition verification after applying
- Returns proof showing all queries and invariants
- **Deterministic:** Same repo state → same PatchSet hash

---

## Migration Strategy

### Phase 1: Add High-Level Runtime (forge-runtime expansion)

**Current State:** `forge-runtime` exists but is a simple orchestrator.

**Changes Needed:**
1. Add Provider trait and injection mechanism
2. Implement core analysis methods (`analyze`, `rename_symbol`, `extract_function`, `find_dead_code`)
3. Add constraint/policy system
4. Implement mutation workflow (`observe → constrain → mutate → verify → commit`)
5. Add `Forge::new()` entry point

**New API:**
```rust
// Create Forge with default providers
let forge = Forge::new();

// Or with custom providers (OdinCode injection point)
let mut runtime = ForgeRuntime::with_providers(vec![
    Box::new(MagellanProvider::new()) as Box<dyn Provider>,
    Box::new(LlmgrepProvider::new()) as Box<dyn Provider>,
]);

// High-level analysis becomes simple
let report = runtime.analyze(&repo).await?;
for finding in report.findings {
    println!("{}", finding);
}
```

### Phase 2: Create Provider Implementations (forge-providers)

**New Crate Purpose:** Implement `Provider` trait for each external tool.

**Providers to Implement:**
1. **MagellanProvider** — Wraps `SymbolIndex`, `GraphTraversal`, `ControlFlow` traits
2. **LlmgrepProvider** — Implements `SemanticSearch` trait
3. **MirageProvider** — Implements `ControlFlow` trait (CFG analysis)
4. **SpliceProvider** — Implements `PatchEngine` trait
5. **SQLiteProvider** — Provides in-memory graph for testing

**Feature Flags:**
- `--features magellan` — enable MagellanProvider
- `--features llmgrep` — enable LlmgrepProvider
- `--features mirage` — enable MirageProvider
- `--features splice` — enable SpliceProvider
- `--features sqlite` — enable SQLiteProvider (default)

**Provider Interface:**
```rust
pub trait Provider {
    fn name(&self) -> &str;
    fn initialize(&mut self) -> Result<()>;
    fn shutdown(&mut self) -> Result<()>;
}

// Example: MagellanProvider
impl Provider for MagellanProvider {
    fn name(&self) -> &str { "magellan" }
    fn initialize(&mut self) -> Result<()> {
        // Connect to Magellan, spawn watcher, etc.
    }
    // ... trait implementations
}
```

### Phase 3: Create Forge-Testing (Deterministic Regression Harness)

**Purpose:** Ensure determinism across providers and catch regressions.

**Capabilities:**
1. **Snapshot Testing** — Verify repo state after operations
2. **Hash Verification** — Ensure deterministic outputs (same repo → same hash)
3. **Graph Invariants** — Validate structure constraints
4. **Patch Safety** — Verify edits are reversible and valid
5. **Cross-Provider Consistency** — Test behavior across different providers

**Not for End Users** — For quality assurance, not external API.

---

## Integration Example for OdinCode

```odin
// Simple entry point
let forge = Forge::new();

// Analysis
let report = forge.analyze(&workspace).await?;

// Symbol lookup (via MagellanProvider)
let symbol = forge.symbol("main");

// Callers (via MagellanProvider)
let callers = forge.callers(symbol);

// CFG query (via MirageProvider)
let cfg = forge.cfg_for(symbol);

// Search (via LlmgrepProvider)
let results = forge.search().pattern("async fn").execute();

// Mutation (via SpliceProvider)
let patch = forge.prepare_rename(symbol, "new_main").compute_patch()?;
forge.apply(patch).await?;

// Rollback if needed
forge.rollback(&patch).await?;
```

---

## Rationale

### Why "Opinionated Kernel"?

1. **Policy Enforcement** — Agent decisions should be enforced by ForgeKit, not left to LLM
2. **Safety Model** — Pre/postcondition checking prevents data corruption
3. **Determinism** — Same inputs must produce same outputs (reproducible builds)
4. **Transaction Semantics** — Mutations are explicit, reversible, and provable
5. **Provider Injection** — Users can swap backends (Magellan → custom index) without changing ForgeKit

### Why Provider Pattern?

1. **Flexibility** — OdinCode can inject `CustomSymbolProvider` without forking ForgeKit
2. **Testing** — Providers can be mocked or swapped for invariant testing
3. **Version Independence** — External tool versions don't break ForgeKit
4. **Feature Flags** — Providers can be disabled selectively

---

## Success Criteria

- [ ] `Forge::new()` creates runtime with default providers
- [ ] Provider injection mechanism works
- [ ] All analysis methods implemented (analyze, rename, extract, find_dead)
- [ ] Mutation workflow (observe → constrain → mutate → verify → commit)
- [ ] PatchSet structure with pre/postconditions and proof
- [ ] `forge-providers` crate with feature flags
- [ ] `forge-testing` crate for deterministic verification

---

## Consequences

### If Accepted

**Immediate Phase:** Phase 04 (Documentation) and Phase 16 (Tool Integration) must be **re-planned** with new architecture.

**Changes:**
- ROADMAP.md — Update to reflect provider-based architecture
- forge-core — Add `Provider` trait to common types
- New milestone phases:
  - Phase 04.1: Define Core Traits (SymbolIndex, SemanticSearch, ControlFlow, PatchEngine)
  - Phase 04.2: Create forge-providers crate
  - Phase 04.3: Implement forge-runtime expansion
  - Phase 04.4: Create forge-testing crate
  - Phase 05: OdinCode Integration (simple entry point)

**Estimated Effort:** 3-4 weeks (down from 1-2 weeks for original Phase 16)

### If Rejected

Continue with Phase 16 as originally planned (Tool Integration via CLI wrapping).

This creates tight coupling and makes LLM dependency mandatory.

---

## Related Decisions

- **Decision 001**: Workspace Structure — Multi-crate workspace ✓
- **Decision 002**: Error Type Pattern — Single `ForgeError` enum ✓
- **Decision 003**: Newtype Pattern for IDs — Type-safe wrappers ✓
- **Decision 004**: Builder Pattern for Queries — Fluent API ✓
- **Decision 005**: Backend Abstraction — `UnifiedGraphStore` with backend trait ✓
- **Decision 006**: Runtime Architecture — Placeholder implementations (now superseded)
- **ADR 2025-12**: Deterministic Code Reasoning Engine — Trait-based architecture ⭐ NEW

---

*Status: Accepted — 2025-12-30*

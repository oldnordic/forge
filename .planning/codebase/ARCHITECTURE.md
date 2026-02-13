# Architecture

**Analysis Date:** 2026-02-13

## Pattern Overview

**Overall:** Three-Tier Workspace with Modular Crate Architecture

ForgeKit implements a **layered workspace architecture** where each crate represents a distinct functional layer:

1. **forge_core**: Core SDK - Programmatic interface to code intelligence
2. **forge_runtime**: Runtime layer - Indexing, caching, and file watching services
3. **forge_agent**: Agent layer - Deterministic six-phase AI loop for automated code operations

**Key Characteristics:**
- **Bottom-up dependency flow**: `forge_agent` depends on `forge_core`; `forge_runtime` depends on `forge_core`
- **Shared storage abstraction**: All crates communicate through `UnifiedGraphStore`
- **Async-first design**: All major operations are async via tokio
- **Feature-gated backends**: SQLite backend enabled via `sqlite` feature flag
- **Graph-centric**: All operations build on top of a shared code graph stored in SQLiteGraph

## Core SDK Architecture

**Pattern:** Module-based with Facade Entry Point

The `forge_core` crate follows a clear architectural pattern:

```
forge_core/src/
├── lib.rs              # Facade/Entry point (Forge struct)
├── types.rs            # Core data types (no dependencies)
├── error.rs            # Error types (depends on types)
├── storage/            # Storage abstraction (depends on types, error)
├── runtime/            # Orchestration layer (combines all below)
├── [modules]/         # Functional modules (depend on storage)
│   ├── graph/
│   ├── search/
│   ├── cfg/
│   ├── edit/
│   └── analysis/
└── [infrastructure]/  # Runtime support modules
    ├── watcher.rs
    ├── indexing.rs
    ├── cache.rs
    └── pool.rs
```

**Layer Dependencies:**
```
Types (foundation)
    ↓
Error
    ↓
Storage
    ↓
Graph ────────┐
Search ├──────────┼──→ Analysis (composition layer)
CFG   ────────┤
Edit  ────────┘
    ↓
Runtime (orchestration)
```

## Data Flow

**SDK Initialization Flow:**

```
User Code
    ↓
Forge::open(path)
    ↓
UnifiedGraphStore::open(path)
    ↓
sqlitegraph::SqliteGraph::open(.forge/graph.db)
    ↓
Store ready for queries
```

**Query Flow:**

```
┌─────────────────────────────────────────────────────────────────┐
│                         Forge Entry Point                        │
│                      (forge_core::Forge)                          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ├──► graph() ──► GraphModule
                              │                    │
                              │                    ├──► find_symbol(name)
                              │                    ├──► callers_of(name)
                              │                    ├──► references(name)
                              │                    ├──► reachable_from(id)
                              │                    └──► cycles()
                              │
                              ├──► search() ──► SearchModule
                              │                    │
                              │                    ├──► symbol(name).kind(...).execute()
                              │                    └──► pattern(pattern)
                              │
                              ├──► cfg() ──► CfgModule
                              │                   │
                              │                   ├──► paths(symbol_id)
                              │                   ├──► dominators(symbol_id)
                              │                   └──► loops(symbol_id)
                              │
                              ├──► edit() ──► EditModule
                              │                   │
                              │                   ├──► rename_symbol(old, new)
                              │                   └──► delete_symbol(name)
                              │
                              └──► analysis() ──► AnalysisModule
                                                   │
                                                   ├──► impact_radius(symbol_id)
                                                   ├──► unused_functions(entries)
                                                   └──► circular_dependencies()
```

**State Management:**
- **Immutable queries**: All query operations are read-only
- **SQLite-backed**: Graph state persisted in `.forge/graph.db`
- **Arc-shared**: UnifiedGraphStore wrapped in Arc for shared ownership
- **Connection pooling**: Semaphore-based pool limits concurrent DB access

## Key Abstractions

### SymbolId

**Purpose:** Stable identifier for symbols across reindexing

**Location:** `forge_core/src/types.rs`

**Pattern:** Newtype wrapper over i64

**Implementation:**
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SymbolId(pub i64);
```

**Key Characteristics:**
- Copy semantics (can be passed by value freely)
- Hashable (useful in HashSet/HashMap keys)
- Display implementation for user-friendly output
- Ord for ordering in collections

### PathId (CFG)

**Purpose:** Stable identifier for execution paths

**Location:** `forge_core/src/types.rs`

**Pattern:** BLAKE3 hash-based identifier

**Implementation:**
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PathId(pub [u8; 16]);
```

**Generation (in cfg module Path construction):**
```rust
let mut hasher = blake3::Hasher::new();
for block in &blocks {
    hasher.update(&block.0.to_le_bytes());
}
let hash = hasher.finalize();
let mut id = [0u8; 16];
id.copy_from_slice(&hash.as_bytes()[0..16]);
PathId(id)
```

**Properties:**
- Deterministic (same blocks → same PathId)
- Collision-resistant (16-byte BLAKE3 hash)
- Display as hex string with colons

### EditOperation Trait

**Purpose:** Polymorphic edit operations with verification pipeline

**Location:** `forge_core/src/edit/mod.rs`

**Pattern:** State machine through trait methods

**Implementation:**
```rust
pub trait EditOperation: Sized {
    type Output;

    fn verify(self) -> Result<Self>;
    fn preview(self) -> Result<Diff>;
    fn apply(self) -> Result<Self::Output>;
    fn rollback(self) -> Result<()>;
}
```

**Implementations:**
- `RenameOperation` - Renames symbols across all references
- `DeleteOperation` - Removes symbols and their references

**Flow:**
1. `verify()` - Validates operation preconditions
2. `preview()` - Generates diff without applying
3. `apply()` - Executes the edit
4. `rollback()` - Undoes changes if needed

### QueryType Enum (Observation)

**Purpose:** Natural language query classification

**Location:** `forge_agent/src/observe.rs`

**Pattern:** Enum for query intent detection

**Implementation:**
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
enum QueryType {
    FunctionsCalling,    // "functions that call X"
    FunctionsCalledBy,  // "functions called by X"
    FindByName,         // "find X named Y"
    AllFunctions,        // "all functions"
    AllStructs,          // "all structs"
    SemanticSearch,       // Generic semantic query
}
```

### Policy Enum (Constraint)

**Purpose:** Constraint definitions for code changes

**Location:** `forge_agent/src/policy.rs`

**Pattern:** Enum with associated validation logic

**Implementation:**
```rust
#[derive(Clone, Debug)]
pub enum Policy {
    NoUnsafeInPublicAPI,
    PreserveTests,
    MaxComplexity(usize),
    Custom { name: String, description: String },
}
```

**Validation functions:**
- `check_no_unsafe_in_public_api()` - Parses code for unsafe in pub context
- `check_preserve_tests()` - Counts test functions before/after
- `check_max_complexity()` - Estimates cyclomatic complexity

### PlanOperation Enum (Planning)

**Purpose:** Atomic operations in execution plan

**Location:** `forge_agent/src/planner.rs`

**Pattern:** Enum representing all possible mutation operations

**Implementation:**
```rust
#[derive(Clone, Debug)]
pub enum PlanOperation {
    Rename { old: String, new: String },
    Delete { name: String },
    Create { path: String, content: String },
    Inspect { symbol_id: SymbolId, symbol_name: String },
    Modify { file: String, start: usize, end: usize },
}
```

### Transaction State (Mutation)

**Purpose:** Track mutation operations for rollback

**Location:** `forge_agent/src/mutate.rs`

**Pattern:** Struct with rollback tracking

**Implementation:**
```rust
#[derive(Clone, Debug)]
struct Transaction {
    applied_steps: Vec<String>,
    rollback_state: Vec<RollbackState>,
}

#[derive(Clone, Debug)]
struct RollbackState {
    file: String,
    original_content: String,
}
```

## Entry Points

### Core SDK Entry Point

**Location:** `forge_core/src/lib.rs`

**Type:** `forge_core::Forge`

**Triggers:**
- Programmatic: `Forge::open("./my-project").await`
- With runtime: `Forge::with_runtime("./my-project").await`
- Builder pattern: `ForgeBuilder::new().path("./").cache_ttl(...).build().await`

**Responsibilities:**
- Initialize `UnifiedGraphStore`
- Create module accessor instances
- Optional runtime initialization

**Module Accessors:**
```rust
pub fn graph(&self) -> GraphModule
pub fn search(&self) -> SearchModule
pub fn cfg(&self) -> CfgModule
pub fn edit(&self) -> EditModule
pub fn analysis(&self) -> AnalysisModule
pub fn runtime(&self) -> Option<&Arc<Runtime>>
```

### Agent CLI Entry Point

**Location:** `forge_agent/src/cli.rs`

**Type:** Binary with subcommands

**Triggers:**
- `forge-agent run "query"` - Full agent loop execution
- `forge-agent plan "query"` - Dry run (plan only)
- `forge-agent status` - Show agent status

**CLI Structure:**
```rust
#[derive(Subcommand)]
enum Action {
    Run { query: String },
    Plan { query: String },
    Status { verbose: bool },
}
```

**Flow for `run` command:**
1. Parse current directory as codebase path
2. Create `Agent::new(&codebase_path).await`
3. Call `agent.run(&query).await`
4. Print success/failure with appropriate exit code

**Flow for `plan` command:**
1. Create agent instance
2. Run `observe()` → `constrain()` → `plan()` only
3. Display generated steps without applying
4. Exit with 0 (no changes made)

### Runtime Entry Point

**Location:** `forge_runtime/src/lib.rs`

**Type:** `forge_runtime::ForgeRuntime`

**Triggers:**
- Programmatic: `ForgeRuntime::new("./project").await`

**Responsibilities:**
- Configure cache TTLs
- Manage file watcher lifecycle
- Provide access to indexer and cache

**Note:** Runtime layer is currently stubbed for v0.2

## Error Handling

**Strategy:** Result types with thiserror derivation

**Core Error Pattern:**
```rust
// forge_core/src/error.rs
pub type Result<T> = std::result::Result<T, ForgeError>;

#[derive(thiserror::Error, Debug)]
pub enum ForgeError {
    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),

    #[error("Edit conflict in {file:?} at {span:?}")]
    EditConflict { file: PathBuf, span: Span },

    #[error("Verification failed: {0}")]
    VerificationFailed(String),

    #[error("Policy violation: {0}")]
    PolicyViolation(String),

    #[error("Backend not available: {0}")]
    BackendNotAvailable(String),

    #[error("CFG not available for symbol: {0:?}")]
    CfgNotAvailable(SymbolId),

    #[error("Path overflow for symbol: {0:?}")]
    PathOverflow(SymbolId),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Graph error: {0}")]
    Graph(#[from] anyhow::Error),

    #[error("Tool error: {0}")]
    ToolError(String),
}
```

**Agent Error Pattern:**
```rust
// forge_agent/src/lib.rs
pub type Result<T> = std::result::Result<T, AgentError>;

#[derive(thiserror::Error, Debug)]
pub enum AgentError {
    #[error("Observation failed: {0}")]
    ObservationFailed(String),

    #[error("Planning failed: {0}")]
    PlanningFailed(String),

    #[error("Mutation failed: {0}")]
    MutationFailed(String),

    #[error("Verification failed: {0}")]
    VerificationFailed(String),

    #[error("Commit failed: {0}")]
    CommitFailed(String),

    #[error("Policy violation: {0}")]
    PolicyViolation(String),

    #[error("Forge error: {0}")]
    ForgeError(#[from] forge_core::ForgeError),
}
```

**Error Conversion Chain:**
- Agent operations can return `ForgeError` via `?` operator
- `From<ForgeError> for AgentError` enables automatic conversion
- Each phase returns `Result<T>` for its specific type

## Cross-Cutting Concerns

### Logging

**Approach:** `eprintln!` for errors, `println!` for output, no structured logging

**Current State:**
- Errors printed to stderr via `eprintln!`
- No log levels (info/warn/debug)
- No structured logging framework
- Emoji indicators in CLI for user-friendly output

**Pattern:**
```rust
// In watcher
eprintln!("Warning: Could not open database: {}", e);

// In agent CLI
eprintln!("❌ Agent failed: {}", e);
println!("✅ Agent completed successfully");
println!("🔄 Running agent loop...");
```

### Validation

**Approach:** Multi-layer validation with early exit

**Pre-transaction validation:**
- Symbol existence checks before edits
- Policy validation before mutation
- Conflict detection before plan execution

**Post-transaction validation:**
- Compile check via `cargo check --message-format=short`
- Test suite execution via `cargo test --message-format=short`
- Graph consistency verification (placeholder)

**Pattern (EditOperation trait):**
```rust
fn verify(mut self) -> Result<Self> {
    // Check preconditions
    if self.old_name == self.new_name {
        return Err(ForgeError::VerificationFailed(
            "Old and new names are the same".to_string()
        ));
    }

    if self.new_name.is_empty() {
        return Err(ForgeError::VerificationFailed(
            "New name cannot be empty".to_string()
        ));
    }

    // ... more checks
    self.verified = true;
    Ok(self) // Return self for chaining
}
```

### Authentication

**Approach:** None (local-only operations)

**Current State:**
- No remote authentication
- No credential management
- All operations are local filesystem

**Future considerations:**
- May add remote graph database access
- Would require authentication layer

## Agent Layer Architecture

**Deterministic Loop Flow:**

```
┌────────────────────────────────────────────────────────────────────────────┐
│                        Agent: Deterministic AI Loop                          │
│                      (forge_agent/src/lib.rs)                                │
└────────────────────────────────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────┴───────────────────┐
                    │           query: "Add X to all API endpoints"    │
                    └───────────────────────────────────┘
                                    ▼
                    ┌───────────────────────────────────┐
                    │           Phase 1: OBSERVE             │
                    │   ┌─────────────────────────┐   │
                    │   │ Observer::gather(query)   │   │
                    │   │                          │   │
                    │   │ • Parse query intent       │   │
                    │   │ • Query graph for symbols  │   │
                    │   │ • Gather references        │   │
                    │   │ • Collect CFG data         │   │
                    │   │ • Cache results           │   │
                    │   └─────────────────────────┘   │
                    │                                  │
                    │ Returns: Observation {              │
                    │   • query: String               │
                    │   • symbols: Vec<ObservedSymbol>│   │
                    │   • references: Vec<ObservedReference>│   │
                    │   • cfg_data: Vec<CfgInfo>         │
                    │   └───────────────────────────────────┘
                                    │
                                    ▼
                    ┌───────────────────────────────────┐
                    │       Phase 2: CONSTRAIN           │
                    │   ┌─────────────────────────┐   │
                    │   │ Agent::constrain()       │   │
                    │   │ • Apply policy rules     │   │
                    │   │ • Check constraints       │   │
                    │   │ • Collect violations     │   │
                    │   └─────────────────────────┘   │
                    │                                  │
                    │ Returns: ConstrainedPlan {         │
                    │   • observation: Observation         │
                    │   • policy_violations: Vec<>     │
                    │   └───────────────────────────────────┘
                                    │
                                    ▼
                    ┌───────────────────────────────────┐
                    │        Phase 3: PLAN              │
                    │   ┌─────────────────────────┐   │
                    │   │ Planner::generate_steps()│   │
                    │   │ • Create PlanStep items  │   │
                    │   │ • Estimate impact         │   │
                    │   │ • Detect conflicts        │   │
                    │   │ • Order dependencies      │   │
                    │   │ • Generate rollback      │   │
                    │   └─────────────────────────┘   │
                    │                                  │
                    │ Returns: ExecutionPlan {            │
                    │   • steps: Vec<PlanStep>         │
                    │   • estimated_impact: ImpactEstimate│
                    │   • rollback_plan: Vec<RollbackStep>│
                    │   └───────────────────────────────────┘
                                    │
                                    ▼
                    ┌───────────────────────────────────┐
                    │       Phase 4: MUTATE             │
                    │   ┌─────────────────────────┐   │
                    │   │ Mutator                 │   │
                    │   │ • begin_transaction()    │   │
                    │   │ • apply_step() for each  │   │
                    │   │ • Track rollback state   │   │
                    │   └─────────────────────────┘   │
                    │                                  │
                    │ Returns: MutationResult {            │
                    │   • modified_files: Vec<PathBuf>   │
                    │   • diffs: Vec<String>             │
                    │   └───────────────────────────────────┘
                                    │
                                    ▼
                    ┌───────────────────────────────────┐
                    │       Phase 5: VERIFY            │
                    │   ┌─────────────────────────┐   │
                    │   │ Verifier                │   │
                    │   │ • compile_check()        │   │
                    │   │ • test_check()          │   │
                    │   │ • graph_check()          │   │
                    │   └─────────────────────────┘   │
                    │                                  │
                    │ Returns: VerificationResult {        │
                    │   • passed: bool                │
                    │   • diagnostics: Vec<String>      │
                    │   └───────────────────────────────────┘
                                    │
                                    ▼
                    ┌───────────────────────────────────┐
                    │       Phase 6: COMMIT             │
                    │   ┌─────────────────────────┐   │
                    │   │ Committer               │   │
                    │   │ • Generate transaction ID │   │
                    │   │ • Track committed files  │   │
                    │   └─────────────────────────┘   │
                    │                                  │
                    │ Returns: CommitResult {             │
                    │   • transaction_id: String         │
                    │   • files_committed: Vec<PathBuf> │
                    │   └───────────────────────────────────┘
```

**Agent Loop Characteristics:**
- **Sequential execution**: Each phase completes before next begins
- **Early exit**: Verification failure prevents commit
- **Deterministic**: Same inputs produce same execution plan
- **Observable**: Each phase returns structured result
- **Rollback-capable**: Rollback plan generated before mutation

## Storage Architecture

**UnifiedGraphStore Abstraction:**

```
┌─────────────────────────────────────────────────────────────────┐
│                    UnifiedGraphStore                             │
│              (forge_core/src/storage/mod.rs)                      │
└─────────────────────────────────────────────────────────────────┘
                              │
        ┌───────────────────┴─────────────────────┐
        │                                       │
        ▼                                       ▼
   ┌─────────────────┐                   ┌──────────────────┐
   │  codebase_path  │                   │    db_path       │
   │  PathBuf        │                   │    PathBuf        │
   └─────────────────┘                   └──────────────────┘
        │                                       │
        └───────────────────┬──────────────────────┘
                        │
                        ▼
              ┌─────────────────────────────┐
              │  graph: Option<Arc<SqliteGraph>> │
              │  (feature-gated)            │
              └─────────────────────────────┘
                        │
                        ▼
              ┌─────────────────────────────┐
              │  .forge/graph.db           │
              │  (SQLite database)          │
              └─────────────────────────────┘
```

**Storage Methods:**
| Method | Purpose | Current State |
|---------|-----------|---------------|
| `open(path)` | Open/create DB at `.forge/graph.db` | Creates directory, opens SQLiteGraph |
| `open_with_path(path, db_path)` | Custom DB location | Creates custom DB path |
| `memory()` | In-memory for testing | Creates `:memory:` DB |
| `query_symbols(name)` | Query symbols | Placeholder, returns `Vec::new()` |
| `query_references(symbol_id)` | Get references | Placeholder, returns `Vec::new()` |
| `symbol_exists(id)` | Check symbol presence | Returns introspection result |
| `get_symbol(id)` | Retrieve symbol | Returns `SymbolNotFound` error |

**SQLiteGraph Backend:**

The `UnifiedGraphStore` wraps `sqlitegraph::SqliteGraph` as its backing store. The feature-gated design allows:

```rust
#[cfg(feature = "sqlite")]
let graph = Some(Arc::new(sqlitegraph::SqliteGraph::open(&db_path)?));

#[cfg(not(feature = "sqlite"))]
let graph = None;
```

**Database Schema (via SQLiteGraph):**

The database is managed by sqlitegraph and contains:
- Symbols table: Symbol metadata (id, name, kind, location, etc.)
- References table: Edges between symbols (from, to, kind, location)
- CFG tables: Control flow data (blocks, edges, paths)

## Runtime Architecture

**Runtime Flow:**

```
┌─────────────────────────────────────────────────────────────────┐
│                    Runtime Orchestration                         │
│                  (forge_core/src/runtime.rs)                       │
└─────────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        │                     │                     │
        ▼                     ▼                     ▼
   ┌─────────┐          ┌──────────┐       ┌──────────┐
   │ Watcher │          │ Indexer  │       │    Cache   │
   └─────────┘          └──────────┘       └──────────┘
        │                     │                     │
        └─────────────────────┴─────────────────────┘
                              │
                              ▼
                    ┌─────────────────────────┐
                    │ UnifiedGraphStore      │
                    │   (SQLite backend)    │
                    └─────────────────────────┘
```

**Runtime Components:**
| Component | File | Purpose | State |
|-----------|-------|----------|--------|
| Watcher | `watcher.rs` | File system monitoring | Emits `WatchEvent` via `mpsc` channel |
| IncrementalIndexer | `indexing.rs` | Batched reindexing | Queues events, flushes to store |
| QueryCache | `cache.rs` | LRU caching with TTL | In-memory cache with expiration |
| ConnectionPool | `pool.rs` | Concurrency limiting | Semaphore-based permits |

**Runtime Flow:**
1. `Watcher` detects file changes via `notify` crate
2. Events published through `mpsc::unbounded_channel`
3. `IncrementalIndexer` queues events for batched processing
4. `flush()` processes pending changes, updates graph store
5. `QueryCache` caches query results with TTL-based expiration
6. `ConnectionPool` limits concurrent database connections

## Module Integration Architecture

**Graph Module Integration:**

```
GraphModule (forge_core/src/graph/mod.rs)
    ├── Uses: magellan::graph::CodeGraph
    ├── Wraps: Symbol/reference queries
    └── Converts: Magellan types → Forge types

Implementation:
    - find_symbol(path, name) → Symbol
    - symbols_in_file(path) → Vec<Symbol>
    - references_to_symbol(id) → Vec<Reference>
    - callers_of(name) → Vec<Symbol>
```

**CFG Module Integration:**

```
CfgModule (forge_core/src/cfg/mod.rs)
    ├── Uses: Internal TestCfg for unit testing
    ├── Provides: PathBuilder for fluent queries
    └── Status: Stubs (returns BackendNotAvailable)

Implementation:
    - paths(function_id).execute() → Vec<Path>
    - dominators(function_id) → DominatorTree
    - loops(function_id) → Vec<Loop>
```

**Analysis Module Composition:**

```
AnalysisModule (forge_core/src/analysis/mod.rs)
    ├── Combines: graph, cfg, edit modules
    └── Provides: Composite operations

Delegates to sub-modules:
    - impact_radius() → via graph/references
    - unused_functions() → via graph queries
    - circular_dependencies() → via graph traversal
```

---

*Architecture analysis: 2026-02-13*

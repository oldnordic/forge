# API Reference

**Version**: 0.1.0 (Design Phase)
**Last Updated**: 2025-12-30

---

## Table of Contents

- [Forge Builder](#forge-builder)
- [Graph Operations](#graph-operations)
- [Search Operations](#search-operations)
- [CFG Operations](#cfg-operations)
- [Edit Operations](#edit-operations)
- [Analysis Operations](#analysis-operations)
- [Agent API](#agent-api)
- [Types](#types)

---

## Forge Builder

### Creating a Forge Instance

```rust
use forge::Forge;

// Basic usage
let forge = Forge::open("./my-project")?;

// With builder
let forge = Forge::builder()
    .path("./my-project")
    .backend(ForgeBackend::Sqlite)
    .cache_config(CacheConfig {
        symbol_cache_size: 10_000,
        path_cache_ttl: Duration::from_secs(300),
    })
    .build()
    .await?;

// With explicit database path
let forge = Forge::builder()
    .path("./my-project")
    .database_path(".forge/graph.db")
    .build()
    .await?;
```

### Configuration Options

| Option | Type | Default | Description |
|---------|--------|----------|-------------|
| `path` | `&str` | *Required* | Path to codebase |
| `backend` | `ForgeBackend` | `Sqlite` | Storage backend |
| `database_path` | `&str` | `.forge/graph.db` | Custom database location |
| `cache_config` | `CacheConfig` | Default config | Cache settings |
| `watch_mode` | `bool` | `false` | Enable file watching |
| `log_level` | `LogLevel` | `Info` | Logging verbosity |

---

## Graph Operations

### Finding Symbols

```rust
let graph = forge.graph();

// Find by name
let symbols = graph.find_symbol("main")?;
// Returns: Vec<Symbol>

// Find by exact name and file
let symbol = graph.find_symbol_in_file("parse", "src/parser.rs")?;

// Find by stable ID
let symbol = graph.find_symbol_by_id(SymbolId(12345))?;

// List all symbols in a file
let symbols = graph.symbols_in_file("src/lib.rs")?;

// With filters
let symbols = graph.find_symbol("data")?
    .kind(SymbolKind::Function)?
    .language(Language::Rust)?
    .execute()?;
```

### Symbol Type

```rust
pub struct Symbol {
    pub id: SymbolId,              // Stable identifier
    pub name: String,              // Display name
    pub fully_qualified_name: String, // Full path
    pub kind: SymbolKind,           // Function, Struct, etc.
    pub language: Language,          // Rust, Python, etc.
    pub location: Location,          // File and span
    pub metadata: SymbolMetadata,     // Additional info
}

pub struct Location {
    pub file_path: String,
    pub byte_start: u32,
    pub byte_end: u32,
    pub line_number: usize,
}
```

### Reference Queries

```rust
// Find all callers
let callers = graph.callers_of("Database::connect")?;
// Returns: Vec<Reference>

// Find all callees
let callees = graph.callees_of("process_request")?;

// Find all references (includes both)
let refs = graph.references("UserSession")?;

// With context
let refs = graph.references("UserSession")?
    .with_context(3)?     // 3 lines of context
    .include_definitions()?
    .execute()?;
```

### Graph Algorithms

```rust
// Reachability
let reachable = graph.reachable_from(SymbolId(100))?;

// Dead code detection
let dead = graph.dead_code(SymbolId(1))?; // entry point

// Cycle detection
let cycles = graph.cycles()?;

// Strongly connected components
let sccs = graph.strongly_connected_components()?;

// Call graph condensation
let condensed = graph.condense()?;
```

---

## Search Operations

### Symbol Search

```rust
let search = forge.search();

// Basic symbol search
let results = search.symbol("Database")
    .kind(SymbolKind::Struct)?
    .limit(10)
    .execute()?;

// Pattern search
let results = search.pattern("impl.*Reader")?
    .language(Language::Rust)?
    .execute()?;
```

### AST Queries

```rust
// Find AST nodes by kind
let functions = search.ast_query(AstQuery {
    kind: AstKind::FunctionDefinition,
    file: "src/lib.rs",
})?;

// Find with property filter
let unsafe_blocks = search.ast_query(AstQuery {
    kind: AstKind::Block,
    has_label: "unsafe",
})?;
```

### Semantic Search

```rust
// When embeddings are available
let similar = search.semantic("handle database connection errors")?
    .limit(5)
    .execute()?;
```

---

## CFG Operations

### Path Enumeration

```rust
let cfg = forge.cfg();

// All execution paths
let paths = cfg.paths(SymbolId(100))?
    .execute()?;

// With filters
let normal_paths = cfg.paths(SymbolId(100))?
    .normal_only()?
    .max_length(10)?
    .limit(100)?
    .execute()?;

// Error paths only
let error_paths = cfg.paths(SymbolId(100))?
    .error_only()?
    .execute()?;
```

### Path Type

```rust
pub struct Path {
    pub id: PathId,
    pub kind: PathKind,
    pub blocks: Vec<BlockId>,
    pub length: usize,
}

pub enum PathKind {
    Normal,      // Successful execution
    Error,       // Error/panic path
    Degenerate,   // Unreachable code
    Infinite,     // Loop without exit
}
```

### Dominance Analysis

```rust
// Dominators
let dominators = cfg.dominators(SymbolId(100))?;
// Returns: DominatorTree

// Post-dominators
let post_doms = cfg.post_dominators(SymbolId(100))?;

// Dominance frontier
let frontiers = cfg.dominance_frontiers(SymbolId(100))?;

// Immediate dominator
let idom = cfg.immediate_dominator(block_id)?;
```

### Loop Analysis

```rust
// Natural loops
let loops = cfg.loops(SymbolId(100))?;

// Loop headers
let headers = cfg.loop_headers(SymbolId(100))?;

// Loop nesting
let nesting = cfg.loop_nesting_depth(SymbolId(100))?;
```

### Unreachable Code

```rust
// Find unreachable blocks
let unreachable = cfg.unreachable_blocks(SymbolId(100))?;

// Verify block is reachable
let is_reachable = cfg.is_reachable(SymbolId(100), block_id)?;
```

---

## Edit Operations

### Rename Symbol

```rust
let edit = forge.edit();

let result = edit.rename_symbol("OldName", "NewName")?
    .verify()?        // Pre-flight validation
    .preview()?       // Show diff without applying
    .apply()?;       // Apply changes

// Result info
println!("Modified {} files", result.files_modified);
println!("Updated {} references", result.references_updated);
```

### Delete Symbol

```rust
let result = edit.delete_symbol("unused_function")?
    .verify()?
    .apply()?;
```

### Inline Function

```rust
let result = edit.inline_function("helper_fn")?
    .verify()?
    .apply()?;
```

### Extract Operation

```rust
// Extract trait
let result = edit.extract_trait(
    "DatabaseBackend",
    vec![
        SymbolId(100),  // connect method
        SymbolId(101),  // query method
    ]
)?
    .trait_name("DbOps")
    .verify()?
    .apply()?;
```

### Edit Validation

```rust
// Custom validation
let result = edit.rename_symbol("A", "B")?
    .validate_with(|edit| {
        // Custom check
        if edit.touches_unsafe_code() {
            Err("Cannot rename unsafe code".into())
        } else {
            Ok(())
        })
    })?
    .apply()?;
```

---

## Analysis Operations

### Impact Analysis

```rust
let analysis = forge.analysis();

// Impact radius
let impact = analysis.impact_radius(SymbolId(100))?;
println!("Affects {} symbols", impact.affected_symbols.len());
println!("Affects {} files", impact.affected_files.len());

// Blast zone (affected by change)
let blast = analysis.blast_zone(SymbolId(100))?;
```

### Dead Code Analysis

```rust
// Find all unused functions
let unused = analysis.unused_functions(entry_points)?;

// Find unreachable code
let unreachable = analysis.unreachable_code()?;

// Find dead imports
let dead_imports = analysis.unused_imports()?;
```

### Dependency Analysis

```rust
// Find circular dependencies
let cycles = analysis.circular_dependencies()?;

// Dependency graph
let deps = analysis.dependencies("src/main.rs")?;

// Reverse dependencies
let dependents = analysis.reverse_dependencies("src/lib.rs")?;
```

---

## Agent API

### Deterministic Agent Loop

```rust
use forge::agent::{Agent, Policy};

let result = Agent::new(&forge)
    .observe("Rename function foo to bar")?
    .constrain(Policy::NoUnsafeInPublicAPI)?
    .plan()?
    .mutate()?
    .verify()?
    .commit()?;
```

### Agent Phases

```rust
// 1. Observe - Gather context
let observation = agent.observe(query)?;
// Returns: Observation { symbols, references, cfg }

// 2. Constrain - Apply policy
let constrained = agent.constrain(observation, Policy::default())?;
// Returns: ConstrainedPlan

// 3. Plan - Generate steps
let plan = agent.plan(constrained)?;
// Returns: ExecutionPlan { steps }

// 4. Mutate - Apply changes
let mutation = agent.mutate(plan)?;
// Returns: MutationResult { files_modified }

// 5. Verify - Validate result
let verified = agent.verify(mutation)?;
// Returns: VerificationResult { success, diagnostics }

// 6. Commit - Finalize
let commit = agent.commit(verified)?;
// Returns: CommitResult { transaction_id }
```

### Policy DSL

```rust
use forge::agent::Policy;

// Built-in policies
let policy = Policy::NoUnsafeInPublicAPI;
let policy = Policy::PreserveSemantics;
let policy = Policy::RequireTests;
let policy = Policy::MaxComplexity(10);

// Compose policies
let policy = Policy::all_of(vec![
    Policy::NoUnsafeInPublicAPI,
    Policy::MaxComplexity(10),
]);

// Custom policy
let policy = Policy::custom(|edit| {
    if edit.affects_public_api() && !edit.has_tests() {
        Err("Public API changes require tests".into())
    } else {
        Ok(())
    }
});
```

---

## Types

### Core Types

```rust
// Symbol identifier (stable across reindex)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SymbolId(pub i64);

// Block identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlockId(pub i64);

// Path identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PathId(pub [u8; 16]); // BLAKE3 hash
```

### Symbol Kind

```rust
pub enum SymbolKind {
    // Declarations
    Function,
    Method,
    Struct,
    Enum,
    Trait,
    Impl,
    Module,
    TypeAlias,
    Constant,
    Static,

    // Variables
    Parameter,
    LocalVariable,
    Field,

    // Other
    Macro,
    Use,
}
```

### Language

```rust
pub enum Language {
    Rust,
    Python,
    C,
    Cpp,
    Java,
    JavaScript,
    TypeScript,
    Go,
    Unknown(String),
}
```

### Reference Type

```rust
pub struct Reference {
    pub from: SymbolId,
    pub to: SymbolId,
    pub ref_kind: ReferenceKind,
    pub location: Location,
}

pub enum ReferenceKind {
    Call,
    Use,
    TypeReference,
    Inherit,
    Implementation,
    Override,
}
```

---

## Error Types

```rust
pub enum ForgeError {
    // Storage
    DatabaseError(String),
    BackendNotAvailable(String),

    // Query
    SymbolNotFound(String),
    InvalidQuery(String),

    // Edit
    EditConflict { file: String, span: Span },
    VerificationFailed { file: String, reason: String },

    // CFG
    CfgNotAvailable(SymbolId),
    PathOverflow(SymbolId),

    // Policy
    PolicyViolation(String),
}

impl std::error::Error for ForgeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // ...
    }
}
```

---

## Examples

### Complete Refactoring Example

```rust
use forge::{Forge, Policy};
use forge::agent::Agent;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open codebase
    let forge = Forge::open("./my-project").await?;

    // Find symbol to rename
    let symbol = forge.graph()
        .find_symbol("old_name")?
        .into_iter()
        .find(|s| s.kind == SymbolKind::Function)
        .ok_or("Symbol not found")?;

    // Check impact
    let impact = forge.analysis().impact_radius(symbol.id)?;
    println!("This will affect {} files", impact.affected_files.len());

    // Execute with agent
    let result = Agent::new(&forge)
        .observe(format!("Rename function {} to {}", symbol.name, "new_name"))?
        .constrain(Policy::NoUnsafeInPublicAPI)?
        .plan()?
        .mutate()?
        .verify()?
        .commit()?;

    println!("Successfully renamed in {} files", result.files_modified);

    Ok(())
}
```

### Analysis Example

```rust
// Find all dead code
let analysis = forge.analysis();

let unused_functions = analysis.unused_functions(&["main", "test_main"])?;
for fn in &unused_functions {
    println!("Unused: {} at {:?}", fn.name, fn.location);
}

// Find circular dependencies
let cycles = analysis.circular_dependencies()?;
for cycle in &cycles {
    println!("Cycle: {}", cycle.join(" -> "));
}

// Find hot paths (most executed)
let hot_paths = forge.cfg().hot_paths(SymbolId(100))?;
println!("Hottest path has {} blocks", hot_paths[0].length);
```

---

*Last updated: 2025-12-30*

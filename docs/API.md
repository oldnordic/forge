# ForgeKit API Reference

Complete API reference for ForgeKit SDK. This documents the working APIs as of v0.2.

## Table of Contents

- [Core Types](#core-types)
- [Forge](#forge)
- [GraphModule](#graphmodule)
- [AnalysisModule](#analysismodule)
- [SearchModule](#searchmodule)
- [CfgModule](#cfgmodule)
- [EditModule](#editmodule)
- [Storage](#storage)

## Core Types

### BackendKind

Enum for selecting storage backend:

```rust
pub enum BackendKind {
    SQLite,      // SQLite database (.forge/graph.db)
    NativeV3,    // Native V3 binary format (.forge/graph.v3)
}
```

**Default:** `BackendKind::SQLite`

**Example:**
```rust
let forge = Forge::open_with_backend("./project", BackendKind::NativeV3).await?;
```

### Symbol

Represents a code symbol (function, struct, etc.):

```rust
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub fully_qualified_name: String,
    pub kind: SymbolKind,
    pub language: Language,
    pub location: Location,
    pub parent_id: Option<SymbolId>,
    pub metadata: Value,
}
```

### SymbolKind

```rust
pub enum SymbolKind {
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
    // Variable types
    Parameter,
    LocalVariable,
    Field,
    // Other
    Macro,
    Use,
}
```

### Location

```rust
pub struct Location {
    pub file_path: PathBuf,
    pub byte_start: u32,
    pub byte_end: u32,
    pub line_number: usize,
}
```

### Reference

Represents a reference from one symbol to another:

```rust
pub struct Reference {
    pub from: SymbolId,
    pub to: SymbolId,
    pub kind: ReferenceKind,
    pub location: Location,
}
```

### ReferenceKind

```rust
pub enum ReferenceKind {
    Call,           // Function call
    Use,            // Variable usage
    TypeReference,  // Type reference
    Inherit,        // Inheritance
    Implementation, // Trait implementation
}
```

## Forge

Main entry point for the SDK.

### Methods

#### `open()`

Open a codebase with default backend (SQLite).

```rust
pub async fn open(path: impl AsRef<Path>) -> Result<Self>
```

**Example:**
```rust
let forge = Forge::open("./my-project").await?;
```

#### `open_with_backend()`

Open with specific backend.

```rust
pub async fn open_with_backend(
    path: impl AsRef<Path>,
    backend: BackendKind
) -> Result<Self>
```

**Example:**
```rust
let forge = Forge::open_with_backend("./project", BackendKind::NativeV3).await?;
```

#### `graph()`

Get graph module for symbol queries.

```rust
pub fn graph(&self) -> GraphModule
```

#### `search()`

Get search module for code search.

```rust
pub fn search(&self) -> SearchModule
```

#### `cfg()`

Get CFG module for control flow analysis.

```rust
pub fn cfg(&self) -> CfgModule
```

#### `edit()`

Get edit module for code modifications.

```rust
pub fn edit(&self) -> EditModule
```

#### `analysis()`

Get analysis module for composite operations.

```rust
pub fn analysis(&self) -> AnalysisModule
```

## GraphModule

Symbol and reference queries using the graph database.

### Methods

#### `find_symbol()`

Find symbols by name (fuzzy search).

```rust
pub async fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>>
```

**Example:**
```rust
let symbols = forge.graph().find_symbol("main").await?;
for symbol in symbols {
    println!("Found: {} ({:?}) at {:?}", 
        symbol.name, symbol.kind, symbol.location);
}
```

#### `find_symbol_by_id()`

Find a symbol by its stable ID.

```rust
pub async fn find_symbol_by_id(&self, id: SymbolId) -> Result<Symbol>
```

#### `callers_of()`

Find all callers of a symbol.

```rust
pub async fn callers_of(&self, name: &str) -> Result<Vec<Reference>>
```

**Example:**
```rust
let callers = forge.graph().callers_of("process_request").await?;
println!("Called by {} functions", callers.len());
for caller in callers {
    println!("  - {} at line {}", 
        caller.from, caller.location.line_number);
}
```

#### `references()`

Find all references to a symbol.

```rust
pub async fn references(&self, name: &str) -> Result<Vec<Reference>>
```

**Example:**
```rust
let refs = forge.graph().references("MyStruct").await?;
println!("Referenced {} times", refs.len());
```

#### `impact_analysis()`

Perform k-hop traversal to find all symbols impacted by changing a symbol.

```rust
pub async fn impact_analysis(
    &self, 
    symbol_name: &str, 
    max_hops: Option<u32>
) -> Result<Vec<ImpactedSymbol>>
```

**Example:**
```rust
// Find all symbols within 2 hops of "process_request"
let impacted = forge.graph()
    .impact_analysis("process_request", Some(2))
    .await?;

for symbol in impacted {
    println!("{} ({} hops away)", 
        symbol.name, symbol.hop_distance);
}
```

**Returns:** `ImpactedSymbol` contains:
- `symbol_id: i64` - Entity ID
- `name: String` - Symbol name
- `kind: String` - Entity kind (fn, struct, etc.)
- `file_path: String` - Source file
- `hop_distance: u32` - Distance from target
- `edge_type: String` - Type of relationship

#### `reachable_from()`

Find all symbols reachable from a given symbol via BFS.

```rust
pub async fn reachable_from(&self, id: SymbolId) -> Result<Vec<SymbolId>>
```

#### `symbol_count()`

Get the total number of symbols in the graph.

```rust
pub async fn symbol_count(&self) -> Result<usize>
```

#### `index()`

Index the codebase using magellan (requires `magellan` feature).

```rust
pub async fn index(&self) -> Result<()>
```

## AnalysisModule

Composite analysis operations combining graph, CFG, and edit modules.

### Methods

#### `analyze_impact()`

Analyze the impact of changing a symbol.

```rust
pub async fn analyze_impact(&self, symbol_name: &str) -> Result<ImpactAnalysis>
```

**Returns:**
- `affected_symbols: Vec<Symbol>` - Directly affected symbols
- `call_sites: usize` - Total number of call sites

#### `deep_impact_analysis()`

Deep impact analysis with k-hop traversal.

```rust
pub async fn deep_impact_analysis(
    &self, 
    symbol_name: &str, 
    depth: u32
) -> Result<Vec<ImpactedSymbol>>
```

#### `find_dead_code()`

Find dead code (symbols with no references).

```rust
pub async fn find_dead_code(&self) -> Result<Vec<Symbol>>
```

**Example:**
```rust
let dead = forge.analysis().find_dead_code().await?;
for symbol in dead {
    println!("Unused: {} in {:?}", 
        symbol.name, symbol.location.file_path);
}
```

#### `complexity_metrics()`

Calculate complexity metrics for a function (placeholder in v0.2).

```rust
pub async fn complexity_metrics(&self, symbol_name: &str) -> Result<ComplexityMetrics>
```

#### `analyze_source_complexity()`

Calculate complexity from source code directly.

```rust
pub fn analyze_source_complexity(&self, source: &str) -> ComplexityMetrics
```

**Example:**
```rust
let source = r#"
fn example(x: i32) -> i32 {
    if x > 0 { 1 } else { 0 }
}
"#;
let metrics = analysis.analyze_source_complexity(source);
println!("Complexity: {}", metrics.cyclomatic_complexity);
println!("Risk: {:?}", metrics.risk_level());
```

**Returns:** `ComplexityMetrics` contains:
- `cyclomatic_complexity: usize` - McCabe complexity
- `decision_points: usize` - Number of branches
- `max_nesting_depth: usize` - Maximum nesting level
- `lines_of_code: usize` - Lines of code

#### `cross_references()`

Get cross-references (both callers and callees).

```rust
pub async fn cross_references(&self, symbol_name: &str) -> Result<CrossReferences>
```

#### `module_dependencies()`

Analyze module dependencies.

```rust
pub async fn module_dependencies(&self) -> Result<Vec<ModuleDependency>>
```

#### `find_dependency_cycles()`

Find circular dependencies between modules.

```rust
pub async fn find_dependency_cycles(&self) -> Result<Vec<Vec<String>>>
```

## SearchModule

Semantic code search via LLMGrep integration.

### Methods

#### `pattern()` / `pattern_search()`

Regex pattern search.

```rust
pub async fn pattern(&self, pattern: &str) -> Result<Vec<Symbol>>
```

**Example:**
```rust
let results = forge.search().pattern(r"fn.*test.*\(").await?;
for symbol in results {
    println!("Test function: {}", symbol.name);
}
```

#### `semantic()` / `semantic_search()`

Semantic search (requires indexing).

```rust
pub async fn semantic(&self, query: &str) -> Result<Vec<Symbol>>
```

#### `symbol_by_name()`

Find a specific symbol by exact name.

```rust
pub async fn symbol_by_name(&self, name: &str) -> Result<Option<Symbol>>
```

#### `symbols_by_kind()`

Find all symbols of a specific kind.

```rust
pub async fn symbols_by_kind(&self, kind: SymbolKind) -> Result<Vec<Symbol>>
```

#### `index()`

Index the codebase for semantic search (requires `llmgrep` feature).

```rust
pub async fn index(&self) -> Result<()>
```

## CfgModule

Control flow graph analysis.

### Methods

#### `index()`

Index source files for CFG extraction.

```rust
pub async fn index(&self) -> Result<()>
```

#### `paths()`

Create a path enumeration builder.

```rust
pub fn paths(&self, function: SymbolId) -> PathBuilder
```

**Example:**
```rust
let paths = forge.cfg()
    .paths(symbol_id)
    .normal_only()
    .max_length(10)
    .execute()
    .await?;
```

#### `dominators()`

Compute dominator tree for a function.

```rust
pub async fn dominators(&self, function: SymbolId) -> Result<DominatorTree>
```

**Returns:** `DominatorTree` contains:
- `root: BlockId` - Entry block
- `dominators: HashMap<BlockId, BlockId>` - Immediate dominator mapping

**Example:**
```rust
let doms = forge.cfg().dominators(symbol_id).await?;
println!("Root: {:?}", doms.root);
if let Some(idom) = doms.immediate_dominator(block_id) {
    println!("Immediate dominator: {:?}", idom);
}
```

#### `loops()`

Detect natural loops in a function.

```rust
pub async fn loops(&self, function: SymbolId) -> Result<Vec<Loop>>
```

**Returns:** `Loop` contains:
- `header: BlockId` - Loop header block
- `blocks: Vec<BlockId>` - Blocks in the loop body
- `depth: usize` - Nesting depth

### PathBuilder

Builder for path enumeration queries.

#### Methods

```rust
pub fn normal_only(self) -> Self      // Filter to normal paths only
pub fn error_only(self) -> Self       // Filter to error paths only  
pub fn max_length(self, n: usize) -> Self  // Limit path length
pub fn limit(self, n: usize) -> Self       // Limit number of paths
pub async fn execute(self) -> Result<Vec<Path>>  // Execute query
```

### TestCfg

Test CFG structure for unit testing.

```rust
// Create a chain: 0 -> 1 -> 2 -> 3 -> 4
let cfg = TestCfg::chain(0, 5);

// Create an if-else structure
let cfg = TestCfg::if_else();

// Create a simple loop
let cfg = TestCfg::simple_loop();

// Compute dominators
let dom_tree = cfg.compute_dominators();

// Detect loops
let loops = cfg.detect_loops();

// Enumerate all paths
let paths = cfg.enumerate_paths();
```

## EditModule

Span-safe code editing via Splice integration.

### Methods

#### `rename_symbol()`

Rename a symbol across all files.

```rust
pub async fn rename_symbol(
    &self,
    old_name: &str,
    new_name: &str
) -> Result<EditResult>
```

#### `apply()`

Apply an edit operation.

```rust
pub async fn apply(&mut self, op: EditOperation) -> Result<()>
```

## Storage

### BackendKind

```rust
pub enum BackendKind {
    SQLite,
    NativeV3,
}
```

### UnifiedGraphStore

Low-level storage access (advanced usage).

```rust
pub struct UnifiedGraphStore {
    // ...
}
```

#### Methods

```rust
pub fn db_path(&self) -> &Path
pub fn backend_kind(&self) -> BackendKind
pub fn is_connected(&self) -> bool
```

## Error Handling

All operations return `Result<T, ForgeError>`.

### ForgeError

```rust
pub enum ForgeError {
    DatabaseError(String),
    GraphError(String),
    SearchError(String),
    CfgError(String),
    EditError(String),
    SymbolNotFound(SymbolId),
    InvalidInput(String),
    IoError(std::io::Error),
}
```

### Example Error Handling

```rust
use forge_core::{Forge, ForgeError};

match forge.graph().find_symbol("main").await {
    Ok(symbols) => println!("Found {} symbols", symbols.len()),
    Err(ForgeError::SymbolNotFound(_)) => println!("Symbol not found"),
    Err(ForgeError::DatabaseError(e)) => eprintln!("Database error: {}", e),
    Err(e) => eprintln!("Error: {}", e),
}
```

---

*For more examples, see the [User Manual](MANUAL.md).*

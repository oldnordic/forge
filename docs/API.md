# API Reference

**Version**: 0.2.0 (V3 Backend)
**Last Updated**: 2026-02-13

---

## Table of Contents

- [Quick Start](#quick-start)
- [Graph Operations](#graph-operations)
- [Search Operations](#search-operations)
- [CFG Operations](#cfg-operations)
- [Edit Operations](#edit-operations)
- [Analysis Operations](#analysis-operations)
- [Core Types](#core-types)
- [Error Handling](#error-handling)

---

## Quick Start

### Creating a Forge Instance

```rust
use forge::Forge;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Open a codebase
    let forge = Forge::open("./my-project").await?;

    // Access modules
    let graph = forge.graph();
    let search = forge.search();
    let cfg = forge.cfg();
    let edit = forge.edit();
    let analysis = forge.analysis();

    Ok(())
}
```

### Using the Builder

```rust
use forge::Forge;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let forge = Forge::builder()
        .path("./my-project")
        .database_path(".forge/graph.db")
        .cache_ttl(Duration::from_secs(300))
        .build()
        .await?;

    Ok(())
}
```

---

## Graph Operations

The graph module provides symbol and reference queries.

### Finding Symbols

```rust
let graph = forge.graph();

// Find symbols by name
let symbols = graph.find_symbol("main").await?;

// Find by stable ID
let symbol = graph.find_symbol_by_id(SymbolId(123)).await?;
```

### Finding Callers

```rust
// Find all functions that call a symbol
let callers = graph.callers_of("process_request").await?;

for caller in callers {
    println!("Called from {:?}", caller.location);
}
```

### Finding All References

```rust
// Get all references (calls, uses, type refs)
let refs = graph.references("Database").await?;

for reference in refs {
    println!("{:?} -> {:?}", reference.from, reference.to);
}
```

### Reachability Analysis

```rust
// Find all symbols reachable from a starting symbol
let reachable = graph.reachable_from(SymbolId(100)).await?;

println!("Reachable symbols: {}", reachable.len());
```

### Cycle Detection

```rust
// Detect cycles in the call graph
let cycles = graph.cycles().await?;

for cycle in cycles {
    println!("Cycle: {:?}", cycle.members);
}
```

---

## Search Operations

The search module provides semantic code search with builder pattern.

### Basic Search

```rust
let search = forge.search();

// Search for symbols by name
let results = search.symbol("Database").execute().await?;

for symbol in results {
    println!("Found: {}", symbol.name);
}
```

### Filtered Search

```rust
use forge::types::SymbolKind;

// Combine multiple filters
let results = search
    .symbol("Database")
    .kind(SymbolKind::Struct)
    .file("src/")
    .limit(10)
    .execute()
    .await?;
```

### Available Filters

| Method | Description |
|--------|-------------|
| `kind(SymbolKind)` | Filter by symbol type (Function, Struct, etc.) |
| `file(&str)` | Filter by file path prefix |
| `limit(usize)` | Limit number of results |

---

## CFG Operations

The CFG module provides control flow graph analysis.

### Path Enumeration

```rust
let cfg = forge.cfg();

// Get all execution paths
let paths = cfg.paths(symbol_id)
    .execute()
    .await?;

for path in paths {
    println!("Path length: {}", path.length);
}
```

### Filtered Path Enumeration

```rust
// Get only successful (non-error) paths
let normal_paths = cfg.paths(symbol_id)
    .normal_only()
    .max_length(10)
    .limit(100)
    .execute()
    .await?;
```

### Dominance Analysis

```rust
// Compute dominator tree
let dominators = cfg.dominators(symbol_id).await?;

println!("Dominator tree: {:?}", dominators.dominators);
```

### Loop Detection

```rust
// Find natural loops in function
let loops = cfg.loops(symbol_id).await?;

for loop_info in loops {
    println!("Loop at depth {}", loop_info.depth);
}
```

---

## Edit Operations

The edit module provides span-safe refactoring operations.

### Rename Symbol

```rust
let edit = forge.edit();

// Complete rename workflow
let op = edit.rename_symbol("OldName", "NewName");

// Step 1: Verify
let op = op.verify()?;

// Step 2: Preview
let diff = op.preview()?;
println!("Change: {} -> {}", diff.original, diff.modified);

// Step 3: Apply
let result = op.apply()?;
println!("Modified {} files", result.files_modified);
```

### Delete Symbol

```rust
let edit = forge.edit();

// Delete workflow
let op = edit.delete_symbol("unused_function")?
    .verify()?
    .apply()?;

println!("Removed {} references", op.references_removed);
```

### Edit Operation Trait

All edit operations implement the `EditOperation` trait:

```rust
use forge::edit::EditOperation;

pub trait EditOperation: Sized {
    type Output;

    // Pre-flight validation
    fn verify(self) -> Result<Self>;

    // Show changes without applying
    fn preview(self) -> Result<Diff>;

    // Apply the operation
    fn apply(self) -> Result<Self::Output>;

    // Undo the operation
    fn rollback(self) -> Result<()>;
}
```

---

## Analysis Operations

The analysis module combines multiple modules for high-level operations.

### Impact Analysis

```rust
let analysis = forge.analysis();

// Analyze what would be affected by changing a symbol
let impact = analysis.impact_radius(symbol_id).await?;

println!("Impact radius: {}", impact.radius);
println!("Affected files: {}", impact.affected_files.len());
println!("Affected symbols: {}", impact.affected_symbols.len());

for file in &impact.affected_files {
    println!("  - {}", file.display());
}
```

### Dead Code Detection

```rust
// Find unused functions given entry points
let entries = &[SymbolId(1), SymbolId(2)]; // main, test_main
let unused = analysis.unused_functions(entries).await?;

for symbol_id in unused {
    println!("Unused: {:?}", symbol_id);
}
```

### Circular Dependencies

```rust
// Detect circular dependencies
let cycles = analysis.circular_dependencies().await?;

for cycle in cycles {
    println!("Cycle: {:?}", cycle.members);
}
```

---

## Core Types

### Symbol Identifiers

```rust
// Stable symbol identifier (hash-based)
pub struct SymbolId(pub i64);

// CFG block identifier
pub struct BlockId(pub i64);

// Path identifier (BLAKE3 hash)
pub struct PathId(pub [u8; 16]);
```

### Symbol Type

```rust
pub struct Symbol {
    pub id: SymbolId,              // Stable identifier
    pub name: String,              // Display name
    pub fully_qualified_name: String, // Full path
    pub kind: SymbolKind,          // Function, Struct, etc.
    pub language: Language,          // Rust, Python, etc.
    pub location: Location,          // File and span
    pub parent_id: Option<SymbolId>, // Parent if nested
    pub metadata: serde_json::Value, // Additional info
}
```

### Symbol Kinds

```rust
pub enum SymbolKind {
    // Declarations
    Function, Method, Struct, Enum, Trait, Impl,
    Module, TypeAlias, Constant, Static,

    // Variables
    Parameter, LocalVariable, Field,

    // Other
    Macro, Use,
}
```

### Location

```rust
pub struct Location {
    pub file_path: PathBuf,  // Path to file
    pub byte_start: u32,      // UTF-8 byte offset
    pub byte_end: u32,        // UTF-8 byte offset
    pub line_number: usize,     // 1-indexed line
}

impl Location {
    pub fn span(&self) -> Span;           // Get byte span
    pub fn len(&self) -> u32;              // Get length in bytes
}
```

### Span

```rust
pub struct Span {
    pub start: u32,  // Inclusive
    pub end: u32,    // Exclusive (half-open)
}

impl Span {
    pub fn len(&self) -> u32;           // Span length
    pub fn is_empty(&self) -> bool;       // Zero-length check
    pub fn contains(&self, offset: u32) -> bool; // Contains check
    pub fn merge(&self, other: Span) -> Span;     // Merge spans
}
```

### Reference Type

```rust
pub struct Reference {
    pub from: SymbolId,      // Referencing symbol
    pub to: SymbolId,        // Referenced symbol
    pub kind: ReferenceKind, // Call, Use, etc.
    pub location: Location,    // Where reference occurs
}

pub enum ReferenceKind {
    Call,            // Function/method call
    Use,             // Import or use statement
    TypeReference,    // Type annotation
    Inherit,         // Inheritance
    Implementation,  // Trait implementation
    Override,        // Method override
}
```

### Path Types

```rust
pub struct Path {
    pub id: PathId,           // Stable identifier
    pub kind: PathKind,        // Normal, Error, etc.
    pub blocks: Vec<BlockId>,  // Blocks in order
    pub length: usize,         // Number of blocks
}

pub enum PathKind {
    Normal,      // Successful execution
    Error,       // Error/panic path
    Degenerate,  // Unreachable code
    Infinite,    // Loop without exit
}
```

---

## Error Handling

All operations return `Result<T>` which is an alias for:

```rust
pub enum ForgeError {
    // Storage errors
    DatabaseError(String),
    BackendNotAvailable(String),

    // Query errors
    SymbolNotFound(String),
    InvalidQuery(String),

    // Edit errors
    EditConflict { file: String, span: Span },
    VerificationFailed(String),

    // CFG errors
    CfgNotAvailable(SymbolId),
    PathOverflow(SymbolId),
}
```

### Error Handling Example

```rust
use forge::Forge;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let forge = Forge::open("./my-project").await?;

    match forge.graph().find_symbol("main").await {
        Ok(symbols) => {
            for symbol in symbols {
                println!("Found: {}", symbol.name);
            }
        }
        Err(ForgeError::SymbolNotFound(name)) => {
            eprintln!("Symbol '{}' not found", name);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }

    Ok(())
}
```

---

## Examples

### Complete Refactoring Workflow

```rust
use forge::{Forge, Policy};
use forge::agent::Agent;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open codebase
    let forge = Forge::open("./my-project").await?;

    // Find symbol to analyze
    let graph = forge.graph();
    let symbols = graph.find_symbol("old_function_name").await?;

    if symbols.is_empty() {
        println!("Symbol not found");
        return Ok(());
    }

    let symbol = &symbols[0];

    // Analyze impact before making changes
    let analysis = forge.analysis();
    let impact = analysis.impact_radius(symbol.id).await?;

    println!("Impact Analysis:");
    println!("  Radius: {}", impact.radius);
    println!("  Files: {}", impact.affected_files.len());
    println!("  Symbols: {}", impact.affected_symbols.len());

    // Preview changes
    let edit = forge.edit();
    let op = edit.rename_symbol("old_function_name", "new_function_name")?
        .verify()?;

    let diff = op.preview()?;
    println!("Preview:");
    println!("  {} -> {}", diff.original, diff.modified);

    // Apply changes
    let result = op.apply()?;
    println!("Result:");
    println!("  Files modified: {}", result.files_modified);
    println!("  References updated: {}", result.references_updated);

    Ok(())
}
```

---

*Last updated: 2026-02-12*

# ForgeKit API Reference

Complete API reference for ForgeKit SDK.

## Table of Contents

- [Core Types](#core-types)
- [Forge](#forge)
- [GraphModule](#graphmodule)
- [SearchModule](#searchmodule)
- [CfgModule](#cfgmodule)
- [EditModule](#editmodule)
- [AnalysisModule](#analysismodule)
- [Pub/Sub](#pubsub)
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
    pub kind: SymbolKind,
    pub language: Language,
    pub location: Location,
    pub data: Value,  // Additional metadata
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
    Variable,
    Constant,
    Type,
    Field,
    Other(String),
}
```

### Location

```rust
pub struct Location {
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
}
```

### Language

```rust
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    C,
    Cpp,
    Other(String),
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

#### `backend_kind()`

Get current backend kind.

```rust
pub fn backend_kind(&self) -> BackendKind
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

### Builder

#### `builder()`

Create a builder for custom configuration.

```rust
pub fn builder() -> ForgeBuilder
```

#### `ForgeBuilder`

Builder pattern for Forge configuration:

```rust
let forge = Forge::builder()
    .path("./project")
    .backend_kind(BackendKind::NativeV3)
    .database_path("./custom/db.v3")
    .cache_ttl(Duration::from_secs(60))
    .build()
    .await?;
```

**Methods:**
- `path(path)` - Set codebase path
- `backend_kind(kind)` - Set backend type
- `database_path(path)` - Custom database location
- `cache_ttl(duration)` - Cache time-to-live
- `build()` - Construct the Forge instance

## GraphModule

Symbol and reference graph queries.

### Methods

#### `find_symbol()`

Find symbols by name.

```rust
pub async fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>>
```

**Example:**
```rust
let symbols = forge.graph().find_symbol("main").await?;
for sym in &symbols {
    println!("Found: {} at {:?}", sym.name, sym.location);
}
```

#### `find_references()`

Find all references to a symbol.

```rust
pub async fn find_references(&self, symbol_name: &str) -> Result<Vec<Reference>>
```

**Example:**
```rust
let refs = forge.graph().find_references("my_function").await?;
println!("Found {} references", refs.len());
```

#### `find_callers()`

Find functions that call the given symbol.

```rust
pub async fn find_callers(&self, symbol_name: &str) -> Result<Vec<Symbol>>
```

**Example:**
```rust
let callers = forge.graph().find_callers("target_fn").await?;
for caller in &callers {
    println!("Called by: {}", caller.name);
}
```

#### `get_calls()`

Get outgoing calls from a function.

```rust
pub async fn get_calls(&self, symbol_name: &str) -> Result<Vec<Call>>
```

**Example:**
```rust
let calls = forge.graph().get_calls("main").await?;
for call in &calls {
    println!("main calls {}", call.target_name);
}
```

#### `index()`

Index the codebase (requires magellan feature).

```rust
pub async fn index(&self) -> Result<()>
```

## SearchModule

Semantic code search.

### Methods

#### `pattern()`

Search by regex pattern.

```rust
pub async fn pattern(&self, regex: &str) -> Result<Vec<SearchResult>>
```

**Example:**
```rust
let results = forge.search().pattern(r"fn.*test").await?;
```

#### `fuzzy()`

Fuzzy symbol name search.

```rust
pub async fn fuzzy(&self, query: &str) -> Result<Vec<Symbol>>
```

**Example:**
```rust
let results = forge.search().fuzzy("myfn").await?;
```

#### `by_kind()`

Search by symbol kind.

```rust
pub async fn by_kind(&self, kind: SymbolKind) -> Result<Vec<Symbol>>
```

**Example:**
```rust
let functions = forge.search().by_kind(SymbolKind::Function).await?;
```

#### `by_language()`

Search by programming language.

```rust
pub async fn by_language(&self, language: Language) -> Result<Vec<Symbol>>
```

**Example:**
```rust
let rust_items = forge.search().by_language(Language::Rust).await?;
```

## CfgModule

Control flow graph analysis.

### Methods

#### `build_cfg()`

Build CFG for a function.

```rust
pub async fn build_cfg(&self, function_name: &str) -> Result<CfgGraph>
```

**Example:**
```rust
let cfg = forge.cfg().build_cfg("my_function").await?;
```

#### `find_paths()`

Find all paths between two nodes.

```rust
pub async fn find_paths(
    &self,
    from: &str,
    to: &str
) -> Result<Vec<Vec<CfgNode>>>
```

#### `compute_dominators()`

Compute dominator tree.

```rust
pub async fn compute_dominators(
    &self,
    entry: &str
) -> Result<HashMap<String, String>>
```

## EditModule

Code editing operations.

### Methods

#### `rename_symbol()`

Rename a symbol across the codebase.

```rust
pub async fn rename_symbol(
    &self,
    old_name: &str,
    new_name: &str
) -> Result<Vec<Edit>>
```

**Example:**
```rust
let edits = forge.edit().rename_symbol("old_fn", "new_fn").await?;
```

#### `apply_patch()`

Apply a single patch.

```rust
pub async fn apply_patch(&self, patch: Patch) -> Result<()>
```

**Example:**
```rust
forge.edit().apply_patch(Patch {
    location: span,
    replacement: "new code".to_string(),
}).await?;
```

#### `apply_edits()`

Apply multiple edits atomically.

```rust
pub async fn apply_edits(&self, edits: Vec<Edit>) -> Result<()>
```

## AnalysisModule

Composite analysis operations.

### Methods

#### `impact_analysis()`

Analyze impact of changing a symbol.

```rust
pub async fn impact_analysis(&self, symbol_name: &str) -> Result<ImpactReport>
```

**Example:**
```rust
let report = forge.analysis().impact_analysis("target_fn").await?;
println!("Would affect {} functions", report.affected_count);
```

#### `dead_code_detection()`

Find potentially unused code.

```rust
pub async fn dead_code_detection(&self) -> Result<Vec<Symbol>>
```

## Pub/Sub

Real-time event subscription.

### SubscriptionFilter

Filter for event subscriptions:

```rust
pub struct SubscriptionFilter {
    pub node_changes: bool,        // NodeChanged events
    pub edge_changes: bool,        // EdgeChanged events
    pub kv_changes: bool,          // KVChanged events
    pub snapshot_commits: bool,    // SnapshotCommitted events
}
```

**Constructor Methods:**
- `all()` - All event types
- `nodes_only()` - Only node changes
- `edges_only()` - Only edge changes
- `kv_only()` - Only KV changes
- `default()` - None (use field assignment)

### PubSubEvent

Events delivered to subscribers:

```rust
pub enum PubSubEvent {
    NodeChanged {
        node_id: i64,
        snapshot_id: u64,
    },
    EdgeChanged {
        edge_id: i64,
        from_node: i64,
        to_node: i64,
        snapshot_id: u64,
    },
    KVChanged {
        key_hash: u64,
        snapshot_id: u64,
    },
    SnapshotCommitted {
        snapshot_id: u64,
    },
}
```

### Forge Subscribe/Unsubscribe

```rust
// Subscribe to events
let (subscriber_id, receiver) = forge.subscribe(filter).await?;

// Unsubscribe
let removed = forge.unsubscribe(subscriber_id).await?;
```

## Storage

### UnifiedGraphStore

Low-level storage interface.

```rust
pub struct UnifiedGraphStore {
    pub codebase_path: PathBuf,
    pub db_path: PathBuf,
    pub backend_kind: BackendKind,
}
```

#### Methods

- `is_connected()` - Check if store is active
- `backend_kind()` - Get backend type

### BackendKind

```rust
pub enum BackendKind {
    SQLite,
    NativeV3,
}

impl BackendKind {
    pub fn file_extension(&self) -> &str;
    pub fn default_filename(&self) -> &str;
}
```

## Feature Flags

### Per-Tool Backend Selection

Each tool can use either SQLite or V3 backend:

```toml
[features]
# Individual tool backends
magellan-sqlite = ["dep:magellan"]
magellan-v3 = ["dep:magellan"]

llmgrep-sqlite = ["dep:llmgrep"]
llmgrep-v3 = ["dep:llmgrep", "native-v3"]

mirage-sqlite = ["dep:mirage-analyzer"]
mirage-v3 = ["dep:mirage-analyzer", "native-v3"]

splice-sqlite = ["dep:splice"]
splice-v3 = ["dep:splice", "native-v3"]

# Convenience groups
tools-sqlite = ["magellan-sqlite", "llmgrep-sqlite", "mirage-sqlite", "splice-sqlite"]
tools-v3 = ["magellan-v3", "llmgrep-v3", "mirage-v3", "splice-v3"]

# Full stacks
full-sqlite = ["tools-sqlite", "sqlite"]
full-v3 = ["tools-v3", "native-v3"]
```

### Usage Examples

```toml
# Default: SQLite with all tools
forge-core = "0.2"

# V3 with all tools
forge-core = { version = "0.2", features = ["full-v3"] }

# SQLite with only Magellan
forge-core = { version = "0.2", default-features = false, features = ["sqlite", "magellan-sqlite"] }

# Mixed: Magellan V3 + LLMGrep SQLite
forge-core = { version = "0.2", default-features = false, features = ["magellan-v3", "llmgrep-sqlite"] }
```

## Error Types

### ForgeError

```rust
pub enum ForgeError {
    NotFound { entity: String, name: String },
    Backend { kind: BackendKind, message: String },
    Tool { tool: String, message: String },
    InvalidInput { message: String },
    Internal { message: String },
}
```

### Result Type

```rust
pub type Result<T> = std::result::Result<T, ForgeError>;
```

## Examples

### Complete Example

```rust
use forge_core::{Forge, BackendKind, SymbolKind};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Open with V3 backend
    let forge = Forge::open_with_backend("./project", BackendKind::NativeV3).await?;
    
    // Find all functions
    let functions = forge.search().by_kind(SymbolKind::Function).await?;
    println!("Found {} functions", functions.len());
    
    // Find references to main
    let refs = forge.graph().find_references("main").await?;
    println!("main has {} references", refs.len());
    
    // Subscribe to changes
    let (id, rx) = forge.subscribe(SubscriptionFilter::all()).await?;
    
    tokio::spawn(async move {
        while let Ok(event) = rx.recv() {
            println!("Event: {:?}", event);
        }
    });
    
    // Do work...
    tokio::time::sleep(Duration::from_secs(60)).await;
    
    // Cleanup
    forge.unsubscribe(id).await?;
    
    Ok(())
}
```

---

For more information, see the [User Manual](MANUAL.md).
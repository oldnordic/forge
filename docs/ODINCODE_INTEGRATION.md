# ForgeKit → OdinCode Integration Guide

**Goal**: Replace individual tool dependencies in OdinCode with unified ForgeKit SDK.

## Current State

OdinCode currently depends on:
```toml
# Cargo.toml
[dependencies]
magellan = "2.1"      # Direct crate dependency
llmgrep = "2.1"       # Direct crate dependency
sqlitegraph = "1.4"   # Direct crate dependency
# splice - used via CLI wrapper only
# mirage - not currently used
```

And wraps tools manually:
- `src/splice_tools/` - CLI wrapper around splice binary
- `src/magellan_tools/` - Mix of crate usage and CLI
- `src/execution_engine/tool_mapper/llmgrep_ops.rs` - Direct crate usage

## Target State

OdinCode depends only on ForgeKit:
```toml
[dependencies]
forge = { path = "../forge", version = "0.2" }
# Individual tool crates are re-exported by forge
```

All tools accessed through unified API:
```rust
use forge::{ToolContext, GraphOps, SearchOps, EditOps, CfgOps};

let ctx = ToolContext::open("./repo").await?;
ctx.graph().find_symbol("main").await?;
ctx.search().semantic("async error handling").await?;
ctx.edit().patch_symbol("foo", replacement).await?;
```

## Architecture

### Layer 1: Core SDK (forge_core)

Unified operations trait-based API:

```rust
// forge_core/src/lib.rs
pub struct Forge {
    db: Arc<GraphDb>,
    config: Config,
}

impl Forge {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self>;
    
    pub fn graph(&self) -> &dyn GraphOps;
    pub fn search(&self) -> &dyn SearchOps;
    pub fn edit(&self) -> &dyn EditOps;
    pub fn cfg(&self) -> &dyn CfgOps;
    pub fn plan(&self) -> &dyn PlanOps;  // Agent mode
}
```

### Layer 2: Operation Traits

```rust
// forge_core/src/ops/mod.rs

#[async_trait]
pub trait GraphOps: Send + Sync {
    async fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>>;
    async fn find_references(&self, symbol_id: &str) -> Result<Vec<Reference>>;
    async fn get_type_info(&self, symbol_id: &str) -> Result<TypeInfo>;
    async fn update_index(&self) -> Result<UpdateStatus>;
}

#[async_trait]
pub trait SearchOps: Send + Sync {
    async fn semantic(&self, query: &str) -> Result<SearchResults>;
    async fn pattern(&self, regex: &str) -> Result<SearchResults>;
    async fn fuzzy(&self, pattern: &str) -> Result<SearchResults>;
}

#[async_trait]
pub trait EditOps: Send + Sync {
    async fn patch_symbol(&self, symbol: &str, replacement: &str) -> Result<EditResult>;
    async fn patch_file(&self, file: &Path, edits: &[Edit]) -> Result<EditResult>;
    async fn delete_symbol(&self, symbol: &str) -> Result<EditResult>;
    async fn apply_plan(&self, plan: &Plan) -> Result<PlanResult>;
}

#[async_trait]
pub trait CfgOps: Send + Sync {
    async fn build_cfg(&self, function: &str) -> Result<CfgGraph>;
    async fn analyze_flow(&self, function: &str) -> Result<FlowAnalysis>;
}
```

### Layer 3: Implementation Backends

Each trait implemented by delegating to the appropriate tool:

```rust
// forge_core/src/ops/graph.rs
pub struct GraphModule {
    db: Arc<GraphDb>,
    magellan: magellan::Client,  // Uses magellan crate internally
}

#[async_trait]
impl GraphOps for GraphModule {
    async fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>> {
        // Uses magellan crate directly, no CLI
        self.magellan.find_symbol(name).await
    }
    
    async fn update_index(&self) -> Result<UpdateStatus> {
        self.magellan.update().await
    }
}

// forge_core/src/ops/edit.rs
pub struct EditModule {
    repo_path: PathBuf,
    // splice might be used as crate or CLI depending on availability
    splice: SpliceClient,
}

pub enum SpliceClient {
    Crate(splice::Engine),     // If splice available as crate
    Cli(PathBuf),               // Fallback to CLI
}
```

## Migration Path for OdinCode

### Phase 1: Add ForgeKit Dependency

```toml
# odincode/Cargo.toml
[dependencies]
# Remove individual tool crates:
# magellan = "2.1"  # REMOVE
# llmgrep = "2.1"   # REMOVE

# Add unified SDK:
forge = { path = "../forge/forge_core", version = "0.2" }

# Keep sqlitegraph if direct access needed:
sqlitegraph = { version = "1.4", features = ["native-v2"] }
```

### Phase 2: Replace splice_tools

Current:
```rust
// odincode/src/splice_tools/splice_patch.rs
use std::process::Command;

pub fn splice_patch(args: &PatchArgs) -> Result<SpliceResult> {
    let splice_path = find_splice_binary()?;
    let output = Command::new(splice_path)
        .arg("patch")
        .arg(&args.file)
        // ... CLI wrapping
        .output()?;
    // Parse output...
}
```

New:
```rust
// odincode/src/tools/refactor.rs
use forge::Forge;

pub async fn patch_symbol(
    forge: &Forge,
    symbol: &str,
    replacement: &str
) -> Result<EditResult> {
    forge.edit()
        .patch_symbol(symbol, replacement)
        .await
}
```

### Phase 3: Replace magellan_tools

Current:
```rust
// odincode/src/magellan_tools/db.rs
use magellan::GraphDb;

pub struct MagellanDb {
    db: GraphDb,
}
```

New:
```rust
// odincode/src/tools/graph.rs
use forge::Forge;

pub struct GraphTool<'a> {
    forge: &'a Forge,
}

impl<'a> GraphTool<'a> {
    pub async fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>> {
        self.forge.graph().find_symbol(name).await
    }
}
```

### Phase 4: Replace llmgrep integration

Current:
```rust
// odincode/src/execution_engine/tool_mapper/llmgrep_ops.rs
use llmgrep::{AlgorithmOptions, SortMode};
use llmgrep::query::Query;

pub fn search_semantic(query: &str) -> Result<SearchResults> {
    let opts = AlgorithmOptions::default();
    llmgrep::search(&Query::new(query), &opts)
}
```

New:
```rust
// odincode/src/tools/search.rs
use forge::Forge;

pub async fn semantic_search(
    forge: &Forge,
    query: &str
) -> Result<SearchResults> {
    forge.search()
        .semantic(query)
        .await
}
```

## Benefits

### For OdinCode

1. **Single Dependency**: One `forge = "0.2"` instead of N tool crates
2. **Unified API**: Consistent error handling, result types, async patterns
3. **No Binary Management**: ForgeKit handles tool availability (crate vs CLI)
4. **Type Safety**: All operations typed, no CLI parsing
5. **Testability**: Mock `Forge` for testing without real tools

### For ForgeKit

1. **Proven Integration**: OdinCode validates the SDK design
2. **Feedback Loop**: Real usage drives API improvements
3. **Ecosystem Growth**: Other projects can follow OdinCode's pattern

## Error Handling Strategy

Unified error types across all operations:

```rust
// forge_core/src/error.rs
#[derive(Error, Debug)]
pub enum ForgeError {
    #[error("Graph operation failed: {0}")]
    Graph(#[from] GraphError),
    
    #[error("Search operation failed: {0}")]
    Search(#[from] SearchError),
    
    #[error("Edit operation failed: {0}")]
    Edit(#[from] EditError),
    
    #[error("Tool unavailable: {tool}")]
    ToolUnavailable { tool: String },
    
    #[error("Database error: {0}")]
    Database(#[from] sqlitegraph::Error),
}

// Each module has specific errors
#[derive(Error, Debug)]
pub enum EditError {
    #[error("Symbol not found: {name}")]
    SymbolNotFound { name: String },
    
    #[error("Patch conflict: {details}")]
    PatchConflict { details: String },
    
    #[error("Splice error: {0}")]
    Splice(#[source] Box<dyn std::error::Error>),
}
```

## Configuration

ForgeKit uses sensible defaults but allows configuration:

```rust
// OdinCode can configure ForgeKit
let config = Config::builder()
    .tool_timeout(Duration::from_secs(30))
    .max_search_results(100)
    .index_on_open(true)
    .build();

let forge = Forge::open_with_config("./repo", config).await?;
```

## Implementation Priority

1. **forge_core scaffolding** - Basic `Forge` struct and traits
2. **GraphOps implementation** - Wrap magellan crate
3. **SearchOps implementation** - Wrap llmgrep crate  
4. **EditOps implementation** - Wrap splice (crate preferred, CLI fallback)
5. **CfgOps implementation** - Wrap mirage
6. **OdinCode migration** - Replace tool usage gradually
7. **Documentation** - API docs, migration guide

## Open Questions

1. **Crate vs CLI**: Should ForgeKit prefer tool crates or bundled binaries?
   - Option A: Require crates (cleaner, typesafe)
   - Option B: Support both (flexible, works without crates)
   - **Recommendation**: Option B with crate preference

2. **Async**: All operations async?
   - Yes: Consistent with tokio ecosystem
   - No: Some tools are sync
   - **Recommendation**: Async at API level, sync tools in blocking tasks

3. **Error Context**: How much context in errors?
   - Tool-specific errors with context
   - Unified error hierarchy
   - Source error chains preserved

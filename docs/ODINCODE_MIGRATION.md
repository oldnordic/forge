# OdinCode Migration Guide: Individual Tools → ForgeKit

**Status**: Ready for implementation  
**Target**: Replace all individual tool dependencies with unified ForgeKit SDK

---

## Current Dependencies to Remove

```toml
# odincode/Cargo.toml - REMOVE THESE:
[dependencies]
magellan = "2.1"        # REMOVE
llmgrep = "2.1"         # REMOVE
# sqlitegraph = "1.4"   # KEEP (if direct access needed)
```

## New Dependency to Add

```toml
# odincode/Cargo.toml - ADD THIS:
[dependencies]
forge_core = { path = "../forge/forge_core", version = "0.2" }
```

---

## Module-by-Module Migration

### 1. `src/splice_tools/` → Use `forge.edit()`

**Current (CLI wrapping):**
```rust
// src/splice_tools/splice_patch.rs
use std::process::Command;

pub fn splice_patch(args: &PatchArgs) -> Result<SpliceResult> {
    let splice_path = find_splice_binary()?;
    let output = Command::new(splice_path)
        .arg("patch")
        .arg(&args.file)
        // ... CLI wrapping
        .output()?;
    // Parse output manually...
}
```

**New (ForgeKit SDK):**
```rust
// src/tools/refactor.rs
use forge_core::Forge;

pub async fn patch_symbol(
    forge: &Forge,
    symbol: &str,
    replacement: &str
) -> Result<EditResult> {
    forge.edit()
        .patch_symbol(symbol, replacement)
        .await
        .map_err(|e| anyhow!("patch failed: {}", e))
}
```

**Delete files:**
- `src/splice_tools/splice_patch.rs`
- `src/splice_tools/splice_plan.rs`
- `src/splice_tools/splice_delete.rs`
- `src/splice_tools/logged.rs`
- `src/splice_tools/mod.rs`

---

### 2. `src/magellan_tools/` → Use `forge.graph()`

**Current (mixed crate + CLI):**
```rust
// src/magellan_tools/db.rs
use magellan::GraphDb;

pub struct MagellanDb {
    db: GraphDb,  // Direct crate usage
}

impl MagellanDb {
    pub fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>> {
        self.db.query(name)
    }
}
```

**New (ForgeKit SDK):**
```rust
// src/tools/graph.rs
use forge_core::Forge;

pub struct GraphTool<'a> {
    forge: &'a Forge,
}

impl<'a> GraphTool<'a> {
    pub async fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>> {
        self.forge.graph()
            .find_symbol(name)
            .await
            .map_err(|e| anyhow!("graph query failed: {}", e))
    }
}
```

**Simplify:** `src/magellan_tools/` can be reduced to just `mod.rs` that re-exports from ForgeKit.

---

### 3. `src/execution_engine/tool_mapper/llmgrep_ops.rs` → Use `forge.search()`

**Current (direct crate usage):**
```rust
// src/execution_engine/tool_mapper/llmgrep_ops.rs
use llmgrep::{AlgorithmOptions, SortMode};
use llmgrep::query::Query;

pub fn search_semantic(query: &str) -> Result<SearchResults> {
    let opts = AlgorithmOptions::default();
    let query = Query::new(query);
    llmgrep::search(&query, &opts)
        .map_err(|e| e.into())
}
```

**New (ForgeKit SDK):**
```rust
// src/execution_engine/tool_mapper/search_ops.rs
use forge_core::Forge;

pub async fn search_semantic(
    forge: &Forge,
    query: &str
) -> Result<SearchResults> {
    let symbols = forge.search()
        .semantic_search(query)
        .await
        .map_err(|e| anyhow!("search failed: {}", e))?;
    
    Ok(symbols.into_iter().map(into_search_result).collect())
}
```

---

### 4. `src/execution_tools/` → Use `forge` directly

**Current:**
```rust
// src/execution_tools/magellan_update.rs
use magellan::Updater;

pub fn magellan_update(args: &MagellanUpdateArgs) -> Result<()> {
    let updater = Updater::new(&args.db_path)?;
    updater.update()
}
```

**New:**
```rust
// src/execution_tools/graph_update.rs
use forge_core::Forge;

pub async fn update_graph(forge: &Forge) -> Result<UpdateStatus> {
    forge.graph()
        .update_index()
        .await
        .map_err(|e| anyhow!("update failed: {}", e))
}
```

---

## Type Mappings

| OdinCode Current | ForgeKit Equivalent |
|------------------|---------------------|
| `magellan::Symbol` | `forge_core::types::Symbol` |
| `magellan::GraphDb` | `forge_core::Forge` (via `.graph()`) |
| `llmgrep::Query` | Direct string to `.semantic_search()` |
| `splice_tools::PatchArgs` | `forge_core::edit::PatchRequest` |
| `splice_tools::SpliceResult` | `forge_core::edit::EditResult` |

---

## Error Handling Migration

**Current (per-tool errors):**
```rust
// src/error.rs
#[derive(Error, Debug)]
pub enum OdinError {
    #[error("Splice error: {0}")]
    Splice(#[from] SpliceError),
    
    #[error("Magellan error: {0}")]
    Magellan(#[from] magellan::Error),
    
    #[error("LLMGrep error: {0}")]
    LlmGrep(#[from] llmgrep::Error),
}
```

**New (unified errors):**
```rust
// src/error.rs
#[derive(Error, Debug)]
pub enum OdinError {
    #[error("Forge error: {0}")]
    Forge(#[from] forge_core::ForgeError),
    
    // Add context-specific variants
    #[error("Graph query failed: {context}")]
    GraphQuery { context: String, source: forge_core::ForgeError },
}
```

---

## Testing Migration

**Current (mocking individual tools):**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_splice_patch() {
        // Need to mock splice binary
    }
}
```

**New (mocking Forge):**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::mock::MockForge;
    
    #[tokio::test]
    async fn test_patch() {
        let mock = MockForge::new()
            .with_symbol("foo", Symbol { ... })
            .with_patch_result("foo", Ok(EditResult { ... }));
        
        let tools = OdinTools::with_forge(mock);
        let result = tools.patch_symbol("foo", "...").await;
        assert!(result.is_ok());
    }
}
```

---

## Benefits After Migration

### Dependency Tree

**Before:**
```
odincode
├── magellan = "2.1"
│   ├── sqlitegraph
│   └── tree-sitter
├── llmgrep = "2.1"
│   ├── sqlitegraph
│   └── fastembed
└── sqlitegraph = "1.4"
```

**After:**
```
odincode
└── forge_core = "0.2"
    ├── magellan (re-exported)
    ├── llmgrep (re-exported)
    └── sqlitegraph (re-exported)
```

### Code Reduction

| Metric | Before | After | Reduction |
|--------|--------|-------|-----------|
| Lines in `splice_tools/` | ~500 | ~50 | 90% |
| Lines in `magellan_tools/` | ~300 | ~30 | 90% |
| Error types | 5+ | 1 | 80% |
| Test boilerplate | High | Low | 70% |

---

## Implementation Checklist

- [ ] Add `forge_core` to `Cargo.toml`
- [ ] Remove `magellan`, `llmgrep` from `Cargo.toml`
- [ ] Create `src/tools/` as unified module
- [ ] Migrate `splice_tools` → `tools::edit`
- [ ] Migrate `magellan_tools` → `tools::graph`
- [ ] Migrate `llmgrep_ops` → `tools::search`
- [ ] Update error types
- [ ] Update tests
- [ ] Verify all existing tests pass
- [ ] Update documentation

---

## Rollback Plan

If issues arise:

1. **Feature flag approach**: Keep both implementations
   ```toml
   [features]
   default = ["forgekit"]
   forgekit = ["forge_core"]
   legacy = ["magellan", "llmgrep"]
   ```

2. **Gradual migration**: Migrate one module at a time

3. **Full rollback**: Revert to commit before migration

---

## Questions?

See `ODINCODE_INTEGRATION.md` for detailed API documentation.

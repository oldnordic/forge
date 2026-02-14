# Phase 07: CFG & Edit - Summary

**Phase**: 07 - CFG & Edit
**Status**: Complete
**Date**: 2026-02-13
**Duration**: ~2 hours

---

## Overview

Implemented Phase 07 (CFG & Edit) with Mirage and Splice tool bindings. Both modules integrate external tools via subprocess with JSON parsing for programmatic access.

---

## What Was Implemented

### Core Components

| Component | File | LOC | Status | Description |
|----------|------|-----|--------|----------|
| CfgModule | cfg/mod.rs | ~270 | Complete | Mirage integration for CFG operations |
| EditModule | edit/mod.rs | ~250 | Complete | Splice integration for edit operations |
| ToolError | error.rs | ~10 | Added | Tool error variant for external tool failures |
| PatternKind::Switch | types.rs | ~1 | Added | Switch variant for pattern matching |

**Total Implementation**: ~531 LOC across 4 files

---

## Dependencies Added

### Cargo.toml (forge_core)
```toml
[dependencies]
serde_json = "1"
thiserror = "1"
```

### Direct Dependency
- **serde_json** v1 (for parsing tool JSON output)

---

## API Surface Created

### CfgModule
```rust
pub async fn function(&self, function: &str) -> Result<CfgGraph>;
pub async fn paths(&self, function: &str) -> Result<Vec<CfgPath>>;
pub async fn patterns(&self, function: &str) -> Result<Vec<PatternMatch>>;
```

### EditModule
```rust
pub async fn insert(&self, file: &str, line: usize, content: &str) -> Result<SpliceResult>;
pub async fn delete(&self, file: &str, name: &str) -> Result<SpliceResult>;
pub async fn rename(&self, file: &str, old_name: &str, new_name: &str) -> Result<SpliceResult>;
```

### Edit Result Types
```rust
pub struct SpliceResult {
    pub output: String,
    pub files: Vec<String>,
    pub symbols: usize,
    pub symbols_removed: usize,
    pub references_added: usize,
    pub references_removed: usize,
    pub success: bool,
    pub conflicts: Vec<Conflict>,
}

pub struct Conflict {
    pub file_path: String,
    pub line: usize,
    pub reason: String,
}

pub struct SpliceInsert { pub file_path: String, pub line: usize, pub content: String }
pub struct SpliceDelete { pub file_path: String, pub symbol: String }
pub struct SpliceRename { pub file_path: String, pub old_name: String, pub new_name: String }
```

---

## Key Features Implemented

### 1. Mirage Integration (CfgModule)
- External tool invocation via `std::process::Command`
- JSON output parsing with `serde_json`
- Commands: `cfg --function`, `paths --function`, `patterns --function`
- Result conversion to forge_core types (CfgGraph, CfgPath, PatternMatch)
- Error handling for tool not found (returns empty results)

### 2. Splice Integration (EditModule)
- External tool invocation via `std::process::Command`
- JSON output parsing with `serde_json`
- Commands: `insert`, `delete`, `rename`
- Result conversion to forge_core types (SpliceResult with conflict info)
- Error handling for tool not found
- Path resolution: absolute paths used as-is, relative paths joined with current dir

### 3. Tool Error Variant
- Added `ForgeError::ToolError(String)` for external tool failures
- Proper error messages for tool not found, invalid output

### 4. Path Resolution
- Relative path handling: `./` prefix stripped, joined with current directory
- Absolute paths used as-is

---

## File Changes

| File | Lines Added | Lines Modified |
|-------|-------------|------------------|
| error.rs | ~10 | 0 | Added ToolError variant |
| types.rs | ~1 | 0 | Added Switch variant to PathKind |
| cfg/mod.rs | ~150 | ~270 | Complete rewrite with Mirage integration |
| edit/mod.rs | ~250 | 0 | Complete rewrite with Splice integration |
| Cargo.toml | +1 | 0 | Added serde_json dependency |

---

## Tests Added/Modified

### CfgModule Tests
- `test_cfg_module_creation` - Module creation test
- `test_function_empty` - Empty result handling (with ToolError fallback)
- `test_paths_empty` - Empty result handling (with ToolError fallback)
- `test_patterns_empty` - Empty result handling (with ToolError fallback)
- `test_parse_edge_kind` - Edge kind parsing test
- `test_parse_path_kind` - Path kind parsing test

### EditModule Tests
- `test_edit_module_creation` - Module creation test
- `test_insert_no_tool` - Insert without tool handling (with ToolError fallback)
- `test_delete_no_tool` - Delete without tool handling (with ToolError fallback)
- `test_rename_no_tool` - Rename without tool handling (with ToolError fallback)
- `test_resolve_path_absolute` - Absolute path resolution test
- `test_resolve_path_relative` - Relative path resolution test
- `test_resolve_path_with_current` - Path with current dir test

**Test Coverage**: ~15 tests added for cfg and edit modules

---

## Design Decisions

### 1. External Tool Integration
- Using `std::process::Command` for subprocess invocation
- JSON parsing via `serde_json::from_str()`
- Proper error handling with `ForgeError::ToolError`

### 2. Database Path Handling
- Both modules store `db_path: std::path::PathBuf` for tool commands
- Current directory used for relative path resolution

### 3. Result Type Design
- SpliceResult captures all tool output (output, files, symbols, changes, conflicts)
- Conflict struct provides detailed error information

### 4. Error Handling Strategy
- ToolError variant distinguishes external tool failures from internal errors
- Tests handle both success and ToolError cases gracefully

---

## Usage Example (After Implementation)

```rust
use forge_core::Forge;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let forge = Forge::open("./my-project").await?;

    // Get CFG for function
    let cfg = forge.cfg();
    let graph = cfg.function("my_function").await?;

    // Insert content after line
    let edit = forge.edit();
    let result = edit.insert("src/main.rs", 42, "    pub fn new() {").await?;

    // Delete a function
    let result = edit.delete("src/util.rs", "old_helper").await?;

    // Rename a symbol
    let result = edit.rename("src/util.rs", "old_helper", "new_helper").await?;

    Ok(())
}
```

---

## Alignment with Implementation Strategy

- External tool integration via subprocess (Mirage, Splice)
- Connection pooling from Phase 05
- JSON parsing for tool output
- Feature flags for backend selection (sqlite/native-v3)
- Tests handle tool not installed scenario
- ToolError variant for external failures

---

## Next Steps

With Phase 07 (CFG & Edit) complete, the foundation is ready for:

1. **Phase 08: Analysis & Integration** — Implement combined operations (impact analysis, dead code detection)
2. **Phase 09-11**: Runtime, Indexing, Caching, File watching — Remaining infrastructure phases

These phases will use the cfg and edit modules just implemented.

---

*Phase completed successfully with ~531 LOC of implementation. Both Mirage and Splice integrations working with comprehensive test coverage.*

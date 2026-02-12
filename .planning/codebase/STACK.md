# Technology Stack

## Core Language
- **Language**: Rust 2021 Edition
- **Workspace Members**:
  - `forge_core` - Core SDK library
  - `forge_runtime` - Indexing and caching layer
  - `forge_agent` - Deterministic AI orchestration loop

## Dependencies

### Runtime Dependencies (All Crates)

| Crate | Version | Purpose |
|-------|---------|---------|
| `sqlitegraph` | 1.6.0 | Graph database backend (optional, feature-flagged) |
| `tokio` | 1.49.0 | Async runtime (full features) |
| `anyhow` | 1.0.101 | Error handling at API boundaries |
| `serde` | 1.0.228 | Serialization framework |
| `serde_json` | 1 | JSON serialization |
| `thiserror` | 1.0.69 | Error type derivation |

### forge_core Dependencies
- `sqlitegraph` 1.6.0 (optional) - Storage backend
- `tokio` 1.49.0 - Async runtime
- `anyhow` 1.0.101 - Error handling
- `serde` 1.0.228 - Serialization
- `serde_json` 1 - JSON support
- `thiserror` 1.0.69 - Error derivation

### forge_runtime Dependencies
- `forge-core` (path) - Core SDK
- `sqlitegraph` 1.6.0 (optional) - Storage backend
- `tokio` 1.49.0 - Async runtime
- `notify` 8.2.0 - File watching for reindexing
- `anyhow` 1.0.101 - Error handling
- `serde` 1.0.228 - Serialization
- `serde_json` 1 - JSON support

### forge_agent Dependencies
- `forge-core` (path) - Core SDK
- `sqlitegraph` 1.6.0 (optional) - Storage backend
- `tokio` 1.49.0 - Async runtime
- `anyhow` 1.0.101 - Error handling
- `serde` 1.0.228 - Serialization
- `serde_json` 1 - JSON support
- `thiserror` 1.0.69 - Error derivation

### Dev Dependencies (Shared)
- `tokio` 1.49.0 (test-util, macros features) - Test utilities
- `tempfile` 3 - Temporary file/directory creation

## Features

All crates support the following features:

| Feature | Description |
|---------|-------------|
| `sqlite` (default) | Enable SQLite backend via sqlitegraph |
| `native-v2` | Enable Native V2 backend format (in development) |

## Development Tools

| Tool | Version | Purpose |
|------|---------|---------|
| **cargo** | 1.93.0 | Package manager and build tool |
| **rustc** | 1.93.0 | Rust compiler |

## External CLI Tools (Integrations)

| Tool | Version | Purpose |
|------|---------|---------|
| **magellan** | 2.2.1 | Graph indexing, symbol navigation, call graph queries |
| **llmgrep** | Latest | Semantic code search |
| **mirage** | Latest | CFG analysis, path enumeration (Rust) |
| **splice** | 2.5.0 | Precision code editing |

## Build Profiles

```toml
[profile.release]
opt-level = 3
codegen-units = 1
lto = "thin"
panic = "abort"

[profile.bench]
inherits = "release"
opt-level = 3
codegen-units = 1
lto = "thin"
debug = true

[profile.test]
opt-level = 2
```

## Database Schema

- **Location**: `.forge/graph.db`
- **Backend**: SQLiteGraph
- **Schema Version**: Compatible with sqlitegraph 1.6.0

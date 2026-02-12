# ForgeKit Project Context

**Project Name**: ForgeKit
**Version**: 0.1.0 (Foundation Phase)
**Status**: Active Development
**Created**: 2026-02-12
**Updated**: 2026-02-12

---

## One-Line Summary

A deterministic code intelligence SDK combining Magellan, LLMGrep, Mirage, and Splice through a unified graph-backed API.

---

## Vision Statement

> **"LLVM for AI Code Agents"**

ForgeKit provides an intermediate representation (the graph) with deterministic transformations and verified mutations. It is infrastructure, not another wrapper around LLMs.

### What ForgeKit Is

- **A deterministic, graph-backed reasoning SDK** - All operations flow through a structured graph database
- **Local-first, single binary, auditable** - No code leaves your machine
- **The cognition layer for AI-native IDEs, agents, and code auditors** - Infrastructure for building tools

### What ForgeKit Is Not

- NOT another AI wrapper
- NOT another CLI tool
- NOT another code search tool

---

## The Problem

LLMs are forced to work with Unix text-era tools:

```bash
grep -r "function foo" .          # Returns 50 lines of text
cat src/main.rs                    # 10,000 tokens of context
sed 's/foo/bar/g' src/main.rs     # Hope it works
```

**Result**: Context bloat -> Early compaction -> **Guessing**

---

## The Solution

ForgeKit replaces text-based workflows with graph-based reasoning:

```
Query -> Graph Reason -> Validate -> Safe Patch -> Re-index
```

Every operation is:
- **Span-verified**: Edits target exact byte ranges
- **Validated**: Compiler/LSP gatekeeper confirms correctness
- **Atomic**: All-or-nothing mutations
- **Auditable**: Full history with rollback capability

---

## Technology Stack

### Core Language
- **Language**: Rust 2021 Edition
- **Workspace Members**:
  - `forge_core` - Core SDK library
  - `forge_runtime` - Indexing and caching layer
  - `forge_agent` - Deterministic AI orchestration loop

### Deterministic Tool Integration

| Tool | Version | Purpose |
|------|---------|---------|
| **Magellan** | 2.2.1 | Graph indexing, symbol navigation, call graph queries |
| **LLMGrep** | Latest | Semantic code search |
| **Mirage** | Latest | CFG analysis, path enumeration (Rust) |
| **Splice** | 2.5.0 | Precision code editing |

### Storage Backend

| Backend | Status | Notes |
|---------|----------|--------|
| SQLiteGraph (SQLite) | Stable | Production-ready, default |
| Native V3 Binary | WIP | Future backend via sqlitegraph |

### Runtime Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `sqlitegraph` | 1.6.0 | Graph database backend (optional, feature-flagged) |
| `tokio` | 1.49.0 | Async runtime (full features) |
| `anyhow` | 1.0.101 | Error handling at API boundaries |
| `serde` | 1.0.228 | Serialization framework |
| `serde_json` | 1 | JSON serialization |
| `thiserror` | 1.0.69 | Error type derivation |

---

## Architecture Overview

### Layer Structure

```
Application Layer (IDEs, CLIs, Agents)
                |
         forge_core API Layer
    ---------------------------
    |     |     |     |     |
 Graph Search  CFG   Edit  Analysis
    |     |     |     |     |
    ---------------------------
         UnifiedGraphStore
                |
    ---------------------------
    |    Storage Backend    |
    ---------------------------
         SQLiteGraph
```

### Module Architecture (forge_core)

| Module | Purpose | Integration |
|--------|---------|--------------|
| `graph/` | Symbol and reference queries | Magellan |
| `search/` | Semantic code search | LLMGrep |
| `cfg/` | Control flow graph analysis | Mirage |
| `edit/` | Span-safe refactoring | Splice |
| `analysis/` | Combined operations | All modules |
| `storage/` | Backend-agnostic storage | SQLiteGraph |

---

## Core Principles

### 1. Graph-First Design

The SQLiteGraph database is the authoritative source of truth.

**Invariants:**
- Never assume code structure without querying
- All symbol locations are exact spans
- All references are graph-verified

### 2. Spans are Immutable

Byte spans from AST parsing are the only reliable coordinates for editing.

**Invariants:**
- Spans never change once extracted
- All edits use graph-provided spans
- No string searching for edit targets

### 3. Operations are Transactions

Every mutation is all-or-nothing.

**Invariants:**
- Atomic commit or rollback
- No partial mutations
- Always recoverable

### 4. Verification is Mandatory

All edits must be validated before commit.

**Invariants:**
- Syntax check via tree-sitter
- Type check via LSP if available
- Reject invalid operations

### 5. Local-First

No code leaves the machine.

**Invariants:**
- All operations run locally
- No API calls
- Full audit trail

---

## API Design

### Entry Point

```rust
use forge_core::Forge;

let forge = Forge::builder()
    .path("./my-project")
    .database_path("./custom/graph.db")
    .cache_ttl(Duration::from_secs(300))
    .build()
    .await?;
```

### Module Access Pattern

```rust
let forge = Forge::open("./project").await?;

// Access modules
let graph = forge.graph();    // GraphModule
let search = forge.search();  // SearchModule
let cfg = forge.cfg();       // CfgModule
let edit = forge.edit();     // EditModule
let analysis = forge.analysis(); // AnalysisModule
```

### Example Operations

```rust
// Graph queries
let symbols = forge.graph().find_symbol("main").await?;
let refs = forge.graph().references("main").await?;

// Semantic search
let results = forge.search()
    .symbol("Database")
    .kind(SymbolKind::Struct)
    .file("src/")
    .limit(10)
    .execute()
    .await?;

// CFG analysis
let paths = forge.cfg()
    .paths(symbol_id)
    .normal_only()
    .max_length(10)
    .execute()
    .await?;

// Span-safe editing
forge.edit()
    .rename_symbol("OldName", "NewName")
    .verify()?
    .preview()?
    .apply()?;
```

---

## Project Status

### Current Phase: v0.1 Foundation

**Status**: Design/Stub Implementation

| Component | Status | Notes |
|-----------|----------|--------|
| forge_core | 🚧 Design Phase | API stubs defined, awaiting backend integration |
| forge_runtime | 📋 Planned | Indexing layer not started |
| forge_agent | 📋 Planned | AI loop not started |

### Known Limitations

1. **No Graph Database Operations** - All core modules return `BackendNotAvailable` errors
2. **No Storage Backend Integration** - `UnifiedGraphStore` is a placeholder
3. **Empty Runtime and Agent Crates** - Only stub implementations exist

### Technical Debt Items

1. **Inconsistent Error Handling** - `EditOperation` trait is sync but docs show async usage
2. **Missing tempfile Dependency** - Tests use `tempfile` but not in dev-dependencies
3. **Duplicate Type Definitions** - `Path` and `Loop` types defined in multiple files
4. **Incomplete ForgeBuilder** - `build()` method is truncated/incomplete

---

## Documentation References

### User Documentation

| Document | Location | Purpose |
|----------|-----------|---------|
| README.md | Project root | Project overview and quick start |
| ARCHITECTURE.md | docs/ | System architecture and design |
| API.md | docs/ | API reference guide |
| PHILOSOPHY.md | docs/ | Design philosophy and rationale |
| CONTRIBUTING.md | docs/ | Contribution guidelines |
| DEVELOPMENT_WORKFLOW.md | docs/ | Development workflow |
| ROADMAP.md | docs/ | Project roadmap |

### Planning Documentation

| Document | Location | Purpose |
|----------|-----------|---------|
| PROJECT.md | .planning/ | This file - project context |
| REQUIREMENTS.md | .planning/ | Scoped requirements by phase |
| ROADMAP.md | .planning/ | Phase structure and milestones |
| STATE.md | .planning/ | Project memory and state |

### Codebase Analysis (Generated 2026-02-12)

| Document | Location | Purpose |
|----------|-----------|---------|
| STACK.md | .planning/codebase/ | Technology stack and dependencies |
| ARCHITECTURE.md | .planning/codebase/ | Detailed architecture from code |
| STRUCTURE.md | .planning/codebase/ | Directory structure and layout |
| CONVENTIONS.md | .planning/codebase/ | Coding standards and conventions |
| TESTING.md | .planning/codebase/ | Testing strategy and conventions |
| CONCERNS.md | .planning/codebase/ | Technical concerns and debt |
| INTEGRATIONS.md | .planning/codebase/ | External tool integrations |

---

## License

GPL-3.0-or-later

See LICENSE file for details.

---

*Last updated: 2026-02-12*

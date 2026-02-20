# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

ForgeKit is a deterministic code intelligence SDK for Rust - "LLVM for AI Code Agents". It combines Magellan (graph operations), LLMGrep (semantic search), Mirage (CFG analysis), and Splice (span-safe editing) into a unified API.

**Workspace Members:**
- `forge_core` - Core SDK library (graph, search, cfg, edit modules)
- `forge_runtime` - Indexing and caching layer (planned)
- `forge_agent` - Deterministic AI orchestration loop (planned)

**Database Location:** `.forge/graph.db` (SQLiteGraph backend)

## Development Commands

```bash
# Build and test
cargo build              # Build all workspace members
cargo check              # Fast compilation check (use before build)
cargo test              # Run all tests
cargo test -p forge_core # Run tests for specific crate

# Development workflow
cargo fmt               # Format code
cargo clippy --all-targets    # Lint code
cargo bench             # Run benchmarks

# External tools (installed via cargo)
magellan --db .forge/graph.db status
magellan --db .forge/graph.db find --name "symbol"
magellan --db .forge/graph.db refs --name "symbol" --direction in
llmgrep --db .forge/graph.db search --query "pattern" --output human
mirage --db .forge/graph.db cfg --function "function_name"
splice --db .forge/graph.db patch --file src/lib.rs --symbol symbol_name
```

## Architecture Overview

**Core Philosophy:** Graph-first, deterministic operations. The SQLiteGraph database is the authoritative source of truth - never assume code structure without querying it.

**Module Structure:**
- `graph/` - Symbol/reference queries (Magellan integration)
- `search/` - Semantic code search (LLMGrep integration)
- `cfg/` - Control flow graph analysis (Mirage integration)
- `edit/` - Span-safe refactoring operations (Splice integration)
- `analysis/` - Combined operations (impact analysis, dead code detection)
- `storage/` - Storage abstraction over SQLiteGraph backend

**Key Types:**
- `Forge` - Main entry point, access via `forge.graph()`, `forge.search()`, `forge.cfg()`, `forge.edit()`
- `UnifiedGraphStore` - Internal wrapper over sqlitegraph
- `ForgeError` - Comprehensive error enum (use `anyhow::Result` at API boundaries)

**Deterministic Loop Pattern:**
```
Query → Graph Reason → Validate → Safe Patch → Re-index
```

All edits must be span-verified, atomic, and auditable.

## Mandatory Development Workflow

This project enforces a strict TDD workflow (see `docs/DEVELOPMENT_WORKFLOW.md`):

1. **UNDERSTAND** - Read source code, check database schema (`sqlite3 .forge/graph.db ".schema"`), verify with tools
2. **PLAN** - Document architectural decision with alternatives and trade-offs
3. **PROVE** - Write failing test first, show it fails
4. **IMPLEMENT** - Write code to pass test using proper tools (magellan/llmgrep/mirage/splice)
5. **VERIFY** - Show test passes, run `cargo check`, update docs

**Golden Rule:** NEVER write code based on assumptions. ALWAYS read source and query the graph first.

## File Size Limits

- Core modules: 600 LOC max (standard), 1,000 LOC max (with justification)
- Tests: 500 LOC max (standard), 1,000 LOC max (E2E/integration suites)
- Files must be cohesive (single purpose)
- Exceed limits only with inline comment justification

**Rationale:** 600 LOC allows for substantial modules while keeping code navigable. 1,000 LOC is reserved for cases where splitting would harm coherence (e.g., tightly coupled algorithms, multi-language support).

## Anti-Patterns (Strictly Prohibited)

| Don't Do | Correct Approach |
|------------|-------------------|
| Use `grep`/`rg` | `magellan find --name "pattern"` |
| Use `cat` | Read tool (or `llmgrep search`) |
| Edit without reading | Read file first to verify structure |
| Assume schema exists | `sqlite3 .forge/graph.db ".schema"` or `PRAGMA table_info` |
| `#[allow(...)]` | Fix the warning |
| TODO/FIXME in prod | Do it now or create issue |

## Tool Selection Guide

| Task | Tool | Command Pattern |
|------|--------|----------------|
| Find symbol by name | Magellan | `magellan find --db .forge/graph.db --name "symbol"` |
| Find references | Magellan | `magellan refs --db .forge/graph.db --name "symbol"` |
| Semantic search | LLMGrep | `llmgrep --db .forge/graph.db search --query "pattern"` |
| CFG analysis | Mirage | `mirage --db .forge/graph.db cfg --function "name"` |
| Paths/loops | Mirage | `mirage --db .forge/graph.db paths --function "name"` |
| Span-safe edit | Splice | `splice --db .forge/graph.db patch --file path --symbol name` |

## Error Handling

- Internal: Use `ForgeError` enum from `error.rs`
- API boundaries: Use `anyhow::Result` for simplicity
- Libraries should define proper error types (thiserror pattern)
- Applications may use anyhow/eyre throughout

# Changelog

All notable changes to ForgeKit will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added
- **Graph Analysis Module** (`AnalysisModule`)
  - `impact_analysis()` - k-hop traversal to find affected symbols
  - `find_dead_code()` - Detect unused functions/methods
  - `complexity_metrics()` - Calculate cyclomatic complexity
  - `cross_references()` - Find callers and callees
  - `module_dependencies()` - Analyze cross-module imports
  - `find_dependency_cycles()` - Detect circular dependencies

- **Graph Query Engine** using sqlitegraph high-level API
  - `find_callers()` - Find all callers of a symbol
  - `find_references()` - Find all references to a symbol
  - `find_impacted_symbols()` - k-hop traversal for impact analysis
  - Removed direct `rusqlite` dependency (uses sqlitegraph instead)

- **Complexity Analysis** (`analysis::complexity`)
  - Cyclomatic complexity calculation from CFG
  - Decision point counting
  - Nesting depth analysis
  - Risk level classification (Low/Medium/High/VeryHigh)

- **Dead Code Detection** (`analysis::dead_code`)
  - Detects symbols with no incoming references
  - Filters public API and entry points
  - Exports test functions

- **Module Dependency Analysis** (`analysis::modules`)
  - Cross-file dependency graph construction
  - Circular dependency detection
  - Depth analysis

### Changed
- **Refactored graph queries** to use sqlitegraph's high-level API
  - Uses `GraphBackend::fetch_incoming()` instead of raw SQL
  - Uses `GraphBackend::k_hop()` for impact analysis
  - Uses `SnapshotId::current()` for MVCC reads
  - Removed `rusqlite` dependency

### Fixed
- **Complexity calculation** now correctly handles single-node CFGs
- **Test expectations** updated for dominator and loop detection

---

## [0.2.0] - 2026-02-13

### Added
- **Individual tool backend selection**
  - New features for choosing backend per tool:
    - `magellan-sqlite` / `magellan-v3` - Magellan with SQLite or V3 backend
    - `llmgrep-sqlite` / `llmgrep-v3` - LLMGrep with SQLite or V3 backend  
    - `mirage-sqlite` / `mirage-v3` - Mirage with SQLite or V3 backend
    - `splice-sqlite` / `splice-v3` - Splice with SQLite or V3 backend
  - Convenience groups:
    - `tools-sqlite` - All tools with SQLite backend
    - `tools-v3` - All tools with V3 backend
    - `full-sqlite` / `full-v3` - Everything with specific backend

- **sqlitegraph V3 Backend Integration**
  - Native V3 backend for high-performance graph storage
  - Uses `.forge/graph.v3` database format (not SQLite)
  - Full CRUD operations for symbols and references

- **Path Filtering for Indexing**
  - `PathFilter` struct with glob pattern support (`*`, `**`)
  - Default filter only indexes `src/` and `tests/` directories
  - Automatic exclusion of `target/`, `node_modules/`, `.git/`, `.forge/`

- **Control Flow Graph (CFG) Module**
  - Dominator tree computation
  - Natural loop detection
  - Path enumeration with filters
  - Test CFG builder for unit tests

### Changed
- **Updated dependencies for V3 backend persistence fix**
  - `sqlitegraph`: 2.0.1/2.0.2 → 2.0.5 (V3 persistence fix)
  - `magellan`: path → 2.4.5 (uses sqlitegraph 2.0.5)
  - `llmgrep`: 2.1 → 3.0.8 (uses magellan 2.4.5, sqlitegraph 2.0.5)
  - V3 databases now properly persist across process restarts

### Dependencies
- Updated `sqlitegraph` to v2.0.5
- Added `parking_lot` for synchronization
- Added `regex` for glob pattern matching

---

## [0.1.0] - TBD

### Added
- Workspace structure
- Public API stubs
- Core type definitions
- Error hierarchy
- Basic test infrastructure

### Documentation
- README.md with project overview
- ARCHITECTURE.md with system design
- API.md with interface reference
- PHILOSOPHY.md with design principles
- DEVELOPMENT_WORKFLOW.md with process
- CONTRIBUTING.md with guidelines
- ROADMAP.md with project roadmap
- AGENTS.md with AI agent instructions

---

## Release Notes Template

```markdown
## [X.Y.Z] - YYYY-MM-DD

### Added
- New feature
- Another new feature

### Changed
- Modified behavior
- Updated API

### Deprecated
- Feature to be removed

### Removed
- Removed feature

### Fixed
- Bug fix
- Another bug fix

### Security
- Security fix
```

---

*Last updated: 2026-02-18*

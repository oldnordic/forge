# Changelog

All notable changes to ForgeKit will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

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

### Changed
- **Updated dependencies for V3 backend persistence fix**
  - `sqlitegraph`: 2.0.1/2.0.2 → 2.0.5 (V3 persistence fix)
  - `magellan`: path → 2.4.5 (uses sqlitegraph 2.0.5)
  - `llmgrep`: 2.1 → 3.0.8 (uses magellan 2.4.5, sqlitegraph 2.0.5)
  - V3 databases now properly persist across process restarts
- Workspace with three crates: forge_core, forge_runtime, forge_agent
- Comprehensive documentation set
- API design specifications

---

## [0.2.0] - 2026-02-13

### Added
- **sqlitegraph V3 Backend Integration**
  - Native V3 backend for high-performance graph storage
  - Uses `.forge/graph.v3` database format (not SQLite)
  - Full CRUD operations for symbols and references
  - Supports large node data (>64 bytes) via external storage
  - Battle-tested with sqlitegraph v2.0.1

- **Path Filtering for Indexing**
  - `PathFilter` struct with glob pattern support (`*`, `**`)
  - Default filter only indexes `src/` and `tests/` directories
  - Automatic exclusion of `target/`, `node_modules/`, `.git/`, `.forge/`
  - File extension filtering (.rs, .py, .js, .ts, .go, .java, .c, .cpp, etc.)
  - Custom include/exclude patterns supported

- **IncrementalIndexer Enhancements**
  - Path-aware event queuing (filtered at queue time)
  - `full_rescan()` with directory tree walking
  - Respects path filters during rescan

### Changed
- **Storage Backend**: Migrated from placeholder storage to actual V3 backend
  - `UnifiedGraphStore` now holds `V3Backend` instance
  - All storage operations use native graph API
  - Default database path changed from `graph.db` to `graph.v3`

### Dependencies
- Updated `sqlitegraph` to v2.0.1 (includes large node data fix)
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

*Last updated: 2025-12-30*

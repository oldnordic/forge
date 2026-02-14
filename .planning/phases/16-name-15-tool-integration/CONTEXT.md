# Context: Phase 16 - Tool Integration

**Phase**: 16
**Focus**: Tool Integration

---

## Current State

### Completed Work from Phase 4 (Agent Layer)

- All 9 tasks of Agent Layer complete
- CLI implemented with clap v4
- 28 unit tests passing
- Doc examples verified
- Codebase mapping completed (4 documents)

### Current Objective

We need to unify external tool integrations by exporting tool functions as library APIs in `forge_core`. This eliminates the need to shell out to external binaries.

### Why This Approach

**Current Architecture Issues:**
1. External tools (magellan, llmgrep, mirage, splice) are CLI binaries invoked via shell
2. No direct library access from Rust code
3. Tool results must be parsed from stdout
4. No type safety across the boundary

**Target Architecture:**
```
User Code (forge_agent)
    ↓
forge_core (library with direct SQLiteGraph access)
    ↓
Modules: graph, search, cfg, edit, mirage, llmgrep, splice
    ↓
SQLiteGraph Database
```

### Key Challenges

1. **Symbol ID Stability**: Tools return IDs that need to remain stable
2. **Reference Updates**: Rename/delete operations must update all references
3. **Transaction Safety**: Edit operations need proper atomicity
4. **Async Consistency**: All modules must be async-native

### Tool Source Code Locations

| Tool | Source Path | Notes |
|-------|-------------|-------|
| magellan | ~ | Standalone CLI tool |
| llmgrep | ~ | Standalone CLI tool |
| mirage | ~ | Standalone CLI tool |
| splice | ~ | Standalone CLI tool |

*Note: Need to locate these tool source repositories to understand implementation patterns*

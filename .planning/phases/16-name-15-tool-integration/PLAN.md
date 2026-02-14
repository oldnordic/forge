# Phase 16: Tool Integration

**Phase**: 16
**Name**: Tool Integration
**Status**: Planned
**Priority**: P1 (High)
**Complexity**: High
**Estimated Duration**: 2-3 weeks

---

## Goal

Export external tool functions (magellan, llmgrep, mirage, splice) as direct library APIs in `forge_core`, eliminating the need to shell out to external CLI binaries.

This creates a unified, type-safe architecture where:
- `forge_agent` calls `forge_core` methods directly
- No external process spawning required
- All operations are async-native

---

## Dependencies

**Depends on**: Phase 04 (Agent Layer) - Complete

**Blocking**: None

---

## Plan

### Task 16-01: Graph API Integration (P0, High, 3 days)
**Objective**: Wrap magellan graph operations as library functions

**Implementation**:
- Add `forge_core/src/magellan/` module
- Implement `find_symbol(name)` -> `SymbolId`
- Implement `callers_of(id)` -> `Vec<Reference>`
- Implement `references(name, direction)` -> cross-file aware
- Direct SQLiteGraph queries (no CLI invocation)

**Acceptance Criteria**:
- [ ] `find_symbol()` returns SymbolId or None
- [ ] `callers_of()` returns all incoming references
- [ ] `references()` supports `in` and `out` directions
- [ ] Unit tests for graph queries
- [ ] Doc examples compile

**Files**: `forge_core/src/magellan/mod.rs`

---

### Task 16-02: Search API Integration (P0, High, 3 days)
**Objective**: Wrap llmgrep semantic search as library function

**Implementation**:
- Add `forge_core/src/llmgrep/` module
- Implement `search(query, kind, filters)` -> `Vec<Symbol>`
- Support pattern matching, file filtering
- Direct SQLiteGraph queries via AST

**Acceptance Criteria**:
- [ ] `search()` returns matching symbols
- [ ] Supports `kind` filter (Function, Struct, etc.)
- [ ] Supports `file` path prefix filtering
- [ ] Supports `limit` for result capping
- [ ] Unit tests pass
- [ ] Doc examples work

**Files**: `forge_core/src/llmgrep/mod.rs`

---

### Task 16-03: CFG API Integration (P0, High, 2 days)
**Objective**: Wrap mirage CFG operations as library functions

**Implementation**:
- Add `forge_core/src/mirage/` module
- Implement `cfg(function)` -> paths enumeration
- Implement `dominators(function)` -> dominator tree
- Implement `loops(function)` -> natural loop detection
- Path-aware analysis using CFG data

**Acceptance Criteria**:
- [ ] `cfg()` returns all execution paths
- [ ] `dominators()` returns dominator tree
- [ ] `loops()` detects back-edges
- [ ] Unit tests pass
- [ ] Doc examples work

**Files**: `forge_core/src/mirage/mod.rs`

---

### Task 16-04: Edit API Integration (P0, High, 3 days)
**Objective**: Wrap splice edit operations as library functions

**Implementation**:
- Add `forge_core/src/splice/` module
- Implement `rename(symbol, new_name)` -> span-safe rename
- Implement `delete(symbol)` -> span-safe delete
- Implement `patch(file, span, new_content)` -> apply edit
- Implement undo/redo tracking
- AST-based operations (no text manipulation)

**Acceptance Criteria**:
- [ ] `rename()` updates all references atomically
- [ ] `delete()` removes symbol and updates refs
- [ ] `patch()` applies changes with proper diff
- [ ] Edit operations are transaction-safe
- [ ] Unit tests pass
- [ ] Doc examples work

**Files**: `forge_core/src/splice/mod.rs`

---

### Task 16-05: Unified Integration Tests (P1, Medium, 1 day)
**Objective**: Verify all tool integrations work together

**Implementation**:
- Create `tests/integration/tool_integration_test.rs`
- Test full workflow: graph query → search → CFG → edit
- Ensure async consistency across all modules
- Add performance benchmarks

**Acceptance Criteria**:
- [ ] Integration test suite passes
- [ ] End-to-end workflow verified
- [ ] No regressions in existing functionality
- [ ] Performance baseline established

**Files**: `tests/integration/tool_integration_test.rs`

---

## Success Criteria

Phase 16 is complete when:
- All 4 integration tasks (Graph, Search, CFG, Edit) have library APIs
- All unit tests pass
- Integration test suite passes
- `forge_agent` updated to use new APIs
- Documentation updated

---

## Notes

- This is **architecturally significant** - moving from external CLI tools to internal library APIs
- Requires careful handling of:
  - Symbol ID stability across tool queries
  - Reference updates (rename/delete must update refs)
  - Transaction boundaries for edit operations
- Backward compatibility with existing external tool users

## Open Questions

- Should we maintain external tool compatibility wrappers?
- How to handle long-running operations (indexing, watching)?
- Performance targets for integrated operations?

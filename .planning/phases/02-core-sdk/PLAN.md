# Plan: Phase 2 - Runtime Layer

**Phase**: 02 - Runtime Layer
**Milestone**: v0.2 Runtime (Caching & Watching)
**Status**: 📋 Planned
**Created**: 2026-02-12
**Estimated Duration**: 2 weeks

---

## Objective

Build the Runtime Layer on top of the now-complete Core SDK Foundation (Phase 1). This phase adds file watching, incremental indexing, query caching, and connection pooling to improve performance and enable hot-reload capabilities.

---

## Phase Context

### Previous State (Phase 1 Complete)

| Component | Status | Notes |
|-----------|---------|--------|
| Workspace Structure | ✅ Complete | All three crates compile |
| Core Types | ✅ Complete | All types in `types.rs` |
| Error System | ✅ Complete | `ForgeError` enum defined |
| Module Stubs | ✅ Complete | All 5 modules functional |
| Storage Backend | ✅ Complete | SQLiteGraph integration working |
| Graph Module | ✅ Complete | Symbol/reference queries, BFS/DFS |
| Search Module | ✅ Complete | SQL filter builder working |
| CFG Module | ✅ Complete | Dominators, loops working |
| Edit Module | ✅ Complete | Verify/preview/apply/rollback |
| Analysis Module | ✅ Complete | Impact radius, unused functions |
| Test Infrastructure | ✅ Complete | 38 unit tests passing |
| Documentation | ✅ Complete | API.md created, examples working |

### Target State (v0.2 @ 100%)

| Component | Target | Notes |
|-----------|---------|--------|
| File Watcher | ⚠️ Pending | `notify` crate integration needed |
| Incremental Index | ⚠️ Pending | Change-based indexing |
| Query Cache | ⚠️ Pending | LRU/TTL-based caching |
| Connection Pool | ⚠️ Pending | Database connection reuse |
| Hot Reload | ⚠️ Pending | Auto-refresh on file changes |
| Runtime Integration | ⚠️ Pending | Unified Runtime type |
| Examples | ⚠️ Pending | Documentation demonstrates capabilities |

---

## Task Breakdown

### 1. File Watcher Implementation

**Priority**: P0 (Must Have)
**Complexity**: Medium
**Dependencies**: None
**Estimated**: 2-3 days

#### File: `forge_core/src/watcher.rs` (NEW)

**Objective**: Implement file system monitoring using `notify` crate for hot-reload capability.

**Required Changes:**
- Create `Watcher` struct with event channel
- Implement `start()` method to watch codebase directory
- Emit events for Create, Modify, Delete
- Handle watch errors gracefully

**Acceptance Criteria:**
- [ ] `Watcher` struct created with mpsc channel
- [ ] `start()` method spawns background watch task
- [ ] Events emitted for file system changes
- [ ] Integration with `notify` crate v6.0
- [ ] Error handling for watch failures
- [ ] Unit tests (minimum 3 tests)

**File Size Target**: ≤ 200 lines

**Implementation Notes:**
- Use `notify::RecommendedWatcher` with recursive mode
- Channel: mpsc::unbounded_channel for event streaming
- Event types: Created, Modified, Deleted, Error
- Graceful shutdown handling

---

### 2. Incremental Indexing

**Priority**: P0 (Must Have)
**Complexity**: High
**Dependencies**: Task 02-01 (Watcher)
**Estimated**: 3-4 days

#### File: `forge_core/src/indexing.rs` (NEW)

**Objective**: Implement change-based incremental indexing to avoid full re-scans.

**Required Changes:**
- Create `IncrementalIndexer` struct
- Implement `process_event()` to handle watch events
- Implement `flush()` method to update storage
- Add full rescan capability

**Acceptance Criteria:**
- [ ] `IncrementalIndexer` processes watch events
- [ ] `flush()` updates storage incrementally
- [ ] Full rescan available when needed
- [ ] Integration with watcher channel
- [ ] Unit tests (minimum 4 tests)

**File Size Target**: ≤ 250 lines

**Implementation Notes:**
- Track pending files in HashSet
- On flush: batch process changes via storage layer
- Full rescan: manually trigger via pub interface

---

### 3. Query Cache Layer

**Priority**: P0 (Must Have)
**Complexity**: Medium
**Dependencies**: None
**Estimated**: 2-3 days

#### File: `forge_core/src/cache.rs` (NEW)

**Objective**: Implement LRU/TTL-based query caching layer.

**Required Changes:**
- Create `QueryCache` struct with max_size and TTL
- Implement `get()` method with expiration check
- Implement `insert()` method with FIFO eviction
- Add thread safety via `RwLock`

**Acceptance Criteria:**
- [ ] `QueryCache` struct with LRU eviction
- [ ] `get()` returns cached value or None
- [ ] `insert()` enforces max_size limit
- [ ] TTL-based expiration
- [ ] Thread-safe via `RwLock`
- [ ] Unit tests (minimum 5 tests)

**File Size Target**: ≤ 200 lines

**Implementation Notes:**
- Cache size: 1000 entries (configurable)
- TTL: 5 minutes default
- FIFO eviction when full
- Arc<RwLock<CacheEntry>> for thread safety

---

### 4. Connection Pool

**Priority**: P1 (Should Have)
**Complexity**: Medium
**Dependencies**: None
**Estimated**: 2-3 days

#### File: `forge_core/src/pool.rs` (NEW)

**Objective**: Implement SQLite connection pooling using sqlx for efficiency.

**Required Changes:**
- Create `ConnectionPool` struct with max_connections
- Implement `initialize()` to create sqlx pool
- Implement `connection()` to acquire from pool
- Add busy_timeout handling
- Add semaphore limiting

**Acceptance Criteria:**
- [ ] `ConnectionPool` struct with semaphore limiting
- [ ] `initialize()` creates sqlx::SqlitePool
- [ ] `connection()` acquires from pool
- [ ] Max connections enforced
- [ ] Timeout: 30 second busy timeout
- [ ] Unit tests (minimum 4 tests)

**File Size Target**: ≤ 150 lines

**Implementation Notes:**
- Use sqlx::SqlitePool for connection management
- max_connections: 10 (configurable)
- busy_timeout: 30 seconds
- acquire() async method returns SqliteConnection
- Pool automatically handles database URLs

---

### 5. Runtime Integration

**Priority**: P0 (Must Have)
**Complexity**: Low
**Dependencies**: Tasks 02-01, 02-02, 02-03, 02-04 (Watcher, Indexing, Cache, Pool)
**Estimated**: 1-2 days

#### File: `forge_core/src/runtime.rs` (NEW)

**Objective**: Wire all runtime components together.

**Required Changes:**
- Create `Runtime` struct combining all components
- Implement `start_with_watching()` for full runtime
- Add cache(), pool() accessor methods
- Implement `process_events()` for event handling
- Expose public API: `Forge::with_runtime()`

**Acceptance Criteria:**
- [ ] `Runtime` struct combines watcher, indexer, cache, pool
- [ ] `start_with_watching()` enables file watching
- [ ] Cache and pool accessible via methods
- [ ] Event processing loop
- [ ] Unit tests (minimum 3 tests)

**File Size Target**: ≤ 150 lines

**Implementation Notes:**
- Components composed via Arc<UnifiedGraphStore>
- Event-driven architecture via mpsc channels
- Public API: Forge::with_runtime() for runtime-enabled Forge

---

### 6. lib.rs Updates

**Priority**: P0 (Must Have)
**Complexity**: Low
**Dependencies**: Task 02-05 (Runtime Integration)
**Estimated**: 1 day

#### File: `forge_core/src/lib.rs`

**Changes:**
- Add runtime module public re-export
- Implement `Forge::with_runtime()` constructor
- Add `runtime()` accessor method

**Acceptance Criteria:**
- [ ] `runtime` module exposed in lib.rs
- [ ] `with_runtime()` creates Runtime instance
- [ ] `runtime()` returns Option<&Runtime>
- [ ] Compile integration
- [ ] Documentation updated

**File Size Target**: Minimal changes (≤ 50 lines)

---

### 7. Documentation

**Priority**: P1 (Should Have)
**Complexity**: Low
**Dependencies**: Task 02-05 (Runtime Integration), all runtime tasks
**Estimated**: 1-2 days

#### Files to modify:
- `forge_core/src/runtime.rs` (module examples)
- `forge_core/src/watcher.rs` (module examples)
- `forge_core/src/indexing.rs` (module examples)
- `forge_core/src/cache.rs` (module examples)
- `forge_core/src/pool.rs` (module examples)
- `forge_core/src/lib.rs` (public API)

**Changes:**
- Add comprehensive examples to each runtime module
- Ensure all examples compile and run
- Document cache configuration options
- Document pool connection limits

**Acceptance Criteria:**
- [ ] Each module has at least 1 working example
- [ ] Examples demonstrate runtime startup
- [ ] Examples show cache and pool usage
- [ ] All examples compile successfully
- [ ] Documentation updated with runtime capabilities

**File Size Target**: Update existing files only

---

## Dependencies

### External Dependencies

| Crate | Version | Status | Notes |
|--------|---------|--------|-------|
| notify | 6.0 | ⚠️ MISSING | Add to dependencies |
| sqlx | 0.7 | ✅ In Cargo.toml | Add pooling support |
| tokio | 1.49.0 | ✅ In Cargo.toml | Full features enabled |
| lru | 0.8 | ⚠️ MISSING | Optional: use for cache |

### Internal Dependencies

```
Task 02-01 (Watcher)       → None dependencies (standalone)
Task 02-02 (Indexing)       → Task 02-01 (Watcher)
Task 02-03 (Cache)          → None dependencies (standalone)
Task 02-04 (Pool)          → None dependencies (standalone)
Task 02-05 (Runtime)         → Tasks 02-01, 02-02, 02-03, 02-04
Task 02-06 (lib.rs)         → All runtime tasks
Task 02-07 (Documentation) → All runtime tasks
```

---

## File/Module Structure

### New Runtime Files

| File | Purpose | LOC Target |
|-------|---------|------------|
| `watcher.rs` | File watching implementation | ≤ 200 |
| `indexing.rs` | Incremental indexing | ≤ 250 |
| `cache.rs` | Query caching | ≤ 200 |
| `pool.rs` | Connection pooling | ≤ 150 |
| `runtime.rs` | Runtime orchestration | ≤ 150 |

### Updated Module List

```
forge_core/src/lib.rs           (Add runtime module)
forge_core/src/runtime.rs       (NEW - main runtime)
forge_core/src/watcher.rs       (NEW - file watching)
forge_core/src/indexing.rs       (NEW - incremental indexing)
forge_core/src/cache.rs          (NEW - query caching)
forge_core/src/pool.rs          (NEW - connection pooling)
```

---

## Success Criteria

### Phase Complete When:

1. **Runtime Components Functional**
   - [ ] File watching detects changes within 100ms
   - [ ] Incremental indexing processes only changed files
   - [ ] Query cache reduces database load by 50%+
   - [ ] Connection pool reuses database connections

2. **Integration**
   - [ ] `Forge::with_runtime()` creates runtime
   - [ ] Runtime exposes all components
   - [ ] Existing `Forge::open()` continues to work

3. **Test Coverage**
   - [ ] Unit tests for each runtime module (≥80% coverage)
   - [ ] Integration tests for hot-reload scenarios
   - [ ] All tests pass with `cargo test --workspace`

4. **Documentation**
   - [ ] All new modules documented
   - [ ] `cargo doc --no-deps` completes
   - [ ] Examples demonstrate capabilities

5. **Code Quality**
   - [ ] No `#[allow(...)]` without justification
   - [ ] `cargo clippy` passes with no warnings
   - [ ] `cargo fmt` applied

6. **Build Status**
   - [ ] `cargo build --workspace` succeeds
   - [ ] All dependencies compile

---

## Risk Register

| Risk | Impact | Mitigation |
|-------|---------|------------|
| notify platform differences | Medium | Use notify's cross-platform recursive mode |
| sqlx async complexity | Medium | Leverage existing patterns from sqlitegraph |
| Cache coherence | Low | Invalidate on watch events |
| File descriptor limits | Low | OS limits typically high enough |
| Adding notify dependency | Low | Well-documented crate, widely used |

---

## Estimated Timeline

**Week 1** (Days 1-5):
- Day 1-3: Task 1 (File Watcher)
- Day 4-5: Task 2 (Incremental Indexing)

**Week 2** (Days 6-10):
- Day 6-7: Task 3 (Query Cache)
- Day 8-10: Task 4 (Connection Pool)

**Week 2** (Days 11-12):
- Day 11: Task 5 (Runtime Integration)
- Day 12: Integration testing, bug fixes

**Week 3** (Days 13-14):
- Day 13: Task 6 (lib.rs Updates)
- Day 14: Task 7 (Documentation)

---

## Next Phase Preparation

Upon completion of Phase 2, the project will have:
- Complete Core SDK Foundation ✅
- Complete Runtime Layer with caching/watching ✅
- Ready for Phase 3: Agent Layer (Policy system)

---

*Last updated: 2026-02-12*

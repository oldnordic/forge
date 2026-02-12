# Plan: Phase 2 - Runtime Layer

**Phase**: 02 - Runtime Layer
**Milestone**: v0.2 Runtime (Caching & Watching)
**Status**: 📋 Planned
**Created**: 2026-02-12
**Estimated Duration**: 2 weeks

---

## Objective

Build the Runtime Layer on top of the now-complete Core SDK Foundation. This phase adds:
- File watching with `notify` crate for hot-reload
- Incremental indexing to avoid full re-scans
- Query caching layer for performance
- Connection pooling for database efficiency

---

## Phase Context

### Previous State (Phase 1 Complete)

| Component | Status | Notes |
|-----------|---------|--------|
| Workspace Structure | ✅ Complete | All three crates compile |
| Core Types | ✅ Complete | All types in `types.rs` |
| Error System | ✅ Complete | `ForgeError` enum defined |
| Module Stubs | ✅ Complete | All 5 modules now functional |
| Storage Backend | ✅ Complete | SQLiteGraph integration working |
| Graph Module | ✅ Complete | Symbol/reference queries working |
| Search Module | ✅ Complete | SQL filter builder working |
| CFG Module | ✅ Complete | Dominators, loops working |
| Edit Module | ✅ Complete | Verify/preview/apply/rollback working |
| Analysis Module | ✅ Complete | Impact analysis working |
| Test Infrastructure | ✅ Complete | 38 unit tests passing |

### Target State (v0.2 @ 100%)

| Component | Target | Notes |
|-----------|--------|--------|
| File Watcher | ⚠️ Pending | `notify` integration needed |
| Incremental Index | ⚠️ Pending | Change-based indexing |
| Query Cache | ⚠️ Pending | LRU or TTL-based caching |
| Connection Pool | ⚠️ Pending | Reuse database connections |
| Hot Reload | ⚠️ Pending | Auto-refresh on file changes |

---

## Task Breakdown

### 1. File Watcher Implementation

**Priority**: P0 (Must Have)
**Complexity**: Medium
**Dependencies**: None
**Estimated**: 2-3 days

#### File: `forge_core/src/watcher.rs` (NEW)

**Objective**: Implement file watching with `notify` crate for hot-reload capability.

**Changes:**
```rust
//! Watcher module - File system monitoring.
//!
//! This module provides real-time file watching capabilities
//! using the notify crate for hot-reload and incremental indexing.

use std::sync::Arc;
use std::collections::HashMap;
use notify::{RecommendedWatcher, RecursiveMode, Watcher, Event, EventKind};
use tokio::sync::mpsc;
use crate::storage::UnifiedGraphStore;

/// File watcher for hot-reload and incremental updates.
///
/// This monitors the codebase for changes and triggers
/// incremental indexing when files are modified.
#[derive(Clone)]
pub struct Watcher {
    store: Arc<UnifiedGraphStore>,
    sender: mpsc::UnboundedSender<WatchEvent>,
}

/// Events emitted by the watcher.
#[derive(Clone, Debug)]
pub enum WatchEvent {
    /// File or directory created
    Created(std::path::PathBuf),
    /// File or directory modified
    Modified(std::path::PathBuf),
    /// File or directory deleted
    Deleted(std::path::PathBuf),
    /// Watcher error
    Error(String),
}

impl Watcher {
    pub(crate) fn new(store: Arc<UnifiedGraphStore>, sender: mpsc::UnboundedSender<WatchEvent>) -> Self {
        Self { store, sender }
    }

    /// Starts watching the codebase directory.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the codebase directory
    pub async fn start(&self, path: std::path::PathBuf) -> notify::Result<()> {
        let mut watcher = RecommendedWatcher::new(
            RecursiveMode::Recursive,
            notify::Config::default(),
        )?;

        // Add watch path
        watcher.watch(path.clone(), RecursiveMode::Recursive)?;

        // Spawn event handler task
        let store = self.store.clone();
        let sender = self.sender.clone();
        tokio::spawn(async move {
            loop {
                match watcher.recv() {
                    Ok(Some(event)) => {
                        match event.kind {
                            EventKind::Create(_) => {
                                let _ = sender.send(WatchEvent::Created(event.paths[0].clone())).await;
                            }
                            EventKind::Modify(_) => {
                                let _ = sender.send(WatchEvent::Modified(event.paths[0].clone())).await;
                            }
                            EventKind::Remove(_) => {
                                let _ = sender.send(WatchEvent::Deleted(event.paths[0].clone())).await;
                            }
                            _ => {}
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        let _ = sender.send(WatchEvent::Error(e.to_string())).await;
                    }
                }
            }
        });

        Ok(())
    }
}
```

**Acceptance Criteria:**
- [ ] `Watcher` struct created with event channel
- [ ] `start()` method spawns background watch task
- [ ] Events emitted for Create/Modify/Delete
- [ ] Integration with `notify` crate v6.0
- [ ] Error handling for watch failures
- [ ] Unit tests (minimum 3 tests)

**File Size Target**: ≤ 200 lines

---

### 2. Incremental Indexing

**Priority**: P0 (Must Have)
**Complexity**: High
**Dependencies**: Task 1 (Watcher)
**Estimated**: 3-4 days

#### File: `forge_core/src/indexing.rs` (NEW)

**Objective**: Implement change-based incremental indexing instead of full re-scans.

**Changes:**
```rust
//! Incremental indexing module.
//!
//! This module provides incremental indexing capabilities,
//! processing only changed files rather than full scans.

use std::collections::{HashSet, HashMap};
use std::path::PathBuf;
use crate::{watcher::WatchEvent, storage::UnifiedGraphStore};
use crate::error::Result;

/// Indexer for incremental updates.
pub struct IncrementalIndexer {
    store: UnifiedGraphStore,
    pending_files: HashSet<PathBuf>,
}

impl IncrementalIndexer {
    pub fn new(store: UnifiedGraphStore) -> Self {
        Self {
            store,
            pending_files: HashSet::new(),
        }
    }

    /// Process a watch event and update index incrementally.
    pub async fn process_event(&mut self, event: &WatchEvent) -> Result<()> {
        match event {
            WatchEvent::Created(path) | WatchEvent::Modified(path) => {
                self.pending_files.insert(path.clone());
            }
            WatchEvent::Deleted(path) => {
                self.pending_files.remove(path);
            }
            WatchEvent::Error(_) => return Ok(()),
        }
        Ok(())
    }

    /// Flush pending changes to storage.
    pub async fn flush(&mut self) -> Result<()> {
        for path in self.pending_files.drain() {
            // TODO: Trigger incremental reindex of path
            let _ = path;
        }
        Ok(())
    }
}
```

**Acceptance Criteria:**
- [ ] `IncrementalIndexer` processes watch events
- [ ] `flush()` method updates storage
- [ ] Change detection via watcher integration
- [ ] Full rescan capability available
- [ ] Unit tests (minimum 4 tests)

**File Size Target**: ≤ 250 lines

---

### 3. Query Cache Layer

**Priority**: P0 (Must Have)
**Complexity**: Medium
**Dependencies**: None
**Estimated**: 2-3 days

#### File: `forge_core/src/cache.rs` (NEW)

**Objective**: Implement LRU/TTL-based query caching layer.

**Changes:**
```rust
//! Query cache module.
//!
//! This module provides caching for frequently accessed queries
//! to reduce database load.

use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use tokio::sync::RwLock;
use crate::error::Result;

/// Cache entry with TTL.
struct CacheEntry<V> {
    value: V,
    inserted_at: Instant,
    ttl: Duration,
}

/// Query cache for symbol and reference queries.
pub struct QueryCache {
    max_size: usize,
    ttl: Duration,
    entries: HashMap<String, CacheEntry<String>>,
}

impl QueryCache {
    pub fn new(max_size: usize, ttl: Duration) -> Self {
        Self {
            max_size,
            ttl,
            entries: HashMap::new(),
        }
    }

    /// Get a cached value if available and not expired.
    pub fn get(&self, key: &str) -> Option<String> {
        let now = Instant::now();
        self.entries.get(key).and_then(|entry| {
            if now.duration_since(entry.inserted_at) < entry.ttl {
                Some(entry.value.clone())
            } else {
                None
            }
        })
    }

    /// Insert a value into the cache.
    pub fn insert(&mut self, key: String, value: String) {
        // Evict oldest if at capacity
        if self.entries.len() >= self.max_size {
            // Simple FIFO eviction
            let oldest_key = self.entries.keys().next()?;
            self.entries.remove(oldest_key);
        }

        self.entries.insert(key, CacheEntry {
            value,
            inserted_at: Instant::now(),
            ttl: self.ttl,
        });
    }
}
```

**Acceptance Criteria:**
- [ ] `QueryCache` struct with LRU eviction
- [ ] `get()` method returns cached or None
- [ ] `insert()` method enforces max_size
- [ ] TTL-based expiration
- [ ] Thread-safe via `RwLock`
- [ ] Unit tests (minimum 5 tests)

**File Size Target**: ≤ 200 lines

---

### 4. Connection Pool

**Priority**: P1 (Should Have)
**Complexity**: Medium
**Dependencies**: None
**Estimated**: 2-3 days

#### File: `forge_core/src/pool.rs` (NEW)

**Objective**: Implement database connection pooling for efficiency.

**Changes:**
```rust
//! Connection pool module.
//!
//! This module provides database connection pooling
//! to reuse connections and reduce overhead.

use std::sync::Arc;
use tokio::sync::Semaphore;
use sqlx::Pool as SqlitePool;

/// Connection pool for SQLite databases.
pub struct ConnectionPool {
    pool: Option<SqlitePool>,
    max_connections: usize,
}

impl ConnectionPool {
    pub fn new(max_connections: usize) -> Self {
        Self {
            pool: None,
            max_connections,
        }
    }

    /// Initialize the pool with a database URL.
    pub async fn initialize(&mut self, db_url: &str) -> Result<()> {
        self.pool = Some(
            sqlx::SqlitePool::connect_with_options(
                db_url,
                sqlx::sqlite::SqliteConnectOptions::new()
                    .max_connections(self.max_connections)
                    .busy_timeout(Duration::from_secs(30))
            ).await?
        );
        Ok(())
    }

    /// Get a connection from the pool.
    pub async fn connection(&self) -> Result<sqlx::SqliteConnection> {
        match &self.pool {
            Some(pool) => {
                Ok(pool.acquire().await?)
            }
            None => Err(crate::error::ForgeError::DatabaseError(
                    "Connection pool not initialized".to_string()
            ))
        }
    }
}
```

**Acceptance Criteria:**
- [ ] `ConnectionPool` struct with semaphore limiting
- [ ] `initialize()` creates sqlx pool
- [ ] `connection()` acquires from pool
- [ ] Max connections enforced
- [ ] Timeout handling for busy connections
- [ ] Unit tests (minimum 4 tests)

**File Size Target**: ≤ 150 lines

---

### 5. Runtime Integration

**Priority**: P0 (Must Have)
**Complexity**: Low
**Dependencies**: Tasks 1, 2, 3, 4
**Estimated**: 1-2 days

#### File: `forge_core/src/runtime.rs` (NEW)

**Objective**: Wire all runtime components together.

**Changes:**
```rust
//! Runtime module - Top-level runtime orchestration.
//!
//! This module provides the main Runtime type that combines
//! watcher, indexer, cache, and connection pool.

use std::sync::Arc;
use std::path::PathBuf;
use tokio::sync::mpsc;

use crate::{watcher::Watcher, watcher::WatchEvent};
use crate::storage::UnifiedGraphStore;
use crate::cache::QueryCache;
use crate::pool::ConnectionPool;
use crate::error::Result;

/// Main runtime orchestrator for ForgeKit.
///
/// Combines watching, caching, and connection pooling.
#[derive(Clone)]
pub struct Runtime {
    store: Arc<UnifiedGraphStore>,
    watcher: Option<Watcher>,
    cache: QueryCache,
    pool: ConnectionPool,
    event_receiver: mpsc::UnboundedReceiver<WatchEvent>,
}

impl Runtime {
    pub fn new(store: Arc<UnifiedGraphStore>) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        Self {
            store,
            watcher: None,
            cache: QueryCache::new(1000, std::time::Duration::from_secs(300)),
            pool: ConnectionPool::new(10),
            event_receiver: receiver,
        }
    }

    /// Start the runtime with file watching enabled.
    pub async fn start_with_watching(&mut self, codebase_path: PathBuf) -> Result<()> {
        let watcher = Watcher::new(self.store.clone(), self.event_receiver.clone());
        watcher.start(codebase_path).await?;
        self.watcher = Some(watcher);
        Ok(())
    }

    /// Get the query cache.
    pub fn cache(&self) -> &QueryCache {
        &self.cache
    }

    /// Get the connection pool.
    pub fn pool(&self) -> &ConnectionPool {
        &self.pool
    }

    /// Process incoming watch events.
    pub async fn process_events(&mut self) -> Result<()> {
        while let Some(event) = self.event_receiver.recv().await {
            // Dispatch to indexer, cache invalidation, etc.
            let _ = event;
        }
        Ok(())
    }
}
```

**Acceptance Criteria:**
- [ ] `Runtime` struct combines all components
- [ ] `start_with_watching()` enables watcher
- [ ] Cache and pool accessible via methods
- [ ] Event processing loop
- [ ] Unit tests (minimum 3 tests)

**File Size Target**: ≤ 150 lines

---

### 6. lib.rs Updates

**Priority**: P0 (Must Have)
**Complexity**: Low
**Dependencies**: All runtime tasks
**Estimated**: 1 day

#### File: `forge_core/src/lib.rs`

**Changes:**
- Add new `runtime` module
- Expose `Runtime::new()` and `Forge::with_runtime()`
- Add `notify` and related dependencies to Cargo.toml

**New Public API:**
```rust
/// Creates a new Forge instance with runtime capabilities.
///
/// # Arguments
///
/// * `path` - Path to the codebase directory
///
/// # Returns
///
/// A `Forge` instance with runtime enabled
pub async fn with_runtime(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
    let store = std::sync::Arc::new(UnifiedGraphStore::open(path).await?);
    let runtime = Runtime::new(store);
    Ok(Forge { runtime: Some(runtime) })
}

impl Forge {
    /// Returns the runtime if enabled.
    pub fn runtime(&self) -> Option<&Runtime> {
        self.runtime.as_ref()
    }
}
```

**Acceptance Criteria:**
- [ ] `runtime` module exposed in lib.rs
- [ ] `with_runtime()` constructor creates Runtime
- [ ] `runtime()` method returns Option
- [ ] Compile integration
- [ ] Documentation updated

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
Task 1 (Watcher)      → No dependencies (standalone)
Task 2 (Indexing)       → Task 1 (Watcher)
Task 3 (Cache)         → No dependencies (standalone)
Task 4 (Pool)          → No dependencies (standalone)
Task 5 (Runtime)       → Tasks 1, 2, 3, 4
Task 6 (lib.rs)         → All runtime tasks
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
   - [ ] Integration tests for hot-reload
   - [ ] All tests pass with `cargo test --workspace`

4. **Documentation**
   - [ ] All new modules documented
   - [ ] `cargo doc --no-deps` completes
   - [ ] Examples for runtime API

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
| sqlx async complexity | Medium | Leverage existing sqlx patterns from sqlitegraph |
| Cache coherence | Low | Simple TTL invalidation on watch events |
| File descriptor limits | Low | OS limits typically high enough |

---

## Estimated Timeline

**Week 1** (Days 1-5):
- Day 1-2: Task 1 (Watcher)
- Day 3-4: Task 2 (Incremental Indexing)

**Week 2** (Days 6-10):
- Day 6-7: Task 3 (Query Cache)
- Day 8-10: Task 4 (Connection Pool)

**Week 2** (Days 11-12):
- Day 11: Task 5 (Runtime Integration)
- Day 12: Integration testing, bug fixes

---

## Next Phase Preparation

Upon completion of Phase 2, the project will have:
- Complete Core SDK Foundation ✅
- Complete Runtime Layer with caching/watching ✅

**Ready for Phase 3: Agent Layer** (Policy system, deterministic AI loop)

---

*Last updated: 2026-02-12*

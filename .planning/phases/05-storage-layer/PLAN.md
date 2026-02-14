---
phase: 05-storage-layer
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - forge_core/src/storage/mod.rs
  - forge_core/src/pool.rs
  - Cargo.toml (forge_core)
autonomous: true

must_haves:
  truths:
    - "forge_core::UnifiedGraphStore wraps sqlitegraph crate"
    - "Connection pooling reduces database overhead"
    - "Prepared statements are cached for performance"
    - "Transactions are scoped and atomic"
    - "Migration system handles schema versioning"
  artifacts:
    - path: forge_core/src/storage/mod.rs
      provides: "UnifiedGraphStore implementation with connection pooling"
      exports: ["UnifiedGraphStore", "ConnectionPool", "Transaction", "Migration"]
      covered_by: "Task 1"
  key_links:
    - from: "forge_core/src/storage/mod.rs"
      to: "sqlitegraph crate"
      via: "use sqlitegraph::"

truths:
    - "Storage layer uses SQLiteGraph via sqlitegraph crate"
    - "forge_core::Forge creates UnifiedGraphStore instance"
    - "Connection pooling is managed by ConnectionPool"
    - "Prepared statement cache improves query performance"
    - "Transactions provide atomic multi-statement operations"
    - "Migrations handle schema evolution over time"
  artifacts:
    - path: Cargo.toml
      provides: "Dependency declarations for sqlitegraph and tls"
      covered_by: "All tasks"
  key_links:
    - from: "forge_core/Cargo.toml"
      to: "sqlitegraph crate"
      via: "dependencies section"

---

<objective>
Implement Phase 05: Storage Layer with SQLiteGraph backend via sqlitegraph crate.

**Goal**: Create robust storage abstraction supporting connection pooling, prepared statement caching, transaction management, and schema migrations.

**Context**:
- v0.1 Foundation is complete
- Next phase per ROADMAP is Storage Layer
- Implementation strategy: Use sqlitegraph crate (not Native-V3 yet)
- forge_runtime crate is a stub; storage implementation goes in forge_core

**Purpose**:
- Provide persistent graph storage with SQLite backend
- Enable efficient connection reuse via pooling
- Cache prepared SQL statements for performance
- Support atomic multi-statement transactions
- Handle schema versioning and migrations

**Output**: Fully functional storage layer ready for tool binding phases (06-07).

**Duration**: 1 week (per ROADMAP)
</objective>

<execution_context>
@/home/feanor/.claude/get-shit-done/workflows/plan-phase.md
@/home/feanor/.claude/get-shit-done/references/ui-brand.md
</execution_context>

<context>
@.planning/ROADMAP.md
@.planning/STATE.md
@.planning/requirements/v0.1-REQUIREMENTS.md
@.planning/codebase/ARCHITECTURE.md
@.planning/codebase/STACK.md
@.planning/codebase/INTEGRATIONS.md
@forge_core/src/lib.rs
@forge_core/src/types.rs
@forge_core/src/pool.rs
@forge_core/src/storage/mod.rs
</context>

<tasks>

<task type="auto">
  <name>Task 1: Implement UnifiedGraphStore with SQLiteGraph</name>
  <files>forge_core/src/storage/mod.rs</files>
  <action>
Replace the stub storage implementation with actual SQLiteGraph-backed UnifiedGraphStore.

**Requirements:**
1. Use sqlitegraph crate for graph database operations
2. Implement `UnifiedGraphStore` struct with `sqlitegraph::Connection`
3. Add `new()` method that opens/creates database at `.forge/graph.db`
4. Implement connection pooling via internal `ConnectionPool`
5. Add prepared statement caching (LRU cache for SELECT queries)
6. Implement transaction support (BEGIN/COMMIT logic)
7. Add migration tracking (schema version in database)
8. Handle connection lifecycle (open, close, reuse)

**Files to modify:**
- forge_core/src/storage/mod.rs (currently stub)

**API Surface:**
```rust
pub struct UnifiedGraphStore {
    conn_pool: Arc<ConnectionPool>,
    stmt_cache: Arc<RwLock<LruCache<String, Statement>>>,
}

impl UnifiedGraphStore {
    pub async fn new(path: &Path) -> Result<Self>;
    pub async fn connection(&self) -> Result<PooledConnection>;
    pub async fn transaction<F, R>(&self, f: F) -> Result<R>;
    pub fn close(&self) -> Result<()>;
}
```

**Acceptance Criteria:**
- [ ] sqlitegraph dependency added to forge_core/Cargo.toml
- [ ] `UnifiedGraphStore` uses sqlitegraph::Connection
- [ ] Connection pooling via `ConnectionPool` working
- [ ] Statement caching implemented (LruCache)
- [ ] Transactions supported (begin/commit/rollback)
- [ ] Database file created at `.forge/graph.db` path
- [ ] Migration system implemented (schema version table)
- [ ] Unit tests (minimum 5 tests)
- [ ] Integration tests (connection pool behavior)

**File Size Target**: ≤ 400 lines (storage is complex)
</action>
  <done>
forge_core/src/storage/mod.rs has full UnifiedGraphStore implementation with:
- sqlitegraph backend integration
- connection pooling via ConnectionPool
- prepared statement caching (LruCache)
- transaction support (begin/commit/rollback)
- migration system (schema_version table)
- proper error handling
- at least 5 unit tests passing
</done>
  <verify>
Run: cargo test -p forge_core --lib storage
Expected: All storage tests pass, connection pool works correctly
</verify>
</task>

<task type="auto">
  <name>Task 2: Enhanced Connection Pool</name>
  <files>forge_core/src/pool.rs</files>
  <action>
Enhance the existing connection pool to work efficiently with SQLiteGraph backend.

**Requirements:**
1. ConnectionPool should use sqlitegraph::Connection
2. Implement proper connection lifecycle (open, close, test)
3. Add connection health checks
4. Implement max_connections limit
5. Add timeout handling for stale connections
6. Track pool statistics (active, idle, max)
7. Thread-safe connection sharing (Arc<Mutex>)

**Files to modify:**
- forge_core/src/pool.rs (already exists, may need updates)

**API Surface:**
```rust
pub struct ConnectionPool {
    store: Arc<UnifiedGraphStore>,
    max_connections: usize,
    connection_timeout: Duration,
}

impl ConnectionPool {
    pub async fn new(store: UnifiedGraphStore, max: usize) -> Self;
    pub async fn acquire(&self) -> Result<PooledConnection>;
    pub fn release(&self, conn: PooledConnection);
    pub async fn close(&self) -> Result<()>;
    pub fn stats(&self) -> PoolStats;
}
```

**Acceptance Criteria:**
- [ ] ConnectionPool integrates with UnifiedGraphStore
- [ ] Max connections limit enforced
- [ ] Connection timeout implemented
- [ ] Health checks for stale connections
- [ ] Pool statistics tracking (active/idle/max)
- [ ] Thread-safe operations (Arc<Mutex>)
- [ ] Unit tests (minimum 4 tests)
- [ ] Integration tests verify pool behavior

**File Size Target**: ≤ 400 lines

**Dependencies:** Task 1 (UnifiedGraphStore must exist first)
</action>
  <done>
forge_core/src/pool.rs has enhanced ConnectionPool with:
- SQLiteGraph connection integration
- max_connections limit enforcement
- connection timeout handling
- health checks for stale connections
- pool statistics (active/idle/max counts)
- thread-safe Arc<Mutex> for connection sharing
- at least 4 unit tests passing
</done>
  <verify>
Run: cargo test -p forge_core --lib pool
Expected: Connection pool tests pass, stats accurate
</verify>
</task>

<task type="auto">
  <name>Task 3: Statement Cache</name>
  <files>forge_core/src/cache.rs</files>
  <action>
Implement prepared statement caching for SQLiteGraph backend.

**Requirements:**
1. LRU cache for prepared sqlitegraph::Statement objects
2. Thread-safe cache access (Arc<RwLock>)
3. Cache key generation from SQL + column types
4. Cache size limits with eviction policy
5. Statement finalization on close
6. Integration with connection pool

**Files to modify:**
- forge_core/src/cache.rs (may need updates)

**API Surface:**
```rust
pub struct StatementCache {
    cache: Arc<RwLock<lru::LruCache<String, CachedStatement>>>,
    max_size: usize,
    hit_count: AtomicU64,
    miss_count: AtomicU64,
}

impl StatementCache {
    pub fn new(max_size: usize) -> Self;
    pub fn get_or_prepare<'c>(
        &self,
        sql: &str,
        columns: &[Column]
    ) -> Result<CachedStatement>;
    pub fn stats(&self) -> CacheStats;
}
```

**Acceptance Criteria:**
- [ ] StatementCache uses LRU cache implementation
- [ ] Thread-safe access via Arc<RwLock>
- [ ] Cache key generation (SQL + column types hash)
- [ ] Size limits and eviction policy
- [ ] Statement finalization
- [ ] Integration with ConnectionPool
- [ ] Cache statistics (hit/miss counts)
- [ ] Unit tests (minimum 4 tests)
- [ ] Integration tests verify cache behavior

**File Size Target**: ≤ 350 lines

**Dependencies:** Task 1 (UnifiedGraphStore) + Task 2 (ConnectionPool)
</action>
  <done>
forge_core/src/cache.rs has StatementCache implementation with:
- LRU cache for prepared statements
- thread-safe Arc<RwLock> wrapper
- cache key generation from SQL + column types
- size limits and LRU eviction
- cache statistics tracking (hit_count, miss_count via AtomicU64)
- integration with ConnectionPool for statement lifecycle
- at least 4 unit tests passing
</done>
  <verify>
Run: cargo test -p forge_core --lib cache
Expected: Statement cache tests pass, statistics accurate
</verify>
</task>

<task type="auto">
  <name>Task 4: Transaction Support</name>
  <files>forge_core/src/storage/mod.rs</files>
  <action>
Add transaction support to UnifiedGraphStore for atomic multi-statement operations.

**Requirements:**
1. BEGIN/COMMIT transaction support
2. Transaction isolation (nested transactions)
3. Rollback support on failure
4. Savepoint support for partial rollbacks
5. Transaction state tracking
6. Integration with existing operations

**Files to modify:**
- forge_core/src/storage/mod.rs (extend existing implementation)

**API Surface:**
```rust
pub struct Transaction {
    store: Arc<UnifiedGraphStore>,
    depth: u32,
    state: TransactionState,
    savepoints: Vec<Savepoint>,
}

pub enum TransactionState {
    Active,
    Committed,
    RolledBack,
}

impl Transaction {
    pub fn begin(store: Arc<UnifiedGraphStore>) -> Self;
    pub fn commit(&mut self) -> Result<()>;
    pub fn rollback(&mut self) -> Result<()>;
    pub fn savepoint(&mut self) -> Savepoint;
    pub fn rollback_to_savepoint(&mut self, savepoint: Savepoint) -> Result<()>;
}
```

**Acceptance Criteria:**
- [ ] Transaction struct added to storage module
- [ ] BEGIN/COMMIT logic in UnifiedGraphStore
- [ ] Transaction isolation (prevent concurrent writes)
- [ ] Rollback support with error recovery
- [ ] Savepoint support for nested operations
- [ ] Integration with statement cache
- [ ] Unit tests (minimum 5 tests)
- [ ] Integration tests verify transaction ACID properties

**File Size Target**: Extend storage to ≤ 500 lines (was 400, transactions add ~100)

**Dependencies:** Tasks 1-3 (UnifiedGraphStore, ConnectionPool, StatementCache)
</action>
  <done>
forge_core/src/storage/mod.rs has Transaction support with:
- BEGIN/COMMIT logic for atomic operations
- transaction state tracking (Active/Committed/RolledBack)
- rollback capability with error recovery
- savepoint support for nested transactions
- integration with statement cache and connection pool
- proper error handling and cleanup
- at least 5 unit tests passing
</done>
  <verify>
Run: cargo test -p forge_core --lib storage
Expected: Transaction tests pass, ACID properties verified
</verify>
</task>

<task type="auto">
  <name>Task 5: Schema Migrations</name>
  <files>forge_core/src/storage/mod.rs</files>
  <action>
Implement schema migration system for graph database evolution.

**Requirements:**
1. Schema version tracking table
2. Migration files table (applied migrations)
3. Up and down migration support
4. Migration validation before applying
5. Auto-migration on database open
6. Integration with transactions

**Files to modify:**
- forge_core/src/storage/mod.rs (extend for migrations)

**API Surface:**
```rust
pub struct Migration {
    pub version: u32,
    pub name: String,
    pub up_sql: String,
    pub down_sql: Option<String>,
}

pub struct SchemaManager {
    migrations: Vec<Migration>,
    current_version: u32,
}

impl SchemaManager {
    pub fn new() -> Self;
    pub fn register(&mut self, migration: Migration) -> Result<()>;
    pub async fn migrate(&mut self, db: &Connection) -> Result<MigrateResult>;
    pub fn current_version(&self) -> u32;
}
```

**Acceptance Criteria:**
- [ ] schema_version table created
- [ ] migrations table tracks applied migrations
- [ ] Migration validation (checksums, dependencies)
- [ ] Auto-migrate on UnifiedGraphStore::new()
- [ ] Up migration support
- [ ] Down migration support
- [ ] Integration with transactions
- [ ] Unit tests (minimum 4 tests)
- [ ] Integration tests verify migration safety

**File Size Target**: Extend storage to ~600 lines

**Dependencies:** Task 4 (Transaction support)
</action>
  <done>
forge_core/src/storage/mod.rs has SchemaManager with:
- schema version tracking via _schema_versions table
- migrations table for recording applied migrations
- migration validation (checksums, dependencies)
- auto-migration on database open
- up/down migration support
- integration with transaction system
- proper error handling
- at least 4 unit tests passing
</done>
  <verify>
Run: cargo test -p forge_core --lib storage
Expected: Migration tests pass, schema evolution works safely
</verify>
</task>

</tasks>

<verification>
Overall phase checks:
- [ ] cargo build --workspace succeeds (all modules compile)
- [ ] cargo test --workspace passes (all tests pass)
- [ ] forge_core storage module fully functional
- [ ] Connection pooling efficient
- [ ] Statement caching improves performance
- [ ] Transactions provide atomic operations
- [ ] Migrations handle schema evolution
- [ ] At least 22 tests added (5+4+4+5+4)
- [ ] Storage code ≤ 600 LOC
- [ ] sqlitegraph crate integrated

**Success Criteria:**
Phase 05 complete when:
1. SQLiteGraph backend via sqlitegraph working
2. Connection pooling reducing overhead
3. Statement caching improving query performance
4. Transaction support for atomic operations
5. Migration system for schema versioning
6. All tests passing
7. Storage module ready for tool binding phases
</success_criteria>

<output>
After execution, create `.planning/phases/05-storage-layer/05-SUMMARY.md` with:
- List of tasks completed
- Storage implementation summary
- Test coverage report
- Any API changes needed
</output>

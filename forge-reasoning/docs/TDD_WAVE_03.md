# TDD Wave 3: Persistence & Durability

**Date**: 2026-02-19  
**Focus**: File-based storage, export/import, compaction, recovery

---

## Wave 3 Tests Summary

| Test | Description | Status |
|------|-------------|--------|
| 21 | Create file-based storage | ✅ |
| 22 | Checkpoints persist to disk | ✅ |
| 23 | Multi-session persistence | ✅ |
| 24 | Export checkpoints to JSON | ✅ |
| 25 | Import checkpoints from JSON | ✅ |
| 26 | Checkpoint compaction | ✅ |
| 27 | Compaction preserves tags | ✅ |
| 28 | Delete specific checkpoint | ✅ |
| 29 | Storage recovery | ✅ |
| 30 | Export/import roundtrip | ✅ |

**Total**: 10 new tests, all passing (30/30 cumulative)

---

## Implementation Details

### New Module: `export_import.rs`

Provides JSON serialization for backup and migration:

```rust
pub struct CheckpointExporter;
pub struct CheckpointImporter;

// Usage:
let exporter = CheckpointExporter::new(storage);
let json = exporter.export_session(&session_id)?;

let importer = CheckpointImporter::new(storage);
let count = importer.import_session(&json)?;
```

### New Type: `CompactionPolicy`

Configurable checkpoint retention:

```rust
pub enum CompactionPolicy {
    /// Keep N most recent checkpoints
    KeepRecent(usize),
    /// Keep all checkpoints with specific tags
    PreserveTagged(Vec<String>),
    /// Keep recent + preserve tagged
    Hybrid { keep_recent: usize, preserve_tags: Vec<String> },
}
```

### Storage Enhancements

**File-based persistence**:
```rust
// Open persistent storage
let storage = SqliteGraphStorage::open("/path/to/checkpoints.db")?;

// Open with recovery
let storage = SqliteGraphStorage::open_with_recovery("/path/to/checkpoints.db")?;
```

**Auto-load from disk**:
- On `open()`, all checkpoints are loaded from SQLite into the cache
- Enables immediate access without database queries
- Cache stays synchronized with disk

### Manager New Methods

```rust
/// Delete a checkpoint
pub fn delete(&self, id: &CheckpointId) -> Result<()>

/// Compact to N most recent
pub fn compact(&self, keep_recent: usize) -> Result<usize>

/// Compact with custom policy
pub fn compact_with_policy(&self, policy: CompactionPolicy) -> Result<usize>
```

---

## Key Design Decisions

### 1. Dual Storage Strategy

SQLite (disk) + HashMap (cache):
- **Durability**: SQLite provides persistent storage
- **Performance**: Cache enables O(1) lookups
- **Recovery**: Load from disk on open

### 2. Export Format

JSON-based with schema version:
```json
{
  "version": "1.0",
  "session_id": "...",
  "exported_at": "2026-02-19T...",
  "checkpoints": [...]
}
```

### 3. Compaction Policies

Flexible retention strategies:
- **KeepRecent**: Simple sliding window
- **PreserveTagged**: Keep important milestones
- **Hybrid**: Combine both approaches

### 4. Recovery Strategy

Best-effort recovery:
```rust
pub fn open_with_recovery(path) -> Result<Self> {
    match Self::open(path) {
        Ok(storage) => Ok(storage),
        Err(_) => {
            // Try to open anyway (may recover partial data)
            Self::open(path)
        }
    }
}
```

---

## Code Metrics

| File | Lines | Change |
|------|-------|--------|
| `src/checkpoint.rs` | ~380 | +60 |
| `src/storage_sqlitegraph.rs` | ~300 | +135 |
| `src/export_import.rs` | ~140 | New |
| `tests/checkpoint_tests.rs` | ~600 | +250 |

---

## Next Wave Preview

**Wave 4: Thread Safety**
- Replace `Rc<RefCell>` with `Arc<Mutex/RwLock>`
- Concurrent checkpoint operations
- Background persistence
- Multi-threaded test suite

**Potential Wave 5: Integration**
- Forge agent loop integration
- WebSocket/API interface
- Real-time checkpoint streaming

---

## Running the Tests

```bash
cd /home/feanor/Projects/forge/forge-reasoning
cargo test

# Just Wave 3 tests
cargo test test_create_file
cargo test test_checkpoint_persistence
cargo test test_export
cargo test test_compact
cargo test test_delete
cargo test test_recovery
```

---

**Wave 3 Complete** ✅

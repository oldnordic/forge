# TDD Wave 9: Global Sequence Numbers

**Status**: ✅ Complete  
**Date**: 2026-02-19  
**Focus**: Globally monotonic checkpoint sequences across all sessions

---

## Overview

Wave 9 addresses the limitation where sequence numbers were per-manager (resetting per session) by implementing a **global sequence counter** in CheckpointService. Now checkpoints across all sessions have monotonic, globally unique sequence numbers.

---

## Test Results

| Test | Description | Status |
|------|-------------|--------|
| 68 | Global sequence starts at 1 | ✅ |
| 69 | Sequences increment across sessions | ✅ |
| 70 | Global sequence persists across restarts | ✅ |
| 71 | Concurrent checkpoints get unique sequences | ✅ |
| 72 | Global sequence is monotonic under load | ✅ |
| 73 | Service exposes current global sequence | ✅ |
| 74 | Global sequence with multiple services | ✅ |
| 75 | Sequence survives compaction | ✅ |
| 76 | Query checkpoints by sequence range | ✅ |
| 77 | Export/import preserves global sequences | ✅ |

**Results**: 10 passed, 0 failed

---

## Implementation Details

### Architecture Change

**Before (Per-Manager Sequences):**
```
Session A: 1, 2, 3, ...
Session B: 1, 2, 3, ...  (Independent counters)
```

**After (Global Sequences):**
```
Session A: 1, 3, 5, ...
Session B: 2, 4, 6, ...  (Shared counter)
```

### Key Changes

#### 1. Global Sequence Counter (service.rs)

Added atomic counter to CheckpointService:

```rust
pub struct CheckpointService {
    // ... other fields ...
    global_sequence: AtomicU64,
}

impl CheckpointService {
    pub fn global_sequence(&self) -> u64 {
        self.global_sequence.load(Ordering::SeqCst)
    }
    
    fn next_sequence(&self) -> u64 {
        self.global_sequence.fetch_add(1, Ordering::SeqCst) + 1
    }
}
```

#### 2. Manager Methods Accept External Sequence

ThreadSafeCheckpointManager now accepts pre-determined sequence numbers:

```rust
impl ThreadSafeCheckpointManager {
    /// Create checkpoint with specific sequence (for global sequencing)
    pub fn checkpoint_with_sequence(
        &self,
        message: impl Into<String>,
        sequence: u64,
    ) -> Result<CheckpointId> {
        // ... creates checkpoint with provided sequence
    }
    
    /// Original method now delegates to _with_sequence
    pub fn checkpoint(&self, message: impl Into<String>) -> Result<CheckpointId> {
        let seq = self.increment_local_counter();
        self.checkpoint_with_sequence(message, seq)
    }
}
```

#### 3. Service Methods Use Global Counter

All checkpoint creation methods in CheckpointService now use `next_sequence()`:

```rust
impl CheckpointService {
    pub fn checkpoint(&self, session_id: &SessionId, message: impl Into<String>) -> Result<CheckpointId> {
        let manager = self.get_manager(*session_id);
        let seq = self.next_sequence();  // ← Global sequence
        let id = manager.checkpoint_with_sequence(message, seq)?;
        // ...
    }
}
```

#### 4. Storage Persistence

Storage layer provides `get_max_sequence()` to initialize the counter on startup:

```rust
impl CheckpointStorage {
    fn get_max_sequence(&self) -> Result<u64> {
        // Scan all checkpoints, return max sequence_number
        // Returns 0 if no checkpoints exist
    }
}
```

#### 5. New Query Capability

Added `list_by_sequence_range()` to query checkpoints by global sequence:

```rust
impl CheckpointService {
    pub fn list_by_sequence_range(
        &self, 
        start_seq: u64, 
        end_seq: u64
    ) -> Result<Vec<CheckpointSummary>> {
        // Returns all checkpoints with sequences in [start, end]
        // Across ALL sessions, sorted by sequence
    }
}
```

#### 6. Export/Import with Sequence Preservation

Export includes global sequence counter, import restores it:

```rust
impl CheckpointService {
    pub fn export_all_checkpoints(&self) -> Result<String> {
        let export = ExportData {
            checkpoints: all_checkpoints,
            global_sequence: self.global_sequence(),  // ← Include counter
            exported_at: Utc::now(),
        };
        serde_json::to_string_pretty(&export)
    }
    
    pub fn import_checkpoints(&self, export_data: &str) -> Result<ImportResult> {
        // ... import checkpoints ...
        // Restore global sequence counter
        self.global_sequence.store(max_sequence, Ordering::SeqCst);
    }
}
```

---

## API Additions

### CheckpointService

| Method | Description |
|--------|-------------|
| `global_sequence()` | Get current global sequence number |
| `list_by_sequence_range(start, end)` | Query checkpoints by sequence range |
| `export_all_checkpoints()` | Export all data with sequence counter |
| `import_checkpoints(data)` | Import and restore sequence counter |

### ThreadSafeCheckpointManager

| Method | Description |
|--------|-------------|
| `checkpoint_with_sequence(msg, seq)` | Create checkpoint with specific sequence |
| `checkpoint_with_tags_and_sequence(msg, tags, seq)` | Tagged checkpoint with sequence |
| `auto_checkpoint_with_sequence(trigger, seq)` | Auto-checkpoint with sequence |

---

## Thread Safety

The global sequence counter uses `AtomicU64` with `SeqCst` ordering:

- **Concurrent access**: Multiple threads can safely increment the counter
- **No duplicates**: `fetch_add` guarantees unique sequences
- **Monotonic**: Sequences always increase (no wraparound in practice)

```rust
// Thread-safe sequence generation
fn next_sequence(&self) -> u64 {
    self.global_sequence.fetch_add(1, Ordering::SeqCst) + 1
}
```

---

## Design Decisions

### 1. Service-Level vs Storage-Level Counter

**Chosen**: Service-level counter (each CheckpointService has its own)

**Rationale**: - Simpler implementation
- No need for distributed consensus
- Most use cases use a single shared service instance
- Multiple services on same storage are an edge case

**Trade-off**: If you create multiple CheckpointService instances sharing the same storage, each will have its own sequence counter starting from the storage's max sequence.

### 2. 1-Based vs 0-Based Sequences

**Chosen**: 1-based (first checkpoint is sequence 1)

**Rationale**: More user-friendly, avoids confusion with uninitialized (0) state.

### 3. Counter Persistence

**Chosen**: Scan storage on startup to find max sequence

**Rationale**: No separate metadata storage needed, works with existing SQLiteGraph backend.

**Future improvement**: Store max sequence in a metadata table for faster startup.

---

## Performance Characteristics

| Operation | Complexity | Notes |
|-----------|------------|-------|
| `next_sequence()` | O(1) | Atomic increment |
| `global_sequence()` | O(1) | Atomic load |
| `list_by_sequence_range()` | O(S × C) | S=sessions, C=checkpoints/session |
| `get_max_sequence()` | O(C) | Scans all checkpoints |

---

## Files Modified

| File | Changes |
|------|---------|
| `src/service.rs` | Added global_sequence counter, new methods |
| `src/thread_safe.rs` | Added *_with_sequence methods to manager |
| `src/storage.rs` | Added get_max_sequence to trait |
| `src/storage_sqlitegraph.rs` | Implemented get_max_sequence |
| `tests/global_sequence_tests.rs` | New test file (10 tests) |

---

## Migration Notes

Existing checkpoints with per-manager sequences remain valid. The global sequence counter starts from the maximum existing sequence, so new checkpoints continue the sequence without conflicts.

---

## Future Enhancements

1. **Distributed sequences**: Use Snowflake-style IDs for multi-node deployments
2. **Sequence gaps detection**: Alert if sequences are non-contiguous (indicating data loss)
3. **Time-series queries**: `list_by_time_range()` for temporal analysis
4. **Sequence metadata**: Store sequence allocation timestamp for debugging

---

**Wave 9 Complete** ✅

Global sequence numbers provide a unified timeline across all checkpoint sessions, enabling cross-session analysis and proper ordering of events in distributed debugging scenarios.

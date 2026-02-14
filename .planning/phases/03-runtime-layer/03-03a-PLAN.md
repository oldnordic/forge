---
phase: 03-test-infrastructure
plan: 03a
type: execute
wave: 2
depends_on: ["03-01", "03-02"]
files_modified:
  - forge_core/src/watcher.rs
  - forge_core/src/indexing.rs
autonomous: true

must_haves:
  truths:
    - "File watcher detects file system events (create, modify, delete)"
    - "Watcher debouncing prevents duplicate events for rapid changes"
    - "Incremental indexer tracks pending changes and flushes correctly"
    - "Indexer handles duplicate file paths correctly"
    - "Indexer statistics accurately reflect processed changes"
  artifacts:
    - path: "forge_core/src/watcher.rs"
      provides: "File watching with comprehensive test coverage"
      exports: ["test_watcher_create_event", "test_watcher_modify_event", "test_watcher_delete_event", "test_watcher_recursive_watching", "test_watcher_multiple_events", "test_watcher_debounce"]
      covered_by: "Task 1"
    - path: "forge_core/src/indexing.rs"
      provides: "Incremental indexing with comprehensive test coverage"
      exports: ["test_indexer_flush_multiple", "test_indexer_delete_handling", "test_indexer_clear", "test_indexer_duplicate_queue", "test_indexer_statistics", "test_indexer_concurrent_flush"]
      covered_by: "Task 2"
  key_links:
    - from: "forge_core/src/watcher.rs"
      to: "forge_core::indexing::IncrementalIndexer"
      via: "Watcher sends events to indexer"
      pattern: "tx.send|indexer.queue"

---

<objective>
Expand test coverage for file watching and incremental indexing with edge cases and concurrency scenarios.

This plan covers the file watching and indexing components:
1. Expand watcher.rs tests with file operations, debouncing, and recursive watching
2. Expand indexing.rs tests with flush scenarios, duplicate handling, and concurrent operations

Purpose: File watching and indexing are core to hot-reload functionality. These tests ensure reliable file change detection and proper indexing under load.

Output: 12 new tests across watcher and indexing modules
</objective>

<execution_context>
@/home/feanor/.claude/get-shit-done/workflows/execute-plan.md
@/home/feanor/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/ROADMAP.md
@.planning/STATE.md
@.planning/phases/03-runtime-layer/03-RESEARCH.md
@forge_core/src/watcher.rs
@forge_core/src/indexing.rs
@tests/common/mod.rs
</context>

<tasks>

<task type="auto">
  <name>Task 1: Expand watcher.rs Tests</name>
  <files>forge_core/src/watcher.rs</files>
  <action>
Add 6 new tests to forge_core/src/watcher.rs in #[cfg(test)] mod tests:

1. test_watcher_create_event - Verify WatchEvent::Created is sent when file is created:
   - Create temp directory
   - Create watcher with channel
   - Start watcher
   - Create test file with tokio::fs::write
   - Wait for event via rx.recv() with timeout
   - Verify event is Created with correct path

2. test_watcher_modify_event - Verify WatchEvent::Modified is sent:
   - Create test file
   - Modify file content
   - Verify Modified event received

3. test_watcher_delete_event - Verify WatchEvent::Deleted is sent:
   - Create then delete file
   - Verify Deleted event received

4. test_watcher_recursive_watching - Verify subdirectory events are detected:
   - Create temp/subdir structure
   - Start watcher on parent
   - Create file in subdir
   - Verify event is received

5. test_watcher_multiple_events - Verify multiple events in sequence:
   - Create, then modify, then delete file
   - Verify all three events received in order

6. test_watcher_debounce - Verify debouncing prevents duplicate events:
   - Create watcher with debounce_ms=50
   - Rapidly modify same file 3 times
   - Verify only 1 or 2 events received (not 3)
   - Verify final event represents last state

Use tokio::time::timeout for event waiting to avoid hanging tests.
  </action>
  <verify>
Run: cargo test -p forge_core watcher
Expected: 9 tests pass (3 existing + 6 new)
  </verify>
  <done>
forge_core/src/watcher.rs has 9 tests covering: basic creation (existing), channel (existing), event equality (existing), file creation, modification, deletion, recursive watching, multiple event sequences, and debouncing behavior.
  </done>
</task>

<task type="auto">
  <name>Task 2: Expand indexing.rs Tests</name>
  <files>forge_core/src/indexing.rs</files>
  <action>
Add 6 new tests to forge_core/src/indexing.rs in #[cfg(test)] mod tests:

1. test_indexer_flush_multiple - Verify flush processes multiple pending changes:
   - Create indexer
   - Queue 3 different Modified events
   - Call flush()
   - Verify IndexStats shows 3 processed
   - Verify pending is empty after flush

2. test_indexer_delete_handling - Verify deleted files are tracked:
   - Create indexer
   - Queue Deleted event for "removed.rs"
   - Flush and verify deleted_count in stats

3. test_indexer_clear - Verify clear() resets state:
   - Add some pending changes
   - Call clear()
   - Verify pending_changes() returns 0

4. test_indexer_duplicate_queue - Verify duplicate file paths are handled:
   - Queue same file twice
   - Verify only one entry in pending

5. test_indexer_statistics - Verify IndexStats accuracy:
   - Create known mix of Created/Modified/Deleted events
   - Flush and verify all counts match

6. test_indexer_concurrent_flush - Verify flush is thread-safe:
   - Create indexer
   - Spawn 5 tasks each queuing different events
   - Wait for all to complete
   - Flush and verify all events processed
   - Verify no race conditions or panics

Use test_forge() or temporary UnifiedGraphStore for testing.
Use tokio::spawn for concurrent tests.
  </action>
  <verify>
Run: cargo test -p forge_core indexing
Expected: 11 tests pass (5 existing + 6 new)
  </verify>
  <done>
forge_core/src/indexing.rs has 11 tests covering: creation (existing), queue (existing), flush (existing), stats (existing), clear (existing), multiple flush, delete handling, clear state, duplicate handling, statistics accuracy, and concurrent flush operations.
  </done>
</task>

</tasks>

<verification>
Overall phase checks:
- [ ] cargo test -p forge_core watcher passes all 9 tests
- [ ] cargo test -p forge_core indexing passes all 11 tests
- [ ] No race conditions in concurrent tests
- [ ] Debounce test verifies duplicate prevention
</verification>

<success_criteria>
Phase 03-03a complete when:
1. watcher.rs has 9 tests covering all event types and debouncing
2. indexing.rs has 11 tests covering flush, duplicates, and concurrency
3. All new tests pass with cargo test -p forge_core
4. No new clippy warnings
</success_criteria>

<output>
After completion, create `.planning/phases/03-runtime-layer/03-03a-SUMMARY.md` with:
- List of tests added to watcher.rs and indexing.rs
- Concurrency test results
- Any issues found with debouncing behavior
</output>

---
phase: 03-test-infrastructure
plan: 03b
type: execute
wave: 2
depends_on: ["03-01", "03-02"]
files_modified:
  - forge_core/src/cache.rs
  - forge_core/src/pool.rs
autonomous: true

must_haves:
  truths:
    - "Query cache handles TTL expiration correctly"
    - "Query cache implements LRU eviction policy"
    - "Query cache is thread-safe under concurrent access"
    - "Connection pool enforces max connections limit"
    - "Connection pool times out when no permits available"
    - "Dropped permits return to pool for reuse"
  artifacts:
    - path: "forge_core/src/cache.rs"
      provides: "Query cache with comprehensive test coverage"
      exports: ["test_cache_lru_touch", "test_cache_update_existing", "test_cache_concurrent_access", "test_cache_stress_eviction", "test_cache_zero_max_size", "test_cache_ttl_expiration"]
      covered_by: "Task 1"
    - path: "forge_core/src/pool.rs"
      provides: "Connection pool with comprehensive test coverage"
      exports: ["test_pool_concurrent_acquires", "test_pool_timeout_behavior", "test_pool_permit_drop_returns", "test_pool_stress", "test_pool_all_permits_acquired", "test_pool_available_count"]
      covered_by: "Task 2"
  key_links:
    - from: "forge_core/src/cache.rs"
      to: "forge_core::pool::ConnectionPool"
      via: "Cache may use pool for database operations"
      pattern: "pool.acquire|ConnectionPool"

---

<objective>
Expand test coverage for query cache and connection pool with edge cases, concurrency, and stress tests.

This plan covers the caching and pooling components:
1. Expand cache.rs tests with LRU behavior, TTL expiration, and concurrency
2. Expand pool.rs tests with timeout behavior, concurrent acquires, and stress testing

Purpose: Cache and pool are critical for performance. These tests ensure correct LRU behavior, proper resource management, and thread safety under load.

Output: 12 new tests across cache and pool modules
</objective>

<execution_context>
@/home/feanor/.claude/get-shit-done/workflows/execute-plan.md
@/home/feanor/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/ROADMAP.md
@.planning/STATE.md
@.planning/phases/03-runtime-layer/03-RESEARCH.md
@forge_core/src/cache.rs
@forge_core/src/pool.rs
@tests/common/mod.rs
</context>

<tasks>

<task type="auto">
  <name>Task 1: Expand cache.rs Tests</name>
  <files>forge_core/src/cache.rs</files>
  <action>
Add 6 new tests to forge_core/src/cache.rs in #[cfg(test)] mod tests:

1. test_cache_lru_touch - Verify accessed items move to end (LRU):
   - Create cache with size 3
   - Insert items 1, 2, 3
   - Access item 1 via get()
   - Insert item 4 (causes eviction)
   - Verify item 2 is evicted (oldest, not 1)

2. test_cache_update_existing - Verify updating existing key refreshes TTL:
   - Insert key with value A
   - Wait partial TTL (use mock_instant or short TTL)
   - Insert same key with value B
   - Immediately get key - should return B with fresh TTL

3. test_cache_concurrent_access - Verify cache is thread-safe:
   - Spawn 10 tasks concurrently inserting different keys
   - Wait for all to complete
   - Verify all 10 items are in cache

4. test_cache_stress_eviction - Verify FIFO eviction under stress:
   - Create cache with size 5
   - Insert 100 items sequentially
   - Verify only 5 items remain
   - Verify remaining are the last 5 inserted

5. test_cache_zero_max_size - Verify cache with size 0 rejects inserts:
   - Create QueryCache with max_size 0
   - Insert should not add entries
   - Verify len() always returns 0

6. test_cache_ttl_expiration - Verify items expire after TTL:
   - Create cache with ttl_ms=100
   - Insert key
   - Sleep 150ms
   - Verify get returns None (expired)
   - Verify len decreased

Use tokio::spawn for concurrent tests and tokio::time::sleep for timing.
  </action>
  <verify>
Run: cargo test -p forge_core cache
Expected: 12 tests pass (6 existing + 6 new)
  </verify>
  <done>
forge_core/src/cache.rs has 12 tests covering: insert/get (existing), miss (existing), expiration (existing), eviction (existing), invalidate (existing), clear (existing), LRU touch behavior, key update refreshes TTL, concurrent access, stress eviction, zero max size edge case, and TTL expiration.
  </done>
</task>

<task type="auto">
  <name>Task 2: Expand pool.rs Tests</name>
  <files>forge_core/src/pool.rs</files>
  <action>
Add 6 new tests to forge_core/src/pool.rs in #[cfg(test)] mod tests:

1. test_pool_concurrent_acquires - Verify multiple tasks can acquire:
   - Create pool with max 5
   - Spawn 10 tasks trying to acquire
   - Use tokio::sync::Barrier to coordinate
   - Verify only 5 acquire at once
   - Verify all 10 complete eventually

2. test_pool_timeout_behavior - Verify acquire times out correctly:
   - Create pool with max 1 and timeout=50ms
   - Acquire 1 permit
   - Try to acquire another
   - Verify timeout occurs after ~50ms
   - Verify error contains timeout information

3. test_pool_permit_drop_returns - Verify dropping permit returns to pool:
   - Acquire permit
   - Verify available decreases
   - Drop permit
   - Verify available increases back to original

4. test_pool_stress - Verify pool handles rapid acquire/release cycles:
   - Create pool with max 10
   - Run 100 acquire/release cycles in loop
   - Verify no deadlocks or panics
   - Verify final available equals max

5. test_pool_all_permits_acquired - Verify pool at capacity:
   - Acquire all permits up to max
   - Verify available_connections() is 0
   - Verify try_acquire returns None
   - Release one and verify try_acquire now works

6. test_pool_available_count - Verify available count is accurate:
   - Create pool with known max
   - Acquire varying number of permits
   - Verify available decreases correctly
   - Drop permits and verify available increases

Use tokio::spawn for concurrent tests, tokio::time::timeout for timeout tests.
  </action>
  <verify>
Run: cargo test -p forge_core pool
Expected: 9 tests pass (3 existing + 6 new)
  </verify>
  <done>
forge_core/src/pool.rs has 9 tests covering: creation (existing), acquire (existing), try_acquire (existing), db_path (existing), concurrent acquires, timeout behavior, permit drop/return, stress testing, capacity limits, and available count accuracy.
  </done>
</task>

</tasks>

<verification>
Overall phase checks:
- [ ] cargo test -p forge_core cache passes all 12 tests
- [ ] cargo test -p forge_core pool passes all 9 tests
- [ ] No race conditions in concurrent tests
- [ ] LRU test verifies correct eviction order
- [ ] Timeout test verifies proper error handling
</verification>

<success_criteria>
Phase 03-03b complete when:
1. cache.rs has 12 tests covering LRU, TTL, and concurrency
2. pool.rs has 9 tests covering timeout, concurrent acquires, and stress
3. All new tests pass with cargo test -p forge_core
4. No new clippy warnings
</success_criteria>

<output>
After completion, create `.planning/phases/03-runtime-layer/03-03b-SUMMARY.md` with:
- List of tests added to cache.rs and pool.rs
- Concurrency test results
- Any race conditions or issues found
</output>

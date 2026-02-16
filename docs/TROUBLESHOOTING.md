# Troubleshooting Guide for ForgeKit

Common issues and solutions for ForgeKit.

## Table of Contents

1. [Build Issues](#build-issues)
2. [Backend Issues](#backend-issues)
3. [Feature Flag Issues](#feature-flag-issues)
4. [Pub/Sub Issues](#pubsub-issues)
5. [Performance Issues](#performance-issues)
6. [Tool Integration Issues](#tool-integration-issues)
7. [Testing Issues](#testing-issues)

## Build Issues

### Error: "cannot find type `V3Backend` in this scope"

**Cause:** Missing `native-v3` feature flag

**Solution:**
```toml
[dependencies]
forge-core = { version = "0.2", features = ["native-v3"] }
```

Or run tests with the feature:
```bash
cargo test --features native-v3
```

### Error: "feature `magellan` not found"

**Cause:** Wrong feature flag name

**Solution:** Use backend-specific features:
```toml
# Wrong
features = ["magellan"]

# Correct
features = ["magellan-sqlite"]  # or magellan-v3
```

### Error: "multiple packages link to native library `sqlite3`"

**Cause:** Version conflict in SQLite dependencies

**Solution:** Update all dependencies:
```bash
cargo update
```

Or force a specific version in `Cargo.toml`:
```toml
[patch.crates-io]
libsqlite3-sys = { version = "0.28" }
```

## Backend Issues

### V3 Database Not Persisting

**Symptoms:** Data lost after process restart

**Diagnosis:**
```bash
# Check file exists
ls -la .forge/graph.v3

# Check file size (should be > 0)
ls -lh .forge/graph.v3

# Check magic header
xxd .forge/graph.v3 | head -1
# Expected: 5351 4c54 4746 (SQLTGF)
```

**Solutions:**

1. **Update sqlitegraph** (most common fix):
   ```toml
   [dependencies]
   forge-core = "0.2"  # Uses sqlitegraph 2.0.5+
   ```

2. **Force flush on drop**:
   ```rust
   {
       let forge = Forge::open_with_backend(path, BackendKind::NativeV3).await?;
       // ... operations ...
   } // forge dropped here, should flush
   
   // Explicit flush if needed
   std::thread::sleep(Duration::from_millis(100));
   ```

3. **Verify file permissions**:
   ```bash
   chmod 644 .forge/graph.v3
   ```

### SQLite Database Locked

**Symptoms:** "database is locked" error

**Causes:**
- Another process has the database open
- Previous process crashed without releasing lock

**Solutions:**

1. **Kill other processes**:
   ```bash
   lsof .forge/graph.db
   kill <pid>
   ```

2. **Remove lock files**:
   ```bash
   rm -f .forge/graph.db-journal
   rm -f .forge/graph.db-wal
   rm -f .forge/graph.db-shm
   ```

3. **Use WAL mode** (automatic in sqlitegraph 2.0.5+):
   ```rust
   // WAL mode is enabled by default
   ```

### Backend Connection Failed

**Symptoms:** `Forge::open()` returns error

**Diagnosis:**
```rust
match Forge::open("./project").await {
    Ok(forge) => println!("Success"),
    Err(e) => {
        eprintln!("Error: {:?}", e);
        eprintln!("Path exists: {}", Path::new("./project").exists());
    }
}
```

**Solutions:**

1. **Check path exists**:
   ```bash
   ls -la ./project
   mkdir -p ./project/src
   ```

2. **Check permissions**:
   ```bash
   ls -la ./project/.forge/
   chmod 755 ./project/.forge
   ```

3. **Try explicit backend**:
   ```rust
   let forge = Forge::open_with_backend(path, BackendKind::SQLite).await?;
   ```

## Feature Flag Issues

### Error: "unresolved import `magellan`"

**Cause:** Feature not enabled

**Solution:**
```toml
[dependencies]
forge-core = { version = "0.2", features = ["magellan-sqlite"] }
```

### Mixed Backend Confusion

**Problem:** Tool using wrong backend

**Example:**
```toml
# This won't work as expected:
[dependencies]
forge-core = { version = "0.2", features = ["native-v3", "magellan-sqlite"] }
# Magellan will use SQLite, but core uses V3
```

**Solution:** Use consistent backends:
```toml
# Option 1: All V3
forge-core = { version = "0.2", features = ["full-v3"] }

# Option 2: All SQLite
forge-core = { version = "0.2", features = ["full-sqlite"] }

# Option 3: Explicit mixed (if you really need it)
forge-core = { version = "0.2", default-features = false, 
               features = ["sqlite", "magellan-v3", "llmgrep-sqlite"] }
```

### Default Features Not Working

**Cause:** Disabled defaults incorrectly

**Solution:**
```toml
# Correct way to disable defaults
[dependencies]
forge-core = { version = "0.2", default-features = false, 
               features = ["sqlite", "magellan-sqlite"] }

# Wrong - this disables ALL features
forge-core = { version = "0.2", features = [] }
```

## Pub/Sub Issues

### Not Receiving Events

**Symptoms:** Subscriber never gets events

**Checklist:**

1. **Verify subscription**:
   ```rust
   let (id, rx) = forge.subscribe(SubscriptionFilter::all()).await?;
   println!("Subscribed: {}", id);  // Should print ID
   ```

2. **Check filter matching**:
   ```rust
   let filter = SubscriptionFilter::nodes_only();
   // This won't match edge events!
   ```

3. **Trigger an event**:
   ```rust
   // Indexing triggers events
   forge.graph().index().await?;
   
   // Or manually (if testing)
   ```

4. **Check timeout**:
   ```rust
   // Use sufficient timeout
   match rx.recv_timeout(Duration::from_secs(5)) {
       Ok(event) => println!("Got: {:?}", event),
       Err(_) => println!("Timeout - no events"),
   }
   ```

**Common Mistakes:**

```rust
// Wrong: Filter too restrictive
let filter = SubscriptionFilter::default();  // All false!

// Correct: Receive all events
let filter = SubscriptionFilter::all();

// Or specific events
let filter = SubscriptionFilter {
    node_changes: true,
    edge_changes: true,
    kv_changes: false,
    snapshot_commits: true,
};
```

### Events Stop After Unsubscribe

**Expected behavior** - not a bug!

```rust
let (id, rx) = forge.subscribe(SubscriptionFilter::all()).await?;

// ... receive some events ...

forge.unsubscribe(id).await?;  // Stops events

// rx will return Err(RecvTimeoutError::Disconnected)
```

### Multiple Subscribers Not Working

**Check:** Each subscriber needs unique ID

```rust
// This is correct - two independent subscribers
let (id1, rx1) = forge.subscribe(SubscriptionFilter::all()).await?;
let (id2, rx2) = forge.subscribe(SubscriptionFilter::all()).await?;

// Both will receive events
```

## Performance Issues

### Slow Queries

**Symptoms:** Graph operations taking >1 second

**Diagnosis:**
```rust
use std::time::Instant;

let start = Instant::now();
let results = forge.graph().find_symbol("main").await?;
println!("Query took: {:?}", start.elapsed());
```

**Solutions:**

1. **Use Native V3 backend**:
   ```rust
   let forge = Forge::open_with_backend(path, BackendKind::NativeV3).await?;
   ```

2. **Enable caching**:
   ```rust
   let forge = Forge::builder()
       .path(path)
       .cache_ttl(Duration::from_secs(60))
       .build()
       .await?;
   ```

3. **Use specific queries**:
   ```rust
   // Slow - broad search
   let all = forge.search().pattern(".*").await?;
   
   // Fast - specific search
   let specific = forge.search().by_kind(SymbolKind::Function).await?;
   ```

4. **Check database size**:
   ```bash
   ls -lh .forge/graph.*
   # If >100MB, consider V3 backend
   ```

### High Memory Usage

**Symptoms:** Process using too much RAM

**Solutions:**

1. **Use Native V3 backend** (lower memory):
   ```rust
   let forge = Forge::open_with_backend(path, BackendKind::NativeV3).await?;
   ```

2. **Limit cache size**:
   ```rust
   let forge = Forge::builder()
       .path(path)
       .cache_ttl(Duration::from_secs(10))  // Shorter TTL
       .build()
       .await?;
   ```

3. **Drop forge when not needed**:
   ```rust
   {
       let forge = Forge::open(path).await?;
       // ... do work ...
   } // forge dropped, memory freed
   ```

## Tool Integration Issues

### Magellan Not Found

**Symptoms:** "magellan feature not enabled" or similar

**Solution:**
```toml
[dependencies]
forge-core = { version = "0.2", features = ["magellan-sqlite"] }
```

Verify installation:
```bash
cargo tree -p magellan
```

### LLMGrep Returns Empty Results

**Symptoms:** Search returns no results

**Diagnosis:**
1. Database indexed?
2. Correct backend?
3. Pattern valid?

**Solutions:**

1. **Index the database**:
   ```rust
   forge.graph().index().await?;
   ```

2. **Check backend compatibility**:
   ```rust
   // LLMGrep works with both backends
   let forge = Forge::open_with_backend(path, BackendKind::NativeV3).await?;
   ```

3. **Validate pattern**:
   ```rust
   // Use valid regex
   let results = forge.search().pattern(r"fn\s+\w+").await?;
   ```

### Splice Edit Failed

**Symptoms:** Edit operation returns error

**Common Causes:**

1. **Symbol not found**:
   ```rust
   // Check symbol exists first
   let symbols = forge.graph().find_symbol("target").await?;
   if symbols.is_empty() {
       println!("Symbol not found!");
       return Ok(());
   }
   ```

2. **Invalid span**:
   ```rust
   // Verify location is valid
   println!("Location: {:?}", symbol.location);
   ```

3. **File permissions**:
   ```bash
   chmod 644 src/lib.rs
   ```

## Testing Issues

### Tests Hang

**Symptoms:** Test never completes

**Causes:**
- Deadlock in async code
- Infinite loop
- Blocked on I/O

**Solutions:**

1. **Add timeout**:
   ```rust
   use tokio::time::{timeout, Duration};
   
   let result = timeout(Duration::from_secs(5), async {
       // Test code
   }).await;
   
   assert!(result.is_ok(), "Test timed out");
   ```

2. **Check for blocking calls**:
   ```rust
   // Wrong - blocks async runtime
   std::thread::sleep(Duration::from_secs(1));
   
   // Correct - yields to runtime
   tokio::time::sleep(Duration::from_secs(1)).await;
   ```

### Tests Fail Intermittently

**Symptoms:** Tests pass/fail randomly

**Common Causes:**
- Race conditions
- Shared state between tests
- Timing issues

**Solutions:**

1. **Use unique temp directories**:
   ```rust
   let temp = tempfile::tempdir().unwrap();
   // Each test gets unique directory
   ```

2. **Run tests single-threaded**:
   ```bash
   cargo test -- --test-threads=1
   ```

3. **Reset state between tests**:
   ```rust
   #[tokio::test]
   async fn test_name() {
       setup().await;
       // ... test ...
       teardown().await;
   }
   ```

### Feature Flag Tests Fail

**Symptoms:** Tests pass without features, fail with features

**Solution:** Check feature combinations:

```bash
# Test each feature individually
cargo test --no-default-features --features sqlite
cargo test --no-default-features --features native-v3

# Test combinations
cargo test --features "sqlite,magellan-sqlite"
cargo test --features "native-v3,magellan-v3"
```

## Getting More Help

If none of these solutions work:

1. **Enable debug logging**:
   ```bash
   RUST_LOG=debug cargo run 2>&1 | tee debug.log
   ```

2. **Get a backtrace**:
   ```bash
   RUST_BACKTRACE=1 cargo run
   ```

3. **Check versions**:
   ```bash
   cargo tree -p forge-core
   cargo tree -p sqlitegraph
   ```

4. **File an issue** with:
   - Error message
   - Backtrace
   - Debug log
   - Minimal reproduction code
   - Backend type (SQLite/V3)
   - Feature flags used

## Quick Fixes

### Reset Everything

```bash
# Clean build artifacts
cargo clean

# Remove database
rm -rf .forge/

# Update dependencies
cargo update

# Rebuild
cargo build --all-features

# Run tests
cargo test --all-features
```

### Verify Installation

```bash
# Check versions
cargo --version
rustc --version

# Check features
cargo metadata --format-version 1 | \
  jq '.packages[] | select(.name == "forge_core") | .features'

# Test basic functionality
cargo test -p forge_core --test pubsub_integration_tests
```

---

For debugging techniques, see [DEBUGGING.md](DEBUGGING.md).
For testing strategies, see [TESTING.md](TESTING.md).
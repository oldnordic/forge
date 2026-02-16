# Debugging Guide for ForgeKit

Techniques and tools for debugging ForgeKit issues.

## Table of Contents

1. [Logging and Tracing](#logging-and-tracing)
2. [Debugging Tests](#debugging-tests)
3. [Backend Debugging](#backend-debugging)
4. [Pub/Sub Debugging](#pubsub-debugging)
5. [Performance Debugging](#performance-debugging)
6. [Database Inspection](#database-inspection)
7. [Common Debugging Scenarios](#common-debugging-scenarios)

## Logging and Tracing

### Enabling Tracing

ForgeKit uses the `tracing` crate for structured logging:

```rust
use tracing::{info, debug, error, warn};

async fn some_function() {
    info!("Starting operation");
    debug!(path = ?db_path, "Opening database");
    
    match operation() {
        Ok(result) => info!(result = ?result, "Success"),
        Err(e) => error!(error = ?e, "Operation failed"),
    }
}
```

### Log Levels

```bash
# Error only
RUST_LOG=error cargo run

# Warn and above
RUST_LOG=warn cargo run

# Info and above (default)
RUST_LOG=info cargo run

# Debug and above
RUST_LOG=debug cargo run

# Trace (very verbose)
RUST_LOG=trace cargo run

# Specific crate
RUST_LOG=forge_core=debug cargo run

# Multiple crates
RUST_LOG=forge_core=debug,forge_runtime=info cargo run
```

### Tracing Subscriber

```rust
use tracing_subscriber;

fn main() {
    // Initialize with environment filter
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_thread_ids(true)
        .with_thread_names(true)
        .init();
}
```

### JSON Logging

```rust
// For structured logging
tracing_subscriber::fmt()
    .json()
    .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
    .init();
```

## Debugging Tests

### Running Single Test

```bash
# Run specific test
cargo test test_name -- --exact

# Run with output
cargo test test_name -- --exact --nocapture

# Run matching pattern
cargo test pubsub -- --nocapture
```

### GDB Debugging

```bash
# Install rust-gdb
rustup component add rust-gdb

# Run test under GDB
rust-gdb --args cargo test test_name -- --exact

# In GDB:
(gdb) break forge_core::Forge::open
(gdb) run
(gdb) next
(gdb) print path
(gdb) continue
```

### LLDB Debugging

```bash
# Run test under LLDB
rust-lldb -- cargo test test_name -- --exact

# In LLDB:
(lldb) breakpoint set --name open
(lldb) run
(lldb) frame variable
(lldb) continue
```

### VS Code Debugging

`.vscode/launch.json`:

```json
{
    "version": "0.2.0",
    "configurations": [
        {
            "type": "lldb",
            "request": "launch",
            "name": "Debug unit tests",
            "cargo": {
                "args": ["test", "--no-run", "--lib"],
                "filter": {
                    "name": "forge_core",
                    "kind": "lib"
                }
            },
            "args": ["test_name", "--", "--exact"],
            "cwd": "${workspaceFolder}"
        }
    ]
}
```

## Backend Debugging

### SQLite Backend

#### Enable SQLite Logging

```rust
use rusqlite;

// Enable trace logging
conn.trace(Some(|sql| {
    eprintln!("SQL: {}", sql);
}));
```

#### Query Database Directly

```bash
# Open SQLite database
sqlite3 .forge/graph.db

# List tables
.tables

# Check schema
.schema nodes
.schema edges

# Query data
SELECT * FROM nodes LIMIT 10;
SELECT COUNT(*) FROM nodes;
```

#### Debug Connection Issues

```rust
use forge_core::{Forge, BackendKind};

#[tokio::main]
async fn main() {
    // Enable debug logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();
    
    match Forge::open_with_backend("./project", BackendKind::SQLite).await {
        Ok(forge) => {
            tracing::info!("Connected successfully");
            tracing::debug!(backend = ?forge.backend_kind(), "Backend info");
        }
        Err(e) => {
            tracing::error!(error = ?e, "Connection failed");
        }
    }
}
```

### Native V3 Backend

#### V3 Header Inspection

```bash
# View V3 file header
xxd .forge/graph.v3 | head -20

# Check magic bytes
cat .forge/graph.v3 | head -c 6
# Should print: SQLTGF
```

#### Enable V3 Debug Logging

```rust
use forge_core::{Forge, BackendKind};

let forge = Forge::open_with_backend("./project", BackendKind::NativeV3)
    .await
    .unwrap();

// Check if pub/sub is initialized
tracing::debug!(
    pubsub_initialized = forge.analysis().storage().is_pubsub_initialized(),
    "Pub/Sub status"
);
```

#### File Structure Debug

```rust
use std::fs;

fn debug_v3_structure(path: &Path) {
    let metadata = fs::metadata(path).unwrap();
    tracing::info!(
        path = ?path,
        size = metadata.len(),
        modified = ?metadata.modified(),
        "V3 file metadata"
    );
    
    // Check first few bytes
    let bytes = fs::read(path).unwrap();
    tracing::debug!(
        magic = ?&bytes[0..6],
        version = bytes[6],
        "V3 header"
    );
}
```

## Pub/Sub Debugging

### Event Tracing

```rust
use std::sync::mpsc::RecvTimeoutError;

async fn debug_pubsub(forge: &Forge) {
    let (id, rx) = forge.subscribe(SubscriptionFilter::all()).await.unwrap();
    
    println!("Subscribed with ID: {}", id);
    
    // Non-blocking receive with timeout
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(event) => println!("Received: {:?}", event),
            Err(RecvTimeoutError::Timeout) => {
                println!("No events...");
                break;
            }
            Err(RecvTimeoutError::Disconnected) => {
                println!("Channel disconnected");
                break;
            }
        }
    }
    
    forge.unsubscribe(id).await.unwrap();
}
```

### Subscriber Count Debug

```rust
fn check_subscribers(forge: &Forge) {
    let store = forge.analysis().storage();
    tracing::debug!(
        backend = ?store.backend_kind(),
        connected = store.is_connected(),
        "Store status"
    );
}
```

### Event Filter Testing

```rust
fn test_filters() {
    let filter = SubscriptionFilter::nodes_only();
    
    let node_event = PubSubEvent::NodeChanged { node_id: 1, snapshot_id: 1 };
    let edge_event = PubSubEvent::EdgeChanged { 
        edge_id: 1, from_node: 1, to_node: 2, snapshot_id: 1 
    };
    
    assert!(filter.matches(&node_event));
    assert!(!filter.matches(&edge_event));
    
    println!("Filter working correctly");
}
```

## Performance Debugging

### Timing Operations

```rust
use std::time::Instant;

async fn profile_operation(forge: &Forge) {
    let start = Instant::now();
    
    let result = forge.graph().find_symbol("main").await;
    
    let duration = start.elapsed();
    tracing::info!(
        duration_ms = duration.as_millis(),
        symbol_count = result.as_ref().map(|r| r.len()).unwrap_or(0),
        "Query performance"
    );
}
```

### Memory Profiling

```bash
# Install cargo-flamegraph
cargo install flamegraph

# Generate flamegraph
cargo flamegraph --test test_name

# View in browser
open flamegraph.svg
```

### Async Task Debugging

```rust
use tokio::task;

async fn debug_async_tasks() {
    let handle1 = task::spawn(async {
        // Operation 1
    });
    
    let handle2 = task::spawn(async {
        // Operation 2
    });
    
    let (r1, r2) = tokio::join!(handle1, handle2);
    
    match (r1, r2) {
        (Ok(v1), Ok(v2)) => println!("Both succeeded: {:?}, {:?}", v1, v2),
        (Err(e), _) => println!("Task 1 panicked: {:?}", e),
        (_, Err(e)) => println!("Task 2 panicked: {:?}", e),
    }
}
```

## Database Inspection

### SQLite Inspection

```bash
# Database info
sqlite3 .forge/graph.db "PRAGMA info;"

# Table sizes
sqlite3 .forge/graph.db "SELECT name, COUNT(*) FROM nodes;"

# Index info
sqlite3 .forge/graph.db ".indexes"

# Query plan
sqlite3 .forge/graph.db "EXPLAIN QUERY PLAN SELECT * FROM nodes WHERE name = 'main';"
```

### V3 Inspection

```rust
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

fn inspect_v3_file(path: &Path) -> std::io::Result<()> {
    let mut file = File::open(path)?;
    
    // Read header
    let mut header = [0u8; 64];
    file.read_exact(&mut header)?;
    
    println!("Magic: {:?}", &header[0..6]);
    println!("Version: {}", header[6]);
    
    // Read more sections...
    
    Ok(())
}
```

## Common Debugging Scenarios

### Scenario 1: Backend Not Connecting

**Symptoms:** `Forge::open()` returns error

**Debug Steps:**
1. Check path exists:
   ```bash
   ls -la ./project/.forge/
   ```
2. Check permissions:
   ```bash
   ls -la ./project/.forge/graph.db
   ```
3. Enable debug logging:
   ```bash
   RUST_LOG=debug cargo run
   ```
4. Try explicit backend:
   ```rust
   Forge::open_with_backend(path, BackendKind::SQLite).await
   ```

### Scenario 2: V3 Database Not Persisting

**Symptoms:** Data lost after process restart

**Debug Steps:**
1. Check file exists after first run:
   ```bash
   ls -la .forge/graph.v3
   ```
2. Check file size (should be > 0):
   ```bash
   ls -lh .forge/graph.v3
   ```
3. Verify magic header:
   ```bash
   xxd .forge/graph.v3 | head -1
   # Should show: 5351 4c54 4746 (SQLTGF)
   ```
4. Check sqlitegraph version:
   ```bash
   cargo tree -p sqlitegraph
   # Should be 2.0.5+
   ```

### Scenario 3: Pub/Sub Not Receiving Events

**Symptoms:** Subscriber never receives events

**Debug Steps:**
1. Check subscription:
   ```rust
   let (id, rx) = forge.subscribe(SubscriptionFilter::all()).await?;
   println!("Subscribed: {}", id);
   ```
2. Trigger event manually:
   ```rust
   forge.graph().index().await?; // Should trigger events
   ```
3. Check with timeout:
   ```rust
   match rx.recv_timeout(Duration::from_secs(5)) {
       Ok(event) => println!("Got: {:?}", event),
       Err(e) => println!("Timeout: {:?}", e),
   }
   ```
4. Verify filter matching:
   ```rust
   let filter = SubscriptionFilter::all();
   let test_event = PubSubEvent::SnapshotCommitted { snapshot_id: 1 };
   assert!(filter.matches(&test_event));
   ```

### Scenario 4: Slow Queries

**Symptoms:** Graph queries taking too long

**Debug Steps:**
1. Profile the query:
   ```rust
   let start = Instant::now();
   let result = forge.graph().find_symbol("main").await;
   println!("Took: {:?}", start.elapsed());
   ```
2. Try different backend:
   ```rust
   // Compare SQLite vs V3
   let forge_v3 = Forge::open_with_backend(path, BackendKind::NativeV3).await?;
   ```
3. Check database size:
   ```bash
   ls -lh .forge/graph.*
   ```
4. Enable backend tracing:
   ```bash
   RUST_LOG=forge_core=trace,sqlitegraph=debug cargo run
   ```

### Scenario 5: Feature Flag Issues

**Symptoms:** Compilation errors with features

**Debug Steps:**
1. Check available features:
   ```bash
   cargo metadata --format-version 1 | jq '.packages[] | select(.name == "forge_core") | .features'
   ```
2. Test minimal features:
   ```bash
   cargo check -p forge_core --no-default-features --features sqlite
   ```
3. Build with specific features:
   ```bash
   cargo build -p forge_core --features "magellan-v3,llmgrep-sqlite"
   ```

## Debugging Tools

### Recommended Tools

1. **tracing** - Structured logging
2. **tokio-console** - Async runtime debugging
3. **cargo-flamegraph** - Performance profiling
4. **rust-gdb/rust-lldb** - Native debugging
5. **sqlite3 CLI** - Database inspection

### Installing Tools

```bash
# tokio-console
cargo install tokio-console

# flamegraph
cargo install flamegraph

# cargo-expand (for macro debugging)
cargo install cargo-expand

# cargo-tree (for dependency debugging)
cargo install cargo-tree
```

## Getting Help

If you're stuck:

1. Check [TROUBLESHOOTING.md](TROUBLESHOOTING.md)
2. Review logs with `RUST_LOG=debug`
3. File an issue with debug output
4. Include:
   - Error message
   - Backtrace (`RUST_BACKTRACE=1`)
   - Minimal reproduction
   - Backend type (SQLite/V3)

---

For testing strategies, see [TESTING.md](TESTING.md).
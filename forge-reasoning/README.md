# forge-reasoning

Temporal Checkpointing for Forge Agent Reasoning Tools

## Overview

This crate implements the **Temporal Checkpointing** reasoning tool from the Forge Reasoning Tools suite. It provides:

- **Automatic checkpointing** - Throttled state capture during long operations
- **Manual checkpointing** - Explicit state snapshots via user request
- **State restoration** - Roll back to previous checkpoint
- **Session management** - Group checkpoints by session
- **Tag-based filtering** - Organize checkpoints with tags
- **WebSocket API** - Real-time remote access to checkpoints
- **Global sequences** - Monotonic ordering across all sessions
- **Data integrity** - SHA-256 checksums for corruption detection

## Quick Start

```rust
use forge_reasoning::*;
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create storage (in-memory for testing)
    let storage = ThreadSafeStorage::in_memory()?;
    
    // Create checkpoint service
    let service = Arc::new(CheckpointService::new(storage));
    
    // Create a debugging session
    let session = service.create_session("debug-session")?;
    
    // Capture a checkpoint
    let checkpoint_id = service.checkpoint(&session, "Before fix attempt")?;
    
    // List checkpoints for this session
    let checkpoints = service.list_checkpoints(&session)?;
    println!("Created {} checkpoints", checkpoints.len());
    
    // Verify data integrity
    let is_valid = service.validate_checkpoint(&checkpoint_id)?;
    println!("Checkpoint valid: {}", is_valid);
    
    Ok(())
}
```

## Features

### Core Checkpointing
- Create, query, restore checkpoints
- Automatic and manual checkpoint triggers
- Session-based organization
- Tag-based filtering

### Persistence
- SQLiteGraph backend for durability
- In-memory backend for testing
- Export/import to JSON

### Concurrency
- Thread-safe operations
- Concurrent session support
- Atomic sequence numbers

### Real-Time API
- WebSocket server for remote access
- Event broadcasting to subscribers
- JSON-RPC protocol

### Data Integrity
- SHA-256 checksums on all checkpoints
- Validation on restore
- Health checks with integrity verification

## Testing

All tests use TDD (Test-Driven Development):

```bash
# Run all tests (110 tests)
cargo test

# Run specific test file
cargo test --test checkpoint_tests
cargo test --test e2e_tests

# Run with output
cargo test -- --nocapture

# Run benchmarks
cargo bench
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   CheckpointService                     │
│  ┌─────────────┐    ┌─────────────┐    ┌────────────┐  │
│  │   Storage   │    │   Global    │    │   Event    │  │
│  │   (Trait)   │    │  Sequence   │    │  Broadcast │  │
│  └─────────────┘    └─────────────┘    └────────────┘  │
└─────────────────────────────────────────────────────────┘
                    │
        ┌───────────┼───────────┐
        ▼           ▼           ▼
┌───────────────┐ ┌───────────┐ ┌────────────────┐
│  InMemory     │ │  SQLite   │ │   WebSocket    │
│  Storage      │ │  Storage  │ │   Server       │
└───────────────┘ └───────────┘ └────────────────┘
```

## Backends

- **ThreadSafeStorage** - Thread-safe wrapper around any storage
- **SqliteGraphStorage** - SQLiteGraph-based, persistent

## API Examples

### Session Management
```rust
let service = Arc::new(CheckpointService::new(storage));
let session = service.create_session("my-debug-session")?;

// Enable auto-checkpointing
let config = AutoCheckpointConfig::default();
service.enable_auto_checkpoint(&session, config)?;
```

### Checkpoint Operations
```rust
// Create checkpoint
let id = service.checkpoint(&session, "Description")?;

// With tags
let id = service.execute(CheckpointCommand::Create {
    session_id: session,
    message: "Tagged checkpoint".to_string(),
    tags: vec!["baseline".to_string()],
})?;

// Restore
let state = service.restore(&session, &checkpoint_id)?;
```

### Data Integrity
```rust
// Validate single checkpoint
let valid = service.validate_checkpoint(&id)?;

// Validate all checkpoints
let report = service.validate_all_checkpoints()?;
println!("Valid: {}, Invalid: {}", report.valid, report.invalid);

// Health check with validation
let health = service.health_check_with_validation()?;
```

### Export/Import
```rust
// Export all checkpoints
let json = service.export_all_checkpoints()?;

// Import to another service
let result = new_service.import_checkpoints(&json)?;
println!("Imported {} checkpoints", result.imported);
```

## Design Docs

See `docs/` for full documentation:
- `TDD_WAVE_01.md` through `TDD_WAVE_10.md` - Implementation waves
- `PROJECT_LOG.md` - Complete project history
- `07_TEMPORAL_CHECKPOINTING.md` - Design specification

## Status

✅ **Production Ready**
- 110 tests passing (10 waves + E2E)
- 11 Criterion benchmarks
- ~6,350 lines of production code

## License

MIT - See Forge project license

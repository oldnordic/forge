# ForgeKit Developer Documentation

Complete developer guide for contributing to ForgeKit.

## Overview

ForgeKit is a code intelligence SDK built in Rust, unifying multiple tools (magellan, llmgrep, mirage, splice) under a single API with dual backend support (SQLite and Native V3).

## Documentation Index

### Getting Started

| Document | Purpose |
|----------|---------|
| [CONTRIBUTING.md](CONTRIBUTING.md) | How to contribute to ForgeKit |
| [DEVELOPMENT_WORKFLOW.md](DEVELOPMENT_WORKFLOW.md) | Mandatory workflow for all changes |
| [PHILOSOPHY.md](PHILOSOPHY.md) | Design principles and philosophy |

### Core Documentation

| Document | Purpose |
|----------|---------|
| [ARCHITECTURE.md](ARCHITECTURE.md) | System design and architecture |
| [API.md](API.md) | Complete API reference |
| [MANUAL.md](MANUAL.md) | User guide and tutorials |

### Developer Guides

| Document | Purpose |
|----------|---------|
| [TESTING.md](TESTING.md) | Testing strategies and examples |
| [DEBUGGING.md](DEBUGGING.md) | Debugging techniques and tools |
| [TROUBLESHOOTING.md](TROUBLESHOOTING.md) | Common issues and solutions |

### Project Management

| Document | Purpose |
|----------|---------|
| [ROADMAP.md](ROADMAP.md) | Project roadmap and milestones |
| [CHANGELOG.md](../CHANGELOG.md) | Version history |

## Quick Reference

### Building

```bash
# Build all crates
cargo build --workspace

# Build with all features
cargo build --workspace --all-features

# Build specific crate
cargo build -p forge_core
```

### Testing

```bash
# Run all tests
cargo test --workspace

# Run with all features
cargo test --workspace --all-features

# Run specific test
cargo test test_name

# Run with output
cargo test -- --nocapture
```

### Feature Flags

```bash
# Test SQLite backend
cargo test --features "sqlite,tools-sqlite"

# Test Native V3 backend
cargo test --features "native-v3,tools-v3"

# Test full stack
cargo test --features "full-v3"
```

### Code Quality

```bash
# Format code
cargo fmt

# Run clippy
cargo clippy --all-targets --all-features -- -D warnings

# Check all features
cargo check --all-features
```

## Project Structure

```
forge/
├── Cargo.toml              # Workspace definition
├── README.md               # Project overview
├── CHANGELOG.md            # Version history
├── AGENTS.md               # AI agent instructions
├── CLAUDE.md               # Claude-specific instructions
├── docs/
│   ├── ARCHITECTURE.md     # System design
│   ├── API.md              # API reference
│   ├── MANUAL.md           # User guide
│   ├── TESTING.md          # Testing guide
│   ├── DEBUGGING.md        # Debugging guide
│   ├── TROUBLESHOOTING.md  # Issue resolution
│   ├── CONTRIBUTING.md     # Contribution guide
│   ├── DEVELOPMENT_WORKFLOW.md  # Workflow
│   ├── PHILOSOPHY.md       # Design principles
│   └── ROADMAP.md          # Future plans
├── forge_core/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs          # Main SDK
│   │   ├── types.rs        # Core types
│   │   ├── error.rs        # Error types
│   │   ├── storage/        # Storage abstraction
│   │   ├── graph/          # Graph module
│   │   ├── search/         # Search module
│   │   ├── cfg/            # CFG module
│   │   ├── edit/           # Edit module
│   │   └── analysis/       # Analysis module
│   └── tests/              # Integration tests
├── forge_runtime/
│   └── src/lib.rs          # Runtime layer
└── forge_agent/
    └── src/                # Agent layer
```

## Development Workflow

1. **Understand** - Read source, check docs
2. **Plan** - Document architectural decisions
3. **Prove** - Write failing test first
4. **Implement** - Write code to pass test
5. **Verify** - Run tests, check quality

See [DEVELOPMENT_WORKFLOW.md](DEVELOPMENT_WORKFLOW.md) for details.

## Key Concepts

### Backend System

ForgeKit supports two storage backends:

- **SQLite** - Stable, mature, SQL access
- **Native V3** - High performance (10-20x faster), pure Rust

Each tool can use either backend via feature flags:
```toml
[features]
magellan-sqlite = ["dep:magellan"]
magellan-v3 = ["dep:magellan"]
```

### Pub/Sub System

Real-time event notifications:

```rust
let (id, rx) = forge.subscribe(SubscriptionFilter::all()).await?;
// Receive NodeChanged, EdgeChanged, KVChanged, SnapshotCommitted
```

### Module System

- **GraphModule** - Symbol and reference queries
- **SearchModule** - Semantic code search
- **CfgModule** - Control flow analysis
- **EditModule** - Safe code editing
- **AnalysisModule** - Composite operations

## Testing Strategy

### Test Categories

1. **Unit Tests** - In `#[cfg(test)]` modules
2. **Integration Tests** - In `tests/` directories
3. **Doc Tests** - In documentation comments

### Backend Testing

Always test both backends:

```rust
#[tokio::test]
async fn test_sqlite_backend() {
    test_backend(BackendKind::SQLite).await;
}

#[tokio::test]
async fn test_native_v3_backend() {
    test_backend(BackendKind::NativeV3).await;
}
```

### Feature Testing

Test feature combinations:

```bash
cargo test --features "sqlite,magellan-sqlite"
cargo test --features "native-v3,magellan-v3"
```

See [TESTING.md](TESTING.md) for comprehensive testing guide.

## Debugging

### Enable Logging

```bash
RUST_LOG=debug cargo run
RUST_LOG=forge_core=trace cargo test
```

### GDB/LLDB

```bash
rust-gdb --args cargo test test_name -- --exact
rust-lldb -- cargo test test_name -- --exact
```

See [DEBUGGING.md](DEBUGGING.md) for debugging techniques.

## Common Tasks

### Adding a New Feature

1. Document in ARCHITECTURE.md
2. Add tests in tests/
3. Implement in src/
4. Update API.md
5. Add to CHANGELOG.md

### Adding a New Backend

1. Implement BackendKind variant
2. Add feature flags
3. Implement UnifiedGraphStore methods
4. Test both backends
5. Document in ARCHITECTURE.md

### Adding a New Tool Integration

1. Add dependency to Cargo.toml
2. Create module (graph/, search/, etc.)
3. Add feature flags (tool-sqlite, tool-v3)
4. Implement integration
5. Add tests
6. Update API.md

## Code Style

### Rust Conventions

- Use `cargo fmt` for formatting
- Follow `cargo clippy` lints
- Document all public APIs
- Use `anyhow` for errors
- Use `tracing` for logging

### Naming Conventions

- `PascalCase` for types and traits
- `snake_case` for functions and variables
- `SCREAMING_SNAKE_CASE` for constants
- `BackendKind` for backend enum
- `test_<feature>_<scenario>` for tests

### Documentation

- All public items must have doc comments
- Include examples in doc comments
- Link to related items
- Document panics and errors

## Dependencies

### Core Dependencies

- `tokio` - Async runtime
- `anyhow` - Error handling
- `serde` - Serialization
- `tracing` - Logging
- `sqlitegraph` - Graph database

### Optional Dependencies

- `magellan` - Code indexing
- `llmgrep` - Semantic search
- `mirage-analyzer` - CFG analysis
- `splice` - Code editing

### Version Requirements

- Rust 1.75+
- sqlitegraph 2.0.5+
- magellan 2.4.5+ (if used)
- llmgrep 3.0.8+ (if used)

## Release Process

1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Run full test suite
4. Create git tag
5. Push to crates.io
6. Create GitHub release

## Communication

- Issues: GitHub Issues
- Discussions: GitHub Discussions
- PRs: GitHub Pull Requests

## Resources

### External Documentation

- [Rust Book](https://doc.rust-lang.org/book/)
- [Tokio Documentation](https://tokio.rs/)
- [SQLite Documentation](https://sqlite.org/docs.html)

### Related Projects

- [sqlitegraph](https://crates.io/crates/sqlitegraph)
- [magellan](https://github.com/oldnordic/magellan)
- [llmgrep](https://github.com/oldnordic/llmgrep)

## Getting Help

1. Check [TROUBLESHOOTING.md](TROUBLESHOOTING.md)
2. Review [DEBUGGING.md](DEBUGGING.md)
3. File an issue with:
   - Error message
   - Backtrace (`RUST_BACKTRACE=1`)
   - Debug output (`RUST_LOG=debug`)
   - Minimal reproduction

## License

This project is licensed under the GPL-3.0 License.

---

Welcome to ForgeKit development! Start with [CONTRIBUTING.md](CONTRIBUTING.md) and [DEVELOPMENT_WORKFLOW.md](DEVELOPMENT_WORKFLOW.md).
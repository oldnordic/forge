# Roadmap

**Version**: 0.1.0 (Design Phase)
**Last Updated**: 2025-12-30

---

## Vision

ForgeKit aims to be "LLVM for AI Code Agents" - a deterministic, graph-backed reasoning SDK for building AI-native developer tools.

---

## Milestones

### Milestone 0.1: Foundation (Current)

**Goal**: Basic SDK with unified API

#### Status: 🚧 Design

| Component | Status | Priority |
|------------|----------|-----------|
| Workspace structure | ✅ Done | P0 |
| Core documentation | ✅ Done | P0 |
| API design | 🚧 In Progress | P0 |
| forge_core stubs | 📋 Planned | P0 |
| forge_runtime stubs | 📋 Planned | P1 |
| forge_agent stubs | 📋 Planned | P2 |

#### Deliverables

- [x] README.md with project overview
- [x] ARCHITECTURE.md with system design
- [x] API.md with interface reference
- [x] PHILOSOPHY.md with design principles
- [x] DEVELOPMENT_WORKFLOW.md with process
- [x] CONTRIBUTING.md with guidelines
- [ ] Workspace Cargo.toml
- [ ] Basic forge_core structure
- [ ] Placeholder modules for graph/search/cfg/edit

---

### Milestone 0.2: Core SDK

**Goal**: Working forge_core with Magellan integration

#### Status: 📋 Planned

| Component | Status | Priority |
|------------|----------|-----------|
| Graph module | 📋 Planned | P0 |
| Search module | 📋 Planned | P0 |
| CFG module | 📋 Planned | P1 |
| Edit module | 📋 Planned | P0 |
| Storage abstraction | 📋 Planned | P0 |
| Error types | 📋 Planned | P0 |

#### Deliverables

- [ ] `Forge::open("./repo")` API working
- [ ] Graph queries via Magellan
- [ ] Semantic search via LLMGrep
- [ ] CFG queries via Mirage
- [ ] Span-safe edits via Splice
- [ ] Integration tests for all modules
- [ ] Basic examples in README

---

### Milestone 0.3: Runtime Layer

**Goal**: Indexing and caching with forge_runtime

#### Status: 📋 Planned

| Component | Status | Priority |
|------------|----------|-----------|
| Watcher integration | 📋 Planned | P0 |
| Incremental indexing | 📋 Planned | P0 |
| Query caching | 📋 Planned | P1 |
| Performance metrics | 📋 Planned | P2 |

#### Deliverables

- [ ] File watcher for codebase
- [ ] Automatic reindex on change
- [ ] Cache for symbol queries
- [ ] Cache for CFG paths
- [ ] Metrics collection
- [ ] Performance dashboard (CLI)

---

### Milestone 0.4: Agent Layer

**Goal**: Deterministic AI loop with forge_agent

#### Status: 📋 Planned

| Component | Status | Priority |
|------------|----------|-----------|
| Policy DSL | 📋 Planned | P0 |
| Agent loop | 📋 Planned | P0 |
| Verification hooks | 📋 Planned | P0 |
| Transaction management | 📋 Planned | P1 |

#### Deliverables

- [ ] `Agent::observe()` - Gather context
- [ ] `Agent::constrain()` - Apply policy
- [ ] `Agent::plan()` - Generate steps
- [ ] `Agent::mutate()` - Apply changes
- [ ] `Agent::verify()` - Validate result
- [ ] `Agent::commit()` - Finalize
- [ ] Built-in policies (NoUnsafe, PreserveTests, etc.)
- [ ] Policy composition

---

### Milestone 0.5: Native V3 Backend

**Goal**: Native binary file backend support

#### Status: 📋 Planned (Depends on sqlitegraph)

| Component | Status | Priority |
|------------|----------|-----------|
| Backend selection | 📋 Planned | P0 |
| Native backend integration | 📋 Planned | P0 |
| Migration tools | 📋 Planned | P1 |
| Performance comparison | 📋 Planned | P2 |

#### Deliverables

- [ ] Runtime backend selection
- [ ] Native V3 backend support
- [ ] SQLite → Native migration
- [ ] Native → SQLite migration
- [ ] Benchmark comparison
- [ ] Documentation for backend choice

---

### Milestone 1.0: Production Release

**Goal**: Stable, production-ready SDK

#### Status: 📋 Future

| Component | Status | Priority |
|------------|----------|-----------|
| Stability guarantees | 📋 Planned | P0 |
| Performance targets | 📋 Planned | P0 |
| Documentation completeness | 📋 Planned | P0 |
| Example applications | 📋 Planned | P1 |

#### Deliverables

- [ ] API stability commitment
- [ ] Performance benchmarks
- [ ] Complete documentation
- [ ] Example IDE integration
- [ ] Example CLI tool
- [ ] Example agent
- [ ] Release notes
- [ ] Migration guide (from 0.x)

---

## Language Support

### Current (via Magellan)

| Language | AST | CFG | References |
|----------|------|-----|------------|
| Rust | ✅ | ✅ | ✅ |
| Python | ✅ | ✅ | ✅ |
| C | ✅ | ✅ | ✅ |
| C++ | ✅ | ✅ | ✅ |
| Java | ✅ | 📋 | ✅ |
| JavaScript | ✅ | 📋 | ✅ |
| TypeScript | ✅ | 📋 | ✅ |

### Planned

| Language | Status | Priority |
|----------|----------|-----------|
| Go | 📋 Planned | P1 |
| C# | 📋 Planned | P2 |
| Ruby | 📋 Planned | P3 |

---

## Performance Targets

### Query Latency

| Operation | Target | Current |
|-----------|---------|---------|
| Symbol lookup | <10ms | TBD |
| Reference query | <50ms | TBD |
| CFG enumeration | <100ms | TBD |
| File listing | <20ms | TBD |

### Indexing Throughput

| Metric | Target | Current |
|--------|---------|---------|
| Files/sec | >100 | TBD |
| MB/sec | >50 | TBD |
| Incremental reindex | <1s | TBD |

### Cache Hit Rate

| Cache Type | Target | Current |
|------------|---------|---------|
| Symbol queries | >90% | TBD |
| CFG paths | >80% | TBD |
| Search results | >70% | TBD |

---

## Experimental Features

### Future Exploration

These are being explored but not committed:

- **GPU-accelerated graph algorithms**: For very large codebases
- **Distributed indexing**: For monorepos at scale
- **LLM integration**: Direct embedding generation
- **Language Server Protocol**: Native LSP server
- **WebAssembly**: Browser-based code analysis

---

## Dependency Tracking

### External Projects

| Project | Role | Status |
|----------|---------|---------|
| sqlitegraph | Graph backend | ✅ Stable |
| Magellan | Code indexing | ✅ v2.2.1 |
| LLMGrep | Semantic search | ✅ Stable |
| Mirage | CFG analysis | ✅ Stable |
| Splice | Span-safe edits | ✅ v2.5.0 |
| tree-sitter | Parsing | ✅ Stable |

### Internal Dependencies

| Dependency | Version | Management |
|------------|---------|------------|
| tokio | Latest | Minor version bumps |
| anyhow | Latest | Minor version bumps |
| serde | Latest | Minor version bumps |
| sqlitegraph | ^1.5 | Track upstream |

---

## Timeline Estimate

| Milestone | Target Q | Target Year |
|-----------|-----------|--------------|
| 0.1 Foundation | Q1 | 2026 |
| 0.2 Core SDK | Q2 | 2026 |
| 0.3 Runtime | Q3 | 2026 |
| 0.4 Agent | Q4 | 2026 |
| 0.5 Native | Q1 | 2027 |
| 1.0 Release | Q2 | 2027 |

*Note: Timeline is estimates and subject to change.*

---

## How to Contribute

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Want to help?

- Pick an issue from [GitHub Issues](https://github.com/oldnordic/forge/issues)
- Comment to claim it
- Follow development workflow
- Submit PR

### Areas Needing Help

- Language grammars for tree-sitter
- Performance benchmarking
- Documentation examples
- Integration tests
- Example applications

---

*Last updated: 2025-12-30*

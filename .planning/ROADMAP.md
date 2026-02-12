# ForgeKit Roadmap

**Project**: ForgeKit
**Version**: 0.1.0
**Status**: Active Development
**Last Updated**: 2026-02-12

---

## Roadmap Overview

This roadmap defines the phased development plan for ForgeKit from foundation (v0.1) through stable release (v1.0).

### Milestone Summary

| Milestone | Focus | Duration | Status |
|-----------|--------|-----------|---------|
| v0.1 Foundation | Project scaffolding and API design | 1 week | In Progress |
| v0.2 Core SDK | SQLiteGraph integration and tool bindings | 4 weeks | Planned |
| v0.3 Runtime | Indexing, caching, file watching | 3 weeks | Planned |
| v0.4 Agent | Deterministic AI loop and policies | 3 weeks | Planned |
| v0.5 Polish | Performance, documentation, examples | 2 weeks | Planned |
| v1.0 Stable | Production-ready release | - | Planned |

---

## Milestone v0.1: Foundation

**Goal**: Establish project foundation with proper workspace structure, documentation, and basic API design.

**Status**: In Progress (80% complete)

### Phase Summary

| Phase | Name | Duration | Status |
|--------|---------|----------|
| 01 | Project Organization | 1 day | Complete |
| 02 | Core SDK Stubs | 2 days | Complete |
| 03 | Test Infrastructure | 1 day | Pending |
| 04 | Documentation Review | Complete    | 2026-02-12 |

### Phase 01: Project Organization

**Status**: Complete

#### Tasks

- [x] 01-01: Workspace Setup
  - [x] Create root `Cargo.toml` with workspace members
  - [x] Create `forge_core/Cargo.toml`
  - [x] Create `forge_runtime/Cargo.toml`
  - [x] Create `forge_agent/Cargo.toml`
  - [x] Verify `cargo build --workspace` succeeds

- [x] 01-02: Directory Structure
  - [x] Create all source directories
  - [x] Create test directories
  - [x] Create documentation directories
  - [x] Update `.gitignore`

### Phase 02: Core SDK Stubs

**Status**: Complete

#### Tasks

- [x] 02-01: Core Types
  - [x] Define `SymbolId`, `BlockId`, `PathId`
  - [x] Define `Location`, `Span`
  - [x] Define `SymbolKind`, `ReferenceKind`, `PathKind`
  - [x] Define `Language` enum

- [x] 02-02: Error Types
  - [x] Define `ForgeError` enum
  - [x] Implement `std::error::Error`
  - [x] Implement `std::fmt::Display`
  - [x] Add `From<>` implementations

- [x] 02-03: Graph Module Stub
  - [x] Define `GraphModule` struct
  - [x] Stub `find_symbol()`, `callers_of()`, `references()`
  - [x] Stub `reachable_from()`, `cycles()`

- [x] 02-04: Search Module Stub
  - [x] Define `SearchModule` struct
  - [x] Define `SearchBuilder` type
  - [x] Stub filter methods and `execute()`

- [x] 02-05: CFG Module Stub
  - [x] Define `CfgModule` struct
  - [x] Define `PathBuilder` type
  - [x] Stub `dominators()`, `loops()`

- [x] 02-06: Edit Module Stub
  - [x] Define `EditModule` struct
  - [x] Define `EditOperation` trait
  - [x] Define `RenameOperation`, `DeleteOperation`

### Phase 03: Test Infrastructure

**Status**: In Progress (3 plans created)

**Goal:** Build comprehensive test infrastructure for forge_core with 80%+ coverage targeting ~100 new tests.

#### Plans

- [ ] 03-01-PLAN.md — Core types and common utilities (25 tests, Wave 1)
- [ ] 03-02-PLAN.md — Forge/ForgeBuilder tests and integration infrastructure (17 tests, Wave 1)
- [ ] 03-03-PLAN.md — Runtime layer expanded tests and integration (25 tests, Wave 2)

#### Tasks (Legacy - superseded by plans above)

- [ ] 03-01: Test Utilities
  - [x] Create `tests/common/mod.rs` — Already exists
  - [ ] Implement `TestForge` fixture builder — Already exists as `test_forge()`
  - [x] Implement temp directory helpers — Already exists
  - [ ] Implement assert helpers — Planned for 03-01

- [ ] 03-02: Unit Tests
  - [ ] Add tests for `types.rs` — Planned for 03-01
  - [ ] Add tests for `error.rs` — Already has 3 tests
  - [ ] Add tests for each module stub — Most modules already tested

- [ ] 03-03: Integration Tests
  - [ ] Create `tests/integration/` directory — Planned for 03-02
  - [ ] Write builder tests — Planned for 03-02
  - [ ] Write module accessor tests — Planned for 03-02

### Phase 04: Documentation Review

**Status**: Pending

#### Tasks

- [ ] 04-01: README Validation
  - [ ] Verify all examples compile
  - [ ] Test quickstart example
  - [ ] Check all links

- [ ] 04-02: API Documentation
  - [ ] Complete `docs/API.md`
  - [ ] Verify rustdoc builds
  - [ ] Add examples to all public items

- [ ] 04-03: Architecture Review
  - [ ] Verify ARCHITECTURE.md accuracy
  - [ ] Update diagrams
  - [ ] Cross-check with code

- [ ] 04-04: Cross-Reference Check
  - [ ] Verify all internal links
  - [ ] Check external references
  - [ ] Ensure consistent terminology

### Exit Criteria

Milestone complete when:
- [ ] `cargo build --workspace` succeeds
- [ ] `cargo test --workspace` passes
- [ ] All documentation reviewed
- [ ] At least one example runs

---

## Milestone v0.2: Core SDK Implementation

**Goal**: Implement actual functionality with SQLiteGraph integration and tool bindings.

**Status**: Planned

**Estimated Duration**: 4 weeks

### Phase Breakdown

| Phase | Focus | Duration | Dependencies |
|--------|---------|--------------|
| 05 | Storage Layer | 1 week | v0.1 complete |
| 06 | Graph & Search | 1 week | Phase 05 |
| 07 | CFG & Edit | 1 week | Phase 05 |
| 08 | Analysis & Integration Tests | 1 week | Phases 06-07 |

### Phase 05: Storage Layer

**Tasks:**
- [ ] Implement `UnifiedGraphStore` with SQLiteGraph backend
- [ ] Add connection pooling
- [ ] Implement prepared statement caching
- [ ] Add transaction management
- [ ] Write migration system

### Phase 06: Graph & Search

**Tasks:**
- [ ] Integrate Magellan for graph operations
- [ ] Implement all `GraphModule` methods
- [ ] Integrate LLMGrep for search
- [ ] Implement `SearchModule` query execution
- [ ] Add comprehensive tests

### Phase 07: CFG & Edit

**Tasks:**
- [ ] Integrate Mirage for CFG operations
- [ ] Implement all `CfgModule` methods
- [ ] Integrate Splice for editing
- [ ] Implement `EditOperation` trait methods
- [ ] Add edit workflow tests

### Phase 08: Analysis & Integration

**Tasks:**
- [ ] Implement `AnalysisModule` composite operations
- [ ] Write end-to-end integration tests
- [ ] Add performance benchmarks
- [ ] Create example programs

### Exit Criteria

- [ ] All module operations functional
- [ ] Integration tests pass
- [ ] Performance targets met
- [ ] Examples demonstrate capabilities

---

## Milestone v0.3: Runtime Layer

**Goal**: Implement indexing, caching, and file watching.

**Status**: Planned

**Estimated Duration**: 3 weeks

### Phase Breakdown

| Phase | Focus | Duration | Dependencies |
|--------|---------|--------------|
| 09 | Indexing System | 1 week | v0.2 complete |
| 10 | Caching Layer | 1 week | Phase 09 |
| 11 | File Watching | 1 week | Phase 09 |

### Phase 09: Indexing System

**Tasks:**
- [ ] Design incremental index format
- [ ] Implement index scheduler
- [ ] Add progress tracking
- [ ] Create index health checks

### Phase 10: Caching Layer

**Tasks:**
- [ ] Design cache architecture
- [ ] Implement symbol cache
- [ ] Implement CFG cache
- [ ] Add TTL management
- [ ] Create cache invalidation

### Phase 11: File Watching

**Tasks:**
- [ ] Integrate `notify` crate
- [ ] Implement debounce logic
- [ ] Add selective reindexing
- [ ] Create watch configuration

### Exit Criteria

- [ ] Automatic reindexing works
- [ ] Cache improves performance
- [ ] Watch mode handles large repos
- [ ] Memory usage acceptable

---

## Milestone v0.4: Agent Layer

**Goal**: Implement deterministic AI orchestration loop.

**Status**: Planned

**Estimated Duration**: 3 weeks

### Phase Breakdown

| Phase | Focus | Duration | Dependencies |
|--------|---------|--------------|
| 12 | Agent Core | 1 week | v0.3 complete |
| 13 | Policy System | 1 week | Phase 12 |
| 14 | LLM Integration (Optional) | 1 week | Phase 12 |

### Phase 12: Agent Core

**Tasks:**
- [ ] Implement agent loop structure
- [ ] Implement observe phase
- [ ] Implement plan phase
- [ ] Implement mutate phase
- [ ] Implement verify phase
- [ ] Implement commit phase

### Phase 13: Policy System

**Tasks:**
- [ ] Define policy DSL
- [ ] Implement built-in policies
- [ ] Create policy validator
- [ ] Add policy composition
- [ ] Document policy patterns

### Phase 14: LLM Integration

**Tasks:**
- [ ] Design pluggable LLM backend
- [ ] Implement prompt generator
- [ ] Add response parser
- [ ] Create fallback logic
- [ ] Add safety validation

### Exit Criteria

- [ ] Agent completes full loop
- [ ] Policies enforced correctly
- [ ] LLM integration optional
- [ ] End-to-end tests pass

---

## Milestone v0.5: Polish

**Goal**: Performance optimization, documentation completion, and example programs.

**Status**: Planned

**Estimated Duration**: 2 weeks

### Tasks

**Performance:**
- [ ] Profile critical paths
- [ ] Optimize hot functions
- [ ] Add memory pools
- [ ] Improve cache hit rates

**Documentation:**
- [ ] Complete API reference
- [ ] Write tutorial guide
- [ ] Add migration guide
- [ ] Create troubleshooting docs

**Examples:**
- [ ] Basic query example
- [ ] Rename refactoring example
- [ ] CFG analysis example
- [ ] Agent usage example

**Quality:**
- [ ] Fix all clippy warnings
- [ ] Achieve 80%+ coverage
- [ ] Add security audit
- [ ] Performance benchmarks

### Exit Criteria

- [ ] Performance targets met
- [ ] Documentation complete
- [ ] Examples work
- [ ] Ready for v1.0

---

## Milestone v1.0: Stable Release

**Goal**: Production-ready ForgeKit SDK.

**Status**: Planned

**Release Criteria:**

**Functionality:**
- [ ] All v0.2-v0.5 features complete
- [ ] No stub implementations remaining
- [ ] All integration tests pass
- [ ] Performance targets met

**Quality:**
- [ ] Zero known critical bugs
- [ ] Zero memory safety issues
- [ ] Full documentation coverage
- [ ] Successful security audit

**Ecosystem:**
- [ ] Published to crates.io
- [ ] CI/CD pipeline
- [ ] Examples in repository
- [ ] Contributor guide complete

---

## Future Considerations (Post-v1.0)

### Potential Enhancements

| Feature | Priority | Complexity | Notes |
|----------|-----------|--------------|--------|
| Native V3 backend | Medium | High | External dependency |
| Language Server | Low | High | Separate project |
| Python support | Medium | Medium | Parser availability |
| WebAssembly | Low | Medium | Browser support |
| Distributed analysis | Low | Very High | Research needed |

### Dependent Projects

| Project | Impact | Status |
|----------|----------|--------|
| sqlitegraph | Critical | Active |
| magellan | Required | Stable |
| llmgrep | Required | Stable |
| mirage | Optional | Stable |
| splice | Required | Stable |

---

## Timeline Visualization

```
v0.1 Foundation        v0.2 Core SDK         v0.3 Runtime    v0.4 Agent    v0.5 Polish     v1.0 Stable
    |                      |                        |                   |              |              |
Week 1                  |                        |                   |              |              |
Phase 01-02            |                        |                   |              |              |
                        |                        |                   |              |              |
Week 2-5               |                        |                   |              |              |
Phase 03-04            |                        |                   |              |              |
(Found Complete)         |                        |                   |              |              |
                        |                        |                   |              |              |
Week 6-9               |                        |                   |              |              |
Phases 05-08           |                        |                   |              |              |
(Core SDK Impl)         |                        |                   |              |              |
                                                 |                   |              |              |
Week 10-12                                      |                   |              |              |
Phases 09-11                                    |                   |              |              |
(Runtime Layer)                                  |                   |              |              |
                                                                     |              |
Week 13-15                                                          |              |
Phases 12-14                                                        |              |
(Agent Layer)                                                       |              |
                                                                                  |
Week 16-17                                                                 |
Phase 15                                                                   |
(Polish)                                                                    |
                                                                                            |
Week 18                                                                              |
v1.0 Release                                                                      |
```

---

## Risk Register

| Risk | Impact | Probability | Mitigation |
|-------|---------|--------------|------------|
| sqlitegraph schema changes | High | Medium | Pin version, track upstream |
| Tool integration complexity | High | High | Prefer library over CLI |
| Performance targets | Medium | Medium | Early benchmarking |
| Agent safety | High | Low | Strict policy enforcement |
| Documentation drift | Low | Medium | Continuous review |

### Phase 15: Tool Integration
**Goal:** Export magellan/llmgrep/mirage/splice functions as library APIs in forge_core

**Status:** Planned

**Depends on:** Phase 04 (Agent Layer)

**Plans:** 0 (run `/gsd:plan-phase 16` to create)

**Goal:** [To be planned]
**Depends on:** Phase 14
**Plans:** 0 plans

Plans:
- [ ] TBD (run /gsd:plan-phase 15 to break down)

### Phase 16: --name 05-tool-integration --after 04 --ordinal 5

**Goal:** [To be planned]
**Depends on:** Phase 15
**Plans:** 0 plans

Plans:
- [ ] TBD (run /gsd:plan-phase 16 to break down)

---

*Last updated: 2026-02-12*

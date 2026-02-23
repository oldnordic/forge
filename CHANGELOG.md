# Changelog

All notable changes to ForgeKit will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.4.0] - 2026-02-23

### Added

**Workflow Orchestration (forge_agent v0.4.0):**
- **WorkflowExecutor** - DAG-based task execution engine
  - Sequential and parallel execution modes
  - Topological sort for dependency resolution
  - Fork-join parallelism via tokio::JoinSet
  - Deadlock detection using Tarjan's SCC algorithm

- **State Management** - Workflow state persistence and recovery
  - CheckpointService for periodic state snapshots
  - Resume from checkpoint after failure
  - WorkflowState and ConcurrentState types
  - Graph drift detection via checksums

- **Cancellation & Timeouts** - Cooperative cancellation and timeout handling
  - CancellationToken with parent-child hierarchy
  - TaskTimeout and WorkflowTimeout types
  - TimeoutConfig with configurable durations
  - Timeout and cancellation audit events

- **Tool Integration** - External tool execution framework
  - ToolRegistry for tool discovery and management
  - ShellCommandTask for command execution
  - FallbackHandler trait (Retry, Skip, Chain)
  - Tool compensation for rollback

- **Compensation & Rollback** - Saga-based transaction pattern
  - CompensationRegistry for undo operations
  - RollbackEngine with strategy selection
  - ToolCompensation for external tools
  - Diamond-pattern rollback support

- **YAML Workflow Parser** - Declarative workflow definitions
  - YamlWorkflow for file-based workflows
  - Task types: GraphQuery, AgentLoop, Shell
  - Flexible parameter serialization
  - Auto-detection of workflow dependencies

- **Validation Framework** - Post-task validation with thresholds
  - ValidationConfig with confidence thresholds
  - ValidationCheckpoint for workflow validation
  - ValidationStatus (Passed, Warning, Failed)
  - Rollback-on-failure support

### Changed

- **WorkflowError** - New variants: TaskFailed, Timeout
- **TaskNode** - Now stores Arc<dyn WorkflowTask> directly
- **execute_task()** - Returns TaskResult instead of ()
- **Removed unwrap() calls** in production executor code
- **Validation** now uses actual task results instead of fake Success

### Dependencies

- `petgraph` 0.8 - Graph algorithms (topological sort, SCC)
- `serde_yaml` 0.9 - YAML workflow parsing
- `tempfile` 3.13 - Temporary directories for tests
- `tokio` 1.49 - Async runtime (JoinSet for parallelism)

### Test Coverage

- **408 tests** passing (workflow module)
- Workflow execution tests (sequential, parallel, diamond)
- Checkpoint and recovery tests
- Cancellation and timeout tests
- Deadlock detection tests
- Rollback and compensation tests

---

## [0.2.2] - 2026-02-21

### Changed
- **Updated sqlitegraph dependency to 2.0.8**
  - Includes compiler warning fixes (31 warnings resolved)
  - Remaining 61 warnings are intentional dead code for API completeness, feature-gated functionality, and future use
  - All 522 tests pass with updated dependency

## [0.2.1] - 2026-02-21

### Fixed
- **Code cleanup** - Removed unused code and fields across all crates
- **forge_core** - Removed unused imports, fields, and inlined unused functions
- **forge_runtime** - Removed unused imports
- **forge_agent** - Removed unused fields from modules (observe, policy, planner, mutate, verify, commit)
- **forge-reasoning** - Removed duplicate SCC module, unused fields and methods
- All 522 tests pass

## [0.2.0] - 2026-02-21

### Added

**Core SDK (forge_core v0.2.0):**
- **GraphModule** - Symbol lookup, reference tracking, call graph navigation
  - `find_symbol(name)` - Find symbols by name
  - `find_symbol_by_id(id)` - Find symbol by ID
  - `callers_of(name)` - Find all callers of a symbol
  - `references(name)` - Find all references to a symbol
  - `impact_analysis(name, depth)` - k-hop traversal for impact analysis
  - `index()` - Run the magellan indexer

- **SearchModule** - Pattern-based and semantic code search
  - `pattern_search(pattern)` - Regex pattern search
  - `semantic_search(query)` - Semantic code search
  - `symbol_by_name(name)` - Find symbols by name
  - `symbols_by_kind(kind)` - Filter symbols by kind

- **CfgModule** - Control flow graph analysis
  - `paths(function)` - Enumerate execution paths
  - `dominators(function)` - Compute dominator tree
  - `loops(function)` - Detect natural loops
  - `index()` - Extract CFG for functions

- **EditModule** - Span-safe code editing
  - `patch_symbol(symbol, replacement)` - Replace symbol definition
  - `rename_symbol(old_name, new_name)` - Rename symbol across codebase

- **AnalysisModule** - Composite operations combining all modules
  - `analyze_impact(symbol)` - Analyze impact of changes
  - `deep_impact_analysis(symbol, depth)` - Deep k-hop traversal
  - `find_dead_code()` - Find unreferenced symbols
  - `complexity_metrics(symbol)` - Calculate cyclomatic complexity
  - `analyze_source_complexity(source)` - Analyze source directly
  - `cross_references(symbol)` - Get callers and callees
  - `module_dependencies()` - Analyze module dependencies
  - `find_dependency_cycles()` - Detect circular dependencies
  - `benchmarks()` - Performance benchmarks for operations

- **EditOperation Trait** - Safe code transformation API
  - `InsertOperation` - Insert content at location
  - `DeleteOperation` - Delete symbols with validation
  - `RenameOperation` - Rename symbols with validation
  - `ErrorResult` - Always-fails operation
  - `ApplyResult` enum (Applied, AlwaysError, Pending, Failed)
  - `Diff` type for change previews

- **DeadCodeAnalyzer** - Find unused code
  - Database-backed dead code detection
  - Filters public API and entry points
  - Graceful handling of empty databases

- **ComplexityMetrics** - Code complexity analysis
  - Cyclomatic complexity calculation
  - Decision point counting
  - Nesting depth analysis
  - Risk level classification (Low/Medium/High/VeryHigh)

- **ModuleAnalyzer** - Module dependency analysis
  - Cross-module dependency graph
  - Circular dependency detection using petgraph
  - Depth analysis

**Forge Runtime (forge_runtime v0.1.0):**
- **ForgeRuntime** - Runtime services stub
  - RuntimeConfig for configuration
  - RuntimeStats for metrics
  - File watching infrastructure (notify dependency)

**Forge Agent (forge_agent v0.1.0):**
- **Agent** - Deterministic AI orchestration loop
  - Six-phase loop: Observe → Constrain → Plan → Mutate → Verify → Commit
  - Observation, ConstrainedPlan, ExecutionPlan types
  - MutationResult, VerificationResult, CommitResult types

**Forge-Reasoning:**
- **Hypothesis tracking** with Bayesian confidence
- **Evidence storage** with source types (Observation, Experiment, Reference, Deduction)
- **Belief dependency graph** with cycle detection (petgraph)
- **Async verification** with retry logic and exponential backoff
- **Knowledge gap analysis** with multi-factor priority scoring
- **Confidence propagation** with cascade preview
- **Impact analysis engine** with snapshot revert

### Changed

- **UnifiedGraphStore** - Unified backend abstraction
  - Dual backend support (SQLite / Native V3)
  - ACID transactions for atomic operations
  - MVCC reads via SnapshotId

- **Storage backend** - Uses sqlitegraph 2.0.7
  - 35+ graph algorithms (cycles, reachability, SCC, dominators)
  - Dual backend: SQLite (stable) or Native V3 (high performance)

### Dependencies

**Core:**
- `sqlitegraph` 2.0.7 - Graph database backend
- `tokio` 1.49.0 - Async runtime
- `serde` / `serde_json` 1.0 - Serialization
- `thiserror` 2.0.18 - Error types
- `async-trait` 0.1.89 - Async trait support
- `petgraph` 0.8.3 - Graph algorithms
- `regex` 1.12.3 - Pattern matching
- `blake3` 1.5.3 - Hashing
- `anyhow` 1.0 - Error handling at API boundaries

**Tool Integrations (optional):**
- `magellan` 2.4.6 - Code indexing
- `llmgrep` 3.0.9 - Semantic search
- `mirage-analyzer` 1.0.3 - CFG analysis
- `splice` 2.5.3 - Code editing

**Forge-Reasoning:**
- `uuid` 1.21.0 - Unique identifiers
- `chrono` 0.4.43 - Timestamps
- `indexmap` 2.13.0 - Ordered collections

### Feature Flags

**Storage Backends:**
- `sqlite` (default) - SQLite backend
- `native-v3` - Native V3 backend

**Tool Integrations:**
- `magellan` / `magellan-sqlite` / `magellan-v3`
- `llmgrep` / `llmgrep-sqlite` / `llmgrep-v3`
- `mirage` / `mirage-sqlite` / `mirage-v3`
- `splice` / `splice-sqlite` / `splice-v3`
- `tools` - All tools
- `tools-sqlite` / `tools-v3` - All tools with specific backend
- `full-sqlite` / `full-v3` - Everything with specific backend

**Forge-Reasoning:**
- `websocket` - WebSocket API support

### Test Coverage

- **535+ tests** passing (100% pass rate)
- E2E tests: 163 tests across 8 waves
- Unit tests: 155 forge_core tests + 217 other tests
- Integration tests for cross-module functionality
- Performance benchmarks included

### Documentation

- API reference for all modules
- Architecture documentation
- User manual with working examples
- Contributing guidelines
- Development workflow guide

---

## [0.1.0] - 2026-02-12

### Added

- Workspace structure with 4 crates
- Public API stubs
- Core type definitions (SymbolId, BlockId, Location, Span)
- Error hierarchy (ForgeError)
- Basic test infrastructure
- Project documentation (README, ARCHITECTURE, API, MANUAL)

---

## Release Notes Template

```markdown
## [X.Y.Z] - YYYY-MM-DD

### Added
- New feature
- Another new feature

### Changed
- Modified behavior
- Updated API

### Deprecated
- Feature to be removed

### Removed
- Removed feature

### Fixed
- Bug fix
- Another bug fix

### Security
- Security fix
```

---

*Last updated: 2026-02-21*

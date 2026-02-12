# Technical Concerns and Debt

**Project**: ForgeKit v0.1.0
**Status**: Design Phase
**Last Updated**: 2026-02-12

---

## Known Limitations

### No Graph Database Operations

All core modules return `BackendNotAvailable` errors for actual operations:

- **GraphModule**: `find_symbol()`, `callers_of()`, `references()`, `reachable_from()`, `cycles()` all stubbed
- **SearchModule**: `pattern()`, `execute()` return unimplemented errors
- **CfgModule**: `dominators()`, `loops()`, path enumeration not functional
- **EditModule**: `preview()`, `apply()`, `rollback()` all return errors

**Workaround**: None - this is foundational work awaiting v0.2 implementation

### No Storage Backend Integration

`UnifiedGraphStore` creates directory structure but provides no actual database access:

- `symbol_exists()` always returns `false`
- `get_symbol()` always returns `SymbolNotFound`
- No sqlitegraph client initialization
- Database schema not validated

**Impact**: All graph-dependent operations are non-functional

### Empty Runtime and Agent Crates

- **forge_runtime**: `watch()` and `clear_cache()` return "not yet implemented" errors
- **forge_agent**: All agent phases (observe, constrain, plan, mutate, verify, commit) are stubs

**Impact**: Cannot perform automated operations or caching

---

## Technical Debt

### Inconsistent Error Handling Between Modules

The `EditOperation` trait defines synchronous methods:

```rust
fn verify(self) -> Result<Self>;
fn preview(self) -> Result<Diff>;
fn apply(self) -> Result<Self::Output>;
```

But the API documentation shows async usage in examples:

```rust
// From docs - but EditOperation is sync!
edit.rename_symbol("OldName", "NewName")?.verify()?.await?
```

**Impact**: Documentation misleading, API inconsistent

**Resolution**: Either make `EditOperation` async or update all documentation/examples

### Missing tempfile Dependency

Test files use `tempfile::tempdir()` but `tempfile` is not in `[dev-dependencies]`:

- `forge_core/src/storage/mod.rs:75`
- `forge_core/src/graph/mod.rs:141`
- `forge_agent/src/lib.rs:312`

**Impact**: Tests may not compile without manual dependency addition

### Duplicate Type Definitions

`Path` and `Loop` types defined in both:

- `forge_core/src/types.rs` (core shared types)
- `forge_core/src/cfg/mod.rs` (CFG-specific module)

This creates potential naming conflicts and import ambiguity.

**Resolution**: Should use cfg-specific names like `CfgPath`, `CfgLoop` or consolidate

### Incomplete ForgeBuilder Implementation

`ForgeBuilder::build()` is truncated in `lib.rs` (ends at line 242):

```rust
pub async fn build(self) -> anyhow::Result<Forge> {
    let path = self.path
        .ok_or_else(|| anyhow!("path is required"))?;

    // File ends here - incomplete implementation
```

**Impact**: Builder pattern is non-functional

---

## Implementation Gaps

### Core SDK (forge_core)

| Module | Status | Missing |
|---------|---------|----------|
| `storage/mod.rs` | Stub | No sqlitegraph client, no queries |
| `graph/mod.rs` | Stub | All methods return `BackendNotAvailable` |
| `search/mod.rs` | Stub | LLMGrep integration missing |
| `cfg/mod.rs` | Stub | Mirage integration missing |
| `edit/mod.rs` | Stub | Splice integration missing |
| `analysis/mod.rs` | Stub | Combined operations non-functional |

### forge_runtime

| Component | Status | Missing |
|-----------|---------|----------|
| File watcher | Not started | No `notify` integration |
| Incremental indexing | Not started | No reindex logic |
| Query cache | Not started | No caching layer |
| Metrics collection | Not started | `RuntimeStats` returns zeros |

### forge_agent

| Phase | Status | Missing |
|-------|---------|----------|
| Observe | Stub | No graph query integration |
| Constrain | Partial | Policy enum defined but no validation logic |
| Plan | Stub | No LLM integration or planning algorithms |
| Mutate | Stub | No edit module integration |
| Verify | Stub | No compilation/test validation |
| Commit | Stub | No transaction management |

### Native V3 Backend

**Status**: External dependency on sqlitegraph project

- Feature flag defined (`native-v2`) but no implementation
- Native V3 backend mentioned in roadmap but status unknown
- Migration tools between SQLite and Native not designed

**Dependency**: sqlitegraph project roadmap must be tracked

---

## Architectural Risks

### Span-Safety Guarantee Not Proven

The entire edit module design assumes span-safety from Splice integration, but:

- No verification that spans remain valid after concurrent edits
- No conflict detection for overlapping edits
- Rollback mechanism unspecified

**Risk**: Data corruption on multi-file edits, especially during refactoring

**Mitigation Needed**:
- Implement span validation before apply
- Add conflict detection
- Design transactional edit log

### Graph Consistency Across Mutations

When edits are applied, graph must stay consistent:

- No defined invalidation strategy for cache
- No incremental reindex specification
- Concurrent edit handling undefined

**Risk**: Stale graph data leading to incorrect queries

**Mitigation Needed**:
- Define cache invalidation protocol
- Specify reindex triggers (file-level vs symbol-level)
- Consider write-ahead log for edits

### Backend Abstraction Leakage

The `UnifiedGraphStore` is supposed to abstract backend differences, but:

- No backend trait defined internally
- SQLite vs Native V3 differences may leak to API
- Feature-specific capabilities unhandled

**Risk**: Code duplication or runtime feature detection needed

**Mitigation**: Define capability negotiation layer before v0.5

---

## Performance Concerns

### No Caching Strategy

Despite `ForgeBuilder` accepting `cache_ttl`, no caching exists:

- Symbol queries always hit database
- CFG paths recomputed every time
- Search results not memoized

**Targets from ROADMAP.md**:
- Symbol lookup <10ms
- Reference query <50ms
- CFG enumeration <100ms

**Current**: Unknown (no backend to measure)

### File Watching Not Designed

Incremental reindex requirements from ARCHITECTURE.md:

```
On file change:
1. Detect modified files (watcher)
2. Invalidate affected graph regions
3. Re-parse only changed files
4. Update adjacency relationships
5. Clear affected cache entries
```

**Concerns**:
- How to determine "affected graph regions" efficiently?
- What about files that don't change but depend on changed files?
- Watching strategies for large codebases (100k+ files)?

---

## Security Considerations

### Arbitrary Code Execution via LLM Integration

forge_agent plans to integrate LLMs for planning:

```rust
/// Generates an execution plan from the constrained observation.
pub async fn plan(&self, _constrained: ConstrainedPlan) -> Result<ExecutionPlan>
```

**Risk**: If LLM output is not sandboxed/validated, could execute arbitrary operations

**Mitigation Needed**:
- Validate all PlanOperations against allowlist
- Require explicit confirmation for destructive operations
- Audit logging for all agent actions

### Database File Permissions

Graph database at `.forge/graph.db` may contain sensitive code structure:

- No specified permission model
- Could expose codebase structure to unintended readers
- No encryption at rest specified

**Impact**: Information disclosure in shared environments

---

## Development Risks

### Strict File Size Limits May Cause Over-Engineering

DEVELOPMENT_WORKFLOW.md specifies:

```
| Component | Limit |
|------------|--------|
| forge_core modules | 300 LOC |
| Tests | 500 LOC |
```

**Risk**: Developers may split code unnaturally to meet limits rather than extracting real modules

**Recommendation**: Focus on single responsibility principle, treat limits as guidelines not hard rules

### TDD Workflow May Slow Initial Development

MANDATORY TDD requirement from DEVELOPMENT_WORKFLOW.md:

```
> NEVER write code based on assumptions. ALWAYS read source and query graph first.
```

**Concern**: When graph doesn't exist yet (current state), this creates circular dependency

**Workaround**: For stub implementation, architecture decisions are appropriate

### External Tool Dependency Chain

ForgeKit depends on correct operation of:

1. **sqlitegraph**: Must be stable and maintain schema
2. **Magellan**: Must produce consistent symbol IDs
3. **LLMGrep**: Must return valid AST queries
4. **Mirage**: Must generate correct CFGs
5. **Splice**: Must provide span-safe edits

**Risk**: Version mismatches or bugs in any tool break ForgeKit

**Mitigation**:
- Pin exact versions in Cargo.toml
- Add integration tests for each tool
- Document known issues with upstream versions

---

## Blocking Issues

### Phase Transition Criteria Unclear

v0.1-ROADMAP.md lists 4 phases, but exit criteria are vague:

```
## Phase 01: Project Organization
**Status**: [ ] Planned
```

**Concern**: No clear definition of "done" for each phase

**Recommendation**: Add concrete checkboxes and automated tests

### Workspace Build Status Unknown

`cargo build --workspace` status not verified in documentation:

- `tempfile` missing from dev-dependencies
- `ForgeBuilder::build()` incomplete
- Example code in comments may not compile

**Impact**: May not actually meet v0.1 exit criterion #1 ("Workspace compiles")

---

## Dependencies on External Projects

| Project | Role | Version | Risk |
|----------|---------|---------|-------|
| sqlitegraph | Graph backend | ^1.5 | Schema changes break compatibility |
| Magellan | Code indexing | v2.2.1 | Symbol ID format changes |
| LLMGrep | Semantic search | Stable | Query language changes |
| Mirage | CFG analysis | Stable | Path representation changes |
| Splice | Edit operations | v2.5.0 | Span handling changes |

**Tracking Needed**: Monitor upstream repositories for breaking changes

---

## Documentation Debt

### Example Code vs Implementation Mismatch

API documentation shows async patterns that don't match implementation:

```rust
// docs/API.md shows:
let symbols = graph.find_symbol("main").await?;

// But actual API may differ due to EditOperation sync methods
```

**Resolution**: Audit all examples against actual code once implemented

### Missing Error Recovery Documentation

Error types defined but recovery strategies only documented in ARCHITECTURE.md:

- No user-facing error handling guide
- No retry policy specifications
- No graceful degradation for unavailable features

---

*Last updated: 2026-02-12*

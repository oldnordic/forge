# Plan: Phase 4 - Agent Layer

**Phase**: 04 - Agent Layer
**Milestone**: v0.4 Agent System
**Status**: 📋 Planned
**Created**: 2026-02-12
**Estimated Duration**: 3 weeks

---

## Objective

Build the Agent Layer on top of the completed Core SDK (Phase 1) and Runtime Layer (Phase 2). This phase implements a deterministic AI-driven agent loop for safe, automated code operations with policy constraints and verification.

---

## Phase Context

### Previous State (Phases 1 & 2 Complete)

| Component | Status | Notes |
|-----------|---------|--------|
| Core SDK Foundation | ✅ Complete | All modules functional |
| Graph Module | ✅ Complete | Symbol/reference queries working |
| Search Module | ✅ Complete | Semantic filter builder working |
| CFG Module | ✅ Complete | Path enumeration working |
| Edit Module | ✅ Complete | Verify/preview/apply working |
| Analysis Module | ✅ Complete | Impact analysis working |
| Runtime Layer | ✅ Complete | Watching, caching, pooling working |
| Agent Stubs | ✅ Complete | Types defined, methods stubbed |

### Target State (v0.4 @ 100%)

| Component | Target | Notes |
|-----------|---------|--------|
| Observation Phase | ⚠️ Pending | Graph-based context gathering |
| Policy Engine | ⚠️ Pending | Constraint validation system |
| Planning Engine | ⚠️ Pending | Step generation from constraints |
| Mutation Engine | ⚠️ Pending | Transaction-based edits |
| Verification Engine | ⚠️ Pending | Post-mutation validation |
| Commit Engine | ⚠️ Pending | Transaction finalization |
| Full Agent Loop | ⚠️ Pending | End-to-end flow |
| Examples | ⚠️ Pending | Usage demonstrations |

---

## Task Breakdown

### 1. Observation Phase Implementation

**Priority**: P0 (Must Have)
**Complexity**: High
**Dependencies**: Phase 1 (Graph, Search, CFG modules)
**Estimated**: 4-5 days

#### File: `forge_agent/src/observe.rs` (NEW)

**Objective**: Implement graph-based context gathering for agent queries.

**Required Changes:**
- Create `Observer` struct that uses Forge SDK
- Implement `gather_symbols()` to find relevant symbols
- Implement `gather_references()` to trace call chains
- Implement `gather_cfg()` for control flow context
- Add LLM integration for semantic understanding
- Implement query parsing for natural language

**Acceptance Criteria:**
- [ ] `Observer` struct created with Forge instance
- [ ] `gather_symbols()` returns symbols matching query
- [ ] `gather_references()` traces incoming/outgoing refs
- [ ] `gather_cfg()` retrieves path information
- [ ] Natural language query parsing works
- [ ] Integration with llmgrep for semantic search
- [ ] Unit tests (minimum 5 tests)

**File Size Target**: ≤ 300 lines

**Implementation Notes:**
- Use forge::Forge for all graph queries
- Semantic search via search module
- Cache observation results for performance
- Query pattern: "find functions that call X"
- Support filters: file, kind, complexity

---

### 2. Policy Engine Implementation

**Priority**: P0 (Must Have)
**Complexity**: High
**Dependencies**: None (standalone validation)
**Estimated**: 4-5 days

#### File: `forge_agent/src/policy.rs` (NEW)

**Objective**: Implement policy constraint validation system.

**Required Changes:**
- Implement `NoUnsafeInPublicAPI` validation
- Implement `PreserveTests` validation
- Implement `MaxComplexity` validation
- Add policy composition (AND/OR logic)
- Implement custom policy DSL evaluation
- Create policy violation reporting

**Acceptance Criteria:**
- [ ] `NoUnsafeInPublicAPI` detects unsafe in pub fn/struct
- [ ] `PreserveTests` checks test preservation
- [ ] `MaxComplexity(N)` validates cyclomatic complexity
- [ ] Policy composition works (All, Any policies)
- [ ] Custom policies can be defined and validated
- [ ] Detailed violation messages with locations
- [ ] Unit tests (minimum 6 tests)

**File Size Target**: ≤ 350 lines

**Implementation Notes:**
- Parse AST for unsafe detection
- Use CFG module for complexity calculation
- Compare before/after for test preservation
- Policy types: Validate, Transform, Enforce
- Violation format: file:line:column - reason

---

### 3. Planning Engine Implementation

**Priority**: P0 (Must Have)
**Complexity**: High
**Dependencies**: Task 04-01 (Observation)
**Estimated**: 5-6 days

#### File: `forge_agent/src/planner.rs` (NEW)

**Objective**: Implement execution plan generation from observations.

**Required Changes:**
- Create `Planner` struct for plan generation
- Implement `generate_steps()` from observation
- Implement `estimate_impact()` for change scope
- Add conflict detection (concurrent edits)
- Implement step ordering based on dependencies
- Add rollback plan generation

**Acceptance Criteria:**
- [ ] `Planner` generates ordered steps from query
- [ ] Steps are: Rename, Delete, Create operations
- [ ] Impact estimation includes affected files and complexity
- [ ] Conflict detection prevents overlapping edits
- [ ] Dependency ordering enforced (symbols before references)
- [ ] Rollback plan generated for all mutations
- [ ] Unit tests (minimum 5 tests)

**File Size Target**: ≤ 300 lines

**Implementation Notes:**
- Step types: Rename, Delete, Create, Modify
- Dependency graph for ordering
- Conflict: same file/region in multiple steps
- Impact: file count, symbol count, complexity delta
- Fallback: abort on unresolvable conflicts

---

### 4. Mutation Engine Implementation

**Priority**: P0 (Must Have)
**Complexity**: High
**Dependencies**: Task 04-03 (Planning), Phase 1 Edit Module
**Estimated**: 4-5 days

#### File: `forge_agent/src/mutate.rs` (NEW)

**Objective**: Implement transaction-based code mutation.

**Required Changes:**
- Create `Mutator` struct with edit module integration
- Implement `begin_transaction()` for change isolation
- Implement `apply_step()` for individual operations
- Implement `rollback()` for transaction abort
- Add preview mode for dry-run execution
- Implement atomic multi-file changes

**Acceptance Criteria:**
- [ ] `Mutator` uses forge::EditModule for changes
- [ ] `begin_transaction()` creates isolated change set
- [ ] `apply_step()` executes Rename/Delete/Create
- [ ] `rollback()` reverts all applied steps
- [ ] Preview mode shows diffs without applying
- [ ] Multi-file transactions are atomic
- [ ] Unit tests (minimum 5 tests)

**File Size Target**: ≤ 300 lines

**Implementation Notes:**
- Use EditModule for actual edits
- Transaction: list of applied operations
- Rollback: reverse operations in opposite order
- Preview: return diffs without committing
- Atomic: all-or-nothing semantics

---

### 5. Verification Engine Implementation

**Priority**: P0 (Must Have)
**Complexity**: Medium
**Dependencies**: Task 04-04 (Mutation)
**Estimated**: 3-4 days

#### File: `forge_agent/src/verify.rs` (NEW)

**Objective**: Implement post-mutation validation system.

**Required Changes:**
- Create `Verifier` struct
- Implement `compile_check()` for syntax validation
- Implement `test_check()` for test preservation
- Implement `graph_check()` for graph consistency
- Add policy re-validation after changes
- Implement diagnostic collection

**Acceptance Criteria:**
- [ ] `Verifier` runs cargo check for syntax
- [ ] `test_check()` ensures tests still pass
- [ ] `graph_check()` validates symbol integrity
- [ ] Policy validation re-runs after mutation
- [ ] Diagnostics collected with locations
- [ ] Verification result is pass/fail with details
- [ ] Unit tests (minimum 4 tests)

**File Size Target**: ≤ 250 lines

**Implementation Notes:**
- Use `cargo check` for compile validation
- `cargo test` for test preservation
- Graph queries for consistency (no orphan refs)
- Re-run policy engine on changed files
- Diagnostic format: level:file:line:msg

---

### 6. Commit Engine Implementation

**Priority**: P0 (Must Have)
**Complexity**: Medium
**Dependencies**: Task 04-05 (Verification)
**Estimated**: 2-3 days

#### File: `forge_agent/src/commit.rs` (NEW)

**Objective**: Implement transaction finalization and persistence.

**Required Changes:**
- Create `Committer` struct
- Implement `finalize()` for transaction commit
- Add git integration for version control
- Implement `generate_summary()` for change description
- Add rollback checkpoint creation
- Implement metadata persistence

**Acceptance Criteria:**
- [ ] `Committer` finalizes verified transactions
- [ ] Git integration creates commits
- [ ] `generate_summary()` creates meaningful messages
- [ ] Rollback checkpoints saved
- [ ] Metadata persisted to database
- [ ] Transaction ID returned for tracking
- [ ] Unit tests (minimum 3 tests)

**File Size Target**: ≤ 200 lines

**Implementation Notes:**
- Use git2 crate for VCS operations
- Commit message format: conventional commits
- Checkpoint: tag or branch for rollback
- Metadata: timestamp, plan ID, verification status

---

### 7. Agent Loop Integration

**Priority**: P0 (Must Have)
**Complexity**: Medium
**Dependencies**: All previous tasks (04-01 through 04-06)
**Estimated**: 2-3 days

#### File: `forge_agent/src/lib.rs` (UPDATE)

**Objective**: Wire all phases into the deterministic agent loop.

**Required Changes:**
- Implement full `observe()` method using Observer
- Implement full `constrain()` method using Policy
- Implement full `plan()` method using Planner
- Implement full `mutate()` method using Mutator
- Implement full `verify()` method using Verifier
- Implement full `commit()` method using Committer
- Add `run()` method for complete loop execution
- Add progress reporting and error handling

**Acceptance Criteria:**
- [ ] All six phases are functional
- [ ] `run()` executes full loop: observe → commit
- [ ] Progress reporting at each phase
- [ ] Errors cause rollback with cleanup
- [ ] Policy violations halt execution early
- [ ] Verification failures trigger rollback
- [ ] Integration tests (minimum 3 tests)

**File Size Target**: Update existing (add ~100 lines)

**Implementation Notes:**
- Loop: Observe → Constrain → Plan → Mutate → Verify → Commit
- Early exit on policy violation
- Rollback on verification failure
- Progress: print or callback-based
- Error handling: detailed with phase context

---

### 8. CLI Integration

**Priority**: P1 (Should Have)
**Complexity**: Medium
**Dependencies**: Task 04-07 (Agent Loop)
**Estimated**: 2-3 days

#### File: `forge_agent/src/cli.rs` (NEW)

**Objective**: Add CLI interface for agent operations.

**Required Changes:**
- Create CLI using clap v4
- Implement `agent run` subcommand
- Implement `agent plan` subcommand (dry-run)
- Implement `agent rollback` subcommand
- Add configuration file support
- Implement verbose/debug output modes

**Acceptance Criteria:**
- [ ] `forge-agent run` executes full loop
- [ ] `forge-agent plan` shows plan without applying
- [ ] `forge-agent rollback` reverts last transaction
- [ ] Config file supports agent settings
- [ ] Verbose mode shows phase-by-phase output
- [ ] Help documentation complete
- [ ] Integration tests (minimum 2 tests)

**File Size Target**: ≤ 200 lines

**Implementation Notes:**
- Use clap derive API
- Config: TOML file in .forge/
- Subcommands: run, plan, rollback, status
- Output: human (default) or json

---

### 9. Documentation

**Priority**: P1 (Should Have)
**Complexity**: Low
**Dependencies**: All implementation tasks
**Estimated**: 1-2 days

#### Files to modify:
- `forge_agent/src/lib.rs` (module docs)
- `forge_agent/src/observe.rs` (examples)
- `forge_agent/src/policy.rs` (policy examples)
- `forge_agent/src/planner.rs` (planning examples)
- `forge_agent/src/mutate.rs` (mutation examples)
- `docs/AGENT.md` (CREATE - user guide)

**Changes:**
- Add comprehensive examples to each module
- Create policy writing guide
- Document agent loop behavior
- Create troubleshooting guide

**Acceptance Criteria:**
- [ ] Each module has 2+ working examples
- [ ] Policy guide shows common patterns
- [ ] Agent loop diagram included
- [ ] CLI reference complete
- [ ] Troubleshooting section covers common issues
- [ ] `cargo doc --no-deps` completes

**File Size Target**: Update existing files + new docs

---

## Dependencies

### External Dependencies

| Crate | Version | Status | Notes |
|--------|---------|--------|-------|
| git2 | 0.18 | ⚠️ MISSING | Add for VCS integration |
| clap | 4.4 | ⚠️ MISSING | Add for CLI |
| serde | 1.0 | ✅ In Cargo.toml | For config serialization |
| toml | 0.8 | ⚠️ MISSING | Add for config parsing |

### Internal Dependencies

```
Task 04-01 (Observation)    → Phase 1 (Graph, Search, CFG)
Task 04-02 (Policy)         → None (standalone)
Task 04-03 (Planning)        → Task 04-01 (Observation)
Task 04-04 (Mutation)       → Task 04-03 (Planning), Phase 1 (Edit)
Task 04-05 (Verification)    → Task 04-04 (Mutation)
Task 04-06 (Commit)         → Task 04-05 (Verification)
Task 04-07 (Loop)          → All previous tasks
Task 04-08 (CLI)            → Task 04-07 (Loop)
Task 04-09 (Docs)           → All implementation tasks
```

---

## File/Module Structure

### New Agent Files

| File | Purpose | LOC Target |
|-------|---------|------------|
| `observe.rs` | Graph-based context gathering | ≤ 300 |
| `policy.rs` | Constraint validation system | ≤ 350 |
| `planner.rs` | Execution plan generation | ≤ 300 |
| `mutate.rs` | Transaction-based edits | ≤ 300 |
| `verify.rs` | Post-mutation validation | ≤ 250 |
| `commit.rs` | Transaction finalization | ≤ 200 |
| `cli.rs` | CLI interface | ≤ 200 |

### Updated Module List

```
forge_agent/src/lib.rs           (Update - full loop implementation)
forge_agent/src/observe.rs       (NEW - observation phase)
forge_agent/src/policy.rs        (NEW - policy engine, expanded from stub)
forge_agent/src/planner.rs       (NEW - planning phase)
forge_agent/src/mutate.rs       (NEW - mutation phase)
forge_agent/src/verify.rs        (NEW - verification phase)
forge_agent/src/commit.rs        (NEW - commit phase)
forge_agent/src/cli.rs          (NEW - CLI interface)
```

---

## Success Criteria

### Phase Complete When:

1. **Agent Loop Functional**
   - [ ] Full observe → commit loop executes successfully
   - [ ] Policy violations prevent unsafe mutations
   - [ ] Verification failures trigger rollback
   - [ ] Transactions are atomic and isolated

2. **Policy System**
   - [ ] All built-in policies validate correctly
   - [ ] Custom policies can be defined
   - [ ] Policy violations include clear messages
   - [ ] Policy composition works (AND/OR)

3. **Planning & Mutation**
   - [ ] Plans include all necessary steps
   - [ ] Impact estimation is accurate
   - [ ] Conflict detection prevents corruption
   - [ ] Rollback works for any failed mutation

4. **Verification**
   - [ ] Compile checks catch syntax errors
   - [ ] Test preservation verified
   - [ ] Graph consistency validated
   - [ ] Diagnostics are clear and actionable

5. **Integration**
   - [ ] CLI provides user-friendly interface
   - [ ] Git integration works
   - [ ] Config file supported
   - [ ] Progress reporting informative

6. **Test Coverage**
   - [ ] Unit tests for each module (≥80% coverage)
   - [ ] Integration tests for full loop
   - [ ] All tests pass with `cargo test --workspace`

7. **Documentation**
   - [ ] All modules documented
   - [ ] Policy writing guide complete
   - [ ] CLI reference complete
   - [ ] `cargo doc --no-deps` passes

8. **Code Quality**
   - [ ] No `#[allow(...)]` without justification
   - [ ] `cargo clippy` passes
   - [ ] `cargo fmt` applied

---

## Risk Register

| Risk | Impact | Mitigation |
|-------|---------|------------|
| LLM integration complexity | High | Phase in: use simple NLP first |
| Policy validation false positives | Medium | Allow user overrides with warnings |
| Transaction atomicity | Medium | Careful rollback implementation |
| Git integration issues | Medium | Use well-tested git2 crate |
| Performance at scale | Low | Cache observations, parallel steps |

---

## Estimated Timeline

**Week 1** (Days 1-5):
- Day 1-5: Task 1 (Observation Phase)

**Week 2** (Days 6-10):
- Day 6-9: Task 2 (Policy Engine)
- Day 10: Start Task 3 (Planning Engine)

**Week 3** (Days 11-15):
- Day 11-13: Complete Task 3 (Planning Engine)
- Day 14-15: Task 4 (Mutation Engine)

**Week 4** (Days 16-20):
- Day 16-18: Task 5 (Verification Engine)
- Day 19-20: Task 6 (Commit Engine)

**Week 5** (Days 21-22):
- Day 21-22: Task 7 (Agent Loop Integration)

**Week 5-6** (Days 23-25):
- Day 23-25: Task 8 (CLI Integration)

**Week 6** (Days 26-27):
- Day 26-27: Task 9 (Documentation, testing, bug fixes)

---

## Next Phase Preparation

Upon completion of Phase 4, project will have:
- Complete Core SDK Foundation ✅
- Complete Runtime Layer ✅
- Complete Agent Loop ✅
- Ready for v0.4 release

Future phases could include:
- v0.5: Advanced policy DSL
- v0.6: Multi-agent coordination
- v0.7: Learning from operations

---

*Last updated: 2026-02-12*

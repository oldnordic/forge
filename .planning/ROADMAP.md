# Roadmap: ForgeKit

## Overview

ForgeKit is a deterministic code intelligence SDK for Rust - "LLVM for AI Code Agents". It combines Magellan (graph operations), LLMGrep (semantic search), Mirage (CFG analysis), and Splice (span-safe editing) into a unified API with multi-step workflow orchestration capabilities.

## Milestones

- ✅ **v0.1 Foundation** — Project setup and initial structure (shipped 2026-02-12)
- ✅ **v0.3 Agent Orchestration & Reasoning** — Phases 1-7 (shipped 2026-02-22)
- 🚧 **v0.4 Advanced Agent Multi-step Workflows** — Phases 8-12 (in progress)

## Phases

<details>
<summary>✅ v0.3 Agent Orchestration & Reasoning (Phases 1-7) — SHIPPED 2026-02-22</summary>

**Archive:** [.planning/milestones/v0.3-ROADMAP.md](.planning/milestones/v0.3-ROADMAP.md)

- [x] Phase 1: Core Runtime & Integration (2/2 plans) — completed 2026-02-22
- [x] Phase 2: Reasoning Foundation (1/1 plan) — completed 2026-02-22
- [x] Phase 3: Agent Orchestration (4/4 plans) — completed 2026-02-22
- [x] Phase 4: Analysis & Quality (2/2 plans) — completed 2026-02-22
- [x] Phase 5: Complete Runtime Integration (2/2 plans) — completed 2026-02-22
- [x] Phase 6: Complete Test Coverage (1/1 plan) — completed 2026-02-22
- [x] Phase 7: Code Cleanup (1/1 plan) — completed 2026-02-22

**Summary:** Runtime layer with re-indexing and caching, deterministic 6-phase agent loop, reasoning foundation with Bayesian confidence, gap closure complete (11/11 TODOs resolved).
</details>

### 🚧 v0.4 Advanced Agent Multi-step Workflows (In Progress)

**Milestone Goal:** Enable agents to orchestrate complex multi-step workflows with external tool integration, parallel execution, and full state management.

#### Phase 8: Workflow Foundation
**Goal**: DAG-based workflow execution with dependency resolution and verification
**Depends on**: Phase 7
**Requirements**: WDEF-01, WDEF-02, WDEF-03, WDEF-04, WDEF-05, WTOOL-03, WOBS-02, WOBS-03
**Success Criteria** (what must be TRUE):
  1. User can define multi-step workflow with dependencies via Rust API
  2. Workflow scheduler executes steps in topological order based on dependencies
  3. System auto-detects step dependencies from code structure using graph queries
  4. Workflow failure triggers rollback of only dependent steps using DAG backward traversal
  5. System verifies workflow correctness before execution (detects cycles, missing dependencies)
  6. User can define simple workflows in YAML, complex workflows via Rust API
  7. User can inspect workflow state including current step, completed steps, and pending work
  8. Workflow execution events are logged to audit trail
**Plans**: 5 plans

Plans:
- [ ] 08-01: DAG scheduler with topological sort and cycle detection
- [ ] 08-02: Workflow definition API (Rust programmatic)
- [ ] 08-03: YAML workflow parser for simple workflows
- [ ] 08-04: Dependency auto-detection using graph queries
- [ ] 08-05: Rollback engine with DAG backward traversal

#### Phase 9: State Management
**Goal**: Workflow checkpointing, recovery, and compensation-based rollback
**Depends on**: Phase 8
**Requirements**: WSTA-01, WSTA-02, WSTA-05, WOBS-04
**Success Criteria** (what must be TRUE):
  1. Workflow state is checkpointed after each step completion
  2. Failed workflow can resume from last checkpoint instead of restarting
  3. External tool side effects use compensation transactions for rollback (Saga pattern)
  4. Validation checkpoints between steps check confidence scores and trigger rollback if needed
**Plans**: 4 plans

Plans:
- [ ] 09-01: State checkpointing with forge-reasoning integration (Wave 1)
- [ ] 09-02: Resume after failure with state recovery (Wave 2)
- [ ] 09-03: Compensation transaction registry for external tool rollback (Wave 3)
- [ ] 09-04: Validation checkpoints with confidence scoring (Wave 4)

#### Phase 10: Cancellation & Timeouts
**Goal**: Async cancellation and configurable timeout limits
**Depends on**: Phase 9
**Requirements**: WSTA-03, WSTA-04
**Success Criteria** (what must be TRUE):
  1. User can cancel running workflow via async cancellation token
  2. Individual tasks and entire workflow have configurable timeout limits
**Plans**: 3 plans

Plans:
- [x] 10-01: CancellationToken integration with parent-child hierarchy
- [x] 10-02: Timeout handling for tasks and workflows
- [ ] 10-03: Cooperative cancellation in async loops

**Status**: 2/3 plans complete (67%)

#### Phase 11: Tool Integration
**Goal**: External tool execution with fallback handlers
**Depends on**: Phase 10
**Requirements**: WTOOL-01, WTOOL-02, WTOOL-04
**Success Criteria** (what must be TRUE):
  1. Workflow can execute shell commands with working directory and environment variables
  2. External tools (magellan, cargo, splice) are registered and callable from workflows
  3. Tool failures trigger fallback handlers for graceful degradation
**Plans**: 3 plans

Plans:
- [ ] 11-01: Shell command execution with tokio::process
- [ ] 11-02: Tool registry with RAII process guards
- [ ] 11-03: Fallback handlers for tool failures

#### Phase 12: Parallel Execution
**Goal**: Fork-join parallelism for independent workflow steps
**Depends on**: Phase 11
**Requirements**: WOBS-01
**Success Criteria** (what must be TRUE):
  1. Independent workflow steps execute in parallel when dependencies allow
**Plans**: 3 plans

Plans:
- [ ] 12-01: Fork-join parallelism with topological sort
- [ ] 12-02: Concurrent state management with dashmap
- [ ] 12-03: Deadlock detection and prevention

## Progress

**Execution Order:**
Phases execute in numeric order: 8 → 9 → 10 → 11 → 12

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Core Runtime & Integration | v0.3 | 2/2 | Complete | 2026-02-22 |
| 2. Reasoning Foundation | v0.3 | 1/1 | Complete | 2026-02-22 |
| 3. Agent Orchestration | v0.3 | 4/4 | Complete | 2026-02-22 |
| 4. Analysis & Quality | v0.3 | 2/2 | Complete | 2026-02-22 |
| 5. Complete Runtime Integration | v0.3 | 2/2 | Complete | 2026-02-22 |
| 6. Complete Test Coverage | v0.3 | 1/1 | Complete | 2026-02-22 |
| 7. Code Cleanup | v0.3 | 1/1 | Complete | 2026-02-22 |
| 8. Workflow Foundation | v0.4 | 0/5 | Not started | - |
| 9. State Management | v0.4 | 0/4 | Not started | - |
| 10. Cancellation & Timeouts | v0.4 | 2/3 | In progress | 2026-02-22 |
| 11. Tool Integration | v0.4 | 0/3 | Planned | - |
| 12. Parallel Execution | v0.4 | 0/3 | Not started | - |

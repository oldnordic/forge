# Roadmap: ForgeKit

## Overview

ForgeKit is a deterministic code intelligence SDK for Rust - "LLVM for AI Code Agents". It combines Magellan (graph operations), LLMGrep (semantic search), Mirage (CFG analysis), and Splice (span-safe editing) into a unified API with multi-step workflow orchestration capabilities.

## Milestones

- ✅ **v0.1 Foundation** — Project setup and initial structure (shipped 2026-02-12)
- ✅ **v0.3 Agent Orchestration & Reasoning** — Phases 1-7 (shipped 2026-02-22)
- ✅ **v0.4 Advanced Agent Multi-step Workflows** — Phases 8-13 (shipped 2026-02-23)
- 🔮 **v0.5** — Future (planned)

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

<details>
<summary>✅ v0.4 Advanced Agent Multi-step Workflows (Phases 8-13) — SHIPPED 2026-02-23</summary>

**Archive:** [.planning/milestones/v0.4-ROADMAP.md](.planning/milestones/v0.4-ROADMAP.md)

**Milestone Goal:** Enable agents to orchestrate complex multi-step workflows with external tool integration, parallel execution, and full state management.

- [x] Phase 8: Workflow Foundation (5/5 plans) — completed 2026-02-22
- [x] Phase 9: State Management (4/4 plans) — completed 2026-02-22
- [x] Phase 10: Cancellation & Timeouts (3/3 plans) — completed 2026-02-22
- [x] Phase 11: Tool Integration (3/3 plans) — completed 2026-02-22
- [x] Phase 12: Parallel Execution (3/3 plans) — completed 2026-02-23
- [x] Phase 13: Task Execution Refactor (3/3 plans) — completed 2026-02-23

**Summary:** DAG-based workflow orchestration with YAML parser, checkpointing/resume, cancellation tokens, timeout handling, tool registry with fallbacks, fork-join parallel execution, concurrent state management, deadlock detection, and real task execution.
</details>

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Core Runtime & Integration | v0.3 | 2/2 | Complete | 2026-02-22 |
| 2. Reasoning Foundation | v0.3 | 1/1 | Complete | 2026-02-22 |
| 3. Agent Orchestration | v0.3 | 4/4 | Complete | 2026-02-22 |
| 4. Analysis & Quality | v0.3 | 2/2 | Complete | 2026-02-22 |
| 5. Complete Runtime Integration | v0.3 | 2/2 | Complete | 2026-02-22 |
| 6. Complete Test Coverage | v0.3 | 1/1 | Complete | 2026-02-22 |
| 7. Code Cleanup | v0.3 | 1/1 | Complete | 2026-02-22 |
| 8. Workflow Foundation | v0.4 | 5/5 | Complete | 2026-02-22 |
| 9. State Management | v0.4 | 4/4 | Complete | 2026-02-22 |
| 10. Cancellation & Timeouts | v0.4 | 3/3 | Complete | 2026-02-22 |
| 11. Tool Integration | v0.4 | 3/3 | Complete | 2026-02-22 |
| 12. Parallel Execution | v0.4 | 3/3 | Complete | 2026-02-23 |
| 13. Task Execution Refactor | v0.4 | 3/3 | Complete | 2026-02-23 |

**Next:** Start planning next milestone with `/gsd:new-milestone`

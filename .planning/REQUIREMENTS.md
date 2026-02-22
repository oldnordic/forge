# Requirements: ForgeKit

**Defined:** 2026-02-22
**Core Value:** Graph-first, deterministic operations — SQLiteGraph database is authoritative ground truth

## v0.4 Requirements

Requirements for Advanced Agent Multi-step Workflows milestone. Each maps to roadmap phases.

### Workflow Definition

- [ ] **WDEF-01**: User can define workflow with multiple steps and dependencies between them
- [ ] **WDEF-02**: Workflow scheduler executes steps in topological order based on dependencies
- [ ] **WDEF-03**: System auto-detects step dependencies from code structure using graph queries
- [ ] **WDEF-04**: Workflow failure triggers rollback of only dependent steps using DAG backward traversal
- [ ] **WDEF-05**: System verifies workflow correctness before execution (detects cycles, missing dependencies)

### State Management

- [ ] **WSTA-01**: Workflow state is checkpointed after each step completion
- [ ] **WSTA-02**: Failed workflow can resume from last checkpoint instead of restarting
- [ ] **WSTA-03**: User can cancel running workflow via async cancellation token
- [ ] **WSTA-04**: Individual tasks and entire workflow have configurable timeout limits
- [ ] **WSTA-05**: External tool side effects use compensation transactions for rollback (Saga pattern)

### Tool Integration

- [ ] **WTOOL-01**: Workflow can execute shell commands with working directory and environment variables
- [ ] **WTOOL-02**: External tools (magellan, cargo, splice) are registered and callable from workflows
- [ ] **WTOOL-03**: Simple workflows can be defined in YAML, complex workflows via Rust API
- [ ] **WTOOL-04**: Tool failures trigger fallback handlers for graceful degradation

### Observability

- [ ] **WOBS-01**: Independent workflow steps execute in parallel when dependencies allow
- [ ] **WOBS-02**: User can inspect workflow state including current step, completed steps, and pending work
- [ ] **WOBS-03**: Workflow execution events are logged to audit trail (reuses AgentLoop AuditLog)
- [ ] **WOBS-04**: Validation checkpoints between steps check confidence scores and trigger rollback if needed

## v2 Requirements

Deferred to future release.

### Multi-Agent Orchestration

- **V2-MA01**: Coordinate multiple specialized agents
- **V2-MA02**: Agent-to-agent communication channels

### Advanced Workflow Features

- **V2-WF01**: Real-time workflow modification
- **V2-WF02**: Dynamic task generation
- **V2-WF03**: Distributed execution

## Out of Scope

| Feature | Reason |
|---------|--------|
| LSP | Use existing LSP servers |
| Web UI | CLI and library only |
| Distributed | Single-machine for v0.4 |
| NL workflows | YAML + Rust API |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| WDEF-01 | Phase 8 | Pending |
| WDEF-02 | Phase 8 | Pending |
| WDEF-03 | Phase 8 | Pending |
| WDEF-04 | Phase 8 | Pending |
| WDEF-05 | Phase 8 | Pending |
| WSTA-01 | Phase 9 | Pending |
| WSTA-02 | Phase 9 | Pending |
| WSTA-03 | Phase 10 | Pending |
| WSTA-04 | Phase 10 | Pending |
| WSTA-05 | Phase 9 | Pending |
| WTOOL-01 | Phase 11 | Pending |
| WTOOL-02 | Phase 11 | Pending |
| WTOOL-03 | Phase 8 | Pending |
| WTOOL-04 | Phase 11 | Pending |
| WOBS-01 | Phase 12 | Pending |
| WOBS-02 | Phase 8 | Pending |
| WOBS-03 | Phase 8 | Pending |
| WOBS-04 | Phase 9 | Pending |

---
*Requirements defined: 2026-02-22*
# Implementation Plan: Chain of Custody

## Grounded Analysis Summary

**DB:** `.magellan/forge.db` — 83 files, 2499 symbols, 3345 calls
**Workspace:** Compiles clean. 50 tests passing on forge-agent.

## Evidence — What Exists

| Symbol | File:Line | Role |
|--------|-----------|------|
| `TaskContext` | task.rs:100-115 | Execution context with `forge`, `audit_log`, `task_id` |
| `TaskResult` | task.rs:79-93 | Enum: Success, Failed, Skipped, WithCompensation |
| `WorkflowTask::execute()` | task.rs:392-399 | Trait method: `&TaskContext -> Result<TaskResult, TaskError>` |
| `WorkflowExecutor::execute_task()` | executor.rs:1087-1176 | Hook point: record_task_started → execute → record_task_completed |
| `AuditLog::record()` | audit.rs:272-276 | Async event recording + persist to `.forge/audit/{tx_id}.json` |
| `AuditEvent` | audit.rs:50-227 | 24 variants (24 after our additions) |
| `PlanGraph` | plan.rs:91-93 | sqlitegraph-backed graph with `PlanNodeKind` + `PlanEdgeKind` |
| `PlanNodeKind` | plan.rs:16-28 | 11 variants |
| `PlanEdgeKind` | plan.rs:50-64 | 13 variants |

**What does NOT exist:**
- No `SubagentRun` type anywhere
- No `ToolCall` type
- No `LogEntry` type
- No `Deliverable` type
- No `ReasoningStep` type
- No chain-of-custody edges (EXECUTED_BY, LOGGED, CALLED, PRODUCED, REASONED)

## Architecture: The Custody Chain

```
Requirement
  └── HAS_REQUIREMENT ──► PlanSection (data: { order: 1 })
       └── DECOMPOSES_INTO ──► Task
            ├── DEPENDS_ON ──► Task (ordering between tasks)
            └── EXECUTED_BY ──► SubagentRun
                 ├── LOGGED ──► LogEntry (data: { level, message, timestamp })
                 ├── CALLED ──► ToolCall (data: { tool, args, result, exit_code, duration_ms })
                 ├── REASONED ──► ReasoningStep (data: { thinking, decision, timestamp })
                 └── PRODUCED ──► Deliverable (data: { file_path, sha256, diff_summary })
```

**Every node gets `created_at: DateTime<Utc>` in its data field.**

### Key Properties

1. **Forward trace**: "For requirement X, show me everything" — walk down via edges
2. **Backward trace**: "This tool call changed auth.rs — why?" — walk up to SubagentRun → Task → PlanSection → Requirement
3. **Timeline reconstruction**: Sort all nodes under a requirement by `created_at`
4. **Gap detection**: Task with no EXECUTED_BY edge = not yet run

### Ordering Between Plan Sections

PlanSection nodes store `{ order: usize }` in their data. The plan graph exposes
`sections_in_order()` which returns sections sorted by order field.

Tasks within a section have DEPENDS_ON edges. The executor respects these
(via existing petgraph DAG).

## Implementation Tasks (dependency-ordered)

### Task A: New PlanNodeKind + PlanEdgeKind variants (plan.rs + audit.rs)

**Adds to `PlanNodeKind`:**
- PlanSection (ordered section of a plan)
- SubagentRun (one execution of a subagent on a task)
- LogEntry (timestamped log from subagent)
- ToolCall (tool invocation with args/result)
- ReasoningStep (LLM thinking/decision captured mid-run)
- Deliverable (file/artifact produced, with SHA)

**Adds to `PlanEdgeKind`:**
- ExecutedBy (Task → SubagentRun)
- Logged (SubagentRun → LogEntry)
- Called (SubagentRun → ToolCall)
- Reasoned (SubagentRun → ReasoningStep)
- Produced (SubagentRun → Deliverable)
- AddressesIn (PlanSection → Requirement — which req this section addresses)

**Adds to `AuditEvent`:**
- SubagentStarted { timestamp, workflow_id, task_id, run_id, agent_name }
- SubagentCompleted { timestamp, workflow_id, task_id, run_id, duration_ms, status }

**Files:** plan.rs, audit.rs
**Tests:** 6 new tests covering new variants and serialization

### Task B: Custody types (plan.rs — new structs)

New serializable structs stored as node `data`:

```rust
struct SubagentRunData {
    run_id: String,          // UUID
    agent_name: String,
    started_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    status: SubagentStatus,  // Running, Completed, Failed, Cancelled
    input_prompt: String,
    output_summary: Option<String>,
}

enum SubagentStatus { Running, Completed, Failed, Cancelled }

struct LogEntryData {
    level: LogLevel,         // Info, Warn, Error, Debug
    message: String,
    timestamp: DateTime<Utc>,
}

struct ToolCallData {
    tool: String,
    args: serde_json::Value,
    result: Option<serde_json::Value>,
    exit_code: i32,
    duration_ms: u64,
    timestamp: DateTime<Utc>,
}

struct ReasoningStepData {
    thinking: String,
    decision: String,
    timestamp: DateTime<Utc>,
}

struct DeliverableData {
    file_path: String,
    sha256: String,
    diff_summary: Option<String>,
    timestamp: DateTime<Utc>,
}

struct PlanSectionData {
    order: usize,
    title: String,
    description: String,
}
```

**Files:** plan.rs
**Tests:** Round-trip serialization for each struct, deserialization edge cases

### Task C: PlanGraph custody methods (plan.rs)

New methods on PlanGraph:

```rust
// Section management (ordered)
fn add_section(&mut self, plan_id: i64, requirement_id: i64, data: PlanSectionData) -> Result<i64>;
fn sections_in_order(&self, plan_id: i64) -> Result<Vec<(i64, PlanSectionData)>>;

// Subagent custody
fn begin_subagent_run(&mut self, task_id: i64, data: SubagentRunData) -> Result<i64>;
fn complete_subagent_run(&mut self, run_id: i64, status: SubagentStatus, summary: &str) -> Result<()>;

// Recording within a run
fn record_log(&mut self, run_id: i64, data: LogEntryData) -> Result<i64>;
fn record_tool_call(&mut self, run_id: i64, data: ToolCallData) -> Result<i64>;
fn record_reasoning(&mut self, run_id: i64, data: ReasoningStepData) -> Result<i64>;
fn record_deliverable(&mut self, run_id: i64, data: DeliverableData) -> Result<i64>;

// Traversal queries
fn trace_forward(&self, requirement_id: i64) -> Result<CustodyChain>;  // Req → ... → Deliverables
fn trace_backward(&self, node_id: i64) -> Result<Vec<CustodyNode>>;    // Any node → Requirement
fn timeline(&self, root_id: i64) -> Result<Vec<CustodyNode>>;          // All nodes sorted by created_at
fn find_gaps(&self, plan_id: i64) -> Result<Vec<i64>>;                  // Tasks with no EXECUTED_BY

// Custody types for query results
struct CustodyChain { requirement_id: i64, sections: Vec<CustodySection>, }
struct CustodySection { section: PlanSectionData, tasks: Vec<CustodyTask>, }
struct CustodyTask { task_id: i64, runs: Vec<CustodyRun>, }
struct CustodyRun { run: SubagentRunData, logs: Vec<LogEntryData>, calls: Vec<ToolCallData>, reasoning: Vec<ReasoningStepData>, deliverables: Vec<DeliverableData>, }
struct CustodyNode { id: i64, kind: PlanNodeKind, data: serde_json::Value, timestamp: Option<DateTime<Utc>>, }
```

**Files:** plan.rs
**Tests:**
- test_add_section_creates_ordered_nodes
- test_begin_and_complete_subagent_run
- test_record_log_tool_call_reasoning_deliverable
- test_trace_forward_returns_full_chain
- test_trace_backward_from_tool_call_to_requirement
- test_timeline_sorted_by_created_at
- test_find_gaps_returns_tasks_without_runs
- test_full_custody_chain_roundtrip (requirement → section → task → run → tool call → deliverable)

## Scope Decisions

**In this slice:**
- All types and graph operations above
- AuditEvent variants for SubagentStarted/Completed
- All traversal queries

**Deferred (next slices):**
- Wiring into WorkflowExecutor (the `execute_task` hook point)
- Wiring into TaskContext (automatic custody recording)
- TUI dashboard rendering of custody chain
- Real subagent integration (actually spawning and recording)

## Estimated Size

- Task A: ~80 lines changes (enum variants + audit variants)
- Task B: ~100 lines (struct definitions)
- Task C: ~400 lines (methods + traversal + query structs)
- Tests: ~300 lines
- **Total: ~880 lines across 2 files** (plan.rs, audit.rs)

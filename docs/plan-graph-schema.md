# Forge Plan Graph Schema

## Grounded Analysis (from magellan DB)

**DB:** `.magellan/forge.db` — 79 files, 2431 symbols, 1644 references

**What already exists in the codebase:**

| Module | Key Types | File | Status |
|--------|-----------|------|--------|
| Workflow DAG | `Workflow`, `TaskNode`, `TaskId` | `forge_agent/src/workflow/dag.rs` | Implemented |
| Task system | `TaskContext`, `TaskResult`, `Dependency` (Hard/Soft) | `forge_agent/src/workflow/task.rs` | Implemented |
| Workflow builder | `WorkflowBuilder` (fluent API) | `forge_agent/src/workflow/builder.rs` | Implemented |
| Executor | `WorkflowExecutor`, `WorkflowResult` | `forge_agent/src/workflow/executor.rs` | Implemented |
| State machine | `WorkflowState` | `forge_agent/src/workflow/state.rs` | Implemented |
| Checkpoints | `WorkflowCheckpoint`, `WorkflowCheckpointService` | `forge_agent/src/workflow/checkpoint.rs` | Implemented |
| Cancellation | `CancellationToken`, `CancellationTokenSource` | `forge_agent/src/workflow/cancellation.rs` | Implemented |
| Timeouts | `TaskTimeout`, `WorkflowTimeout`, `TimeoutConfig` | `forge_agent/src/workflow/timeout.rs` | Implemented |
| YAML config | `YamlWorkflow`, `YamlTask`, `YamlTaskParams` | `forge_agent/src/workflow/yaml.rs` | Implemented |
| Agent loop | `AgentLoop`, `AgentPhase` (6 phases) | `forge_agent/src/loop.rs` | Implemented |
| Agent phases | Observe → Constrain → Plan → Mutate → Verify → Commit | `forge_agent/src/loop.rs:19-39` | Implemented |
| Audit trail | `AuditEvent` (20 variants), `AuditLog` | `forge_agent/src/audit.rs` | Implemented |
| Policy | `PolicyValidator` | `forge_agent/src/policy.rs` | Implemented |
| Reasoning | `HypothesisBoard`, `Evidence`, `KnowledgeGapAnalyzer` | `forge-reasoning/src/` | Implemented |
| Dead code | `DeadCodeAnalyzer` | `forge_core/src/analysis/dead_code.rs` | Implemented |
| Complexity | `ComplexityMetrics`, `RiskLevel` | `forge_core/src/analysis/complexity.rs` | Implemented |
| CFG | `CfgModule`, `FunctionCfg`, `PathBuilder` | `forge_core/src/cfg/mod.rs` | Implemented |
| Metrics | `RuntimeMetrics`, `MetricKind`, `MetricsSummary` | `forge_runtime/src/metrics.rs` | Implemented |

**What does NOT exist yet:**
- Quality gate definitions (no `Gate`, `GateResult`, `SemgrepRule`, `SemgrepFinding`)
- Plan-level requirements (no `Requirement`, `Plan`, `Decision`, `Constraint` nodes)
- User approval/rejection (no `Approval`, `Rejection` types)
- User-written tests as graph nodes (no `UserTest`)
- LLM turn/session tracking (no `Turn`, `Session` types)
- Edit tracking with SHA before/after (no `Edit` node with diffs)
- Tool call recording (no `ToolCall` with structured input/output)
- Semgrep integration layer (rules exist but no programmatic gate runner)

---

## Overview

The forge plan is stored as a graph in sqlitegraph. Every artifact — requirements,
tasks, edits, gates, decisions, test results — is a node. Every relationship is
an edge. The graph is the audit trail.

Before any code is written, the plan exists as a subgraph that the user can inspect,
modify, and approve. During execution, nodes and edges are added. After delivery,
the full graph is the proof of what happened and why.

**This schema extends the existing forge_agent types.** The `TaskId`, `Workflow`,
`AuditEvent`, and `AgentPhase` types already exist — the plan graph adds the layer
above (requirements, decisions, gates) and below (edits, tool calls, findings).

---

## Node Kinds

### Planning Layer (user-visible, editable before approval)

| Kind | Description | Key Properties |
|------|-------------|----------------|
| `Requirement` | Business requirement from user | `text`, `priority`, `source` (user/llm) |
| `Plan` | Top-level plan container | `title`, `status` (draft/approved/executing/completed) |
| `Task` | Atomic unit of work | `description`, `status` (pending/in_progress/done/failed) |
| `Subtask` | Decomposition of a Task | `description`, `status` |
| `Decision` | A choice point in the plan | `question`, `chosen`, `alternatives[]`, `rationale` |
| `Constraint` | Hard limit on scope/behavior | `text`, `enforced_by` (gate name) |

### Execution Layer (added during agent runs)

| Kind | Description | Key Properties |
|------|-------------|----------------|
| `Agent` | An LLM agent instance | `model`, `provider`, `role` |
| `Edit` | A file change | `path`, `diff`, `sha_before`, `sha_after` |
| `ToolCall` | A tool invocation | `tool`, `input`, `output`, `exit_code`, `duration_ms` |
| `TestResult` | Outcome of a test run | `framework`, `total`, `passed`, `failed`, `output` |

### Quality Gate Layer (automated checks)

| Kind | Description | Key Properties |
|------|-------------|----------------|
| `Gate` | A quality gate definition | `gate_type`, `language`, `tool`, `config`, `strict` |
| `GateResult` | Outcome of running a gate | `passed`, `errors`, `warnings`, `structured_output` |
| `SemgrepRule` | A semgrep rule or ruleset | `rule_id`, `severity`, `config_path` |
| `SemgrepFinding` | A finding from semgrep scan | `check_id`, `file`, `line`, `message`, `severity` |
| `Benchmark` | A performance benchmark | `name`, `baseline_mean_ns`, `current_mean_ns`, `regression_pct` |
| `CoverageReport` | Test coverage summary | `line_pct`, `branch_pct`, `files_checked` |

### Audit Layer (provenance, traceability)

| Kind | Description | Key Properties |
|------|-------------|----------------|
| `Turn` | One LLM interaction | `model`, `prompt_hash`, `tokens_in`, `tokens_out`, `duration_ms` |
| `Session` | A forge execution session | `started_at`, `ended_at`, `total_turns`, `total_edits` |
| `Approval` | User approval event | `approved_by`, `at`, `note` |
| `Rejection` | User rejection event | `rejected_by`, `at`, `reason` |
| `UserTest` | Test written by the human | `path`, `description`, `framework` |

---

## Edge Types

### Planning edges

| Type | From → To | Meaning |
|------|-----------|---------|
| `HAS_REQUIREMENT` | Plan → Requirement | Plan addresses this requirement |
| `DECOMPOSES_INTO` | Task → Subtask | Task broken into subtasks |
| `DEPENDS_ON` | Task → Task | Execution ordering |
| `BLOCKS` | Task → Task | Inverse of DEPENDS_ON |
| `CHOSEN_FOR` | Decision → Task | The chosen alternative |
| `CONSTRAINED_BY` | Task → Constraint | Task must satisfy this constraint |
| `IMPLEMENTS` | Task → Requirement | Task satisfies this requirement |
| `ASSIGNED_TO` | Task → Agent | Agent responsible for execution |

### Execution edges

| Type | From → To | Meaning |
|------|-----------|---------|
| `PRODUCED` | Agent → Edit | Agent made this edit |
| `CALLED` | Agent → ToolCall | Agent invoked this tool |
| `TESTED_BY` | Task → TestResult | Task verified by this test run |
| `MODIFIED_BY` | Edit → ToolCall | Edit created by this tool call |
| `PART_OF` | Turn → Session | Turn belongs to session |
| `TRIGGERED_BY` | Turn → ToolCall | Turn produced this tool call |

### Quality gate edges

| Type | From → To | Meaning |
|------|-----------|---------|
| `VALIDATED_BY` | Task → Gate | Task must pass this gate |
| `RAN_ON` | GateResult → Edit | Gate checked this edit |
| `GATE_DEF` | GateResult → Gate | Result is for this gate definition |
| `FOUND_IN` | SemgrepFinding → Edit | Finding located in this edit |
| `DETECTED_BY` | SemgrepFinding → SemgrepRule | Finding matched this rule |
| `REGRESSED_BY` | Benchmark → Edit | Benchmark regressed due to this edit |
| `COVERED_BY` | Task → CoverageReport | Task's code covered by this report |
| `CHECKS` | Gate → SemgrepRule | Gate includes this semgrep rule |

### Approval edges

| Type | From → To | Meaning |
|------|-----------|---------|
| `APPROVED` | Approval → Plan | Plan was approved |
| `APPROVED` | Approval → Task | Task was approved |
| `REJECTED` | Rejection → Task | Task was rejected |
| `WRITTEN_BY` | UserTest → Agent | Human wrote this test (optionally with agent help) |
| `VERIFIES` | UserTest → Requirement | Test validates this requirement |

---

## Gate Definitions (per language)

### Universal gates (all projects)

```yaml
gates:
  - name: format
    tool: auto-detect    # rustfmt, ruff format, prettier, gofmt
    priority: 1          # runs first (fastest)
    exit_code: 0         # pass
    on_fail: fix         # auto-fix if possible, then re-check

  - name: semgrep
    tool: semgrep
    priority: 3
    config:              # layered config
      - auto             # language-auto-detected rules
      - p/security-audit
      - p/owasp-top-ten
      - .semgrep/        # project-local custom rules
    output: json
    on_fail: block       # block the task, report findings

  - name: test
    tool: auto-detect    # cargo test, pytest, jest, go test
    priority: 5
    on_fail: block
```

### Rust gates

```yaml
gates:
  - name: clippy
    tool: cargo clippy --all-targets -- -D warnings
    priority: 2
    
  - name: miri
    tool: cargo miri test
    priority: 4
    scope: unsafe-only   # only runs if edit touches unsafe blocks
    
  - name: audit
    tool: cargo audit
    priority: 4
    
  - name: deny
    tool: cargo deny check
    priority: 4
```

### Python gates

```yaml
gates:
  - name: ruff
    tool: ruff check .
    priority: 2
    
  - name: mypy
    tool: mypy src/
    priority: 3
    
  - name: bandit
    tool: bandit -r src/ -f json
    priority: 3
    
  - name: pip-audit
    tool: pip-audit --format json
    priority: 4
```

### TypeScript gates

```yaml
gates:
  - name: eslint
    tool: npx eslint . --format json
    priority: 2
    
  - name: tsc
    tool: npx tsc --noEmit --strict
    priority: 3
    
  - name: npm-audit
    tool: npm audit --json
    priority: 4
```

### Go gates

```yaml
gates:
  - name: golangci-lint
    tool: golangci-lint run ./... --out-format=json
    priority: 2
    
  - name: govulncheck
    tool: govulncheck ./... --json
    priority: 4
```

---

## Semgrep Integration

### Why semgrep as a first-class gate

1. **Multi-language** — one tool covers Rust, Python, TypeScript, Go, and more
2. **Custom rules** — projects can define their own (forge already has `.semgrep/rules/`)
3. **JSON output** — machine-readable, maps directly to `SemgrepFinding` nodes
4. **Fast** — pattern-matching, not full analysis, runs in seconds
5. **OWASP coverage** — `p/owasp-top-ten` ruleset maps to OWASP LLM Top 10

### Semgrep gate schema

```
Gate(semgrep)
  ├── uses: SemgrepRule(auto)           # auto-detected language rules
  ├── uses: SemgrepRule(p/security-audit)
  ├── uses: SemgrepRule(p/owasp-top-ten)
  ├── uses: SemgrepRule(.semgrep/)      # project-local custom rules
  └── produces: SemgrepFinding[]        # zero or more findings

Each SemgrepFinding:
  ├── FOUND_IN → Edit                   # where the finding is
  ├── DETECTED_BY → SemgrepRule         # which rule matched
  └── properties: {check_id, file, line, message, severity}
```

### Semgrep rulesets for forge

| Ruleset | What it catches | Why it matters for LLM code |
|---------|----------------|-----------------------------|
| `auto` | Language-specific best practices | LLMs reproduce common antipatterns |
| `p/security-audit` | Injection, auth bypass, secrets | LLMs generate SQL with string concat |
| `p/owasp-top-ten` | OWASP Top 10 patterns | A03: Injection, A05: Misconfig |
| `p/secret-detection` | Hardcoded API keys, tokens | LLMs copy-paste example code with keys |
| `.semgrep/` | Project-specific rules | Unwrap in prod, TODO macros (forge already has these) |

### Custom rules for LLM-generated code (to add)

```yaml
# .semgrep/rules/llm-patterns.yml
rules:
  - id: llm-string-concat-sql
    patterns:
      - pattern: |
          $DB.execute($QUERY + ...)
      - pattern-not: |
          $DB.execute($PARAMS)
    message: "String concatenation in SQL query. Use parameterized queries."
    severity: ERROR
    languages: [python, rust]

  - id: llm-hardcoded-path
    pattern: |
      "/home/..." 
    message: "Hardcoded absolute path. Use config or relative paths."
    severity: WARNING

  - id: llm-bare-except
    pattern: |
      except: ...
    message: "Bare except silently swallows errors. Use specific exception types."
    severity: ERROR
    languages: [python]

  - id: llm-clone-escape
    patterns:
      - pattern: $EXPR.clone()
      - pattern-not-inside: |
          fn $FUNC(..., $ARG: &$T) { ... }
    message: ".clone() used outside of function argument context. Consider borrowing."
    severity: WARNING
    languages: [rust]
```

---

## Example: Plan Graph for "Fix Cypher injection in navigator"

```
Session(forge-2026-05-18)
  │
  ├── HAS_REQUIREMENT ← Requirement("Prevent Cypher injection in navigator.py")
  │
  ├── Plan("Fix Cypher injection")
  │     ├── DECOMPOSES_INTO → Task("Escape user input in Cypher queries")
  │     │     ├── VALIDATED_BY → Gate(clippy)         # not applicable (Python)
  │     │     ├── VALIDATED_BY → Gate(ruff)
  │     │     ├── VALIDATED_BY → Gate(mypy)
  │     │     ├── VALIDATED_BY → Gate(semgrep)
  │     │     │     ├── CHECKS → SemgrepRule(auto)
  │     │     │     ├── CHECKS → SemgrepRule(p/security-audit)
  │     │     │     └── CHECKS → SemgrepRule(.semgrep/)
  │     │     ├── VALIDATED_BY → Gate(pytest)
  │     │     ├── VALIDATED_BY → Gate(bandit)
  │     │     │
  │     │     └── ASSIGNED_TO → Agent(model="glm-5.1", role="coder")
  │     │           ├── PRODUCED → Edit(navigator.py, diff="+_cypher_escape")
  │     │           ├── PRODUCED → Edit(test_navigator_safety.py)
  │     │           ├── CALLED → ToolCall("ruff check")
  │     │           └── CALLED → ToolCall("pytest")
  │     │                 └── PRODUCED → TestResult(9 passed, 0 failed)
  │     │
  │     ├── DECOMPOSES_INTO → Task("Reject semicolons in Cypher input")
  │     │     ├── IMPLEMENTS → Requirement("Prevent Cypher injection")
  │     │     └── CONSTRAINED_BY → Constraint("No false positives on normal text")
  │     │
  │     └── APPROVED ← Approval(by="user", note="Looks good, proceed")
  │
  └── UserTest("test_semicolon_rejection")
        └── VERIFIES → Requirement("Prevent Cypher injection")
```

### After execution — gate results added:

```
Gate(ruff)
  └── RAN_ON → GateResult(passed, 0 errors)

Gate(mypy)
  └── RAN_ON → GateResult(passed, 0 errors)

Gate(semgrep)
  ├── RAN_ON → GateResult(passed, 0 findings)
  └── (no SemgrepFinding nodes — clean scan)

Gate(pytest)
  └── RAN_ON → GateResult(passed, 9 total, 9 passed)

Gate(bandit)
  └── RAN_ON → GateResult(passed, 0 issues)
```

### If semgrep had found something:

```
Gate(semgrep)
  └── RAN_ON → GateResult(FAILED, 1 finding)
        └── DETECTED_BY → SemgrepFinding(
              check_id="llm-string-concat-sql",
              file="navigator.py", line=42,
              message="String concatenation in SQL query",
              severity=ERROR
            )
              └── FOUND_IN → Edit(navigator.py)
```

---

## Cypher Queries for the Dashboard

```cypher
-- Show the full plan for a session
MATCH (s:Session)-[:HAS_REQUIREMENT]->(r:Requirement)
RETURN s, r

-- Show all tasks and their status
MATCH (t:Task)
RETURN t.name, t.status

-- Show all gate results for a task
MATCH (t:Task)-[:VALIDATED_BY]->(g:Gate)-[:RAN_ON]->(gr:GateResult)
RETURN t.name, g.name, gr.passed

-- Show all semgrep findings
MATCH (f:SemgrepFinding)-[:FOUND_IN]->(e:Edit)
RETURN f.check_id, f.file, f.line, f.severity

-- Show the full audit trail for an edit
MATCH (a:Agent)-[:PRODUCED]->(e:Edit)<-[:FOUND_IN]-(f:SemgrepFinding)
RETURN a.model, e.path, f.check_id, f.message

-- Show user-written tests and what they verify
MATCH (ut:UserTest)-[:VERIFIES]->(r:Requirement)
RETURN ut.path, ut.description, r.text

-- Show decisions and alternatives
MATCH (d:Decision)
RETURN d.question, d.chosen, d.alternatives

-- Show the approval chain
MATCH (a:Approval)-[:APPROVED]->(p:Plan)-[:DECOMPOSES_INTO]->(t:Task)
RETURN a.approved_by, a.at, t.name

-- Gate pass rate across all tasks
MATCH (t:Task)-[:VALIDATED_BY]->()<-[:GATE_DEF]-(gr:GateResult)
RETURN gr.passed, count(*)
```

---

## Gate Execution Order (short-circuit)

```
1. format   ─── fail → auto-fix, re-run ─── fail again → BLOCK
2. lint     ─── fail → report, agent fixes → re-run ── fail → BLOCK
3. type     ─── fail → report, agent fixes → re-run ── fail → BLOCK
4. semgrep  ─── fail → report findings → agent fixes → re-run → fail → BLOCK
5. test     ─── fail → report failures → agent fixes → re-run → fail → BLOCK
6. bench    ─── fail → report regression → WARN (not block)
```

Each gate produces a `GateResult` node linked to the `Gate` definition and the
`Edit` it checked. The dashboard renders these as a checklist. Green checkmarks
for passes, red X for failures with expandable details.

---

## Priority Ordering for Benchmarks

For measuring forge itself (not for the quality gates above):

| Benchmark | What it measures | Relevance |
|-----------|-----------------|-----------|
| SWE-bench | Can agent solve real GitHub issues | Gold standard for agent eval |
| EvalPlus | Function-level correctness with 80x more tests | Exposes false positives |
| Aider leaderboard | Real-world coding workflow accuracy | Practical comparison |
| LiveCodeBench | Contamination-free problems | Avoids training data overlap |

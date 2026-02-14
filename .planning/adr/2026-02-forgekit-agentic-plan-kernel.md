# ADR: ForgeKit Agentic Plan Kernel (C Mode)

**Date**: 2026-02-12
**Status**: Accepted
**Context**: Defining the deterministic plan kernel architecture for multi-agent coordination using Native-V3 graph as source of truth

---

## Usage Modes (Menu Approach)

ForgeKit provides multiple usage modes. Users (or agent frameworks) choose based on their needs:

### 1. Tool Mode (Direct API)

**For**: Simple refactors, scripts, CI/CD pipelines, direct tool usage

```rust
// Direct API usage - no Plan Kernel
use forge_core::{Forge, GraphModule, SearchModule};

let forge = Forge::open("./repo").await?;
let symbols = forge.graph().find_symbol("main")?;
let results = forge.search().pattern("async fn").execute()?;
```

**Characteristics**:
- Direct calls to graph/search/cfg/edit modules
- No planning overhead
- Suitable for single-file operations
- Works with any toolchain

### 2. Agent Mode (Plan Kernel C Mode)

**For**: Multi-step operations requiring coordination, handoffs, parallel agents

```rust
// Agent Mode - Plan Kernel coordinates work
use forge_runtime::{PlanKernel, Agent};

let kernel = PlanKernel::new("./repo").await?;
let plan_id = kernel.plan.create("Refactor to async", constraints).await?;
kernel.step.execute(step_id).await?;
```

**Characteristics**:
- Plan Graph stores all operations (append-only)
- Pub/Sub coordinates multiple agents
- Handoff protocol for token budgets
- File lease system prevents conflicts

### 3. Hybrid Mode

**For**: Complex workflows mixing both approaches

```rust
// Mix direct API and Plan Kernel as needed
let forge = Forge::open("./repo").await?;
let kernel = PlanKernel::new(&forge).await?;  // Optional!

// Use direct API for simple queries
let symbols = forge.graph().find_symbol("main")?;

// Use Plan Kernel for complex multi-file refactors
if needs_planning {
    let plan_id = kernel.plan.create("Complex task", constraints).await?;
    kernel.step.execute(plan_id).await?;
}
```

**Characteristics**:
- Flexibility to choose per-operation
- Direct API for fast operations
- Plan Kernel for complex workflows

### Mode Selection

| Use Case | Recommended Mode | Reason |
|-----------|-----------------|--------|
| Single-file refactor | Tool Mode | No planning overhead |
| Multi-file project | Agent Mode | Coordination needed |
| CI/CD pipeline | Tool Mode | Deterministic, reproducible |
| One-shot query | Tool Mode | Fast, direct |
| Multi-agent swarm | Agent Mode | Handoff, scaling |
| Custom orchestrator | Agent Mode | Framework integration |

**Key Point**: Users choose their mode. ForgeKit is the **library**, not the framework.

---

## Important Clarification

**ForgeKit is a standalone SDK/library. It is NOT tied to OdinCode.**

ForgeKit can be used independently by:
- CI/CD pipelines (direct tool integration)
- Other agent frameworks (LangGraph, LangChain, custom orchestrators)
- Direct tool usage (magellan, llmgrep, mirage, splice APIs)
- OdinCode (one of many potential consumers)

**Users choose how to use ForgeKit:**
1. **Tool mode** — Use magellan/llmgrep/mirage/splice APIs directly
2. **Agent mode** — Use the Plan Kernel for coordinated multi-agent workflows
3. **Hybrid mode** — Mix direct tool calls with agent orchestration

**OdinCode is ONE consumer of ForgeKit**, not the only one. The Plan Kernel enables ANY agent framework to use ForgeKit deterministically.

---

## Problem Statement

AI agents need a deterministic, queryable planning and execution system. Current approaches use ephemeral context and opaque decision-making. ForgeKit enables:

1. **Immutable Plan Graph** — All plans, steps, and artifacts stored as graph nodes/edges
2. **Deterministic Scoring** — Objective metrics only, no subjective LLM evaluation
3. **Pub/Sub Coordination** — Multiple agents coordinate via events, not context stuffing
4. **Context Handoff Protocol** — Structured handoff at token budget boundaries
5. **Lease-Based Safety** — Prevent concurrent file modification conflicts

---

## Decision

**ForgeKit will implement an Agentic Plan Kernel using Native-V3 as the authoritative plan graph.**

### 1. Plan Graph (Native-V3)

All planning state is stored as an immutable, append-only graph structure.

**Nodes:**

```rust
// Plan node
pub struct Plan {
    pub id: PlanId,
    pub goal: String,
    pub constraints: Vec<Constraint>,
    pub success_criteria: Vec<Criterion>,
    pub created_at: Timestamp,
}

// Step node
pub struct Step {
    pub id: StepId,
    pub plan_id: PlanId,
    pub order: u32,
    pub scope: StepScope,
    pub status: StepStatus,
}

// Artifact node (file, symbol, patch, test)
pub struct Artifact {
    pub id: ArtifactId,
    pub artifact_type: ArtifactType,  // File | Symbol | Patch | Test
    pub hash: ContentHash,
}

// Risk node
pub struct Risk {
    pub id: RiskId,
    pub description: String,
    pub severity: Severity,
}
```

**Edges:**

```
PLAN_HAS_STEP       (Plan -> Step)
STEP_DEPENDS_ON    (Step -> Step)
STEP_TOUCHES_FILE   (Step -> Artifact[file])
STEP_PRODUCES_PATCH (Step -> Artifact[patch])
STEP_BLOCKED_BY     (Step -> Risk)
PLAN_SUPERSEDES     (Plan -> Plan)
```

**Key Property:** Append-only. Nothing is ever overwritten. Plan changes create new nodes with `PLAN_SUPERSEDES` edges.

---

### 2. Deterministic Scoreboard (Per Step)

Every step has an objective score computed from deterministic metrics. No LLM "judgement."

```rust
pub struct StepScore {
    // Compile verification
    pub compile_ok: bool,

    // Test results
    pub tests_passed: u32,
    pub tests_total: u32,

    // Quality metrics
    pub warnings_delta: i32,   // Change from baseline
    pub clippy_delta: i32,

    // Impact metrics
    pub diff_size_loc: u32,

    // Performance
    pub time_cost_ms: u64,

    // Safety
    pub rollback_count: u32,
    pub regression_hits: u32,
}

impl StepScore {
    pub fn compute(&self) -> Score {
        // Deterministic formula
        // - Higher test pass ratio → better
        // - Lower diff size → better (for targeted changes)
        // - Zero regressions → required
        // - Compile OK → required
    }
}
```

**No subjective evaluation.** All scores are derived from observable metrics.

---

### 3. Pub/Sub Event Bus (Native-V3)

Agents communicate via events published to the graph. No direct context passing.

**Events:**

```rust
pub enum PlanEvent {
    PLAN_CREATED(PlanId),
    STEP_STARTED(StepId),
    PATCH_PROPOSED(StepId, PatchSet),
    PATCH_APPLIED(StepId, PatchSet),
    VERIFY_OK(StepId, VerifyResult),
    VERIFY_FAILED(StepId, Vec<Failure>),
    STEP_COMPLETED(StepId, StepScore),
    PLAN_RESCOPED(PlanId, RescopeReason),
    HANDOFF_EMITTED(HandoffSummary),
}
```

**Lease System:**

```rust
pub struct FileLease {
    pub file_path: PathBuf,
    pub holder: AgentId,
    pub acquired_at: Timestamp,
    pub expires_at: Timestamp,
}

// Lease rules:
// - Agent must acquire lease before editing file
// - Lease expires after timeout or step completion
// - Conflicting lease requests block or retry
// - All file modifications require active lease
```

**Subscription Model:**

```rust
pub trait Agent {
    fn role(&self) -> AgentRole;
    fn on_event(&mut self, event: PlanEvent);
}

pub enum AgentRole {
    Planner,    // Creates and modifies plans
    Worker,     // Executes individual steps
    Auditor,     // Independent verification
}
```

---

### 4. Agent Roles

#### Planner Agent

**Responsibility:** Create and reschedule plans based on graph state.

```rust
impl PlannerAgent {
    fn create_plan(&mut self, goal: &str, constraints: &[Constraint]) -> PlanId {
        // Query PlanGraph for existing context
        // Generate Step nodes with dependencies
        // Identify Risk nodes
        // Write to graph (append-only)
        // Emit PLAN_CREATED event
    }

    fn rescope(&mut self, plan_id: PlanId, reason: RescopeReason) {
        // On VERIFY_FAILED or PLAN_RESCOPED event
        // Create new Plan node with same goal
        // Add PLAN_SUPERSEDES edge
        // Regenerate steps
        // Emit PLAN_RESCOPED
    }
}
```

#### Worker Agent

**Responsibility:** Execute a single step and report results.

```rust
impl WorkerAgent {
    fn execute_step(&mut self, step_id: StepId) -> StepScore {
        // Acquire file leases for STEP_TOUCHES_FILE artifacts
        // Generate PatchSet
        // Apply changes via Splice
        // Run verification pipeline
        // Release leases
        // Update Scoreboard
        // Emit STEP_COMPLETED
    }
}
```

#### Auditor Agent

**Responsibility:** Independent verification of completed steps.

```rust
impl AuditorAgent {
    fn audit_step(&mut self, step_id: StepId) -> AuditResult {
        // Read PatchSet from graph
        // Re-run compile check
        // Re-run tests
        // Verify graph invariants
        // Emit VERIFY_OK or VERIFY_FAILED
    }
}
```

---

### 5. Context Budget Handoff Protocol

When agent context reaches 100k/128k tokens, structured handoff is **mandatory**.

**HANDOFF_SUMMARY Structure:**

```rust
pub struct HandoffSummary {
    pub plan_id: PlanId,
    pub current_step_id: StepId,

    // What was touched (minimal state)
    pub touched_files: Vec<(PathBuf, ContentHash)>,

    // Verification state
    pub verification_results: Vec<VerifyResult>,

    // What went wrong (if anything)
    pub failures: Vec<Failure>,
    pub logs: Vec<LogEntry>,

    // Next agent doesn't need full context
    pub next_3_actions: Vec<Action>,

    // Precise queries to resume
    pub minimal_required_queries: Vec<GraphQuery>,
}
```

**Next Agent Load:**

```rust
impl HandoffSummary {
    pub fn load_next_agent(&self) -> AgentContext {
        AgentContext {
            // Load ONLY the summary
            summary: self.clone(),

            // Run minimal queries to get needed state
            graph_state: run_queries(&self.minimal_required_queries),

            // NO full file injection
            // NO chat history injection
            // NO "here's everything" dump
        }
    }
}
```

**Handoff Trigger:** Agent monitors token count during execution. At 100k tokens:
1. Pause current work
2. Construct HANDOFF_SUMMARY
3. Emit HANDOFF_EMITTED event
4. Terminate

**Next Agent:**
1. Receives HANDOFF_SUMMARY as only context
2. Runs `minimal_required_queries` against PlanGraph
3. Resumes from `next_3_actions`

---

### 6. Forge Runtime API Surface

The plan kernel exposes a minimal, deterministic API.

```rust
// Planning
forge.plan.create(goal: &str, constraints: Vec<Constraint>) -> PlanId;
forge.plan.next_steps(plan_id: PlanId) -> Vec<StepId>;
forge.plan.rescope(plan_id: PlanId, reason: RescopeReason) -> PlanId;

// Execution
forge.step.execute(step_id: StepId) -> Result<PatchSet>;
forge.step.score(step_id: StepId) -> StepScore;

// Handoff
forge.handoff.emit(step_id: StepId, budget_state: TokenBudget) -> HandoffId;

// Query
forge.graph.query(query: GraphQuery) -> QueryResult;
forge.graph.file_lease(path: &Path) -> Option<Lease>;
```

---

### 7. Safety Rules

**Immutable Proofs**

```
Plans can change.
Steps can be rescheduled.
Artifacts can be superseded.

BUT:

Proofs cannot change.
Events cannot change.
Scores cannot change.
```

All patches, metrics, and events are immutable nodes with immutable edges.
**Deterministic replay must always be possible** given the same graph state.

**Replay Property:**

```rust
pub struct PlanProof {
    pub plan_id: PlanId,
    pub patches: Vec<PatchSet>,      // Immutable
    pub events: Vec<PlanEvent>,      // Immutable
    pub scores: Vec<StepScore>,      // Immutable
    pub handoffs: Vec<HandoffSummary>, // Immutable
}

// Given same PlanGraph state, replay produces:
// - Identical patches
// - Identical event sequence
// - Identical scores
// - Deterministically verifiable
```

---

## Rationale

### Why Optional Plan Kernel?

**The Plan Kernel is an OPTIONAL, OPT-IN subsystem.**

ForgeKit provides two usage modes:

| Mode | Description | User Chooses |
|-------|-------------|--------------|
| **Tool Mode** | Direct API calls to graph/search/cfg/edit modules | For simple scripts, CI/CD, direct tools |
| **Agent Mode** | Plan Kernel for coordinated multi-agent workflows | For complex tasks requiring planning, handoffs |
| **Hybrid** | Mix both approaches as needed | User decides per use case |

**Like a menu, not a mandate:**
- Use Tool Mode for simple refactors, scripts, direct queries
- Use Agent Mode for multi-step operations requiring planning
- Switch between modes freely
- No enforcement of agent framework

**ForgeKit works standalone** — Users can:
- Use LangGraph with ForgeKit
- Use LangChain with ForgeKit
- Use OdinCode with ForgeKit
- Use custom orchestrators with ForgeKit
- Use NOTHING — just get magellan/llmgrep/mirage results

**Plan Kernel is infrastructure, not a requirement.**

### Why Native-V3 as Plan Graph?

1. **Append-only** — No overwrite semantics, perfect for audit trails
2. **Queryable** — Graph algorithms find blocking steps, impact analysis
3. **Persistent** — Handoffs don't lose state
4. **Provable** — Edges form verifiable dependency chains

### Why Deterministic Scoring?

1. **Replayability** — Same code → same score, always
2. **No AI Judgement** — LLMs shouldn't decide "good enough"
3. **Debuggability** — Failed steps have objective failure reasons
4. **Regression Detection** — Delta metrics expose changes

### Why Pub/Sub?

1. **Decoupling** — Agents don't need direct references
2. **Scalability** — Add/remove agents without changing core
3. **Observability** — All coordination flows through events
4. **No Context Bloat** — Events are minimal, structured data

### Why Handoff Protocol?

1. **Token Budgets** — 128k context is real constraint
2. **Continuity** — Next agent resumes without full history
3. **Query-Based** — Resume via graph queries, not context dump
4. **Minimal** — Only touched files + next actions needed

---

## Success Criteria

- [ ] Plan Graph schema defined in Native-V3 format
- [ ] StepScore computation is deterministic and objective
- [ ] Pub/Sub event bus implemented with subscriptions
- [ ] File lease system prevents concurrent edits
- [ ] HANDOFF_SUMMARY structure defined and used
- [ ] Agent roles (Planner, Worker, Auditor) implemented
- [ ] Forge Runtime API surface matches specification
- [ ] Immutable proofs property enforced
- [ ] Replay produces identical results

---

## Consequences

### If Accepted

1. **Forge Runtime expands** — New `plan` module for graph-based planning
2. **Forge Agent simplifies** — Agent loop becomes event-driven subscriber
3. **Native-V3 dependency** — Plan graph requires V3 append-only format
4. **Multi-agent ready** — Pub/Sub enables swarm coordination
5. **Context resilience** — Handoff protocol handles arbitrary token budgets

### If Rejected

1. Continue with current ephemeral planning
2. Accept context bloat at handoff boundaries
3. Lose determinism in planning and scoring
4. Risk of non-reproducible agent behavior

---

## Related Decisions

- **ADR 2025-12**: ForgeKit as Deterministic Code Reasoning Engine — Trait-based architecture
- **ADR 2025-12**: OdinCode Integration — Provider pattern
- **ADR 2025-12**: Native-V3 Source of Truth — Graph database authority

---

*Status: Accepted — 2026-02-12*

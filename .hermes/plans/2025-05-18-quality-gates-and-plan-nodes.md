# Implementation Plan: Knowledge Explorer + Quality Gates + Plan Nodes

## Grounded Analysis Summary

**DB:** `.magellan/forge.db` — 79 files, 2431 symbols, 1644 references
**Wiki DB:** `/home/feanor/wiki/atheneum.db` — 912 entities (Agent/Event/Knowledge), 608 edges, HNSW vectors
**Workspace:** Compiles clean (`cargo check --workspace` passes)

### Evidence Cited

| Evidence | Source | Implication |
|----------|--------|-------------|
| `KnowledgeSource` trait at `observe.rs:13` | read_file | Already exists, `query(&self, target) -> Option<Vec<Value>>` |
| `Observer` has `knowledge_source` field at `observe.rs:34` | read_file | Wired but not connected to anything real |
| `Observer::gather()` checks knowledge_source at line 96 | read_file | Step 0: queries knowledge source before graph search |
| Wiki `atheneum.db` has `graph_entities` (kind: Agent/Event/Knowledge), `graph_edges` (edge_type), `hnsw_vectors` | sqlite3 query | Full semantic graph with history |
| Wiki edges: `created` (304), `performed_by` (304) | sqlite3 query | Who did what, when — provenance chain |
| Wiki entities contain full articles with SHA256, ingest timestamps, source URLs | sqlite3 query | Rich metadata for relevance matching |
| `forge_agent` sqlitegraph dep optional behind `"sqlite"` feature | Cargo.toml | Knowledge explorer must be feature-gated same way |
| `WorkflowExecutor::execute_task()` at `executor.rs:1085` | read_file | Gate hook point: after task exec, before record_task_completed |
| `TaskContext { forge: Option<Forge> }` at `task.rs:101` | read_file | Tasks have sqlitegraph access |
| `AuditLog` cloned into each `TaskContext` at `executor.rs:1133` | read_file | Gate results recordable as AuditEvent variants |
| No existing gate/quality/explore code | grep -rn | Green field |
| `HypothesisBoard`, `Evidence`, `KnowledgeGapAnalyzer` in forge-reasoning | magellan find | Reasoning layer exists but doesn't query external knowledge |

---

## Slice: Knowledge Explorer + Plan Nodes + Quality Gates + Semgrep

5 tasks, ordered by dependency. Each task follows RED-GREEN-REFACTOR.

### Task 0: Knowledge Explorer (the new piece)

**Purpose:** Before the model proposes a plan, it explores the wiki graph and project history
to find relevant knowledge — past decisions, dead ends, working patterns, related research.

**Files to create:**
- `forge_agent/src/workflow/explorer.rs` — KnowledgeExplorer types + explorer

**Types:**
```rust
/// What to explore and how deep to go.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExploreQuery {
    /// The project or topic to explore (e.g. "sparse inference", "forge")
    pub topic: String,
    /// Kinds of entities to look for
    pub entity_kinds: Vec<String>,
    /// How many hops from seed entities
    pub depth: u32,
    /// Max results to return
    pub limit: usize,
    /// Include project history (past decisions, dead ends)
    pub include_history: bool,
}

/// A piece of discovered knowledge relevant to the current task.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveredKnowledge {
    /// What was found
    pub title: String,
    /// Entity kind (Agent, Event, Knowledge)
    pub kind: String,
    /// Relevance snippet
    pub summary: String,
    /// Where it came from (wiki path, project, URL)
    pub source: String,
    /// How it was found (semantic, graph traversal, keyword)
    pub discovery_method: String,
    /// Relevance score (0.0-1.0)
    pub relevance: f64,
    /// Connected entities (for graph navigation)
    pub related: Vec<String>,
    /// Whether this is a historical decision (past plan, dead end, lesson)
    pub is_historical: bool,
}

/// Explores wiki graph + project metadata for relevant knowledge.
pub struct KnowledgeExplorer {
    /// Path to atheneum wiki DB
    wiki_db: PathBuf,
    /// Path to project's magellan DB (if exists)
    project_db: Option<PathBuf>,
}

impl KnowledgeExplorer {
    /// Create explorer pointing to wiki DB.
    /// Returns None if wiki DB doesn't exist — caller should degrade gracefully.
    pub fn new(wiki_db: PathBuf) -> Option<Self>;

    /// Create explorer in code-graph-only mode (no wiki).
    /// Used when user has no wiki DB — all knowledge comes from
    /// the project's own magellan DB.
    pub fn code_only(project_db: PathBuf) -> Self;

    /// Set project DB for project-specific history.
    pub fn with_project_db(mut self, db: PathBuf) -> Self;

    /// Explore wiki for knowledge relevant to a query.
    /// Uses HNSW semantic search on the wiki graph, then
    /// traverses edges to find connected decisions and history.
    /// No-ops in code_only mode (returns empty vec).
    pub async fn explore(&self, query: &ExploreQuery) -> anyhow::Result<Vec<DiscoveredKnowledge>>;

    /// Find project history — past decisions, dead ends, lessons.
    /// Queries the wiki for entities tagged with the project name
    /// and traverses created/performed_by edges for provenance.
    /// Falls back to project's magellan DB if no wiki.
    pub async fn find_project_history(&self, project: &str) -> anyhow::Result<Vec<DiscoveredKnowledge>>;

    /// Find cross-project connections.
    /// Searches for entities related to concepts that appear
    /// in the current project's codebase.
    pub async fn find_connections(&self, symbols: &[String]) -> anyhow::Result<Vec<DiscoveredKnowledge>>;
}

/// NOTE: Installation flow (forge-py, not this crate) should:
/// 1. Check if wiki DB exists at default path (~/.forge/wiki.db or user-specified)
/// 2. If not, prompt: "Would you like to create a knowledge base? This helps
///    forge learn from past decisions and find cross-project patterns."
/// 3. If yes, initialize empty sqlitegraph DB with wiki schema
/// 4. If no, proceed in code-only mode (no wiki exploration)
/// This is a distribution concern, not a library concern — but the API
/// must support both paths cleanly.
```

**Implementation:**
- `explore()`: Opens atheneum.db as sqlitegraph, uses `search(query, k)` for HNSW semantic
  search, then traverses `graph_edges` (created, performed_by) from seed entities to find
  connected decisions and history. Returns ranked `DiscoveredKnowledge`.
- `find_project_history()`: Cypher query on wiki graph:
  `MATCH (e:Knowledge) WHERE e.data LIKE '%project%' RETURN e`
  Then follows `created`/`performed_by` edges for provenance chain.
- `find_connections()`: For each symbol name, HNSW search in wiki, then deduplicate.

**Integration with Observer:**
- Implement `KnowledgeSource` trait from `observe.rs:13` for `KnowledgeExplorer`
- Wire into `Observer::with_knowledge_source()` so `gather()` at line 96 automatically
  queries wiki before doing expensive graph searches
- The Observer already checks knowledge_source at line 96 — this makes it real

**Test file:** inline tests
- test_explore_returns_relevant_knowledge (mock wiki DB)
- test_find_project_history_traverses_edges
- test_find_connections_deduplicates
- test_discovered_knowledge_serialization
- test_knowledge_source_trait_impl
- test_code_only_mode_returns_empty_explore
- test_new_returns_none_when_no_db

---

### Task 1: AuditEvent variants for gates + exploration

**File to modify:** `forge_agent/src/audit.rs`

**Add to AuditEvent enum:**
```rust
/// Knowledge explored before planning
KnowledgeExplored {
    timestamp: DateTime<Utc>,
    query: String,
    results_count: usize,
    top_relevance: f64,
},
/// Quality gate passed
GatePassed {
    timestamp: DateTime<Utc>,
    workflow_id: String,
    task_id: String,
    gate_name: String,
    duration_ms: u64,
},
/// Quality gate failed
GateFailed {
    timestamp: DateTime<Utc>,
    workflow_id: String,
    task_id: String,
    gate_name: String,
    exit_code: i32,
    errors: u32,
    warnings: u32,
},
/// Semgrep finding
SemgrepFinding {
    timestamp: DateTime<Utc>,
    workflow_id: String,
    task_id: String,
    check_id: String,
    file: String,
    line: u32,
    message: String,
    severity: String,
},
```

**Test file:** inline tests
- test_audit_event_knowledge_explored_serialization
- test_audit_event_gate_passed_serialization
- test_audit_event_gate_failed_serialization
- test_audit_event_semgrep_finding_serialization

---

### Task 2: Gate types and runner

**Files to create:**
- `forge_agent/src/workflow/gate.rs` — Gate types + GateRunner

**Types:**
```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GateLanguage { Rust, Python, TypeScript, Go }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Gate {
    pub name: String,
    pub tool: String,
    pub language: GateLanguage,
    pub priority: u32,
    pub on_fail: GateAction,
    pub config: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GateAction { Block, Warn, AutoFix }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GateResult {
    pub gate_name: String,
    pub passed: bool,
    pub exit_code: i32,
    pub stdout: String,
    pub structured_output: Option<serde_json::Value>,
    pub errors: u32,
    pub warnings: u32,
    pub duration_ms: u64,
}

pub struct GateRunner { gates: Vec<Gate> }
```

**Test file:** inline tests
- test_gate_priority_ordering
- test_short_circuit_on_block
- test_warn_does_not_block
- test_gate_result_serialization

---

### Task 3: Semgrep gate implementation

**File to create:** `forge_agent/src/workflow/semgrep.rs`

**Types:**
```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SemgrepFinding {
    pub check_id: String,
    pub file: String,
    pub start_line: u32,
    pub end_line: u32,
    pub message: String,
    pub severity: String,
    pub category: Option<String>,
}

pub struct SemgrepRunner {
    configs: Vec<String>,
    json_output: bool,
}
```

**Built-in gate presets:**
```rust
impl Gate {
    pub fn semgrep(project_root: &Path) -> Self { ... }
    pub fn clippy() -> Self { ... }
    pub fn cargo_fmt() -> Self { ... }
    pub fn ruff() -> Self { ... }
    pub fn mypy() -> Self { ... }
}
```

**Test file:** inline tests
- test_semgrep_finding_parse_from_json
- test_semgrep_runner_empty_findings_passes
- test_all_presets_have_valid_priority

---

### Task 4: Plan graph nodes in sqlitegraph

**File to create:** `forge_agent/src/workflow/plan.rs`

**Types:**
```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PlanNodeKind {
    Requirement, Plan, Task, Decision, Constraint,
    Gate, GateResult, SemgrepFinding,
    Approval, Rejection,
    DiscoveredKnowledge,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PlanEdgeKind {
    HasRequirement, DecomposesInto, Implements, DependsOn,
    ValidatedBy, AssignedTo, Approved, Rejected,
    FoundIn, DetectedBy, Checks,
    InformedBy,       // Plan/Decision informed by DiscoveredKnowledge
    RelatedTo,        // Cross-project connection from explorer
}

pub struct PlanGraph { graph: SqliteGraph }
```

**New methods for knowledge integration:**
```rust
impl PlanGraph {
    // ... existing methods from prior plan ...

    /// Record that a plan was informed by discovered knowledge.
    pub fn link_knowledge(&mut self, plan_id: i64, knowledge: &DiscoveredKnowledge) -> anyhow::Result<()>;

    /// Query all knowledge that informed a plan.
    pub fn get_plan_knowledge(&self, plan_id: i64) -> anyhow::Result<Vec<DiscoveredKnowledge>>;
}
```

**Test file:** inline tests
- test_add_requirement_creates_node
- test_add_plan_links_to_requirements
- test_link_knowledge_creates_informed_by_edge
- test_gate_result_links_to_gate
- test_semgrep_finding_links_to_gate_result
- test_approve_creates_approval_edge
- test_reject_creates_rejection_edge
- test_cypher_query_returns_results
- test_full_plan_graph_roundtrip

---

## Execution Order

```
Task 0 (KnowledgeExplorer)  → no dependencies, new file
Task 1 (AuditEvent variants) → no dependencies, pure enum additions
Task 2 (Gate types + GateRunner) → depends on Task 1 (emits AuditEvents)
Task 3 (Semgrep runner) → depends on Task 2 (produces GateResult)
Task 4 (Plan graph nodes) → depends on Task 0, 2, 3 (stores all types)
```

Tasks 0 and 1 are independent — can run in parallel.

## The Flow

```
User inputs requirement
    ↓
KnowledgeExplorer.explore(requirement)
    → HNSW search wiki graph → DiscoveredKnowledge[]
    → find_project_history(project) → past decisions, dead ends
    ↓
Model proposes Plan (informed by discovered knowledge)
    ↓
User reviews, adjusts, approves → PlanGraph.approve()
    ↓
Plan decomposes into Tasks → PlanGraph.add_task()
    ↓
Each Task executes → GateRunner.run()
    → semgrep, clippy, ruff, mypy in priority order
    → GateResult nodes in PlanGraph
    → SemgrepFinding nodes linked to GateResult
    ↓
All gates pass → Deliver
Any Block gate fails → Rollback, report to user
```

## Verification Gates

After all tasks:
- `cargo check --workspace` — clean
- `cargo clippy --workspace -- -D warnings` — clean
- `cargo test -p forge-agent --features sqlite` — all new tests pass
- `cargo fmt --check` — clean

## Not In Scope (future work)

- Turn/Session tracking (LLM interaction logging)
- Edit nodes with SHA diff tracking
- ToolCall recording
- Benchmark regression nodes
- CoverageReport nodes
- TUI dashboard rendering
- Gate integration into WorkflowExecutor::execute_task() hook
- Python bindings (forge-py)
- Multi-agent orchestration via envoy

# Knowledge Graph Architecture — sqlitegraph native-v3

**Date:** 2026-05-13
**Status:** Draft
**Affects:** forge_core, forge_agent
**Depends on:** sqlitegraph native-v3 backend

---

## Problem

The forge framework has rich symbol and reference data in Magellan's SQLite database, but no way to traverse relationships graphically. Agents (ContextComposer, Observer) need to discover callers, callees, correlations, affected symbols, and semantically similar code through graph traversal — not SQL queries.

The current architecture queries the SQLite `.db` directly for everything. This works for lookups but cannot express:

- "What discoveries correlate with this symbol?"
- "What's the shortest path between these two functions?"
- "What symbols are semantically similar to this one?"
- "What issues affect the callers of this function?"

These are graph problems, not SQL problems.

## Solution

A **KnowledgeGraph** module in forge_core backed by sqlitegraph native-v3 (`.graph` binary file). The graph holds typed nodes with KV properties and typed edges representing relationships. SQLite FTS5 remains the entry-point index (keyword → node ID), and graph traversal handles everything after that.

The `.graph` file lives alongside the existing `.db` file. Magellan/llmgrep/mirage continue using the `.db` unchanged.

## Architecture

```
┌─────────────────────────────────────────────┐
│  Agent / ContextComposer / Observer          │
│  navigates the graph                         │
└──────────────┬──────────────────────────────┘
               │
┌──────────────▼──────────────────────────────┐
│  forge_core::KnowledgeGraph                  │
│  (sqlitegraph native-v3 backend)             │
│                                              │
│  Symbol nodes ──calls──► Symbol nodes        │
│       │                                      │
│  File nodes ◄──contains── Symbol nodes       │
│       │                                      │
│  Discovery nodes ◄──correlates── Symbol nodes│
│       │                                      │
│  Issue nodes ──affects──► Symbol nodes       │
│       │                                      │
│  CFG Block nodes ──flows_to──► CFG Blocks    │
│       │                                      │
│  Pattern nodes ──derived_from──► Symbols     │
│       │                                      │
│  Knowledge nodes ──mentions──► Symbols       │
│       │                                      │
│  Hotspot nodes ◄──belongs_to── Symbols       │
│       │                                      │
│  HNSW vectors (semantic similarity)          │
└──────────────┬──────────────────────────────┘
               │
┌──────────────▼──────────────────────────────┐
│  SQLite FTS5 (entry-point index only)        │
│  in existing Magellan .db                    │
│                                              │
│  "process_payment" → node #47                │
│  "auth" → node #23                          │
│  "middleware" → node #89                     │
│                                              │
│  graph_node_index table maps                 │
│  FTS5 results to graph node IDs              │
└─────────────────────────────────────────────┘
```

## File Layout

```
project/
├── .magellan/
│   ├── magellan.db          ← Magellan SQLite DB (FTS5, symbols, refs)
│   └── knowledge.graph      ← Knowledge graph (sqlitegraph native-v3)
└── src/
```

- `magellan.db` is owned by Magellan. forge_core reads from it during sync but never writes to Magellan tables.
- `knowledge.graph` is owned by forge_core. Created and managed by the KnowledgeGraph module.
- One `graph_node_index` table is added to `magellan.db` to bridge FTS5 results to graph node IDs. This is the only structural change to the `.db`.

## Node Types

Every node is a `sqlitegraph::GraphEntity` with:
- `kind`: discriminates the node type
- `name`: human-readable identifier
- `file_path`: source file (for code-related nodes)
- `data`: JSON object with type-specific KV properties

### symbol

Source code symbol — function, struct, method, trait, enum, etc. Mirrors what Magellan indexes.

| Property | Type | Description |
|----------|------|-------------|
| `symbol_kind` | string | Function, Struct, Method, Enum, Trait, etc. |
| `qualified_name` | string | Fully qualified name (e.g. `crate::module::function`) |
| `file` | string | Source file path |
| `line` | number | Line number |
| `byte_start` | number | Start byte offset |
| `byte_end` | number | End byte offset |
| `language` | string | Rust, Python, etc. |
| `parent_id` | number? | Parent symbol node ID (for methods in impl blocks, etc.) |

### file

Source file.

| Property | Type | Description |
|----------|------|-------------|
| `path` | string | File path relative to project root |
| `language` | string | Detected language |
| `hash` | string | BLAKE3 hash of file contents |
| `last_modified` | string | ISO timestamp |

### discovery

Cached knowledge from an agent observation or external source.

| Property | Type | Description |
|----------|------|-------------|
| `discovery_type` | string | Symbol, CFG, Issue, Pattern |
| `agent` | string | Agent that created this discovery |
| `timestamp` | string | ISO timestamp |
| `metadata` | object | Arbitrary JSON metadata |

### cfg_block

CFG basic block from mirage analysis.

| Property | Type | Description |
|----------|------|-------------|
| `function_id` | number | Symbol node ID of the containing function |
| `start_byte` | number | Start byte offset in function |
| `end_byte` | number | End byte offset in function |
| `block_kind` | string | Entry, Basic, LoopHeader, Condition |
| `is_error` | boolean | Whether this is an error/panic path |

### hotspot

Performance or risk area identified by analysis.

| Property | Type | Description |
|----------|------|-------------|
| `complexity` | number | Cyclomatic complexity |
| `risk_score` | number | 0.0–1.0 risk score |
| `loop_depth` | number | Maximum loop nesting |
| `description` | string | Human-readable description |

### pattern

Recognized code pattern (e.g., "builder pattern", "clone escape hatch").

| Property | Type | Description |
|----------|------|-------------|
| `pattern_type` | string | Category of pattern |
| `confidence` | number | 0.0–1.0 detection confidence |
| `description` | string | What was detected |

### issue

Bug or code smell.

| Property | Type | Description |
|----------|------|-------------|
| `severity` | string | critical, high, medium, low |
| `description` | string | What the issue is |
| `rule_id` | string? | Lint rule or check that found it |

### knowledge

External knowledge entry — wiki article, spec document, design doc.

| Property | Type | Description |
|----------|------|-------------|
| `source` | string | wiki, docs, specs |
| `title` | string | Title of the entry |
| `tags` | array | String tags for categorization |
| `summary` | string | Brief summary |

## Edge Types

Every edge is a `sqlitegraph::GraphEdge` with:
- `edge_type`: discriminates the relationship
- `data`: JSON object with optional metadata

### calls

symbol → symbol. Function A calls function B.

| Property | Type | Description |
|----------|------|-------------|
| `location_file` | string | Where the call occurs |
| `location_line` | number | Line number of the call site |

### contains

file → symbol. File contains this symbol.

No additional properties.

### references

symbol → symbol. A references B (use, type reference, import, implementation).

| Property | Type | Description |
|----------|------|-------------|
| `ref_kind` | string | Call, Use, TypeReference, Inherit, Implementation, Override |
| `location_file` | string | Where the reference occurs |
| `location_line` | number | Line number |

### correlates

discovery ↔ symbol, discovery ↔ discovery. Discovery is about a symbol, or two discoveries are related.

| Property | Type | Description |
|----------|------|-------------|
| `confidence` | number | 0.0–1.0 correlation confidence |
| `agent` | string | Agent that established the correlation |
| `correlation_type` | string? | For discovery↔discovery: "related", "supersedes", "contradicts" |

Stored as two directed edges for bidirectional traversal.

### affects

issue → symbol. Issue affects this symbol.

No additional properties.

### flows_to

cfg_block → cfg_block. Control flow from one block to the next.

| Property | Type | Description |
|----------|------|-------------|
| `condition` | string? | Branch condition text, if applicable |

### similar_to

symbol ↔ symbol. Semantic similarity from HNSW vector search.

| Property | Type | Description |
|----------|------|-------------|
| `distance` | number | Cosine distance (0.0 = identical) |

Stored as two directed edges for bidirectional traversal. Populated by HNSW index queries.

### derived_from

pattern → symbol. Pattern was detected in this symbol.

| Property | Type | Description |
|----------|------|-------------|
| `confidence` | number | 0.0–1.0 detection confidence |

### discovered_by

discovery → discovery. Agent X's discovery built on agent Y's earlier finding.

| Property | Type | Description |
|----------|------|-------------|
| `agent` | string | Agent that made the derived discovery |

### mentions

knowledge → symbol. Knowledge entry mentions this symbol.

| Property | Type | Description |
|----------|------|-------------|
| `context` | string? | Surrounding context of the mention |

### belongs_to

cfg_block → symbol, hotspot → symbol. Block or hotspot belongs to this function.

No additional properties.

## FTS5 Bridge

A single table in the existing Magellan `.db` bridges FTS5 results to graph node IDs:

```sql
CREATE TABLE IF NOT EXISTS graph_node_index (
    node_id INTEGER PRIMARY KEY,
    magellan_id INTEGER,          -- Magellan symbol_id for cross-referencing
    node_kind TEXT NOT NULL,
    graph_file TEXT NOT NULL
);
```

**Query flow:**

1. FTS5 search: `SELECT * FROM symbols_fts WHERE symbols_fts MATCH 'process_payment'` → returns Magellan symbol data with `symbol_id`
2. Node lookup: `SELECT node_id FROM graph_node_index WHERE magellan_id = <symbol_id>` → graph node ID
3. Graph traversal: `knowledge_graph.callers_of(node_id, depth=3)` → results

The bridge is populated during `sync_symbols()`. It requires no changes to Magellan's existing tables or FTS5 configuration.

## KnowledgeGraph API

```rust
pub struct KnowledgeGraph {
    graph: sqlitegraph::SqliteGraph,
    db_path: PathBuf,
}

impl KnowledgeGraph {
    // -- Lifecycle --
    pub fn open(graph_path: &Path, db_path: &Path) -> Result<Self>;
    pub fn close(self) -> Result<()>;

    // -- Sync from Magellan --
    pub async fn sync_symbols(&self) -> Result<SyncReport>;
    pub async fn sync_references(&self) -> Result<SyncReport>;

    // -- Node CRUD --
    pub fn add_symbol(&self, symbol: &Symbol) -> Result<i64>;
    pub fn add_file(&self, path: &str, language: &str, hash: &str) -> Result<i64>;
    pub fn add_discovery(&self, agent: &str, discovery_type: &str, target: &str, metadata: Value) -> Result<i64>;
    pub fn add_issue(&self, severity: &str, description: &str, affected_symbols: &[i64]) -> Result<i64>;
    pub fn add_pattern(&self, pattern_type: &str, confidence: f64, target_node: i64) -> Result<i64>;
    pub fn add_knowledge(&self, source: &str, title: &str, tags: &[String], summary: &str) -> Result<i64>;
    pub fn add_cfg_block(&self, function_id: i64, block: &CfgBlockData) -> Result<i64>;
    pub fn add_hotspot(&self, symbol_id: i64, complexity: u32, risk_score: f64) -> Result<i64>;
    pub fn get_node(&self, node_id: i64) -> Result<GraphNode>;
    pub fn find_nodes_by_kind(&self, kind: &str) -> Result<Vec<GraphNode>>;

    // -- Edge operations --
    pub fn add_edge(&self, from: i64, to: i64, edge_type: &str, data: Value) -> Result<i64>;
    pub fn add_correlation(&self, from: i64, to: i64, confidence: f64, agent: &str) -> Result<i64>;

    // -- Traversal --
    pub fn callers_of(&self, symbol_id: i64, max_depth: u32) -> Result<Vec<GraphNode>>;
    pub fn callees_of(&self, symbol_id: i64, max_depth: u32) -> Result<Vec<GraphNode>>;
    pub fn neighbors(&self, node_id: i64, edge_type: &str, direction: Direction) -> Result<Vec<GraphNode>>;
    pub fn affected_by(&self, symbol_id: i64, depth: u32) -> Result<Vec<GraphNode>>;
    pub fn correlated(&self, node_id: i64) -> Result<Vec<GraphNode>>;
    pub fn similar_symbols(&self, symbol_id: i64, k: usize) -> Result<Vec<(f64, GraphNode)>>;

    // -- Entry point: FTS5 query → graph traversal --
    pub async fn query(&self, query: &str, depth: u32) -> Result<QueryResult>;

    // -- Graph algorithms --
    pub fn pagerank(&self) -> Result<Vec<(i64, f64)>>;
    pub fn cycles(&self) -> Result<Vec<Vec<i64>>>;
    pub fn reachability(&self, from: i64) -> Result<Vec<i64>>;
    pub fn shortest_path(&self, from: i64, to: i64) -> Result<Option<Vec<i64>>>;
    pub fn community_detection(&self) -> Result<Vec<Vec<i64>>>;
}

pub enum Direction { Incoming, Outgoing }

pub struct GraphNode { /* wraps sqlitegraph::GraphEntity with typed accessors */ }

pub struct SyncReport {
    pub nodes_added: usize,
    pub nodes_updated: usize,
    pub nodes_unchanged: usize,
    pub edges_added: usize,
    pub edges_updated: usize,
}

pub struct QueryResult {
    pub entry_node: GraphNode,
    pub callers: Vec<GraphNode>,
    pub callees: Vec<GraphNode>,
    pub correlated: Vec<GraphNode>,
    pub affected: Vec<GraphNode>,
    pub similar: Vec<(f64, GraphNode)>,
}

pub struct CfgBlockData {
    pub start_byte: u32,
    pub end_byte: u32,
    pub block_kind: String,
    pub is_error: bool,
}
```

## Agent Navigation Example

Query: "what could break if I change process_payment?"

```
1. knowledge_graph.query("process_payment", depth=3)
   ├── FTS5: "process_payment" → node #47 (symbol)
   └── Returns QueryResult

2. callers_of(47, depth=2)
   ├── #47 --calls--> #12 (validate_card)
   ├── #47 --calls--> #15 (charge_amount)
   ├── #23 (checkout_handler) --calls--> #47
   └── #31 (retry_worker) --calls--> #47

3. correlated(47)
   ├── #47 <--correlates-- #88 (discovery: "handles 3 edge cases")
   └── #47 <--correlates-- #92 (discovery: "shared mutex with process_refund")

4. affected_by(47, depth=3)
   ├── #47 --affects--> #55 (order_service)
   ├── #47 --affects--> #61 (notification_queue)
   └── #47 --affects--> #67 (webhook_handler)

5. similar_symbols(47, k=5)
   ├── (0.06, process_refund)       ← HNSW cosine distance
   ├── (0.13, process_subscription)
   └── (0.19, void_transaction)

6. correlated(#process_refund node)
   └── #47 <--correlates-- #92 (discovery: "shared mutex with process_refund")
```

Step 6 is the powerful one: the agent discovers that `process_refund` shares a mutex with `process_payment` through a correlation edge stored as a discovery. This requires graph traversal, not SQL.

## Sync Strategy

Symbols and references in the `.graph` file are mirrors of Magellan's `.db`. Knowledge-only nodes (discoveries, patterns, issues, knowledge, hotspots) are written directly by agents.

### Initial sync

1. `sync_symbols()` reads all symbols from `magellan.db`
2. Creates `symbol` and `file` nodes in `.graph`
3. Creates `contains` edges (file → symbol)
4. Populates `graph_node_index` table in `.db`
5. Returns `SyncReport`

### Reference sync

1. `sync_references()` reads all references from `magellan.db`
2. Creates `calls` and `references` edges between existing symbol nodes
3. Returns `SyncReport`

### Incremental sync

On subsequent calls, only changed symbols (by hash comparison) are updated. sqlitegraph's MVCC snapshots ensure consistent reads during sync.

### Knowledge writes

Agents write discoveries, patterns, issues, and knowledge entries directly to `.graph`:

```rust
let discovery_id = kg.add_discovery(
    "claude1",
    "Symbol",
    "process_payment",
    json!({"complexity": 8, "handles_edge_cases": 3})
)?;

let symbol_id = kg.find_nodes_by_kind("symbol")?
    .into_iter()
    .find(|n| n.name() == "process_payment")
    .map(|n| n.id());

kg.add_correlation(discovery_id, symbol_id, 0.95, "claude1")?;
```

## HNSW Vector Integration

Symbol nodes can have vector embeddings for semantic similarity search:

1. During sync, if embedding data is available (from llmgrep or an embedding model), vectors are added to an HNSW index in the `.graph` file
2. Vectors are linked to symbol node IDs
3. `similar_symbols()` queries the HNSW index and returns ranked results

The HNSW index is optional — the graph works without it, but semantic similarity requires it.

## Integration with forge_agent

The `ContextComposer` and `Observer` in forge_agent use `KnowledgeGraph` instead of direct SQL queries:

- `Observer::gather()` calls `knowledge_graph.query(query, depth)` for the entry point
- `ContextComposer::for_task()` uses graph traversal for callers, callees, correlations
- The `DiscoveryStore` trait implementation writes discoveries directly to the `.graph` file
- The `KnowledgeSource` trait implementation reads from the `.graph` file via graph traversal

No changes to forge_agent's public API — the KnowledgeGraph is an internal implementation detail.

## Migration Path

1. **Phase 1:** Add `KnowledgeGraph` module to forge_core with node/edge types, sync, and basic traversal
2. **Phase 2:** Wire `ContextComposer` to use `KnowledgeGraph` instead of direct Magellan queries
3. **Phase 3:** Wire `Observer` to use `KnowledgeGraph` for correlation and knowledge lookup
4. **Phase 4:** Add HNSW vector support for semantic similarity
5. **Phase 5:** Add graph algorithms (PageRank, community detection) to power insights

Each phase is independently testable and deployable.

## Constraints

- **No changes to Magellan's schema** beyond adding `graph_node_index` table
- **No changes to llmgrep or mirage** — they continue using the `.db` directly
- **The `.graph` file is a forge_core concern** — other tools don't need to know about it
- **Symbol nodes are mirrors** — Magellan is the indexing authority, the graph is for traversal
- **Knowledge nodes are authoritative** — discoveries, patterns, issues live only in the graph

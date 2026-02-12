# Philosophy

**Version**: 0.1.0
**Created**: 2025-12-30

---

## The Problem

LLMs are forced to work with Unix text-era tools:

```
grep -r "function foo" .          # Returns 50 lines of text
cat src/main.rs                    # 10,000 tokens of context
sed 's/foo/bar/g' src/main.rs     # Hope it works
```

**Result**: Context bloat → Early compaction → **Guessing**

---

## Why Another Tool?

Most "AI coding assistants" share fundamental flaws:

| Problem | Conventional Tool | ForgeKit |
|----------|------------------|------------|
| Code discovery | grep (text search) | Graph queries (structured facts) |
| Symbol resolution | Regex matching | AST-verified references |
| Code mutation | Text substitution | Span-safe patches |
| Validation | Hope | Compiler/LSP gatekeeper |
| Rollback | Manual git revert | Atomic transactions |

---

## Deterministic Stack

ForgeKit is built on four deterministic components:

```
┌─────────────────────────────────────────────────────────────┐
│                    ForgeKit                            │
│                                                          │
│  ┌────────────┐  ┌────────────┐  ┌─────────────┐  │
│  │  Magellan  │  │  LLMGrep   │  │   Mirage    │  │
│  │  Symbol    │  │  Search     │  │   CFG       │  │
│  │  Graph     │  │  Engine     │  │  Analysis   │  │
│  └─────┬──────┘  └─────┬──────┘  └──────┬──────┘  │
│         │                 │                  │           │
│         └─────────────────┴──────────────────┘           │
│                            │                            │
│                    ┌───────┴────────┐                 │
│                    │     Splice       │                 │
│                    │  Span-Safe     │                 │
│                    │   Editing       │                 │
│                    └────────────────┘                 │
└─────────────────────────────┬────────────────────────────┘
                          │
         ┌────────────────┴────────────────┐
         │                                  │
┌────────┴────────┐            ┌───────────┴──────────┐
│  SQLiteGraph     │            │   Native V3 (WIP)     │
│  (Production)    │            │   (Future)              │
└─────────────────┘            └──────────────────────┘
```

### Magellan: Symbol Graph

- AST-based indexing for 7 languages
- Exact symbol locations (byte spans)
- Reference relationships (calls, uses, inherits)
- 35+ graph algorithms

### LLMGrep: Semantic Search

- AST-aware pattern matching
- Symbol lookup by semantic properties
- Structured query results (JSON)

### Mirage: CFG Analysis

- Control flow graph construction
- Path enumeration
- Dominance analysis
- Loop detection

### Splice: Span-Safe Editing

- Precise byte-range targeting
- Multi-file refactoring
- Validation before commit
- Rollback on failure

---

## The Deterministic Loop

Most AI tools:

```
Prompt → Guess → Rewrite → Hope
```

ForgeKit:

```
Query → Graph Reason → Validate → Safe Patch → Re-index
```

### Why This Matters

| Step | Conventional | ForgeKit |
|-------|--------------|-----------|
| Query | grep → 5000 lines | Graph → 3 JSON objects |
| Reason | LLM inference | Graph traversal |
| Validate | None | Compiler/LSP |
| Patch | sed -i | Span-safe edit |
| Re-index | Manual | Automatic |

---

## Core Principles

### 1. Graph is Truth

The SQLiteGraph database is the authoritative source.

```rust
// NEVER assume structure
// ALWAYS query the graph

let symbol = forge.graph().find_symbol("main")?;
let refs = forge.graph().references(&symbol.id)?;
```

**Consequences:**
- No ambiguous symbol resolution
- No missed cross-file references
- No stale code assumptions

### 2. Spans are Immutable

Byte spans from the AST are the only reliable coordinates.

```rust
// WRONG: String search
let pos = code.find("foo");  // Which foo?

// RIGHT: Span from graph
let span = symbol.location.byte_span;
let edit = Edit::replace(span, "bar");
```

**Consequences:**
- No false positive matches
- No ambiguity in renames
- No broken patches

### 3. Operations are Transactions

Every operation is all-or-nothing.

```rust
let result = forge.edit()
    .rename_symbol("A", "B")?
    .verify()?      // Pre-flight check
    .apply()?       // Atomic commit

// If any step fails:
// - No files modified
// - No database corruption
// - Full rollback
```

**Consequences:**
- No partial mutations
- No inconsistent state
- Always recoverable

### 4. Verification is Mandatory

Compiler/LSP must approve all changes.

```rust
// Splice validates:
// 1. Parse result is valid AST
// 2. No syntax errors
// 3. Type checking passes (via LSP if available)

// Only then does commit proceed
```

**Consequences:**
- No broken code committed
- No silent failures
- Immediate feedback

---

## LLVM for AI Code Agents

ForgeKit is infrastructure, not another wrapper.

### What "LLVM for Agents" Means

| LLVM Aspect | ForgeKit Equivalent |
|-------------|-------------------|
| Intermediate Representation | Graph (AST + CFG) |
| Optimization Passes | Analysis modules |
| Code Generation | Splice patches |
| Backend Targets | Languages (Rust, Python, etc.) |

### Why This Analogy

**LLVM:**
- Takes IR as input
- Applies verified transformations
- Generates target code
- Backend-agnostic design

**ForgeKit:**
- Takes graph as input
- Applies verified mutations
- Generates modified code
- Language-agnostic design

### The Killer Feature

**Deterministic AI Loop:**

```rust
forge.agent()
    .observe(query)        // Graph-based observation
    .constrain(policy)     // Policy enforcement
    .plan()               // Graph-based planning
    .mutate()            // Span-safe mutation
    .verify()             // Compiler validation
    .commit();            // Atomic commit
```

Every step is:
- **Observable**: Logs, metrics
- **Verifiable**: Graph artifacts
- **Reversible**: Transaction rollback

---

## Against Hallucination

ForgeKit is designed to prevent LLM hallucination at every step.

### Prevention Layers

| Layer | Mechanism |
|--------|-----------|
| Input | Structured queries, not text |
| Reasoning | Graph traversal, not inference |
| Output | Spans, not guesses |
| Validation | Compiler, not hope |

### Example: Rename Operation

**Without ForgeKit:**

```
LLM: "Rename foo to bar"
1. grep -r "foo" .      → 5000 results
2. LLM filters            → misses string literals
3. sed -i 's/foo/bar/g'  → breaks "foo_bar"
4. Tests fail            → manual fix
```

**With ForgeKit:**

```
LLM: "Rename foo to bar"
1. forge.graph().references("foo")
   → [(file, span), ...]  // Exact locations
2. forge.edit().rename("foo", "bar").verify()
   → Validates syntax, types
3. .apply()
   → Atomic mutation
4. Tests pass
```

---

## Local-First by Design

ForgeKit runs entirely on your machine.

### What This Means

- No code leaves your machine
- No API rate limits
- No network dependency
- Full audit trail

### Privacy Properties

| Data | Stored | Sent | Retained |
|-------|---------|--------|-----------|
| Source code | Local | Never | Never |
| Graph database | Local | Never | Never |
| Query results | Local | Never | Never |
| Edit operations | Local | Never | Never |

---

## Performance Philosophy

### Compact Representation

Text is expensive. Facts are cheap.

| Query | grep Output | ForgeKit Output | Savings |
|--------|-------------|-----------------|----------|
| Functions in file | ~500 lines | 3 JSON objects | 98% |
| Callers of X | ~200 lines | 5 JSON objects | 95% |
| All symbols | ~5000 lines | 150 JSON objects | 97% |

**Impact on LLMs:**
- Less context bloat
- Fewer tokens consumed
- More accurate responses
- Lower costs

### Incremental Updates

ForgeKit only reindexes what changed.

```
File change detected
    ↓
Parse file (tree-sitter)
    ↓
Extract symbols/refs
    ↓
Update affected graph regions
    ↓
Invalidate stale cache entries
    ↓
Ready for queries
```

**Result:** Sub-second reindex for typical changes

---

## The ForgeKit Contract

ForgeKit guarantees:

1. **Determinism**: Same input → Same output
2. **Verification**: Invalid operations → Reject
3. **Atomicity**: All-or-nothing mutations
4. **Audibility**: Every operation logged
5. **Reversibility**: Rollback always available

### What ForgeKit Does NOT Do

- ❌ Guess symbol locations
- ❌ Apply invalid patches
- ❌ Hide errors
- ❌ Send code to servers
- ❌ Require subscriptions

---

## Vision Statement

> **"Make LLMs stop guessing. Make them reason."**

ForgeKit exists to provide:

- **Facts, not search results**
- **Spans, not text positions**
- **Verification, not hope**
- **Graphs, not grep**

This is the foundation for AI-native development tools.

---

*Last updated: 2025-12-30*

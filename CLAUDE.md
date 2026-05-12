# ForgeKit Development Rules - Unified Code Intelligence SDK

**Project:** ForgeKit - Deterministic code intelligence SDK + agent orchestration
**Workspace Members:** forge-core (0.2.2), forge-runtime (0.1.2), forge-agent (0.4.0), forge-reasoning (0.1.2)
**Last Updated:** 2026-05-04

---

## Shared Agent Workflow

Follow `/home/feanor/Projects/CLAUDE.md` for the shared rules: state assumptions before coding, use Magellan/llmgrep/Mirage for code-structure claims, keep edits surgical, preserve dirty worktree changes, and report fresh verification evidence before claiming completion. Repo-specific rules below add ForgeKit architecture and workspace conventions.

## Quick Start

```bash
# Build all workspace members
cargo build

# Build release
cargo build --release

# Run tests
cargo test

# Run tests for specific member
cargo test -p forge-core
cargo test -p forge-agent

# Lint
cargo clippy --all-targets
```

**Note:** This is a Cargo workspace with 4 members. `cargo build` builds all of them.

---

## Database Convention

Project database: `.magellan/forge.db`

Since this is a workspace, the database indexes all member `src/` directories:
```bash
# Index each workspace member
magellan watch --root ./forge_core/src --db .magellan/forge.db --debounce-ms 500
magellan watch --root ./forge_runtime/src --db .magellan/forge.db --debounce-ms 500
magellan watch --root ./forge_agent/src --db .magellan/forge.db --debounce-ms 500
magellan watch --root ./forge-reasoning/src --db .magellan/forge.db --debounce-ms 500
```

---

## Workspace Architecture

```
forge/                          # Workspace root
├── Cargo.toml                  # Workspace definition
├── AGENTS.md                   # Subagent rules (epistemic discipline)
├── forge_core/                 # Core SDK - graph, search, CFG, edit
│   └── src/
│       ├── graph/              # Symbol graph (queries.rs)
│       ├── analysis/           # Complexity, dead code, modules
│       ├── cfg/                # Control flow graph
│       ├── edit/               # Code editing operations
│       ├── search/             # Pattern-based code search
│       ├── storage/            # Storage backends
│       ├── treesitter/         # Tree-sitter parsing
│       └── types.rs            # Core types
├── forge_runtime/              # Runtime - indexing, caching, metrics
│   └── src/
│       ├── lib.rs              # Runtime interface
│       └── metrics.rs          # Performance metrics
├── forge_agent/                # Agent - workflow orchestration, AI loop
│   └── src/
│       ├── cli.rs              # CLI entry point
│       ├── loop.rs             # Agent execution loop
│       ├── planner.rs          # Task planning
│       ├── audit.rs            # Audit trail
│       ├── commit.rs           # Commit management
│       ├── mutate.rs           # Code mutation
│       ├── observe.rs          # Code observation
│       ├── policy.rs           # Policy enforcement
│       ├── transaction.rs      # Transaction management
│       ├── verify.rs           # Verification gates
│       └── workflow/           # Workflow engine (DAG, checkpoint, rollback, timeout, etc.)
├── forge-reasoning/            # Reasoning - debugging, hypotheses, contradictions
│   └── src/
│       ├── belief/             # Belief graph
│       ├── checkpoint/         # Debug checkpointing
│       ├── gaps/               # Knowledge gap analysis
│       ├── hypothesis/         # Hypothesis management
│       ├── impact/             # Impact analysis (preview, propagation, snapshot)
│       ├── verification/       # Verification runner (check, retry)
│       ├── service.rs          # Main service interface
│       ├── storage.rs          # Storage backends
│       ├── storage_sqlitegraph.rs  # sqlitegraph backend
│       ├── websocket.rs        # WebSocket interface
│       └── export_import.rs    # State export/import
```

---

## Mandatory Protocol

**Before ANY code change:**

1. **Check graph health**
   ```bash
   magellan status --db .magellan/forge.db
   ```

2. **Discover symbols before creating or changing them**
   ```bash
   llmgrep --db .magellan/forge.db search --query "symbol_name" --output human
   magellan find --db .magellan/forge.db --name "symbol_name"
   ```

3. **Trace relationships before signature changes**
   ```bash
   magellan refs --db .magellan/forge.db --name "func" --path "forge_core/src/graph/queries.rs" --direction in
   magellan refs --db .magellan/forge.db --name "func" --path "forge_core/src/graph/queries.rs" --direction out
   ```

4. **Analyze control flow (Rust only)**
   ```bash
   mirage --db .magellan/forge.db cfg --function "func_name"
   ```

5. **Check impact before refactoring**
   ```bash
   splice reachable --symbol "func_name" --path "src/file.rs" --db .magellan/forge.db
   ```

---

## Known Issues

- **License mismatch**: `forge_core`, `forge_runtime`, and `forge_agent` have `GPL-3.0-or-later` but should be `GPL-3.0 only`. The pre-commit hook will block until fixed.
- **Crate naming**: `forge-reasoning` should be `forgekit-reasoning` for consistency (requires new crate publish on crates.io, cannot rename in place).
- **34 clippy warnings** in forgekit-agent (mostly elided lifetimes) — should be addressed.

---

## When In Doubt

1. Read the source code
2. Check the graph database
3. Run tests
4. Document the decision
5. Ask for clarification

**DO NOT GUESS.**

# CLAUDE.md

Behavioral guidelines to reduce common LLM coding mistakes. Merge with project-specific instructions as needed.

**Tradeoff:** These guidelines bias toward caution over speed. For trivial tasks, use judgment.

## 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:
- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

## 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

## 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:
- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:
- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

## 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:
- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

---

**These guidelines are working if:** fewer unnecessary changes in diffs, fewer rewrites due to overcomplication, and clarifying questions come before implementation rather than after mistakes.
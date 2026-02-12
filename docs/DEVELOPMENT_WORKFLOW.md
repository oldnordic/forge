# Development Workflow

**Purpose**: Prevent code drift, database drift, and "works on my machine" syndrome.
**Status**: MANDATORY for all changes.

**Last Updated**: 2025-12-30

---

## The Golden Rule

> **NEVER write code based on assumptions. ALWAYS read source and query the graph first.**

---

## Workflow Diagram

```
┌─────────────────────────────────────────────────────────────┐
│ 1. UNDERSTAND (READ FIRST)                                 │
│    - Read source code                                       │
│    - Check database schema                                    │
│    - Read existing docs/                                    │
└────────────┬────────────────────────────────────────────────┘
             │
             ▼
┌─────────────────────────────────────────────────────────────┐
│ 2. PLAN (STORE DECISION)                                    │
│    - Architectural decision with reasoning                  │
│    - Alternatives considered                                │
│    - Trade-offs documented                                  │
└────────────┬────────────────────────────────────────────────┘
             │
             ▼
┌─────────────────────────────────────────────────────────────┐
│ 3. PROVE (TDD)                                               │
│    - Write failing test                                     │
│    - Show expected failure                                  │
└────────────┬────────────────────────────────────────────────┘
             │
             ▼
┌─────────────────────────────────────────────────────────────┐
│ 4. IMPLEMENT                                                │
│    - Write code to pass test                                │
│    - Use proper tools                                   │
└────────────┬────────────────────────────────────────────────┘
             │
             ▼
┌─────────────────────────────────────────────────────────────┐
│ 5. VERIFY                                                   │
│    - Show test passes with full output                      │
│    - Run cargo check / equivalent                           │
│    - Update documentation                                   │
└─────────────────────────────────────────────────────────────┘
```

---

## Step 1: UNDERSTAND (READ FIRST)

### Check Database Schema

Before any graph operation, verify the schema:

```bash
# Get actual schema
sqlite3 .forge/graph.db ".schema"

# Check row counts
sqlite3 .forge/graph.db "
SELECT 'graph_entities', COUNT(*) FROM graph_entities
UNION ALL
SELECT 'graph_edges', COUNT(*) FROM graph_edges
UNION ALL
SELECT 'cfg_blocks', COUNT(*) FROM cfg_blocks;
"

# Verify columns exist
sqlite3 .forge/graph.db "PRAGMA table_info(graph_entities);"
```

### Read Source Code

```bash
# Use Read tool, NOT cat
Read /home/feanor/Projects/forge/forge_core/src/graph/mod.rs

# Get specific line range
Read /home/feanor/Projects/forge/forge_core/src/graph/mod.rs:100-200
```

### Check Existing Tools

```bash
# What tools are available?
magellan --help
llmgrep --help
mirage --help
splice --help

# Check their capabilities
magellan status --db .forge/graph.db
```

---

## Step 2: PLAN (STORE DECISION)

### Decision Template

Document architectural decisions before coding:

```markdown
## Decision: [Title]

**Context**: What problem are we solving?

**Proposed Solution**: What are we changing?

**Alternatives Considered**:
- Alternative 1: [description]
- Alternative 2: [description]
- Why rejected: [reason]

**Trade-offs**:
- Benefit: [what we gain]
- Cost: [time/complexity]
- Risk: [what could break]

**Implementation Notes**:
- Files affected: [list]
- Tests needed: [list]
- Documentation updates: [list]
```

### For Bugfixes Specifically

```markdown
## Fix: [Bug Description]

**ROOT CAUSE**: [exact location, proven evidence]
**PROOF**: [error message, stack trace, test output]

**Fix**: [how this addresses root cause]

**Regression Risk**: [what could break]
**Mitigation**: [test coverage]
```

---

## Step 3: PROVE (TDD)

### Write Failing Test First

```rust
#[tokio::test]
async fn test_find_symbol_returns_exact_location() {
    // Given: A symbol exists in database
    let forge = setup_test_forge().await;
    let expected_id = insert_test_symbol(&forge, "main").await;

    // When: We query for it
    let result = forge.graph()
        .find_symbol("main")
        .await
        .unwrap();

    // Then: Should return symbol with correct location
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, expected_id);
    assert_eq!(result[0].name, "main");
}
```

### Run and Show Failure

```bash
$ cargo test test_find_symbol

FAILURES:
---- test_find_symbol_returns_exact_location stdout ----
thread panicked at 'assertion failed: `(left == right)`
  left: `0`,
  right: `1`
```

**This proves the test catches the bug.**

---

## Step 4: IMPLEMENT

### Use Proper Tools

| Task | Tool | Why |
|------|------|-----|
| Find symbols | `magellan find` | Exact byte spans |
| Find references | `magellan refs` | Cross-file aware |
| Search code | `llmgrep search` | AST-aware |
| CFG analysis | `mirage cfg` | Path-aware |
| Edit code | `splice patch` | Span-safe |
| Query graph | `sqlite3 .db` | Direct access |

### Write Code

```rust
// Implement fix based on ROOT CAUSE analysis
impl GraphModule {
    pub async fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>> {
        // PROVEN: This table exists (checked schema)
        // PROVEN: These columns exist (checked PRAGMA)
        let query = "
            SELECT id, name, kind, file_path, byte_start, byte_end
            FROM graph_entities
            WHERE name = ?
            ORDER BY file_path
        ";
        // ... implementation
    }
}
```

---

## Step 5: VERIFY

### Show Test Passes (Full Output)

```bash
$ cargo test test_find_symbol

running 1 test
test test_find_symbol_returns_exact_location ... ok

test result: ok. 1 passed; 0 failed; 0 ignored
```

### Run Compiler Check

```bash
$ cargo check
    Checking forge_core v0.1.0
    Finished `dev` profile
```

### Update Documentation

- Update ARCHITECTURE.md if design changed
- Update API.md if public API changed
- Update DEVELOPMENT_WORKFLOW.md if workflow changed
- Add/update examples in README.md

---

## File Size Limits

ForgeKit follows strict file size limits:

| Component | Limit | Rationale |
|------------|--------|------------|
| forge_core modules | 300 LOC | Maintainability |
| forge_runtime modules | 300 LOC | Single responsibility |
| forge_agent modules | 300 LOC | Focused behavior |
| Tests | 500 LOC | Comprehensive coverage |

**When to exceed:**
- Only with justification in comments
- Consider module extraction first
- File MUST be cohesive (single purpose)

---

## Anti-Patterns (DO NOT DO)

| ❌ Anti-Pattern | ✅ Correct Approach |
|------------------|-------------------|
| `grep "function_name"` | `magellan find --name "function_name"` |
| `cat file.rs` | `Read /path/to/file.rs` |
| Edit without reading | Read first, then Edit |
| Assume schema | `sqlite3 .db ".schema"` first |
| "I'll fix later" | Fix now or document debt |
| Comment out broken code | Delete or fix properly |
| `#[allow(...)]` | Fix the warning |
| TODO/FIXME in prod | Do it now or create issue |

---

## Quick Reference

### Database Commands

```bash
# Check schema
sqlite3 .forge/graph.db ".schema"

# Check specific table
sqlite3 .forge/graph.db "PRAGMA table_info(graph_entities);"

# Check row counts
sqlite3 .forge/graph.db "SELECT COUNT(*) FROM graph_entities;"

# Check indexes
sqlite3 .forge/graph.db ".indexes"

# Test query
sqlite3 .forge/graph.db "SELECT * FROM graph_entities WHERE name = 'main' LIMIT 5;"
```

### Tool Commands

```bash
# Magellan
magellan status --db .forge/graph.db
magellan find --db .forge/graph.db --name "symbol"
magellan refs --db .forge/graph.db --name "symbol" --direction in

# LLMGrep
llmgrep --db .forge/graph.db search --query "symbol" --output json

# Mirage
mirage --db .forge/graph.db cfg --function "symbol_name"
mirage --db .forge/graph.db paths --function "symbol_name"

# Splice
splice --db .forge/graph.db patch --file src/lib.rs --symbol symbol_name
splice --db .forge/graph.db delete --file src/lib.rs --symbol symbol_name
```

### Cargo Commands

```bash
# Build all workspace members
cargo build

# Build specific member
cargo build -p forge_core

# Run tests
cargo test

# Run tests for specific member
cargo test -p forge_core

# Check compilation (faster than build)
cargo check

# Format code
cargo fmt

# Lint
cargo clippy --all-targets

# Run benchmarks
cargo bench
```

---

## Remember

> **"Two days of debugging can save you ten minutes of planning."**

Plan first. Read source. Check schema. Store decision. Then code.

---

*Last updated: 2025-12-30*

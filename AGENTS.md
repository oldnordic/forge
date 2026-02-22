---
description: ForgeKit agent for unified code intelligence SDK. Enforces epistemic discipline - NEVER GUESS, ALWAYS VERIFY.
mode: subagent
temperature: 0.1
permission:
  edit: allow
  bash: allow
  webfetch: allow
tools:
  write: true
  edit: true
---

# Development Rules - NON-NEGOTIABLE

**Last Updated:** 2025-12-30
**Status:** MANDATORY for ALL code changes

---

## EPISTEMIC DISCIPLINE (NON-NEGOTIABLE)

### Rule #1: NEVER GUESS - ALWAYS VERIFY

Before ANY code change:

```bash
# 1. READ source code first
Read /path/to/file.rs

# 2. CHECK actual database schema
sqlite3 .forge/graph.db ".schema"

# 3. VERIFY with compiler/tests
cargo test
cargo check
```

**FORBIDDEN:**
- ❌ Assuming what a function does without reading it
- ❌ Guessing database table/column names
- ❌ Writing code based on "how it should work"
- ❌ Using grep/rg to find symbols (use graph queries instead)

---

### Rule #2: STORE ARCHITECTURAL DECISIONS

Before ANY code change, document decisions:

```markdown
## Decision: [Title]

**Context**: What problem are we solving?

**Proposed Solution**: What are we changing?

**Alternatives Considered**:
- Alternative 1: [description] - Why rejected
- Alternative 2: [description] - Why rejected

**Trade-offs**:
- Benefit: [what we gain]
- Cost: [time/complexity]
- Risk: [what could break]

**Proof**: [cite exact file:line numbers]
```

---

### Rule #3: TDD - PROVE IT FIRST

1. Write a test that FAILS for the expected reason
2. Show the failure output
3. Fix the code
4. Show the test now passes

```bash
cargo test test_name
```

---

### Rule #4: USE PROPER TOOLS

| Task | Use This | NEVER Use |
|------|----------|-----------|
| Find symbols | `forge.graph().find_symbol()` | grep/rg |
| Find references | `forge.graph().references()` | grep/rg |
| CFG queries | `forge.cfg().paths()` | Custom parsing |
| Edit code | `forge.edit().rename()` | sed |
| Read code | `Read` tool | cat/head/tail |

---

### Rule #4a: GRAPH-TOOLS-FIRST MANDATE (2026-02-22)

**When analyzing code, ALWAYS use graph tools before falling back to grep/file reads.**

#### Decision Matrix

| What you're looking for | Tool to use | Example |
|------------------------|-------------|---------|
| Function/struct by name | `llmgrep search --query <name>` | `llmgrep search --query forward_slice` |
| Symbol with specific ID | `magellan find --symbol-id <id>` | `magellan find --symbol-id abc123...` |
| All callers of a function | `mirage blast-zone --function <name>` | `mirage blast-zone --function main` |
| CFG structure | `mirage cfg --function <name>` | `mirage cfg --function process_request` |
| Execution paths | `mirage paths --function <name>` | `mirage paths --function handle_data` |
| Dead code detection | `mirage unreachable --function <name>` | `mirage unreachable --function old_func` |
| Impact analysis | `mirage slice --function <name>` | `mirage slice --function auth_check` |

#### Why Graph Tools Are Superior

**Using grep:**
```bash
$ grep -rn "forward_slice" src/
# Returns: 47 matches including:
# - Comments mentioning forward_slice
# - Variable names like forward_slice_result
# - String literals
# - False positives in unrelated files
# - No semantic context (is it a call? definition?)
```

**Using llmgrep:**
```bash
$ llmgrep search --query forward_slice --db codegraph.db
# Returns: 3 symbols with:
# - Exact type (Function)
# - File location + byte spans
# - Symbol ID for precise references
# - AST context
# - Complexity metrics
# - No false positives
```

#### Enforcement

**BEFORE using grep, ask yourself:**
1. Is the database indexed? (`magellan status --db .codemcp/codegraph.db`)
2. Can llmgrep/magellan find this symbol?
3. Do I need semantic context or just text?

**ONLY use grep when:**
- Searching for TODO/FIXME comments
- Finding string literals or log messages
- Pattern matching in non-code files
- The database doesn't exist yet

**Token Savings:**
- Manual grep + analysis: ~500-1000 tokens
- Graph tool query + structured result: ~100-200 tokens
- **Savings: 60-80% fewer tokens**

---

### Rule #5: CITE YOUR SOURCES

Before making changes, cite EXACTLY what you read:

```
I read /home/feanor/Projects/forge/forge_core/src/graph/mod.rs:123-456
The function `find_symbol` takes parameters X, Y, Z
I checked the graph.db schema
Table `graph_entities` has columns: id, name, kind, file_path, ...
Therefore I will change...
```

---

### Rule #6: NO DIRTY FIXES

- ❌ "TODO: fix later"
- ❌ `#[allow(dead_code)]` to silence warnings
- ❌ Commenting out broken code
- ❌ Minimal/half-hearted fixes

**ONLY**: Complete, tested, documented code.

---

## RUST-SPECIFIC STANDARDS

### Code Quality

- Max 300 LOC per file (600 with justification)
- No `unwrap()` in prod paths
- Proper error handling with `anyhow::Result`
- All public APIs must have rustdoc

### File Organization

```
forge_core/src/
├── lib.rs           # Public API, Forge type
├── types.rs         # Core types (≤300 LOC)
├── error.rs         # Error types (≤300 LOC)
├── graph/           # Graph operations
│   ├── mod.rs      # Module exports (≤300 LOC)
│   ├── symbols.rs   # Symbol queries (≤300 LOC)
│   └── references.rs # Reference queries (≤300 LOC)
├── search/          # Search operations
├── cfg/             # CFG operations
├── edit/            # Edit operations
└── analysis/        # Combined operations
```

---

## API Design Guidelines

### Forge Instance

The main entry point must be simple:

```rust
use forge::Forge;

let forge = Forge::open("./repo")?;

// Access modules
let graph = forge.graph();
let search = forge.search();
let cfg = forge.cfg();
let edit = forge.edit();
```

### Module Pattern

Each module follows the same pattern:

```rust
pub struct Module {
    store: Arc<UnifiedGraphStore>,
}

impl Module {
    // Query methods return Result
    pub fn query(&self) -> Result<Output>;

    // Builder pattern for complex queries
    pub fn builder(&self) -> Builder;
}
```

### Error Handling

```rust
// UseForgeError for public API
pub fn public_api() -> Result<Output, ForgeError> {
    // ...
}

// Use anyhow::Result for internal
pub fn internal_function() -> anyhow::Result<()> {
    // ...
}
```

---

## Testing Guidelines

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_specific_behavior() {
        // Given
        let input = create_test_input();

        // When
        let result = input.process();

        // Then
        assert!(result.is_ok());
        assert_eq!(result.unwrap().value, expected);
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_full_workflow() {
    let forge = Forge::open("./test-repo").await?;

    let symbols = forge.graph().find_symbol("main")?;
    assert!(!symbols.is_empty());
}
```

---

## When In Doubt

1. Read source code
2. Check database schema
3. Run tests
4. Store decision
5. Ask for clarification

**DO NOT GUESS.**

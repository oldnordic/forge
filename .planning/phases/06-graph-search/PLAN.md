---
phase: 06-graph-search
plan: 01
type: execute
wave: 1
depends_on: ["05-storage-layer"]
files_modified:
  - forge_core/src/graph/mod.rs
  - forge_core/src/search/mod.rs
  - forge_core/Cargo.toml
autonomous: true

must_haves:
  truths:
    - "Magellan provides call graph indexing and symbol navigation"
    - "LLMGrep provides semantic code search"
    - "Tools are invoked as external binaries or library APIs"
    - "GraphModule wraps Magellan operations"
    - "SearchModule wraps LLMGrep operations"
  artifacts:
    - path: forge_core/src/graph/mod.rs
      provides: "GraphModule with Magellan tool integration"
      exports: ["GraphModule", "find_symbol", "callers_of", "references", "reachable_from", "cycles"]
      covered_by: "Task 1"
    - path: forge_core/src/search/mod.rs
      provides: "SearchModule with LLMGrep tool integration"
      exports: ["SearchModule", "SearchBuilder", "pattern"]
      covered_by: "Task 2"
  key_links:
    - from: "forge_core/src/graph/mod.rs"
      to: "magellan binary"
      via: "std::process::Command"
    - from: "forge_core/src/search/mod.rs"
      to: "llmgrep binary"
      via: "std::process::Command"

truths:
    - "Magellan command: magellan --db <db> find --name <symbol>"
    - "Magellan command: magellan --db <db> refs --name <symbol> --direction <in|out>"
    - "Magellan command: magellan --db <db> cycles"
    - "LLMGrep command: llmgrep --db <db> search --query <pattern> --output json"
    - "Tool results are parsed from JSON output"
    - "Database path: .forge/graph.db"
  artifacts:
    - path: Cargo.toml
      provides: "Dependencies for tool integration (serde_json for parsing)"
      covered_by: "All tasks"
  key_links:
    - from: "forge_core/Cargo.toml"
      to: "serde_json crate"
      via: "dependencies section"

---

<objective>
Implement Phase 06: Graph & Search with Magellan and LLMGrep tool bindings.

**Goal**: Create functional graph and search modules using external tool binaries (Magellan, LLMGrep) as backend.

**Context**:
- Phase 05 (Storage Layer) is complete
- Magellan and LLMGrep are external tools installed via cargo
- Tools provide JSON output for programmatic access
- forge_core wraps these tools for a unified API

**Purpose**:
- Provide symbol navigation via Magellan integration
- Enable semantic code search via LLMGrep integration
- Maintain API compatibility with existing stub implementations

**Output**: Fully functional graph and search modules ready for use.

**Duration**: 1 week (per ROADMAP)
</objective>

<execution_context>
@/home/feanor/.claude/get-shit-done/workflows/plan-phase.md
@/home/feanor/.claude/get-shit-done/references/ui-brand.md
</execution_context>

<context>
@.planning/ROADMAP.md
@.planning/STATE.md
@.planning/requirements/v0.1-REQUIREMENTS.md
@.planning/codebase/ARCHITECTURE.md
@.planning/codebase/STACK.md
@.planning/codebase/INTEGRATIONS.md
@forge_core/src/lib.rs
@forge_core/src/types.rs
@forge_core/src/storage/mod.rs
@forge_core/src/graph/mod.rs
@forge_core/src/search/mod.rs
</context>

<tasks>

<task type="auto">
  <name>Task 1: Implement GraphModule with Magellan</name>
  <files>forge_core/src/graph/mod.rs</files>
  <action>
Replace stub GraphModule implementation with actual Magellan tool integration.

**Requirements:**
1. Invoke `magellan` binary as subprocess
2. Commands: `find --name`, `refs --name --direction`, `cycles`
3. Parse JSON output from Magellan
4. Convert to forge_core types (Symbol, Reference, Cycle)
5. Handle errors gracefully (tool not found, invalid output)
6. Add comprehensive tests (minimum 6 tests)

**Files to modify:**
- forge_core/src/graph/mod.rs (currently stub, ~240 LOC)

**API Surface:**
```rust
impl GraphModule {
    pub async fn find_symbol(&self, name: &str) -> Result<Vec<Symbol>>;
    pub async fn find_symbol_by_id(&self, id: SymbolId) -> Result<Symbol>;
    pub async fn callers_of(&self, name: &str) -> Result<Vec<Reference>>;
    pub async fn references(&self, name: &str) -> Result<Vec<Reference>>;
    pub async fn reachable_from(&self, id: SymbolId) -> Result<Vec<SymbolId>>;
    pub async fn cycles(&self) -> Result<Vec<Cycle>>;
}
```

**Magellan Command Patterns:**
```bash
# Find symbol by name
magellan --db .forge/graph.db find --name "symbol_name" --output json

# Find references (incoming calls)
magellan --db .forge/graph.db refs --name "symbol_name" --path "src/file.rs" --direction in --output json

# Find all references
magellan --db .forge/graph.db refs --name "symbol_name" --output json

# Find cycles
magellan --db .forge/graph.db cycles --output json
```

**Acceptance Criteria:**
- [ ] Magellan binary invoked via std::process::Command
- [ ] JSON output parsed with serde_json
- [ ] Results converted to forge_core types
- [ ] Error handling for tool not found
- [ ] find_symbol() returns matching symbols
- [ ] callers_of() returns incoming Call references
- [ ] references() returns all references
- [ ] cycles() returns detected cycles
- [ ] Unit tests (minimum 6 tests)
- [ ] Integration tests verify tool commands

**File Size Target**: Extend graph to ~400 LOC (was 240, tool integration adds ~160)
</action>
  <done>
forge_core/src/graph/mod.rs has Magellan integration with:
- std::process::Command for invoking magellan binary
- serde_json for parsing JSON output
- conversion to forge_core types (Symbol, Reference, Cycle)
- error handling for tool not found and invalid output
- find_symbol(), callers_of(), references() methods functional
- cycles() detection working
- at least 6 unit tests passing
</done>
  <verify>
Run: cargo test -p forge_core --lib graph
Expected: Graph module tests pass, Magellan integration working
</verify>
</task>

<task type="auto">
  <name>Task 2: Implement SearchModule with LLMGrep</name>
  <files>forge_core/src/search/mod.rs</files>
  <action>
Replace stub SearchModule implementation with actual LLMGrep tool integration.

**Requirements:**
1. Invoke `llmgrep` binary as subprocess
2. Commands: `search --query` with filters (kind, path, limit)
3. Parse JSON output from LLMGrep
4. Convert to forge_core Symbol type
5. Handle errors gracefully (tool not found, invalid output)
6. Add comprehensive tests (minimum 5 tests)

**Files to modify:**
- forge_core/src/search/mod.rs (currently stub, ~230 LOC)

**API Surface:**
```rust
impl SearchModule {
    pub fn symbol(&self, name: &str) -> SearchBuilder;
    pub async fn pattern(&self, pattern: &str) -> Result<Vec<Symbol>>;
}

impl SearchBuilder {
    pub fn kind(self, kind: SymbolKind) -> Self;
    pub fn file(self, path: &str) -> Self;
    pub fn limit(self, n: usize) -> Self;
    pub async fn execute(self) -> Result<Vec<Symbol>>;
}
```

**LLMGrep Command Patterns:**
```bash
# Basic search
llmgrep --db .forge/graph.db search --query "symbol_name" --output json

# With kind filter
llmgrep --db .forge/graph.db search --query "symbol_name" --kind Function --output json

# With path filter
llmgrep --db .forge/graph.db search --query "symbol_name" --path "src/" --output json

# With limit
llmgrep --db .forge/graph.db search --query "symbol_name" --limit 10 --output json
```

**Acceptance Criteria:**
- [ ] LLMGrep binary invoked via std::process::Command
- [ ] JSON output parsed with serde_json
- [ ] Results converted to forge_core Symbol type
- [ ] Error handling for tool not found
- [ ] symbol() creates SearchBuilder correctly
- [ ] kind() filter applied to command
- [ ] file() filter applied to command
- [ ] limit() applied to command
- [ ] execute() returns matching symbols
- [ ] Unit tests (minimum 5 tests)
- [ ] Integration tests verify tool commands

**File Size Target**: Extend search to ~350 LOC (was 230, tool integration adds ~120)
</action>
  <done>
forge_core/src/search/mod.rs has LLMGrep integration with:
- std::process::Command for invoking llmgrep binary
- serde_json for parsing JSON output
- conversion to forge_core Symbol type
- error handling for tool not found and invalid output
- SearchBuilder with kind/file/limit filters functional
- execute() returns matching symbols
- at least 5 unit tests passing
</done>
  <verify>
Run: cargo test -p forge_core --lib search
Expected: Search module tests pass, LLMGrep integration working
</verify>
</task>

<task type="auto">
  <name>Task 3: Add serde_json dependency</name>
  <files>forge_core/Cargo.toml</files>
  <action>
Add serde_json dependency for parsing tool output.

**Requirements:**
1. Add serde_json to dependencies
2. Enable serde feature on existing types
3. Verify build succeeds

**Files to modify:**
- forge_core/Cargo.toml

**Dependencies:**
```toml
[dependencies]
serde_json = "1"
```

**Acceptance Criteria:**
- [ ] serde_json added to Cargo.toml
- [ ] cargo build succeeds
- [ ] No version conflicts

**File Size Target**: +1 line to Cargo.toml
</action>
  <done>
serde_json dependency added to forge_core/Cargo.toml:
- version 1.x specified
- cargo build succeeds
- no dependency conflicts
</done>
  <verify>
Run: cargo build -p forge-core
Expected: Build succeeds, serde_json available
</verify>
</task>

</tasks>

<verification>
Overall phase checks:
- [ ] cargo build --workspace succeeds (all modules compile)
- [ ] cargo test --workspace passes (all tests pass)
- [ ] forge_core graph module fully functional with Magellan
- [ ] forge_core search module fully functional with LLMGrep
- [ ] External tool integration tested
- [ ] At least 11 tests added (6+5)
- [ ] Graph code ≤ 400 LOC
- [ ] Search code ≤ 350 LOC

**Success Criteria:**
Phase 06 complete when:
1. Magellan integration working for graph operations
2. LLMGrep integration working for search operations
3. All tests passing
4. Tool output parsed correctly
5. Error handling for missing tools
</success_criteria>

<output>
After execution, create `.planning/phases/06-graph-search/06-SUMMARY.md` with:
- List of tasks completed
- Tool integration summary
- Test coverage report
- Any API changes needed
</output>

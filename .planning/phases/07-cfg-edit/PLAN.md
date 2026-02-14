---
phase: 07-cfg-edit
plan: 01
type: execute
wave: 1
depends_on: ["06-graph-search"]
files_modified:
  - forge_core/src/cfg/mod.rs
  - forge_core/src/edit/mod.rs
  - forge_core/Cargo.toml
autonomous: true

must_haves:
  truths:
    - "Mirage provides CFG analysis and path enumeration for Rust"
    - "Splice provides span-safe code editing"
    - "Tools are invoked as external binaries or library APIs"
    - "CfgModule wraps Mirage operations"
    - "EditModule wraps Splice operations"
  artifacts:
    - path: forge_core/src/cfg/mod.rs
      provides: "CfgModule with Mirage tool integration"
      exports: ["CfgModule", "PathBuilder", "PathBuilder"]
      covered_by: "Task 1"
    - path: forge_core/src/edit/mod.rs
      provides: "EditModule with Splice tool integration"
      exports: ["EditModule", "InsertOperation", "DeleteOperation", "RenameOperation"]
      covered_by: "Task 2"
  key_links:
    - from: "forge_core/src/cfg/mod.rs"
      to: "mirage binary"
      via: "std::process::Command"
    - from: "forge_core/src/edit/mod.rs"
      to: "splice binary"
      via: "std::process::Command"

truths:
    - "Mirage command: mirage --db <db> cfg --function <name>"
    - "Mirage command: mirage --db <db> paths --function <name>"
    - "Mirage command: mirage --db <db> patterns --function <name>"
    - "Splice command: splice patch --file <path> --symbol <name> --with <patchfile>"
    - "Splice command: splice insert --file <path> --after <line> --content <file>"
    - "Splice command: splice delete --file <path> --symbol <name>"
    - "Tool results are parsed from JSON output"
    - "Database path: .forge/graph.db"
  artifacts:
    - path: Cargo.toml
      provides: "Dependencies for tool integration"
      covered_by: "All tasks"
  key_links:
    - from: "forge_core/Cargo.toml"
      to: "serde_json crate"
      via: "dependencies section"

---

<objective>
Implement Phase 07: CFG & Edit with Mirage and Splice tool bindings.

**Goal**: Create functional CFG and edit modules using external tool binaries (Mirage, Splice) as backend.

**Context**:
- Phase 06 (Graph & Search) is complete
- Mirage and Splice are external tools installed via cargo
- Tools provide JSON output for programmatic access
- forge_core wraps these tools for a unified API

**Purpose**:
- Provide CFG analysis via Mirage integration
- Enable span-safe code editing via Splice integration
- Maintain API compatibility with existing stub implementations

**Output**: Fully functional CFG and edit modules ready for use.

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
@forge_core/src/cfg/mod.rs
@forge_core/src/edit/mod.rs
</context>

<tasks>

<task type="auto">
  <name>Task 1: Implement CfgModule with Mirage</name>
  <files>forge_core/src/cfg/mod.rs</files>
  <action>
Replace stub CfgModule implementation with actual Mirage tool integration.

**Requirements:**
1. Invoke `mirage` binary as subprocess
2. Commands: `cfg --function`, `paths --function`, `patterns --function`
3. Parse JSON output from Mirage
4. Convert to forge_core types (Path, BlockId, Loop)
5. Handle errors gracefully (tool not found, invalid output)
6. Add comprehensive tests (minimum 6 tests)

**Files to modify:**
- forge_core/src/cfg/mod.rs (currently stub, ~100 LOC)

**API Surface:**
```rust
impl CfgModule {
    pub async fn cfg(&self, function: &str) -> Result<CfgGraph>;
    pub async fn paths(&self, function: &str) -> Result<Vec<Path>>;
    pub async fn patterns(&self, function: &str) -> Result<Vec<Pattern>>;
}
```

**Mirage Command Patterns:**
```bash
# Get CFG for function
mirage --db .forge/graph.db cfg --function "function_name" --output json

# Get all execution paths
mirage --db .forge/graph.db paths --function "function_name" --output json

# Get pattern matches (if/else, match)
mirage --db .forge/graph.db patterns --function "function_name" --output json
```

**Acceptance Criteria:**
- [ ] Mirage binary invoked via std::process::Command
- [ ] JSON output parsed with serde_json
- [ ] Results converted to forge_core types
- [ ] Error handling for tool not found
- [ ] cfg() returns CfgGraph with blocks and edges
- [ ] paths() returns all execution paths
- [ ] patterns() returns pattern matches
- [ ] Unit tests (minimum 6 tests)
- [ ] Integration tests verify tool commands

**File Size Target**: Extend cfg to ~300 LOC (was 100, tool integration adds ~200)
</action>
  <done>
forge_core/src/cfg/mod.rs has Mirage integration with:
- std::process::Command for invoking mirage binary
- serde_json for parsing JSON output
- conversion to forge_core types (Path, BlockId, Loop, Pattern)
- error handling for tool not found and invalid output
- cfg(), paths(), patterns() methods functional
- at least 6 unit tests passing
</done>
  <verify>
Run: cargo test -p forge_core --lib cfg
Expected: Cfg module tests pass, Mirage integration working
</verify>
</task>

<task type="auto">
  <name>Task 2: Implement EditModule with Splice</name>
  <files>forge_core/src/edit/mod.rs</files>
  <action>
Replace stub EditModule implementation with actual Splice tool integration.

**Requirements:**
1. Invoke `splice` binary as subprocess
2. Operations: patch, insert, delete, rename
3. Parse JSON output from Splice
4. Convert to forge_core types (EditResult)
5. Handle errors gracefully (tool not found, invalid output)
6. Add comprehensive tests (minimum 6 tests)

**Files to modify:**
- forge_core/src/edit/mod.rs (currently stub, ~200 LOC)

**API Surface:**
```rust
impl EditModule {
    pub async fn insert(&self, file: &str, line: usize, content: &str) -> Result<EditResult>;
    pub async fn delete(&self, file: &str, symbol: &str) -> Result<EditResult>;
    pub async fn rename(&self, file: &str, old: &str, new: &str) -> Result<EditResult>;
    pub async fn patch(&self, file: &str, symbol: &str, patch_file: &str) -> Result<EditResult>;
}
```

**Splice Command Patterns:**
```bash
# Insert content after line
splice insert --file <path> --after <line> --content <file>

# Delete symbol
splice delete --file <path> --symbol <name>

# Rename symbol
splice rename --file <path> --old <old_name> --new <new_name>

# Patch function with file
splice patch --file <path> --symbol <name> --with <patchfile> --output json
```

**Acceptance Criteria:**
- [ ] Splice binary invoked via std::process::Command
- [ ] JSON output parsed with serde_json
- [ ] Results converted to forge_core EditResult type
- [ ] Error handling for tool not found
- [ ] insert() adds content successfully
- [ ] delete() removes symbol successfully
- [ ] rename() renames symbol successfully
- [ ] patch() applies patch successfully
- [ ] Unit tests (minimum 6 tests)
- [ ] Integration tests verify tool commands

**File Size Target**: Extend edit to ~400 LOC (was 200, tool integration adds ~200)
</action>
  <done>
forge_core/src/edit/mod.rs has Splice integration with:
- std::process::Command for invoking splice binary
- serde_json for parsing JSON output
- conversion to forge_core EditResult type
- error handling for tool not found and invalid output
- insert(), delete(), rename(), patch() methods functional
- at least 6 unit tests passing
</done>
  <verify>
Run: cargo test -p forge_core --lib edit
Expected: Edit module tests pass, Splice integration working
</verify>
</task>

<task type="auto">
  <name>Task 3: Implement EditOperation trait methods</name>
  <files>forge_core/src/edit/mod.rs</files>
  <action>
Implement EditOperation trait methods for concrete operation types.

**Requirements:**
1. InsertOperation - insert content at location
2. DeleteOperation - delete symbol by name
3. RenameOperation - rename symbol with validation
4. ErrorResult - return edit conflicts
5. ApplyResult - confirmation of applied changes

**Files to modify:**
- forge_core/src/edit/mod.rs (extend for EditOperation impls)

**API Surface:**
```rust
impl EditOperation for InsertOperation {
    pub async fn apply(&self, module: &EditModule) -> Result<ApplyResult>;
}

impl EditOperation for DeleteOperation {
    pub async fn apply(&self, module: &EditModule) -> Result<ApplyResult>;
}

impl EditOperation for RenameOperation {
    pub async fn apply(&self, module: &EditModule) -> Result<ApplyResult>;
}
```

**Acceptance Criteria:**
- [ ] InsertOperation::apply() calls EditModule::insert()
- [ ] DeleteOperation::apply() calls EditModule::delete()
- [ ] RenameOperation::apply() calls EditModule::rename()
- [ ] Proper error propagation
- [ ] ApplyResult includes success/failure status
- [ ] Unit tests (minimum 4 tests)
- [ ] Integration tests verify operation workflow

**File Size Target**: Extend edit by ~100 LOC for trait impls
</action>
  <done>
forge_core/src/edit/mod.rs has EditOperation implementations with:
- InsertOperation::apply() inserts content at location
- DeleteOperation::apply() deletes symbol by name
- RenameOperation::apply() renames symbol with validation
- proper error propagation through EditModule
- ApplyResult includes success/failure status
- at least 4 unit tests passing
</done>
  <verify>
Run: cargo test -p forge_core --lib edit
Expected: EditOperation trait tests pass
</verify>
</task>

<task type="auto">
  <name>Task 4: Add edit workflow tests</name>
  <files>forge_core/src/edit/mod.rs</files>
  <action>
Add comprehensive tests for edit operations and workflows.

**Requirements:**
1. Test insert operation with valid/invalid inputs
2. Test delete operation (existing/nonexistent symbols)
3. Test rename operation (conflict detection, validation)
4. Test patch operation (before/after verification)
5. Test error scenarios (tool not found, conflicts)
6. Test workflow with multiple operations

**Files to modify:**
- forge_core/src/edit/mod.rs (add tests)

**Test Categories:**
```rust
#[tokio::test]
async fn test_insert_success() { ... }

#[tokio::test]
async fn test_insert_no_tool() { ... }

#[tokio::test]
async fn test_delete_existing() { ... }

#[tokio::test]
async fn test_rename_conflict() { ... }

#[tokio::test]
async fn test_patch_apply() { ... }
```

**Acceptance Criteria:**
- [ ] Insert success test passes
- [ ] Insert failure test passes (no tool)
- [ ] Delete existing symbol test passes
- [ ] Delete nonexistent test handles error
- [ ] Rename conflict test detected
- [ ] Rename validation test passes
- [ ] Patch apply test passes
- [ ] Workflow test passes (multi-operation)
- [ ] Error scenarios covered
- [ ] At least 6 tests added

**File Size Target**: Extend edit by ~150 LOC for tests
</action>
  <done>
forge_core/src/edit/mod.rs has comprehensive edit tests with:
- insert operation tests (success/failure)
- delete operation tests (existing/nonexistent)
- rename operation tests (conflict/validation)
- patch operation tests (apply/verify)
- workflow tests (multi-operation sequences)
- error scenario tests (tool not found)
- at least 6 tests passing
</done>
  <verify>
Run: cargo test -p forge_core --lib edit
Expected: All edit module tests pass
</verify>
</task>

</tasks>

<verification>
Overall phase checks:
- [ ] cargo build --workspace succeeds (all modules compile)
- [ ] cargo test --workspace passes (all tests pass)
- [ ] forge_core cfg module fully functional with Mirage
- [ ] forge_core edit module fully functional with Splice
- [ ] External tool integration tested
- [ ] At least 22 tests added (6+6+4+6)
- [ ] Cfg code ≤ 300 LOC
- [ ] Edit code ≤ 500 LOC
- [ ] mirage and splice tools integrated

**Success Criteria:**
Phase 07 complete when:
1. Mirage integration working for CFG operations
2. Splice integration working for edit operations
3. All tests passing
4. Tool output parsed correctly
5. Error handling for missing tools
6. EditOperation trait implemented
</success_criteria>

<output>
After execution, create `.planning/phases/07-cfg-edit/07-SUMMARY.md` with:
- List of tasks completed
- CFG and edit implementation summary
- Test coverage report
- Any API changes needed
</output>

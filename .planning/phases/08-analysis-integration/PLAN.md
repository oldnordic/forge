---
phase: 08-analysis-integration
plan: 01
type: execute
wave: 1
depends_on: ["07-cfg-edit"]
files_modified:
  - forge_core/src/analysis/mod.rs
  - forge_core/src/Cargo.toml
autonomous: true

must_haves:
  truths:
    - "AnalysisModule composites operations from all modules"
    - "End-to-end tests validate full workflow integration"
    - "Impact analysis uses graph, search, cfg, edit modules"
    - "Performance benchmarks measure operation costs"
    - "Example programs demonstrate capabilities"
  artifacts:
    - path: forge_core/src/analysis/mod.rs
      provides: "AnalysisModule with composite operations"
      exports: ["impact_analysis", "dead_code_detection", "reference_chain", "call_chain"]
      covered_by: "Task 1"
    - path: Cargo.toml
      provides: "Dependencies for analysis module"
      covered_by: "All tasks"
  key_links:
    - from: "forge_core/src/analysis/mod.rs"
      to: "forge_core/src/graph, search, cfg, edit modules"
      via: "use forge_core::*"
    - from: "forge_core/Cargo.toml"
      to: "serde_json crate"
      via: "dependencies section"

truths:
    - "Impact analysis uses graph, search, cfg, edit modules"
    - "Dead code detection finds unreferenced symbols"
    - "Reference chain traces call graph"
    - "Call chain traces callers to function"
    - "Performance benchmarks measure operation timing"
    - "Example programs show practical usage"

---
<objective>
Implement Phase 08: Analysis & Integration with end-to-end integration tests.

**Goal**: Create unified analysis module combining operations from GraphModule, SearchModule, CfgModule, and EditModule. Add comprehensive end-to-end integration tests.

**Context**:
- Phase 07 (CFG & Edit) is complete
- All individual modules (graph, search, cfg, edit) are functional
- Need composite operations that demonstrate integration
- End-to-end tests ensure full workflow validation

**Purpose**:
- Provide unified entry point for code analysis
- Enable impact analysis before making changes
- Detect dead code (unreferenced symbols)
- Trace reference and call chains
- Measure operation performance
- Demonstrate complete capabilities via examples

**Output**: Fully functional AnalysisModule with composite operations and comprehensive test coverage.

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
@forge_core/src/graph/mod.rs
@forge_core/src/search/mod.rs
@forge_core/src/cfg/mod.rs
@forge_core/src/edit/mod.rs
@forge_core/src/analysis/mod.rs
</context>

<tasks>

<task type="auto">
  <name>Task 1: Implement AnalysisModule composite operations</name>
  <files>forge_core/src/analysis/mod.rs</files>
  <action>
Create AnalysisModule with composite operations using individual modules.

**Requirements:**
1. Import graph, search, cfg, edit modules
2. Implement impact_analysis:
   - Finds all symbols referencing a target symbol
   - Uses graph module for call chains
   - Returns ImpactData with ref_count, call_count, etc.
3. Implement dead_code_detection:
   - Finds symbols with no incoming references
   - Returns list of unreferenced symbols
4. Implement reference_chain:
   - Traces all references from a symbol
   - Returns ordered list of symbols in reference chain
5. Implement call_chain:
   - Traces all callers to a function
   - Returns ordered list of calling symbols
6. Add performance benchmarks:
   - Measure timing for key operations
7. Add example programs:
   - Demonstrate impact analysis, dead code, chains
8. Add comprehensive tests (minimum 8 tests)

**Files to modify:**
- forge_core/src/analysis/mod.rs (currently stub, ~100 LOC)

**API Surface:**
```rust
impl AnalysisModule {
    pub async fn impact_analysis(&self, symbol: &str) -> Result<ImpactData>;
    pub async fn dead_code(&self) -> Result<Vec<Symbol>>;
    pub async fn reference_chain(&self, symbol: &str) -> Result<Vec<Symbol>>;
    pub async fn call_chain(&self, symbol: &str) -> Result<Vec<Symbol>>;
    pub async fn benchmarks(&self) -> Result<BenchmarkResults>;
}
```

**Acceptance Criteria:**
- [ ] AnalysisModule imports all sub-modules
- [ ] impact_analysis() returns ref_count, call_count
- [ ] dead_code_detection() returns unreferenced symbols
- [ ] reference_chain() returns ordered reference list
- [ ] call_chain() returns ordered caller list
- [ ] benchmarks() returns timing data
- [ ] Example programs demonstrating features
- [ ] Unit tests (minimum 8 tests)
- [ ] Integration tests verify cross-module functionality

**File Size Target**: ~300 LOC

</action>
  <done>
forge_core/src/analysis/mod.rs has AnalysisModule with:
- Impact analysis using graph/search/cfg/edit modules
- Dead code detection via unreferenced symbols
- Reference chain tracing via graph module
- Call chain tracing via graph module
- Performance benchmarks for key operations
- Example programs demonstrating features
- at least 8 unit tests passing
</done>
  <verify>
Run: cargo test -p forge_core --lib analysis
Expected: Analysis module tests pass, composite operations working
</verify>
</task>

<task type="auto">
  <name>Task 2: Implement EditOperation trait methods</name>
  <files>forge_core/src/analysis/mod.rs</files>
  <action>
Implement EditOperation trait with concrete operation types.

**Requirements:**
1. InsertOperation - inserts content at location
2. DeleteOperation - deletes symbol by name
3. RenameOperation - renames symbol with validation
4. ErrorResult - indicates operation failed (conflict, etc.)
5. ApplyResult - confirms operation was applied
6. Each operation has verify(), preview(), apply() methods
7. Operations use AnalysisModule for validation

**Files to modify:**
- forge_core/src/analysis/mod.rs (extend for EditOperation impls)

**API Surface:**
```rust
impl EditOperation for InsertOperation {
    pub fn verify(&self, module: &AnalysisModule) -> Result<ApplyResult>;
    pub fn preview(&self, module: &AnalysisModule) -> Result<Diff>;
    pub fn apply(&self, module: &mut AnalysisModule) -> Result<ApplyResult>;
}

impl EditOperation for DeleteOperation {
    pub fn verify(&self, module: &AnalysisModule) -> Result<ApplyResult>;
    pub fn preview(&self, module: &AnalysisModule) -> Result<Diff>;
    pub fn apply(&self, module: &mut AnalysisModule) -> Result<ApplyResult>;
}

impl EditOperation for RenameOperation {
    pub fn verify(&self, module: &AnalysisModule) -> Result<ApplyResult>;
    pub fn preview(&self, module: &AnalysisModule) -> Result<Diff>;
    pub fn apply(&self, module: &mut AnalysisModule) -> Result<ApplyResult>;
}

impl EditOperation for ErrorResult {
    // No verification needed, always fails
    pub fn verify(&self, _module: &AnalysisModule) -> Result<ApplyResult> { Ok(ApplyResult::AlwaysError) }
}
```

**Acceptance Criteria:**
- [ ] InsertOperation::verify() uses AnalysisModule for validation
- [ ] InsertOperation::preview() shows changes using Diff type
- [ ] InsertOperation::apply() executes via AnalysisModule
- [ ] DeleteOperation operations verified via AnalysisModule
- [ ] RenameOperation operations verified via AnalysisModule
- [ ] ErrorResult always returns failure
- [ ] Unit tests (minimum 4 tests)

**File Size Target**: Extend analysis by ~150 LOC for trait impls

</action>
  <done>
forge_core/src/analysis/mod.rs has EditOperation implementations with:
- InsertOperation using AnalysisModule for validation and execution
- DeleteOperation using AnalysisModule for validation
- RenameOperation using AnalysisModule for validation and execution
- ErrorResult with always-fail implementation
- All operations use AnalysisModule for validation
- at least 4 unit tests passing
</done>
  <verify>
Run: cargo test -p forge_core --lib analysis
Expected: EditOperation trait tests pass
</verify>
</task>

<task type="auto">
  <name>Task 3: Write end-to-end integration tests</name>
  <files>forge_core/src/analysis/mod.rs, tests/analysis_test.rs</files>
  <action>
Write comprehensive end-to-end integration tests for AnalysisModule.

**Requirements:**
1. Test impact_analysis with mock and real data
2. Test dead_code_detection with unreferenced symbols
3. Test reference_chain with mock dependencies
4. Test call_chain with mock call graph
5. Test EditOperation implementations
6. Test cross-module error handling
7. Test full workflow from symbol lookup to edit
8. Add performance benchmarks

**Files to modify:**
- forge_core/src/analysis/mod.rs (extend for tests)
- tests/analysis_test.rs (create new file)

**API Surface:**
```rust
// In analysis/mod.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_impact_analysis() { ... }

    #[tokio::test]
    async fn test_dead_code() { ... }

    #[tokio::test]
    async fn test_reference_chain() { ... }

    #[tokio::test]
    async fn test_call_chain() { ... }

    #[tokio::test]
    async fn test_edit_operation_insert() { ... }

    #[tokio::test]
    async fn test_edit_operation_delete() { ... }

    #[tokio::test]
    async fn test_edit_operation_rename() { ... }

    #[tokio::test]
    async fn test_full_workflow() { ... }
}
```

**Acceptance Criteria:**
- [ ] Impact analysis test with mock data
- [ ] Dead code detection test with unreferenced symbols
- [ ] Reference chain test with mock dependencies
- [ ] Call chain test with mock call graph
- [ ] EditOperation tests (insert, delete, rename)
- [ ] Full workflow test from lookup to edit
- [ ] Performance benchmarks included
- [ ] Unit tests (minimum 8 tests)
- [ ] Integration tests verify cross-module functionality

**File Size Target**: ~200 LOC in analysis/mod.rs + ~150 LOC in tests/analysis_test.rs

</action>
  <done>
forge_core/src/analysis/mod.rs has test module with:
- impact_analysis, dead_code_detection tests
- reference_chain, call_chain tests
- edit_operation tests (insert, delete, rename)
- full_workflow end-to-end test
- performance benchmarks
at least 8 unit tests passing
</done>
  <verify>
Run: cargo test -p forge_core --lib analysis
Expected: All analysis tests pass, end-to-end integration verified
</verify>
</task>

</tasks>

<verification>
Overall phase checks:
- [ ] cargo build --workspace succeeds (all modules compile)
- [ ] cargo test --workspace passes (all tests pass)
- [ ] forge_core analysis module fully functional
- [ ] EditOperation trait implemented by analysis module
- [ ] End-to-end integration tests written
- [ ] At least 20 tests added (8+4+8)
- [ ] Analysis code ≤ 350 LOC
- [ ] Integration tests verify cross-module functionality

**Success Criteria:**
Phase 08 complete when:
1. AnalysisModule with composite operations working
2. EditOperation trait implemented with concrete types
3. All tests passing
4. End-to-end workflows validated
5. Performance benchmarks demonstrate capabilities
6. All modules integrated properly

</success_criteria>

<output>
After execution, create `.planning/phases/08-analysis-integration/08-SUMMARY.md` with:
- List of tasks completed
- Analysis & Integration implementation summary
- Test coverage report
- Any API changes needed
</output>

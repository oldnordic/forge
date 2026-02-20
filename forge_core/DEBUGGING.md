# ForgeKit Debugging Guide - When Things Go Wrong

> **Golden Rule**: When in doubt, `forge checkpoint restore`. Then investigate.

---

## Emergency Procedures

### 🔥 The LLM Just Broke Everything

```bash
# 1. STOP - Don't panic
# 2. List recent checkpoints
forge checkpoint list

# 3. Restore to last known good state
forge checkpoint restore checkpoint_5

# 4. Verify restoration
forge verify --all

# 5. Now investigate what went wrong
forge analysis --since-checkpoint checkpoint_5
```

### 🕵️ The LLM Claims Something That's Not True

```rust
use forge_core::Forge;

let forge = Forge::open("./my-project").await?;

// LLM: "This function handles all error cases"
let cfg = forge.cfg()
    .extract_function_cfg(Path::new("src/lib.rs"), "suspicious_func")
    .await?;

if let Some(cfg) = cfg {
    // Check reality
    let paths = cfg.enumerate_paths();
    let error_paths = paths.iter().filter(|p| p.is_error()).count();
    let normal_paths = paths.iter().filter(|p| p.is_normal()).count();
    
    println!("Total paths: {}", paths.len());
    println!("Error paths: {}", error_paths);
    println!("Normal paths: {}", normal_paths);
    
    // LLM said "handles all errors" but CFG shows no error path?
    if error_paths == 0 {
        println!("🚨 CONTRADICTION: LLM claim doesn't match CFG!");
        
        // Check for unreachable code
        let dead_code = cfg.find_unreachable_blocks();
        if !dead_code.is_empty() {
            println!("Found {} unreachable blocks", dead_code.len());
        }
    }
}
```

---

## Common Issues

### Issue: LLM Removed Error Handling

**Symptoms**: Tests pass, but edge cases fail in production.

**Diagnosis**:
```rust
// Before and after comparison
let before_cfg = forge.cfg()
    .extract_function_cfg(path, "function_before")
    .await?;
let after_cfg = forge.cfg()
    .extract_function_cfg(path, "function_after")
    .await?;

let before_error_paths = before_cfg.count_error_paths();
let after_error_paths = after_cfg.count_error_paths();

if after_error_paths < before_error_paths {
    println!("🚨 Error paths reduced: {} -> {}", 
        before_error_paths, after_error_paths);
}
```

**Fix**: 
```bash
# Restore from checkpoint
forge checkpoint restore <before_change>

# Or manually inspect
forge cfg show --function function_after --format graphviz
```

### Issue: LLM Introduced Infinite Loop

**Symptoms**: Program hangs, CPU at 100%.

**Diagnosis**:
```rust
let cfg = forge.cfg()
    .extract_function_cfg(path, "suspicious_loop")
    .await?;

if let Some(cfg) = cfg {
    let loops = cfg.detect_loops();
    
    for loop_info in loops {
        println!("Loop detected:");
        println!("  Header: {:?}", loop_info.header);
        println!("  Blocks: {:?}", loop_info.blocks);
        println!("  Depth: {}", loop_info.depth);
        
        // Check if there's an exit path
        let has_exit = cfg.has_exit_from_loop(loop_info.header);
        if !has_exit {
            println!("🚨 POTENTIAL INFINITE LOOP: No exit from loop!");
        }
    }
}
```

### Issue: LLM Claimed Dead Code is Live

**Symptoms**: "This code is used" - but is it?

**Diagnosis**:
```rust
// Check if function is actually called
let callers = forge.graph()
    .callers_of("maybe_dead_function")
    .await?;

if callers.is_empty() {
    println!("🚨 Function 'maybe_dead_function' has no callers!");
    println!("   LLM claim: 'This is used' -> CONTRADICTION");
}

// Check if code is reachable from entry
let cfg = forge.cfg()
    .extract_function_cfg(path, "main")
    .await?;

let unreachable = cfg.find_unreachable_blocks();
for block in unreachable {
    println!("Unreachable block: {:?}", block);
}
```

### Issue: Hypothesis Without Evidence

**Symptoms**: "I optimized this" - but where's the proof?

**Diagnosis**:
```rust
// Find all changes without evidence
let suspect = forge.analysis()
    .find_changes_without_hypothesis()
    .await?;

for change in suspect {
    println!("🚨 SUSPECT CHANGE: {}", change.description);
    println!("   No hypothesis attached!");
    println!("   Files: {:?}", change.files);
}
```

---

## Debugging Workflows

### Workflow 1: Verify a Specific LLM Claim

```rust
// LLM: "This refactor reduces cyclomatic complexity from 15 to 5"

// Step 1: Extract CFGs
let old_cfg = forge.cfg().extract_function_cfg(path, "old_func").await?;
let new_cfg = forge.cfg().extract_function_cfg(path, "new_func").await?;

// Step 2: Calculate complexity
let old_complexity = old_cfg.cyclomatic_complexity();
let new_complexity = new_cfg.cyclomatic_complexity();

// Step 3: Verify claim
if new_complexity >= old_complexity {
    println!("🚨 CLAIM FALSE: Complexity {} -> {}", 
        old_complexity, new_complexity);
} else {
    println!("✅ Claim verified: {} -> {}", old_complexity, new_complexity);
}

// Step 4: Check paths weren't just deleted
let old_paths = old_cfg.enumerate_paths();
let new_paths = new_cfg.enumerate_paths();

if new_paths.len() < old_paths.len() / 2 {
    println!("⚠️  Warning: {} paths reduced to {} - functionality may be lost",
        old_paths.len(), new_paths.len());
}
```

### Workflow 2: Trace Through Control Flow

```rust
// Debug: "Why isn't this code path taken?"

let cfg = forge.cfg()
    .extract_function_cfg(path, "buggy_function")
    .await?;

// Enumerate all paths
let paths = cfg.enumerate_paths();

for (i, path) in paths.iter().enumerate() {
    println!("Path {}: {:?}", i, path.blocks);
    
    // Check if target block is reached
    if path.contains(BlockId(5)) {
        println!("  -> Contains target block 5");
    }
}

// Check dominators
let dom_tree = cfg.compute_dominators();

// Block 5 is never reached? Check what dominates it
let idom = dom_tree.immediate_dominator(BlockId(5));
println!("Block 5 immediate dominator: {:?}", idom);

// Check if dominator condition is ever true
println!("Block {:?} must execute before block 5", idom);
```

### Workflow 3: Compare Before/After States

```rust
use forge_reasoning::Checkpoint;

// Load two checkpoints
let before = Checkpoint::load("checkpoint_5").await?;
let after = Checkpoint::load("checkpoint_6").await?;

// Compare symbol graphs
let diff = forge.analysis()
    .diff_checkpoints(&before, &after)
    .await?;

println!("Changes between checkpoints:");
println!("  Functions added: {}", diff.added_functions.len());
println!("  Functions removed: {}", diff.removed_functions.len());
println!("  Functions modified: {}", diff.modified_functions.len());

// Check specific function
if let Some(func_diff) = diff.get_function("main") {
    println!("\nMain function changes:");
    println!("  Complexity: {} -> {}", 
        func_diff.old_complexity, func_diff.new_complexity);
    println!("  Paths: {} -> {}", 
        func_diff.old_paths, func_diff.new_paths);
}
```

---

## Using Checkpoints for Bisection

When you don't know when the bug was introduced:

```bash
# List all checkpoints
forge checkpoint list

# Binary search approach
forge checkpoint restore checkpoint_5  # Known good
# Test...

forge checkpoint restore checkpoint_8  # Known bad
# Test...

forge checkpoint restore checkpoint_6  # Middle
# Test...

# Narrow down to first bad checkpoint
```

Or programmatically:

```rust
use forge_reasoning::Checkpoint;

let checkpoints = Checkpoint::list_all().await?;
let (mut good, mut bad) = (0, checkpoints.len() - 1);

while good < bad - 1 {
    let mid = (good + bad) / 2;
    checkpoints[mid].restore().await?;
    
    if run_tests().await? {
        good = mid;
        println!("Checkpoint {} is good", mid);
    } else {
        bad = mid;
        println!("Checkpoint {} is bad", mid);
    }
}

println!("First bad checkpoint: {}", bad);
```

---

## Log Analysis

### Reading the Audit Log

```bash
# Show all hypothesis changes
forge log --hypotheses

# Show changes without evidence
forge log --unverified

# Show contradictions detected
forge log --contradictions

# Show specific time range
forge log --since "2026-02-18T18:00:00" --until "2026-02-18T19:00:00"
```

### Programmatic Log Analysis

```rust
// Find all "suspect" changes in last hour
let recent = forge.analysis()
    .changes_in_last(Duration::from_secs(3600))
    .await?;

for change in recent {
    if change.has_hypothesis() {
        let hypo = change.hypothesis();
        if hypo.confidence() < Confidence::Medium {
            println!("⚠️  Low confidence change: {}", change.description);
        }
    } else {
        println!("🚨 No hypothesis for: {}", change.description);
    }
}
```

---

## Emergency Recovery Checklist

When everything is on fire:

- [ ] **Don't panic** - Checkpoints exist for this reason
- [ ] **List checkpoints**: `forge checkpoint list`
- [ ] **Identify last known good**: Look at timestamps
- [ ] **Restore**: `forge checkpoint restore <id>`
- [ ] **Verify**: `forge verify --all`
- [ ] **Document**: Note what went wrong for future prevention
- [ ] **Investigate**: Use `forge analysis` to understand the failure

### After Recovery

```bash
# Create new checkpoint at stable state
forge checkpoint create "Stable after recovery from bug"

# Mark bad checkpoint as "do not use"
forge checkpoint tag <bad_id> --label "BROKEN: introduced infinite loop"

# Generate incident report
forge report --from <bad_id> --to <recovery_id> > incident_report.md
```

---

## Advanced Debugging

### Custom Contradiction Detectors

```rust
// Define custom verification rule
let detector = forge.analysis()
    .contradiction_detector()
    .rule(|cfg| {
        // Custom rule: All public functions must have error paths
        if cfg.is_public() && cfg.count_error_paths() == 0 {
            Some("Public function lacks error handling".to_string())
        } else {
            None
        }
    });

// Run on all functions
let issues = detector.scan_all().await?;
```

### Export for External Analysis

```bash
# Export CFG as GraphViz
forge cfg export --function main --format dot > main.dot
dot -Tpng main.dot -o main.png

# Export checkpoint diff
forge checkpoint diff checkpoint_5 checkpoint_6 --format json > changes.json
```

---

## Getting Help

When all else fails:

1. **Check the E2E tests**: `cargo test --test e2e_tests`
2. **Verify installation**: `forge doctor`
3. **Check logs**: `forge log --errors`
4. **Create minimal reproduction**: Isolate the issue
5. **File issue**: Include checkpoint ID, hypothesis ID, and CFG analysis

**Remember**: The checkpoint is your friend. When in doubt, restore and investigate.

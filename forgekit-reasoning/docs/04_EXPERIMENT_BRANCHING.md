# Experiment Branching

**Status**: Design Phase  
**Priority**: P1 - Enables parallel debugging strategies  
**Related**: Temporal Checkpointing (saves branches), Hypothesis Board (tracks branch-specific hypotheses)

---

## Problem Statement

Debugging often requires trying multiple approaches:
- "What if the offset is calculated wrong?" → Try fix A
- "What if dequantization is wrong?" → Try fix B
- "What if we skip normalization?" → Try fix C

Without branching:
1. **Sequential only** - Try A, revert, try B, revert, try C
2. **Lost work** - Good ideas abandoned when they don't immediately work
3. **No comparison** - Can't see which approach was best
4. **Fear of exploration** - "I don't want to mess up my current progress"

---

## Design Goals

1. **Git-like branching** for debugging state
2. **Lightweight snapshots** - Fast branch creation
3. **Easy comparison** - Diff between branches
4. **Merge capability** - Combine successful experiments
5. **Pruning** - Delete failed experiment branches

---

## Core Types

```rust
/// A branch in the debugging experiment tree
#[derive(Clone, Debug)]
pub struct ExperimentBranch {
    pub id: BranchId,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub parent_id: Option<BranchId>,
    pub base_checkpoint: CheckpointId,
    pub head_checkpoint: CheckpointId,
    pub status: BranchStatus,
    pub metadata: BranchMetadata,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BranchStatus {
    Active,      // Currently being worked on
    Merged,      // Successfully merged into parent
    Abandoned,   // Failed experiment, kept for reference
    Archived,    // Old, not relevant anymore
}

#[derive(Clone, Debug)]
pub struct BranchMetadata {
    pub hypothesis_being_tested: Option<HypothesisId>,
    pub approach_description: String,
    pub success_criteria: Vec<String>,
    pub results_summary: Option<String>,
}

/// A checkpoint captures full debugging state
#[derive(Clone, Debug)]
pub struct Checkpoint {
    pub id: CheckpointId,
    pub branch_id: BranchId,
    pub created_at: DateTime<Utc>,
    pub message: String,
    
    /// Captured state components
    pub state: DebugState,
}

/// Full debugging state that can be snapshotted
#[derive(Clone, Debug)]
pub struct DebugState {
    /// Code state (if modified)
    pub code_state: Option<CodeSnapshot>,
    
    /// Hypothesis board state
    pub hypotheses: Vec<Hypothesis>,
    pub evidence: Vec<Evidence>,
    
    /// Beliefs held at this point
    pub beliefs: Vec<Belief>,
    
    /// Verification results
    pub verification_results: Vec<VerificationResult>,
    
    /// User notes and observations
    pub notes: Vec<DebugNote>,
    
    /// Environment state
    pub env_vars: HashMap<String, String>,
    pub working_dir: PathBuf,
}

/// A note taken during debugging
#[derive(Clone, Debug)]
pub struct DebugNote {
    pub timestamp: DateTime<Utc>,
    pub content: String,
    pub tags: Vec<String>,
    pub related_hypotheses: Vec<HypothesisId>,
}

/// Manager for experiment branches
pub struct ExperimentManager {
    storage: Arc<dyn ExperimentStorage>,
    current_branch: BranchId,
}

/// Reference to a branch comparison
#[derive(Clone, Debug)]
pub struct BranchComparison {
    pub branch_a: BranchId,
    pub branch_b: BranchId,
    pub common_ancestor: CheckpointId,
    
    /// Differences in hypotheses
    pub hypothesis_diff: Vec<HypothesisDiff>,
    
    /// Differences in beliefs
    pub belief_diff: Vec<BeliefDiff>,
    
    /// Differences in verification results
    pub verification_diff: Vec<VerificationDiff>,
    
    /// Differences in code (if any)
    pub code_diff: Option<String>,
}

#[derive(Clone, Debug)]
pub enum HypothesisDiff {
    Added { hypothesis: Hypothesis },
    Removed { hypothesis_id: HypothesisId },
    ConfidenceChanged { hypothesis_id: HypothesisId, old: f64, new: f64 },
    StatusChanged { hypothesis_id: HypothesisId, old: HypothesisStatus, new: HypothesisStatus },
}
```

---

## Experiment Manager API

```rust
impl ExperimentManager {
    /// Create the main/debugging branch
    pub fn initialize(main_branch_name: &str) -> Result<Self> {
        let storage = Arc::new(SqliteExperimentStorage::new(".forge/experiments.db")?);
        
        // Create initial checkpoint with empty state
        let initial_checkpoint = Checkpoint {
            id: CheckpointId::new(),
            branch_id: BranchId::default(),
            created_at: Utc::now(),
            message: "Initial state".to_string(),
            state: DebugState::empty(),
        };
        
        storage.store_checkpoint(&initial_checkpoint)?;
        
        let main_branch = ExperimentBranch {
            id: BranchId::default(),
            name: main_branch_name.to_string(),
            description: "Main debugging branch".to_string(),
            created_at: Utc::now(),
            parent_id: None,
            base_checkpoint: initial_checkpoint.id,
            head_checkpoint: initial_checkpoint.id,
            status: BranchStatus::Active,
            metadata: BranchMetadata::default(),
        };
        
        storage.store_branch(&main_branch)?;
        
        Ok(Self {
            storage,
            current_branch: main_branch.id,
        })
    }
    
    /// Create a new branch from current state
    pub fn branch(&self, name: &str, description: &str) -> Result<BranchId> {
        let current = self.storage.get_branch(self.current_branch)?;
        let current_checkpoint = self.storage.get_checkpoint(current.head_checkpoint)?;
        
        let new_branch = ExperimentBranch {
            id: BranchId::new(),
            name: name.to_string(),
            description: description.to_string(),
            created_at: Utc::now(),
            parent_id: Some(self.current_branch),
            base_checkpoint: current.head_checkpoint,
            head_checkpoint: current.head_checkpoint,  // Starts at same point
            status: BranchStatus::Active,
            metadata: BranchMetadata::default(),
        };
        
        self.storage.store_branch(&new_branch)?;
        
        // Automatically switch to new branch
        self.switch_branch(new_branch.id)?;
        
        Ok(new_branch.id)
    }
    
    /// Switch to a different branch
    pub fn switch_branch(&mut self, branch_id: BranchId) -> Result<()> {
        let branch = self.storage.get_branch(branch_id)?;
        let checkpoint = self.storage.get_checkpoint(branch.head_checkpoint)?;
        
        // Restore state from checkpoint
        self.restore_state(&checkpoint.state)?;
        
        self.current_branch = branch_id;
        
        Ok(())
    }
    
    /// Create a checkpoint on current branch
    pub fn checkpoint(&self, message: &str) -> Result<CheckpointId> {
        let state = self.capture_current_state()?;
        
        let checkpoint = Checkpoint {
            id: CheckpointId::new(),
            branch_id: self.current_branch,
            created_at: Utc::now(),
            message: message.to_string(),
            state,
        };
        
        self.storage.store_checkpoint(&checkpoint)?;
        
        // Update branch head
        self.storage.update_branch_head(self.current_branch, checkpoint.id)?;
        
        Ok(checkpoint.id)
    }
    
    /// Compare two branches
    pub fn compare(&self, branch_a: BranchId, branch_b: BranchId) -> Result<BranchComparison> {
        let a = self.storage.get_branch(branch_a)?;
        let b = self.storage.get_branch(branch_b)?;
        
        let checkpoint_a = self.storage.get_checkpoint(a.head_checkpoint)?;
        let checkpoint_b = self.storage.get_checkpoint(b.head_checkpoint)?;
        
        // Find common ancestor
        let ancestor = self.find_common_ancestor(branch_a, branch_b)?;
        
        // Compute diffs
        let hypothesis_diff = self.diff_hypotheses(
            &checkpoint_a.state.hypotheses,
            &checkpoint_b.state.hypotheses,
        );
        
        let belief_diff = self.diff_beliefs(
            &checkpoint_a.state.beliefs,
            &checkpoint_b.state.beliefs,
        );
        
        let verification_diff = self.diff_verifications(
            &checkpoint_a.state.verification_results,
            &checkpoint_b.state.verification_results,
        );
        
        Ok(BranchComparison {
            branch_a,
            branch_b,
            common_ancestor: ancestor,
            hypothesis_diff,
            belief_diff,
            verification_diff,
            code_diff: None,  // Would need git integration
        })
    }
    
    /// Merge a branch back into its parent
    pub fn merge(&self, branch_id: BranchId, strategy: MergeStrategy) -> Result<MergeResult> {
        let branch = self.storage.get_branch(branch_id)?;
        
        if branch.status != BranchStatus::Active {
            return Err(ExperimentError::BranchNotActive);
        }
        
        let parent_id = branch.parent_id
            .ok_or(ExperimentError::NoParentBranch)?;
        
        let parent = self.storage.get_branch(parent_id)?;
        let branch_checkpoint = self.storage.get_checkpoint(branch.head_checkpoint)?;
        let parent_checkpoint = self.storage.get_checkpoint(parent.head_checkpoint)?;
        
        // Apply merge strategy
        let merged_state = match strategy {
            MergeStrategy::TakeTheirs => branch_checkpoint.state.clone(),
            MergeStrategy::TakeOurs => parent_checkpoint.state.clone(),
            MergeStrategy::Union => self.merge_union(
                &parent_checkpoint.state,
                &branch_checkpoint.state,
            )?,
            MergeStrategy::Interactive => {
                // Would require user interaction
                return Err(ExperimentError::InteractiveMergeNotImplemented);
            }
        };
        
        // Create merge checkpoint on parent branch
        let merge_checkpoint = Checkpoint {
            id: CheckpointId::new(),
            branch_id: parent_id,
            created_at: Utc::now(),
            message: format!("Merge branch '{}' into {}", branch.name, parent.name),
            state: merged_state,
        };
        
        self.storage.store_checkpoint(&merge_checkpoint)?;
        self.storage.update_branch_head(parent_id, merge_checkpoint.id)?;
        
        // Mark branch as merged
        self.storage.update_branch_status(branch_id, BranchStatus::Merged)?;
        
        Ok(MergeResult {
            merge_checkpoint_id: merge_checkpoint.id,
            parent_branch_id: parent_id,
        })
    }
    
    /// Abandon a branch (keep for reference but mark as failed)
    pub fn abandon(&self, branch_id: BranchId, reason: &str) -> Result<()> {
        self.storage.update_branch_status(branch_id, BranchStatus::Abandoned)?;
        self.storage.update_branch_metadata(branch_id, |meta| {
            meta.results_summary = Some(format!("Abandoned: {}", reason));
        })?;
        Ok(())
    }
    
    /// List all branches
    pub fn list_branches(&self) -> Result<Vec<BranchSummary>> {
        self.storage.get_all_branches()
    }
    
    /// Get current branch info
    pub fn current(&self) -> Result<ExperimentBranch> {
        self.storage.get_branch(self.current_branch)
    }
    
    /// Show branch history (like git log)
    pub fn history(&self, branch_id: Option<BranchId>) -> Result<Vec<CheckpointSummary>> {
        let branch_id = branch_id.unwrap_or(self.current_branch);
        self.storage.get_branch_history(branch_id)
    }
}
```

---

## Merge Strategies

```rust
#[derive(Clone, Copy, Debug)]
pub enum MergeStrategy {
    /// Take all state from the branch being merged
    TakeTheirs,
    
    /// Keep all state from the current (parent) branch
    TakeOurs,
    
    /// Union of both states (combine hypotheses, beliefs, etc.)
    Union,
    
    /// User decides each conflict
    Interactive,
}

impl ExperimentManager {
    /// Merge two states by taking the union
    fn merge_union(&self, parent: &DebugState, branch: &DebugState) -> Result<DebugState> {
        // Combine hypotheses - if same ID, take the one with more evidence
        let mut merged_hypotheses: HashMap<HypothesisId, Hypothesis> = HashMap::new();
        
        for h in &parent.hypotheses {
            merged_hypotheses.insert(h.id, h.clone());
        }
        
        for h in &branch.hypotheses {
            if let Some(existing) = merged_hypotheses.get(&h.id) {
                // Keep the one with more evidence
                if h.confidence > existing.confidence {
                    merged_hypotheses.insert(h.id, h.clone());
                }
            } else {
                merged_hypotheses.insert(h.id, h.clone());
            }
        }
        
        // Combine beliefs - latest timestamp wins for same statement
        let mut merged_beliefs: HashMap<String, Belief> = HashMap::new();
        
        for b in parent.beliefs.iter().chain(branch.beliefs.iter()) {
            let key = b.statement.clone();
            if let Some(existing) = merged_beliefs.get(&key) {
                if b.timestamp > existing.timestamp {
                    merged_beliefs.insert(key, b.clone());
                }
            } else {
                merged_beliefs.insert(key, b.clone());
            }
        }
        
        // Combine verification results - deduplicate by plan_id
        let mut seen_verifications = HashSet::new();
        let mut merged_verifications = Vec::new();
        
        for v in parent.verification_results.iter().chain(branch.verification_results.iter()) {
            if seen_verifications.insert(v.plan_id) {
                merged_verifications.push(v.clone());
            }
        }
        
        Ok(DebugState {
            code_state: branch.code_state.clone().or(parent.code_state.clone()),
            hypotheses: merged_hypotheses.into_values().collect(),
            evidence: parent.evidence.clone(),  // Evidence is append-only
            beliefs: merged_beliefs.into_values().collect(),
            verification_results: merged_verifications,
            notes: parent.notes.iter()
                .chain(branch.notes.iter())
                .cloned()
                .collect(),
            env_vars: branch.env_vars.clone(),  // Take latest env
            working_dir: branch.working_dir.clone(),
        })
    }
}
```

---

## Real-World Example (ROCmForge Debugging)

```rust
let mut manager = ExperimentManager::initialize("main")?;

// Checkpoint current understanding
manager.checkpoint("Initial analysis: Layer 2 weights 6.7x larger than Layer 0")?;

// Create branch to test "offset calculation is wrong"
let offset_branch = manager.branch(
    "fix-offset",
    "Try alternative tensor offset calculation"
)?;

// In this branch, we modify the offset calculation
// ... (make code changes, add beliefs)

manager.checkpoint("Changed offset to absolute from tensor_data_start")?;

// Run verifications in this branch
let verify_result = runner.run_suite(&offset_suite).await?;

if verify_result.all_passed() {
    // This approach worked!
    manager.current().metadata.results_summary = Some(
        "Offset fix resolved weight loading issue".to_string()
    );
    
    // Merge back to main
    manager.switch_branch(BranchId::default())?;
    manager.merge(offset_branch, MergeStrategy::TakeTheirs)?;
} else {
    // This approach failed
    manager.abandon(offset_branch, "Offset fix did not resolve the issue")?;
    
    // Switch back to main and try something else
    manager.switch_branch(BranchId::default())?;
    
    // Create another branch for different approach
    let dequant_branch = manager.branch(
        "fix-dequant",
        "Try alternative Q4_0 dequantization"
    )?;
    
    // ... continue experimenting
}

// Compare branches to see what was tried
let comparison = manager.compare(offset_branch, dequant_branch)?;
println!("Branch differences:");
for diff in comparison.hypothesis_diff {
    println!("  {:?}", diff);
}
```

---

## CLI Integration

```bash
# Show current branch
forge experiment status

# Create new branch
forge experiment branch fix-normalization \
    --description "Try skipping weight normalization"

# List all branches
forge experiment list

# Switch branch
forge experiment switch fix-normalization

# Checkpoint current state
forge experiment checkpoint "Before trying normalization skip"

# Show branch history
forge experiment log

# Compare two branches
forge experiment diff main fix-normalization

# Merge successful branch
forge experiment merge fix-normalization --strategy take-theirs

# Abandon failed branch
forge experiment abandon wrong-offset-theory --reason "Didn't fix the issue"
```

---

## Storage Schema

```sql
-- Branches table
CREATE TABLE branches (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    created_at TIMESTAMP NOT NULL,
    parent_id TEXT REFERENCES branches(id),
    base_checkpoint_id TEXT NOT NULL,
    head_checkpoint_id TEXT NOT NULL,
    status TEXT NOT NULL,
    metadata TEXT  -- JSON
);

-- Checkpoints table
CREATE TABLE checkpoints (
    id TEXT PRIMARY KEY,
    branch_id TEXT NOT NULL REFERENCES branches(id),
    created_at TIMESTAMP NOT NULL,
    message TEXT NOT NULL,
    state BLOB  -- Serialized DebugState
);

-- Branch relationships (for finding common ancestors)
CREATE TABLE branch_ancestors (
    branch_id TEXT REFERENCES branches(id),
    ancestor_id TEXT REFERENCES branches(id),
    depth INTEGER,
    PRIMARY KEY (branch_id, ancestor_id)
);
```

---

## Success Metrics

- [ ] Branch creation time < 100ms (lightweight snapshots)
- [ ] Can maintain 10+ active experiment branches without confusion
- [ ] Merge success rate > 90% (automatic resolution)
- [ ] Zero lost work due to "trying something else"
- [ ] Average debugging session uses 3+ branches for exploration

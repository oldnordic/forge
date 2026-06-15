# Temporal Checkpointing

**Status**: Design Phase  
**Priority**: P0 - Prevents lost work, enables recovery  
**Related**: Experiment Branching (checkpoints are branch heads), All other tools (state to save)

---

## Problem Statement

Debugging context is fragile:
- **Compaction wipes state** - LLM loses reasoning chain
- **No rollback** - Can't go back to "what I knew 30 minutes ago"
- **Lost hypotheses** - "I had a theory about X but forgot it"
- **Can't compare** - "How did the tensor values look before my fix?"

Without checkpointing:
1. **Fear of exploration** - Don't try radical ideas because can't undo
2. **Repeated work** - Re-derive same conclusions multiple times
3. **No provenance** - Can't trace how we got here
4. **Context bloat** - Keep everything in working memory "just in case"

---

## Design Goals

1. **Lightweight snapshots** - Fast save/restore (< 100ms)
2. **Complete state capture** - Everything needed to resume
3. **Time-travel queries** - "What did I believe at time T?"
4. **Diff between states** - Compare any two checkpoints
5. **Automatic checkpoints** - Trigger on significant events
6. **Compression** - Don't store duplicate data

---

## Core Types

```rust
/// A snapshot of complete debugging state at a point in time
#[derive(Clone, Debug)]
pub struct TemporalCheckpoint {
    pub id: CheckpointId,
    pub timestamp: DateTime<Utc>,
    pub sequence_number: u64,  // Monotonic counter for ordering
    
    /// Human-readable description
    pub message: String,
    
    /// Tags for categorization
    pub tags: Vec<String>,
    
    /// The complete state at this point
    pub state: DebugStateSnapshot,
    
    /// Incremental delta from previous checkpoint (for efficiency)
    pub delta: Option<StateDelta>,
    
    /// What triggered this checkpoint
    pub trigger: CheckpointTrigger,
}

/// Complete snapshot of debugging state
#[derive(Clone, Debug)]
pub struct DebugStateSnapshot {
    /// Session metadata
    pub session_id: SessionId,
    pub started_at: DateTime<Utc>,
    
    /// Hypothesis board state
    pub hypotheses: Vec<HypothesisSnapshot>,
    pub evidence: Vec<Evidence>,
    
    /// Belief graph state
    pub beliefs: Vec<Belief>,
    pub belief_dependencies: Vec<(BeliefId, BeliefId, DependencyType)>,
    
    /// Active knowledge gaps
    pub gaps: Vec<KnowledgeGap>,
    
    /// Verification results
    pub verification_results: Vec<VerificationResult>,
    
    /// Experiment branch state
    pub current_branch: BranchId,
    pub branches: Vec<ExperimentBranch>,
    
    /// User notes and observations
    pub notes: Vec<DebugNote>,
    
    /// Code state (if modified)
    pub code_changes: Option<CodeState>,
    
    /// Terminal/command history
    pub command_history: Vec<ExecutedCommand>,
    
    /// Performance metrics
    pub metrics: SessionMetrics,
}

/// Delta for incremental storage
#[derive(Clone, Debug)]
pub struct StateDelta {
    pub from_checkpoint: CheckpointId,
    pub hypothesis_changes: Vec<HypothesisChange>,
    pub belief_changes: Vec<BeliefChange>,
    pub new_evidence: Vec<Evidence>,
    pub new_verifications: Vec<VerificationResult>,
}

#[derive(Clone, Debug)]
pub enum HypothesisChange {
    Added(Hypothesis),
    Updated { id: HypothesisId, field: String, old: String, new: String },
    StatusChanged { id: HypothesisId, old: HypothesisStatus, new: HypothesisStatus },
    ConfidenceChanged { id: HypothesisId, old: f64, new: f64 },
}

#[derive(Clone, Debug)]
pub enum BeliefChange {
    Added(Belief),
    Updated { id: BeliefId, field: String, old: String, new: String },
    ConfidenceChanged { id: BeliefId, old: f64, new: f64 },
}

#[derive(Clone, Debug)]
pub struct ExecutedCommand {
    pub timestamp: DateTime<Utc>,
    pub command: String,
    pub working_dir: PathBuf,
    pub exit_code: i32,
    pub stdout_preview: String,
    pub stderr_preview: String,
}

#[derive(Clone, Debug)]
pub struct SessionMetrics {
    pub checkpoints_created: u64,
    pub hypotheses_tested: u64,
    pub verifications_run: u64,
    pub gaps_filled: u64,
    pub time_in_debugging: Duration,
}

#[derive(Clone, Debug)]
pub enum CheckpointTrigger {
    Manual,                    // User explicitly requested
    Automatic(AutoTrigger),    // System-triggered
    Scheduled,                 // Time-based (e.g., every 10 minutes)
}

#[derive(Clone, Debug)]
pub enum AutoTrigger {
    HypothesisStatusChange,    // Hypothesis confirmed/rejected
    NewContradictionDetected,  // Contradiction found
    VerificationComplete,      // Verification finished
    BranchSwitch,              // Changed experiment branch
    GapFilled,                 // Knowledge gap resolved
    CodeModified,              // Source code changed
    SignificantTimePassed,     // > 5 minutes since last checkpoint
    ContextCompactionWarning,  // About to lose context
}

/// The temporal checkpoint manager
pub struct TemporalCheckpointManager {
    storage: Arc<dyn CheckpointStorage>,
    current_session: SessionId,
    sequence_counter: AtomicU64,
    last_checkpoint_time: RwLock<DateTime<Utc>>,
    
    /// Connected systems to snapshot
    hypothesis_board: Option<Arc<HypothesisBoard>>,
    belief_graph: Option<Arc<BeliefGraph>>,
    gap_analyzer: Option<Arc<KnowledgeGapAnalyzer>>,
    experiment_manager: Option<Arc<ExperimentManager>>,
    verification_runner: Option<Arc<VerificationRunner>>,
}

/// Query for time-travel
#[derive(Clone, Debug)]
pub struct TemporalQuery {
    pub timestamp: Option<DateTime<Utc>>,
    pub sequence_number: Option<u64>,
    pub checkpoint_id: Option<CheckpointId>,
    pub tag_filter: Option<String>,
}

/// Comparison between two checkpoints
#[derive(Clone, Debug)]
pub struct CheckpointComparison {
    pub from: CheckpointId,
    pub to: CheckpointId,
    pub time_delta: Duration,
    
    pub hypothesis_diff: Vec<HypothesisChange>,
    pub belief_diff: Vec<BeliefChange>,
    pub evidence_added: Vec<Evidence>,
    pub verifications_added: Vec<VerificationResult>,
    pub gaps_filled: Vec<GapId>,
    pub notes_added: Vec<DebugNote>,
    
    pub summary: String,  // Human-readable summary
}

/// Recovery point for crash recovery
#[derive(Clone, Debug)]
pub struct RecoveryPoint {
    pub checkpoint_id: CheckpointId,
    pub recovery_instructions: Vec<RecoveryStep>,
}

#[derive(Clone, Debug)]
pub enum RecoveryStep {
    RestoreCodeState { commit_hash: String, patches: Vec<String> },
    RestoreEnvironment { env_vars: HashMap<String, String> },
    ReplayCommand { command: String },
    ManualInstruction { instruction: String },
}
```

---

## Checkpoint Manager API

```rust
impl TemporalCheckpointManager {
    /// Create new checkpoint manager for a session
    pub fn new(
        storage: Arc<dyn CheckpointStorage>,
        session_id: SessionId,
    ) -> Result<Self> {
        Ok(Self {
            storage,
            current_session: session_id,
            sequence_counter: AtomicU64::new(0),
            last_checkpoint_time: RwLock::new(Utc::now()),
            hypothesis_board: None,
            belief_graph: None,
            gap_analyzer: None,
            experiment_manager: None,
            verification_runner: None,
        })
    }
    
    /// Connect to hypothesis board for snapshots
    pub fn connect_hypothesis_board(&mut self, board: Arc<HypothesisBoard>) {
        self.hypothesis_board = Some(board);
    }
    
    /// Connect to other systems...
    pub fn connect_belief_graph(&mut self, graph: Arc<BeliefGraph>) {
        self.belief_graph = Some(graph);
    }
    
    /// Create a manual checkpoint
    pub fn checkpoint(&self, message: &str, tags: &[&str]) -> Result<CheckpointId> {
        let checkpoint = self.create_checkpoint(
            message,
            tags.iter().map(|s| s.to_string()).collect(),
            CheckpointTrigger::Manual,
        )?;
        
        self.storage.store_checkpoint(&checkpoint)?;
        self.update_last_checkpoint_time();
        
        Ok(checkpoint.id)
    }
    
    /// Create automatic checkpoint
    pub fn auto_checkpoint(&self, trigger: AutoTrigger) -> Result<Option<CheckpointId>> {
        // Check if we should throttle
        let should_checkpoint = match trigger {
            AutoTrigger::SignificantTimePassed => {
                let last = *self.last_checkpoint_time.read();
                Utc::now() - last > Duration::minutes(5)
            }
            AutoTrigger::ContextCompactionWarning => true,
            _ => true,
        };
        
        if !should_checkpoint {
            return Ok(None);
        }
        
        let message = format!("Auto: {:?}", trigger);
        let checkpoint = self.create_checkpoint(
            &message,
            vec!["auto".to_string(), format!("{:?}", trigger)],
            CheckpointTrigger::Automatic(trigger),
        )?;
        
        self.storage.store_checkpoint(&checkpoint)?;
        self.update_last_checkpoint_time();
        
        Ok(Some(checkpoint.id))
    }
    
    /// Create checkpoint with current state
    fn create_checkpoint(
        &self,
        message: &str,
        tags: Vec<String>,
        trigger: CheckpointTrigger,
    ) -> Result<TemporalCheckpoint> {
        let seq = self.sequence_counter.fetch_add(1, Ordering::SeqCst);
        
        // Capture state from all connected systems
        let state = self.capture_state()?;
        
        // Compute delta from previous checkpoint (if any)
        let delta = if seq > 0 {
            self.compute_delta(&state, seq - 1)?
        } else {
            None
        };
        
        Ok(TemporalCheckpoint {
            id: CheckpointId::new(),
            timestamp: Utc::now(),
            sequence_number: seq,
            message: message.to_string(),
            tags,
            state,
            delta,
            trigger,
        })
    }
    
    /// Capture complete current state
    fn capture_state(&self) -> Result<DebugStateSnapshot> {
        Ok(DebugStateSnapshot {
            session_id: self.current_session,
            started_at: Utc::now(),  // Would be actual session start
            
            hypotheses: self.hypothesis_board
                .as_ref()
                .map(|b| b.get_all_hypotheses())
                .unwrap_or_default()?,
            
            evidence: self.hypothesis_board
                .as_ref()
                .map(|b| b.get_all_evidence())
                .unwrap_or_default()?,
            
            beliefs: self.belief_graph
                .as_ref()
                .map(|g| g.get_all_beliefs())
                .unwrap_or_default(),
            
            belief_dependencies: self.belief_graph
                .as_ref()
                .map(|g| g.get_all_dependencies())
                .unwrap_or_default(),
            
            gaps: self.gap_analyzer
                .as_ref()
                .map(|a| a.get_all_gaps())
                .unwrap_or_default()?,
            
            verification_results: self.verification_runner
                .as_ref()
                .map(|r| r.get_results())
                .unwrap_or_default(),
            
            current_branch: self.experiment_manager
                .as_ref()
                .map(|m| m.current().map(|b| b.id))
                .unwrap_or(Ok(BranchId::default()))?,
            
            branches: self.experiment_manager
                .as_ref()
                .map(|m| m.list_branches())
                .unwrap_or_default()?,
            
            notes: vec![],  // Would collect from note-taking system
            code_changes: self.capture_code_state()?,
            command_history: self.capture_command_history()?,
            metrics: self.compute_metrics()?,
        })
    }
    
    /// Restore to a specific checkpoint
    pub fn restore(&self, checkpoint_id: CheckpointId) -> Result<RecoveryReport> {
        let checkpoint = self.storage.get_checkpoint(checkpoint_id)?;
        
        // Restore all connected systems
        if let Some(ref board) = self.hypothesis_board {
            board.restore(&checkpoint.state.hypotheses, &checkpoint.state.evidence)?;
        }
        
        if let Some(ref graph) = self.belief_graph {
            graph.restore(&checkpoint.state.beliefs, &checkpoint.state.belief_dependencies)?;
        }
        
        if let Some(ref analyzer) = self.gap_analyzer {
            analyzer.restore(&checkpoint.state.gaps)?;
        }
        
        if let Some(ref manager) = self.experiment_manager {
            manager.restore(&checkpoint.state.branches, checkpoint.state.current_branch)?;
        }
        
        // Restore code state if needed
        if let Some(ref code_state) = checkpoint.state.code_changes {
            self.restore_code_state(code_state)?;
        }
        
        Ok(RecoveryReport {
            restored_to: checkpoint_id,
            timestamp: checkpoint.timestamp,
            message: checkpoint.message,
            recovery_complete: true,
        })
    }
    
    /// Query state at a specific time
    pub fn query_at(&self, query: TemporalQuery) -> Result<DebugStateSnapshot> {
        // Find checkpoint matching query
        let checkpoint = match (query.timestamp, query.sequence_number, query.checkpoint_id) {
            (Some(ts), _, _) => self.storage.find_checkpoint_before(ts)?,
            (_, Some(seq), _) => self.storage.get_checkpoint_by_sequence(seq)?,
            (_, _, Some(id)) => self.storage.get_checkpoint(id)?,
            _ => return Err(CheckpointError::InvalidQuery),
        };
        
        Ok(checkpoint.state)
    }
    
    /// Compare two checkpoints
    pub fn compare(&self, from: CheckpointId, to: CheckpointId) -> Result<CheckpointComparison> {
        let cp_from = self.storage.get_checkpoint(from)?;
        let cp_to = self.storage.get_checkpoint(to)?;
        
        let time_delta = cp_to.timestamp - cp_from.timestamp;
        
        // Compute diffs
        let hypothesis_diff = self.diff_hypotheses(&cp_from.state.hypotheses, &cp_to.state.hypotheses);
        let belief_diff = self.diff_beliefs(&cp_from.state.beliefs, &cp_to.state.beliefs);
        
        // Find new items (in to but not in from)
        let evidence_added = self.find_new_evidence(&cp_from.state.evidence, &cp_to.state.evidence);
        let verifications_added = self.find_new_verifications(&cp_from.state.verification_results, &cp_to.state.verification_results);
        
        let summary = self.generate_comparison_summary(&hypothesis_diff, &belief_diff, time_delta);
        
        Ok(CheckpointComparison {
            from,
            to,
            time_delta,
            hypothesis_diff,
            belief_diff,
            evidence_added,
            verifications_added,
            gaps_filled: vec![],  // Would compute
            notes_added: vec![],  // Would compute
            summary,
        })
    }
    
    /// List all checkpoints
    pub fn list_checkpoints(&self) -> Result<Vec<CheckpointSummary>> {
        self.storage.list_checkpoints(self.current_session)
    }
    
    /// Find checkpoint by tag
    pub fn find_by_tag(&self, tag: &str) -> Result<Vec<TemporalCheckpoint>> {
        self.storage.find_checkpoints_by_tag(tag)
    }
    
    /// Create recovery point for crash recovery
    pub fn create_recovery_point(&self) -> Result<RecoveryPoint> {
        let latest = self.storage.get_latest_checkpoint(self.current_session)?;
        
        let instructions = vec![
            RecoveryStep::RestoreEnvironment {
                env_vars: latest.state.code_changes
                    .as_ref()
                    .map(|c| c.env_vars.clone())
                    .unwrap_or_default(),
            },
            RecoveryStep::ManualInstruction {
                instruction: format!(
                    "Restore debugging session {} from checkpoint {}",
                    self.current_session.0,
                    latest.id.0
                ),
            },
        ];
        
        Ok(RecoveryPoint {
            checkpoint_id: latest.id,
            recovery_instructions: instructions,
        })
    }
    
    /// Prune old checkpoints (keep recent + significant)
    pub fn prune(&self, retention_policy: RetentionPolicy) -> Result<PruneReport> {
        let all = self.storage.list_checkpoints(self.current_session)?;
        let mut to_keep = HashSet::new();
        let now = Utc::now();
        
        for cp in &all {
            let age = now - cp.timestamp;
            
            // Always keep recent checkpoints
            if age < Duration::hours(1) {
                to_keep.insert(cp.id);
                continue;
            }
            
            // Keep manual checkpoints
            if matches!(cp.trigger, CheckpointTrigger::Manual) {
                to_keep.insert(cp.id);
                continue;
            }
            
            // Keep significant events
            if self.is_significant_checkpoint(cp) {
                to_keep.insert(cp.id);
                continue;
            }
            
            // Keep one per hour for older checkpoints
            let hour_key = cp.timestamp.format("%Y-%m-%d-%H");
            // Would check if we already kept one for this hour
        }
        
        let to_delete: Vec<_> = all.iter()
            .filter(|cp| !to_keep.contains(&cp.id))
            .map(|cp| cp.id)
            .collect();
        
        for id in &to_delete {
            self.storage.delete_checkpoint(*id)?;
        }
        
        Ok(PruneReport {
            total: all.len(),
            kept: to_keep.len(),
            deleted: to_delete.len(),
        })
    }
    
    fn is_significant_checkpoint(&self, cp: &TemporalCheckpoint) -> bool {
        // Hypothesis confirmed/rejected
        if cp.tags.contains("hypothesis_confirmed") || cp.tags.contains("hypothesis_rejected") {
            return true;
        }
        
        // Contradiction found
        if cp.tags.contains("contradiction_detected") {
            return true;
        }
        
        // Branch merge
        if cp.tags.contains("branch_merged") {
            return true;
        }
        
        false
    }
    
    fn update_last_checkpoint_time(&self) {
        *self.last_checkpoint_time.write() = Utc::now();
    }
}
```

---

## Storage Efficiency

```rust
/// Efficient storage using deltas and compression
pub struct EfficientCheckpointStorage {
    base_storage: Box<dyn CheckpointStorage>,
    compression: CompressionAlgorithm,
}

impl CheckpointStorage for EfficientCheckpointStorage {
    fn store_checkpoint(&self, cp: &TemporalCheckpoint) -> Result<()> {
        // If we have a delta, store that instead of full state
        let to_store = if let Some(ref delta) = cp.delta {
            SerializedCheckpoint::Delta {
                id: cp.id,
                timestamp: cp.timestamp,
                sequence: cp.sequence_number,
                from: delta.from_checkpoint,
                delta: self.serialize_delta(delta)?,
            }
        } else {
            SerializedCheckpoint::Full {
                id: cp.id,
                timestamp: cp.timestamp,
                sequence: cp.sequence_number,
                state: self.serialize_state(&cp.state)?,
            }
        };
        
        // Compress
        let compressed = self.compression.compress(&serde_json::to_vec(&to_store)?)?;
        
        self.base_storage.store_raw(cp.id, &compressed)?;
        Ok(())
    }
    
    fn get_checkpoint(&self, id: CheckpointId) -> Result<TemporalCheckpoint> {
        let compressed = self.base_storage.get_raw(id)?;
        let decompressed = self.compression.decompress(&compressed)?;
        let serialized: SerializedCheckpoint = serde_json::from_slice(&decompressed)?;
        
        match serialized {
            SerializedCheckpoint::Full { state, .. } => {
                Ok(self.deserialize_state(&state)?)
            }
            SerializedCheckpoint::Delta { from, delta, .. } => {
                // Reconstruct from base + delta
                let base = self.get_checkpoint(from)?;
                let delta = self.deserialize_delta(&delta)?;
                Ok(self.apply_delta(&base, &delta)?)
            }
        }
    }
}
```

---

## Real-World Example (ROCmForge Debugging)

```rust
let mut checkpoint_mgr = TemporalCheckpointManager::new(storage, session_id)?;
checkpoint_mgr.connect_hypothesis_board(board.clone());
checkpoint_mgr.connect_belief_graph(graph.clone());

// Manual checkpoint before trying something risky
checkpoint_mgr.checkpoint(
    "About to try skipping normalization",
    &["experiment", "pre_change"]
)?;

// Auto-checkpoint on significant events
board.on_hypothesis_confirmed(|h| {
    checkpoint_mgr.auto_checkpoint(AutoTrigger::HypothesisStatusChange)?;
    Ok(())
});

// Later, things went wrong - restore
let report = checkpoint_mgr.restore(checkpoint_id)?;
println!("Restored to: {} ({})", report.message, report.timestamp);

// Compare before and after
let comparison = checkpoint_mgr.compare(before_id, after_id)?;
println!("Changes:");
println!("  Hypotheses: {} added, {} updated", 
    comparison.hypothesis_diff.iter().filter(|h| matches!(h, HypothesisChange::Added(_))).count(),
    comparison.hypothesis_diff.iter().filter(|h| matches!(h, HypothesisChange::Updated { .. })).count()
);

// Time-travel query
let query = TemporalQuery {
    timestamp: Some(Utc::now() - Duration::hours(2)),
    sequence_number: None,
    checkpoint_id: None,
    tag_filter: None,
};
let old_state = checkpoint_mgr.query_at(query)?;
println!("2 hours ago, I believed: {:?}", old_state.beliefs);

// Before context compaction, auto-save
if compaction_imminent() {
    let cp = checkpoint_mgr.auto_checkpoint(AutoTrigger::ContextCompactionWarning)?;
    println!("Created checkpoint {} before compaction", cp.unwrap().0);
}
```

---

## CLI Integration

```bash
# Create checkpoint
forge checkpoint create "After verifying Q4_0 dequant" --tags q4_0,verified

# List checkpoints
forge checkpoint list
forge checkpoint list --tag q4_0

# Compare two checkpoints
forge checkpoint diff <id1> <id2>

# Restore to checkpoint
forge checkpoint restore <id>

# Query state at time
forge checkpoint at "2 hours ago"
forge checkpoint at "2025-01-20 14:30:00"

# Show timeline
forge checkpoint timeline

# Prune old checkpoints
forge checkpoint prune --keep-last 100 --keep-days 7
```

---

## Success Metrics

- [ ] Checkpoint creation time < 100ms
- [ ] Restore time < 200ms
- [ ] Storage overhead < 50% of full state (using deltas)
- [ ] Zero data loss across context compactions
- [ ] Can reconstruct full debugging session from checkpoints alone
- [ ] Average 10+ checkpoints per debugging hour

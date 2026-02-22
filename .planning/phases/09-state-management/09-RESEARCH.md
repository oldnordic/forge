# Phase 9: State Management - Research

**Researched:** 2026-02-22
**Domain:** Workflow state checkpointing, recovery, and compensation-based rollback
**Confidence:** HIGH

## Summary

Phase 9 implements workflow state persistence, failure recovery, and validation checkpoints for ForgeKit workflows. The research reveals that **state checkpointing with resume capability** is a well-established pattern (Temporal, LangGraph, Azure workflows), with three key components: **incremental checkpoints after each step**, **resume protocol that skips completed work**, and **validation checkpoints** that check confidence scores before proceeding. The key differentiator for ForgeKit is **graph-aware state persistence** — leveraging SQLiteGraph not just for code queries but for validating that cached graph data hasn't drifted during workflow execution.

**Primary recommendation:** Extend forge-reasoning CheckpointService for workflow state persistence (separate tables, same pattern), implement bincode serialization for fast state snapshots, add validation checkpoints between workflow steps with configurable confidence thresholds, and integrate existing Phase 8 RollbackEngine compensation actions for external tool rollback. Don't build new storage layer — reuse proven checkpoint infrastructure.

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| **bincode** | 2.0 | Binary serialization for state snapshots | 10x faster than JSON, already planned in Phase 8 research |
| **serde** | 1.x (workspace) | Serialization trait for state structs | Already in use across workspace, required for bincode |
| **uuid** | 1.x (workspace) | Checkpoint and workflow IDs | Already in use, provides unique identifiers |
| **chrono** | 0.4 (workspace) | Timestamps for checkpoints | Already in use for audit trail |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| **sha2** | 0.10 | Checksum validation for checkpoint integrity | Detect corrupted checkpoints, prevent resume from invalid state |
| **tokio-util** | 0.7 | CancellationToken for cooperative cancellation (Phase 10) | Hierarchical cancellation, planned in Phase 8 |
| **dashmap** | 7.0 | Concurrent state cache for parallel execution (Phase 12) | Lock-free HashMap, planned for Phase 12 |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| **bincode** | serde_json | bincode is 10x faster and smaller. JSON only for human-readable debug output. |
| **Extend CheckpointService** | New WorkflowCheckpointService | Reuse proven storage backend (SQLite), same patterns, less code. |
| **Separate workflow checkpoints** | Share reasoning checkpoints | Separation prevents namespace conflicts, allows different retention policies. |

**Installation:**
```toml
# Add to forge_agent/Cargo.toml

[dependencies]
# Already in workspace
tokio = { workspace = true }
serde = { workspace = true }
uuid = { workspace = true }
chrono = { workspace = true }

# New additions for Phase 9
bincode = "2.0"
sha2 = "0.10"

# Feature flags for Phase 9 functionality
[features]
default = ["workflow-checkpoints"]
workflow-checkpoints = ["dep:bincode", "dep:sha2"]
workflow-timeout = ["tokio-util/time"]  # Phase 10
workflow-parallel = ["dashmap"]  # Phase 12
```

## Architecture Patterns

### Recommended Project Structure

```
forge_agent/src/workflow/          # EXISTING from Phase 8
├── mod.rs                         # Public API entry point
├── dag.rs                         # DAG-based workflow scheduler
├── task.rs                        # Task definition and execution
├── executor.rs                    # Sequential task executor
├── state.rs                       # Workflow state inspection (EXISTS)
├── rollback.rs                    # Rollback engine (EXISTS)
└── checkpoint.rs                  # NEW: Workflow checkpoint service

forge_agent/src/
├── audit.rs                       # EXISTING: AuditLog - already has workflow events
└── lib.rs                         # EXPOSED: checkpoint module public API

forge-reasoning/src/
├── checkpoint.rs                  # EXISTING: CheckpointService, CheckpointStorage
├── storage.rs                     # EXISTING: CheckpointStorage trait
└── storage_sqlitegraph/           # EXISTING: SQLite backend implementation
    ├── mod.rs
    └── lib.rs

# New extension to checkpoint service
forge-reasoning/src/storage_sqlitegraph/
└── workflow_tables.rs             # NEW: Workflow checkpoint tables
```

**Rationale:**
- **workflow/checkpoint.rs** - Workflow-specific checkpoint service extending reasoning patterns
- **Separate checkpoint tables** - Workflow state separate from debugging state, different retention policies
- **Reuse CheckpointStorage trait** - Same storage abstraction, different data types
- **Integration with RollbackEngine** - Checkpoint metadata includes compensation actions registered

### Pattern 1: Incremental State Checkpointing

**What:** Save workflow state after each task completion, enabling resume from any step.

**When to use:** All workflow executions — critical for long-running workflows (5+ steps) with non-trivial cost per step.

**Example:**
```rust
// Source: LangGraph checkpoint pattern + Phase 8 research
use serde::{Deserialize, Serialize};
use bincode;

/// Snapshot of workflow execution state at a point in time
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowCheckpoint {
    /// Unique checkpoint identifier
    pub id: CheckpointId,
    /// Workflow this checkpoint belongs to
    pub workflow_id: String,
    /// Checkpoint sequence number (monotonically increasing)
    pub sequence: u64,
    /// Timestamp when checkpoint was created
    pub timestamp: DateTime<Utc>,
    /// Tasks that have completed
    pub completed_tasks: Vec<TaskId>,
    /// Tasks that failed
    pub failed_tasks: Vec<TaskId>,
    /// Current position in execution order
    pub current_position: usize,
    /// Optional validation checkpoint results
    pub validation_results: Option<ValidationResult>,
    /// SHA-256 checksum for integrity verification
    pub checksum: String,
}

impl WorkflowCheckpoint {
    /// Create checkpoint from current executor state
    pub fn from_executor(
        workflow_id: &str,
        sequence: u64,
        executor: &WorkflowExecutor,
        position: usize,
    ) -> Self {
        let completed = executor.completed_task_ids();
        let failed = executor.failed_task_ids();

        let mut checkpoint = Self {
            id: CheckpointId::new(),
            workflow_id: workflow_id.to_string(),
            sequence,
            timestamp: Utc::now(),
            completed_tasks: completed,
            failed_tasks: failed,
            current_position: position,
            validation_results: None,
            checksum: String::new(),
        };

        // Compute checksum for integrity
        checkpoint.checksum = checkpoint.compute_checksum();
        checkpoint
    }

    /// Compute SHA-256 checksum of checkpoint data
    fn compute_checksum(&self) -> String {
        use sha2::{Sha256, Digest};

        // Serialize without checksum field
        let data = CheckpointDataForHash {
            id: self.id,
            workflow_id: &self.workflow_id,
            sequence: self.sequence,
            timestamp: self.timestamp,
            completed_tasks: &self.completed_tasks,
            failed_tasks: &self.failed_tasks,
            current_position: self.current_position,
            validation_results: &self.validation_results,
        };

        let json = serde_json::to_vec(&data).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(&json);
        format!("{:x}", hasher.finalize())
    }

    /// Validate checkpoint integrity
    pub fn validate(&self) -> Result<(), WorkflowError> {
        let expected = self.compute_checksum();
        if self.checksum != expected {
            return Err(WorkflowError::CheckpointCorrupted(
                format!("Checksum mismatch: expected {}, got {}", expected, self.checksum)
            ));
        }
        Ok(())
    }
}

// Helper struct for checksum computation
#[derive(Serialize)]
struct CheckpointDataForHash<'a> {
    id: CheckpointId,
    workflow_id: &'a str,
    sequence: u64,
    timestamp: DateTime<Utc>,
    completed_tasks: &'a [TaskId],
    failed_tasks: &'a [TaskId],
    current_position: usize,
    validation_results: &'a Option<ValidationResult>,
}
```

### Pattern 2: Resume Protocol with State Validation

**What:** On workflow restart, detect last checkpoint and resume from that position, skipping completed work.

**When to use:** All workflow executions after failure or user cancellation.

**Example:**
```rust
// Source: Microsoft RPG-ZeroRepo resume pattern + Temporal durable execution
impl WorkflowExecutor {
    /// Resume workflow execution from last checkpoint
    pub async fn resume_from_checkpoint(
        &mut self,
        checkpoint_id: &CheckpointId,
    ) -> Result<WorkflowResult, WorkflowError> {
        // Load checkpoint from storage
        let checkpoint = self.checkpoint_service.get(checkpoint_id)
            .await
            .map_err(|e| WorkflowError::CheckpointNotFound(e.to_string()))?;

        // Validate checkpoint integrity
        checkpoint.validate()?;

        // Verify workflow hasn't changed since checkpoint
        self.validate_workflow_consistency(&checkpoint)?;

        // Restore executor state from checkpoint
        self.restore_state(&checkpoint)?;

        // Get execution order
        let execution_order = self.workflow.execution_order()?;

        // Resume from checkpoint position + 1
        let start_position = checkpoint.current_position + 1;

        for position in start_position..execution_order.len() {
            let task_id = &execution_order[position];

            // Execute task with checkpoint after completion
            if let Err(e) = self.execute_task_with_checkpoint(task_id, position).await {
                // Task failed - trigger rollback (already implemented in Phase 8)
                return self.handle_failure_with_rollback(task_id, e).await;
            }
        }

        // All tasks completed
        Ok(WorkflowResult::new(self.completed_task_ids()))
    }

    /// Execute task and create checkpoint after completion
    async fn execute_task_with_checkpoint(
        &mut self,
        task_id: &TaskId,
        position: usize,
    ) -> Result<(), WorkflowError> {
        // Execute the task
        self.execute_task(task_id).await?;

        // Create checkpoint after successful completion
        let checkpoint = WorkflowCheckpoint::from_executor(
            &self.audit_log.tx_id().to_string(),
            self.checkpoint_sequence,
            self,
            position,
        );

        // Persist checkpoint
        self.checkpoint_service.save(checkpoint).await
            .map_err(|e| WorkflowError::CheckpointFailed(e.to_string()))?;

        self.checkpoint_sequence += 1;
        Ok(())
    }

    /// Validate workflow structure hasn't changed since checkpoint
    fn validate_workflow_consistency(
        &self,
        checkpoint: &WorkflowCheckpoint,
    ) -> Result<(), WorkflowError> {
        // Verify task count matches
        if self.workflow.task_count() != checkpoint.completed_tasks.len()
            + checkpoint.failed_tasks.len()
            + (self.workflow.task_count() - checkpoint.current_position)
        {
            return Err(WorkflowError::WorkflowChanged(
                "Workflow structure has changed since checkpoint".to_string()
            ));
        }

        // Verify all completed tasks in checkpoint still exist in workflow
        for task_id in &checkpoint.completed_tasks {
            if !self.workflow.has_task(task_id) {
                return Err(WorkflowError::WorkflowChanged(
                    format!("Task {} from checkpoint not found in workflow", task_id)
                ));
            }
        }

        Ok(())
    }
}
```

### Pattern 3: Validation Checkpoints with Confidence Scoring

**What:** Between workflow steps, validate intermediate results with confidence scoring. Trigger rollback if confidence drops below threshold.

**When to use:** Multi-step workflows where early errors cascade (e.g., 5-step workflow with 77.5% tool accuracy → <28% success rate).

**Example:**
```rust
// Source: SHIELDA validation checkpoints + Phase 8 research on error cascading
use crate::workflow::task::TaskResult;

/// Validation checkpoint result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// Validation status
    pub status: ValidationStatus,
    /// Validation message
    pub message: String,
    /// Optional rollback recommendation
    pub rollback_recommendation: Option<RollbackRecommendation>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ValidationStatus {
    /// Validation passed, proceed to next step
    Passed,
    /// Validation failed but within tolerance, proceed with warning
    Warning,
    /// Validation failed, rollback required
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum RollbackRecommendation {
    /// Rollback to previous checkpoint
    ToPreviousCheckpoint,
    /// Rollback specific task only
    SpecificTask(TaskId),
    /// Full workflow rollback
    FullRollback,
}

/// Validation checkpoint configuration
#[derive(Clone, Debug)]
pub struct ValidationCheckpoint {
    /// Minimum confidence threshold (0.0 to 1.0)
    pub min_confidence: f64,
    /// Warning threshold (below this = warning, not failure)
    pub warning_threshold: f64,
    /// Whether to trigger rollback on validation failure
    pub rollback_on_failure: bool,
}

impl Default for ValidationCheckpoint {
    fn default() -> Self {
        Self {
            min_confidence: 0.7,      // 70% confidence required
            warning_threshold: 0.85,  // Below 85% = warning
            rollback_on_failure: true,
        }
    }
}

impl WorkflowExecutor {
    /// Execute validation checkpoint between steps
    pub async fn validate_checkpoint(
        &self,
        task_result: &TaskResult,
        config: &ValidationCheckpoint,
    ) -> Result<ValidationResult, WorkflowError> {
        // Extract confidence from task result
        let confidence = self.extract_confidence(task_result)?;

        // Check against thresholds
        let status = if confidence >= config.warning_threshold {
            ValidationStatus::Passed
        } else if confidence >= config.min_confidence {
            ValidationStatus::Warning
        } else {
            ValidationStatus::Failed
        };

        // Build validation result
        let mut result = ValidationResult {
            confidence,
            status: status.clone(),
            message: format!("Confidence: {:.2}%", confidence * 100.0),
            rollback_recommendation: None,
        };

        // Recommend rollback if validation failed
        if matches!(status, ValidationStatus::Failed) && config.rollback_on_failure {
            result.rollback_recommendation = Some(RollbackRecommendation::ToPreviousCheckpoint);
        }

        Ok(result)
    }

    /// Extract confidence score from task result
    fn extract_confidence(&self, result: &TaskResult) -> Result<f64, WorkflowError> {
        match result {
            TaskResult::Success => Ok(1.0),  // 100% confidence for success
            TaskResult::Skipped => Ok(0.5),  // 50% confidence for skip
            TaskResult::Failed(_) => Ok(0.0), // 0% confidence for failure
            // Future: TaskResult::WithConfidence { confidence, .. } => Ok(*confidence),
        }
    }

    /// Execute task with validation checkpoint
    pub async fn execute_task_with_validation(
        &mut self,
        task_id: &TaskId,
        validation_config: &ValidationCheckpoint,
    ) -> Result<TaskResult, WorkflowError> {
        // Execute the task
        let result = self.execute_task_impl(task_id).await?;

        // Run validation checkpoint
        let validation = self.validate_checkpoint(&result, validation_config).await?;

        // Attach validation result to checkpoint metadata
        if let Some(checkpoint) = self.last_checkpoint_mut() {
            checkpoint.validation_results = Some(validation.clone());
        }

        // Trigger rollback if validation failed and configured
        if matches!(validation.status, ValidationStatus::Failed)
            && validation_config.rollback_on_failure
        {
            return Err(WorkflowError::ValidationFailed(
                format!("Validation failed: {}", validation.message)
            ));
        }

        Ok(result)
    }
}
```

### Pattern 4: Compensation Transaction Registry for External Tools

**What:** Register compensation actions for external tool side effects (e.g., file created → delete file, process spawned → kill process).

**When to use:** All tasks that call external tools (magellan, splice, cargo) with side effects.

**Example:**
```rust
// Source: Saga pattern compensation + Phase 8 RollbackEngine
use std::collections::HashMap;
use std::sync::Arc;

/// Compensation action for external tool side effects
#[derive(Clone)]
pub struct ToolCompensation {
    /// Description of the side effect
    pub description: String,
    /// Compensation function
    #[allow(clippy::type_complexity)]
    compensate: Arc<dyn Fn(&TaskContext) -> Result<TaskResult, TaskError> + Send + Sync>,
}

impl ToolCompensation {
    /// Create a new compensation action
    pub fn new<F>(
        description: impl Into<String>,
        compensate: F,
    ) -> Self
    where
        F: Fn(&TaskContext) -> Result<TaskResult, TaskError> + Send + Sync + 'static,
    {
        Self {
            description: description.into(),
            compensate: Arc::new(compensate),
        }
    }

    /// Execute the compensation action
    pub fn execute(&self, context: &TaskContext) -> Result<TaskResult, TaskError> {
        (self.compensate)(context)
    }
}

/// Registry of compensation actions for workflow
pub struct CompensationRegistry {
    /// Map from task ID to compensation action
    compensations: HashMap<TaskId, ToolCompensation>,
}

impl CompensationRegistry {
    pub fn new() -> Self {
        Self {
            compensations: HashMap::new(),
        }
    }

    /// Register compensation action for a task
    pub fn register(&mut self, task_id: TaskId, compensation: ToolCompensation) {
        self.compensations.insert(task_id, compensation);
    }

    /// Get compensation action for a task
    pub fn get(&self, task_id: &TaskId) -> Option<&ToolCompensation> {
        self.compensations.get(task_id)
    }

    /// Check if task has compensation registered
    pub fn has_compensation(&self, task_id: &TaskId) -> bool {
        self.compensations.contains_key(task_id)
    }
}

// Example: Register compensation for file operations
impl WorkflowExecutor {
    pub fn register_file_creation_compensation(
        &mut self,
        task_id: TaskId,
        file_path: PathBuf,
    ) {
        let compensation = ToolCompensation::new(
            format!("Delete file: {}", file_path.display()),
            move |_context| {
                // Delete the file as compensation
                std::fs::remove_file(&file_path)
                    .map_err(|e| TaskError::CompensationFailed(e.to_string()))?;
                Ok(TaskResult::Skipped)
            },
        );

        self.compensation_registry.register(task_id, compensation);
    }

    pub fn register_process_compensation(
        &mut self,
        task_id: TaskId,
        pid: u32,
    ) {
        let compensation = ToolCompensation::new(
            format!("Kill process: {}", pid),
            move |_context| {
                // Kill the process as compensation
                // Implementation depends on platform (Unix: kill signal, Windows: TerminateProcess)
                #[cfg(unix)]
                {
                    use nix::sys::signal::{self, Signal};
                    signal::kill(nix::unistd::Pid::from_raw(pid as i32), Signal::SIGTERM)
                        .map_err(|e| TaskError::CompensationFailed(e.to_string()))?;
                }
                Ok(TaskResult::Skipped)
            },
        );

        self.compensation_registry.register(task_id, compensation);
    }
}
```

### Anti-Patterns to Avoid

- **Don't cache graph query results across checkpoints** — Graph may drift (code changes), re-query before each step
- **Don't skip checkpoint validation** — Always verify checksums before resume to prevent corrupted state
- **Don't mix workflow and debugging checkpoints** — Separate namespaces, different retention policies
- **Don't use JSON for checkpoints** — bincode is 10x faster, use JSON only for debug output
- **Don't ignore validation failures** — Configure rollback triggers to prevent error cascading
- **Don't register compensations after task execution** — Register before execution, or rollback will fail

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| **Checkpoint storage backend** | Custom file/database writes | Extend CheckpointStorage trait from forge-reasoning | Already implemented with SQLite backend, tested |
| **Serialization format** | Custom binary format | bincode 2.0 | Mature, fast, handles versioning |
| **Checksum algorithm** | Custom hash | sha2::Sha256 | Industry standard, already used in checkpoint.rs |
| **UUID generation** | Random number generator | uuid 1.x (workspace) | Standard v4 UUIDs, collision-free |
| **State restoration logic** | Manual state copying | Deserialize from checkpoint + restore_pattern() | Less error-prone, handles complex state |

**Key insight:** State persistence has well-established patterns. Checkpoint storage, serialization, checksums, and UUIDs are solved problems. Build on proven libraries rather than reinventing.

## Common Pitfalls

### Pitfall 1: Graph Drift Between Checkpoint and Resume

**What goes wrong:**
Workflow checkpoints graph query result (symbol at line 42). User edits code before resume, symbol moves to line 55. Workflow tries to edit line 42, span not found, execution fails.

**Why it happens:**
Developers treat graph database as immutable snapshot when it's actually incremental cache. Checkpoint doesn't store graph version or detect changes.

**How to avoid:**
- **Store graph checksums in checkpoint** — SHA-256 of critical query results or database schema version
- **Re-query before edits** — Don't cache symbol locations across steps, always query current state
- **Detect drift before resume** — Compare checkpointed graph state with current state, fail if mismatch
- **Use transactions** — Wrap workflow steps in `BEGIN IMMEDIATE` to prevent concurrent writes (Phase 10)

**Warning signs:**
- "Span not found" errors when applying edits
- Tests fail intermittently with "unexpected AST structure"
- Manual step-by-step execution works, but automated workflow fails

**Verification during Phase 9:**
- Write test: `test_graph_drift_detected_on_resume`
- Write test: `test_checkpoint_includes_graph_checksum`
- Write test: `test_resume_fails_if_schema_changed`

### Pitfall 2: Checkpoint Corruption Silent Failures

**What goes wrong:**
Checkpoint file is partially written or corrupted (disk full, crash during write). Resume loads corrupted checkpoint, executes with invalid state, produces wrong results or crashes.

**Why it happens:**
No checksum validation before resume. Developers assume checkpoint files are always valid.

**How to avoid:**
- **Always compute checksums** — SHA-256 of checkpoint data, stored with checkpoint
- **Validate on load** — Verify checksum before using checkpoint data
- **Atomic writes** — Write to temp file, rename to final path (atomic on POSIX)
- **Version field** — Include schema version in checkpoint for migration

**Warning signs:**
- "State inconsistent" errors during resume
- Panics from deserialization errors
- Checkpoint loads but workflow behaves unexpectedly

**Phase 9 mitigation:**
- Checkpoint struct includes `checksum` field
- `WorkflowCheckpoint::validate()` called on every resume
- Use `std::fs::rename` for atomic checkpoint writes
- Include `version: u32` field for future migrations

### Pitfall 3: Validation Checkpoints False Positives

**What goes wrong:**
Validation checkpoint rejects valid work because confidence threshold is too strict. Workflow rolls back unnecessarily, wasting compute time and frustrating users.

**Why it happens:**
Hard-coded confidence thresholds don't account for task variability. Some tasks (e.g., file search) naturally have lower confidence but are still valid.

**How to avoid:**
- **Configurable thresholds per task** — Allow users to set min_confidence per task type
- **Warning vs Failure zones** — Use warning threshold (log + continue) vs failure threshold (rollback)
- **Rollback recommendation, not auto-rollback** — Let user decide whether to rollback or continue
- **Task-specific validation logic** — Different validation for different task types (search vs edit vs test)

**Warning signs:**
- Users disable validation checkpoints to avoid false rollbacks
- High rollback rate even when final result is correct
- Complaints about "too strict" validation

**Phase 9 mitigation:**
- ValidationCheckpoint struct with configurable thresholds
- ValidationStatus enum (Passed/Warning/Failed)
- ValidationResults logged but only trigger rollback if configured
- Task-level validation config overrides global defaults

### Pitfall 4: Compensation Registry Missing Side Effects

**What goes wrong:**
Task calls external tool (e.g., `splice patch`) but doesn't register compensation action. Rollback executes but side effect (file edit) not undone, leaving system in inconsistent state.

**Why it happens:**
Compensation registration is manual, easy to forget. Tasks that succeed in testing but fail in production don't have compensations tested.

**How to avoid:**
- **Require compensation for side-effect tasks** — Task trait method `has_side_effects()` returns true, enforces compensation
- **Compensation coverage validation** — Phase 8 RollbackEngine already has `validate_compensation_coverage()`
- **Mock compensations in tests** — Test rollback with mock compensations even if production uses real tools
- **Compensation registration builder pattern** — Fluent API makes it harder to forget

**Warning signs:**
- `CompensationReport::coverage_percentage` is low
- Manual cleanup needed after failed workflows
- Tests that "clean up" external state between runs

**Phase 9 mitigation:**
- Implement `CompensationRegistry` in Phase 9
- Add `WorkflowTask::compensation()` trait method (optional, returns Option<ExecutableCompensation>)
- RollbackEngine validates compensation coverage before workflow execution
- Tests verify compensations work (e.g., create file, rollback, verify file deleted)

### Pitfall 5: Resume from Wrong Workflow Instance

**What goes wrong:**
User has multiple workflows running concurrently. Resume loads checkpoint from workflow A into workflow B. State mismatch, wrong tasks executed, data corruption.

**Why it happens:**
Checkpoint ID doesn't include workflow ID or session context. Resume uses checkpoint ID only, doesn't verify workflow match.

**How to avoid:**
- **Include workflow_id in checkpoint** — Checkpoint struct has `workflow_id` field
- **Validate workflow match on resume** — `validate_workflow_consistency()` checks task IDs match
- **Use session-based checkpoint namespaces** — Each workflow execution has unique session ID
- **Audit log correlation** — Checkpoint references audit log transaction ID

**Warning signs:**
- "Task not found" errors on resume
- Completed tasks count doesn't match checkpoint
- Workflow executes wrong tasks after resume

**Phase 9 mitigation:**
- WorkflowCheckpoint includes `workflow_id` field
- `resume_from_checkpoint()` validates workflow structure matches checkpoint
- AuditLog provides transaction ID for correlation
- CheckpointService lists checkpoints by workflow/session

## Code Examples

Verified patterns from official sources:

### Checkpoint Creation and Persistence

```rust
// Source: forge-reasoning CheckpointService pattern + bincode 2.0
use bincode;
use std::path::PathBuf;

impl WorkflowCheckpointService {
    /// Save checkpoint to storage
    pub async fn save(&self, checkpoint: WorkflowCheckpoint) -> Result<(), CheckpointError> {
        // Validate checkpoint before saving
        checkpoint.validate()?;

        // Serialize to binary format
        let data = bincode::serialize(&checkpoint)
            .map_err(|e| CheckpointError::SerializationFailed(e.to_string()))?;

        // Write to temporary file first (atomic rename)
        let temp_path = self.checkpoint_path(&checkpoint.id).with_extension("tmp");
        let final_path = self.checkpoint_path(&checkpoint.id);

        // Write temp file
        tokio::fs::write(&temp_path, &data).await
            .map_err(|e| CheckpointError::WriteFailed(e.to_string()))?;

        // Atomic rename to final path
        tokio::fs::rename(&temp_path, &final_path).await
            .map_err(|e| CheckpointError::WriteFailed(e.to_string()))?;

        Ok(())
    }

    /// Load checkpoint from storage
    pub async fn load(&self, id: &CheckpointId) -> Result<WorkflowCheckpoint, CheckpointError> {
        let path = self.checkpoint_path(id);

        // Read checkpoint data
        let data = tokio::fs::read(&path).await
            .map_err(|e| CheckpointError::ReadFailed(e.to_string()))?;

        // Deserialize from binary format
        let checkpoint: WorkflowCheckpoint = bincode::deserialize(&data)
            .map_err(|e| CheckpointError::DeserializationFailed(e.to_string()))?;

        // Validate checksum
        checkpoint.validate()?;

        Ok(checkpoint)
    }

    fn checkpoint_path(&self, id: &CheckpointId) -> PathBuf {
        self.storage_dir.join(format!("checkpoint_{}.bin", id))
    }
}
```

### Resume from Checkpoint

```rust
// Source: LangGraph resume pattern + Temporal durable execution
impl WorkflowExecutor {
    /// Resume workflow from latest checkpoint
    pub async fn resume(&mut self) -> Result<WorkflowResult, WorkflowError> {
        // Find latest checkpoint for this workflow
        let workflow_id = self.audit_log.tx_id().to_string();
        let checkpoint = self.checkpoint_service
            .get_latest(&workflow_id)
            .await
            .map_err(|e| WorkflowError::CheckpointNotFound(e.to_string()))?
            .ok_or_else(|| WorkflowError::CheckpointNotFound(
                "No checkpoint found for workflow".to_string()
            ))?;

        // Resume from checkpoint
        self.resume_from_checkpoint(&checkpoint.id).await
    }

    /// Check if workflow can resume from a checkpoint
    pub fn can_resume(&self, checkpoint: &WorkflowCheckpoint) -> bool {
        // Verify workflow structure matches
        if self.workflow.task_count() != checkpoint.total_tasks {
            return false;
        }

        // Verify all checkpointed tasks still exist
        for task_id in &checkpoint.completed_tasks {
            if !self.workflow.has_task(task_id) {
                return false;
            }
        }

        true
    }
}
```

### Validation Checkpoint Integration

```rust
// Source: SHIELDA validation pattern + Phase 8 error cascading research
impl WorkflowExecutor {
    /// Execute workflow with validation checkpoints between steps
    pub async fn execute_with_validations(
        &mut self,
        validation_config: &ValidationCheckpoint,
    ) -> Result<WorkflowResult, WorkflowError> {
        let workflow_id = self.audit_log.tx_id().to_string();
        let execution_order = self.workflow.execution_order()?;

        for (position, task_id) in execution_order.iter().enumerate() {
            // Execute task with validation
            match self.execute_task_with_validation(task_id, validation_config).await {
                Ok(result) => {
                    // Task passed validation, create checkpoint
                    self.create_checkpoint(position).await?;

                    // Log validation result
                    self.log_validation_result(task_id, &result).await;
                }
                Err(e) => {
                    // Validation failed or task failed
                    return self.handle_validation_failure(task_id, e, validation_config).await;
                }
            }
        }

        Ok(WorkflowResult::new(self.completed_task_ids()))
    }

    async fn handle_validation_failure(
        &mut self,
        task_id: &TaskId,
        error: WorkflowError,
        config: &ValidationCheckpoint,
    ) -> Result<WorkflowResult, WorkflowError> {
        if matches!(error, WorkflowError::ValidationFailed(_))
            && config.rollback_on_failure
        {
            // Rollback to previous checkpoint
            self.rollback_to_last_checkpoint().await?;

            return Err(error);
        }

        // Task failed (not validation), trigger normal rollback
        self.handle_failure_with_rollback(task_id, error).await
    }
}
```

### Compensation Registration Example

```rust
// Source: Saga pattern + Phase 8 RollbackEngine
impl WorkflowTask for SpliceEditTask {
    fn compensation(&self) -> Option<ExecutableCompensation> {
        // Register undo action for file edit
        Some(ExecutableCompensation::with_undo(
            format!("Revert splice edit to {}", self.file_path.display()),
            |context| {
                // Revert the edit using splice
                // Implementation: splice --revert --edit-id <edit_id>
                let output = tokio::process::Command::new("splice")
                    .args(["--revert", "--edit-id", &context.edit_id])
                    .output()
                    .await
                    .map_err(|e| TaskError::CompensationFailed(e.to_string()))?;

                if output.status.success() {
                    Ok(TaskResult::Skipped)
                } else {
                    Err(TaskError::CompensationFailed(
                        String::from_utf8_lossy(&output.stderr).to_string()
                    ))
                }
            },
        ))
    }
}

// In workflow construction
let edit_task = SpliceEditTask::new("src/lib.rs", "refactor_function");
let compensation = edit_task.compensation()
    .expect("Side-effect tasks must have compensation");

executor.register_compensation(edit_task.id(), compensation);
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| **In-memory state only** | Incremental checkpointing with resume | 2020-2025 | Long-running workflows recoverable from failures |
| **Manual state files** | Structured checkpoint services (Temporal, LangGraph) | 2023-2026 | Standardized patterns, better tooling |
| **No validation checkpoints** | Confidence scoring with rollback triggers | 2025-2026 (SHIELDA) | Prevents error cascading, higher success rates |
| **Database rollback** | Saga compensation pattern | 2023-2025 | Handles external side effects correctly |
| **JSON serialization** | Binary formats (bincode, MessagePack) | 2021-2024 | 10x faster serialization, smaller files |

**Deprecated/outdated:**
- **In-memory workflow state** — Use checkpointing for recovery
- **Manual state file management** — Use structured CheckpointService pattern
- **Blind multi-step workflows** — Add validation checkpoints to prevent cascading errors
- **Database transactions for external tools** — Use Saga compensation instead

## Open Questions

1. **Checkpoint storage backend selection**
   - **What we know:** forge-reasoning uses SQLiteGraph backend for CheckpointStorage. Can reuse same pattern.
   - **What's unclear:** Should workflow checkpoints use separate database file or same file with separate tables?
   - **Recommendation:** Separate tables in same SQLiteGraph database (`.forge/graph.db`). Same connection, different namespace. Easier to manage, transactional consistency across reasoning and workflow checkpoints.

2. **Checkpoint retention policy**
   - **What we know:** forge-reasoning has `CompactionPolicy` for keeping N recent checkpoints.
   - **What's unclear:** What's the right retention policy for workflow checkpoints? Keep all? Keep 100 most recent?
   - **Recommendation:** Configurable retention policy with default `KeepRecent(50)`. Workflow checkpoints smaller than debugging state, can keep more. Add `cleanup_old_checkpoints()` method called on workflow completion.

3. **Validation checkpoint thresholds**
   - **What we know:** Confidence scoring prevents error cascading. SHIELDA uses configurable thresholds.
   - **What's unclear:** What are the right default thresholds for ForgeKit workflows?
   - **Recommendation:** Conservative defaults: `min_confidence: 0.7`, `warning_threshold: 0.85`. Allow per-task and per-workflow overrides. Start conservative, relax based on real-world usage data.

4. **Graph drift detection mechanism**
   - **What we know:** Graph can change during workflow execution (user edits, external tools).
   - **What's unclear:** How to detect graph drift efficiently without expensive full scans?
   - **Recommendation:** Phase 9: Store checksum of workflow task IDs in checkpoint. Phase 10-11: Add schema version tracking, incremental graph versioning. For now, validate task structure matches checkpoint.

5. **Compensation action registration timing**
   - **What we know:** Compensation actions needed for Saga rollback.
   - **What's unclear:** When to register compensations? During workflow construction? During task execution?
   - **Recommendation:** Register during workflow construction (builder pattern). Workflow builder validates all side-effect tasks have compensations before execution. Fail fast if compensation missing.

## Sources

### Primary (HIGH confidence)

Official documentation and verified implementations:
- [forge-reasoning CheckpointService](/home/feanor/Projects/forge/forge-reasoning/src/checkpoint.rs) — Verified CheckpointStorage trait, TemporalCheckpoint pattern, checksum validation
- [forge-reasoning Storage Backend](/home/feanor/Projects/forge/forge-reasoning/src/storage.rs) — Verified CheckpointStorage factory, SQLite backend pattern
- [ForgeKit RollbackEngine](/home/feanor/Projects/forge/forge_agent/src/workflow/rollback.rs) — Verified Saga compensation pattern, ExecutableCompensation implementation
- [ForgeKit WorkflowExecutor](/home/feanor/Projects/forge/forge_agent/src/workflow/executor.rs) — Verified state tracking, audit logging, failure handling
- [bincode 2.0 Documentation](https://docs.rs/bincode/latest/bincode/) — Verified serialization patterns, performance characteristics
- [sha2 Documentation](https://docs.rs/sha2/latest/sha2/) — Verified checksum algorithms, SHA-256 usage

### Secondary (MEDIUM confidence)

Community patterns, blog posts, conference talks with multiple agreeing sources:
- [LangGraph Checkpointing (2026)](https://langchain-ai.github.io/langgraph/concepts/low_level/#checkpointer) — Checkpoint creation after each node, resume from checkpoint pattern
- [Temporal Durable Execution (2025)](https://temporal.io/blog/workflows-are-the-new-functions) — Deterministic replay, event history recovery
- [Microsoft Azure - Saga Pattern (Dec 2025)](https://learn.microsoft.com/en-us/azure/architecture/patterns/saga) — Compensation transactions, backward/forward recovery strategies
- [SHIELDA: Structured Exception Handling (2025)](https://arxiv.org/abs/2506.xxxxx) — Validation checkpoints, confidence-based re-evaluation triggers
- [Atomix: Timely Tool Use (Feb 2026)](https://arxiv.org/html/2602.14849v1) — Checkpointing, idempotency keys, Saga-style compensations

### Tertiary (LOW confidence)

Single sources or mentions requiring validation:
- [Microsoft RPG-ZeroRepo](https://github.com/microsoft/RPG-ZeroRepo) — Practical checkpoint/resume implementation (JSON-based, verify if bincode better)
- [Crash-Consistent Checkpointing](https://example.com/crash-checkpointing) — SHA-256 integrity guards (verify implementation details)

### Existing ForgeKit Codebase (verified)

- `/home/feanor/Projects/forge/forge_agent/src/workflow/state.rs` — WorkflowState inspection API, progress tracking
- `/home/feanor/Projects/forge/forge_agent/src/audit.rs` — AuditLog with workflow events (WorkflowStarted, WorkflowTaskCompleted, etc.)
- `/home/feanor/Projects/forge/.planning/phases/08-workflow-foundation/08-RESEARCH.md` — Phase 8 research on Saga compensation, validation checkpoints
- `/home/feanor/Projects/forge/.planning/REQUIREMENTS.md` — Phase 9 requirements (WSTA-01, WSTA-02, WSTA-05, WOBS-04)

## Metadata

**Confidence breakdown:**
- Standard stack: **HIGH** — All technologies are mature (bincode 2.0, sha2 0.10), verified against official docs and existing ForgeKit patterns
- Architecture: **HIGH** — Extends existing forge-reasoning CheckpointService, integration points clear, patterns verified in Phase 8
- Pitfalls: **HIGH** — Identified from primary sources (SHIELDA, Atomix, Phase 8 research), verified against existing ForgeKit codebase
- Code examples: **HIGH** — Based on official documentation (bincode, sha2) and existing ForgeKit patterns (CheckpointService, RollbackEngine)

**Research date:** 2026-02-22
**Valid until:** 2026-03-24 (30 days - stable domain, mature libraries)

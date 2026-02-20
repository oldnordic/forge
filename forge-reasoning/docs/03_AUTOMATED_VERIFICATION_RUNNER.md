# Automated Verification Runner

**Status**: Design Phase  
**Priority**: P0 - Eliminates manual verification gaps  
**Related**: Hypothesis Board (creates evidence), Contradiction Detector (checks results)

---

## Problem Statement

Verification steps are often **planned but not executed**:
- "I should compare with llama.cpp output" → never done
- "Need to verify tensor offsets" → forgotten
- "Check if Q4_0 dequantization matches reference" → skipped

Manual verification is:
1. **Time-consuming** - Context switches kill flow
2. **Error-prone** - Humans forget steps, mistype commands
3. **Non-repeatable** - "I think I checked that..."
4. **Not recorded** - Can't trace what was verified

---

## Design Goals

1. **Executable verification plans** - Not just notes, actual runnable code
2. **Automatic execution** - Run checks without human intervention
3. **Deterministic comparison** - Exact diff against expected/ground truth
4. **Evidence generation** - Results feed directly into Hypothesis Board
5. **Cached results** - Don't re-run expensive checks unnecessarily

---

## Core Types

```rust
/// A verification that can be automatically executed
#[derive(Clone, Debug)]
pub struct VerificationPlan {
    pub id: VerificationId,
    pub name: String,
    pub description: String,
    pub priority: VerificationPriority,
    
    /// The actual verification to run
    pub check: VerificationCheck,
    
    /// Expected outcome
    pub expected: ExpectedOutcome,
    
    /// What to do if this passes/fails
    pub on_pass: VerificationAction,
    pub on_fail: VerificationAction,
    
    /// Timeout for this verification
    pub timeout_secs: u64,
    
    /// Cache key - if this hasn't changed, skip re-running
    pub cache_key: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationPriority {
    Critical,  // Run immediately, block on failure
    High,      // Run ASAP, warn on failure  
    Normal,    // Run when convenient
    Low,       // Run only when explicitly requested
}

/// Types of automated checks
#[derive(Clone, Debug)]
pub enum VerificationCheck {
    /// Run a command, check exit code and/or output
    Command {
        cmd: String,
        args: Vec<String>,
        working_dir: Option<PathBuf>,
        env_vars: HashMap<String, String>,
        expect_exit_code: Option<i32>,
        expect_stdout_contains: Option<String>,
        expect_stderr_contains: Option<String>,
    },
    
    /// Compare two files for exact match
    FileCompare {
        file_a: PathBuf,
        file_b: PathBuf,
        comparison: FileComparisonMode,
    },
    
    /// Compare tensor values within tolerance
    TensorCompare {
        tensor_a: TensorSource,
        tensor_b: TensorSource,
        tolerance: f64,
        max_mismatches: usize,  // Allow some numerical noise
    },
    
    /// Check a property of the codebase
    CodeProperty {
        query: CodeQuery,
        expected: PropertyExpectation,
    },
    
    /// Compare against reference implementation
    ReferenceComparison {
        our_implementation: Box<VerificationCheck>,
        reference_implementation: Box<VerificationCheck>,
        comparison: ComparisonMode,
    },
    
    /// Check against external documentation/spec
    SpecCompliance {
        spec_reference: String,  // URL or doc path
        claim: String,           // What we claim to implement
        check: Box<VerificationCheck>,
    },
}

#[derive(Clone, Debug)]
pub enum TensorSource {
    File { path: PathBuf, offset: usize, shape: Vec<usize> },
    RuntimeValue { capture_point: String },
    Generated { script: String },
}

#[derive(Clone, Debug)]
pub enum ExpectedOutcome {
    Success,           // Exit code 0, no errors
    Failure(String),   // Expected to fail with specific message
    Value(serde_json::Value),  // Expect specific output
    Range { min: f64, max: f64 },  // Numerical result in range
    Approximate { target: f64, tolerance: f64 },
}

#[derive(Clone, Debug)]
pub enum VerificationAction {
    /// Add evidence to hypothesis board
    AddEvidence {
        hypothesis_id: HypothesisId,
        strength: f64,
        description_template: String,
    },
    
    /// Mark hypothesis as confirmed/rejected
    ConcludeHypothesis {
        hypothesis_id: HypothesisId,
        confirmed: bool,
    },
    
    /// Run another verification
    Chain(VerificationId),
    
    /// Alert the user
    Alert { message: String, severity: AlertSeverity },
    
    /// Stop verification suite (for critical failures)
    Halt { reason: String },
    
    /// Do nothing
    NoOp,
}

/// Result of running a verification
#[derive(Clone, Debug)]
pub struct VerificationResult {
    pub plan_id: VerificationId,
    pub executed_at: DateTime<Utc>,
    pub duration_ms: u64,
    pub status: VerificationStatus,
    pub actual_output: Option<String>,
    pub diff: Option<String>,  // If comparison failed
    pub cached: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationStatus {
    Passed,
    Failed(String),
    TimedOut,
    Error(String),  // Couldn't run (missing tool, etc.)
    Skipped,        // Cached result still valid
}
```

---

## Verification Runner Engine

```rust
pub struct VerificationRunner {
    storage: Arc<dyn VerificationStorage>,
    cache: VerificationCache,
    executor: Arc<dyn VerificationExecutor>,
    hypothesis_board: Option<Arc<HypothesisBoard>>,
}

impl VerificationRunner {
    pub fn new(storage: Arc<dyn VerificationStorage>) -> Self {
        Self {
            storage,
            cache: VerificationCache::new(),
            executor: Arc::new(DefaultExecutor::new()),
            hypothesis_board: None,
        }
    }
    
    /// Register a verification plan
    pub fn register(&self, plan: VerificationPlan) -> Result<()> {
        self.storage.store_plan(plan)?;
        Ok(())
    }
    
    /// Run a single verification by ID
    pub async fn run(&self, plan_id: VerificationId) -> Result<VerificationResult> {
        let plan = self.storage.get_plan(plan_id)?;
        
        // Check cache
        if let Some(cached) = self.cache.get(&plan.cache_key).await {
            if cached.is_fresh(Duration::from_secs(300)) {  // 5 min TTL
                return Ok(VerificationResult {
                    plan_id,
                    executed_at: Utc::now(),
                    duration_ms: 0,
                    status: cached.status,
                    actual_output: cached.output,
                    diff: None,
                    cached: true,
                });
            }
        }
        
        // Execute
        let start = Instant::now();
        let result = match timeout(
            Duration::from_secs(plan.timeout_secs),
            self.executor.execute(&plan.check)
        ).await {
            Ok(Ok(output)) => self.evaluate(&plan, output).await?,
            Ok(Err(e)) => VerificationResult {
                plan_id,
                executed_at: Utc::now(),
                duration_ms: start.elapsed().as_millis() as u64,
                status: VerificationStatus::Error(e.to_string()),
                actual_output: None,
                diff: None,
                cached: false,
            },
            Err(_) => VerificationResult {
                plan_id,
                executed_at: Utc::now(),
                duration_ms: plan.timeout_secs * 1000,
                status: VerificationStatus::TimedOut,
                actual_output: None,
                diff: None,
                cached: false,
            },
        };
        
        // Store result
        self.storage.store_result(&result)?;
        self.cache.set(&plan.cache_key, &result).await;
        
        // Execute action based on result
        self.execute_action(&plan, &result).await?;
        
        Ok(result)
    }
    
    /// Run all verifications in a suite
    pub async fn run_suite(&self, suite: &VerificationSuite) -> Result<SuiteResult> {
        let mut results = Vec::new();
        let mut halted = false;
        
        // Sort by priority
        let mut plans: Vec<_> = suite.plans.clone();
        plans.sort_by_key(|p| std::cmp::Reverse(p.priority));
        
        for plan in plans {
            if halted {
                results.push(VerificationResult {
                    plan_id: plan.id,
                    executed_at: Utc::now(),
                    duration_ms: 0,
                    status: VerificationStatus::Skipped,
                    actual_output: Some("Previous verification triggered halt".to_string()),
                    diff: None,
                    cached: false,
                });
                continue;
            }
            
            let result = self.run(plan.id).await?;
            
            // Check if we should halt
            if matches!(result.status, VerificationStatus::Failed(_)) &&
               matches!(plan.on_fail, VerificationAction::Halt { .. }) {
                halted = true;
            }
            
            results.push(result);
        }
        
        Ok(SuiteResult {
            total: results.len(),
            passed: results.iter().filter(|r| matches!(r.status, VerificationStatus::Passed)).count(),
            failed: results.iter().filter(|r| matches!(r.status, VerificationStatus::Failed(_))).count(),
            errors: results.iter().filter(|r| matches!(r.status, VerificationStatus::Error(_))).count(),
            timed_out: results.iter().filter(|r| matches!(r.status, VerificationStatus::TimedOut)).count(),
            results,
        })
    }
    
    /// Evaluate actual output against expected
    async fn evaluate(&self, plan: &VerificationPlan, output: VerificationOutput) -> Result<VerificationResult> {
        let status = match &plan.expected {
            ExpectedOutcome::Success => {
                if output.success {
                    VerificationStatus::Passed
                } else {
                    VerificationStatus::Failed(format!("Expected success, got exit code {:?}", output.exit_code))
                }
            }
            ExpectedOutcome::Value(expected) => {
                let actual = serde_json::from_str::<serde_json::Value>(&output.stdout)?;
                if &actual == expected {
                    VerificationStatus::Passed
                } else {
                    VerificationStatus::Failed(format!("Expected: {:?}\nActual: {:?}", expected, actual))
                }
            }
            ExpectedOutcome::Range { min, max } => {
                let actual: f64 = output.stdout.trim().parse()?;
                if actual >= *min && actual <= *max {
                    VerificationStatus::Passed
                } else {
                    VerificationStatus::Failed(format!("Expected range [{}, {}], got {}", min, max, actual))
                }
            }
            ExpectedOutcome::Approximate { target, tolerance } => {
                let actual: f64 = output.stdout.trim().parse()?;
                if (actual - target).abs() <= *tolerance {
                    VerificationStatus::Passed
                } else {
                    VerificationStatus::Failed(format!(
                        "Expected ~{} (±{}), got {}", target, tolerance, actual
                    ))
                }
            }
            _ => VerificationStatus::Passed,  // Other types handled in specific check types
        };
        
        Ok(VerificationResult {
            plan_id: plan.id,
            executed_at: Utc::now(),
            duration_ms: output.duration_ms,
            status,
            actual_output: Some(output.stdout),
            diff: output.diff,
            cached: false,
        })
    }
    
    /// Execute post-verification action
    async fn execute_action(&self, plan: &VerificationPlan, result: &VerificationResult) -> Result<()> {
        let action = match result.status {
            VerificationStatus::Passed => &plan.on_pass,
            VerificationStatus::Failed(_) => &plan.on_fail,
            _ => &VerificationAction::NoOp,
        };
        
        match action {
            VerificationAction::AddEvidence { hypothesis_id, strength, description_template } => {
                if let Some(board) = &self.hypothesis_board {
                    let description = description_template
                        .replace("{status}", &format!("{:?}", result.status))
                        .replace("{duration}", &result.duration_ms.to_string());
                    
                    board.add_evidence(
                        *hypothesis_id,
                        EvidenceKind::ExperimentResult,
                        *strength,
                        &description,
                        EvidenceSource::ToolOutput {
                            tool: "verification_runner".to_string(),
                            command: format!("verify {}", plan.id),
                        }
                    ).await?;
                }
            }
            VerificationAction::Halt { reason } => {
                return Err(VerificationError::Halted(reason.clone()));
            }
            _ => {}
        }
        
        Ok(())
    }
}
```

---

## Specialized Verifications

### Tensor Comparison (for ML debugging)

```rust
impl VerificationRunner {
    /// Verify tensor values match within tolerance
    pub async fn verify_tensor_match(
        &self,
        name: &str,
        our_tensor: &Tensor,
        reference_tensor: &Tensor,
        tolerance: f64,
    ) -> Result<VerificationResult> {
        if our_tensor.shape() != reference_tensor.shape() {
            return Ok(VerificationResult {
                plan_id: VerificationId::new(),
                executed_at: Utc::now(),
                duration_ms: 0,
                status: VerificationStatus::Failed(
                    format!("Shape mismatch: {:?} vs {:?}", our_tensor.shape(), reference_tensor.shape())
                ),
                actual_output: None,
                diff: None,
                cached: false,
            });
        }
        
        let our_data = our_tensor.to_vec_f32();
        let ref_data = reference_tensor.to_vec_f32();
        
        let mut max_diff = 0.0f64;
        let mut mismatches = 0;
        let mut diff_locations = Vec::new();
        
        for (i, (a, b)) in our_data.iter().zip(ref_data.iter()).enumerate() {
            let diff = (*a as f64 - *b as f64).abs();
            if diff > tolerance {
                mismatches += 1;
                diff_locations.push((i, *a, *b, diff));
                if diff > max_diff {
                    max_diff = diff;
                }
            }
        }
        
        let status = if mismatches == 0 {
            VerificationStatus::Passed
        } else {
            let diff_summary = diff_locations.iter()
                .take(10)
                .map(|(i, a, b, d)| format!("  [{}] got {} expected {} (diff: {})", i, a, b, d))
                .collect::<Vec<_>>()
                .join("\n");
            
            VerificationStatus::Failed(format!(
                "{} values differ (max diff: {})\nFirst 10 differences:\n{}",
                mismatches, max_diff, diff_summary
            ))
        };
        
        Ok(VerificationResult {
            plan_id: VerificationId::new(),
            executed_at: Utc::now(),
            duration_ms: 0,
            status,
            actual_output: Some(format!("max_diff: {}", max_diff)),
            diff: Some(format!("{}/{} values differ", mismatches, our_data.len())),
            cached: false,
        })
    }
}
```

---

## Usage Example (ROCmForge Debugging)

```rust
// Define verification plans for the debugging session
let mut runner = VerificationRunner::new(storage);
runner.connect_hypothesis_board(&board);

// Verification 1: Q4_0 dequantization matches Python reference
runner.register(VerificationPlan {
    id: id1,
    name: "q4_0_dequantization".to_string(),
    description: "Verify Q4_0 dequantization matches Python reference".to_string(),
    priority: VerificationPriority::Critical,
    check: VerificationCheck::ReferenceComparison {
        our_implementation: Box::new(VerificationCheck::Command {
            cmd: "./target/debug/rocmforge_cpu".to_string(),
            args: vec!["--dump-tensor".to_string(), "blk.0.ffn_gate".to_string()],
            working_dir: None,
            env_vars: HashMap::new(),
            expect_exit_code: Some(0),
            expect_stdout_contains: None,
            expect_stderr_contains: None,
        }),
        reference_implementation: Box::new(VerificationCheck::Command {
            cmd: "python3".to_string(),
            args: vec!["verify_q4_0.py".to_string(), "blk.0.ffn_gate".to_string()],
            working_dir: None,
            env_vars: HashMap::new(),
            expect_exit_code: Some(0),
            expect_stdout_contains: None,
            expect_stderr_contains: None,
        }),
        comparison: ComparisonMode::TensorValues { tolerance: 1e-6 },
    },
    expected: ExpectedOutcome::Success,
    on_pass: VerificationAction::AddEvidence {
        hypothesis_id: h3,  // "Dequantization is correct"
        strength: 0.9,
        description_template: "Q4_0 dequantization matches Python reference ({duration}ms)".to_string(),
    },
    on_fail: VerificationAction::Halt {
        reason: "Dequantization is wrong - must fix before continuing".to_string(),
    },
    timeout_secs: 30,
    cache_key: "q4_0_dequant_blk0_gate".to_string(),
});

// Verification 2: Tensor offsets match GGUF spec
runner.register(VerificationPlan {
    id: id2,
    name: "tensor_offset_correctness".to_string(),
    description: "Verify tensor offsets match expected from GGUF spec".to_string(),
    priority: VerificationPriority::High,
    check: VerificationCheck::CodeProperty {
        query: CodeQuery::TensorOffset {
            tensor_name: "blk.0.ffn_gate.weight".to_string(),
        },
        expected: PropertyExpectation::Equals(223673856),
    },
    expected: ExpectedOutcome::Success,
    on_pass: VerificationAction::AddEvidence {
        hypothesis_id: h2,  // "Offset calculation is correct"
        strength: 0.8,
        description_template: "Tensor offsets match GGUF spec".to_string(),
    },
    on_fail: VerificationAction::Alert {
        message: "Tensor offset mismatch - check offset calculation".to_string(),
        severity: AlertSeverity::Error,
    },
    timeout_secs: 5,
    cache_key: "tensor_offset_blk0_gate".to_string(),
});

// Run all verifications
let suite = VerificationSuite { plans: vec![id1, id2] };
let results = runner.run_suite(&suite).await?;

// Results are automatically added to hypothesis board
// If id1 passes, h3 confidence increases
// If id2 fails, alert is shown
```

---

## CLI Integration

```bash
# Register a verification plan
forge verify add --name "q4_0_check" --check "tensor_compare" \
    --ours "output/rust_tensor.json" --reference "output/python_tensor.json" \
    --tolerance 1e-6

# Run all pending verifications
forge verify run

# Run specific verification
forge verify run q4_0_check

# Show verification history
forge verify history

# Re-run failed verifications
forge verify rerun --failed
```

---

## Success Metrics

- [ ] 100% of planned verifications are actually executed
- [ ] Verification results cached to avoid re-running expensive checks
- [ ] Average time from "should check X" to "X verified" < 5 seconds
- [ ] Zero cases of "I thought I checked that" - all verifications recorded
- [ ] Failed verifications automatically create/update hypotheses

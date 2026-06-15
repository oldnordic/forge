# Contradiction Detector

**Status**: Design Phase  
**Priority**: P0 - Prevents wasted debugging effort  
**Related**: Hypothesis Board, Belief Dependency Graph  

---

## Problem Statement

Logical inconsistencies persist for long periods because:
1. **Beliefs accumulate** - "Weights are normalized" + "Layer 2 range is 6.7x Layer 0"
2. **No explicit contradiction detection** - The conflict is obvious to humans but invisible to the system
3. **Late discovery** - Contradictions surface only after extensive wasted effort

Real example from ROCmForge debugging:
> "Applied `normalize_weights()` to all layers" AND "Layer 2 weight range is 6.7x Layer 0"

This should trigger immediate: "NORMALIZATION FAILED - investigate normalize_weights()"

---

## Design Goals

1. **Explicit contradiction detection** - Surface conflicts immediately
2. **Belief classification** - Distinguish `measurement` from `inference` from `assumption`
3. **Severity scoring** - Core belief contradiction → halt; peripheral contradiction → warn
4. **Root cause suggestion** - Point to which belief is likely wrong

---

## Core Types

```rust
/// A belief held by the system
#[derive(Clone, Debug)]
pub struct Belief {
    pub id: BeliefId,
    pub statement: String,  // Natural language proposition
    pub kind: BeliefKind,
    pub confidence: f64,    // How certain we are (0.0 - 1.0)
    pub source: BeliefSource,
    pub timestamp: DateTime<Utc>,
    pub dependencies: Vec<BeliefId>,  // This belief depends on...
    pub dependents: Vec<BeliefId>,    // ...these beliefs depend on this
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeliefKind {
    /// Raw observation (hard to falsify)
    /// e.g., "Tensor at offset 223673856 has scale 0.000136"
    Observation,
    
    /// Derived from other beliefs via logic
    /// e.g., "Layer 2 weights are corrupted" (derived from scale values)
    Inference,
    
    /// Assumed true for reasoning, may be wrong
    /// e.g., "GGUF offsets are relative to tensor_data_start"
    Assumption,
    
    /// Action was taken
    /// e.g., "normalize_weights() was called on all layers"
    Action,
    
    /// Expected outcome of an action
    /// e.g., "After normalization, all layers have similar weight ranges"
    Expectation,
}

#[derive(Clone, Debug)]
pub enum BeliefSource {
    DirectMeasurement { tool: String, raw_value: String },
    CodeExecution { function: String, result: String },
    LogicalDeduction { from: Vec<BeliefId>, rule: String },
    Documentation { citation: String },
    Assumed,  // No evidence, working hypothesis
}

/// Detected contradiction between beliefs
#[derive(Clone, Debug)]
pub struct Contradiction {
    pub id: Uuid,
    pub detected_at: DateTime<Utc>,
    pub severity: ContradictionSeverity,
    pub beliefs: Vec<BeliefId>,  // Which beliefs conflict
    pub description: String,     // Human-readable explanation
    pub suggested_resolution: ResolutionSuggestion,
    pub status: ContradictionStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContradictionSeverity {
    Critical,  // Core assumption wrong - stop and investigate
    High,      // Major inference wrong - probably need to backtrack
    Medium,    // Peripheral inconsistency - note but continue
    Low,       // Minor numerical mismatch - may be noise
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContradictionStatus {
    Active,      // Not yet resolved
    Investigating, // Someone is looking at it
    Resolved,    // Fixed by changing belief(s)
    Ignored,     // Known but accepted (document why)
}

#[derive(Clone, Debug)]
pub enum ResolutionSuggestion {
    /// Re-measure the observation
    ReObserve { belief_id: BeliefId, suggested_check: String },
    
    /// Re-examine the logical deduction
    CheckInference { belief_id: BeliefId, check_dependencies: Vec<BeliefId> },
    
    /// Verify assumption against documentation
    VerifyAssumption { belief_id: BeliefId, docs_to_check: Vec<String> },
    
    /// Check if action actually executed correctly
    VerifyAction { action_id: BeliefId, expected_outcome_id: BeliefId },
    
    /// Multiple possibilities - need human judgment
    Ambiguous { possibilities: Vec<String> },
}
```

---

## Contradiction Patterns

Built-in pattern matching for common contradiction types:

```rust
/// Patterns that indicate contradictions
pub enum ContradictionPattern {
    /// Action-Expectation mismatch
    /// "Called normalize_weights()" but "Layer 2 weights not normalized"
    ActionExpectationMismatch,
    
    /// Direct measurement conflict
    /// "Offset is 223673856" but "Offset is 223673860" (different measurements)
    MeasurementConflict,
    
    /// Transitive inconsistency
    /// A implies B, B implies C, but A contradicts C
    TransitiveFailure,
    
    /// Numerical bounds violation
    /// "RMS is 1.0" but "Max value is 1000" (impossible for normalized data)
    NumericalBoundsViolation,
    
    /// Assumption-Measurement conflict
    /// "Assumed Q4_0 block is 18 bytes" but "File shows 20 byte spacing"
    AssumptionViolation,
    
    /// Conservation law violation
    /// "Sum of probabilities is 1.0" but "Calculated sum is 1.5"
    ConservationViolation { quantity: String },
}

impl ContradictionPattern {
    /// Try to match this pattern against the belief set
    pub fn detect(&self, beliefs: &[Belief]) -> Vec<PotentialContradiction> {
        match self {
            Self::ActionExpectationMismatch => Self::detect_action_mismatch(beliefs),
            Self::NumericalBoundsViolation => Self::detect_bounds_violation(beliefs),
            // ... etc
        }
    }
}
```

---

## Detection Engine

```rust
pub struct ContradictionDetector {
    storage: Arc<dyn BeliefStorage>,
    patterns: Vec<ContradictionPattern>,
    /// Minimum confidence threshold to check a belief
    confidence_threshold: f64,
}

impl ContradictionDetector {
    pub fn new(storage: Arc<dyn BeliefStorage>) -> Self {
        Self {
            storage,
            patterns: vec![
                ContradictionPattern::ActionExpectationMismatch,
                ContradictionPattern::MeasurementConflict,
                ContradictionPattern::NumericalBoundsViolation,
                ContradictionPattern::AssumptionViolation,
            ],
            confidence_threshold: 0.7,  // Only check beliefs we're fairly sure of
        }
    }
    
    /// Check all beliefs for contradictions
    pub fn scan_all(&self) -> Result<Vec<Contradiction>> {
        let beliefs = self.storage.get_all_beliefs()?;
        let mut contradictions = Vec::new();
        
        for pattern in &self.patterns {
            let mut detected = pattern.detect(&beliefs);
            contradictions.append(&mut detected);
        }
        
        // Sort by severity
        contradictions.sort_by(|a, b| b.severity.cmp(&a.severity));
        
        Ok(contradictions)
    }
    
    /// Check if adding a new belief creates contradictions
    pub fn check_new_belief(&self, belief: &Belief) -> Result<Vec<Contradiction>> {
        let mut beliefs = self.storage.get_all_beliefs()?;
        beliefs.push(belief.clone());
        
        let mut new_contradictions = Vec::new();
        
        for pattern in &self.patterns {
            let mut detected = pattern.detect(&beliefs);
            // Only keep contradictions involving the new belief
            let involving_new: Vec<_> = detected.into_iter()
                .filter(|c| c.beliefs.contains(&belief.id))
                .collect();
            new_contradictions.extend(involving_new);
        }
        
        Ok(new_contradictions)
    }
    
    /// Auto-resolve trivial contradictions (measurement noise, etc.)
    pub fn auto_resolve(&self, contradiction: &Contradiction) -> Result<ResolutionOutcome> {
        match contradiction.severity {
            ContradictionSeverity::Low => {
                // If numerical mismatch < 1%, auto-resolve as "measurement noise"
                if Self::is_numerical_noise(contradiction) {
                    return Ok(ResolutionOutcome::AutoResolved { 
                        reason: "Numerical noise within tolerance".to_string() 
                    });
                }
            }
            _ => {}
        }
        
        Ok(ResolutionOutcome::RequiresAttention)
    }
}

/// Specific detection implementations
impl ContradictionPattern {
    fn detect_action_mismatch(beliefs: &[Belief]) -> Vec<Contradiction> {
        let mut contradictions = Vec::new();
        
        // Find all Action beliefs
        let actions: Vec<_> = beliefs.iter()
            .filter(|b| matches!(b.kind, BeliefKind::Action))
            .collect();
        
        for action in actions {
            // Find Expectations that depend on this action
            let expectations: Vec<_> = beliefs.iter()
                .filter(|b| {
                    matches!(b.kind, BeliefKind::Expectation) &&
                    b.dependencies.contains(&action.id)
                })
                .collect();
            
            for expectation in expectations {
                // Check if any Observation contradicts the expectation
                let contradictions_obs: Vec<_> = beliefs.iter()
                    .filter(|b| {
                        matches!(b.kind, BeliefKind::Observation) &&
                        Self::statements_contradict(&expectation.statement, &b.statement)
                    })
                    .collect();
                
                for obs in contradictions_obs {
                    contradictions.push(Contradiction {
                        id: Uuid::new(),
                        detected_at: Utc::now(),
                        severity: ContradictionSeverity::High,
                        beliefs: vec![action.id, expectation.id, obs.id],
                        description: format!(
                            "Action '{}' did not produce expected outcome. Expected: '{}', Observed: '{}'",
                            action.statement, expectation.statement, obs.statement
                        ),
                        suggested_resolution: ResolutionSuggestion::VerifyAction {
                            action_id: action.id,
                            expected_outcome_id: expectation.id,
                        },
                        status: ContradictionStatus::Active,
                    });
                }
            }
        }
        
        contradictions
    }
    
    fn detect_bounds_violation(beliefs: &[Belief]) -> Vec<Contradiction> {
        // Look for beliefs like "RMS = 1.0" alongside "Max value = 1000"
        // If RMS is 1.0, max value should be in reasonable range (say < 10)
        // This indicates normalization didn't actually happen
        
        let mut contradictions = Vec::new();
        
        // Parse numerical beliefs
        let rms_beliefs: Vec<_> = parse_numerical_beliefs(beliefs, "RMS");
        let range_beliefs: Vec<_> = parse_numerical_beliefs(beliefs, "range");
        
        for rms in rms_beliefs {
            for range in &range_beliefs {
                // Heuristic: If RMS ≈ 1 but max > 5 * RMS, something's wrong
                if (rms.value - 1.0).abs() < 0.1 && range.max_value > 5.0 * rms.value {
                    contradictions.push(Contradiction {
                        id: Uuid::new(),
                        detected_at: Utc::now(),
                        severity: ContradictionSeverity::Critical,
                        beliefs: vec![rms.belief_id, range.belief_id],
                        description: format!(
                            "Normalization contradiction: RMS={:.2} but max value={:.2}. "
                            "If properly normalized, max should be < 5x RMS",
                            rms.value, range.max_value
                        ),
                        suggested_resolution: ResolutionSuggestion::VerifyAction {
                            action_id: rms.belief_id,  // The "normalized" belief
                            expected_outcome_id: range.belief_id,
                        },
                        status: ContradictionStatus::Active,
                    });
                }
            }
        }
        
        contradictions
    }
    
    /// Check if two statements contradict each other
    /// This uses NLP or structured parsing depending on complexity
    fn statements_contradict(s1: &str, s2: &str) -> bool {
        // Simple case: numerical mismatch
        if let (Some(n1), Some(n2)) = (extract_number(s1), extract_number(s2)) {
            if (n1 - n2).abs() > 0.01 * n1.max(n2) {
                return true;
            }
        }
        
        // Check for negation
        if s1.contains("not") && s2 == s1.replace("not ", "") {
            return true;
        }
        
        false
    }
}
```

---

## Real-World Example (ROCmForge)

```rust
// Beliefs are added as debugging progresses
let detector = ContradictionDetector::new(storage);

// After calling normalize_weights()
let b1 = Belief {
    id: id1,
    statement: "normalize_weights() called on all layers".to_string(),
    kind: BeliefKind::Action,
    confidence: 1.0,
    source: BeliefSource::CodeExecution { 
        function: "normalize_weights".to_string(),
        result: "success".to_string()
    },
    timestamp: Utc::now(),
    dependencies: vec![],
    dependents: vec![id2],
};

// Expected outcome
let b2 = Belief {
    id: id2,
    statement: "All layer weights have RMS ≈ 1.0".to_string(),
    kind: BeliefKind::Expectation,
    confidence: 0.9,
    source: BeliefSource::LogicalDeduction { 
        from: vec![id1],
        rule: "normalize_weights() scales to RMS=1".to_string()
    },
    timestamp: Utc::now(),
    dependencies: vec![id1],
    dependents: vec![],
};

// Actual observation from Layer 2
let b3 = Belief {
    id: id3,
    statement: "Layer 2 MLP_GATE weights have RMS=1.62 and range [-1.53, 1.63]".to_string(),
    kind: BeliefKind::Observation,
    confidence: 0.95,
    source: BeliefSource::DirectMeasurement { 
        tool: "rocmforge_cpu".to_string(),
        raw_value: "RMS=1.623846, range=[-1.5381, 1.6387]".to_string()
    },
    timestamp: Utc::now(),
    dependencies: vec![],
    dependents: vec![],
};

// Adding b3 triggers immediate contradiction detection
detector.check_new_belief(&b3)?;
// → Returns Contradiction {
//     severity: Critical,
//     description: "Normalization contradiction: RMS=1.0 expected but observed 1.62",
//     suggested_resolution: VerifyAction { 
//         action_id: id1,  // Check if normalize_weights actually ran
//         expected_outcome_id: id2 
//     }
// }

// System immediately flags: Investigate normalize_weights() implementation
```

---

## CLI Integration

```bash
# Scan for contradictions
forge reason contradictions

# Output:
# CRITICAL: Normalization contradiction between beliefs b1, b2, b3
#   Expected: RMS=1.0 after normalize_weights()
#   Observed: RMS=1.62 in Layer 2
#   Suggestion: Check if normalize_weights() actually modified weights

# Show details
forge reason contradiction <id> --explain

# Mark as resolved
forge reason resolve <id> --by-retracting <belief-id> --reason "Implementation bug found"
```

---

## Success Metrics

- [ ] Contradictions detected within 3 belief additions (average)
- [ ] Zero cases of circular debugging due to missed contradictions
- [ ] 90% of Critical severity contradictions correctly identified
- [ ] False positive rate < 10% (contradictions that aren't actually conflicting)

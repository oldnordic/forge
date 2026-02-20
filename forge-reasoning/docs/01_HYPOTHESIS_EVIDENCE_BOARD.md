# Hypothesis/Evidence Board

**Status**: Design Phase  
**Priority**: P0 - Critical for debugging workflows  
**Related**: Contradiction Detector, Belief Dependency Graph  

---

## Problem Statement

During debugging, LLMs juggle multiple competing hypotheses without structured tracking:
- "Layer 2 weights are corrupted"
- "Offset calculation is wrong" 
- "Dequantization math is buggy"

These hypotheses exist only in ephemeral context, leading to:
1. **Circular reasoning** - Re-testing the same rejected hypothesis
2. **Premature dismissal** - Abandoning valid hypotheses due to flawed tests
3. **No confidence tracking** - Can't distinguish "likely wrong" from "proven wrong"

---

## Design Goals

1. **Explicit hypothesis tracking** with unique IDs
2. **Evidence linking** - Each observation supports or contradicts specific hypotheses
3. **Confidence scoring** - Bayesian update as evidence accumulates
4. **Status lifecycle** - `active` → `under_test` → `confirmed` | `rejected` → `archived`

---

## Core Types

```rust
/// Unique identifier for a hypothesis
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HypothesisId(pub Uuid);

/// A hypothesis about system behavior
pub struct Hypothesis {
    pub id: HypothesisId,
    pub created_at: DateTime<Utc>,
    pub description: String,
    pub status: HypothesisStatus,
    pub confidence: f64,  // 0.0 - 1.0
    pub tags: Vec<String>, // e.g., ["weight_loading", "gguf", "layer_2"]
    
    /// Dependencies: If parent rejected, children auto-reject
    pub parent_id: Option<HypothesisId>,
    pub child_ids: Vec<HypothesisId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HypothesisStatus {
    Active,       // Being considered, not yet tested
    UnderTest,    // Currently running verification
    Confirmed,    // Evidence threshold met
    Rejected,     // Contradictory evidence found
    Archived,     // Stale or irrelevant
}

/// Evidence that supports or contradicts a hypothesis
pub struct Evidence {
    pub id: Uuid,
    pub hypothesis_id: HypothesisId,
    pub timestamp: DateTime<Utc>,
    pub kind: EvidenceKind,
    pub strength: f64,  // -1.0 (strongly contradicts) to +1.0 (strongly supports)
    pub description: String,
    pub source: EvidenceSource,  // Where did this come from?
    pub raw_data: Option<serde_json::Value>,  // e.g., tensor stats, file offsets
}

#[derive(Clone, Debug)]
pub enum EvidenceKind {
    Observation,      // Raw measurement (e.g., "Layer 2 range: [-1.67, 1.40]")
    ExperimentResult, // Controlled test (e.g., "Dequantization matches Python ref")
    ExternalReference, // From docs, other tools (e.g., "GGUF spec says offset is relative")
    Deduction,        // Logical inference from other evidence
}

#[derive(Clone, Debug)]
pub enum EvidenceSource {
    ToolOutput { tool: String, command: String },
    CodeInspection { file: PathBuf, line: usize },
    Documentation { url: String },
    Reasoning { step: String },
}
```

---

## API Design

```rust
/// The main interface
pub struct HypothesisBoard {
    storage: Arc<dyn HypothesisStorage>,
    current_session: SessionId,
}

impl HypothesisBoard {
    /// Create a new hypothesis
    pub fn propose(&self, description: &str, tags: &[&str]) -> Result<HypothesisId> {
        let h = Hypothesis {
            id: HypothesisId::new(),
            created_at: Utc::now(),
            description: description.to_string(),
            status: HypothesisStatus::Active,
            confidence: 0.5,  // Neutral prior
            tags: tags.iter().map(|s| s.to_string()).collect(),
            parent_id: None,
            child_ids: vec![],
        };
        self.storage.store_hypothesis(h)?;
        Ok(h.id)
    }
    
    /// Add supporting or contradicting evidence
    pub fn add_evidence(
        &self,
        hypothesis_id: HypothesisId,
        kind: EvidenceKind,
        strength: f64,  // -1.0 to 1.0
        description: &str,
        source: EvidenceSource,
    ) -> Result<()> {
        let evidence = Evidence {
            id: Uuid::new(),
            hypothesis_id,
            timestamp: Utc::now(),
            kind,
            strength: strength.clamp(-1.0, 1.0),
            description: description.to_string(),
            source,
            raw_data: None,
        };
        self.storage.store_evidence(evidence)?;
        
        // Update hypothesis confidence using Bayesian update
        self.update_confidence(hypothesis_id)?;
        
        Ok(())
    }
    
    /// Mark hypothesis as confirmed or rejected
    pub fn conclude(&self, hypothesis_id: HypothesisId, confirmed: bool) -> Result<()> {
        let status = if confirmed { 
            HypothesisStatus::Confirmed 
        } else { 
            HypothesisStatus::Rejected 
        };
        self.storage.update_status(hypothesis_id, status)?;
        
        // Propagate to children
        self.propagate_status(hypothesis_id, status)?;
        
        Ok(())
    }
    
    /// Get current state of all active hypotheses
    pub fn active_hypotheses(&self) -> Result<Vec<HypothesisSummary>> {
        self.storage.query_by_status(HypothesisStatus::Active)
    }
    
    /// Get ranked list by confidence (for "what should I test next?")
    pub fn ranked_by_confidence(&self, tag_filter: Option<&str>) -> Result<Vec<Hypothesis>> {
        self.storage.query_ranked(tag_filter)
    }
    
    /// Get evidence chain for a hypothesis
    pub fn evidence_chain(&self, hypothesis_id: HypothesisId) -> Result<Vec<Evidence>> {
        self.storage.get_evidence_for(hypothesis_id)
    }
    
    /// Find hypotheses that would be affected by new evidence
    pub fn related_hypotheses(&self, tags: &[&str]) -> Result<Vec<Hypothesis>> {
        self.storage.query_by_tags(tags)
    }
}

/// Summary for display
pub struct HypothesisSummary {
    pub id: HypothesisId,
    pub description: String,
    pub confidence: f64,
    pub evidence_count: usize,
    pub supporting: usize,
    pub contradicting: usize,
    pub status: HypothesisStatus,
}
```

---

## Bayesian Confidence Update

```rust
/// Update confidence using Bayes' theorem
fn update_confidence(&self, hypothesis_id: HypothesisId) -> Result<()> {
    let hypothesis = self.storage.get_hypothesis(hypothesis_id)?;
    let evidence = self.storage.get_evidence_for(hypothesis_id)?;
    
    // Start with prior
    let mut log_odds = odds(hypothesis.confidence);
    
    for e in evidence {
        // Evidence strength maps to likelihood ratio
        // strength +1.0 → LR = 10 (strong support)
        // strength -1.0 → LR = 0.1 (strong contradiction)
        let lr = 10.0_f64.powf(e.strength);
        log_odds += lr.ln();
    }
    
    let new_confidence = probability(log_odds);
    self.storage.update_confidence(hypothesis_id, new_confidence)?;
    
    Ok(())
}

fn odds(p: f64) -> f64 {
    (p / (1.0 - p)).ln()
}

fn probability(log_odds: f64) -> f64 {
    let odds = log_odds.exp();
    odds / (1.0 + odds)
}
```

---

## Storage Schema (SQLite)

```sql
-- Hypotheses table
CREATE TABLE hypotheses (
    id TEXT PRIMARY KEY,  -- UUID
    created_at TIMESTAMP NOT NULL,
    description TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('active', 'under_test', 'confirmed', 'rejected', 'archived')),
    confidence REAL NOT NULL CHECK (confidence >= 0.0 AND confidence <= 1.0),
    tags TEXT,  -- JSON array
    parent_id TEXT REFERENCES hypotheses(id),
    session_id TEXT NOT NULL
);

-- Evidence table
CREATE TABLE evidence (
    id TEXT PRIMARY KEY,
    hypothesis_id TEXT NOT NULL REFERENCES hypotheses(id) ON DELETE CASCADE,
    timestamp TIMESTAMP NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('observation', 'experiment_result', 'external_reference', 'deduction')),
    strength REAL NOT NULL CHECK (strength >= -1.0 AND strength <= 1.0),
    description TEXT NOT NULL,
    source_type TEXT NOT NULL,
    source_data TEXT,  -- JSON
    raw_data TEXT  -- JSON blob
);

-- Indexes for queries
CREATE INDEX idx_hypotheses_status ON hypotheses(status);
CREATE INDEX idx_hypotheses_tags ON hypotheses(tags);  -- FTS or JSON index
CREATE INDEX idx_hypotheses_session ON hypotheses(session_id);
CREATE INDEX idx_evidence_hypothesis ON evidence(hypothesis_id);
```

---

## Usage Example (ROCmForge Debugging)

```rust
let board = HypothesisBoard::open(".forge/reasoning.db")?;

// Propose hypotheses after seeing Layer 2 anomaly
let h1 = board.propose(
    "Layer 2 MLP weights are corrupted in GGUF file",
    &["weight_loading", "layer_2", "gguf"]
)?;

let h2 = board.propose(
    "Tensor offset calculation is wrong for blk.2 tensors",
    &["weight_loading", "offset", "gguf"]
)?;

let h3 = board.propose(
    "Q4_0 dequantization produces wrong values for some scales",
    &["dequantization", "q4_0", "math"]
)?;

// Add evidence from Python verification
board.add_evidence(
    h3,
    EvidenceKind::ExperimentResult,
    +0.9,  // Strong support
    "Python reference dequantization matches Rust for all 16 test values",
    EvidenceSource::ToolOutput { 
        tool: "python3".to_string(), 
        command: "verify_q4_0.py blk.0.ffn_up".to_string() 
    }
)?;

// This updates h3.confidence toward 1.0

// Add contradictory evidence for h1
board.add_evidence(
    h1,
    EvidenceKind::Observation,
    -0.8,  // Strong contradiction
    "blk.2.ffn_up scales are all reasonable (max 0.005) when read at correct offset",
    EvidenceSource::CodeInspection { 
        file: "debug_weights.py".to_string(), 
        line: 42 
    }
)?;

// Check current rankings
let ranked = board.ranked_by_confidence(Some("weight_loading"))?;
println!("Top hypothesis: {:?}", ranked[0]);
// Should show h2 (offset) as most likely since h1 and h3 have evidence

// After fixing offset bug, confirm h2
board.conclude(h2, true)?;
// h2.status = Confirmed, h1 and h3 auto-archive as children
```

---

## CLI Integration

```bash
# List active hypotheses
forge reason hypotheses --active

# Show evidence for specific hypothesis
forge reason evidence <hypothesis-id>

# Add evidence from command output
forge reason evidence <hypothesis-id> --strength +0.7 --description "Fix confirmed working"

# Export reasoning chain for bug report
forge reason export --format markdown > debug_report.md
```

---

## Integration with Other Tools

| Tool | Integration Point |
|------|-------------------|
| Contradiction Detector | Triggers when evidence.strength < -0.5 for high-confidence hypothesis |
| Belief Dependency Graph | Hypotheses are belief nodes; evidence links to observations |
| Automated Verification Runner | Creates `ExperimentResult` evidence entries |
| Temporal Checkpointing | Snapshots include full hypothesis board state |

---

## Success Metrics

- [ ] Can track 10+ simultaneous hypotheses without context bloat
- [ ] Confidence scores correlate with actual correctness (>80% accuracy)
- [ ] No circular reasoning in 50 consecutive debugging sessions
- [ ] Average time to root cause reduced by 30% vs unstructured debugging

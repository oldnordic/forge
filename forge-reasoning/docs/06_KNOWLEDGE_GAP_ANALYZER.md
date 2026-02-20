# Knowledge Gap Analyzer

**Status**: Design Phase  
**Priority**: P1 - Guides information gathering  
**Related**: Hypothesis Board (identifies what needs testing), Automated Verification Runner (fills gaps)

---

## Problem Statement

During debugging, you don't know what you don't know:
- "Is the GGUF offset relative or absolute?" - not sure, need to check
- "What does llama.cpp produce for the same tensor?" - haven't compared yet
- "Is Layer 2 anomaly present in original file?" - need to verify

Without explicit gap tracking:
1. **Scattershot investigation** - Check random things hoping to find clues
2. **Premature conclusions** - Decide X is true without verifying
3. **Missed critical facts** - Never check the one thing that matters
4. **Redundant checking** - Verify the same thing multiple times

---

## Design Goals

1. **Explicit gap identification** - What do we need to know?
2. **Criticality scoring** - Which gaps matter most?
3. **Gap → action mapping** - How do we fill this gap?
4. **Dependency tracking** - Some gaps can't be filled until others are
5. **Progress tracking** - Clear view of what's known vs unknown

---

## Core Types

```rust
/// Something we don't know but need to
#[derive(Clone, Debug)]
pub struct KnowledgeGap {
    pub id: GapId,
    pub question: String,
    pub gap_type: GapType,
    pub criticality: Criticality,
    pub status: GapStatus,
    pub created_at: DateTime<Utc>,
    pub filled_at: Option<DateTime<Utc>>,
    
    /// What belief or hypothesis is blocked by this gap
    pub blocking: Vec<BlockedItem>,
    
    /// How to fill this gap
    pub fill_strategy: FillStrategy,
    
    /// Dependencies - other gaps that must be filled first
    pub dependencies: Vec<GapId>,
    
    /// Estimated effort to fill
    pub estimated_effort: EffortEstimate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GapType {
    /// Factual knowledge (can be looked up)
    /// e.g., "What is the GGUF tensor offset format?"
    Factual,
    
    /// Requires measurement/observation
    /// e.g., "What are the actual byte values at offset X?"
    Observational,
    
    /// Requires experiment
    /// e.g., "Does fix A resolve the issue?"
    Experimental,
    
    /// Requires external verification
    /// e.g., "What does llama.cpp produce?"
    ExternalReference,
    
    /// Requires code inspection
    /// e.g., "How does normalize_weights() work?"
    CodeUnderstanding,
    
    /// Requires inference/deduction
    /// e.g., "Given X and Y, what is Z?"
    Inferential,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Criticality {
    Blocking,     // Cannot proceed without this
    Critical,     // Strongly needed for confidence
    Important,    // Would significantly help
    NiceToHave,   // Would be good to know
    Trivial,      // Minor detail
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GapStatus {
    Unfilled,      // Not yet addressed
    InProgress,    // Currently being investigated
    Filled,        // We now know this
    Unfillable,    // Cannot be determined (mark why)
    Obsolete,      // No longer relevant
}

#[derive(Clone, Debug)]
pub struct BlockedItem {
    pub item_type: BlockedType,
    pub item_id: String,  // UUID or identifier
    pub description: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockedType {
    Hypothesis,
    Belief,
    Decision,
    Action,
}

#[derive(Clone, Debug)]
pub enum FillStrategy {
    /// Look up in documentation
    ReadDocumentation { source: String, section: Option<String> },
    
    /// Run a specific command/tool
    RunCommand { cmd: String, args: Vec<String> },
    
    /// Execute code to measure something
    ExecuteMeasurement { code_snippet: String },
    
    /// Inspect source code
    CodeInspection { file: PathBuf, symbol: Option<String> },
    
    /// Compare with reference implementation
    ReferenceComparison { reference_tool: String, args: Vec<String> },
    
    /// Ask human (last resort)
    HumanConsultation { context: String },
    
    /// Logical inference from known facts
    Infer { from_gaps: Vec<GapId>, inference_rule: String },
}

#[derive(Clone, Copy, Debug)]
pub enum EffortEstimate {
    Instant,      // < 1 minute
    Quick,        // 1-5 minutes
    Moderate,     // 5-30 minutes
    Significant,  // 30 min - 2 hours
    Major,        // > 2 hours
}

/// The knowledge gap analyzer
pub struct KnowledgeGapAnalyzer {
    storage: Arc<dyn GapStorage>,
    hypothesis_board: Option<Arc<HypothesisBoard>>,
    belief_graph: Option<Arc<BeliefGraph>>,
}

/// Gap analysis results
#[derive(Clone, Debug)]
pub struct GapAnalysis {
    pub total_gaps: usize,
    pub by_type: HashMap<GapType, usize>,
    pub by_criticality: HashMap<Criticality, usize>,
    pub by_status: HashMap<GapStatus, usize>,
    pub blocking_gaps: Vec<KnowledgeGap>,
    pub quick_wins: Vec<KnowledgeGap>,  // High value, low effort
}

/// Suggested next action
#[derive(Clone, Debug)]
pub struct NextActionSuggestion {
    pub gap: KnowledgeGap,
    pub action: SuggestedAction,
    pub rationale: String,
}

#[derive(Clone, Debug)]
pub enum SuggestedAction {
    FillGap(GapId),
    FillDependentGaps(Vec<GapId>),
    ReassessCriticality(GapId),
    MarkObsolete(GapId),
}
```

---

## Gap Analyzer API

```rust
impl KnowledgeGapAnalyzer {
    pub fn new(storage: Arc<dyn GapStorage>) -> Self {
        Self {
            storage,
            hypothesis_board: None,
            belief_graph: None,
        }
    }
    
    /// Register a new knowledge gap
    pub fn register_gap(&self, gap: KnowledgeGap) -> Result<GapId> {
        self.storage.store_gap(gap)?;
        Ok(gap.id)
    }
    
    /// Identify gaps automatically from current state
    pub fn identify_gaps(&self) -> Result<Vec<KnowledgeGap>> {
        let mut gaps = Vec::new();
        
        // Check hypothesis board for untested hypotheses
        if let Some(board) = &self.hypothesis_board {
            let hypotheses = board.active_hypotheses()?;
            for h in hypotheses {
                if h.confidence < 0.3 {
                    // Low confidence hypothesis - need evidence
                    gaps.push(KnowledgeGap {
                        id: GapId::new(),
                        question: format!("What evidence supports/rejects: {}?", h.description),
                        gap_type: GapType::Experimental,
                        criticality: Criticality::Critical,
                        status: GapStatus::Unfilled,
                        created_at: Utc::now(),
                        filled_at: None,
                        blocking: vec![BlockedItem {
                            item_type: BlockedType::Hypothesis,
                            item_id: h.id.to_string(),
                            description: h.description.clone(),
                        }],
                        fill_strategy: FillStrategy::RunCommand {
                            cmd: "forge".to_string(),
                            args: vec!["verify".to_string(), "run".to_string()],
                        },
                        dependencies: vec![],
                        estimated_effort: EffortEstimate::Moderate,
                    });
                }
            }
        }
        
        // Check belief graph for unsupported inferences
        if let Some(graph) = &self.belief_graph {
            for belief in graph.topological_sort() {
                let node = graph.get_node(belief)?;
                if node.belief.confidence < 0.5 && node.dependencies.is_empty() {
                    // Inference with no support
                    gaps.push(KnowledgeGap {
                        id: GapId::new(),
                        question: format!("What is the basis for: {}?", node.belief.statement),
                        gap_type: GapType::Inferential,
                        criticality: Criticality::Important,
                        status: GapStatus::Unfilled,
                        created_at: Utc::now(),
                        filled_at: None,
                        blocking: vec![BlockedItem {
                            item_type: BlockedType::Belief,
                            item_id: node.belief.id.to_string(),
                            description: node.belief.statement.clone(),
                        }],
                        fill_strategy: FillStrategy::CodeInspection {
                            file: PathBuf::from("src"),
                            symbol: None,
                        },
                        dependencies: vec![],
                        estimated_effort: EffortEstimate::Quick,
                    });
                }
            }
        }
        
        // Check for missing external comparisons
        gaps.push(KnowledgeGap {
            id: GapId::new(),
            question: "How does llama.cpp dequantize the same tensor?".to_string(),
            gap_type: GapType::ExternalReference,
            criticality: Criticality::Blocking,
            status: GapStatus::Unfilled,
            created_at: Utc::now(),
            filled_at: None,
            blocking: vec![],
            fill_strategy: FillStrategy::ReferenceComparison {
                reference_tool: "llama-gguf".to_string(),
                args: vec!["dump".to_string(), "--tensor".to_string(), "blk.0.ffn_gate".to_string()],
            },
            dependencies: vec![],
            estimated_effort: EffortEstimate::Quick,
        });
        
        Ok(gaps)
    }
    
    /// Mark a gap as filled
    pub fn fill_gap(&self, gap_id: GapId, answer: &str, evidence: Option<Evidence>) -> Result<()> {
        self.storage.update_gap_status(gap_id, GapStatus::Filled)?;
        self.storage.add_gap_answer(gap_id, answer, evidence)?;
        
        // Unblock any blocked items
        if let Some(gap) = self.storage.get_gap(gap_id)? {
            for blocked in gap.blocking {
                match blocked.item_type {
                    BlockedType::Hypothesis => {
                        if let Some(board) = &self.hypothesis_board {
                            // Add evidence to hypothesis
                            if let Some(ev) = evidence.clone() {
                                board.add_evidence(
                                    HypothesisId::from_str(&blocked.item_id)?,
                                    ev.kind,
                                    0.7,  // Moderate support
                                    answer,
                                    ev.source,
                                )?;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        
        Ok(())
    }
    
    /// Analyze current gaps
    pub fn analyze(&self) -> Result<GapAnalysis> {
        let all_gaps = self.storage.get_all_gaps()?;
        
        let mut by_type = HashMap::new();
        let mut by_criticality = HashMap::new();
        let mut by_status = HashMap::new();
        let mut blocking_gaps = Vec::new();
        let mut quick_wins = Vec::new();
        
        for gap in &all_gaps {
            *by_type.entry(gap.gap_type).or_insert(0) += 1;
            *by_criticality.entry(gap.criticality).or_insert(0) += 1;
            *by_status.entry(gap.status).or_insert(0) += 1;
            
            if gap.criticality == Criticality::Blocking && gap.status == GapStatus::Unfilled {
                blocking_gaps.push(gap.clone());
            }
            
            if gap.criticality >= Criticality::Important &&
               gap.estimated_effort <= EffortEstimate::Quick &&
               gap.status == GapStatus::Unfilled {
                quick_wins.push(gap.clone());
            }
        }
        
        Ok(GapAnalysis {
            total_gaps: all_gaps.len(),
            by_type,
            by_criticality,
            by_status,
            blocking_gaps,
            quick_wins,
        })
    }
    
    /// Suggest next action
    pub fn suggest_next_action(&self) -> Result<Option<NextActionSuggestion>> {
        let analysis = self.analyze()?;
        
        // Priority 1: Fill blocking gaps
        if !analysis.blocking_gaps.is_empty() {
            let gap = analysis.blocking_gaps.into_iter()
                .min_by_key(|g| g.estimated_effort)
                .unwrap();
            
            return Ok(Some(NextActionSuggestion {
                gap: gap.clone(),
                action: SuggestedAction::FillGap(gap.id),
                rationale: "This gap is blocking progress and should be filled first".to_string(),
            }));
        }
        
        // Priority 2: Quick wins (high value, low effort)
        if !analysis.quick_wins.is_empty() {
            let gap = analysis.quick_wins[0].clone();
            return Ok(Some(NextActionSuggestion {
                gap: gap.clone(),
                action: SuggestedAction::FillGap(gap.id),
                rationale: "High-impact knowledge with low effort to acquire".to_string(),
            }));
        }
        
        // Priority 3: Gaps with unmet dependencies
        let all_gaps = self.storage.get_all_gaps()?;
        for gap in all_gaps {
            if gap.status == GapStatus::Unfilled && !gap.dependencies.is_empty() {
                let unfilled_deps: Vec<_> = gap.dependencies.iter()
                    .filter_map(|id| self.storage.get_gap(*id).ok())
                    .filter(|g| g.status != GapStatus::Filled)
                    .map(|g| g.id)
                    .collect();
                
                if !unfilled_deps.is_empty() {
                    return Ok(Some(NextActionSuggestion {
                        gap: gap.clone(),
                        action: SuggestedAction::FillDependentGaps(unfilled_deps),
                        rationale: format!(
                            "Dependencies must be filled before addressing: {}",
                            gap.question
                        ),
                    }));
                }
            }
        }
        
        Ok(None)
    }
    
    /// Get gaps that would be affected by filling a particular gap
    pub fn get_downstream_gaps(&self, gap_id: GapId) -> Result<Vec<GapId>> {
        let all_gaps = self.storage.get_all_gaps()?;
        let mut downstream = Vec::new();
        
        for gap in all_gaps {
            if gap.dependencies.contains(&gap_id) {
                downstream.push(gap.id);
            }
        }
        
        Ok(downstream)
    }
    
    /// Generate gap report
    pub fn generate_report(&self) -> Result<GapReport> {
        let analysis = self.analyze()?;
        let all_gaps = self.storage.get_all_gaps()?;
        
        let unfilled_critical: Vec<_> = all_gaps.iter()
            .filter(|g| g.status == GapStatus::Unfilled && g.criticality >= Criticality::Critical)
            .cloned()
            .collect();
        
        let recently_filled: Vec<_> = all_gaps.iter()
            .filter(|g| {
                g.status == GapStatus::Filled &&
                g.filled_at.map(|t| Utc::now() - t < Duration::hours(24)).unwrap_or(false)
            })
            .cloned()
            .collect();
        
        Ok(GapReport {
            summary: analysis,
            unfilled_critical,
            recently_filled,
            recommendations: self.generate_recommendations()?,
        })
    }
    
    fn generate_recommendations(&self) -> Result<Vec<String>> {
        let mut recommendations = Vec::new();
        let analysis = self.analyze()?;
        
        if analysis.blocking_gaps.len() > 3 {
            recommendations.push(format!(
                "You have {} blocking gaps. Consider taking a step back to reassess approach.",
                analysis.blocking_gaps.len()
            ));
        }
        
        if analysis.by_status.get(&GapStatus::Unfilled).unwrap_or(&0) > &10 {
            recommendations.push(
                "Many unfilled gaps - consider focusing on critical path only.".to_string()
            );
        }
        
        let factual_unfilled = self.storage.get_gaps_by_type(GapType::Factual)?
            .into_iter()
            .filter(|g| g.status == GapStatus::Unfilled)
            .count();
        
        if factual_unfilled > 5 {
            recommendations.push(
                "Several factual gaps unfilled - spend time reading documentation.".to_string()
            );
        }
        
        Ok(recommendations)
    }
}
```

---

## Real-World Example (ROCmForge Debugging)

```rust
let analyzer = KnowledgeGapAnalyzer::new(storage);
analyzer.connect_hypothesis_board(&board);
analyzer.connect_belief_graph(&graph);

// Auto-identify gaps from current state
let gaps = analyzer.identify_gaps()?;
for gap in &gaps {
    println!("Gap: {} ({})", gap.question, gap.criticality);
}

// Manual registration of specific gaps
analyzer.register_gap(KnowledgeGap {
    id: id1,
    question: "Is GGUF tensor offset relative or absolute?".to_string(),
    gap_type: GapType::Factual,
    criticality: Criticality::Blocking,
    status: GapStatus::Unfilled,
    created_at: Utc::now(),
    filled_at: None,
    blocking: vec![
        BlockedItem {
            item_type: BlockedType::Belief,
            item_id: offset_belief_id.to_string(),
            description: "Offset calculation is correct".to_string(),
        }
    ],
    fill_strategy: FillStrategy::ReadDocumentation {
        source: "GGUF Specification".to_string(),
        section: Some("Tensor Data Layout".to_string()),
    },
    dependencies: vec![],
    estimated_effort: EffortEstimate::Quick,
})?;

// Get analysis
let analysis = analyzer.analyze()?;
println!("Total gaps: {}", analysis.total_gaps);
println!("Blocking: {}", analysis.blocking_gaps.len());
println!("Quick wins: {}", analysis.quick_wins.len());

// Get suggestion
if let Some(suggestion) = analyzer.suggest_next_action()? {
    println!("\nSuggested next action:");
    println!("  Fill gap: {}", suggestion.gap.question);
    println!("  Rationale: {}", suggestion.rationale);
    
    // Execute the suggestion
    match suggestion.action {
        SuggestedAction::FillGap(gap_id) => {
            let gap = analyzer.get_gap(gap_id)?;
            println!("  Strategy: {:?}", gap.fill_strategy);
            // ... execute the strategy
        }
        _ => {}
    }
}

// Fill a gap
analyzer.fill_gap(
    id1,
    "GGUF spec says tensor offsets are RELATIVE to tensor_data_start",
    Some(Evidence {
        // ... evidence from GGUF spec
    })
)?;
```

---

## CLI Integration

```bash
# Show all gaps
forge gap list

# Show blocking gaps only
forge gap list --blocking

# Show gap analysis
forge gap analyze

# Register a new gap
forge gap add "What is the Q4_0 block size?" \
    --type factual \
    --criticality blocking \
    --blocks "hypothesis:offset_calculation"

# Mark gap as filled
forge gap fill <gap-id> --answer "18 bytes (2 scale + 16 nibbles)"

# Get next action suggestion
forge gap suggest

# Generate gap report
forge gap report --format markdown
```

---

## Success Metrics

- [ ] Zero critical gaps remain unfilled at conclusion
- [ ] Average time from gap identification to filling < 10 minutes
- [ ] No redundant information gathering (same gap filled twice)
- [ ] All blocking gaps identified within first 5 minutes of debugging
- [ ] Suggestions accepted > 80% of the time (indicates good recommendations)

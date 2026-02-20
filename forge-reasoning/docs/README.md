# Reasoning Tools Design Document

**Status**: Design Phase  
**Purpose**: Tools to help LLMs reason, remember, and retrace steps during complex debugging  
**Related**: Forge SDK integration, ROCmForge debugging workflows

---

## The Problem

Current LLM debugging suffers from:

| Issue | Symptom | Impact |
|-------|---------|--------|
| **Context loss** | "What was I testing 10 minutes ago?" | Repeated work, circular reasoning |
| **No contradiction detection** | "Weights are normalized" AND "Range is 6.7x" | Wasted hours on wrong assumptions |
| **No verification tracking** | "I should check X" → never does | Missed critical information |
| **No state rollback** | Can't undo radical experiment | Fear of exploration |
| **Lost reasoning chains** | "Why did I believe that?" | Can't verify logic |

---

## The Solution: 7 Reasoning Tools

### 1. [Hypothesis/Evidence Board](01_HYPOTHESIS_EVIDENCE_BOARD.md)
**Track competing explanations with Bayesian confidence**

```rust
let h1 = board.propose("Layer 2 weights corrupted", &["gguf"])?;
board.add_evidence(h1, EvidenceKind::ExperimentResult, -0.8, "Scales look normal", source)?;
board.conclude(h2, true)?;  // Confirm offset bug hypothesis
```

**Prevents**: Circular reasoning, premature dismissal of valid hypotheses

---

### 2. [Contradiction Detector](02_CONTRADICTION_DETECTOR.md)
**Surface logical inconsistencies immediately**

```rust
let c = detector.check_new_belief(&belief)?;
// → Contradiction: "Normalized weights" but "RMS=1.62"
// → Suggestion: Check normalize_weights() implementation
```

**Prevents**: Hours wasted on contradictory assumptions

---

### 3. [Automated Verification Runner](03_AUTOMATED_VERIFICATION_RUNNER.md)
**Execute planned checks automatically**

```rust
runner.register(VerificationPlan {
    name: "q4_0_check",
    check: VerificationCheck::ReferenceComparison { ... },
    on_pass: AddEvidence { hypothesis: h3, strength: 0.9 },
    on_fail: Halt { reason: "Dequant wrong" },
})?;
let results = runner.run_suite(&suite).await?;
```

**Prevents**: "I should check X" → never done

---

### 4. [Experiment Branching](04_EXPERIMENT_BRANCHING.md)
**Git-like branching for debugging state**

```bash
forge experiment branch fix-offset
# Try fix...
forge experiment merge fix-offset --strategy take-theirs
# Or: forge experiment abandon fix-offset --reason "Didn't work"
```

**Prevents**: Lost work, fear of exploration

---

### 5. [Belief Dependency Graph](05_BELIEF_DEPENDENCY_GRAPH.md)
**Track reasoning chains with impact analysis**

```rust
graph.add_dependency(b3, b1, DependencyType::Evidential, 0.9, "Python is reference")?;
let impact = graph.analyze_impact(b2)?;
// → If Rust dequant wrong, 5 beliefs affected
```

**Prevents**: Cascade failures, lone unsupported beliefs

---

### 6. [Knowledge Gap Analyzer](06_KNOWLEDGE_GAP_ANALYZER.md)
**Explicitly track what we need to know**

```rust
analyzer.register_gap(KnowledgeGap {
    question: "Is GGUF offset relative or absolute?",
    criticality: Criticality::Blocking,
    fill_strategy: ReadDocumentation { source: "GGUF Spec" },
})?;
let suggestion = analyzer.suggest_next_action()?;
// → Fill gap: "GGUF offset question" (blocking)
```

**Prevents**: Scattershot investigation, missed critical facts

---

### 7. [Temporal Checkpointing](07_TEMPORAL_CHECKPOINTING.md)
**Save/restore complete debugging state**

```rust
checkpoint_mgr.checkpoint("Before radical fix", &["pre_change"])?;
// ... try things ...
checkpoint_mgr.restore(cp_id)?;  // Rollback
let comparison = checkpoint_mgr.compare(before, after)?;
```

**Prevents**: Context loss across compaction, no rollback

---

## Integration Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     LLM Agent (Kimi Code CLI)                    │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │              Temporal Checkpoint Manager                  │  │
│  │           (Saves/restores all tool states)                │  │
│  └──────────────────────────────────────────────────────────┘  │
│                              │                                   │
│         ┌────────────────────┼────────────────────┐             │
│         ▼                    ▼                    ▼             │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐       │
│  │ Hypothesis  │◄──►│ Contradict. │◄──►│   Belief    │       │
│  │   Board     │    │  Detector   │    │    Graph    │       │
│  └──────┬──────┘    └──────┬──────┘    └──────┬──────┘       │
│         │                  │                  │                │
│         └──────────────────┼──────────────────┘                │
│                            ▼                                   │
│                   ┌─────────────────┐                          │
│                   │  Automated      │                          │
│                   │  Verification   │                          │
│                   │    Runner       │                          │
│                   └────────┬────────┘                          │
│                            │                                   │
│         ┌──────────────────┼──────────────────┐               │
│         ▼                  ▼                  ▼               │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐       │
│  │ Experiment  │    │   Knowledge │    │   Forge     │       │
│  │  Branching  │    │    Gap      │    │   SDK       │       │
│  │             │    │  Analyzer   │    │ (storage)   │       │
│  └─────────────┘    └─────────────┘    └─────────────┘       │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Tool Dependencies

```
Temporal Checkpointing ──► All other tools (saves their state)
    │
    ▼
Experiment Branching ──► Temporal Checkpointing (checkpoints as branch heads)
    │
    ├──► Hypothesis Board (branch-specific hypotheses)
    ├──► Belief Graph (branch-specific beliefs)
    └──► Knowledge Gap Analyzer (branch-specific gaps)

Contradiction Detector ◄──► Belief Graph (uses dependencies for root cause)
    │
    └──► Hypothesis Board (creates contradictions from evidence conflicts)

Automated Verification ──► Hypothesis Board (adds evidence on results)
    │
    └──► Knowledge Gap Analyzer (fills gaps with results)
```

---

## Implementation Priority

| Priority | Tool | Why First? | Effort |
|----------|------|------------|--------|
| **P0** | Temporal Checkpointing | Foundation for all others | Medium |
| **P0** | Hypothesis Board | Immediate debugging value | Low |
| **P0** | Contradiction Detector | Prevents biggest waste of time | Low |
| **P1** | Automated Verification | Multiplies effectiveness | Medium |
| **P1** | Knowledge Gap Analyzer | Guides investigation | Medium |
| **P1** | Belief Dependency Graph | Root cause analysis | High |
| **P2** | Experiment Branching | Advanced workflows | High |

---

## Integration with Forge SDK

The Forge SDK (`/home/feanor/Projects/forge`) provides:
- **Storage backends** (SQLite, Native V3)
- **Pub/Sub events** (real-time state changes)
- **Agent loop structure** (observe → plan → act → verify)

These reasoning tools extend Forge's `forge_agent` crate:

```rust
// forge_agent/src/reasoning/mod.rs
pub mod hypothesis_board;
pub mod contradiction_detector;
pub mod verification_runner;
pub mod experiment_branching;
pub mod belief_graph;
pub mod gap_analyzer;
pub mod checkpointing;

/// Unified reasoning layer for Forge agent
pub struct ReasoningLayer {
    pub checkpointing: TemporalCheckpointManager,
    pub hypothesis_board: HypothesisBoard,
    pub contradiction_detector: ContradictionDetector,
    pub belief_graph: BeliefGraph,
    pub gap_analyzer: KnowledgeGapAnalyzer,
    pub verification_runner: VerificationRunner,
    pub experiment_manager: ExperimentManager,
}
```

---

## Success Metrics for All Tools

| Metric | Target |
|--------|--------|
| Time to root cause | -30% vs unstructured debugging |
| Repeated work | Zero (all state checkpointed) |
| Missed contradictions | Zero (auto-detected) |
| Verification completion | 100% (vs ~30% manual) |
| Context loss events | Zero across compaction |
| Circular reasoning | Zero detected instances |

---

## Next Steps

1. **Implement Temporal Checkpointing** (foundation)
2. **Implement Hypothesis Board** (immediate value)
3. **Integrate with ROCmForge** for real-world testing
4. **Iterate based on debugging sessions**
5. **Add remaining tools incrementally**

---

## Design Documents

| Tool | Document |
|------|----------|
| Hypothesis Board | [01_HYPOTHESIS_EVIDENCE_BOARD.md](01_HYPOTHESIS_EVIDENCE_BOARD.md) |
| Contradiction Detector | [02_CONTRADICTION_DETECTOR.md](02_CONTRADICTION_DETECTOR.md) |
| Automated Verification | [03_AUTOMATED_VERIFICATION_RUNNER.md](03_AUTOMATED_VERIFICATION_RUNNER.md) |
| Experiment Branching | [04_EXPERIMENT_BRANCHING.md](04_EXPERIMENT_BRANCHING.md) |
| Belief Dependency Graph | [05_BELIEF_DEPENDENCY_GRAPH.md](05_BELIEF_DEPENDENCY_GRAPH.md) |
| Knowledge Gap Analyzer | [06_KNOWLEDGE_GAP_ANALYZER.md](06_KNOWLEDGE_GAP_ANALYZER.md) |
| Temporal Checkpointing | [07_TEMPORAL_CHECKPOINTING.md](07_TEMPORAL_CHECKPOINTING.md) |

---

*These tools are designed to help LLMs overcome their fundamental limitations in maintaining state, detecting logical errors, and executing planned actions. They form a "cognitive scaffold" that extends LLM capabilities during complex debugging tasks.*

# Tracking: Phase 4 - Agent Layer

**Phase**: 04 - Agent Layer
**Status**: 📋 Planned
**Started**: 2026-02-12

---

## Task Status

| Task ID | Task | Owner | Status | Notes |
|----------|-------|--------|---------|-------|
| 04-01 | Observation Phase | - | Pending | Graph-based context gathering |
| 04-02 | Policy Engine | - | Pending | Constraint validation system |
| 04-03 | Planning Engine | - | Pending | Execution plan generation |
| 04-04 | Mutation Engine | - | Pending | Transaction-based edits |
| 04-05 | Verification Engine | - | Pending | Post-mutation validation |
| 04-06 | Commit Engine | - | Pending | Transaction finalization |
| 04-07 | Agent Loop Integration | - | Pending | Full loop wiring |
| 04-08 | CLI Integration | - | Pending | User interface |
| 04-09 | Documentation | - | Pending | Examples and guides |

---

## Progress Summary

- **Tasks Completed**: 0 / 9
- **Overall Progress**: 0%

### Pending Tasks (9)

1. **04-01: Observation Phase** (P0, High, 4-5 days)
   - Create Observer struct
   - Implement gather_symbols, gather_references, gather_cfg
   - Add LLM integration for semantic understanding

2. **04-02: Policy Engine** (P0, High, 4-5 days)
   - Implement NoUnsafeInPublicAPI validation
   - Implement PreserveTests validation
   - Implement MaxComplexity validation
   - Add policy composition

3. **04-03: Planning Engine** (P0, High, 5-6 days)
   - Create Planner struct
   - Implement generate_steps, estimate_impact
   - Add conflict detection

4. **04-04: Mutation Engine** (P0, High, 4-5 days)
   - Create Mutator struct
   - Implement transaction support
   - Add rollback capability

5. **04-05: Verification Engine** (P0, Medium, 3-4 days)
   - Create Verifier struct
   - Implement compile, test, graph checks
   - Add diagnostic collection

6. **04-06: Commit Engine** (P0, Medium, 2-3 days)
   - Create Committer struct
   - Implement git integration
   - Add checkpoint system

7. **04-07: Agent Loop Integration** (P0, Medium, 2-3 days)
   - Wire all six phases
   - Implement run() method
   - Add error handling

8. **04-08: CLI Integration** (P1, Medium, 2-3 days)
   - Create CLI with clap
   - Implement subcommands
   - Add config support

9. **04-09: Documentation** (P1, Low, 1-2 days)
   - Add module examples
   - Create policy guide
   - Write CLI reference

---

## Dependencies

### External Dependencies to Add

| Crate | Version | Purpose | Status |
|--------|---------|---------|--------|
| git2 | 0.18 | VCS integration | Not added |
| clap | 4.4 | CLI framework | Not added |
| toml | 0.8 | Config parsing | Not added |

### Task Dependencies

```
04-01 (Observation)    → None (depends on Phase 1 complete)
04-02 (Policy)         → None
04-03 (Planning)        → 04-01
04-04 (Mutation)       → 04-03
04-05 (Verification)    → 04-04
04-06 (Commit)         → 04-05
04-07 (Loop)          → 04-01 through 04-06
04-08 (CLI)            → 04-07
04-09 (Docs)           → All implementation tasks
```

---

## Files to Create

| File | Status | LOC Target |
|-------|---------|------------|
| `forge_agent/src/observe.rs` | Not created | ≤ 300 |
| `forge_agent/src/policy.rs` | Not created | ≤ 350 |
| `forge_agent/src/planner.rs` | Not created | ≤ 300 |
| `forge_agent/src/mutate.rs` | Not created | ≤ 300 |
| `forge_agent/src/verify.rs` | Not created | ≤ 250 |
| `forge_agent/src/commit.rs` | Not created | ≤ 200 |
| `forge_agent/src/cli.rs` | Not created | ≤ 200 |
| `docs/AGENT.md` | Not created | - |

---

## Blockers

**Current Blockers**: None

**Recent Blockers Resolved**:
- N/A (phase not started)

---

## Notes

- Phase 1 (Core SDK) and Phase 2 (Runtime Layer) are complete
- Agent stub exists in `forge_agent/src/lib.rs` with types defined
- All methods return errors with "not yet implemented"
- This phase will make the agent fully functional

---

*Last updated: 2026-02-12*

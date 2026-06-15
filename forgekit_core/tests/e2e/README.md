# Forge Core E2E Tests - TDD Waves

**Status**: ✅ **COMPLETE** - 53/53 Tests Passing  
**Goal**: End-to-end tests for ForgeKit SDK

---

## Wave Plan

| Wave | Focus | Tests | Status |
|------|-------|-------|--------|
| Wave 1 | Core Initialization & Storage | 5 | ✅ Complete |
| Wave 2 | Graph Module - Symbol Queries | 5 | ✅ Complete |
| Wave 3 | Search Module - Semantic Search | 5 | ✅ Complete |
| Wave 4 | Edit Module - Code Modifications | 5 | ✅ Complete |
| Wave 5 | CFG Module - Control Flow | 5 | ✅ Complete |
| Wave 6 | Analysis Module - Combined Ops | 5 | ✅ Complete |
| Wave 7 | Full Workflow E2E | 5 | ✅ Complete |
| Wave 8 | Tree-sitter CFG (C/Java/Rust) | 18 | ✅ Complete |

**Total**: 53 E2E tests passing

---

## Test Structure

```
tests/e2e/
├── README.md                    # This file
├── mod.rs                       # Test module entry
├── wave_01_core.rs             # ✅ Core SDK tests (5)
├── wave_02_graph.rs            # ✅ Graph module tests (5)
├── wave_03_search.rs           # ✅ Search module tests (5)
├── wave_04_edit.rs             # ✅ Edit module tests (5)
├── wave_05_cfg.rs              # ✅ CFG module tests (5)
├── wave_06_analysis.rs         # ✅ Analysis module tests (5)
├── wave_07_workflow.rs         # ✅ Workflow tests (5)
├── wave_08_treesitter_cfg.rs   # ✅ Real CFG for C/Java/Rust (18)
└── reports/                     # Phase reports
    ├── WAVE_05_CFG_REPORT.md
    ├── WAVE_06_ANALYSIS_REPORT.md
    ├── WAVE_07_WORKFLOW_REPORT.md
    ├── WAVE_08_TREESITTER_CFG_REPORT.md
    └── FINAL_SUMMARY.md
```

---

## Running Tests

```bash
# All E2E tests with full features (C, Java, Rust CFG)
cargo test --package forgekit-core --features treesitter-cfg --test e2e_tests

# Without tree-sitter (35 tests)
cargo test --package forgekit-core --test e2e_tests

# Specific waves
cargo test --package forgekit-core --features treesitter-cfg wave_08
cargo test --package forgekit-core wave_01

# With output
cargo test --package forgekit-core --features treesitter-cfg --test e2e_tests -- --nocapture
```

---

## Supported Languages for CFG Extraction

| Language | CFG Extraction | Loop Detection | Dominator Analysis |
|----------|---------------|----------------|-------------------|
| C | ✅ Full | ✅ | ✅ |
| Java | ✅ Full | ✅ | ✅ |
| Rust | ✅ Beta | ⚠️ Partial | ✅ |

---

## Documentation

### For Developers (The Safety Guides)

- [MANUAL.md](../MANUAL.md) - **Start here!** The Anti-Hallucination Check, Audit Trails, and "I Screwed Up" button
- [DEBUGGING.md](../DEBUGGING.md) - Emergency procedures and debugging workflows

### Test Reports

- [Wave 5: CFG Report](reports/WAVE_05_CFG_REPORT.md)
- [Wave 6: Analysis Report](reports/WAVE_06_ANALYSIS_REPORT.md)
- [Wave 7: Workflow Report](reports/WAVE_07_WORKFLOW_REPORT.md)
- [Wave 8: Tree-sitter CFG Report](reports/WAVE_08_TREESITTER_CFG_REPORT.md)
- [Final Summary](reports/FINAL_SUMMARY.md)

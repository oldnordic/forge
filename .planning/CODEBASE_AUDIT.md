# ForgeKit Codebase Audit - TODOs and Stubs

**Date**: 2026-02-12
**Purpose**: Assess what's real vs aspirational before deciding next steps

---

## Executive Summary

| Category | Count | Status |
|----------|--------|--------|
| Stub crates (entire file stub) | 0 | Good - no full stub crates |
| TODO comments | 6 | Moderate - tracked below |
| Module stub (partial implementation) | 1 | forge_runtime |
| unimplemented!() calls | 1 | forge_core/src/indexing.rs |
| Total Rust files | 24 | Baseline |

---

## What's COMPLETE ✅

### Core SDK (forge_core)

**Fully Functional:**
- `types.rs` - All core types defined (691 LOC)
- `error.rs` - Comprehensive error handling
- `lib.rs` - Main API entry point (512 LOC)
- `pool.rs` - Connection pooling (372 LOC)
- `cache.rs` - Query caching (413 LOC)
- `watcher.rs` - File watching with notify (436 LOC)
- `graph/mod.rs` - Graph module (238 LOC)
- `cfg/mod.rs` - CFG module (871 LOC)
- `edit/mod.rs` - Edit operations (391 LOC)
- `analysis/mod.rs` - Analysis operations (210 LOC)
- `storage/mod.rs` - Storage abstraction (347 LOC)

**Test Infrastructure:**
- 233+ tests passing
- Integration tests working
- Test utilities complete

### Agent Layer (forge_agent)

**Fully Functional:**
- `lib.rs` - Agent orchestration (369 LOC)
- `cli.rs` - CLI with clap v4 (174 LOC)
- `commit.rs` - Transaction finalization (~128 LOC)
- `mutate.rs` - Transaction-based edits (194 LOC)
- `verify.rs` - Post-mutation validation (198 LOC)
- `observe.rs` - Graph-based context gathering (534 LOC)
- `policy.rs` - Policy validation (587 LOC)
- `planner.rs` - Execution planning (425 LOC)

**Total agent code: ~2,600 LOC across 9 files**

---

## What's STUB / INCOMPLETE ⚠️

### forge_runtime (STUB CRATE)

**File**: `forge_runtime/src/lib.rs` (120 LOC)

**Status**: **Entire crate is a stub**

```rust
//! # Status
//!
//! This crate is currently a stub. Full implementation is planned for v0.3.
```

**TODOs (4)**:
- Line 80: `// TODO: Implement runtime initialization`
- Line 91: `// TODO: Implement runtime initialization`
- Line 100: `// TODO: Implement file watching`
- Line 106: `// TODO: Implement cache clearing`

**All methods return errors**:
- `new()` - Returns stub runtime
- `with_config()` - Returns stub runtime
- `watch()` - Returns "not yet implemented" error
- `clear_cache()` - Returns "not yet implemented" error
- `stats()` - Returns zero/placeholder stats

**Impact**: **HIGH** - Runtime layer is completely non-functional

---

## What's DEFERRED / TODO 📝

### forge_core/src/search/mod.rs

**TODO (1)**:
- Line 61: `// TODO: Implement via LLMGrep integration`
- **Current behavior**: Returns empty Vec for all searches
- **Impact**: **MEDIUM** - Search appears functional but filters through store only; no semantic search capability
- **Note**: This is marked as deferred for v0.1

### forge_core/src/indexing.rs

**Stub Implementation (1)**:
- Line 26: `/// # let store = unimplemented!();`
- **Impact**: **LOW** - Commented out (inactive); rest of module functional
- **Note**: Would prevent store writes if active

---

## File Size Analysis

**Largest files**:
1. `cfg/mod.rs` - 871 LOC (CFG implementation is substantial)
2. `types.rs` - 691 LOC (comprehensive type definitions)
3. `cache.rs` - 413 LOC
4. `pool.rs` - 372 LOC
5. `storage/mod.rs` - 347 LOC
6. `edit/mod.rs` - 391 LOC
7. `watcher.rs` - 436 LOC
8. `lib.rs` - 512 LOC

**Agent files (by LOC)**:
1. `policy.rs` - 587 LOC
2. `observe.rs` - 534 LOC
3. `planner.rs` - 425 LOC
4. `lib.rs` - 369 LOC
5. `verify.rs` - 198 LOC
6. `mutate.rs` - 194 LOC
7. `commit.rs` - ~128 LOC (estimated)

---

## Priority Assessment

### Priority 1: CRITICAL - forge_runtime stub crate

**Blocker**: The entire runtime layer is non-functional.
- All 4 core methods are stubs
- File watching doesn't work
- Cache clearing doesn't work
- Runtime initialization is placeholder

**Decision Required**: Should we implement forge_runtime for v0.3 or deprecate it?

**Context**: Given the Plan Kernel (C Mode) ADR direction:
- The Plan Kernel architecture provides event-based coordination
- Native-V3 has built-in pub/sub capabilities
- The forge_runtime layer as designed may be superseded by the kernel's event system

**Options**:
1. **Implement as designed** - File watching, caching, pooling as separate layer
2. **Supersede by Plan Kernel** - Runtime services become part of kernel's event coordination
3. **Defer indefinitely** - Use forge_core directly without runtime abstraction

### Priority 2: MEDIUM - Search LLMGrep integration

**Single TODO**: `forge_core/src/search/mod.rs:61`
- Search currently filters via store only
- LLMGrep integration deferred to v0.2+
- **Impact**: Semantic search not functional; searches return empty results unless matched by store

### Priority 3: LOW - Indexing store write

**Commented unimplemented**: `forge_core/src/indexing.rs:26`
- Currently not active (commented out)
- Would prevent index writes if active
- Low impact as indexing works via store queries

---

## Recommendations

### For Immediate Decision

**forge_runtime is the key question**

The entire runtime crate (120 LOC) is a stub. This represents:
- 0% functional code
- 4 TODO comments
- All methods returning errors

**Options**:
| Option | Description | Pro | Con |
|---------|-------------|------|------|
| A. Implement forge_runtime | File watching, caching work as designed | Matches original ROADMAP | 1-2 weeks |
| B. Supersede with Plan Kernel | Event-based coordination | Matches new ADR direction | Integrated with v0.5 |
| C. Remove forge_runtime entirely | Use forge_core directly | Simplest, lowest complexity | Immediate |

**Note**: The Plan Kernel (C Mode) ADR defines architecture that may make forge_runtime obsolete.

### For v0.2 Planning

**Search LLMGrep integration** - The semantic search TODO is the main gap before Core SDK implementation.

### Summary

- **Complete**: Core SDK (all modules), Agent Layer (all 9 modules), Tests (233 passing)
- **Stub**: forge_runtime (entire crate)
- **TODOs**: 6 total (4 in forge_runtime, 1 in search, 1 commented in indexing)
- **Total LOC**: ~8,400 in forge_core, ~2,600 in forge_agent, ~120 in forge_runtime

---

*Last updated: 2026-02-12*

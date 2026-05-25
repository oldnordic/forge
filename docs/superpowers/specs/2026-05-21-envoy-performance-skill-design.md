# Envoy Performance Evaluation Skill — Design Spec

**Date:** 2026-05-21
**Skill name:** `grounded-coding-perf`
**Location:** `/home/feanor/.claude/skills/grounded-coding-perf/SKILL.md`
**Target:** Envoy HTTP service at `http://127.0.0.1:9876`

---

## Purpose

Measure envoy latency, throughput, and resource usage with reproducible benchmarks. Complements `grounded-coding-doctor` (which checks health) with quantitative performance data.

## Scope

- Envoy-specific: knows endpoints, auth headers, registration lifecycle
- Single-machine only (no distributed load testing)
- Terminal output by default, optional markdown report file
- Requires `wrk`, `hey`, `perf`, `jq`, `sysstat` — skill includes install commands

## Out of Scope

- Flamegraph generation (requires additional tooling + sustained workload)
- Soak testing (hours-long)
- Distributed load testing
- Frontend/WebSocket performance

## Phases

### Phase 0 — Prerequisites

Check for `wrk`, `hey`, `perf`, `jq`, `pidstat`. Provide Arch install command. Abort if envoy is down.

### Phase 1 — Agent Registration

Register a perf-test agent via `/agents`, capture `AGENT_ID`, use in authenticated requests. Retire at end.

### Phase 2 — Warmup

10 sequential curl hits to `/health`. Results discarded.

### Phase 3 — Latency Profiling

Hit 5 envoy endpoints with 50 sequential `curl -w` requests each:
- `/health` (unauthenticated)
- `/agents` (list, authenticated)
- `/knowledge/search` (authenticated, POST)
- `/ontology` (authenticated)
- `/planning/status` (authenticated)

Capture: `time_connect`, `time_starttransfer`, `time_total`. Compute P50/P95/P99 via sort + awk.

Output table: endpoint | P50 | P95 | P99 | median TTFB.

### Phase 4 — Load Testing

Three wrk scenarios:
1. `wrk -t4 -c50 -d15s /health` — baseline
2. `wrk -t4 -c100 -d15s /health` — high concurrency
3. `wrk -t2 -c20 -d10s /agents` — authenticated endpoint

Optional: POST script for `/knowledge/search` with sample payload.

### Phase 5 — Resource Profiling

- Capture envoy PID
- Snapshot `/proc/<pid>/status` before load (VmRSS, VmSize, Threads)
- `perf stat -p <pid> -d 15` during load — cycles, instructions, cache misses
- Snapshot `/proc/<pid>/status` after load, diff memory delta
- If `pidstat` available: `pidstat -p <pid> 1 15`

### Phase 6 — Report

Optional `--report` flag writes `.perf-reports/YYYY-MM-DD-HHMM.md` with all results. Without flag, terminal-only.

## Cross-References

- Uses endpoint patterns from `grounded-coding-atheneum`
- Complements `grounded-coding-doctor`
- Follows same SKILL.md frontmatter and section conventions

## Success Criteria

- Skill runs end-to-end against a live envoy instance
- Produces P50/P95/P99 latency table for all 5 endpoints
- wrk throughput numbers for 3 scenarios
- Memory delta (before/after load test)
- Optional report file generation works

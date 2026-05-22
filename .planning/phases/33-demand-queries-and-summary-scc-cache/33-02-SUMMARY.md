---
phase: 33-demand-queries-and-summary-scc-cache
plan: 02
subsystem: analysis-engine
tags: [demand-query, memoization, trace, kernel-run-report, incremental]

# Dependency graph
requires:
  - phase: 33-demand-queries-and-summary-scc-cache
    plan: 01
    provides: demand query contracts (QueryKind, QueryBudget, QueryStatus), QueryContext, QueryTrace
  - phase: 23-input-snapshot-and-cache-identity
    provides: KernelRunReport, CacheStats, InputSnapshot, ProviderOutputMeta
provides:
  - DemandQueryEngine with BTreeMap-based in-run memoization keyed by QueryKey
  - DemandQueryResult with query_key, output_digest, precision_tier, provenance, was_cached
  - DemandQueryTrace and DemandQueryTraceEntry for kernel-level demand query debug output
  - KernelRunReport.demand_query_trace field with demand query stats folded into aggregate CacheStats
affects: [33-04-scc-closure, 34-rust-extension-provider-sink, demand-query-consumers]

# Tech tracking
tech-stack:
  added: []
  patterns: [in-run demand query memoization via BTreeMap, demand query trace recording at kernel level, aggregate demand stats in CacheStats]

key-files:
  created:
    - crates/polint/src/analysis_kernel/incremental/demand.rs
  modified:
    - crates/polint/src/analysis_kernel/incremental/mod.rs
    - crates/polint/src/analysis_kernel/incremental/run_report.rs
    - crates/polint/src/analysis_kernel/mod.rs

key-decisions:
  - "DemandQueryEngine lives in analysis_kernel/incremental/ to compose with the existing layer cache and run report infrastructure."
  - "In-run memoization uses BTreeMap<QueryKey, DemandQueryResult> for deterministic iteration order."
  - "Trace entries record query kind, version, parameter digest, input layer digests, cache status, compute duration, result digest, and precision tier."
  - "Demand query hits/misses fold into aggregate CacheStats via aggregate_demand_query_stats helper."
  - "Default DemandQueryTrace::default() is passed in existing KernelRunReport::new calls to preserve current behavior."

patterns-established:
  - "DemandQueryEngine pattern: lookup/insert memoization + automatic trace entry recording"
  - "Cache status vocabulary: 'computed' for insert, 'hit' for record_cache_hit, 'miss' for future recompute"
  - "Demand query stats aggregation: hits counted from trace entries with 'hit' status, recomputes from 'miss'/'computed'"

requirements-completed: []

# Metrics
duration: 4min
completed: 2026-05-22
---

# Phase 33 Plan 02: Demand Query Infrastructure Contracts Summary

**DemandQueryEngine with BTreeMap-based in-run memoization, kernel-level trace recording, and KernelRunReport demand_query_trace integration**

## Performance

- **Duration:** 4 min
- **Started:** 2026-05-22T09:11:51Z
- **Completed:** 2026-05-22T09:16:07Z
- **Tasks:** 2/2
- **Files modified:** 4 (1 created, 3 modified)

## Accomplishments
- Created DemandQueryEngine at analysis_kernel/incremental/demand.rs with BTreeMap<QueryKey, DemandQueryResult> memoization
- DemandQueryResult carries query_key, output_digest, precision_tier, provenance, and was_cached fields
- DemandQueryTraceEntry records query_kind, query_version, parameter_digest, input_layer_digests, cache_status, compute_duration_micros, result_digest, precision_tier
- DemandQueryTrace wraps Vec<DemandQueryTraceEntry> with record_entry, entries, len, is_empty methods
- DemandQueryEngine provides lookup, insert, record_cache_hit, into_trace, and trace methods
- Extended KernelRunReport with demand_query_trace: DemandQueryTrace field
- Added aggregate_demand_query_stats helper that folds demand hit/miss into aggregate CacheStats
- All 1042+ existing tests pass, clippy clean with -D warnings
- 6 new unit tests covering: insert/lookup, absent key, trace ordering, into_trace, cache hit recording, default engine

## Task Commits

Each task was committed atomically:

1. **Task 1: Create demand query engine module with memoization and trace types** - `b46a9c9` (feat)
2. **Task 2: Extend KernelRunReport with demand query trace** - `f936bcc` (feat)

## Files Created/Modified
- `crates/polint/src/analysis_kernel/incremental/demand.rs` - DemandQueryEngine, DemandQueryResult, DemandQueryTrace, DemandQueryTraceEntry with unit tests
- `crates/polint/src/analysis_kernel/incremental/mod.rs` - Added demand module and re-exports for DemandQueryEngine, DemandQueryResult, DemandQueryTrace, DemandQueryTraceEntry
- `crates/polint/src/analysis_kernel/incremental/run_report.rs` - Added demand_query_trace field to KernelRunReport, updated new() signature, added aggregate_demand_query_stats
- `crates/polint/src/analysis_kernel/mod.rs` - Updated KernelRunReport::new call to pass DemandQueryTrace::default()

## Decisions Made
- DemandQueryEngine is placed in `analysis_kernel/incremental/demand.rs` (not `analysis/demand/`) to compose directly with the incremental cache infrastructure (QueryKey, KernelRunReport, CacheStats).
- BTreeMap used for memoization rather than HashMap for deterministic iteration order matching established project patterns.
- DemandQueryTraceEntry uses String fields for display-form digests and precision tier rather than raw types, keeping trace serialization simple for debug JSON.
- The aggregate_demand_query_stats function counts "hit" entries as cache hits and "miss"/"computed" entries as recomputes, consistent with existing CacheStats semantics.
- Default (empty) DemandQueryTrace preserves all existing behavior until Plan 04 wires real demand queries.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- DemandQueryEngine is ready for Plan 04 (SCC closure) to wire as the memoization substrate for summary SCC fixpoint computation.
- KernelRunReport demand_query_trace field is ready to receive real trace data once demand queries are executed during kernel runs.
- DemandQueryTrace entries will feed into eval fixture validation for demand query cache behavior.

## Self-Check: PASSED

All files verified present. Both commits verified in git log.

---
*Phase: 33-demand-queries-and-summary-scc-cache*
*Completed: 2026-05-22*

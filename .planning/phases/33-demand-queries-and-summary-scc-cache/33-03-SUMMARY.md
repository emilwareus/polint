---
phase: 33-demand-queries-and-summary-scc-cache
plan: 03
subsystem: analysis
tags: [scc, tarjan, petgraph, call-graph, summaries, scheduling]

# Dependency graph
requires:
  - phase: 33-01
    provides: demand query infrastructure with SccGraph in analysis/demand/scc.rs
  - phase: 33-02
    provides: DemandQueryEngine with memoization and trace
  - phase: 30
    provides: direct call target facts in CallStore
  - phase: 32
    provides: direct summaries in SummaryStore
provides:
  - SCC discovery from direct call target facts via compute_scc_schedule
  - Scc, SccSchedule, SccScheduleDebug types for summary scheduling
  - Reverse topological SCC ordering with deterministic tie-breaking
  - Recursive vs non-recursive SCC classification
affects: [33-04, 33-05, 37-refined-call-graph, 38-data-flow]

# Tech tracking
tech-stack:
  added: []
  patterns: [petgraph tarjan_scc for SCC from AnalysisDb, BTreeMap-keyed node map for deterministic graph construction]

key-files:
  created:
    - crates/polint/src/analysis/summaries/scc.rs
  modified:
    - crates/polint/src/analysis/summaries/mod.rs

key-decisions:
  - "Used petgraph::algo::tarjan_scc directly on DiGraph<FunctionId, ()> instead of reusing analysis/demand/scc.rs SccGraph (different abstraction levels: demand/scc works on BTreeMap<String, BTreeSet<String>> call edges, summaries/scc works on AnalysisDb with FunctionId-keyed stores)"
  - "SCC member ordering uses callable_stable_key from SummaryFact for determinism (D-17), not FunctionId numeric ordering"
  - "Only Resolved call targets create edges; Unresolved/Unsupported/SetupMissing targets are excluded from the SCC graph"
  - "Only functions with at least one SummaryFact participate in the SCC graph; functions without summaries are excluded even if they appear as call targets"

patterns-established:
  - "compute_scc_schedule pattern: query SummaryStore for function set, build DiGraph from CallStore edges, run tarjan_scc, sort members by stable key"

requirements-completed: [SAE-INT-03]

# Metrics
duration: 3min
completed: 2026-05-22
---

# Phase 33 Plan 03: SCC Discovery from Direct Call Targets Summary

**SCC discovery builds a petgraph call graph from CallStore/SummaryStore facts and computes Tarjan SCCs in reverse topological order with deterministic stable-key tie-breaking for summary scheduling**

## Performance

- **Duration:** 3 min
- **Started:** 2026-05-22T09:18:32Z
- **Completed:** 2026-05-22T09:21:49Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments
- Implemented compute_scc_schedule that builds a DiGraph from CallStore outgoing edges between summary-bearing functions and computes SCCs via petgraph tarjan_scc
- Defined Scc, SccSchedule, and SccScheduleDebug types with Serialize support for debug output (D-14)
- SCCs are scheduled in reverse topological order (leaf callees first) per D-04 with deterministic ordering per D-17
- Recursive vs non-recursive SCC classification for fixpoint vs single-pass computation per D-05
- 9 unit tests covering empty, chain, self-call, mutual recursion, independence, exclusion of unresolved targets, and serialization

## Task Commits

Each task was committed atomically:

1. **Task 1: Create SCC types and compute_scc_schedule function** - `928356b` (feat)

## Files Created/Modified
- `crates/polint/src/analysis/summaries/scc.rs` - Scc, SccSchedule, SccScheduleDebug types and compute_scc_schedule function with 9 unit tests
- `crates/polint/src/analysis/summaries/mod.rs` - Added pub(crate) mod scc declaration

## Decisions Made
- Used petgraph::algo::tarjan_scc on DiGraph<FunctionId, ()> rather than the existing analysis/demand/scc.rs SccGraph which operates on abstract string-keyed call edges. The summaries/scc module works directly with AnalysisDb stores for type safety and avoids an unnecessary string conversion layer.
- callable_stable_key from SummaryFact is used as the stable key for member ordering within SCCs, ensuring deterministic output across runs regardless of FunctionId assignment order.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- SCC scheduling infrastructure is ready for interprocedural summary closure (future plans)
- SccSchedule provides the processing order needed by fixpoint iteration consumers
- SccScheduleDebug provides trace output for eval/debug per D-14

---
*Phase: 33-demand-queries-and-summary-scc-cache*
*Completed: 2026-05-22*

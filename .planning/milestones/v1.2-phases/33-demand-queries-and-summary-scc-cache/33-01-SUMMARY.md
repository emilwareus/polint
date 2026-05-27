---
phase: 33-demand-queries-and-summary-scc-cache
plan: 01
subsystem: analysis-engine
tags: [demand-query, scc, tarjan, fixpoint, quarantine, cache, trace, incremental]

# Dependency graph
requires:
  - phase: 32-summary-kernel-and-direct-summaries
    provides: summary contracts, summary store, direct summary builder, summary provider
  - phase: 31-p0-abstract-domain-kernel
    provides: abstract domain facts, solver, domain provider
  - phase: 30-direct-call-facts
    provides: call site, call target, and unresolved call facts
provides:
  - private demand query contracts (QueryKind, QueryBudget, QueryStatus, QueryResult)
  - dependency-tracking QueryContext with layer/query/summary/extension read recording
  - iterative Tarjan SCC decomposition with topological ordering for bottom-up summary computation
  - SCC cache entries with backdating validation for summary fixpoint reuse
  - extension-aware quarantine set with six typed quarantine reasons
  - query trace types for internal debug/eval output
  - DemandQuery layer kind for demand query cache identity
  - public-boundary proof with 21 internal marker assertions
affects: [34-rust-extension-provider-sink, 35-framework-entrypoints, demand-query-provider, summary-scc-provider]

# Tech tracking
tech-stack:
  added: []
  patterns: [demand-driven query execution, SCC-ordered summary fixpoint, extension quarantine, query trace recording]

key-files:
  created:
    - crates/polint/src/analysis/demand/mod.rs
    - crates/polint/src/analysis/demand/query.rs
    - crates/polint/src/analysis/demand/context.rs
    - crates/polint/src/analysis/demand/scc.rs
    - crates/polint/src/analysis/demand/quarantine.rs
    - crates/polint/src/analysis/demand/trace.rs
  modified:
    - crates/polint/src/analysis/mod.rs
    - crates/polint/src/analysis_kernel/incremental/keys.rs
    - crates/polint/src/analysis_kernel/provider.rs

key-decisions:
  - "Keep demand query contracts crate-private under analysis::demand with no SDK, runner, CLI, or public surface."
  - "Use iterative Tarjan's SCC algorithm to avoid stack overflow on large call graphs."
  - "SCC graph produces bottom-up topological order where callees come before callers for summary computation."
  - "Extension quarantine uses six typed reasons rather than generic strings for machine-processable quarantine evidence."
  - "Query trace is optional and only collected when trace mode is enabled to avoid production overhead."
  - "DemandQuery added as a LayerKind variant for future demand query cache identity."
  - "Public-boundary proof uses 21 specific internal markers covering types, modules, and function names."

patterns-established:
  - "Demand query pattern: QueryKind + QueryBudget + QueryContext dependency tracking + QueryResult typed output"
  - "SCC scheduling pattern: Tarjan decomposition -> topological iteration -> per-SCC fixpoint with budget"
  - "Extension quarantine pattern: QuarantineSet tracks quarantined keys with typed reasons and extension digests"
  - "Query trace pattern: optional trace recording with typed entries for debug/eval output"

requirements-completed: []

# Metrics
duration: 12min
completed: 2026-05-22
---

# Phase 33 Plan 01: Demand Query Contracts, SCC Scheduling, Extension Quarantine, and Query Trace Summary

**Private demand query layer with iterative Tarjan SCC decomposition, dependency-tracking query context, extension-aware quarantine, and typed query trace for expensive analysis views**

## Performance

- **Duration:** 12 min
- **Started:** 2026-05-22T08:49:10Z
- **Completed:** 2026-05-22T09:01:05Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments
- Added private demand query contracts covering seven query families (function CFG, def-use, direct call target, function summary, diagnostic evidence, bounded alias, summary SCC fixpoint)
- Implemented iterative Tarjan SCC algorithm with correct bottom-up topological ordering for summary fixpoint computation
- Added dependency-tracking QueryContext with layer, query, summary, and extension read recording plus in-run memoization
- Added extension-aware QuarantineSet with six typed quarantine reasons for stale extension output isolation
- Added typed QueryTrace for optional debug/eval output with entries for query lifecycle, SCC iterations, and quarantine skips
- Verified all 1036 existing tests pass and demand query internals do not leak to any public surface

## Task Commits

Each task was committed atomically:

1. **Task 1: Private demand query contracts, SCC scheduling, extension quarantine, and query trace** - `2710655` (feat)
2. **Task 2: DemandQuery layer kind and public-boundary proof** - `cc9305a` (feat)

## Files Created/Modified
- `crates/polint/src/analysis/demand/mod.rs` - Module declarations for demand query submodules
- `crates/polint/src/analysis/demand/query.rs` - QueryKind, QueryBudget, QueryStatus, QueryResult, demand_query_key
- `crates/polint/src/analysis/demand/context.rs` - QueryContext with dependency tracking, memoization, and depth limits
- `crates/polint/src/analysis/demand/scc.rs` - SccGraph, SccComponent, SccId, SccCacheEntry, SccFixpointStatus, iterative Tarjan SCC
- `crates/polint/src/analysis/demand/quarantine.rs` - QuarantineSet, QuarantineEntry, QuarantineReason
- `crates/polint/src/analysis/demand/trace.rs` - QueryTrace, QueryTraceEntry with typed debug JSON
- `crates/polint/src/analysis/mod.rs` - Added demand module declaration
- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Added DemandQuery to LayerKind enum
- `crates/polint/src/analysis_kernel/provider.rs` - Added public-boundary proof test with 21 markers

## Decisions Made
- Keep all demand query types crate-private under `analysis::demand` with no SDK, runner, CLI, or public surface promotion.
- Use iterative (not recursive) Tarjan's SCC algorithm to avoid stack overflow on large call graphs with deep recursive chains.
- SCC graph is built from callable stable keys with BTreeMap/BTreeSet for deterministic ordering.
- SCC topological order puts callees before callers (bottom-up) for summary fixpoint computation.
- SCC cache entries support backdating validation: if dependency SCC digests match, the entry is reusable even if upstream sources changed.
- Extension quarantine uses typed `QuarantineReason` variants rather than generic strings so quarantine evidence is machine-processable.
- Query trace recording is optional (controlled by `trace_enabled` flag) to avoid production overhead.
- QueryBudget has configurable max iterations (100), max nodes (10,000), and max depth (64) with explicit budget-exceeded status.
- In-run memoization table in QueryContext avoids redundant query execution within a single analysis run.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed iterative Tarjan SCC algorithm stack management**
- **Found during:** Task 1 (SCC implementation)
- **Issue:** Initial iterative Tarjan implementation had a bug in the call stack management where lowlink propagation from child to parent was not happening correctly, causing panics on the SCC stack.
- **Fix:** Rewrote the iterative algorithm using a simpler frame structure with `(node, next_neighbor)` tracking and correct lowlink propagation after child completion.
- **Files modified:** `crates/polint/src/analysis/demand/scc.rs`
- **Verification:** All 6 SCC tests pass including mutual recursion, topological ordering, and complex multi-SCC graphs.
- **Committed in:** `2710655` (part of task 1 commit)

**2. [Rule 1 - Bug] Fixed DigestKind variant name for quarantine digests**
- **Found during:** Task 1 (quarantine implementation)
- **Issue:** Used `DigestKind::Extension` which doesn't exist; the correct variant is `DigestKind::ExtensionCode`.
- **Fix:** Changed to `DigestKind::ExtensionCode` in quarantine digest construction.
- **Files modified:** `crates/polint/src/analysis/demand/quarantine.rs`
- **Verification:** Compilation succeeds, quarantine digest tests pass.
- **Committed in:** `2710655` (part of task 1 commit)

---

**Total deviations:** 2 auto-fixed (2 bugs)
**Impact on plan:** Both auto-fixes necessary for correctness. No scope creep.

## Issues Encountered
None beyond the auto-fixed issues above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Demand query contracts are ready for wiring into the kernel run sequence in plan 33-02.
- SCC graph can be built from call facts once call store indexes are accessible to the demand layer.
- Extension quarantine is ready for integration with extension provider sink in Phase 34.
- Query trace is ready for integration with eval fixtures and debug output in later plans.

## Self-Check: PASSED

All files verified present. Both commits verified in git log.

---
*Phase: 33-demand-queries-and-summary-scc-cache*
*Completed: 2026-05-22*

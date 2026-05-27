---
phase: 33-demand-queries-and-summary-scc-cache
plan: 04
subsystem: analysis
tags: [scc, summaries, fixpoint, backdating, demand-query, interprocedural]

# Dependency graph
requires:
  - phase: 33-demand-queries-and-summary-scc-cache
    plan: 01
    provides: demand query contracts and SCC vocabulary
  - phase: 33-demand-queries-and-summary-scc-cache
    plan: 02
    provides: DemandQueryEngine and DemandQueryTrace
  - phase: 33-demand-queries-and-summary-scc-cache
    plan: 03
    provides: compute_scc_schedule and deterministic SCC order
  - phase: 32-summary-kernel-and-direct-summaries
    provides: SummaryStore, SummaryFact, and direct summaries
provides:
  - interprocedural SCC summary closure
  - recursive SCC fixpoint iteration with budget status
  - SCC output digest backdating
  - SCC closure demand-query trace entries
  - kernel wiring after direct summaries and before metrics
affects: [33-06-validation-debug-json, 33-07-eval-proof, 37-refined-call-graphs, 38-data-flow]

# Tech tracking
tech-stack:
  added: []
  patterns: [SCC-ordered summary closure, bounded fixpoint, backdating, demand-query tracing]

key-files:
  created:
    - crates/polint/src/analysis/summaries/closure.rs
  modified:
    - crates/polint/src/analysis/summaries/mod.rs
    - crates/polint/src/analysis/summaries/provider.rs
    - crates/polint/src/analysis_kernel/mod.rs

key-decisions:
  - "SCC closure stays crate-private under analysis::summaries with no public SDK, runner, CLI, README, or docs/facts promotion."
  - "Non-recursive SCCs run a single interprocedural pass over current callee summaries."
  - "Recursive SCCs iterate with the configured budget and produce BudgetExceeded summaries instead of silently claiming convergence."
  - "Backdating compares deterministic SCC output digests against previous SCC digests and counts unchanged SCCs."
  - "Kernel execution runs SCC closure after direct summaries and passes its DemandQueryTrace into KernelRunReport."

patterns-established:
  - "close_summaries_by_scc pattern: SccSchedule -> per-SCC closure -> output digest -> optional backdating -> DemandQueryEngine trace"
  - "SccClosureProviderOutput pattern: schedule discovery, closure result, diagnostics, and demand query trace returned as one provider-level output"

requirements-completed: [SAE-INT-03]

# Metrics
duration: recovered from existing implementation commits
completed: 2026-05-22
---

# Phase 33 Plan 04: Interprocedural Summary SCC Closure Summary

**SCC-ordered interprocedural summary closure is implemented and wired into the kernel after direct summaries, with recursive fixpoint budgeting, backdating, and demand-query trace recording.**

## Performance

- **Duration:** recovered from existing implementation commits
- **Completed:** 2026-05-22
- **Tasks:** 2/2
- **Files modified:** 4

## Accomplishments

- Added `crates/polint/src/analysis/summaries/closure.rs` with `SccClosureConfig`, `SccClosureResult`, and `close_summaries_by_scc`.
- Implemented non-recursive SCC closure, recursive SCC iteration, explicit budget-exceeded accounting, deterministic SCC output digesting, and backdating counts.
- Wired `run_scc_closure` into the summary provider and kernel sequence after direct summaries.
- Passed SCC closure `DemandQueryTrace` into `KernelRunReport` so downstream debug/eval work can observe real demand-query execution.
- Added unit coverage for non-recursive callee propagation, recursive SCC convergence or budget exhaustion, backdating, provider ordering, demand trace rows, and empty-schedule skips.

## Task Commits

Existing production commits found in history:

1. **Task 1: Add SCC closure module with fixpoint iteration and backdating** - `c95b0b2` (feat)
2. **Task 2: Wire SCC closure into kernel provider sequence after direct summaries** - `8cdceb2` (feat)

These commits are included in the current merged `origin/main` commit `7f1bf53`. This summary closes out the missing GSD artifact for the already-shipped implementation.

## Files Created/Modified

- `crates/polint/src/analysis/summaries/closure.rs` - SCC closure config/result, closure execution, output digesting, backdating, and tests
- `crates/polint/src/analysis/summaries/mod.rs` - Added `closure` module
- `crates/polint/src/analysis/summaries/provider.rs` - Added `run_scc_closure` and provider tests
- `crates/polint/src/analysis_kernel/mod.rs` - Runs SCC closure after direct summaries and forwards demand query trace into `KernelRunReport`

## Decisions Made

- Keep SCC closure internal and crate-private.
- Use deterministic SCC member stable keys as the backdating key.
- Treat cross-run previous SCC digest loading as future work; the current provider call supplies an empty previous digest map while the closure API supports backdating.
- Emit budget diagnostics from provider output when SCC closure reports budget exhaustion.

## Deviations from Plan

### Close-Out Recovery

- **Found during:** execute-phase resume for Phase 33
- **Issue:** Production commits for `33-04` existed, but `33-04-SUMMARY.md` was missing. This violated the GSD close-out invariant.
- **Fix:** Inspected the existing commits, verified plan acceptance criteria against the current code, and wrote this missing summary artifact before proceeding to later plans.
- **Files modified:** `.planning/phases/33-demand-queries-and-summary-scc-cache/33-04-SUMMARY.md`
- **Verification:** See verification commands below.

**Total deviations:** 1 close-out recovery.
**Impact:** No production code changes were needed; this aligns GSD artifacts with already-merged implementation work.

## Issues Encountered

None in the recovered implementation. Cargo verification initially waited on normal package/artifact locks while parallel commands compiled dependencies.

## Verification

- `cargo test --lib -p polint -- scc` - passed, 24 tests
- `cargo test --lib -p polint -- closure` - passed, 13 tests
- `cargo test --lib -p polint -- direct_summaries` - passed, 8 tests
- `cargo clippy -p polint -- -D warnings` - passed

## User Setup Required

None.

## Next Phase Readiness

- `33-06` can now extend validation/debug JSON using the SCC closure result and demand-query trace wiring.
- `33-07` can add eval/no-leak proof once debug/eval observation includes SCC and demand-query sections.

## Self-Check: PASSED

- `closure.rs`: FOUND
- `close_summaries_by_scc`: FOUND
- `run_scc_closure`: FOUND
- kernel SCC closure call: FOUND
- demand query trace forwarding: FOUND
- verification commands: PASSED

---
*Phase: 33-demand-queries-and-summary-scc-cache*
*Completed: 2026-05-22*

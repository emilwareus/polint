---
phase: 33-demand-queries-and-summary-scc-cache
plan: 06
subsystem: analysis-kernel
tags: [validation, debug-json, scc, demand-query, observability]

# Dependency graph
requires:
  - phase: 33-demand-queries-and-summary-scc-cache
    plan: 03
    provides: deterministic SCC schedule computation
  - phase: 33-demand-queries-and-summary-scc-cache
    plan: 04
    provides: SCC closure result and demand query trace
  - phase: 33-demand-queries-and-summary-scc-cache
    plan: 05
    provides: cache quarantine vocabulary for later validation consumers
provides:
  - SCC closure provenance validation inside validate_summaries
  - BudgetExceeded evidence validation for SCC-produced summaries
  - Exact precision rejection for interprocedural closure summaries
  - metadata debug JSON sections for SCC schedule, closure execution stats, and demand query trace rows
affects: [33-07-eval-proof, phase-34-extension-provider-sink]

# Tech tracking
tech-stack:
  added: []
  patterns: [crate-private test-facing debug JSON, interprocedural summary provenance, closure-result-backed observability]

key-files:
  modified:
    - crates/polint/src/analysis/summaries/facts.rs
    - crates/polint/src/analysis/summaries/closure.rs
    - crates/polint/src/analysis/summaries/validate.rs
    - crates/polint/src/analysis_kernel/debug.rs
    - crates/polint/src/analysis_kernel/incremental/run_report.rs
    - crates/polint/src/analysis_kernel/mod.rs

key-decisions:
  - "Added SummaryProvenance::InterproceduralClosure so SCC-produced summaries can be validated distinctly from local direct summaries."
  - "Kept metadata debug output behind cfg(test) and crate-private helpers; no public SDK, runner, CLI, README, or docs/facts surface was promoted."
  - "Stored the SCC closure result in KernelRunReport so debug JSON can include iteration counts and backdating stats without recomputing closure."
  - "Demand query debug rows include precision tier, cache status, compute duration, and a bounded result digest prefix."

patterns-established:
  - "validate_scc_closure_results pattern: validate summary provenance/evidence/precision from existing AnalysisDb stores."
  - "metadata_debug_json_with_demand_trace_for_test pattern: AnalysisDb facts plus run-report execution trace become one deterministic debug report."

requirements-completed: [SAE-INT-03]

# Metrics
duration: in-session
completed: 2026-05-22
---

# Phase 33 Plan 06: Validation and Debug JSON Summary

**SCC closure summaries are now validated for provenance, budget evidence, and precision, and metadata debug JSON exposes SCC schedule, closure iteration/backdating stats, and demand query trace rows.**

## Performance

- **Completed:** 2026-05-22
- **Tasks:** 2/2
- **Files modified:** 6

## Accomplishments

- Added `SummaryProvenance::InterproceduralClosure` and tagged SCC closure outputs with it when summaries are produced by interprocedural propagation.
- Extended `validate_summaries` with SCC closure checks for missing budget-exceeded events, invalid Exact precision on interprocedural summaries, and orphan interprocedural summaries with no resolved outgoing call evidence.
- Extended metadata debug JSON with `scc_schedule` and `demand_queries` sections.
- Added SCC schedule fields for counts, sizes, processing order, closure iteration counts, total iterations, and backdated SCC count.
- Added demand query debug rows with query kind, precision tier, cache status, compute time, and result digest prefix.
- Carried `SccClosureResult` through `KernelRunReport` for crate-private test-facing debug output.

## Task Commits

1. **Task 1 and Task 2 core implementation:** `bd6b357` - `feat(33-06): add scc validation and debug traces`
2. **Task 2 observability completion:** `9d524c3` - `feat(33-06): surface closure execution stats in debug output`

## Files Modified

- `crates/polint/src/analysis/summaries/facts.rs` - Added `InterproceduralClosure` summary provenance.
- `crates/polint/src/analysis/summaries/closure.rs` - Tagged interprocedural closure summaries and derived equality for `SccClosureResult`.
- `crates/polint/src/analysis/summaries/validate.rs` - Added SCC closure validation and regression tests.
- `crates/polint/src/analysis_kernel/debug.rs` - Added SCC schedule/closure stats and demand query debug sections.
- `crates/polint/src/analysis_kernel/incremental/run_report.rs` - Exposed demand trace and SCC closure result to test-facing debug helpers.
- `crates/polint/src/analysis_kernel/mod.rs` - Passed closure result and demand trace into `KernelRunReport` and debug JSON helper.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Coverage] Added missing closure execution stats after initial debug implementation**
- **Found during:** Self-review before summary close-out
- **Issue:** Initial debug JSON exposed SCC schedule ordering but not closure iteration counts or backdating stats from `SccClosureResult`.
- **Fix:** Carried `SccClosureResult` through `KernelRunReport` and added `total_iterations`, `backdated_sccs`, and `iteration_counts` to the SCC debug section.
- **Verification:** `metadata_debug_json_with_scc_closure_result_contains_iteration_and_backdating_rows` passes.

**2. [Rule 1 - Coverage] Added precision tier to demand query debug rows**
- **Found during:** Self-review against D-13/objective text
- **Issue:** Initial demand query debug rows included kind, cache status, duration, and result digest prefix but omitted precision tier.
- **Fix:** Added `precision_tier` to `DemandQueryDebugEntry`.
- **Verification:** `metadata_debug_json_with_demand_trace_contains_query_rows` asserts the field.

**Total deviations:** 2 auto-fixed coverage gaps.
**Impact on plan:** Both fixes strengthen the planned observability surface without widening public API.

## Verification

- `cargo test --lib -p polint -- validate_summaries` - passed, 2 tests
- `cargo test --lib -p polint -- metadata_debug` - passed, 12 tests
- `cargo clippy -p polint -- -D warnings` - passed

## User Setup Required

None.

## Next Phase Readiness

- `33-07` can now add eval fixtures and no-leak proof using the new metadata debug sections.
- SCC closure validation now catches malformed interprocedural summaries before eval proof relies on them.

## Self-Check: PASSED

- `SummaryProvenance::InterproceduralClosure`: FOUND
- `validate_scc_closure_results`: FOUND
- `scc_schedule` debug section: FOUND
- `demand_queries` debug section: FOUND
- closure iteration/backdating debug fields: FOUND
- verification commands: PASSED

---
*Phase: 33-demand-queries-and-summary-scc-cache*
*Completed: 2026-05-22*

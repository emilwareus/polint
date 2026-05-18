---
phase: 23-input-snapshots-and-cache-key-vocabulary
plan: 04
subsystem: analysis-kernel-cache
tags: [rust, analysis-kernel, input-snapshot, cache-stats, provider-metadata]

# Dependency graph
requires:
  - phase: 20-private-analysis-kernel-facade
    provides: crate-private AnalysisKernel and provider manifests
  - phase: 21-provenance-precision-and-validation-metadata
    provides: fact metadata sidecar and native validation vocabulary
  - phase: 23-input-snapshots-and-cache-key-vocabulary
    provides: input snapshots from 23-02 and provider cache stats from 23-03
provides:
  - crate-private KernelRunReport on KernelOutput
  - provider output metadata rows for all six current provider manifests
  - deterministic provider output digests using manifest schema, language scope, cache policy, precision, outputs, and fact summaries
  - aggregate cache stats with real Go and TS/JS syntax-provider counters
affects: [phase-23, phase-24, incremental-cache, eval-fixtures, analysis-kernel]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - crate-private run reports attached to internal kernel output only
    - provider output digests built from direct manifest identity instead of synthetic weight tokens
    - zero-valued CacheStats for providers that do not access the current file-fact cache

key-files:
  created:
    - crates/polint/src/analysis_kernel/incremental/run_report.rs
  modified:
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/provider.rs
    - crates/polint/src/analysis_kernel/metadata.rs
    - crates/polint/src/analysis_kernel/incremental/mod.rs

key-decisions:
  - "Keep KernelRunReport and test helpers crate-private/cfg(test), with no SDK, runner, CLI, or public JSON exposure."
  - "Use provider manifest schema, language scope, cache policy, precision ceiling, and outputs as real provider-output digest inputs."
  - "Report real Go and TS/JS adapter cache counters, while source/module/symbol/metrics providers carry explicit zero CacheStats."

patterns-established:
  - "AnalysisKernel builds one provider output row per manifest id in manifest order."
  - "Provider output summaries are derived from deterministic fact metadata rows rather than raw source or machine-local paths."
  - "InputSnapshot, ProviderOutputMeta, CacheStats, and KernelRunReport remain internal test/eval observability artifacts."

requirements-completed: [SAE-FND-04]

# Metrics
duration: 12m
completed: 2026-05-18
---

# Phase 23 Plan 04: Kernel Run Report Summary

**Crate-private kernel run reports now carry deterministic input snapshots, provider output metadata, and aggregate cache stats.**

## Performance

- **Duration:** 12m
- **Started:** 2026-05-18T06:38:55Z
- **Completed:** 2026-05-18T06:50:43Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- Added `KernelRunReport` and provider output construction under `analysis_kernel::incremental`.
- Added provider manifest helpers for provider version, schema label, language scope label, and cache policy label.
- Attached `run_report` to `KernelOutput` with an `InputSnapshot`, six provider output rows, and aggregate cache stats.
- Switched kernel Go/TS syntax calls to the stats-returning adapter wrappers while preserving public check output.
- Removed the old synthetic provider-manifest metadata token path from `analysis_kernel/mod.rs`.

## Task Commits

Each task was committed atomically:

1. **Task 1 RED: Run report metadata tests** - `4413aea` (test)
2. **Task 1 GREEN: Run report metadata construction** - `960a921` (feat)
3. **Task 2 RED: Kernel run report tests** - `b56ca46` (test)
4. **Task 2 GREEN: Kernel run report attachment** - `a56f533` (feat)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/incremental/run_report.rs` - Defines `KernelRunReport`, provider output rows, deterministic output digests, and focused unit tests.
- `crates/polint/src/analysis_kernel/provider.rs` - Adds manifest identity helpers consumed by snapshot/report construction.
- `crates/polint/src/analysis_kernel/mod.rs` - Builds snapshots, provider output metadata, and aggregate stats during kernel execution.
- `crates/polint/src/analysis_kernel/incremental/mod.rs` - Registers/re-exports the run-report internals for crate-private kernel use.
- `crates/polint/src/analysis_kernel/metadata.rs` - Removes obsolete synthetic vocabulary-weight helpers after real manifest consumption replaced them.

## Decisions Made

- `KernelRunReport` is an internal observability artifact only; public `polint check`, SDK facts, runner behavior, ignores, and diagnostic rendering are unchanged.
- Provider output digests consume direct manifest identity, including language scope and cache policy, rather than a synthetic dropped token.
- Providers without cache access use explicit `CacheStats::default()` rows so the report does not claim unsupported hits, reuse, or quarantines.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Warning Hygiene] Removed obsolete metadata vocabulary weight helpers**
- **Found during:** Task 2 (Attach run report to AnalysisKernel::run)
- **Issue:** Removing synthetic manifest consumption left `metadata_vocabulary_weight` and related vocabulary arrays unused.
- **Fix:** Removed the obsolete helper/constants and added a scoped `dead_code` expectation for future validation statuses that are intentionally not all constructed yet.
- **Files modified:** `crates/polint/src/analysis_kernel/metadata.rs`
- **Verification:** `cargo test -p polint --lib analysis_kernel --locked`, `cargo test -p polint --test cli kernel_metadata_preserves_public_check_behavior --locked`
- **Committed in:** `a56f533`

---

**Total deviations:** 1 auto-fixed warning-hygiene issue.
**Impact on plan:** The cleanup was directly caused by replacing synthetic manifest consumption with real report construction; no public surface changed.

## Issues Encountered

- Task 1 and Task 2 RED runs failed as expected before the report helpers and `KernelOutput::run_report` field existed.
- The targeted CLI compatibility test still emits existing Phase 23 future-vocabulary dead-code warnings from incremental key types; the test passes and those warnings are unrelated to this plan.
- `.planning/config.json` was dirty before and after execution with orchestrator-owned auto-chain state. It was not staged or modified by this plan's commits.

## User Setup Required

None - no external service configuration required.

## Known Stubs

None.

## Threat Flags

None - this plan adds crate-private report metadata only and introduces no public endpoint, external file-access behavior, schema migration, or trust-boundary expansion beyond the planned internal run report.

## Verification

- `cargo test -p polint --lib run_report --locked`
- `cargo test -p polint --lib kernel_run_report --locked`
- `cargo test -p polint --lib analysis_kernel --locked`
- `cargo test -p polint --lib incremental --locked`
- `cargo test -p polint --test cli kernel_metadata_preserves_public_check_behavior --locked`
- `cargo fmt --all -- --check`
- Acceptance greps confirmed required report/helper symbols, stats-wrapper kernel calls, synthetic helper removal from `analysis_kernel/mod.rs`, and no public SDK/runner/CLI leakage.

## Next Phase Readiness

Phase 24 can consume a crate-private run report with deterministic snapshots, provider output metadata, and current Go/TS cache counters without changing current cache reuse behavior or public output.

## Self-Check: PASSED

- Created/modified files exist: run report module, kernel wiring, provider helpers, metadata cleanup, incremental re-export, and this summary.
- Commits exist: `4413aea`, `960a921`, `b56ca46`, and `a56f533`.
- Final verification passed: run report filter, analysis kernel filter, incremental filter, targeted CLI public-behavior test, public-surface leak grep, and `cargo fmt --all -- --check`.

---
*Phase: 23-input-snapshots-and-cache-key-vocabulary*
*Completed: 2026-05-18*

---
phase: 54-benchmark-promotion-gate-extension
plan: 01
subsystem: eval
tags: [rust, eval, metrics, markdown, promotion-gates]

requires:
  - phase: 51-adaptation-model-layer
    provides: F-score beta=0.5 promotion decision and delta-reporting expectations
  - phase: 52-refined-calls-rework-unknown-taxonomy-consolidation
    provides: per-language reporting expectations and refined-call benchmark context
provides:
  - F0.5 scanner metric computation
  - Defaulted per-language delta report rows
  - Markdown rendering for F0.5 and per-language deltas
affects: [phase-54-promotion-gates, eval-reports, benchmark-audit]

tech-stack:
  added: []
  patterns: [defaulted-internal-report-sections, deterministic-row-normalization]

key-files:
  created: []
  modified:
    - crates/polint/src/eval/metrics.rs
    - crates/polint/src/eval/report.rs
    - crates/polint/src/eval/markdown.rs

key-decisions:
  - "F0.5 lives in internal metric sections rather than widening the locked MetricSummary top-level shape."
  - "Per-language deltas are explicit rows keyed by language, suite, scoring mode, and precision tier."

patterns-established:
  - "New eval report fields use defaulted MetricSections entries for older JSON compatibility."
  - "Markdown renders empty optional sections deterministically instead of omitting them."

requirements-completed: [BENCH-01]

duration: 38min
completed: 2026-06-06
---

# Phase 54: Benchmark Promotion Gate Extension Summary

**F0.5 and per-language delta report foundations for v1.3 promotion gates**

## Performance

- **Duration:** 38 min
- **Started:** 2026-06-06T04:35:00Z
- **Completed:** 2026-06-06T05:13:00Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments

- Added `f0_5` computation through the existing internal `f_score` helper.
- Added defaulted `PerLanguageDeltaRow` report rows sorted by language, suite, scoring mode, and precision tier.
- Updated markdown reports to show F0.5 in comparison/scanner tables and a deterministic `Per-Language Deltas` section.

## Task Commits

1. **Task 1/2: F0.5 metrics and delta rows** - `d574db91`
2. **Task 3: Markdown report rendering** - `39ce95a8`

## Files Created/Modified

- `crates/polint/src/eval/metrics.rs` - Computes and tests F0.5.
- `crates/polint/src/eval/report.rs` - Adds defaulted scanner F0.5 and per-language delta rows.
- `crates/polint/src/eval/markdown.rs` - Renders F0.5 and per-language delta report sections.

## Decisions Made

- Kept `MetricSummary` top-level layout unchanged and put F0.5 under `MetricSections::scanner`, matching the existing schema-extension discipline.
- Stored deltas as rows rather than an aggregate map so later gates cannot average away weak language/suite results.

## Deviations from Plan

None - plan executed as written. Task 1 and Task 2 landed in one commit because `metrics.rs` and `report.rs` have a compile-time dependency through `ScannerMetricSection`.

## Issues Encountered

- Initial F0.5 test expectations used the wrong hand-computed fractions. The implementation used the existing `f_score(0.5, ...)` helper correctly; expectations were corrected and tests reran green.

## Verification

- `cargo test -p polint --lib eval::metrics` - passed, 19 tests.
- `cargo test -p polint --lib eval::report` - passed, 19 tests.
- `cargo test -p polint --lib eval::markdown` - passed, 4 tests.
- `git diff --check` - passed.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Promotion gates can now consume F0.5 and per-language delta rows in Plan 54-02.

## Self-Check: PASSED

---
*Phase: 54-benchmark-promotion-gate-extension*
*Completed: 2026-06-06*

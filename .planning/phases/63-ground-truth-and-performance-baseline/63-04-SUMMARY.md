---
phase: 63-ground-truth-and-performance-baseline
plan: 04
subsystem: testing
tags: [benchmark, regression-gate, peak-rss, cold-wall-clock, budget, store-disabled, fail-not-silent]

# Dependency graph
requires:
  - phase: 63-ground-truth-and-performance-baseline
    plan: 03
    provides: StoreDisabledBaseline type + committed store-disabled-check/review.json reference baselines
  - phase: 63-ground-truth-and-performance-baseline
    plan: 02
    provides: CurvePoint measurement contract (peak RSS, cold/warm wall-clock)
  - phase: 63-ground-truth-and-performance-baseline
    plan: 01
    provides: GateVerdict/GateCheck verdict vocabulary
provides:
  - "BaselineThresholds extended with locked max_peak_rss_ratio (1.20) and max_cold_wall_clock_ratio (1.25) budget fields, serde-defaulted for backward-compatible deserialization"
  - "evaluate_regression_budget: crate-private regression-budget gate comparing a measured CurvePoint vs the committed StoreDisabledBaseline on peak RSS + cold wall-clock, producing a Fail/Pass RegressionGateReport"
  - "is_blocking(report) -> bool exposing the fail-not-silent signal a Phase 64+ store phase converts to a non-zero exit at its boundary"
affects: [phase-64, regression-gates, store-milestone]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Locked regression budgets are pinned as named constants (DEFAULT_MAX_PEAK_RSS_RATIO=1.20, DEFAULT_MAX_COLD_WALL_CLOCK_RATIO=1.25) and asserted for exact equality in a default-value test, so a silent loosening fails CI (threat T-63-04-01)"
    - "The gate reuses the existing GateVerdict/GateCheck vocabulary and .max() aggregation so a regression verdict composes like every other gate; Fail dominates"
    - "Divide-by-zero guard: a zero baseline denominator is an explicit missing-baseline Fail rather than a panic or silent pass (threat T-63-04-02)"
    - "is_blocking is the fail-not-silent seam: it stays crate-internal (no CLI wired) so later store phases decide where to convert Fail into a non-zero exit"

key-files:
  created:
    - crates/polint/src/eval/bench/gate.rs
  modified:
    - crates/polint/src/eval/baseline.rs
    - crates/polint/src/eval/bench/mod.rs

key-decisions:
  - "max_peak_rss_ratio / max_cold_wall_clock_ratio are NEW fields distinct from the existing warn/fail_runtime_overhead_ratio: the eval-runner runtime overhead gates the accuracy runner's observed runtime, whereas cold wall-clock and whole-repo peak RSS are the store-phase scale/latency signals — keeping both avoids conflating the two budgets (per plan instruction)"
  - "Both new fields carry serde defaults (#[serde(default = ...)]) so thresholds documents serialized before this plan still deserialize under the struct's deny_unknown_fields"
  - "Exactly-at-budget (1.20x RSS, 1.25x cold) is a Pass: the check Fails only when ratio strictly exceeds the budget, matching the plan's '+20%/+25%' phrasing"
  - "RegressionGateReport is a plain crate-private struct (not serde) — it is an in-process gate result consumed by a later phase, not a committed artifact, so no schema/wire version is introduced"

patterns-established:
  - "Store-phase regression gates divide a measured CurvePoint against the committed StoreDisabledBaseline and Fail past a locked ratio; is_blocking is the boundary signal"

requirements-completed: [BENCH-03]

# Metrics
duration: 20min
completed: 2026-07-09
---

# Phase 63 Plan 04: Store-Phase Regression-Budget Gate Summary

**A crate-private regression-budget gate compares a measured whole-repo `CurvePoint` against the committed store-disabled baseline and returns a Fail verdict (with a blocking signal) when peak RSS exceeds +20% or cold wall-clock exceeds +25% — the enforceable mechanism that turns Phase 63's committed baseline into a fail-not-silent gate from Phase 64 onward.**

## Performance

- **Duration:** ~20 min
- **Started:** 2026-07-09
- **Completed:** 2026-07-09
- **Tasks:** 2
- **Files modified:** 3 (1 created, 2 modified)

## Accomplishments

- Extended `BaselineThresholds` with two new locked-budget fields: `max_peak_rss_ratio` (default 1.20) and `max_cold_wall_clock_ratio` (default 1.25), pinned as named constants (`DEFAULT_MAX_PEAK_RSS_RATIO`, `DEFAULT_MAX_COLD_WALL_CLOCK_RATIO`) and serde-defaulted so pre-existing serialized thresholds still deserialize. The existing `warn/fail_runtime_overhead_ratio` fields are untouched — cold wall-clock is a distinct signal from the eval-runner runtime.
- Created `crates/polint/src/eval/bench/gate.rs` with `evaluate_regression_budget(baseline: &StoreDisabledBaseline, measured: &CurvePoint, thresholds: &BaselineThresholds) -> RegressionGateReport`: it produces a `peak_rss_ratio` check (measured/baseline peak RSS vs `max_peak_rss_ratio`) and a `cold_wall_clock_ratio` check (measured/baseline cold ms vs `max_cold_wall_clock_ratio`), each Fail when the ratio strictly exceeds its budget, aggregating the verdict via `.max()` (Fail dominates).
- Added `is_blocking(report) -> bool` returning true on Fail — the fail-not-silent seam a Phase 64+ store phase converts to a non-zero exit at its boundary (BENCH-03). It stays crate-internal, not wired into a public CLI.
- Guarded the divide-by-zero case (threat T-63-04-02): a zero baseline denominator is an explicit "missing baseline" Fail rather than a panic or silent pass.
- Registered `pub(crate) mod gate;` under `eval::bench`.

## Task Commits

1. **Task 1: Locked peak-RSS + cold-wall-clock budget ratios on BaselineThresholds** - `94c1e7be` (feat)
2. **Task 2: Regression-budget gate with fail-not-silent blocking signal** - `81bb382b` (feat)

## Files Created/Modified

- `crates/polint/src/eval/bench/gate.rs` - `evaluate_regression_budget` + `RegressionGateReport` + `is_blocking` + `ratio_budget_check` divide-by-zero guard + 5 tests (created)
- `crates/polint/src/eval/baseline.rs` - `BaselineThresholds` extended with `max_peak_rss_ratio`/`max_cold_wall_clock_ratio` + `DEFAULT_*` constants + serde-default fns + Default impl + 2 tests (locked defaults, backward-compatible deserialization) (modified)
- `crates/polint/src/eval/bench/mod.rs` - registered the `gate` submodule (modified)

## Decisions Made

- **Distinct budget fields, not a reuse of runtime overhead:** `max_peak_rss_ratio` / `max_cold_wall_clock_ratio` are new fields. The eval-runner `warn/fail_runtime_overhead_ratio` gates the accuracy runner's observed runtime; the store-phase scale/latency signals are whole-repo peak RSS and cold wall-clock. Conflating them would hide one regression behind the other, so both pairs coexist (per plan instruction).
- **Locked defaults pinned + tested for exact equality:** the +20% / +25% budgets are the locked Milestone Decisions (REQUIREMENTS.md, D). They are pinned to named constants and a test asserts `default().max_peak_rss_ratio == 1.20` and `== 1.25`; a silent loosening fails that test (threat T-63-04-01, tampering).
- **Exactly-at-budget passes:** a check Fails only when the ratio strictly exceeds the budget, matching the "+20% / +25%" phrasing; a `ratio_exactly_at_budget_passes` test locks this boundary.
- **RegressionGateReport is in-process, not an artifact:** it carries no serde/schema version because it is a gate result a later phase consumes in-process, not a committed reference file.

## Deviations from Plan

None - plan executed exactly as written. The gate consumes the exact `StoreDisabledBaseline` shape and `CurvePoint` fields shipped by Plans 03/02, reuses the `GateVerdict`/`GateCheck` vocabulary from `eval::gates`, and stays `pub(crate)` under `eval::bench`. The plan explicitly required a divide-by-zero guard and it is implemented + tested; that is a plan requirement, not a deviation.

## Threat Flags

None — no new security surface. The gate is a crate-private (`pub(crate)`) pure function over already-committed reference data and an in-process measured point; no new network, auth, file, or schema surface is introduced. The plan's threat register is fully covered: budget defaults pinned + exact-equality tested (T-63-04-01); zero-denominator guarded and tested (T-63-04-02); `is_blocking` true on Fail with an over-budget-run-is-blocking test (T-63-04-03).

## Known Stubs

None. `is_blocking` is intentionally not wired into a CLI exit path in this plan — the plan scopes that to the Phase 64+ store phases that invoke the gate at their boundary. This is a deliberate crate-internal seam, not an unfinished stub.

## Verification

- `cargo test -p polint --lib eval::baseline --locked` — 9 passed (includes the 2 new threshold tests).
- `cargo test -p polint --lib eval::bench::gate --locked` — 5 passed.
- `cargo build -p polint --locked` — clean.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` — clean (pre-commit hook, both commits).
- `cargo test -p polint --test public_surface_leak --locked` — 5 passed (the gate stays `pub(crate)` under `eval::bench`).

## Next Phase Readiness

- The regression gate is ready for Phase 64: a store phase measures a `CurvePoint`, loads the committed `StoreDisabledBaseline`, calls `evaluate_regression_budget`, and converts `is_blocking` into a non-zero exit at its phase boundary. The locked +20% peak-RSS / +25% cold-wall-clock budgets are now enforceable. No blockers.

---
*Phase: 63-ground-truth-and-performance-baseline*
*Completed: 2026-07-09*

## Self-Check: PASSED

`crates/polint/src/eval/bench/gate.rs` and the SUMMARY exist on disk, and both task commits (94c1e7be, 81bb382b) are in git history.

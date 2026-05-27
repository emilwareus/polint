---
phase: 40-external-benchmark-adapters-and-promotion-gates
plan: 04
subsystem: eval
tags: [rust, evaluation-harness, promotion-gates, partial-truth, fixtures]
requires:
  - phase: 40-03
    provides: grouped metrics and provider/cache report evidence
provides:
  - promotion gate verdict model
  - native promotion fixture for CFG, refined calls, data-flow, evidence, unknowns, budgets, and cache determinism
  - partial-truth path matching over ordered evidence nodes
affects: [phase-40, eval, matcher, native-fixtures]
tech-stack:
  added: []
  patterns: [threshold-based gate reports, synthetic native promotion fixtures, partial-truth unconfirmed extras]
key-files:
  created:
    - crates/polint/src/eval/gates.rs
    - tests/eval-fixtures/promotion/cfg-call-flow-evidence/expected.polint-eval.toml
    - tests/eval-fixtures/promotion/cfg-call-flow-evidence/repo/.polint.toml
    - tests/eval-fixtures/promotion/cfg-call-flow-evidence/repo/service.ts
  modified:
    - crates/polint/src/eval/mod.rs
    - crates/polint/src/eval/model.rs
    - crates/polint/src/eval/fixtures.rs
    - crates/polint/src/eval/matcher.rs
    - crates/polint/src/eval/observed.rs
key-decisions:
  - "Native promotion fixtures can use synthetic observed rows under a dedicated promotion fixture area."
  - "Partial-truth graph/path extras classify as unconfirmed unless a forbidden expected row proves they are false."
  - "Gate reports name each metric, observed value, and threshold."
patterns-established:
  - "Promotion gates evaluate normalized EvaluationRun data, not strings."
  - "Partial paths match ordered evidence nodes so endpoint/evidence correctness can tolerate extra explanation detail."
requirements-completed: [SAE-PROM-01]
duration: 7 min
completed: 2026-05-26
---

# Phase 40 Plan 04: Native Graph Fact Path Promotion Gates Summary

**Native promotion gate verdicts and fixture coverage for graph, fact, path, unknown, budget, and cache evidence**

## Performance

- **Duration:** 7 min
- **Started:** 2026-05-26T07:32:39Z
- **Completed:** 2026-05-26T07:39:57Z
- **Tasks:** 3
- **Files modified:** 9

## Accomplishments

- Added `PromotionGateThresholds`, `SuiteGateConfig`, `GateCheck`, and `PromotionGateReport` over normalized `EvaluationRun` data.
- Added pass/fail/warn gate checks for pass rate, graph misses, path misses, unknown budgets, rejected facts, runtime budget failures, cache quarantines, and deterministic output hash.
- Added a synthetic native promotion fixture covering CFG, refined calls, data-flow, evidence, partial graph/path truth, budget unknowns, runtime budget, and cache determinism invariant rows.
- Updated matcher path logic so partial truth can match ordered evidence nodes rather than requiring exact internal path equality.

## Task Commits

1. **Tasks 1-3: Gate model, promotion fixture, and partial-truth matching** - `d451417` (`feat(40-04)`)

**Plan metadata:** this summary commit.

## Files Created/Modified

- `crates/polint/src/eval/gates.rs` - Promotion gate threshold config and verdict evaluation.
- `crates/polint/src/eval/matcher.rs` - Partial path matching and expanded graph/path tests.
- `crates/polint/src/eval/model.rs` and `crates/polint/src/eval/fixtures.rs` - Added `promotion` fixture area and synthetic observed allowance.
- `crates/polint/src/eval/observed.rs` - Added eval-observed test proving the promotion fixture runs through the native fixture runner.
- `tests/eval-fixtures/promotion/cfg-call-flow-evidence/*` - Native fixture manifest and tiny owned TS fixture repo.

## Decisions Made

- Added a dedicated `FixtureArea::Promotion` rather than overloading extension fixtures for promotion-gate coverage.
- Kept the fixture source tiny and repo-owned; no external benchmark content was committed.
- Treated a budget-exceeded data-flow fact as an unknown-class metric input while still making it explicit in the fixture.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

One gate test helper initially shadowed its own function name. The helper was renamed and the checks were rerun.

## Verification

- `cargo fmt --all --check` - passed
- `cargo test -p polint --lib eval::gates --locked` - passed, 3 tests
- `cargo test -p polint --lib eval::matcher --locked` - passed, 11 tests
- `cargo test -p polint --lib eval_observed --locked` - passed, 13 tests

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for Plan 40-05. The tier runner can now require native promotion gates before supported-language external smoke suites are used for claims.

## Self-Check: PASSED

All plan tasks and acceptance criteria were implemented and verified. Native promotion fixture coverage is present before external benchmark execution.

---
*Phase: 40-external-benchmark-adapters-and-promotion-gates*
*Completed: 2026-05-26*

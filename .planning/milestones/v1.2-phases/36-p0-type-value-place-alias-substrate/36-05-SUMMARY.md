---
phase: 36-p0-type-value-place-alias-substrate
plan: 05
subsystem: analysis
tags: [rust, points-to, aliases, solver, type-value-alias]
requires:
  - phase: 36-p0-type-value-place-alias-substrate
    provides: Go and TS/JS type, value, allocation, and access-path facts
provides:
  - Bounded deterministic points-to constraint generation and solving
  - Internal alias provider stack with all SAE-PREC-01 alias statuses
  - Provider output wiring for points-to constraints, points-to sets, and alias answers
affects: [phase-36, phase-37, phase-38, call-graph, data-flow]
tech-stack:
  added: []
  patterns: [bounded optional solver, evidence-backed alias answers, provider-stack query]
key-files:
  created:
    - crates/polint/src/analysis/points_to/constraints.rs
    - crates/polint/src/analysis/points_to/solver.rs
    - crates/polint/src/analysis/aliases/query.rs
    - crates/polint/src/analysis/aliases/provider_stack.rs
  modified:
    - crates/polint/src/analysis/points_to/mod.rs
    - crates/polint/src/analysis/aliases/mod.rs
    - crates/polint/src/analysis/types/provider.rs
key-decisions:
  - "Points-to remains bounded and optional inside the private provider; budget exhaustion emits BudgetExceeded/Unknown facts."
  - "Alias answers are query/provider-stack results, not a primary alias graph."
patterns-established:
  - "Constraint generation is separated from solving so future providers can add constraints without replacing solver semantics."
  - "Alias classification prefers exact identity, then partial projection overlap, then budget/points-to evidence."
requirements-completed: [SAE-PREC-01]
duration: 7 min
completed: 2026-05-24
---

# Phase 36 Plan 05: Bounded Points-To Constraints And Alias Query Service Summary

**Bounded points-to constraints and evidence-backed alias answers stored by the private Phase 36 provider**

## Performance

- **Duration:** 7 min
- **Started:** 2026-05-24T13:50:54Z
- **Completed:** 2026-05-24T13:57:36Z
- **Tasks:** 3
- **Files modified:** 7

## Accomplishments

- Added deterministic points-to constraint generation and a bounded inclusion-style solver with budget-exceeded status rows.
- Added an internal alias query/provider stack that returns `NoAlias`, `MayAlias`, `MustAlias`, `PartialAlias`, and `Unknown` with evidence.
- Wired constraints, solved points-to sets, and alias answers into the private type/value/alias provider output.

## Task Commits

1. **Task 1: Build deterministic points-to constraints and solver core** - `5b0e304` (feat)
2. **Task 2: Implement alias provider stack** - `59633e3` (feat)
3. **Task 3: Wire constraints and alias answers into Phase 36 provider output** - `f1d67c1` (feat)

**Plan metadata:** pending in docs commit.

## Files Created/Modified

- `crates/polint/src/analysis/points_to/constraints.rs` - Constraint generation from values, allocations, and access paths.
- `crates/polint/src/analysis/points_to/solver.rs` - Bounded inclusion-style solver and result conversion.
- `crates/polint/src/analysis/aliases/query.rs` - Alias query classifier over access paths and points-to sets.
- `crates/polint/src/analysis/aliases/provider_stack.rs` - Deterministic provider-stack answer generation.
- `crates/polint/src/analysis/types/provider.rs` - Provider wiring and TS fixture regression.

## Decisions Made

- Solver budgets default to conservative internal constants and produce `BudgetExceeded` rather than silently truncating exact-looking output.
- Alias `MustAlias` is only returned for same stable operand or singleton-equal object evidence.
- Common-base/different-projection access paths return `PartialAlias` instead of being flattened to exact field-insensitive identity.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- A test-only unused import warning was caught by the focused test runs and removed before final verification.

## Verification

- `cargo fmt --all --check` - passed.
- `cargo test -p polint --lib analysis::points_to --locked` - passed.
- `cargo test -p polint --lib analysis::aliases --locked` - passed.
- `cargo test -p polint --lib analysis::types::provider --locked` - passed.
- `cargo check -p polint --locked` - passed.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for `36-06`: extension-provided type/value/alias precision can now merge into concrete provider output and quarantine paths.

---
*Phase: 36-p0-type-value-place-alias-substrate*
*Completed: 2026-05-24*

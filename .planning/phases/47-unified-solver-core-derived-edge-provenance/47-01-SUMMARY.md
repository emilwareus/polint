---
phase: 47-unified-solver-core-derived-edge-provenance
plan: 01
subsystem: analysis
tags: [rust, solver, points-to, worklist, budget, determinism, pub-crate]

# Dependency graph
requires:
  - phase: 44-semantic-graph-skeleton-constraint-vocabulary
    provides: "ConstraintKind unified constraint vocabulary + naming-collision-guard pattern + reserved provider slot"
  - phase: 45-46
    provides: "points_to sub-domain (PointsToBudget/PointsToBudgetStatus, solve_points_to VecDeque fixpoint) folded in by composition"
provides:
  - "Private analysis::solver module (pub(crate)) registered between slicing and stable_key"
  - "Unified SolverBudget struct (max_steps + bounded outer-iteration cap + per-sub-domain channel) and BudgetStatus closed enum"
  - "SolverPolicy trait with one real impl (PointsToPolicy, points-to folded by composition) + two honest Go/TS stubs"
  - "SolverEngine: deterministic VecDeque worklist + budget enforcement + monotonic u64 step counter driving policies to a single fixpoint per run"
affects: [47-02 (provenance on derived edges + explain), 47-03 (provider/cache-key/determinism gate wiring), 48 (Go RTA driver), 49 (TS tokens driver), 52 (GRAPH-05 refined_calls rework)]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Generalize-and-fold: lift the proven points_to worklist/budget into a unified core; fold points-to in by composition (no rewrite)"
    - "Budget projection: SolverBudget::points_to_budget() maps onto existing PointsToBudget without editing its Default"
    - "Honest stubs: reserved Go/TS policies derive nothing, naming the reserving phase (mirrors ConstraintKind::ModelEdge)"

key-files:
  created:
    - crates/polint/src/analysis/solver/mod.rs
    - crates/polint/src/analysis/solver/budget.rs
    - crates/polint/src/analysis/solver/policy.rs
    - crates/polint/src/analysis/solver/engine.rs
  modified:
    - crates/polint/src/analysis/mod.rs

key-decisions:
  - "Folded points-to by composition (PointsToPolicy invokes solve_points_to in place) rather than relocating the fixpoint engine — points-to fixtures stay byte-identical (D-03)."
  - "Wrapped (not aliased) the points-to budget: SolverBudget carries a PointsToSubBudget channel and projects onto PointsToBudget via points_to_budget() (D-05)."
  - "Modeled the bounded outer-iteration cap (D-11) as max_outer_iterations on SolverBudget, enforced in the engine's worklist drain."
  - "Kept the engine worklist as a policy-index VecDeque drain; the points-to policy's inner fixpoint remains the proven points_to/solver.rs worklist (composition)."

patterns-established:
  - "Pattern 1: Unified budget generalizes a sub-domain budget via an explicit projection fn, never by editing the sub-domain Default."
  - "Pattern 2: Reserved-but-stubbed SolverPolicy impls produce honest emptiness and name their reserving phase in the doc comment."
  - "Pattern 3: The engine surfaces budget exhaustion as BudgetStatus::BudgetExceeded (worst-case-wins across policies), never a silent drop."

requirements-completed: [GRAPH-03]

# Metrics
duration: 9min
completed: 2026-06-02
---

# Phase 47 Plan 01: Unified Solver Core Summary

**Private `analysis::solver` core with a deterministic `VecDeque` worklist engine, a unified `SolverBudget`/`BudgetStatus` generalizing the points-to budget by projection, and a `SolverPolicy` scaffold whose one real impl folds v1.2's `points_to::solver` fixpoint in by composition (byte-identical) alongside two honest Go/TS stubs.**

## Performance

- **Duration:** ~9 min
- **Started:** 2026-06-02T06:10:53Z
- **Completed:** 2026-06-02T06:19:51Z
- **Tasks:** 3
- **Files modified:** 5 (4 created, 1 modified)

## Accomplishments
- Registered a new `pub(crate) mod solver;` carrying the D-04 naming-collision guard (unified core vs. points-to sub-domain) and the D-11 dependency contract (closed input set / single fixpoint per run / bounded outer iterations).
- Added `SolverBudget` (cross-domain `max_steps` + `max_outer_iterations` + a `PointsToSubBudget` channel) and the `BudgetStatus` closed enum, with locked pinned-declaration-order and exhaustive variant-count tests; the points-to budget defaults (10_000 / 64 / 512) stay unchanged via an explicit projection.
- Built `SolverEngine`: a deterministic policy-index `VecDeque` worklist driving registered `SolverPolicy` impls to a single fixpoint per run, with a monotonic `u64` step counter and honest budget-exhaustion latching.
- Folded the points-to fixpoint in by composition (`PointsToPolicy` invokes `solve_points_to` in place) and proved equivalence: points-to-via-engine equals `solve_points_to` over the same constraints; points-to snapshot/determinism fixtures are byte-identical; the public-surface-leak gate stays green with `ALLOWED_PRELUDE` untouched.

## Task Commits

Each task was committed atomically:

1. **Task 1: Create analysis::solver module + naming-collision/dependency-contract guard** - `79601a0e` (feat)
2. **Task 2: Unified SolverBudget struct + BudgetStatus closed enum (D-05, D-06)** - `2377514c` (feat)
3. **Task 3: SolverPolicy scaffold + points-to fold + unified worklist engine (D-02, D-03, D-07)** - `ef2f5b1b` (feat)

_Tasks 2 and 3 are TDD tasks; their behavior tests and implementation landed in a single atomic commit each (tests + code co-located in the new files)._

## Files Created/Modified
- `crates/polint/src/analysis/solver/mod.rs` - Module root with the D-04 naming-collision guard and D-11 dependency-contract doc; declares `budget`/`policy`/`engine`.
- `crates/polint/src/analysis/solver/budget.rs` - `SolverBudget` + `PointsToSubBudget` + `BudgetStatus` closed enum; `points_to_budget()` projection; locked byte-stability tests.
- `crates/polint/src/analysis/solver/policy.rs` - `SolverPolicy` trait, `PolicyOutcome`, real `PointsToPolicy` (composition fold), honest `GoRtaPolicy`/`TsTokensPolicy` stubs.
- `crates/polint/src/analysis/solver/engine.rs` - `SolverEngine` deterministic `VecDeque` worklist, `SolverRunResult`/`PolicyRunRecord`, budget enforcement + monotonic step counter; equivalence/determinism/exhaustion/stub tests.
- `crates/polint/src/analysis/mod.rs` - Registered `pub(crate) mod solver;` between `slicing` and `stable_key`.

## Decisions Made
- **Composition over relocation (D-03):** the points-to fixpoint engine stays in `points_to/solver.rs`; `PointsToPolicy::solve` calls `solve_points_to` in place. This keeps points-to fixtures byte-identical, which the equivalence test enforces.
- **Wrap, not alias (D-05):** `SolverBudget` carries a `PointsToSubBudget` channel and exposes `points_to_budget()` to project onto the existing `PointsToBudget`. `PointsToBudget::default` is never edited.
- **Bounded outer-iteration cap (D-11):** modeled as `SolverBudget::max_outer_iterations` and enforced as a step ceiling in the engine's worklist drain; exceeding it latches `BudgetStatus::BudgetExceeded` rather than looping unbounded.
- **Worst-case budget projection:** the engine reports `BudgetExceeded` if any driven policy exceeds its budget; `NotRun` when no policy is registered; otherwise `WithinBudget`.

## Deviations from Plan

None - plan executed exactly as written. (The only adjustment was a `cargo fmt` reflow of a derive line and removal of one unused test import, both flagged by the pre-commit `make lint` hook; neither changed behavior.)

## Issues Encountered
- The pre-commit `make lint` hook initially rejected Task 2 over a `rustfmt` derive-line reflow; resolved by running `cargo fmt --all` and re-running `make lint` before committing.
- An unused `SolverPolicy` import surfaced in `engine.rs` tests (the engine drives policies without naming the trait directly); removed before the Task 3 commit. `cargo clippy -p polint --all-targets` is clean.

## Known Stubs
- `solver::policy::GoRtaPolicy` — honest emptiness, reserved for **Phase 48 (GO-05)**. Intentional per D-07; documented in the policy doc comment.
- `solver::policy::TsTokensPolicy` — honest emptiness, reserved for **Phase 49 (JS-04)**. Intentional per D-07; documented in the policy doc comment.

These are intentional reserved stubs (the D-07 contract for Phase 47), not goal-blocking gaps: GRAPH-03 for Phase 47 ships only the `SolverPolicy` scaffolding plus the one real points-to impl.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- The unified core (worklist + budget + policy scaffold + step counter) is in place for **Plan 02** to attach `DerivedEdgeProvenance` (the engine already maintains the monotonic `u64` solver-step the provenance needs) and the deletion property test.
- The provider/cache-key/determinism-gate wiring and the cycle-detection fixture remain for **Plan 03**; no provider was registered in this plan (intentional), so the ~7 provider-order snapshot sites are untouched here.
- Determinism discipline (BTree-ordered accumulation, single fixpoint, byte-stable enums) is established and matches the points-to template; ready for the Phase 43 determinism gate to auto-enroll once the provider lands in Plan 03.

## Self-Check: PASSED

- Files: all four created files (`solver/mod.rs`, `solver/budget.rs`, `solver/policy.rs`, `solver/engine.rs`) and the modified `analysis/mod.rs` exist on disk.
- Commits: `79601a0e`, `2377514c`, `ef2f5b1b` present in `git log`.
- Tests: `solver::` (20), `analysis::points_to` (10, locked determinism + budget tests green), `public_surface_leak` (5, `ALLOWED_PRELUDE` untouched) all pass; `cargo build` and `cargo clippy -p polint --all-targets` clean.

---
*Phase: 47-unified-solver-core-derived-edge-provenance*
*Completed: 2026-06-02*

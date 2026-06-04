---
phase: 50-js-ts-object-property-prototype-this-model-driver
plan: 02
subsystem: analysis
tags: [rust, solver, typescript, object-model, config, cache]

requires:
  - phase: 50-js-ts-object-property-prototype-this-model-driver
    plan: 01
    provides: Private TS object-model rows and semantic-graph lowering
  - phase: 49-js-ts-function-token-propagation-driver
    provides: TS token solver budget/config/cache patterns
provides:
  - Disabled-by-default JS/TS object-model solver flag
  - Distinct object-model sub-budget with positive-only config overlay
  - Solver parameter/output digest participation for object-model flag and caps
  - Provider registration seam for the future TS object-model policy
affects: [phase-50, solver, config, cache, ts-object-model]

tech-stack:
  added: []
  patterns:
    - crate-private solver sub-budget structs
    - positive-only `.polint.toml` cap overlay
    - locked provider parameter digest part lists

key-files:
  created:
    - tests/eval-fixtures/ts-object-model/object-literal/repo/.polint.toml
  modified:
    - crates/polint/src/analysis/solver/budget.rs
    - crates/polint/src/config/mod.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis/solver/cache_key.rs
    - crates/polint/src/analysis/solver/provider.rs
    - crates/polint/src/eval/ts_tokens.rs

key-decisions:
  - "Kept the object model disabled by default through `SolverBudget::object_model_enabled`."
  - "Added object caps as a distinct `JsObjectModelSubBudget` instead of overloading TS token caps."
  - "Used explicit `[solver.js]` object-prefixed cap names with positive-only overlay and `0` fallback."
  - "Appended object-model algorithm/flag/cap parts after existing JS token digest fields to preserve existing part order."

patterns-established:
  - "`[solver.js] object_model = true` is the opt-in control for future object-model policy registration."
  - "Object-model caps participate in both solver provider parameter and output digests."
  - "The provider has a narrow registration seam for `TsObjectModelPolicy`, but Plan 50-02 emits no placeholder object edges."

requirements-completed: [JS-05]

duration: 12 min
completed: 2026-06-04
---

# Phase 50 Plan 02: Object-Model Solver Controls Summary

**Disabled-by-default object-model control plane with config, budgets, digests, and provider gate**

## Performance

- **Duration:** 12 min
- **Started:** 2026-06-04T06:25:27Z
- **Completed:** 2026-06-04T06:37:28Z
- **Tasks:** 4
- **Files modified:** 7

## Accomplishments

- Added `JsObjectModelSubBudget` with positive finite defaults for objects per place, property buckets, tokens per property, computed buckets, prototype depth, receiver candidates, and object worklist steps.
- Added `SolverBudget.object_model_enabled`, disabled by default, plus config mapping from `[solver.js] object_model` and object-prefixed cap fields.
- Threaded object-model flag/caps through production kernel budget construction and TS-token eval fixture helpers.
- Added `ts_object_model_fixpoint_v1` plus object flag/cap parts to solver parameter and output digests.
- Added a provider-side registration seam that only activates when the object-model flag is enabled; before Plan 50-03 it emits no object-derived edges.
- Added the first `tests/eval-fixtures/ts-object-model/object-literal/repo/.polint.toml` config example with `[solver.js] object_model = true`.

## Task Commits

1. **Plan 50-02 implementation:** `c000483a` (`feat(50-02): add object-model solver controls`)

## Verification

- `cargo test -p polint analysis::solver::budget` - passed, 9 tests.
- `cargo test -p polint config::tests::solver` - passed, 13 tests.
- `cargo test -p polint analysis_kernel::tests` - passed, 24 tests.
- `cargo test -p polint analysis::solver::cache_key` - passed, 4 tests.
- `cargo test -p polint analysis::solver::provider` - passed, 12 tests.
- `cargo test -p polint eval::ts_tokens` - passed, 4 tests.
- `cargo fmt --all -- --check` - passed.
- `git diff --check` - passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` - passed.

## Decisions Made

- Object-model config lives under `[solver.js]` using explicit names such as `max_object_properties_per_object` and `max_object_receiver_candidates_per_callsite`.
- `object_model = false` is implicit for absent config; `object_model = true` enables only the provider registration seam until the real policy lands in Plan 50-03.
- Object-model budget changes invalidate solver output even before the policy emits edges, preventing stale cache reuse when Plan 50-03 starts consuming the controls.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 4 - Task Coupling] Committed provider digest and gate changes together**
- **Found during:** Task 3 and Task 4
- **Issue:** Both output-digest participation and the object-model registration gate live in `crates/polint/src/analysis/solver/provider.rs`, and splitting them into separate commits would create an artificial intermediate state.
- **Fix:** Landed the plan as one cohesive implementation commit while keeping tests mapped to each acceptance area.
- **Files modified:** `crates/polint/src/analysis/solver/provider.rs`
- **Verification:** `cargo test -p polint analysis::solver::provider`
- **Committed in:** `c000483a`

---

**Total deviations:** 1 auto-fixed (1 task-coupling)
**Impact on plan:** Scope stayed within private solver/config/cache control-plane work. No public SDK, runner, README, or public CLI surface was added.

## Issues Encountered

- The plan's combined Cargo test filters are not valid Cargo syntax because Cargo accepts one test-name filter per invocation. I ran the equivalent focused filters separately and recorded the passing results.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 50-03 can add the real `analysis::solver::ts_object_model` policy behind the existing `object_model_enabled` gate and consume the distinct object caps without changing the public CLI or SDK surface.

---
*Phase: 50-js-ts-object-property-prototype-this-model-driver*
*Completed: 2026-06-04*

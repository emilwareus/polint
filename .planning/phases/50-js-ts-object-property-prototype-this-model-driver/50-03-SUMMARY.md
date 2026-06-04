---
phase: 50-js-ts-object-property-prototype-this-model-driver
plan: 03
subsystem: analysis
tags: [rust, solver, typescript, object-model, property-flow]

requires:
  - phase: 50-js-ts-object-property-prototype-this-model-driver
    plan: 01
    provides: TS object-model facts lowered to semantic graph constraints
  - phase: 50-js-ts-object-property-prototype-this-model-driver
    plan: 02
    provides: Object-model flag, sub-budget, and solver digest participation
provides:
  - Private `analysis::solver::ts_object_model` input, fixpoint, and dispatch modules
  - `TsObjectModelPolicy` registered behind `SolverBudget.object_model_enabled`
  - Exact/computed property bucket propagation with object budget enforcement
  - Property-backed conservative `DerivedEdgeFact` call edges
affects: [phase-50, solver, ts-object-model, derived-edges]

tech-stack:
  added: []
  patterns:
    - closed solver input snapshots
    - deterministic BTree property buckets
    - conservative derived-edge provenance

key-files:
  created:
    - crates/polint/src/analysis/solver/ts_object_model/mod.rs
    - crates/polint/src/analysis/solver/ts_object_model/inputs.rs
    - crates/polint/src/analysis/solver/ts_object_model/fixpoint.rs
    - crates/polint/src/analysis/solver/ts_object_model/dispatch.rs
  modified:
    - crates/polint/src/analysis/solver/mod.rs
    - crates/polint/src/analysis/solver/policy.rs
    - crates/polint/src/analysis/solver/provider.rs

key-decisions:
  - "Built the object driver as a private `SolverPolicy` over a closed `TsObjectModelInputs` snapshot."
  - "Used exact field labels directly and kept computed/unknown buckets separate from exact keys."
  - "Reused Phase 49 token seeds to place callable function tokens into property buckets without deriving from property names alone."
  - "Kept prototype/class/receiver lookup out of this plan; Plan 50-04 owns those semantics."

patterns-established:
  - "Object-model inputs normalize allocation, write, read, handoff, token, and node-kind rows deterministically."
  - "Property buckets carry function/object tokens plus stable contributing evidence for allocation, write, read, callsite, and token-flow facts."
  - "Object-model budget exhaustion latches run-level `BudgetStatus::BudgetExceeded` without emitting fake targets after a cap."

requirements-completed: [JS-05]

duration: 63 min
completed: 2026-06-04
---

# Phase 50 Plan 03: TS Object-Model Solver Policy Summary

**Private property-bucket object-model policy for property-backed JS/TS call edges**

## Performance

- **Duration:** 63 min
- **Started:** 2026-06-04T06:37:28Z
- **Completed:** 2026-06-04T07:40:25Z
- **Tasks:** 4
- **Files modified:** 7

## Accomplishments

- Added `TsObjectModelInputs::from_db` to build a closed snapshot from private object rows, semantic graph constraints, TS callsite identities, direct-binding handoffs, and Phase 49 token seeds.
- Implemented a deterministic property-bucket fixpoint for exact and computed/unknown property labels.
- Enforced object-model caps for objects per place, properties per object, tokens per property, computed buckets per object, receiver candidates per callsite, and object worklist steps.
- Added property-backed dispatch that emits conservative `DerivedEdgeFact` call edges only from callable tokens stored in property buckets.
- Registered `TsObjectModelPolicy` behind `SolverBudget.object_model_enabled` while preserving default-disabled behavior.

## Task Commits

1. **Plan 50-03 implementation:** `b62c7703` (`feat(50-03): add TS object-model solver policy`)

## Verification

- `cargo test -p polint analysis::solver::ts_object_model` - passed, 14 tests.
- `cargo test -p polint analysis::solver::policy` - passed, 6 tests.
- `cargo test -p polint analysis::solver::engine` - passed, 17 tests.
- `cargo test -p polint analysis::solver::provider` - passed, 13 tests.
- `cargo test -p polint analysis::solver::validate` - passed, 13 tests.
- `cargo test -p polint eval::ts_tokens` - passed, 4 tests.
- `cargo test -p polint eval::go_rta` - passed, 8 tests.
- `cargo fmt --all -- --check` - passed.
- `git diff --check` - passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` - passed.

## Decisions Made

- The object model derives call edges from property bucket contents, never from property names alone.
- Computed/unknown buckets are separate labels and do not read every exact key by default.
- Function tokens can enter property buckets directly from function-node writes or through Phase 49 token propagation when the write source is a place.
- Provider-level enabled runs register `ts_object_model`; runs without object facts still emit the same edge set as default-disabled runs while carrying distinct digests from Plan 50-02.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Enforced `max_objects_per_place` in the fixpoint**
- **Found during:** Final clippy/acceptance review
- **Issue:** The initial property-bucket implementation used the object cap in config/digests but did not enforce the per-place object ceiling.
- **Fix:** Added allocation-place tracking and a focused `max_objects_per_place` budget test.
- **Files modified:** `crates/polint/src/analysis/solver/ts_object_model/{inputs.rs,fixpoint.rs}`, `crates/polint/src/analysis/solver/policy.rs`
- **Verification:** `cargo test -p polint analysis::solver::ts_object_model`
- **Committed in:** `b62c7703`

---

**Total deviations:** 1 auto-fixed (1 missing critical)
**Impact on plan:** Scope stayed inside private solver policy internals and improved budget honesty. No public SDK, runner, README, or public CLI surface was added.

## Issues Encountered

- The plan's combined Cargo test filters are not valid Cargo syntax because Cargo accepts one test-name filter per invocation. I ran equivalent filters separately.
- Clippy flagged an oversized helper signature in the fixpoint; I refactored the mutable bucket indexes into `PropertyBucketAccumulator` instead of suppressing the lint.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 50-04 can layer bounded prototype/class/accessor lookup and receiver binding on top of the now-registered object policy. Property-backed call edges and budget latching are ready for those semantics to compose with.

---
*Phase: 50-js-ts-object-property-prototype-this-model-driver*
*Completed: 2026-06-04*

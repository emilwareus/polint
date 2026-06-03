---
phase: 49-js-ts-function-token-propagation-driver
plan: 02
subsystem: solver
tags: [js, ts, solver, function-token, derived-edge, budget]

requires:
  - phase: 49-js-ts-function-token-propagation-driver
    plan: 01
    provides: "JS token budgets, [solver.js] config, cache-key participation, and TokenFlowRequired handoff classifier"
  - phase: 47-unified-solver-core-derived-edge-provenance
    provides: "SolverEngine, SolverPolicy, DerivedEdgeFact, DerivedEdgeProvenance, BudgetStatus"
  - phase: 45-js-ts-inventory-scope-bindings-module-graph-direct-calls
    provides: "TS inventory/direct-binding facts plus semantic CopyEdge and CallConstraint rows"
provides:
  - "crate-private analysis::solver::ts_tokens closed input snapshot"
  - "deterministic JS/TS function-token fixpoint with too-many-tokens sentinel"
  - "token-derived DerivedEdgeFact call edges with provenance and conservative precision"
  - "real TsTokensPolicy wired into SolverEngine production provider"
affects: [JS-04, phase-49-plan-03-fixtures, solver, semantic_graph]

tech-stack:
  added: []
  patterns:
    - "TS token propagation mirrors Go RTA structure: closed inputs, BTree/BTreeSet ordering, bounded worklist, normalized derived edges"
    - "The sentinel is a lattice state only; it is never emitted as a callable target"

key-files:
  created:
    - "crates/polint/src/analysis/solver/ts_tokens/inputs.rs"
    - "crates/polint/src/analysis/solver/ts_tokens/fixpoint.rs"
    - "crates/polint/src/analysis/solver/ts_tokens/dispatch.rs"
  modified:
    - "crates/polint/src/analysis/solver/ts_tokens/mod.rs"
    - "crates/polint/src/analysis/solver/policy.rs"
    - "crates/polint/src/analysis/solver/provider.rs"
    - "crates/polint/src/analysis/solver/engine.rs"
    - "crates/polint/src/eval/go_rta.rs"

requirements-progress: [JS-04]

duration: 90min
completed: 2026-06-03T18:27:54Z
---

# Phase 49 Plan 02: TS Token Driver Summary

Phase 49 Plan 02 replaced the reserved `TsTokensPolicy` behavior with a real private JS/TS function-token solver. The policy now owns a closed `TsTokenInputs` snapshot, propagates function tokens through semantic `CopyEdge`s, collapses over-cap variables to `"too-many-tokens"`, and emits conservative `DerivedEdgeFact` call edges through the existing solver engine.

## Accomplishments

- Added `analysis::solver::ts_tokens::{inputs, fixpoint, dispatch}` as crate-private modules.
- Built `TsTokenInputs::from_db(db)` over TS semantic function/callsite nodes, `CopyEdge`, `CallConstraint`, and `TokenFlowRequired` handoff rows while excluding property/prototype/`this` reasons.
- Implemented deterministic token propagation with `VecDeque`, `BTreeMap`, and `BTreeSet`; per-variable overflow renders as `"too-many-tokens"` and latches `BudgetStatus::BudgetExceeded`.
- Implemented token call dispatch to `DerivedEdgeFact` rows with caller-function source, function-token target, `CallConstraint` provenance, token-flow contributing facts, and non-exact precision.
- Wired `TsTokensPolicy::solve()` to return derived edges, budget status, and steps through `PolicyOutcome`.
- Updated the polyglot Go+TS canary assertion from the old Phase 48 "TS policy empty" invariant to the Phase 49 invariant: TS token edges may exist, but must remain intra-TS and never cross into Go.

## Task Commits

1. Task 1: closed TS token input model and provider registration - `5839eaf5`
2. Tasks 2-4: token fixpoint, dispatch, policy replacement, and engine/eval assertion updates - `28606b63`

## Verification

- `cargo test -p polint analysis::solver::ts_tokens` - passed, 14 tests
- `cargo test -p polint analysis::solver::policy` - passed, 5 tests
- `cargo test -p polint analysis::solver::engine` - passed, 17 tests
- `cargo test -p polint analysis::solver::provider` - passed, 8 tests
- `cargo test -p polint --test public_surface_leak` - passed, 5 tests
- `cargo test -p polint polyglot_go_ts_canary_resolves_go_edges_without_ts_interference` - passed
- Pre-commit `make lint` passed on both commits (`cargo fmt --all -- --check` plus workspace clippy/all-targets/all-features with `-D warnings`).

## Deviations

- The Plan 02 implementation committed Tasks 2-4 together because fixpoint budget behavior, callsite candidate caps, dispatch, and policy output are coupled in the same solver result.
- The plan's combined Cargo filters used invalid syntax (`cargo test` accepts one test-name filter). I ran the same coverage as separate valid filters.
- The polyglot canary text/assertion was updated in Plan 02 to stop encoding the obsolete Phase 48 TS-stub assumption; Plan 03 still owns the fixture-level assertion that TS token edges are present.

## Next Phase Readiness

Plan 49-03 can now add native TS token fixtures, token-explosion budget proof, determinism coverage, Jelly evidence, and final roadmap bookkeeping over a real `TsTokensPolicy`.

## Self-Check: PASSED

- Summary artifact: `.planning/phases/49-js-ts-function-token-propagation-driver/49-02-SUMMARY.md`
- Required commits: `5839eaf5`, `28606b63`
- Key invariant: no public SDK/runner/CLI surface promoted; token solver internals remain crate-private.

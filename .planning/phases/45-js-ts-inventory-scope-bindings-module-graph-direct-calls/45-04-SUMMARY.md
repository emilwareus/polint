---
phase: 45-js-ts-inventory-scope-bindings-module-graph-direct-calls
plan: 04
subsystem: analysis
tags: [rust, typescript, javascript, direct-bindings, semantic-graph, constraints]

requires:
  - phase: 45-03
    provides: private TS direct binding rows and cache input contract
  - phase: 44
    provides: semantic graph node/edge/constraint vocabulary
provides:
  - Projection from resolved TS direct bindings into semantic graph `CopyEdge` and `CallConstraint` constraints
  - Calls/refined-calls contract guard proving Phase 45 does not emit `CallTargetFact` rows
  - Semantic graph output digest participation for TS direct binding output
  - Debug and validation coverage for TS direct binding constraint rows
affects: [phase-45, js-ts-analysis, direct-binding, semantic-graph]

tech-stack:
  added: []
  patterns: [semantic-graph-only projection, stable identity endpoint matching, no-solver boundary tests]

key-files:
  modified:
    - crates/polint/src/analysis/semantic_graph/build.rs
    - crates/polint/src/analysis/semantic_graph/cache_key.rs
    - crates/polint/src/analysis/semantic_graph/debug.rs
    - crates/polint/src/analysis/semantic_graph/provider.rs
    - crates/polint/src/analysis/semantic_graph/validate.rs
    - crates/polint/src/ts/binding/store.rs

key-decisions:
  - "TS direct bindings project into semantic graph constraints only; calls/refined-calls facts remain unchanged in Phase 45."
  - "Endpoint resolution matches TS inventory stable identities back to existing FunctionFact and CallSiteFact semantic nodes by file/span/name."
  - "Unresolved token/property/prototype/this-required binding rows emit no target constraints."
  - "Semantic graph cache/output digest now folds a TS direct binding output digest and parameter digest."

patterns-established:
  - "collect_ts_direct_bindings(db) derives private TS direct binding output from source files and existing module graph facts."
  - "project_ts_direct_bindings emits constraints only for TsDirectBindingStatus::Resolved."
  - "Debug JSON marks TS direct binding constraint origin and referenced node ids for deterministic snapshots."

requirements-advanced: [JS-03]
requirements-completed: []

duration: 25 min
completed: 2026-05-31
---

# Phase 45 Plan 04: Semantic Graph Projection Summary

**TS direct binding rows now feed semantic graph `CopyEdge` and `CallConstraint` constraints**

## Performance

- **Duration:** 25 min
- **Started:** 2026-05-31T19:12:00Z
- **Completed:** 2026-05-31T19:36:38Z
- **Tasks:** 4
- **Files modified:** 6

## Accomplishments

- Added semantic graph projection for resolved TS direct binding rows, including local aliases and imported aliases.
- Preserved the existing calls/refined-calls contract: Phase 45 does not create or mutate `CallTargetFact` rows.
- Threaded TS direct binding output into semantic graph provider digesting, parameter cache key text, debug JSON, and validation coverage.
- Added no-solver boundary tests for token-flow, property-flow, prototype-model, and `this`-model unresolved direct binding rows.

## Task Commits

1. **Task 1: Add semantic graph projection for TS direct bindings** - `9dee6d35` (`feat`)
2. **Task 2: Preserve calls/refined-calls contract** - `21e7928d` (`test`)
3. **Task 3: Thread TS binding output into semantic graph cache/debug/validation** - `c3219924` (`feat`)
4. **Task 4: Preserve no-solver boundary in semantic graph projection** - `7ef2e22b` (`test`)

## Files Created/Modified

- `crates/polint/src/analysis/semantic_graph/build.rs` - Added TS direct binding collection/projection and no-solver/calls-contract tests.
- `crates/polint/src/analysis/semantic_graph/cache_key.rs` - Added TS direct binding output/projection parts to semantic graph parameter digest.
- `crates/polint/src/analysis/semantic_graph/debug.rs` - Added TS direct binding source and referenced node fields to debug constraint rows.
- `crates/polint/src/analysis/semantic_graph/provider.rs` - Folded TS direct binding output digest into semantic graph output digest.
- `crates/polint/src/analysis/semantic_graph/validate.rs` - Added row-level validation helper and dangling TS direct binding constraint test.
- `crates/polint/src/ts/binding/store.rs` - Added TS direct binding output digest helper.

## Decisions Made

- No public API promotion: all touched APIs remain crate-private.
- No call-target promotion: direct TS binding constraints are semantic-graph-only for Phase 45.
- No solver behavior: unresolved dynamic/direct-binding boundary rows are skipped by projection rather than guessed.

## Deviations from Plan

- The plan verify command with multiple Cargo filters was split into separate valid Cargo invocations for `analysis::semantic_graph::{cache_key,validate,debug}`.

## Verification

- `cargo fmt --all --check` - passed
- `cargo check -p polint` - passed
- `cargo test -p polint --lib analysis::semantic_graph::build::tests::ts_direct_bindings` - passed
- `cargo test -p polint --lib analysis::semantic_graph` - passed
- `cargo test -p polint --lib analysis::calls` - passed
- `cargo test -p polint --lib ts::binding` - passed
- `cargo test -p polint --lib analysis::semantic_graph::cache_key` - passed
- `cargo test -p polint --lib analysis::semantic_graph::validate` - passed
- `cargo test -p polint --lib analysis::semantic_graph::debug` - passed
- `cargo test -p polint --lib analysis::semantic_graph::provider` - passed
- `cargo test -p polint --lib ts::binding::store` - passed
- `rg -n "analysis::solver|fixed-point|token propagation|token-set" crates/polint/src/analysis/semantic_graph crates/polint/src/ts/binding` - no matches

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 45-05 can add end-to-end fixture coverage for Jelly spans, semantic graph TS direct binding snapshots, cache/determinism behavior, and the public surface leak gate. JS-03 should be marked complete after those fixtures pass.

## Self-Check: PASSED

All semantic graph projection tasks and acceptance checks passed.

---
*Phase: 45-js-ts-inventory-scope-bindings-module-graph-direct-calls*
*Completed: 2026-05-31*

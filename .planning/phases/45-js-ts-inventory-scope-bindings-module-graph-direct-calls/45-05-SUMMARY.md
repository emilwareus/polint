---
phase: 45-js-ts-inventory-scope-bindings-module-graph-direct-calls
plan: 05
subsystem: eval
tags: [rust, typescript, javascript, fixtures, determinism, public-surface]

requires:
  - phase: 45-01
    provides: private TS/JS inventory extraction
  - phase: 45-02
    provides: private TS/JS scope extraction and module graph coverage
  - phase: 45-03
    provides: private TS direct binding rows
  - phase: 45-04
    provides: semantic graph projection for TS direct binding constraints
provides:
  - Jelly-shaped JS/TS inventory span fixture coverage for all JS-01 forms
  - Semantic graph fixture for TS direct binding CopyEdge and CallConstraint rows
  - Cache/digest regression coverage for TS direct binding and semantic graph inputs
  - Determinism and public-surface gate closure for Phase 45
affects: [phase-45, js-ts-analysis, semantic-graph, eval-fixtures]

tech-stack:
  added: []
  patterns: [native eval fixtures, semantic graph debug snapshots, private cache digest tests]

key-files:
  modified:
    - crates/polint/src/eval/external/jelly_callgraph.rs
    - crates/polint/src/eval/semantic_graph_snapshot.rs
    - crates/polint/src/analysis/semantic_graph/cache_key.rs
    - crates/polint/src/analysis/semantic_graph/provider.rs
    - crates/polint/src/ts/binding/store.rs
  created:
    - tests/eval-fixtures/jelly/ts-inventory-spans/
    - tests/eval-fixtures/semantic-graph/ts_direct_bindings/

key-decisions:
  - "Fixture tests copy semantic-graph fixture repos into temp directories before running the kernel, so layer-cache files are not written into checked-in fixtures."
  - "TS direct binding coverage remains private/test-facing and does not extend `ALLOWED_PRELUDE` or any SDK surface."
  - "Plan verify commands containing multiple Cargo filters were split into valid one-filter Cargo invocations."

patterns-established:
  - "Jelly inventory span coverage is asserted as named JS-01 expectations with file/span/reason diagnostics for misses."
  - "Semantic graph direct-binding fixture asserts TS direct CopyEdge/CallConstraint rows are stable, sorted, and node-resolvable."
  - "TS direct binding output digest tests cover source, scope, module, status/reason changes plus no-op row order preservation."

requirements-advanced: [JS-01, JS-02, JS-03]
requirements-completed: [JS-03]

duration: 58 min
completed: 2026-05-31
---

# Phase 45 Plan 05: Fixture and Gate Closure Summary

**Phase 45 is complete. JS-01, JS-02, and JS-03 are now fixture-backed and gate-verified.**

## Performance

- **Duration:** 58 min
- **Started:** 2026-05-31T19:36:38Z
- **Completed:** 2026-05-31T20:34:23Z
- **Tasks:** 4
- **Implementation commits:** 3

## Accomplishments

- Added `tests/eval-fixtures/jelly/ts-inventory-spans/` and a Jelly span coverage test covering declarations, expressions, arrows, methods, constructors, accessors, class static blocks, calls, `new`, tagged templates, optional calls, dynamic import, non-string dynamic import, and `require`.
- Added `tests/eval-fixtures/semantic-graph/ts_direct_bindings/` with ESM named/default/namespace imports, re-export, CommonJS require member, TypeScript path alias, destructuring, local alias, and unresolved non-string dynamic import coverage.
- Extended semantic graph snapshots to assert TS direct-binding `CopyEdge` and `CallConstraint` rows are stable and reference existing graph nodes.
- Added cache/digest regressions proving TS direct binding output changes invalidate semantic graph/cache identity while no-op row order changes preserve hits.
- Confirmed the determinism gate and frozen public-surface leak gate remain green without extending `ALLOWED_PRELUDE`.

## Task Commits

1. **Task 1: Add fixture coverage for inventory and Jelly spans** - `9799c5c6` (`test`)
2. **Task 2: Add module/binding semantic graph fixture** - `a9f95b80` (`test`)
3. **Task 3/4: Cache, determinism, and public-surface gate proof** - `2c66974c` (`test`)

## Files Created/Modified

- `crates/polint/src/eval/external/jelly_callgraph.rs` - Added named JS-01 inventory span coverage assertions through the Jelly span renderer.
- `crates/polint/src/eval/semantic_graph_snapshot.rs` - Added TS direct-binding semantic graph fixture assertions and temp-copy kernel execution.
- `crates/polint/src/analysis/semantic_graph/cache_key.rs` - Added pre-Phase-45 digest invalidation proof.
- `crates/polint/src/analysis/semantic_graph/provider.rs` - Added output digest folding proof for TS direct binding and module topology digests.
- `crates/polint/src/ts/binding/store.rs` - Added output digest must-invalidate and no-op row-order preservation tests.
- `tests/eval-fixtures/jelly/ts-inventory-spans/` - New JS/TS inventory span fixture.
- `tests/eval-fixtures/semantic-graph/ts_direct_bindings/` - New TS direct binding semantic graph fixture.

## Deviations from Plan

- Split invalid multi-filter Cargo commands into separate valid invocations.
- The full `cargo test -p polint` suite passed before a final clippy-only redundant clone cleanup; after that cleanup, `cargo test -p polint --lib ts::binding::store` and the pre-commit `make lint` hook passed.

## Verification

- `cargo fmt --all --check` - passed
- `cargo check -p polint` - passed
- `cargo test -p polint ts_inventory_spans` - passed
- `cargo test -p polint ts_direct_bindings_fixture` - passed
- `cargo test -p polint --lib ts::binding::store` - passed
- `cargo test -p polint --lib analysis::semantic_graph::cache_key` - passed
- `cargo test -p polint --lib analysis::semantic_graph::provider` - passed
- `cargo test -p polint --lib analysis_kernel::incremental::keys` - passed
- `cargo test -p polint --lib determinism_gate` - passed
- `cargo test -p polint jelly` - passed
- `cargo test -p polint semantic_graph` - passed
- `cargo test -p polint --test public_surface_leak` - passed
- `cargo test -p polint --lib ts::inventory` - passed
- `cargo test -p polint --lib ts::scope` - passed
- `cargo test -p polint --lib ts::binding` - passed
- `cargo test -p polint` - passed
- `rg -n "analysis::solver|fixed-point|token propagation|token-set" crates/polint/src/analysis/semantic_graph crates/polint/src/ts/binding` - no matches
- `git diff -- crates/polint/tests/public_surface_leak.rs` - no diff

## User Setup Required

None.

## Next Phase Readiness

Phase 46 is now the next GSD phase: Go Semantic Frontend & Sidecar.

## Self-Check: PASSED

All Plan 05 acceptance checks passed, and Phase 45 is ready to close.

---
*Phase: 45-js-ts-inventory-scope-bindings-module-graph-direct-calls*
*Completed: 2026-05-31*

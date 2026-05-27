---
phase: 38-local-plus-summary-projected-data-flow
plan: 10
subsystem: data-flow-proof
tags: [rust, data-flow, eval, debug, public-boundary]
requires:
  - phase: 38-local-plus-summary-projected-data-flow
    provides: local and interprocedural data-flow rows
provides:
  - Data-flow debug/eval proof, validation closeout, and public no-leak coverage
affects: [data-flow, eval, docs, public-boundary]
tech-stack:
  added: []
  patterns: [test-only debug rows, eval taxonomy fixtures, reserved SDK view docs]
key-files:
  created:
    - crates/polint/src/analysis/data_flow/debug.rs
    - tests/eval-fixtures/data-flow/core/expected.polint-eval.toml
    - tests/eval-fixtures/data-flow/core/repo/.polint.toml
    - tests/eval-fixtures/data-flow/core/repo/src/app.ts
  modified:
    - crates/polint/src/analysis/data_flow/validate.rs
    - crates/polint/src/analysis_kernel/debug.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/validation.rs
    - crates/polint/src/eval/fixtures.rs
    - crates/polint/src/eval/model.rs
    - crates/polint/src/eval/observed.rs
    - crates/polint/tests/cli.rs
    - docs/facts/data-flow.md
key-decisions:
  - "Keep public docs limited to a reserved-view note; internal data-flow row names remain private."
  - "Use test-only data-flow debug JSON as the eval observation bridge for Phase 38 proof."
patterns-established:
  - "Data-flow eval fixtures must cover local, direct-call, summary-projected, model, uncertainty, budget, and false-positive-trap categories."
  - "Dedicated public-boundary tests allow the reserved `DataFlow<'_>` SDK marker while blocking internal row/provider markers."
requirements-completed: [SAE-PREC-03]
duration: 75min
completed: 2026-05-25
---

# Phase 38 Plan 10 Summary

**Data-flow eval fixtures, debug, and public boundary proof**

## Accomplishments
- Added deterministic test-only data-flow debug JSON rows and counts for nodes, edges, models, budgets, edge kinds, statuses, local edges, direct-call edges, summary-projected edges, unknown/havoc rows, and budget rows.
- Integrated data-flow debug observations into the eval harness and added focused fixture/taxonomy tests.
- Added a data-flow eval fixture covering local, direct-call, summary-projected, model, unknown/havoc, budget, false-positive-trap, and runtime-budget proof rows.
- Extended validation and metadata diagnostics for malformed data-flow output.
- Added a dedicated public no-leak test and rewrote `docs/facts/data-flow.md` as a reserved-view note.

## Verification
- `cargo test -p polint --lib eval_native_fixture_runner_data_flow_fixture_passes --locked`
- `cargo test -p polint --lib eval_data_flow_manifests_cover_required_taxonomy --locked`
- `cargo test -p polint --lib data_flow_public_no_leak --locked`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`

## Deviations from Plan
The data-flow eval fixture uses the eval harness's supported `synthetic_observed = true` path for focused taxonomy proof. Native provider/debug observation is still covered by unit, integration, public-boundary, and full workspace gates, but the fixture is not a full end-to-end real-repo data-flow observation fixture yet.

## Issues Encountered
Older public-boundary tests for semantic MIR and abstract domains treated `DataFlow<'_>` as an internal marker. They were updated to preserve those families' no-leak checks while allowing the now-intentional reserved SDK marker.

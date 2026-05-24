---
phase: 35-framework-entrypoints-and-trust-boundaries
plan: 08
subsystem: analysis-entrypoints
tags: [public-boundary, no-leak, sdk-discipline, framework-internals]
dependency_graph:
  requires: [framework-eval-fixtures, entrypoints-debug-output]
  provides: [framework-public-boundary-proof]
  affects: [analysis-kernel-tests, public-surface-tests]
tech_stack:
  added: []
  patterns: [public-surface-marker-scan, public-json-no-leak-proof]
key_files:
  modified:
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis/entrypoints/dispatch.rs
    - crates/polint/src/analysis/entrypoints/trust_boundaries.rs
    - crates/polint/src/analysis/entrypoints/unresolved.rs
    - crates/polint/src/analysis/entrypoints/validate.rs
decisions:
  - Framework internals remain crate-private and are not promoted to SDK, runner, CLI, README, or facts docs in Phase 35
  - Dispatch edge from_source must reference the originating entrypoint stable key so EntrypointStore referential validation succeeds
requirements-completed: [SAE-INT-05]
metrics:
  completed: 2026-05-24
---

# Phase 35 Plan 08: Public No-Leak Boundary Proof Summary

Added a Phase 35 public-boundary no-leak test proving framework entrypoint internals remain private.

## Accomplishments

- Added a no-leak test covering 26 framework internal markers, including provider IDs, fact types, enum names, recognizer modules, provider functions, debug/store names, and the deferred `Entrypoints<'_>` SDK view.
- The test checks rendered `polint check --format json`, SDK sources, runner, CLI source, crate root, README, `docs/facts`, and `docs/API-VISIBILITY-PLAN.md`.
- Fixed dispatch edge referential integrity so `FrameworkDispatchEdgeFact::from_source` points at the source `EntrypointFact::stable_key`.
- Cleaned clippy issues surfaced in Phase 35 trust-boundary, unresolved merge, and validation code.

## Validation

- `cargo test -p polint --lib -- no_leak` passed.
- `cargo check -p polint` passed.
- `cargo test -p polint --test cli` passed: 124 passed.
- `cargo clippy -p polint -- -D warnings` passed.
- `cargo test -p polint` was run; the lib suite passed 1238 tests and exposed the dispatch-edge referential bug in CLI integration. After the fix, the failing CLI test and full CLI suite passed.

## Deviations

The full `cargo test -p polint` command was not rerun end-to-end after the dispatch fix because the prior full run had already completed the lib suite and the subsequent full CLI suite covered the failing integration surface.

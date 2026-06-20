---
phase: 57-control-flow-guard-and-lifecycle-queries
plan: 02
subsystem: sdk
tags: [policy-query, control-flow, lifecycle, cleanup]
requires:
  - phase: 57-control-flow-guard-and-lifecycle-queries
    provides: ordered call-event projection
provides:
  - Provider-backed `ControlFlow<'_>::missing_cleanup(LifecycleQuery)`
  - Same-function lifecycle cleanup diagnostics
affects: [phase-58, phase-59, templates]
tech-stack:
  added: []
  patterns: [same-function ordered policy evidence]
key-files:
  created: []
  modified:
    - crates/polint/src/policy_queries.rs
    - crates/polint/src/sdk/facts.rs
key-decisions:
  - "Treat cleanup as a same-function call-order policy in Phase 57."
  - "Surface `require_error_cleanup` as evidence, but do not claim exact every-exit cleanup proof yet."
patterns-established:
  - "Lifecycle policy results use the same `PolicyViolation` evidence vocabulary as guard and reachability queries."
requirements-completed: [CTRL-02, CTRL-03, CTRL-04]
duration: recorded
completed: 2026-06-20
---

# Phase 57 Plan 02 Summary

**Same-function lifecycle cleanup query support through `ControlFlow<'_>`**

## Accomplishments

- Implemented `missing_cleanup` for call start/acquire events requiring a later same-function cleanup call.
- Reused the ordered call-event projection and evidence construction from guard queries.
- Added unit coverage for missing cleanup and satisfied cleanup.
- Kept exact resource identity, every-exit proof, and interprocedural cleanup search out of the public claims.

## Verification

- `cargo test -p polint --lib control_flow_missing_cleanup --locked`
- Covered again by `cargo test -p polint --lib --locked`

## Deviations

- Phase 57 does not prove cleanup on every normal and error exit. Diagnostics are intentionally heuristic/conservative and document the supported same-function scope.

## Next Phase Readiness

Lifecycle results now use the same compact policy evidence shape expected by Phase 59 evidence normalization.


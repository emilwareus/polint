---
phase: 56-events-and-calls-query-surface
plan: 02
subsystem: sdk
tags: [policy-query, reachability, calls, unknowns]
requires:
  - phase: 56-events-and-calls-query-surface
    provides: event pattern matching
provides:
  - Provider-backed `Calls<'_>::forbidden_reachable(ReachQuery)`
  - Deterministic reachable-call search over private refined-call and root facts
affects: [phase-57, phase-58, phase-59, templates]
tech-stack:
  added: []
  patterns: [bounded private BFS, policy-level evidence projection]
key-files:
  created:
    - crates/polint/src/policy_queries.rs
  modified:
    - crates/polint/src/sdk/facts.rs
    - crates/polint/src/sdk/policy.rs
key-decisions:
  - "Expose minimum confidence as a small public enum instead of leaking refined-call confidence internals."
  - "Filter by precision/confidence before traversal, but still surface unresolved matching targets honestly."
patterns-established:
  - "Reachability queries return `PolicyViolation`s with root/path/target/depth/status/precision/confidence evidence."
requirements-completed: [CALL-02, CALL-03, CALL-04]
duration: recorded
completed: 2026-06-20
---

# Phase 56 Plan 02 Summary

**Reachable-call policy queries over private refined-call edges and reachability roots**

## Accomplishments

- Implemented bounded deterministic BFS for `Calls::forbidden_reachable`.
- Supported `ReachQuery` root filters, target pattern, test exclusion/inclusion, max depth, max paths, minimum precision, and minimum confidence.
- Added `PolicyConfidence` as the public query-level confidence knob.
- Added coverage for selected roots, test-root exclusion/inclusion, unresolved matching targets, and path-budget truncation evidence.

## Verification

- `cargo test -p polint --lib calls_forbidden_reachable --locked`
- Covered again by `cargo test -p polint --lib --locked`

## Deviations

- Package/module scoping was not added in Phase 56. It would introduce a second scope surface before query evidence is normalized, so it is deferred rather than exposed prematurely.

## Next Phase Readiness

Later query families can reuse the `PolicyViolation` projection style and confidence/precision vocabulary without exposing private graph IDs.

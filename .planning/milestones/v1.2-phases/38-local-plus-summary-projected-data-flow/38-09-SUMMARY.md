---
phase: 38-local-plus-summary-projected-data-flow
plan: 09
subsystem: data-flow-interprocedural
tags: [rust, data-flow, calls, summaries]
requires:
  - phase: 38-local-plus-summary-projected-data-flow
    provides: local value-flow edges and stored uncertainty
provides:
  - Direct/refined-call and summary-projected data-flow closure
affects: [data-flow, calls, direct-summaries]
tech-stack:
  added: []
  patterns: [refined-call projection, summary projection, explicit setup-missing rows]
key-files:
  created:
    - crates/polint/src/analysis/data_flow/direct_calls.rs
    - crates/polint/src/analysis/data_flow/summary_edges.rs
  modified:
    - crates/polint/src/analysis/data_flow/facts.rs
    - crates/polint/src/analysis/data_flow/mod.rs
    - crates/polint/src/analysis/data_flow/provider.rs
key-decisions:
  - "Resolved refined calls produce role-specific data-flow edges; unresolved refined calls remain visible as setup-missing/unknown rows."
  - "DataFlowTito summary facts project into compact summary edges; uncertain summary facts/events project into unknown, havoc, or budget-truncated rows."
patterns-established:
  - "Interprocedural data-flow projection consumes existing refined-call and summary facts without introducing a dependency cycle."
  - "Summary-projected rows carry upstream stable keys and compact evidence only."
requirements-completed: [SAE-PREC-03]
duration: 55min
completed: 2026-05-25
---

# Phase 38 Plan 09 Summary

**Summary-projected and interprocedural data-flow closure**

## Accomplishments
- Added direct/refined-call projection for argument-to-parameter, receiver-to-method, and call-return-to-use data-flow edges.
- Added unresolved refined-call handling that stores evidence-backed unknown/setup-missing data-flow rows.
- Added summary projection for `DataFlowTito` summary facts and uncertain summary facts/events.
- Rewired the data-flow provider to derive local edges, direct-call projection, and summary-projected edges before model derivation.

## Verification
- `cargo test -p polint --lib analysis::data_flow::direct_calls --locked`
- `cargo test -p polint --lib analysis::data_flow::summary_edges --locked`
- `cargo test -p polint --lib data_flow --locked`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`

## Deviations from Plan
None.

## Issues Encountered
An unused non-test import in the direct-call projection module was caught during verification and moved into the test module.

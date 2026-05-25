---
phase: 38-local-plus-summary-projected-data-flow
plan: 04
subsystem: analysis
tags: [rust, data-flow, calls, summaries]
requires:
  - phase: 38-local-plus-summary-projected-data-flow
    provides: refined calls and data-flow graph storage
provides:
  - Direct-call projected data-flow edges from resolved refined-call edges
affects: [data-flow, refined-calls]
tech-stack:
  added: []
  patterns: [resolved refined-call projection]
key-files:
  created: []
  modified:
    - crates/polint/src/analysis/data_flow/provider.rs
key-decisions:
  - "Only resolved refined-call edges produce direct-call data-flow edges."
patterns-established:
  - "Interprocedural edges retain `call_site`, base `call_target`, refined-call id, evidence, and input stable keys."
requirements-completed: [SAE-PREC-03]
duration: 8min
completed: 2026-05-25
---

# Phase 38 Plan 04 Summary

**Resolved refined-call edges projected as interprocedural data-flow edges**

## Accomplishments
- Added call-boundary nodes for call arguments and returns.
- Added `CallArgumentToParameter` edges for resolved refined-call rows.
- Preserved provenance and input stable-key linkage back to refined-call facts.

## Task Commits
1. **Direct-call data-flow projection** - `bf41e6c` (feat)

## Verification
- `cargo check -p polint`
- `cargo test -p polint data_flow --lib`

## Deviations from Plan
Summary-projected TITO edges are represented in the vocabulary and manifest inputs, but deeper summary event payload projection remains a follow-up because current summary rows expose domain/status metadata rather than structured source/target roots.

## Issues Encountered
None.

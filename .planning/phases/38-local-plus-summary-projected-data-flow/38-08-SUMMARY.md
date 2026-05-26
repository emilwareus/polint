---
phase: 38-local-plus-summary-projected-data-flow
plan: 08
subsystem: data-flow-local
tags: [rust, data-flow, mir, validation]
requires:
  - phase: 38-local-plus-summary-projected-data-flow
    provides: private data-flow facts, provider wiring, and path search
provides:
  - Local MIR value-flow edges and stored uncertainty/budget rows
affects: [data-flow, semantic-mir, eval]
tech-stack:
  added: []
  patterns: [private fact derivation, deterministic stable keys, explicit uncertainty rows]
key-files:
  created:
    - crates/polint/src/analysis/data_flow/local.rs
  modified:
    - crates/polint/src/analysis/data_flow/facts.rs
    - crates/polint/src/analysis/data_flow/mod.rs
    - crates/polint/src/analysis/data_flow/provider.rs
    - crates/polint/src/analysis/data_flow/query.rs
    - crates/polint/src/analysis/data_flow/store.rs
    - crates/polint/src/analysis/data_flow/validate.rs
key-decisions:
  - "Keep Phase 38 local value-flow private and evidence-backed; unsupported local shapes produce explicit unknown/havoc rows."
  - "Convert budget-exceeded path observations into stored DataFlowBudgetFact rows instead of leaving them only in query memory."
patterns-established:
  - "Local data-flow edges are keyed from stable MIR/place inputs and not from dense IDs."
  - "Budget-truncated edges must reference an existing budget row."
requirements-completed: [SAE-PREC-03]
duration: 70min
completed: 2026-05-25
---

# Phase 38 Plan 08 Summary

**Local value-flow edges and stored uncertainty**

## Accomplishments
- Added a local MIR value-flow builder for bindings, assignments, reads, writes, returns, projections, conservative call return flow, and unsupported local shapes.
- Added edge vocabulary for local bindings/reads/writes, return values, index projection, call argument-to-parameter, summary-projected, unknown, havoc, and budget-truncated rows.
- Extended the store with budget/status/place indexes and stricter validation for budget-truncated edges.
- Added path budget observation storage so budget-exceeded query results can be represented as stored data-flow budget rows.

## Verification
- `cargo test -p polint --lib data_flow --locked`
- `cargo test -p polint --lib analysis::data_flow::local --locked`
- `cargo test -p polint --lib analysis::data_flow::query --locked`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`

## Deviations from Plan
None.

## Issues Encountered
The first full workspace run exposed an overly brittle summary stable-key validation check while validating new stored rows. It was fixed in Plan 10 validation closeout.

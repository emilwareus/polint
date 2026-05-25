---
status: in-progress
created: 2026-05-25
---

# Quick Task 260525-otb: Implement TDD data-flow fixes

## Scope

Fix data-flow review issues in:
- `crates/polint/src/analysis/data_flow/direct_calls.rs`
- `crates/polint/src/analysis/data_flow/summary_edges.rs`
- `crates/polint/src/analysis/data_flow/query.rs`
- `crates/polint/src/analysis/data_flow/provider.rs`
- `crates/polint/src/analysis/data_flow/store.rs`
- `crates/polint/src/analysis/data_flow/facts.rs`

## Plan

1. Add focused failing unit tests for summary projection filtering, direct call-site places, refined-call status preservation, and explicit query budget reasons.
2. Implement only the data-flow changes required to satisfy those tests.
3. Run `cargo test -p polint analysis::data_flow --locked`.

---
status: complete
phase: 38-local-plus-summary-projected-data-flow
source:
  - .planning/phases/38-local-plus-summary-projected-data-flow/38-01-SUMMARY.md
  - .planning/phases/38-local-plus-summary-projected-data-flow/38-02-SUMMARY.md
  - .planning/phases/38-local-plus-summary-projected-data-flow/38-03-SUMMARY.md
  - .planning/phases/38-local-plus-summary-projected-data-flow/38-04-SUMMARY.md
  - .planning/phases/38-local-plus-summary-projected-data-flow/38-05-SUMMARY.md
  - .planning/phases/38-local-plus-summary-projected-data-flow/38-06-SUMMARY.md
  - .planning/phases/38-local-plus-summary-projected-data-flow/38-07-SUMMARY.md
  - .planning/phases/38-local-plus-summary-projected-data-flow/38-08-SUMMARY.md
  - .planning/phases/38-local-plus-summary-projected-data-flow/38-09-SUMMARY.md
  - .planning/phases/38-local-plus-summary-projected-data-flow/38-10-SUMMARY.md
started: 2026-05-25T13:45:51Z
updated: 2026-05-25T13:45:51Z
mode: technical
---

## Current Test

[testing complete]

## Tests

### 1. Local Data-Flow Edges
expected: Phase 38 derives real local MIR value-flow edges for bindings, assignments, reads, writes, returns, projections, and unsupported local uncertainty instead of only mirroring place nodes.
result: pass
evidence:
  - `cargo test -p polint --lib analysis::data_flow::local --locked`
  - `cargo test -p polint --lib data_flow --locked`

### 2. Interprocedural and Summary Projection
expected: Phase 38 consumes refined-call and direct-summary facts to produce role-specific interprocedural data-flow edges, summary-projected TITO rows, and visible unknown/havoc/budget rows for unresolved or uncertain summaries.
result: pass
evidence:
  - `cargo test -p polint --lib analysis::data_flow::direct_calls --locked`
  - `cargo test -p polint --lib analysis::data_flow::summary_edges --locked`
  - `cargo test -p polint --lib data_flow --locked`

### 3. Stored Budget, Unknown, Debug, and Validation Rows
expected: Budget-exceeded path observations and uncertainty are stored as deterministic data-flow output, validation rejects malformed rows, and debug JSON exposes deterministic internal rows and counts without absolute paths.
result: pass
evidence:
  - `cargo test -p polint --lib analysis::data_flow::query --locked`
  - `cargo test -p polint --lib data_flow --locked`
  - `cargo test --workspace --locked`

### 4. Eval and Public Boundary Proof
expected: Data-flow eval coverage runs non-zero focused tests for taxonomy and fixture proof, while public CLI, docs, runner, crate-root, and SDK surfaces do not expose internal data-flow row/provider details beyond the reserved `DataFlow<'_>` view.
result: pass
evidence:
  - `cargo test -p polint --lib eval_native_fixture_runner_data_flow_fixture_passes --locked`
  - `cargo test -p polint --lib eval_data_flow_manifests_cover_required_taxonomy --locked`
  - `cargo test -p polint --lib data_flow_public_no_leak --locked`
  - `cargo test --workspace --locked`

### 5. Whole-Workspace Gate
expected: The completed Phase 38 branch formats, lints with warnings denied, and passes the full workspace test suite.
result: pass
evidence:
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --locked`

## Summary

total: 5
passed: 5
issues: 0
pending: 0
skipped: 0

## Gaps

[none]

## Artifact Check

Current-phase artifact scan found no open Phase 38 UAT sessions, verification gaps, or context questions.

## Notes

Phase 38 has no UI or end-user workflow checkpoint. UAT was therefore completed as technical validation against the developer-observable outcomes from the phase summaries and the automated verification gates.

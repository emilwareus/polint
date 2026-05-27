---
phase: 38-local-plus-summary-projected-data-flow
verified: 2026-05-25T12:34:37Z
status: passed
score: 10/10 must-haves verified
overrides_applied: 0
gaps: []
---

# Phase 38: Local Plus Summary-Projected Data Flow Verification Report

**Phase Goal:** Add local value-flow graph, direct-call interprocedural edges, summary-projected edges, model sinks, budgets, unknowns, and query-scoped path search.
**Requirement:** SAE-PREC-03
**Verified:** 2026-05-25T12:34:37Z
**Status:** passed
**Re-verification:** Yes - gap-closure plans 38-08 through 38-10 executed.

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|---|---|---|
| 1 | A private `analysis::data_flow` module exists and is not a public SDK/CLI provider surface. | VERIFIED | `analysis::data_flow` remains crate-private; `DataFlow<'_>` is reserved without public query methods; `data_flow_public_no_leak` passed. |
| 2 | Data-flow fact vocabulary exists for nodes, edges, models, budgets, precision, status, validation, confidence, and provenance. | VERIFIED | `facts.rs` defines row families and status/precision/provenance vocabulary, including local, direct-call, summary, unknown, havoc, and budget edge kinds. |
| 3 | Stored output normalizes stable-key order, reassigns dense IDs, validates dangling references, and refreshes metadata. | VERIFIED | `store.rs` normalizes nodes/models/budgets/edges, remaps endpoints, indexes budget/status/place views, and rejects invalid budget-truncated edges. |
| 4 | `polint.data_flow` runs after refined calls and before metrics with provider output identity. | VERIFIED | Provider-order tests passed in full workspace verification. |
| 5 | Cache identity includes provider/schema, upstream digests, deterministic parameters, and input snapshot sentinels. | VERIFIED | Existing data-flow cache-key tests passed in full workspace verification. |
| 6 | Local data-flow has real local value-flow edges over MIR operations. | VERIFIED | `local.rs` derives local binding, assignment/read/write, return, projection, conservative call-return, unknown, and havoc rows; focused local tests passed. |
| 7 | Direct/refined call projection creates interprocedural data-flow facts. | VERIFIED | `direct_calls.rs` derives argument-to-parameter, receiver-to-method, call-return-to-use, and unresolved unknown/setup-missing rows; focused direct-call tests passed. |
| 8 | Summary-projected flow and missing-summary unknown/havoc behavior are present. | VERIFIED | `summary_edges.rs` consumes `DataFlowTito` summary facts/events and emits summary TITO, unknown, havoc, and budget-truncated rows; focused summary tests passed. |
| 9 | Source/sink/sanitizer/barrier models are represented with provenance. | VERIFIED | Existing model ingestion remains wired, and the data-flow eval taxonomy fixture covers source, sink, sanitizer, barrier, and extension model rows. |
| 10 | Budgets, unknowns, eval fixtures, debug/no-leak proof, and bounded path search satisfy the Phase 38 requirement. | VERIFIED | Stored budget observations, data-flow debug rows, eval fixture/taxonomy tests, validation checks, public no-leak test, and bounded path-search tests all passed. |

**Score:** 10/10 must-haves verified

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Formatting | `cargo fmt --all --check` | Passed | PASS |
| Focused data-flow tests | `cargo test -p polint --lib data_flow --locked` | Passed | PASS |
| Data-flow eval fixture | `cargo test -p polint --lib eval_native_fixture_runner_data_flow_fixture_passes --locked` | Passed | PASS |
| Data-flow eval taxonomy | `cargo test -p polint --lib eval_data_flow_manifests_cover_required_taxonomy --locked` | Passed | PASS |
| Public no-leak proof | `cargo test -p polint --lib data_flow_public_no_leak --locked` | Passed | PASS |
| Full workspace tests | `cargo test --workspace --locked` | Passed | PASS |
| Clippy | `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed | PASS |

### Requirements Coverage

| Requirement | Source Plans | Description | Status | Evidence |
|---|---|---|---|---|
| SAE-PREC-03 | 38-01 through 38-10 | polint has local and summary-projected data-flow facts, source/sink/sanitizer/barrier model sinks, budgets, unknown/havoc facts, and query-scoped path search. | COMPLETE | The private data-flow substrate, local/interprocedural derivation, model facts, explicit uncertainty/budgets, debug/eval proof, and public-boundary checks are implemented and verified. |

### Critical Review Findings

No blocking findings remain.

Residual risk: the focused data-flow eval fixture uses the eval harness's supported `synthetic_observed = true` mode for taxonomy coverage. The implementation itself is covered by focused provider/unit tests, metadata/debug tests, public-boundary tests, clippy, and full workspace tests.

### Human Verification Required

None. The phase is internal Rust analysis behavior and was verified through automated tests and source-level checks.

---

_Verified: 2026-05-25T12:34:37Z_
_Verifier: Codex_

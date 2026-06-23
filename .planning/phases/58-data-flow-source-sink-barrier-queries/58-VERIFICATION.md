# Phase 58 Verification: Data-Flow Source/Sink/Barrier Queries

## Verdict

PASS.

Phase 58 delivers a provider-backed `DataFlow<'_>::forbidden(FlowQuery)` preview
surface for the documented bounded source/sink/barrier scope.

## Requirement Coverage

| Requirement | Status | Evidence |
|-------------|--------|----------|
| FLOW-01 | Complete | `DataFlow<'_>::forbidden` calls the shared private policy-query engine and reports source-to-sink violations. |
| FLOW-02 | Complete | `BarrierPattern::call_any` suppresses paths that cross matching barrier calls and reports uncovered paths. |
| FLOW-03 | Complete | Supported patterns cover HTTP request sources, secret-like source names, exact call sinks, logger sinks, and explicit call barriers. |
| FLOW-04 | Complete | Query execution uses bounded private path search over stored data-flow facts with deterministic caps. |
| FLOW-05 | Complete | Diagnostic evidence includes path status, barrier status, precision/confidence, requested budgets, and budget reasons. |

## Checks Run

- `cargo test -p polint --lib data_flow_forbidden --locked`
- `cargo test -p polint --lib source_models_create_source_introduction_edges_to_matching_parameters --locked`
- `cargo test -p polint --lib policy_capabilities_report_phase_support_boundaries --locked`
- `cargo test -p polint --test cli phase5 --locked`
- `cargo test -p polint --test cli unknowns_json_reports_public_setup_and_resolution_gaps --locked`
- `cargo test -p polint --test cli inspect_unknowns_json_reports_consolidated_and_cap_filtered_rows --locked`
- `cargo fmt --all --check`
- `cargo test -p polint --test public_surface_leak --locked`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo doc -p polint --no-deps --locked`
- `cargo run -p polint --locked -- facts list --format json`
- `cargo test -p polint --lib --locked`

## Notes

- `DataFlow<'_>` remains a policy-query view, not a public raw graph view.
- `polint facts list --format json` reports `dataflow` as a preview capability.
- `dataflow` does not expose row sampling or cap-filtered unknown rows yet; the
  unknown/evidence normalization work is Phase 59.
- Broader SQL, HTML, SSRF, file path, analytics, PII, outbound network, and
  model-pack taxonomy remains future work unless expressed through the current
  exact-call and explicit-name patterns.


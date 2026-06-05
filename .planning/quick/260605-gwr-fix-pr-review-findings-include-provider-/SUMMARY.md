---
quick_id: 260605-gwr
slug: fix-pr-review-findings-include-provider-
status: complete
completed: 2026-06-05
---

# Summary: Fix PR Review Findings

## Completed

- Carried `AnalysisKernel` diagnostics through the private agent JSON analysis helper.
- Added diagnostics-aware unknown taxonomy collection so Go semantic provider failures are visible in `inspect unknowns`.
- Added a CLI regression for `POLINT_GO_FRONTEND` setup failure reporting as a `GoSemanticDiagnostic` unknown row.
- Strengthened the refined-calls fixture taxonomy test to require the extension-model delta invariant value to stay `0`.

## Verification

- `cargo test -p polint graph_engine_unknowns_include_go_provider_diagnostics_without_facts -- --nocapture`
- `cargo test -p polint eval_refined_calls_manifests_cover_required_taxonomy -- --nocapture`
- `cargo test -p polint inspect_unknowns_json_reports_go_provider_diagnostics -- --nocapture`
- `cargo test -p polint unknowns -- --nocapture`
- `cargo fmt --all -- --check`
- `cargo clippy -p polint --all-targets -- -D warnings`

---
phase: 37-refined-call-graph-providers
verified: 2026-05-27T07:32:55Z
status: passed
score: 6/6 plans verified from closeout artifacts
overrides_applied: 0
gaps: []
reverification: closeout-artifact-reconciliation
---

# Phase 37 Verification: Refined Call Graph Providers

## Result

PASS. Phase 37 satisfies `SAE-PREC-02`.

## Evidence Reviewed

- `37-01-SUMMARY.md`: private refined-call fact contracts and store.
- `37-02-SUMMARY.md`: provider, cache identity, and kernel wiring.
- `37-03-SUMMARY.md`: framework dispatch and summary-assisted refinements.
- `37-04-SUMMARY.md`: Go receiver and type-aware refinements.
- `37-05-SUMMARY.md`: TS/JS function-token, points-to, and extension-model refinements.
- `37-06-SUMMARY.md`: validation, debug, eval fixtures, public no-leak proof, clippy, and review fixes.

## Verification Commands Recorded In Phase Summaries

- `cargo test -p polint --lib analysis::refined_calls::validate --locked`
- `cargo test -p polint --lib analysis_kernel::validation --locked`
- `cargo test -p polint --lib analysis_kernel::debug --locked`
- `cargo test -p polint --lib eval_native_fixture_runner_refined_calls_fixture_passes --locked`
- `cargo test -p polint --lib eval_refined_calls_manifests_cover_required_taxonomy --locked`
- `cargo test -p polint --lib refined_call_rows --locked`
- `cargo test -p polint --lib eval_native_fixture_suite_covers_required_categories --locked`
- `cargo test -p polint --lib eval --locked`
- `cargo test -p polint --lib refined_call --locked`
- `cargo test -p polint --test cli --locked -- checked_in_examples_are_runnable_cli_fixtures`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo fmt --all --check`

## Requirement Coverage

| Requirement | Status | Evidence |
|---|---|---|
| SAE-PREC-02 | passed | Opt-in refined call providers over direct calls, framework dispatch, summaries, type/value facts, function tokens, receiver types, bounded points-to constraints, and explicit unresolved/budget statuses were implemented and verified. |

## Closeout Note

This verification file was restored during v1.2 archival reconciliation from existing phase summaries. No product code was changed.

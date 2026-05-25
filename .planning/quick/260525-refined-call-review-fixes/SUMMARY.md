# Refined-Call Review Fixes Summary

Completed: 2026-05-25

## Changes

- Converted `refined-calls-extension-model` from synthetic observed rows to real kernel observations.
- Updated the fixture extension to use `call_site:29` and `synthetic_target=extension:model-target`, avoiding brittle native function-id assumptions while still exercising the extension-to-refined-edge path.
- Strengthened `direct-vs-refined` expected facts to assert `precision = "unknown"` and `status = "unresolved"`.
- Added refined-call validation for missing `evidence` and `input_stable_keys`, with a regression test.

## Verification

- `cargo test -p polint --lib analysis::refined_calls::validate --locked`
- `cargo test -p polint --lib eval_native_fixture_runner_refined_calls_fixture_passes --locked`
- `cargo test -p polint --lib eval_refined_calls_manifests_cover_required_taxonomy --locked`
- `cargo test -p polint --lib refined_call --locked`
- `cargo test -p polint --lib eval --locked`
- `cargo test -p polint --test cli --locked -- checked_in_examples_are_runnable_cli_fixtures`
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `git diff --check`

## Review Result

No remaining Phase 37 refined-call review findings after the second pass.

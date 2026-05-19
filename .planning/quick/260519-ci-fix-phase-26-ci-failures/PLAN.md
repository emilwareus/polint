# Quick Task 260519-ci: Fix Phase 26 CI Failures

## Goal

Make the Phase 26 PR pass the attached GitHub Actions runs without widening public SDK, CLI, or eval surfaces.

## Diagnosis

The attached CI logs show three platform-sensitive failures:

- Rule manifest inspect test hardcodes SDK version `0.1.13`, while CI is building package version `0.1.14`.
- Symbol graph semantic payload validation uses host-native absolute-path detection, so a Unix-style absolute path is not rejected on Windows.
- The real-provider layer-cache eval fixture exceeded its 90 second Windows CI budget by 1.68 seconds while still passing all cache invariants.

## Scope

- Derive rule manifest expected SDK versions from `CARGO_PKG_VERSION`.
- Make absolute-path-like validation reject Unix, UNC, and Windows drive absolute paths on all hosts.
- Raise only the layer-cache eval runtime budget to 120 seconds.
- Leave public JSON, CLI, SDK, runner, and layer-cache invariant contracts unchanged.

## Verification

- `cargo test -p polint --lib rule_manifest::tests::inspect_rule_report_sorts_rules_and_uses_stable_top_level_fields`
- `cargo test -p polint --lib symbol_graph::semantic_layer_payload::validation_rejects_missing_keys_absolute_paths_and_stable_export_conflicts`
- `cargo test -p polint --lib eval::fixtures::eval_native_fixture_runner_tests::eval_layer_cache_fixture_passes --locked`
- `cargo test -p polint --lib eval::fixtures::eval_native_fixture_runner_tests::eval_native_fixture_suite_covers_required_categories --locked`
- `cargo clippy --all-targets --all-features --locked -- -D warnings`
- `cargo test --locked`
- `go test ./...`

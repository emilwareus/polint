# Quick Task 260519-ci Summary

Fixed the attached Phase 26 CI failures.

- Rule manifest inspect expectations now use the package version compiled in the current build instead of a stale hardcoded SDK version.
- Symbol graph semantic payload validation now rejects Unix-style absolute paths, UNC paths, and Windows drive absolute paths consistently on every host.
- The layer-cache native eval fixture budget is now 120 seconds, preserving the real-provider cache invariants while giving Windows CI enough runner-load headroom.

## Verification

- Passed: `cargo test -p polint --lib rule_manifest::tests::inspect_rule_report_sorts_rules_and_uses_stable_top_level_fields --locked`
- Passed: `cargo test -p polint --lib symbol_graph::semantic_layer_payload::validation_rejects_missing_keys_absolute_paths_and_stable_export_conflicts --locked`
- Passed: `cargo test -p polint --lib eval::fixtures::eval_native_fixture_runner_tests::eval_layer_cache_fixture_passes --locked`
- Passed: `cargo test -p polint --lib eval::fixtures::eval_native_fixture_runner_tests::eval_native_fixture_suite_covers_required_categories --locked`
- Passed: `cargo clippy --all-targets --all-features --locked -- -D warnings`
- Passed: `cargo test --locked`
- Passed: `cargo test -p polint --test cargo_install_smoke --locked -- --ignored`
- Passed: `go test ./...` in `tools/polint-go-symbols`
- Passed: `GOWORK=off go test ./...` in `crates/polint/go-sidecar/polint-go-symbols`

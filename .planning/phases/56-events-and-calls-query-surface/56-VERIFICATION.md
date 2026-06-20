---
phase: 56-events-and-calls-query-surface
status: passed
verified_at: 2026-06-20T15:38:50.000Z
requirements: [CALL-01, CALL-02, CALL-03, CALL-04]
human_verification: none
---

# Phase 56 Verification

## Verdict

Passed. Phase 56 now backs `Events<'_>::matching(EventPattern::call(...))` and `Calls<'_>::forbidden_reachable(ReachQuery)` with private provider facts, while preserving the public SDK boundary.

## Automated Checks

- `cargo test -p polint --lib events_matching_call_returns_provider_backed_violation --locked` — passed
- `cargo test -p polint --lib calls_forbidden_reachable --locked` — passed
- `cargo test -p polint --lib metadata_debug_helpers_are_not_public --locked` — passed
- `cargo test -p polint --lib refined_call_internals_do_not_leak_into_public_surfaces_no_leak --locked` — passed
- `cargo test -p polint --lib sdk_runner_and_bench_sources_do_not_leak_semantic_mir_storage --locked` — passed
- `cargo test -p polint --lib policy_capabilities_report_phase_support_boundaries --locked` — passed
- `cargo test -p polint --test cli phase56_events_and_calls_rule_reports_json --locked` — passed
- `cargo test -p polint --test cli phase55_preview_rule_syntax_compiles_and_fails_closed --locked` — passed
- `cargo test -p polint --test public_surface_leak --locked` — passed
- `cargo test -p polint --lib --locked` — passed, 2296 tests
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` — passed
- `cargo fmt --all --check` — passed
- `cargo doc -p polint --no-deps --locked` — passed
- `cargo run -p polint --locked -- facts list --format json` — passed
- `git diff --check` — passed

## Requirement Traceability

- `CALL-01`: `Events<'_>` matches call events over refined-call/call-site facts without public raw IDs. Non-call event families remain preview no-results.
- `CALL-02`: `Calls<'_>` reports reachable forbidden calls with root/path/target/depth/status/precision/confidence evidence.
- `CALL-03`: `ReachQuery` supports root filters, target patterns, tests inclusion, max depth, max paths, minimum precision, and minimum confidence. Package/module scoping is deferred intentionally.
- `CALL-04`: Results map refined-call status/precision/confidence to policy-level status and evidence, including unresolved and budget-exceeded cases.

## Residual Limits

- `EventPattern::write_field` has no backed results in Phase 56.
- Package/module scope filters are not exposed yet.
- After Phase 56, `ControlFlow<'_>` and `DataFlow<'_>` remained fail-closed until their later provider-backed phases.

---
phase: 57-control-flow-guard-and-lifecycle-queries
status: passed
verified_at: 2026-06-20T16:01:14.000Z
requirements: [CTRL-01, CTRL-02, CTRL-03, CTRL-04]
human_verification: none
---

# Phase 57 Verification

## Verdict

Passed. Phase 57 now backs `ControlFlow<'_>::missing_guard(GuardQuery)` and `ControlFlow<'_>::missing_cleanup(LifecycleQuery)` for same-function call-event policies over private call/refined-call facts and CFG operation order where available, while keeping raw CFG and MIR internals private.

## Automated Checks

- `cargo test -p polint --lib control_flow_missing_guard --locked` — passed
- `cargo test -p polint --lib control_flow_missing_cleanup --locked` — passed
- `cargo test -p polint --lib policy_capabilities_report_phase_support_boundaries --locked` — passed
- `cargo test -p polint --test cli phase55_preview_rule_syntax_compiles_and_fails_closed --locked` — passed
- `cargo test -p polint --test cli phase56_events_and_calls_rule_reports_json --locked` — passed
- `cargo test -p polint --test cli phase57_control_flow_rule_reports_json --locked` — passed
- `cargo test -p polint --test public_surface_leak --locked` — passed
- `cargo test -p polint --lib --locked` — passed, 2302 tests
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` — passed
- `cargo fmt --all --check` — passed
- `cargo doc -p polint --no-deps --locked` — passed
- `cargo run -p polint --locked -- facts list --format json` — passed
- `git diff --check` — passed

## Requirement Traceability

- `CTRL-01`: `ControlFlow<'_>::missing_guard` reports call events missing an earlier same-function guard call matched by `GuardPattern::call_any`.
- `CTRL-02`: `ControlFlow<'_>::missing_cleanup` reports call start/acquire events missing a later same-function cleanup call.
- `CTRL-03`: Rule authors use one typed API and do not see dominance, postdominance, CFG node, MIR operation, or call graph IDs. Bounded interprocedural behavior remains deferred behind the existing `max_depth` shape.
- `CTRL-04`: Diagnostics include event spans, target, function, required guard or cleanup, same-function uncovered path, call status, call precision, confidence when available, conservative policy status/precision, and budget evidence when truncated.

## Residual Limits

- `EventPattern::write_field` has no backed control-flow results yet.
- Phase 57 does not prove exact cleanup on every normal/error exit.
- Phase 57 does not pair cleanup to resource identity.
- `max_depth > 1` does not enable interprocedural search yet.
- `DataFlow<'_>` remains fail-closed for Phase 58.

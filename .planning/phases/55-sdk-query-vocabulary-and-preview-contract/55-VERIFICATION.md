---
phase: 55-sdk-query-vocabulary-and-preview-contract
verified: 2026-06-20
status: passed
---

# Phase 55 Verification

Status: PASS
Date: 2026-06-20

## Goal

Establish the Phase 55 public preview vocabulary and constraints for v1.4 policy queries before implementing query behavior. Rule authors should get one clear API shape: typed preview views, plain query objects, typed patterns, macro-derived capabilities, honest fail-closed diagnostics, and no public raw graph traversal.

## Result

Phase 55 is implemented.

- `Events<'_>`, `Calls<'_>`, `ControlFlow<'_>`, and policy-level `DataFlow<'_>` are exported through `polint::sdk::prelude::*`.
- `ReachQuery`, `GuardQuery`, `LifecycleQuery`, `FlowQuery`, `EventPattern`, `SourcePattern`, `SinkPattern`, `GuardPattern`, `BarrierPattern`, `PolicyViolation`, `PolicyStatus`, and `PolicyPrecision` are public preview SDK vocabulary.
- Macro-derived capabilities, rule manifests, analysis-plan support rows, and `polint facts list` understand `events`, `calls`, `control_flow`, and preview `dataflow`.
- Unsupported preview capabilities fail closed through `polint/capability`; external temp-repo tests prove the rule body does not execute with placeholder facts.
- `Cfg<'_>` and `CallGraph<'_>` remain reserved raw capabilities, not aliases for `ControlFlow<'_>` or `Calls<'_>`.
- Public docs and agent skill guidance describe preview status honestly and defer full query-result behavior to Phases 56-59.
- Public-surface leak gates were updated deliberately for the new preview names and still block private analysis namespaces.

## Requirement Verification

| Requirement | Status | Evidence |
|-------------|--------|----------|
| API-01 | PASS | SDK prelude exports and external temp-repo rule signatures request all four preview views. |
| API-02 | PASS | Docs and tests use `Query::new(required...)`, explicit option fields, one view method, and `PolicyViolation::diagnostic(...)`; no alternate DSL was added. |
| API-03 | PASS | `ReachQuery`, `GuardQuery`, `LifecycleQuery`, and `FlowQuery` exist with deterministic defaults and documented option fields. |
| API-04 | PASS | Pattern structs exist and are documented for exact/list matching; broader behavior remains deferred and honest. |
| API-05 | PASS | Macro capability mapping, manifests, `facts list`, and fail-closed capability diagnostics cover the preview views. |
| API-06 | PASS | Reserved raw `Cfg<'_>` and `CallGraph<'_>` tests remain unsupported; public docs position `ControlFlow<'_>` and `Calls<'_>` as policy-level views. |

## Verification Commands

Passed:

- `cargo fmt --all --check`
- `cargo check -p polint --locked`
- `cargo test -p polint --lib sdk_prelude_exports_rule_authoring_surface --locked`
- `cargo doc -p polint --no-deps --locked`
- `cargo test -p polint-macros capability_for_type_maps_supported_fact_views --locked`
- `cargo test -p polint --lib preview_policy_capabilities_remain_fail_closed --locked`
- `cargo test -p polint --lib policy_preview_capabilities_have_distinct_names --locked`
- `cargo test -p polint --lib facts_list_reports_phase55_preview_capabilities --locked`
- `cargo run -p polint --locked -- facts list --format json`
- `cargo test -p polint --test cli phase55_preview_rule_syntax_compiles_and_fails_closed --locked`
- `cargo test -p polint --test cli reserved_cfg_and_call_graph_remain_unsupported --locked`
- `cargo test -p polint --test cli facts_list_json_is_stable_and_public_only --locked`
- `cargo test -p polint --test public_surface_leak --locked`
- `cargo test -p polint-macros --locked`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `git diff --check`

## Scope Boundaries

Phase 55 proves compile, manifest, capability, diagnostics, docs, and public-boundary behavior only. It does not implement provider-backed query results. `Events<'_>` and `Calls<'_>` behavior is Phase 56, `ControlFlow<'_>` behavior is Phase 57, `DataFlow<'_>` behavior is Phase 58, and shared evidence/cache/unknown semantics are Phase 59.

## Manual Tracking Note

`gsd-sdk` was not available on PATH in this workspace during closeout, so roadmap, requirements, and state updates were applied manually with the same intended GSD phase-completion semantics.

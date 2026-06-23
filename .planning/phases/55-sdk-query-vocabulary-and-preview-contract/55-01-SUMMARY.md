---
phase: 55-sdk-query-vocabulary-and-preview-contract
plan: 01
subsystem: sdk
tags: [sdk, policy-query, preview-api]
key-files:
  created:
    - crates/polint/src/sdk/policy.rs
  modified:
    - crates/polint/src/sdk/facts.rs
    - crates/polint/src/sdk/mod.rs
requirements-completed: [API-01, API-02, API-03, API-04, API-06]
duration: 20 min
completed: 2026-06-20
---

# Phase 55 Plan 01: Preview SDK Vocabulary And Query Types Summary

Preview policy-query SDK vocabulary with explicit query structs, pattern constructors, prelude exports, and fail-closed view method signatures.

## Commits

| Task | Commit | Notes |
|------|--------|-------|
| 1-3 | 44e8f603 | Added `sdk::policy`, preview `Events`, `Calls`, `ControlFlow`, policy-level `DataFlow` methods, and explicit prelude exports. |

## Verification

- `cargo fmt --all --check` PASS
- `cargo check -p polint --locked` PASS
- `cargo test -p polint --lib sdk_prelude_exports_rule_authoring_surface --locked` PASS
- `cargo doc -p polint --no-deps --locked` PASS
- `rg -n "pub struct (Events|Calls|ControlFlow|DataFlow|Cfg|CallGraph)" crates/polint/src/sdk/facts.rs` PASS
- `rg -n "pub struct (ReachQuery|GuardQuery|LifecycleQuery|FlowQuery|EventPattern|SourcePattern|SinkPattern|GuardPattern|BarrierPattern)" crates/polint/src/sdk/policy.rs` PASS

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- Initial GSD commit attempt reported a truncated hook failure while `make lint` was still in progress. Running `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` directly passed, and retrying the GSD commit succeeded.

## Next Phase Readiness

Plan 55-02 can wire the new preview view names into macro-derived capabilities, capability support rows, manifests, and `facts list` metadata. The preview query methods intentionally panic if reached because rules requesting these unsupported capabilities must fail closed before execution.

## Self-Check: PASSED


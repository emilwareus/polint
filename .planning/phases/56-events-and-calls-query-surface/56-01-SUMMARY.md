---
phase: 56-events-and-calls-query-surface
plan: 01
subsystem: sdk
tags: [policy-query, events, calls, sdk]
requires:
  - phase: 55-sdk-query-vocabulary-and-preview-contract
    provides: preview policy views and query vocabulary
provides:
  - Provider-backed `Events<'_>::matching(EventPattern::call(...))`
  - Internal policy-query module with call-event projection helpers
affects: [phase-57, phase-58, phase-59, rule-authoring]
tech-stack:
  added: []
  patterns: [private query implementation behind public typed SDK views]
key-files:
  created:
    - crates/polint/src/policy_queries.rs
  modified:
    - crates/polint/src/sdk/facts.rs
    - crates/polint/src/sdk/policy.rs
    - crates/polint/src/lib.rs
key-decisions:
  - "Keep public SDK views thin; move provider-backed query logic into a private `policy_queries` module."
  - "Support call events in Phase 56 and leave non-call event families as deterministic preview no-results."
patterns-established:
  - "Public policy views delegate to private query functions instead of importing provider internals into `crates/polint/src/sdk/facts.rs`."
requirements-completed: [CALL-01, CALL-04]
duration: recorded
completed: 2026-06-20
---

# Phase 56 Plan 01 Summary

**Provider-backed call-event matching through `Events<'_>` without exposing raw analysis internals**

## Accomplishments

- Added private pattern inspection and violation-construction helpers for policy queries.
- Implemented `Events::matching(EventPattern::call(...))` over refined-call edges, with direct call-site fallback.
- Kept `EventPattern::write_field` as preview vocabulary returning no backed matches until write-event facts are promoted.
- Moved internal call/refined-call/MIR fixture tests out of `crates/polint/src/sdk/facts.rs` so public-surface leak gates remain meaningful.

## Verification

- `cargo test -p polint --lib events_matching_call_returns_provider_backed_violation --locked`
- Covered again by `cargo test -p polint --lib --locked`

## Deviations

- The plan allowed helpers in `sdk::facts`; implementation moved them to `crate::policy_queries` to keep SDK source free of private provider marker types.

## Next Phase Readiness

`Events<'_>` now supplies the shared event pattern behavior needed by control-flow and data-flow policy queries, while raw `Cfg<'_>` and `CallGraph<'_>` remain reserved.

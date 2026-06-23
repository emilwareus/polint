---
phase: 57-control-flow-guard-and-lifecycle-queries
plan: 01
subsystem: sdk
tags: [policy-query, control-flow, guards, sdk]
requires:
  - phase: 56-events-and-calls-query-surface
    provides: call event and refined-call query helpers
provides:
  - Provider-backed `ControlFlow<'_>::missing_guard(GuardQuery)`
  - Internal ordered call-event projection for control-flow policy queries
affects: [phase-58, phase-59, templates, rule-authoring]
tech-stack:
  added: []
  patterns: [private query implementation behind public typed SDK views]
key-files:
  created: []
  modified:
    - crates/polint/src/policy_queries.rs
    - crates/polint/src/sdk/facts.rs
    - crates/polint/src/sdk/policy.rs
key-decisions:
  - "Keep `ControlFlow<'_>` as the one public view; implement query behavior in private `policy_queries`."
  - "Support same-function call-event guard checks first; leave write-field events and interprocedural proof deferred."
  - "Return heuristic/conservative policy results because the shipped proof is same-function ordering, not exact path coverage."
patterns-established:
  - "Control-flow policy queries consume private call/refined-call facts and CFG operation order without exposing raw CFG or MIR IDs."
requirements-completed: [CTRL-01, CTRL-03, CTRL-04]
duration: recorded
completed: 2026-06-20
---

# Phase 57 Plan 01 Summary

**Same-function guard query support through `ControlFlow<'_>`**

## Accomplishments

- Added private `GuardPattern` accessors for internal query interpretation without changing the public API.
- Replaced the `ControlFlow::missing_guard` preview panic with a thin SDK wrapper into `crate::policy_queries`.
- Built an ordered call-event projection from refined calls joined to call sites, using CFG operation order when available and source-span order as a deterministic fallback.
- Implemented missing-guard detection for `EventPattern::call(...)` plus `GuardPattern::call_any([...])`.
- Added unit coverage for missing guard, satisfied guard, write-field no-result behavior, and budget truncation evidence.

## Verification

- `cargo test -p polint --lib control_flow_missing_guard --locked`
- Covered again by `cargo test -p polint --lib --locked`

## Deviations

- The roadmap originally referenced sensitive writes and dominance/postdominance. Phase 57 intentionally shipped call-event same-function semantics only; write-event facts and exact graph proof remain deferred.

## Next Phase Readiness

Phase 58 can reuse the private policy-query module and evidence style while adding `DataFlow<'_>` behavior behind the same query-object model.


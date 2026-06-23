---
phase: 56-events-and-calls-query-surface
plan: 03
subsystem: sdk
tags: [capabilities, docs, external-tests, public-surface]
requires:
  - phase: 56-events-and-calls-query-surface
    provides: event and call query behavior
provides:
  - Supported `events` and `calls` capabilities
  - External temp-repo CLI test for public rule-authoring usage
  - Public docs for Phase 56 event/call query limits
affects: [phase-57, phase-58, phase-61, phase-62]
tech-stack:
  added: []
  patterns: [external temp-repo SDK validation]
key-files:
  created: []
  modified:
    - crates/polint/src/analysis_plan.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/tests/cli.rs
    - docs/facts/events.md
    - docs/facts/calls.md
    - .agents/skills/polint/SKILL.md
    - .claude/skills/polint/SKILL.md
key-decisions:
  - "Mark `events` and `calls` supported only after provider-backed behavior exists."
  - "Keep `control_flow`, `dataflow`, raw `cfg`, and raw `call_graph` blocked or reserved until their later phases."
patterns-established:
  - "Capability support tests assert current phase boundaries instead of broad preview blocking behavior."
requirements-completed: [CALL-01, CALL-02, CALL-03, CALL-04]
duration: recorded
completed: 2026-06-20
---

# Phase 56 Plan 03 Summary

**Events and calls promoted as supported preview capabilities with docs and external-rule proof**

## Accomplishments

- Marked `events` and `calls` as supported capabilities and semantic-pipeline triggers.
- Updated the Phase 55 preview regression so later-phase views still block execution while event/call rules execute.
- Added a temp-repo CLI test where generated `.polint/rules` imports only `polint::sdk::prelude::*` and emits JSON diagnostics through `Events` and `Calls`.
- Updated docs, public-surface allowlist, and polint skill text to describe the provider-backed Phase 56 surface honestly.

## Verification

- `cargo test -p polint --lib policy_capabilities_report_phase_support_boundaries --locked`
- `cargo test -p polint --test cli phase56_events_and_calls_rule_reports_json --locked`
- Phase 55 preview blocking regression — passed
- `cargo test -p polint --test public_surface_leak --locked`
- `cargo run -p polint --locked -- facts list --format json`

## Deviations

- Updated one public-boundary guard to stop treating the generic word `confidence` as forbidden, because `PolicyConfidence` is now intentional public API.

## Next Phase Readiness

Phase 57 can request `Events<'_>` as a supported dependency while adding `ControlFlow<'_>` behavior behind the same typed query style.

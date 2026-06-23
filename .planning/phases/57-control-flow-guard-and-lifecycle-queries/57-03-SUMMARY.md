---
phase: 57-control-flow-guard-and-lifecycle-queries
plan: 03
subsystem: sdk
tags: [capabilities, docs, external-tests, public-surface]
requires:
  - phase: 57-control-flow-guard-and-lifecycle-queries
    provides: guard and lifecycle query behavior
provides:
  - Supported `control_flow` capability
  - External temp-repo CLI test for `ControlFlow<'_>` public usage
  - Public docs for Phase 57 control-flow query limits
affects: [phase-58, phase-61, phase-62]
tech-stack:
  added: []
  patterns: [external temp-repo SDK validation]
key-files:
  created: []
  modified:
    - crates/polint/src/analysis_plan.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/tests/cli.rs
    - docs/facts/control-flow.md
    - docs/facts/capability-plans.md
    - docs/API-VISIBILITY-PLAN.md
    - .agents/skills/polint/SKILL.md
    - .claude/skills/polint/SKILL.md
key-decisions:
  - "Mark `control_flow` supported only after provider-backed same-function behavior and external-rule proof exist."
  - "Keep `dataflow`, raw `cfg`, and raw `call_graph` reserved until their later phases."
  - "Update milestone requirements to match shipped same-function call-event semantics instead of overclaiming the original broader wording."
patterns-established:
  - "Capability support tests track the current phase boundary: `events`, `calls`, and `control_flow` supported; `dataflow` unsupported."
requirements-completed: [CTRL-01, CTRL-02, CTRL-03, CTRL-04]
duration: recorded
completed: 2026-06-20
---

# Phase 57 Plan 03 Summary

**Control-flow promoted as a supported preview capability with docs and external-rule proof**

## Accomplishments

- Marked `control_flow` as a supported capability and semantic-pipeline trigger.
- Updated the Phase 55 preview regression so `dataflow` still produces capability diagnostics while `events`, `calls`, and `control_flow` do not.
- Added a temp-repo CLI test where generated `.polint/rules` imports only `polint::sdk::prelude::*` and emits guard plus cleanup diagnostics through `ControlFlow<'_>`.
- Updated public docs, API visibility guidance, and agent skill text to state the provider-backed Phase 57 surface and residual limits.
- Updated roadmap, requirements, and state to close Phase 57 and make Phase 58 the next autonomous target.

## Verification

- `cargo test -p polint --lib policy_capabilities_report_phase_support_boundaries --locked`
- Phase 55 preview blocking regression — passed
- `cargo test -p polint --test cli phase57_control_flow_rule_reports_json --locked`
- `cargo test -p polint --test public_surface_leak --locked`
- `cargo run -p polint --locked -- facts list --format json`

## Deviations

- The original success criteria referenced bounded interprocedural mode and every-exit cleanup. Requirements and roadmap wording were narrowed to the shipped same-function call-event scope instead of claiming behavior that is still reserved.

## Next Phase Readiness

Phase 58 starts with a consistent policy-query support matrix: `Events<'_>`, `Calls<'_>`, and `ControlFlow<'_>` are provider-backed preview views; `DataFlow<'_>` remains reserved until source/sink/barrier behavior lands.

---
phase: 37-refined-call-graph-providers
plan: 05
subsystem: static-analysis
tags: [rust, ts-js, refined-calls, extensions]
requires:
  - phase: 37-02
    provides: polint.refined_calls provider and type/value/alias inputs
provides:
  - TS/JS callable type and function-token refined call candidates
  - TS/JS bounded points-to-assisted refined call candidates
  - Validated extension/model refined call edge candidates
affects: [refined-calls, ts-js-analysis, extension-validation, SAE-PREC-02]
tech-stack:
  added: []
  patterns: [callable fact binding, extension payload validation, precision ceilings]
key-files:
  created: []
  modified:
    - crates/polint/src/analysis/refined_calls/ts_js.rs
    - crates/polint/src/analysis/refined_calls/extensions.rs
    - crates/polint/src/analysis/extensions/sinks.rs
    - crates/polint/src/analysis/extensions/validate.rs
key-decisions:
  - "TS/JS refinements require existing call targets and present callable type/value or within-budget points-to evidence."
  - "Extension refined-call rows must pass payload validation and bind native target ids before entering the refined-call store."
  - "Extension precision is capped so generated or heuristic extension facts cannot become exact refined call edges."
patterns-established:
  - "Extension-backed refined-call payloads use refined_calls.edge with explicit site, target, algorithm, and status labels."
requirements-completed: [SAE-PREC-02]
duration: 45min
completed: 2026-05-25
---

# Phase 37 Plan 05: TS/JS Function-Token, Points-To, And Extension Model Refinements Summary

**TS/JS and validated extension/model evidence now participate in refined call candidates**

## Performance

- **Duration:** 45 min
- **Started:** 2026-05-25T01:10:00Z
- **Completed:** 2026-05-25T01:55:00Z
- **Tasks:** 4
- **Files modified:** 4

## Accomplishments

- Added TS/JS callable type and function-value refinement over existing call targets using the function-token tier.
- Added TS/JS within-budget points-to-assisted refinement and explicit budget-exceeded rows.
- Added `refined_calls.edge` extension payload validation for site, target, algorithm, and status labels.
- Added extension/model refined-call translation with extension/provider evidence, precision ceilings, and dangling target-id rejection.
- Added focused tests for callable values, callable types, dynamic unresolved calls, points-to budget behavior, extension evidence, extension precision ceilings, and malformed refined-call payloads.

## Task Commits

1. **Tasks 1-4: TS/JS and extension/model refined-call providers** - `572aa6a` (feat)

## Files Created/Modified

- `crates/polint/src/analysis/refined_calls/ts_js.rs` - TS/JS callable type/value/points-to refinement pass and tests.
- `crates/polint/src/analysis/refined_calls/extensions.rs` - accepted extension refined-edge translation and tests.
- `crates/polint/src/analysis/extensions/sinks.rs` - refined-call extension fact family and payload validation.
- `crates/polint/src/analysis/extensions/validate.rs` - refined-call payload rejection path.

## Decisions Made

Extension facts are treated as validated external evidence, not native truth. Native IDs must bind to existing functions or symbols, synthetic targets must be explicit, and precision is capped at setup-aware or lower.

## Deviations from Plan

None - implementation stayed within the internal provider and extension-validation scope.

## Issues Encountered

The first GSD commit attempt encountered a stale Git worktree lock. The lock was gone on inspection, and the commit succeeded on retry.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for Plan 37-06 provider verification, reporting, and phase closure.

---
*Phase: 37-refined-call-graph-providers*
*Completed: 2026-05-25*

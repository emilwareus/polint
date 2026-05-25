---
phase: 37-refined-call-graph-providers
plan: 04
subsystem: static-analysis
tags: [rust, go, refined-calls, points-to]
requires:
  - phase: 37-02
    provides: polint.refined_calls provider and type/value/alias inputs
provides:
  - Go receiver type refined call candidates
  - Go points-to-assisted refined call candidates
  - Explicit Go setup-missing and budget-exceeded refined rows
affects: [refined-calls, go-analysis, SAE-PREC-02]
tech-stack:
  added: []
  patterns: [receiver-place type binding, bounded points-to refinement]
key-files:
  created: []
  modified:
    - crates/polint/src/analysis/refined_calls/go.rs
key-decisions:
  - "Go type-assisted edges require an existing direct call target for the call site and a present receiver type fact."
  - "Go points-to-assisted edges require a present within-budget points-to set; budget-exceeded inputs produce explicit budget rows for unresolved dispatch."
patterns-established:
  - "Go refined-call algorithms use TypeHierarchy and PointsTo only when corresponding type or points-to evidence is present."
requirements-completed: [SAE-PREC-02]
duration: 35min
completed: 2026-05-25
---

# Phase 37 Plan 04: Go Receiver And Type-Aware Refinements Summary

**Go receiver type and bounded points-to evidence now create explicit refined call candidates**

## Performance

- **Duration:** 35 min
- **Started:** 2026-05-25T00:35:00Z
- **Completed:** 2026-05-25T01:10:00Z
- **Tasks:** 3
- **Files modified:** 1

## Accomplishments

- Added Go receiver type refinement over existing call targets using `TypeHierarchy` and `TypeValueFunctionToken` tier.
- Added Go points-to-assisted refinement over present within-budget receiver points-to sets.
- Added explicit setup-missing and budget-exceeded refined rows for unresolved Go interface/function-value dispatch.
- Added focused tests for receiver type, setup-missing interface dispatch, within-budget points-to, and budget-exceeded points-to behavior.

## Task Commits

1. **Tasks 1-3: Go receiver and points-to refinements** - `3670078` (feat)

## Files Created/Modified

- `crates/polint/src/analysis/refined_calls/go.rs` - Go receiver/type/points-to refinement pass and tests.

## Decisions Made

The Go pass remains conservative: it narrows existing call targets when supporting type or points-to facts exist, and it reports uncertainty for unresolved interface/function-value dispatch rather than inventing exact edges.

## Deviations from Plan

None - plan executed within the intended internal provider scope.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for Plan 37-05 TS/JS function-token, points-to, and extension/model refinements.

---
*Phase: 37-refined-call-graph-providers*
*Completed: 2026-05-25*

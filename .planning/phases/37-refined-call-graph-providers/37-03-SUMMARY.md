---
phase: 37-refined-call-graph-providers
plan: 03
subsystem: static-analysis
tags: [rust, refined-calls, framework-dispatch, summaries]
requires:
  - phase: 37-02
    provides: polint.refined_calls provider and private refined edge store
provides:
  - Framework dispatch refined call edges
  - Explicit unresolved framework refined rows
  - Summary-assisted refined call candidates over existing direct call targets
affects: [refined-calls, analysis-kernel, SAE-PREC-02]
tech-stack:
  added: []
  patterns: [evidence-backed synthetic framework edges, bind-only summary projection]
key-files:
  created: []
  modified:
    - crates/polint/src/analysis/refined_calls/framework.rs
    - crates/polint/src/analysis/refined_calls/summaries.rs
    - crates/polint/src/analysis/refined_calls/provider.rs
    - crates/polint/src/analysis/refined_calls/validate.rs
key-decisions:
  - "Framework dispatch refinements are synthetic DirectPlusFramework edges with model provenance and non-exact precision."
  - "Summary-assisted refinements only project over existing direct call targets for the same function; unbound summaries emit no guessed edges."
patterns-established:
  - "Provider output is finalized by sorting and reassigning dense refined-call IDs after merging independent refinement sources."
  - "Synthetic framework rows may carry a synthetic site only when evidence marks them as framework dispatch or unresolved framework rows."
requirements-completed: [SAE-PREC-02]
duration: 35min
completed: 2026-05-25
---

# Phase 37 Plan 03: Framework Dispatch And Summary-Assisted Refinements Summary

**Framework dispatch and bind-only summary hints now produce private refined call edges**

## Performance

- **Duration:** 35 min
- **Started:** 2026-05-25T00:00:00Z
- **Completed:** 2026-05-25T00:35:00Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Added framework dispatch projection from `FrameworkDispatchEdgeFact` to `DirectPlusFramework` refined edges.
- Added unresolved framework projection with explicit unresolved/setup-missing/unsupported/budget-exceeded statuses.
- Added summary-assisted projection from present call-effect summaries to existing direct call targets.
- Added provider finalization that sorts merged outputs and reassigns dense IDs to avoid collisions across refinement passes.

## Task Commits

1. **Tasks 1-3: Framework and summary refinements** - `13e0b08` (feat)

## Files Created/Modified

- `crates/polint/src/analysis/refined_calls/framework.rs` - Framework dispatch and unresolved framework projections.
- `crates/polint/src/analysis/refined_calls/summaries.rs` - Summary-assisted projections over existing call targets.
- `crates/polint/src/analysis/refined_calls/provider.rs` - Deterministic merged-output finalization.
- `crates/polint/src/analysis/refined_calls/validate.rs` - Synthetic framework site validation rule.

## Decisions Made

Framework dispatch rows use `CallAlgorithm::FrameworkModel` and `CallProvenance::Model` because the existing call vocabulary has no framework-specific provenance variant. Summary-assisted rows use existing direct targets as their binding anchor to avoid broad or optimistic graph expansion.

## Deviations from Plan

Synthetic framework dispatch facts do not always correspond to a concrete `CallSiteId`. Validation now allows a synthetic site only for base-target-free synthetic edges whose evidence marks them as framework dispatch or unresolved framework rows.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for Plan 37-04 Go receiver and type-aware refinements.

---
*Phase: 37-refined-call-graph-providers*
*Completed: 2026-05-25*

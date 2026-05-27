---
phase: 37-refined-call-graph-providers
plan: 01
subsystem: static-analysis
tags: [rust, calls, metadata, store]
requires:
  - phase: 28
    provides: semantic MIR, call-site substrate, and metadata conventions
provides:
  - Private refined call edge fact vocabulary
  - Normalized refined call output and store indexes
  - RefinedCallEdge metadata family
affects: [refined-calls, analysis-kernel, SAE-PREC-02]
tech-stack:
  added: []
  patterns: [crate-private fact layer, normalized provider output, metadata-backed internal facts]
key-files:
  created:
    - crates/polint/src/analysis/refined_calls/facts.rs
    - crates/polint/src/analysis/refined_calls/store.rs
    - crates/polint/src/analysis/refined_calls/mod.rs
  modified:
    - crates/polint/src/analysis/ids.rs
    - crates/polint/src/analysis/mod.rs
    - crates/polint/src/analysis_kernel/metadata.rs
    - crates/polint/src/core/mod.rs
key-decisions:
  - "Keep refined calls crate-private and represent them as a layer over existing call targets."
  - "Reuse existing call status, precision, provenance, algorithm, and reason vocabulary to avoid parallel public semantics."
patterns-established:
  - "Refined call indexes are rebuildable views over normalized edge facts."
  - "Refined call metadata uses FactFamily::RefinedCallEdge with polint.refined_calls as producer."
requirements-completed: [SAE-PREC-02]
duration: 50min
completed: 2026-05-24
---

# Phase 37 Plan 01: Private Refined Call Fact Contracts And Store Summary

**Private refined call edge facts with deterministic storage, metadata, and internal tier indexes**

## Performance

- **Duration:** 50 min
- **Started:** 2026-05-24T21:12:00Z
- **Completed:** 2026-05-24T22:02:33Z
- **Tasks:** 4
- **Files modified:** 7

## Accomplishments

- Added `RefinedCallEdgeId` and registered `analysis::refined_calls` as a crate-private module.
- Added `RefinedCallEdgeFact` with preserved `CallSiteId`, optional base `CallTargetId`, target fields, tier/status/precision/provenance, validation, confidence, evidence, inputs, and stable key.
- Added `RefinedCallOutput` and `RefinedCallStore` with deterministic normalization and indexes by site, caller, target function/symbol, status, algorithm, provenance, and tier.
- Added `FactFamily::RefinedCallEdge` and metadata refresh/missing-metadata support in `AnalysisDb`.

## Task Commits

1. **Tasks 1-4: Private refined call contracts and metadata** - `dfff859` (feat)

## Files Created/Modified

- `crates/polint/src/analysis/refined_calls/facts.rs` - Private refined edge fact and tier/validation/confidence vocabulary.
- `crates/polint/src/analysis/refined_calls/store.rs` - Normalized output container and query indexes.
- `crates/polint/src/analysis/refined_calls/mod.rs` - Crate-private module registration for refined-call internals.
- `crates/polint/src/analysis/ids.rs` - `RefinedCallEdgeId`.
- `crates/polint/src/analysis_kernel/metadata.rs` - `RefinedCallEdge` fact family.
- `crates/polint/src/core/mod.rs` - Refined-call storage, metadata, and accessors.

## Decisions Made

Reused existing call enums rather than creating duplicate refined-call status/provenance enums. This keeps refined calls truthfully tied to the base call layer and avoids creating a second semantic vocabulary before public SDK exposure exists.

## Deviations from Plan

The store rejects duplicate stable keys and duplicate IDs directly. Dangling base-target validation lives in `analysis/refined_calls/validate.rs`, where it can compare refined edges against the owning `AnalysisDb` call target set.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for Plan 37-03. Plan 37-02 provider wiring was completed in the same production commit and has its own summary.

---
*Phase: 37-refined-call-graph-providers*
*Completed: 2026-05-24*

---
phase: 32-summary-kernel-and-direct-summaries
plan: 02
subsystem: analysis
tags: [summary-store, summary-output, fact-family, analysis-db, metadata]

requires:
  - phase: 32-summary-kernel-and-direct-summaries
    plan: 01
    provides: "SummaryDomain trait, four core domain types, fact vocabulary enums, SummaryFact and SummaryEventFact structs"
  - phase: 31-p0-abstract-domain-kernel
    provides: "DomainStore/DomainOutput pattern, FactFamily enum, AnalysisDb metadata refresh pattern"
provides:
  - "SummaryOutput with deterministic normalized() sorting by stable_key and sequential ID reassignment"
  - "SummaryStore with from_output constructor and BTreeMap indexes by callable, domain, and function"
  - "AnalysisDb replace_summary_facts, summary_facts, summary_events, summary_store accessors"
  - "FactFamily variants SummaryControl, SummaryCall, SummaryMemory, SummaryTito, SummaryEvent"
  - "Summary metadata refresh with polint.direct_summaries producer_id"
  - "SummaryPrecision to FactPrecision mapping"
affects: [32-summary-kernel-and-direct-summaries, 33-demand-queries-and-summary-scc-cache]

tech-stack:
  added: []
  patterns: ["SummaryOutput/SummaryStore follows CallOutput/CallStore pattern from D-03", "Summary metadata refresh follows domain metadata refresh pattern"]

key-files:
  created:
    - "crates/polint/src/analysis/summaries/store.rs"
  modified:
    - "crates/polint/src/analysis/summaries/mod.rs"
    - "crates/polint/src/analysis_kernel/metadata.rs"
    - "crates/polint/src/core/mod.rs"

key-decisions:
  - "SummaryOutput normalized() sorts by stable_key then id, matching the CallOutput pattern"
  - "SummaryStore::from_output returns Result<Self, AnalysisError> for consistency with CallStore"
  - "Each summary domain maps to a separate FactFamily variant for independent metadata tracking"
  - "SummaryPrecision::Local and SetupAware both map to FactPrecision::SetupAware since summary facts are never Exact"

patterns-established:
  - "SummaryOutput/SummaryStore: normalize then index pattern matching CallOutput/CallStore"
  - "Summary domain to FactFamily mapping via summary_domain_to_fact_family helper"
  - "Summary metadata refresh removes five families then re-inserts with polint.direct_summaries producer"

requirements-completed: []

duration: 5min
completed: 2026-05-21
---

# Phase 32 Plan 02: Summary Store and AnalysisDb Integration Summary

**SummaryOutput normalization with SummaryStore indexed accessors, FactFamily summary variants, and AnalysisDb storage with metadata refresh under polint.direct_summaries**

## Performance

- **Duration:** 5 min
- **Started:** 2026-05-21T18:43:33Z
- **Completed:** 2026-05-21T18:48:33Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- SummaryOutput with deterministic normalized() sorting and sequential ID reassignment
- SummaryStore with callable/domain/function BTreeMap indexes and accessor methods
- FactFamily extended with five summary variants (SummaryControl, SummaryCall, SummaryMemory, SummaryTito, SummaryEvent)
- AnalysisDb stores and retrieves summary facts with full metadata refresh under polint.direct_summaries producer

## Task Commits

Each task was committed atomically:

1. **Task 1: Add SummaryOutput, SummaryStore with deterministic normalization and indexed accessors** - `946a776` (feat)
2. **Task 2: Extend FactFamily, add AnalysisDb summary storage, accessors, and metadata refresh** - `9f12844` (feat)

## Files Created/Modified
- `crates/polint/src/analysis/summaries/store.rs` - SummaryOutput, SummaryStore with normalization, indexes, accessors, and 7 unit tests
- `crates/polint/src/analysis/summaries/mod.rs` - Added pub(crate) mod store declaration
- `crates/polint/src/analysis_kernel/metadata.rs` - FactFamily extended with SummaryControl, SummaryCall, SummaryMemory, SummaryTito, SummaryEvent and label() arms
- `crates/polint/src/core/mod.rs` - AnalysisDb summary fields, replace_summary_facts, summary accessors, metadata refresh, precision mapping helpers

## Decisions Made
- SummaryOutput normalized() sorts by (stable_key, id) then reassigns IDs sequentially, matching the established CallOutput pattern from D-03
- SummaryStore::from_output returns Result for API consistency with CallStore even though current validation is minimal
- Each SummaryDomainKind maps to a distinct FactFamily variant so metadata can be removed/refreshed per domain independently
- SummaryPrecision::Local and SetupAware both map to FactPrecision::SetupAware because summary facts are setup-aware internal rows, never Exact

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- SummaryOutput and SummaryStore are ready for the summary builder to produce and the provider to store
- AnalysisDb can accept summary facts with full metadata tracking
- FactFamily variants enable validation, debug, and eval to distinguish summary fact categories
- Ready for Plan 32-03 (summary builder) and subsequent provider/validation/debug/eval plans

---
*Phase: 32-summary-kernel-and-direct-summaries*
*Completed: 2026-05-21*

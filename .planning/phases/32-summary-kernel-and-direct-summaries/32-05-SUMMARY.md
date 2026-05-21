---
phase: 32-summary-kernel-and-direct-summaries
plan: 05
subsystem: analysis
tags: [summary-validation, debug-json, precision-ceiling, referential-integrity, deterministic-snapshots]

requires:
  - phase: 32-summary-kernel-and-direct-summaries
    plan: 04
    provides: "Summary provider wired into kernel with parameter digest, output digest, and provider order"
  - phase: 32-summary-kernel-and-direct-summaries
    plan: 02
    provides: "SummaryOutput, SummaryStore, AnalysisDb summary storage and metadata refresh"
  - phase: 31-p0-abstract-domain-kernel
    provides: "Domain validation pattern (domains/validate.rs) used as template"
provides:
  - "validate_summaries function checking dangling function refs, duplicate stable keys, empty callable keys, precision ceiling"
  - "SummaryDebugReport with compact rows, event rows, status counts, domain counts"
  - "Summary validation wired into kernel validation sequence after validate_abstract_domains"
affects: [32-summary-kernel-and-direct-summaries, 33-demand-queries-and-summary-scc-cache]

tech-stack:
  added: []
  patterns: ["Summary validation follows domains/validate.rs pattern with push_summary_diagnostic helper", "Summary debug rows use as_str labels only, no dense IDs, absolute paths, or timestamps"]

key-files:
  created:
    - "crates/polint/src/analysis/summaries/validate.rs"
  modified:
    - "crates/polint/src/analysis/summaries/mod.rs"
    - "crates/polint/src/analysis_kernel/validation.rs"
    - "crates/polint/src/analysis_kernel/debug.rs"

key-decisions:
  - "Summary validation runs after validate_abstract_domains in the kernel validation sequence"
  - "Precision ceiling check rejects FactPrecision::Exact from polint.direct_summaries metadata rows"
  - "Debug rows use domain.as_str(), status.as_str(), precision.as_str(), provenance.as_str() labels"

patterns-established:
  - "Summary validation pattern: collect function ID set, check duplicate keys per domain family, validate each fact and event, enforce precision ceiling"
  - "Summary debug report pattern: compact rows sorted by stable_key, SummaryCounts with status breakdown, domain_counts BTreeMap"

requirements-completed: [SAE-INT-02]

duration: 9min
completed: 2026-05-21
---

# Phase 32 Plan 05: Summary Validation and Debug JSON Summary

**Summary validation catches dangling function references, duplicate stable keys, and precision ceiling violations; debug JSON provides compact deterministic summary snapshots with status and domain counts**

## Performance

- **Duration:** 9 min
- **Started:** 2026-05-21T19:14:36Z
- **Completed:** 2026-05-21T19:23:17Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Created validate_summaries function checking dangling function references, duplicate stable keys per domain family, empty callable keys and payload digests, and precision ceiling enforcement (Exact rejected for polint.direct_summaries)
- Wired validate_summaries into kernel validation sequence after validate_abstract_domains
- Extended MetadataDebugReport with SummaryDebugReport containing compact rows, event rows, SummaryCounts by status, and domain_counts BTreeMap
- Added 6 tests total: 4 validation tests (dangling refs, duplicate keys, precision ceiling, valid facts) and 2 debug JSON tests (rows/counts/domain_counts, no dense IDs/absolute paths/timestamps)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add summary validation for referential integrity, duplicate keys, precision ceilings, and malformed payloads** - `bb3f5a0` (feat)
2. **Task 2: Add summary debug report section with compact deterministic rows** - `d4196a5` (feat)

## Files Created/Modified
- `crates/polint/src/analysis/summaries/validate.rs` - validate_summaries function with duplicate-key, dangling-ref, empty-field, and precision-ceiling checks plus 4 unit tests
- `crates/polint/src/analysis/summaries/mod.rs` - Added validate module declaration
- `crates/polint/src/analysis_kernel/validation.rs` - Wired validate_summaries call after validate_abstract_domains
- `crates/polint/src/analysis_kernel/debug.rs` - Added SummaryDebugReport, SummaryDebugRow, SummaryEventDebugRow, SummaryCounts structs; summaries_report function; summaries field in MetadataDebugReport; 2 debug JSON tests

## Decisions Made
- Summary validation runs after validate_abstract_domains in the kernel validation sequence, consistent with provider ordering (direct_summaries runs after abstract_domains)
- Precision ceiling check rejects FactPrecision::Exact from polint.direct_summaries metadata, matching the pattern from domains/validate.rs and calls/validate.rs
- Debug rows use as_str labels for domain, status, precision, and provenance instead of dense IDs or enum discriminants

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Summary validation is wired into kernel pipeline and catches invalid summary facts before they propagate
- Debug JSON provides internal observation path for eval fixtures and development
- Ready for Plan 32-06 (eval integration) or Plan 32-07 (public boundary proof)
- All 985 tests pass including 6 new tests from this plan

## Self-Check: PASSED

- [x] `crates/polint/src/analysis/summaries/validate.rs` exists
- [x] `crates/polint/src/analysis/summaries/mod.rs` updated with validate module
- [x] `crates/polint/src/analysis_kernel/validation.rs` calls validate_summaries
- [x] `crates/polint/src/analysis_kernel/debug.rs` includes summaries field
- [x] Commit bb3f5a0 verified
- [x] Commit d4196a5 verified
- [x] All 985 tests pass
- [x] Formatting clean

---
*Phase: 32-summary-kernel-and-direct-summaries*
*Completed: 2026-05-21*

---
phase: 32-summary-kernel-and-direct-summaries
plan: 03
subsystem: analysis
tags: [summary-builder, direct-summaries, control-effects, call-effects, memory-effects, data-flow-tito, mir-traversal]

requires:
  - phase: 32-summary-kernel-and-direct-summaries
    plan: 01
    provides: "SummaryDomain trait, four core domain types, fact vocabulary enums"
  - phase: 32-summary-kernel-and-direct-summaries
    plan: 02
    provides: "SummaryOutput, SummaryStore, AnalysisDb summary storage and metadata refresh"
  - phase: 31-p0-abstract-domain-kernel
    provides: "Domain solver results and observation facts consumed by control-effect lifting"
  - phase: 30-direct-call-facts
    provides: "Direct call facts, call targets, and unresolved call facts consumed by call-effect builder"
provides:
  - "DirectSummaryBuilder::build(db: &AnalysisDb) -> SummaryOutput computing all four domains per function"
  - "Control-effect builder lifting from domain observations and CFG/MIR exit operations"
  - "Call-effect builder reading call targets and unresolved calls per function"
  - "Memory-effect builder tracking parameter/local/global access kinds from MIR operations"
  - "TITO builder detecting parameter-to-return Value flow and parameter BySideEffect mutation"
  - "Unknown/top summary entries and events for functions with unresolved calls (D-06)"
affects: [32-summary-kernel-and-direct-summaries, 33-demand-queries-and-summary-scc-cache, 38-local-plus-summary-projected-data-flow]

tech-stack:
  added: []
  patterns: ["DirectSummaryBuilder reads AnalysisDb facts without re-running the solver (D-12)", "Simple transitive-closure source tracing for TITO without field-level access paths (D-07/D-10)"]

key-files:
  created:
    - "crates/polint/src/analysis/summaries/builder.rs"
  modified:
    - "crates/polint/src/analysis/summaries/mod.rs"

key-decisions:
  - "All four domain builders implemented in a single DirectSummaryBuilder::build pass for determinism and simplicity"
  - "Control effects lift does-not-return evidence from domain observation unreachable labels at exit blocks"
  - "TITO uses simple copy-chain tracing without field-level access path tracking per D-07/D-10"
  - "Memory effects treat all PlaceRoot::Parameter variants uniformly as Param(index) since there is no separate Receiver root in the place model"
  - "Unresolved calls set may_have_external_effects=true on memory effects and add Unknown exit kind on control effects"

patterns-established:
  - "DirectSummaryBuilder: single-pass builder reading AnalysisDb facts to produce SummaryOutput for all four domains"
  - "Source-level D-12 proof: test verifies production code contains no solver type references via include_str! split at #[cfg(test)]"

requirements-completed: []

duration: 6min
completed: 2026-05-21
---

# Phase 32 Plan 03: Summary Builder and Direct Summary Computation Summary

**DirectSummaryBuilder producing four-domain SummaryOutput from MIR/CFG/calls/domain facts with explicit unknown/top for unresolved calls**

## Performance

- **Duration:** 6 min
- **Started:** 2026-05-21T18:50:37Z
- **Completed:** 2026-05-21T18:56:37Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Implemented DirectSummaryBuilder::build producing SummaryOutput with all four summary domains per analyzed function
- Control effects lift does-not-return evidence from domain reachability observations and detect exit kinds from MIR/CFG
- Call effects read direct call targets and unresolved calls, with proper unknown/top for uncertainty
- Memory effects track per-resource access kinds (Read/Write/ReadWrite) from MIR operations with may_have_external_effects for unresolved calls
- TITO builder detects parameter-to-return Value flow through copy-chain tracing and parameter BySideEffect mutation
- Seven unit tests including source-level D-12 proof, empty DB, four-domain output, unresolved call events, memory tracking, TITO detection, and external effects

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement control-effect and call-effect summary builders that lift from domain results and call facts** - `130bf2d` (feat)
2. **Task 2: Add memory-effect and TITO summary builders with MIR/place traversal** - `a970032` (test)

## Files Created/Modified
- `crates/polint/src/analysis/summaries/builder.rs` - DirectSummaryBuilder with build_control_effects, build_call_effects, build_memory_effects, build_tito, trace_sources, and classify_domain_output helpers plus 7 unit tests
- `crates/polint/src/analysis/summaries/mod.rs` - Added pub(crate) mod builder declaration

## Decisions Made
- Implemented all four domain builders in a single DirectSummaryBuilder::build pass rather than separate sub-passes, for deterministic output and reduced AnalysisDb traversal
- Control-effect does-not-return detection checks domain observation reachability labels at exit blocks rather than re-running the solver
- TITO uses simple transitive copy-chain tracing (BTreeMap worklist) without field-level access paths per D-07/D-10
- Memory effects use PlaceRoot::Parameter uniformly for Param(index) tracking since the place model has no separate Receiver variant
- D-12 compliance verified by source-level test that splits at #[cfg(test)] and checks production code for solver type references

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed D-12 source-level proof test detecting its own assertion strings**
- **Found during:** Task 1
- **Issue:** The builder_does_not_call_solver test contained literal solver type names in its assertion messages, causing include_str!-based detection to find them in the test code itself
- **Fix:** Split source at #[cfg(test)] boundary to check only production code, and used string concatenation for test type name references
- **Files modified:** `crates/polint/src/analysis/summaries/builder.rs`
- **Verification:** All 7 tests pass
- **Committed in:** 130bf2d

---

**Total deviations:** 1 auto-fixed (1 bug fix)
**Impact on plan:** Fix was essential for correct D-12 source proof. No scope creep.

## Issues Encountered
None beyond the D-12 test detection issue documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- DirectSummaryBuilder is ready for provider wiring (future plan will call builder and store output)
- SummaryOutput from builder can be passed to SummaryStore::from_output and AnalysisDb::replace_summary_facts
- All four domains produce summary facts and events suitable for validation, debug, and eval integration
- Ready for Plan 32-04 and subsequent provider/validation/debug/eval plans

## Self-Check: PASSED

- [x] `crates/polint/src/analysis/summaries/builder.rs` exists
- [x] `crates/polint/src/analysis/summaries/mod.rs` updated
- [x] Commit 130bf2d verified
- [x] Commit a970032 verified
- [x] All 7 builder tests pass
- [x] Formatting clean

---
*Phase: 32-summary-kernel-and-direct-summaries*
*Completed: 2026-05-21*

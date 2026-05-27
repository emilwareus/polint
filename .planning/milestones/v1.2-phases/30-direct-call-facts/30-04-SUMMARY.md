---
phase: 30-direct-call-facts
plan: 04
subsystem: analysis
tags: [rust, analysis-kernel, call-facts, semantic-mir, unresolved-calls]

requires:
  - phase: 30-direct-call-facts
    provides: "Plan 01 call fact contracts, CallStore indexes, and AnalysisDb call storage"
  - phase: 30-direct-call-facts
    provides: "Plan 02 private polint.calls provider and output digest identity"
  - phase: 30-direct-call-facts
    provides: "Plan 03 call validation and test-only debug snapshots"
provides:
  - "MIR-driven CallSiteFact extraction for semantic MIR call operations"
  - "Deterministic call-site stable keys from language, file, caller, span, callee shape, operation key or same-span ordinal, and call kind"
  - "First-class unresolved-call derivation for function values, unknown callees, dynamic property/index shapes, and UnsupportedDomain::Calls evidence"
  - "Provider publication of populated call-site and unresolved rows with repeatable output digests and debug/index proof"
affects: [analysis, analysis-kernel, direct-calls, future-direct-targets, summaries]

tech-stack:
  added: []
  patterns: ["MIR-only call extraction", "specific unresolved reason evidence", "provider-derived populated call output proof"]

key-files:
  created:
    - crates/polint/src/analysis/calls/extract.rs
    - crates/polint/src/analysis/calls/unresolved.rs
  modified:
    - crates/polint/src/analysis/calls/mod.rs
    - crates/polint/src/analysis/calls/facts.rs
    - crates/polint/src/analysis/calls/provider.rs
    - crates/polint/src/analysis_kernel/debug.rs
    - crates/polint/src/core/mod.rs

key-decisions:
  - "Call-site extraction consumes semantic MIR and place rows only; no parser AST or source reparsing dependency was added."
  - "Direct targets remain empty in this plan; function-value, dynamic, unknown, setup-missing, and unsupported call evidence is published as unresolved rows."
  - "Call output digest proof now covers provider-derived populated sites and unresolved rows, while direct target coverage remains in the later direct-target plan."

patterns-established:
  - "Call-site stable keys use metadata-backed file/function identity plus MIR operation stable keys with same-span fallback."
  - "Unsupported call evidence maps to specific UnresolvedCallReason labels instead of collapsing to generic unknown."

requirements-completed: [SAE-SEM-05]

duration: 17 min
completed: 2026-05-21
---

# Phase 30 Plan 04: MIR Call Sites and Unresolved Evidence Summary

**MIR-driven call-site extraction with explicit unresolved-call evidence and populated provider/debug proof**

## Performance

- **Duration:** 17 min
- **Started:** 2026-05-21T08:26:53Z
- **Completed:** 2026-05-21T08:43:41Z
- **Tasks:** 3
- **Files modified:** 7

## Accomplishments

- Added `extract_call_sites(db)` to publish `CallSiteFact` rows from `MirOperationKind::Call` without using parser ASTs.
- Added `derive_unresolved_calls(db, sites)` for function values, unknown callees, dynamic property/index/member shapes, and `UnsupportedDomain::Calls` rows.
- Wired `polint.calls` provider output to store populated sites and unresolved rows, with repeatable digest and debug/index regression proof.

## Task Commits

1. **Task 1 RED:** `96206b9` test(30-04): add failing test for call-site extraction
2. **Task 1 GREEN:** `86d111b` feat(30-04): extract call-site facts from MIR
3. **Task 2 RED:** `61f3095` test(30-04): add failing test for unresolved calls
4. **Task 2 GREEN:** `b10ab1d` feat(30-04): derive unresolved call evidence
5. **Task 3:** `8694e8c` test(30-04): prove populated call indexes and debug counts

## Files Created/Modified

- `crates/polint/src/analysis/calls/extract.rs` - MIR call operation to call-site fact extraction, callee-shape mapping, stable-key construction, and extraction tests.
- `crates/polint/src/analysis/calls/unresolved.rs` - Unresolved-call row derivation from call-site shapes and unsupported semantic evidence.
- `crates/polint/src/analysis/calls/provider.rs` - Provider wiring for extracted sites/unresolved rows plus populated digest/index tests.
- `crates/polint/src/analysis/calls/facts.rs` - Added missing call syntax/reason vocabulary required by extraction and unresolved evidence.
- `crates/polint/src/analysis/calls/mod.rs` - Registered `extract` and `unresolved` modules.
- `crates/polint/src/analysis_kernel/debug.rs` - Added labels/tests for new call syntax and unresolved reason counts.
- `crates/polint/src/core/mod.rs` - Added metadata labels for new call syntax and unresolved reason variants.

## Decisions Made

- Kept all direct-call work crate-private under `analysis::calls`; no SDK, runner, CLI, README, or public docs surface was promoted.
- Treated direct targets as intentionally empty for this plan. Sites and unresolved rows are enough for Phase 31/32 consumers while Plan 30-05 owns direct target facts.
- Used specific unresolved reason labels for unsupported evidence so dynamic/unsupported calls remain auditable.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added missing call syntax and unresolved reason vocabulary**
- **Found during:** Tasks 1 and 2
- **Issue:** The planned extraction/unresolved behavior required `FunctionValue`, `DynamicImport`, `Require`, `GoroutineBoundary`, and `UnknownCallee` labels, but the existing enums and metadata/debug label functions did not include all of them.
- **Fix:** Added the missing enum variants and label mappings so provider metadata/debug output can preserve the specific evidence required by the plan.
- **Files modified:** `crates/polint/src/analysis/calls/facts.rs`, `crates/polint/src/core/mod.rs`, `crates/polint/src/analysis_kernel/debug.rs`
- **Verification:** `cargo test -p polint --lib analysis::calls --locked`
- **Committed in:** `86d111b`, `b10ab1d`

---

**Total deviations:** 1 auto-fixed (Rule 2).
**Impact on plan:** Required vocabulary completion only; no public call graph surface or direct target resolution was added.

## Issues Encountered

- Task 3 proof tests passed against the provider/store/debug plumbing already built in Tasks 1 and 2 plus prior plans, so Task 3 produced a test-only commit rather than an additional GREEN implementation commit.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --lib analysis::calls --locked`
- `cargo test -p polint --lib calls_provider --locked`
- `cargo test -p polint --lib analysis_kernel::debug::calls_debug_json --locked`
- `cargo fmt --all -- --check`

## Known Stubs

None.

## Threat Flags

None - MIR-to-calls, unsupported-to-unresolved, and aggregate debug-count surfaces were covered by the plan threat model.

## Next Phase Readiness

Plan 30-05 can derive direct call targets on top of populated call sites while unresolved evidence remains explicit for forms that direct/binding resolution cannot prove.

## Self-Check: PASSED

- Verified created key files exist on disk.
- Verified summary file exists on disk.
- Verified all task commit hashes exist in git history.

---
*Phase: 30-direct-call-facts*
*Completed: 2026-05-21*

---
phase: 32-summary-kernel-and-direct-summaries
plan: 01
subsystem: analysis
tags: [summary-domain, lattice, control-effects, call-effects, memory-effects, data-flow-tito]

requires:
  - phase: 31-p0-abstract-domain-kernel
    provides: "AbstractDomain trait, TopReason enum, Changed enum, lattice law test patterns"
  - phase: 30-direct-call-facts
    provides: "Direct call facts and call-site/target model consumed by call effect summaries"
provides:
  - "SummaryDomain trait with bottom, unknown_top, join, leq, stable_digest_parts"
  - "SummaryTopReason enum for summary-level top causes"
  - "Four core summary domain types: ControlEffects, CallEffects, MemoryEffects, DataFlowTito"
  - "Summary fact vocabulary: SummaryDomainKind, SummaryStatus, SummaryPrecision, SummaryProvenance"
  - "Flow vocabulary: FlowKind, FlowRoot, FlowEdge, ExitKind, AsyncKind, AccessKind, ExternalEffectKind, MemoryResource"
  - "SummaryFact and SummaryEventFact structs with dense IDs and stable keys"
  - "SummaryId and SummaryEventId newtypes in analysis/ids.rs"
  - "AccessKind::join lattice method"
affects: [32-summary-kernel-and-direct-summaries, 33-demand-queries-and-summary-scc-cache, 38-local-plus-summary-projected-data-flow]

tech-stack:
  added: []
  patterns: ["SummaryDomain trait parallel to AbstractDomain but at callable granularity", "Three-variant enum pattern: Bottom/Effects/Top(Reason) for each domain"]

key-files:
  created:
    - "crates/polint/src/analysis/summaries/mod.rs"
    - "crates/polint/src/analysis/summaries/domain.rs"
    - "crates/polint/src/analysis/summaries/facts.rs"
    - "crates/polint/src/analysis/summaries/core.rs"
  modified:
    - "crates/polint/src/analysis/mod.rs"
    - "crates/polint/src/analysis/ids.rs"

key-decisions:
  - "Use max instead of saturating_add for CallEffects unresolved_count join to preserve lattice idempotence"
  - "Re-declare Changed enum locally in summaries::domain rather than importing from domains::lattice to keep module boundaries clean"
  - "AccessKind join is impl'd on the enum in core.rs since it is specific to summary domain join behavior"

patterns-established:
  - "SummaryDomain trait: const ID, const VERSION, bottom(), unknown_top(reason), is_bottom(), is_top(), leq(), join(), join_into() with default, stable_digest_parts()"
  - "Three-variant domain pattern: Bottom/Effects{fields}/Top(SummaryTopReason) with explicit unknown_top constructors"
  - "Law test helpers: assert_bottom_leq_all, assert_top_geq_all, assert_join_commutative, assert_join_idempotent, assert_digest_deterministic"

requirements-completed: []

duration: 8min
completed: 2026-05-21
---

# Phase 32 Plan 01: Summary Kernel Contracts Summary

**SummaryDomain trait, four core direct-summary domain types with lattice law tests, fact vocabulary enums, and summary ID newtypes**

## Performance

- **Duration:** 8 min
- **Started:** 2026-05-21T18:32:28Z
- **Completed:** 2026-05-21T18:40:28Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- Established SummaryDomain trait with callable-level lattice semantics parallel to AbstractDomain
- Implemented four core summary domains (ControlEffects, CallEffects, MemoryEffects, DataFlowTito) with correct join, leq, and stable digest behavior
- Defined complete fact vocabulary for summary status, precision, provenance, flow kinds, exit kinds, async kinds, access kinds, and memory resources
- Added 56 tests including comprehensive lattice law proofs for all four domains

## Task Commits

Each task was committed atomically:

1. **Task 1: Add summary module root, SummaryDomain trait, SummaryTopReason, summary ID newtypes, and fact vocabulary enums** - `be56503` (feat)
2. **Task 2: Implement four core summary domain types with lattice semantics and law tests** - `3209a18` (feat)

## Files Created/Modified
- `crates/polint/src/analysis/summaries/mod.rs` - Module root declaring domain, core, facts submodules with dead_code expectation
- `crates/polint/src/analysis/summaries/domain.rs` - SummaryDomain trait, SummaryTopReason enum, Changed enum, and trait law tests
- `crates/polint/src/analysis/summaries/facts.rs` - Summary fact vocabulary: 10 enums plus SummaryFact and SummaryEventFact structs
- `crates/polint/src/analysis/summaries/core.rs` - Four domain types implementing SummaryDomain with 37 law and behavior tests
- `crates/polint/src/analysis/mod.rs` - Added pub(crate) mod summaries declaration
- `crates/polint/src/analysis/ids.rs` - Added SummaryId and SummaryEventId newtypes with standard ID contract

## Decisions Made
- Used `max` instead of `saturating_add` for CallEffects `unresolved_count` in join to preserve lattice idempotence (join(x, x) == x requires max, not sum)
- Re-declared `Changed` enum locally in `summaries::domain` rather than importing from `domains::lattice` to keep the summaries module boundary clean and self-contained
- Placed `AccessKind::join` impl in `core.rs` since it is used exclusively by MemoryEffects join logic

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed CallEffects join idempotence violation**
- **Found during:** Task 2 (core domain implementation)
- **Issue:** Using `saturating_add` for `unresolved_count` join caused join(x, x) to double the count, violating lattice idempotence
- **Fix:** Changed to `max(*left_unresolved, *right_unresolved)` which preserves join idempotence
- **Files modified:** `crates/polint/src/analysis/summaries/core.rs`
- **Verification:** All 37 core tests pass including call_join_idempotent
- **Committed in:** 3209a18

---

**Total deviations:** 1 auto-fixed (1 bug fix)
**Impact on plan:** Fix was essential for lattice correctness. No scope creep.

## Issues Encountered
None beyond the idempotence fix documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- SummaryDomain trait and four core domains are ready for store, builder, provider, validation, debug, and eval integration in subsequent plans
- All domains support bottom, unknown_top, join, leq, and stable_digest_parts
- Fact vocabulary is complete for summary metadata in later plans

---
*Phase: 32-summary-kernel-and-direct-summaries*
*Completed: 2026-05-21*

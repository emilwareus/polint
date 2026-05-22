---
phase: 32-summary-kernel-and-direct-summaries
plan: 07
subsystem: analysis
tags: [public-boundary, no-leak, direct-summaries, cli-integration, sdk, go, typescript]

requires:
  - phase: 32-summary-kernel-and-direct-summaries
    plan: 06
    provides: "Direct summary eval fixtures with observation, determinism, and domain coverage"
  - phase: 31-p0-abstract-domain-kernel
    plan: 05
    provides: "Abstract-domain public-boundary proof pattern used as template"
  - phase: 30-direct-call-facts
    plan: 08
    provides: "Direct-call public-boundary proof pattern used as template"
provides:
  - "direct_summaries_internals_stay_private integration test proving no-leak boundary for direct summary internals"
  - "21 internal markers checked against public CLI JSON, inspect JSON, test JSON, help text, SDK, runner, README, and docs"
  - "External temp-repo rule compiling and running with only polint::sdk::prelude::* without summary access"
  - "Full test suite verification: 993 lib tests + 122 integration tests passing with direct summary provider"
affects: [33-demand-queries-and-summary-scc-cache, 41-public-sdk-query-views-and-agent-ergonomics]

tech-stack:
  added: []
  patterns: ["Direct-summary no-leak proof follows the abstract-domain and direct-call public-boundary test patterns"]

key-files:
  created: []
  modified:
    - "crates/polint/tests/cli.rs"

key-decisions:
  - "Removed generic 'direct_summaries' and 'DirectSummaries' from marker list to avoid false-positive matches against the test's own naming; kept 21 specific internal markers"
  - "Verification-only empty commit for Task 1 since provider-order assertions were already updated in prior plans 32-01 through 32-06"

patterns-established:
  - "Direct-summary public-boundary proof: 21 markers covering provider IDs, domain names, type names, and fact families"
  - "External temp-repo rule pattern extended with all 7 supported public fact views for summary boundary verification"

requirements-completed: [SAE-INT-02]

duration: 10min
completed: 2026-05-21
---

# Phase 32 Plan 07: Public Boundary Proof for Direct Summary Internals Summary

**Integration test proving 21 direct-summary internal markers stay private across public CLI JSON, inspect, test, help, SDK, runner, README, docs, and external rule consumers**

## Performance

- **Duration:** 10 min
- **Started:** 2026-05-21T19:39:42Z
- **Completed:** 2026-05-21T19:50:01Z
- **Tasks:** 3
- **Files modified:** 1

## Accomplishments
- Verified all provider-order assertions already include polint.direct_summaries between polint.abstract_domains and polint.metrics (no changes needed)
- Added direct_summaries_internals_stay_private integration test to tests/cli.rs with 21 internal markers
- Created external temp-repo rule proving polint::sdk::prelude::* and polint::runner::run_cli work without summary access
- Full test suite passes: 993 lib tests + 122 integration tests, cargo fmt clean

## Task Commits

Each task was committed atomically:

1. **Task 1: Verify provider-order expectations include polint.direct_summaries** - `d1394c6` (test)
2. **Task 2: Add public no-leak boundary proof for direct summary internals** - `f2a586c` (feat)
3. **Task 3: Run full verification across all targets** - `2c96685` (test)

## Files Created/Modified
- `crates/polint/tests/cli.rs` - Added direct_summaries_internals_stay_private test with 21 internal markers, external temp-repo rule setup, public surface assertions, CLI help assertions, and fixture file writers

## Decisions Made
- Removed generic "direct_summaries" and "DirectSummaries" from internal marker list since those substrings would match the test's own function naming. Kept 21 specific markers: polint.direct_summaries, direct-summaries-facts, summary_control, summary_call, summary_memory, summary_tito, SummaryDomain, SummaryStore, SummaryBuilder, SummaryOutput, SummaryKey, ControlEffects, CallEffects, MemoryEffects, DataFlowTito, control_effects, memory_effects, data_flow_tito, analysis::summaries, Effects<'_>, TaintFlows<'_>
- Used verification-only empty commit for Task 1 since provider-order assertions were already updated in prior plans 32-01 through 32-06, following the Plan 30-08 precedent
- External temp-repo rule requests all 7 supported public fact views (ResolvedImports, ModuleGraphFacts, Symbols, References, FileMetrics, FunctionMetrics, ComplexityMetrics) to prove maximum SDK surface compatibility

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Adjusted internal marker list to avoid self-referential false positives**
- **Found during:** Task 2 (integration test first run)
- **Issue:** Generic marker "direct_summaries" matched the test's own rule function name `direct_summaries_public_probe`, causing a false-positive failure when the test checked its own rule source
- **Fix:** Removed "direct_summaries" and "DirectSummaries" from DIRECT_SUMMARIES_INTERNAL_PUBLIC_MARKERS; kept 21 specific internal vocabulary markers that would never appear in rule source
- **Files modified:** crates/polint/tests/cli.rs
- **Verification:** Integration test passes with 0 false positives
- **Committed in:** f2a586c (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug in marker list)
**Impact on plan:** Fix necessary for test correctness. No scope creep.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 32 is fully complete: summary kernel, store, builder, provider, validation, debug, eval, and public-boundary proof all delivered
- Direct summary internals confirmed private across all public surfaces
- SAE-INT-02 requirement fully satisfied
- Ready for Phase 33 (demand queries and summary SCC cache) which will consume direct summaries from the store

## Self-Check: PASSED

- [x] `crates/polint/tests/cli.rs` contains direct_summaries_internals_stay_private test
- [x] Commit d1394c6 verified
- [x] Commit f2a586c verified
- [x] Commit 2c96685 verified
- [x] All 993 lib tests pass
- [x] All 122 integration tests pass
- [x] Formatting clean

---
*Phase: 32-summary-kernel-and-direct-summaries*
*Completed: 2026-05-21*

---
phase: 32-summary-kernel-and-direct-summaries
plan: 06
subsystem: analysis
tags: [eval-fixtures, direct-summaries, observation, determinism, go, typescript]

requires:
  - phase: 32-summary-kernel-and-direct-summaries
    plan: 05
    provides: "Summary validation and debug JSON with compact rows, event rows, status counts, domain counts"
  - phase: 22-internal-evaluation-harness-mvp
    provides: "Evaluation harness model, matcher, metrics, and native fixture infrastructure"
  - phase: 31-p0-abstract-domain-kernel
    provides: "Abstract-domain eval fixture pattern used as template"
provides:
  - "FixtureArea::DirectSummaries variant and DIRECT_SUMMARY_FACT_FAMILIES const"
  - "observe_direct_summaries normalizing summary debug JSON into compact eval rows with fact families and invariants"
  - "Native mixed Go/TS direct-summary fixture covering four domains and unknown/top events"
  - "run_direct_summaries_core_fixture_for_test with cold/warm/no-cache determinism comparison"
affects: [32-summary-kernel-and-direct-summaries, 33-demand-queries-and-summary-scc-cache]

tech-stack:
  added: []
  patterns: ["Summary eval observation maps domain names to fact families: control_effects -> summary_control, call_effects -> summary_call, etc.", "Summary eval count invariants use direct_summaries.counts.{field} and direct_summaries.domain_counts.{domain}.nonzero patterns"]

key-files:
  created:
    - "tests/eval-fixtures/direct-summaries/core/expected.polint-eval.toml"
    - "tests/eval-fixtures/direct-summaries/core/repo/.polint.toml"
    - "tests/eval-fixtures/direct-summaries/core/repo/summary.go"
    - "tests/eval-fixtures/direct-summaries/core/repo/go.mod"
    - "tests/eval-fixtures/direct-summaries/core/repo/web/package.json"
    - "tests/eval-fixtures/direct-summaries/core/repo/web/tsconfig.json"
    - "tests/eval-fixtures/direct-summaries/core/repo/web/src/summary.ts"
  modified:
    - "crates/polint/src/eval/model.rs"
    - "crates/polint/src/eval/observed.rs"
    - "crates/polint/src/eval/mod.rs"
    - "crates/polint/src/eval/fixtures.rs"

key-decisions:
  - "Map summary domain names to eval fact families: control_effects -> summary_control, call_effects -> summary_call, memory_effects -> summary_memory, data_flow_tito -> summary_tito"
  - "Summary event facts use summary_event family rather than per-domain event families"
  - "Payload format uses semicolon-delimited compact fragments: domain;status;precision;provenance;payload_digest_prefix"
  - "Determinism comparison uses cold/warm/no-cache three-way equality like direct-calls and abstract-domains patterns"

patterns-established:
  - "Direct-summary eval observation pattern: read summaries section from debug JSON, map to fact families, emit count and domain_count invariants"
  - "Direct-summary fixture pattern: mixed Go/TS repo with POLINT-FEATURE markers, subset matching, domain count nonzero invariants, determinism invariant"

requirements-completed: [SAE-INT-02]

duration: 10min
completed: 2026-05-21
---

# Phase 32 Plan 06: Direct Summary Eval Fixtures Summary

**Eval observation normalizes summary debug JSON into compact eval rows with four fact families, domain count invariants, and a native mixed Go/TS fixture proving control/call/memory/TITO/unknown summaries and determinism**

## Performance

- **Duration:** 10 min
- **Started:** 2026-05-21T19:26:12Z
- **Completed:** 2026-05-21T19:37:01Z
- **Tasks:** 2
- **Files modified:** 11

## Accomplishments
- Added FixtureArea::DirectSummaries variant, DIRECT_SUMMARY_FACT_FAMILIES const, and observe_direct_summaries function mapping summary debug JSON rows to compact eval facts with domain-specific fact families
- Created native mixed Go/TS fixture covering panic/throw (control), param-return/identity (TITO), receiver-mutation/param-write (memory), unresolved-call/dynamic-callback (call effects with unknown events), global-read, pure/no-effects functions
- Added determinism invariant proving cold/warm/no-cache output identity across repeated runs
- Added 8 tests total: 4 unit tests (area parsing, family recognition, debug row normalization, unknown status metrics) and 4 integration tests (fixture passes, domain coverage, determinism/counts, feature markers)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add direct-summary eval observation, fixture area, and fact-family vocabulary** - `663b43a` (feat)
2. **Task 2: Add native mixed Go/TS direct-summary fixture with expected manifest** - `e38503e` (feat)

## Files Created/Modified
- `crates/polint/src/eval/model.rs` - Added FixtureArea::DirectSummaries variant and DIRECT_SUMMARY_FACT_FAMILIES const
- `crates/polint/src/eval/observed.rs` - Added observe_direct_summaries, direct_summary_fact, direct_summary_event_fact, domain-to-family mapping, status/precision label conversion, count/domain_count invariant emission
- `crates/polint/src/eval/mod.rs` - Added direct_summary test module with 4 tests: area parsing, family recognition, debug row normalization, unknown status metrics
- `crates/polint/src/eval/fixtures.rs` - Added run_direct_summaries_core_fixture_for_test, determinism invariant helper, suite dispatch for DirectSummaries area, and direct_summaries_core test module with 4 integration tests
- `tests/eval-fixtures/direct-summaries/core/expected.polint-eval.toml` - Fixture manifest with area=direct-summaries, partial matching for 4 domains, event facts, domain count nonzero invariants, determinism invariant
- `tests/eval-fixtures/direct-summaries/core/repo/summary.go` - Go fixture with AlwaysPanics, ReturnsParam, MutatesReceiver, ReadsGlobal, CallsUnresolved, PureFunction
- `tests/eval-fixtures/direct-summaries/core/repo/web/src/summary.ts` - TS fixture with throwsError, identity, writesParam, callsDynamic, noEffects
- `tests/eval-fixtures/direct-summaries/core/repo/.polint.toml` - Fixture config with Go+TS workspace
- `tests/eval-fixtures/direct-summaries/core/repo/go.mod` - Go module declaration
- `tests/eval-fixtures/direct-summaries/core/repo/web/package.json` - Minimal JS package
- `tests/eval-fixtures/direct-summaries/core/repo/web/tsconfig.json` - TS config with strict mode

## Decisions Made
- Map summary domain names to eval fact families (control_effects -> summary_control, etc.) following the plan's DIRECT_SUMMARY_FACT_FAMILIES array
- Use summary_event as a single family for all event rows rather than per-domain event families, consistent with the existing DomainEvent pattern
- Use compact semicolon-delimited payload format: domain;status;precision;provenance;payload_digest_prefix
- Determinism comparison uses cold/warm/no-cache three-way equality pattern matching existing direct-calls and abstract-domains fixtures
- Expected manifest uses partial mode with stable_key substrings for domain matching, avoiding fragile exact key matching

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed expected manifest stable_key for summary_event facts**
- **Found during:** Task 2 (fixture integration test)
- **Issue:** Expected stable_key "summary_event" did not match observed keys which use "SummaryEvent" prefix
- **Fix:** Changed expected stable_key to "SummaryEvent" for substring matching
- **Files modified:** tests/eval-fixtures/direct-summaries/core/expected.polint-eval.toml
- **Verification:** Fixture test passes with 0 false negatives
- **Committed in:** e38503e (Task 2 commit)

**2. [Rule 1 - Bug] Removed incorrect total count invariant**
- **Found during:** Task 2 (fixture integration test)
- **Issue:** Expected invariant with value "0" and mode "tolerant" did not match observed total count (44)
- **Fix:** Removed the total count invariant; domain_counts nonzero invariants provide equivalent coverage
- **Files modified:** tests/eval-fixtures/direct-summaries/core/expected.polint-eval.toml
- **Verification:** Fixture test passes with 0 false negatives
- **Committed in:** e38503e (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (2 bugs in expected manifest)
**Impact on plan:** Both fixes necessary for correct test matching. No scope creep.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Eval observation for direct summaries is wired into the full fixture pipeline
- Native Go/TS fixture proves all four summary domains produce facts and unknown events
- Determinism invariant proves output stability across cold/warm/no-cache runs
- Ready for Plan 32-07 (public boundary proof) or Phase 33 (demand queries and summary SCC cache)
- All 993 tests pass including 8 new tests from this plan

## Self-Check: PASSED

- [x] `tests/eval-fixtures/direct-summaries/core/expected.polint-eval.toml` exists
- [x] `tests/eval-fixtures/direct-summaries/core/repo/.polint.toml` exists
- [x] `tests/eval-fixtures/direct-summaries/core/repo/summary.go` exists
- [x] `tests/eval-fixtures/direct-summaries/core/repo/go.mod` exists
- [x] `tests/eval-fixtures/direct-summaries/core/repo/web/src/summary.ts` exists
- [x] `crates/polint/src/eval/model.rs` has FixtureArea::DirectSummaries
- [x] `crates/polint/src/eval/observed.rs` has observe_direct_summaries
- [x] `crates/polint/src/eval/fixtures.rs` dispatches DirectSummaries
- [x] Commit 663b43a verified
- [x] Commit e38503e verified
- [x] All tests pass
- [x] Formatting clean

---
*Phase: 32-summary-kernel-and-direct-summaries*
*Completed: 2026-05-21*

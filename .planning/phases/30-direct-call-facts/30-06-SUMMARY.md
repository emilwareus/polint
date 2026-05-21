---
phase: 30-direct-call-facts
plan: 06
subsystem: analysis
tags: [rust, eval, direct-calls, call-facts, debug-json]

requires:
  - phase: 30-direct-call-facts
    provides: "Plans 30-01 through 30-05 call fact contracts, provider/debug rows, call-site extraction, unresolved evidence, and direct target resolution"
  - phase: 22-internal-evaluation-harness-mvp
    provides: "Crate-private eval model, observed rows, matcher, metrics, and fixture loading"
provides:
  - "DirectCalls eval fixture area and CallSite, CallTarget, and UnresolvedCall expected fact families"
  - "Test-facing call_facts_for_test normalization from metadata_debug_json_for_test()[\"calls\"] rows"
  - "Plan-targeted proof that unresolved, unsupported, and setup_missing call rows count as unknown-like eval evidence"
affects: [direct-calls, eval-fixtures, future-call-validation, phase-30-final-proof]

tech-stack:
  added: []
  patterns: ["test-only debug JSON observation", "compact semicolon eval payload fragments", "unknown-like status reuse"]

key-files:
  created:
    - .planning/phases/30-direct-call-facts/30-06-SUMMARY.md
  modified:
    - crates/polint/src/eval/mod.rs
    - crates/polint/src/eval/model.rs
    - crates/polint/src/eval/observed.rs

key-decisions:
  - "Eval call observation stays crate-private/test-facing; no public SDK, runner, CLI, docs, or call graph API was promoted."
  - "Call eval payloads use relative path, source span, status/kind/algorithm/reason/provider, and stable-key target identity only."
  - "Existing matcher/metrics/report unknown-like status accounting already covered unresolved, unsupported, and setup_missing; plan-specific tests now prove it for call rows."

patterns-established:
  - "Call debug sections sites, targets, and unresolved normalize into eval Fact rows with producer polint.calls."
  - "PascalCase call debug labels normalize to snake_case eval precision/status labels where eval manifests use them."

requirements-completed: [SAE-SEM-05]

duration: 5min
completed: 2026-05-21
---

# Phase 30 Plan 06: Direct Call Eval Observation Summary

**Direct-call debug rows now normalize into deterministic eval facts with compact call evidence and unknown-like status proof**

## Performance

- **Duration:** 5 min
- **Started:** 2026-05-21T09:06:28Z
- **Completed:** 2026-05-21T09:11:26Z
- **Tasks:** 1
- **Files modified:** 3

## Accomplishments

- Added `DirectCalls` as an internal eval fixture area and `CALL_FACT_FAMILIES` for `CallSite`, `CallTarget`, and `UnresolvedCall`.
- Added `call_facts_for_test` and wired call debug rows from `metadata_debug_json_for_test()["calls"]` into eval fact observation.
- Added TDD coverage for call model parsing, compact payload normalization, and unknown-like metrics for unresolved, unsupported, and setup-missing call statuses.

## Task Commits

1. **Task 1 RED:** `811d134` test(30-06): add failing tests for call eval rows
2. **Task 1 GREEN:** `b5e8968` feat(30-06): observe call debug rows in eval

## Files Created/Modified

- `crates/polint/src/eval/mod.rs` - Added direct-call eval tests covering model parsing, observation payloads, and unknown-like metrics.
- `crates/polint/src/eval/model.rs` - Added call fact family constants and the `direct-calls` fixture area.
- `crates/polint/src/eval/observed.rs` - Added call debug JSON normalization into compact eval fact rows.

## Decisions Made

- Kept all eval observation crate-private/test-facing and did not expose a public call graph surface.
- Reused existing `ObservedStatus` unknown-like accounting instead of adding call-specific metric branches.
- Included only compact payload fragments: relative path, span, kind, algorithm, status, reason, provider/provenance, and stable-key call/target identity.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- The literal no-leak scan for `source_text|absolute_path|raw_source|ast` still reports pre-existing `snapshot.source_text_digest.present` test checks in `eval/observed.rs`. A diff-limited scan of the new call observation code returned no matches, so no new call payload identity inputs were introduced.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --lib eval::direct_call_rows --locked`
- `cargo fmt --all -- --check`

## Known Stubs

None.

## Threat Flags

None - the debug JSON to eval-row trust boundary and status accounting changes were covered by the plan threat model.

## Next Phase Readiness

Phase 30 can now add final fixture/no-leak proof on top of direct call observations without promoting public call graph APIs.

## Self-Check: PASSED

- Verified summary and all modified key files exist on disk.
- Verified task commits `811d134` and `b5e8968` exist in git history.

---
*Phase: 30-direct-call-facts*
*Completed: 2026-05-21*

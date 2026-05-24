---
phase: 36-p0-type-value-place-alias-substrate
plan: 04
subsystem: analysis
tags: [rust, ts-js, type-facts, value-facts, access-paths, cache-identity]
requires:
  - phase: 36-p0-type-value-place-alias-substrate
    provides: private type/value/alias provider contracts and kernel wiring
provides:
  - TS/JS type, narrowed type, value, allocation, and access-path facts over private MIR evidence
  - Conservative unknown/unsupported rows for dynamic TS/JS constructs
  - TS/JS lifecycle cache identity regression coverage
affects: [phase-36, phase-37, phase-38, refined-call-graph, data-flow]
tech-stack:
  added: []
  patterns: [private MIR-derived fact extraction, conservative dynamic-language precision]
key-files:
  created:
    - crates/polint/src/analysis/types/ts_js.rs
  modified:
    - crates/polint/src/analysis/types/mod.rs
    - crates/polint/src/analysis/types/provider.rs
    - crates/polint/src/analysis/types/cache_key.rs
key-decisions:
  - "TS/JS precision is derived from polint MIR/place evidence and remains heuristic or conservative unless the source evidence is local and exact."
  - "Dynamic property and unsupported rows produce Unknown/Unsupported facts instead of Exact precision."
patterns-established:
  - "Language-specific type/value extractors merge through the private provider before normalization and digesting."
  - "TS/JS lifecycle inputs are covered by provider parameter digest tests."
requirements-completed: [SAE-PREC-01]
duration: 12 min
completed: 2026-05-24
---

# Phase 36 Plan 04: TS/JS Type, Value, Allocation, Access Path, And Narrowing Facts Summary

**Private TS/JS MIR-derived type/value/access-path facts with conservative dynamic unknowns and TS/JS lifecycle cache identity coverage**

## Performance

- **Duration:** 12 min
- **Started:** 2026-05-24T13:38:12Z
- **Completed:** 2026-05-24T13:50:09Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Added `analysis::types::ts_js`, deriving TS/JS type facts, narrowed type facts, value facts, allocation tokens, and access paths from existing MIR/place/unsupported semantic rows.
- Wired TS/JS output into the private Phase 36 provider alongside Go output before provider digesting and database replacement.
- Added cache-key regression coverage proving TS/JS lifecycle inputs such as tsconfig/package evidence alter Phase 36 provider identity.

## Task Commits

1. **Task 1: Extract TS/JS type and narrowing facts** - `0bd64df` (feat)
2. **Task 2: Emit TS/JS value, allocation, and access-path facts** - `0bd64df` (feat)
3. **Task 3: Include TS/JS semantic input digests when used** - `e99dbd5` (feat)

**Plan metadata:** pending in docs commit.

## Files Created/Modified

- `crates/polint/src/analysis/types/ts_js.rs` - TS/JS MIR-derived fact extraction and regression tests.
- `crates/polint/src/analysis/types/mod.rs` - Internal module registration for the TS/JS extractor.
- `crates/polint/src/analysis/types/provider.rs` - Provider merge path for Go plus TS/JS output.
- `crates/polint/src/analysis/types/cache_key.rs` - TS/JS lifecycle digest regression test.

## Decisions Made

- TS/JS type facts stay conservative because Phase 36 does not invoke a TypeScript checker or claim whole-program JavaScript precision.
- Unsupported dynamic constructs are represented as Unknown/Unsupported facts with source evidence and never upgraded to Exact precision.
- Task 1 and Task 2 share one implementation commit because the extractor and tests need the same module and fixture scaffolding.

## Deviations from Plan

None - plan executed within the intended private provider scope.

## Issues Encountered

- Initial focused test run failed on a byte-range type mismatch in the new source slicing helper; fixed by converting stored byte offsets to `usize`.
- Initial synthetic MIR fixture used a dangling unsupported operation ID; fixed the test fixture to reference an existing operation.

## Verification

- `cargo fmt --all --check` - passed.
- `cargo test -p polint --lib analysis::types::ts_js --locked` - passed.
- `cargo test -p polint --lib analysis::types::cache_key --locked` - passed.
- `cargo test -p polint --test cli --locked` - passed, 124 tests.
- `cargo check -p polint --locked` - passed.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for `36-05`: bounded points-to constraints and alias query service can now consume Go and TS/JS type/value/allocation/access-path substrate facts.

---
*Phase: 36-p0-type-value-place-alias-substrate*
*Completed: 2026-05-24*

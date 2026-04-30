---
phase: 06-sdk-and-example-rules
plan: "04"
subsystem: sdk
tags: [rust, sdk, rules, go, diagnostics, tdd]

# Dependency graph
requires:
  - phase: 06-sdk-and-example-rules
    provides: SDK RuleCtx helpers, literal config, and hardened non-heuristic example rules from Plans 06-01 through 06-03
  - phase: 04-go-adapter
    provides: Go branch obligations, test facts, assertion counts, subtest counts, and table-row evidence
provides:
  - SDK-facing Go branch-obligation heuristic diagnostics with companion test evidence matching
  - weighted Go test-suite maintainability score diagnostics with reproducible score evidence
  - Go assertion-after-action diagnostics with assertion and evidence-term labels
affects: [06-sdk-and-example-rules, polint-rules]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - borrowed RuleCtx iteration with diagnostics collected before ctx.report
    - heuristic diagnostics include reproducible evidence labels and explicit wording
    - TDD rule tests use synthetic AnalysisDb facts through run_rules

key-files:
  created:
    - .planning/phases/06-sdk-and-example-rules/06-04-SUMMARY.md
  modified:
    - crates/polint-rules/src/lib.rs

key-decisions:
  - "Used RuleCtx::branches and RuleCtx::go_tests_for_related_file for Go branch evidence instead of direct AnalysisDb access."
  - "Defined the Go test-suite score as 1 + subtests*4 + table_rows*2 + assertions with default max 24."
  - "Kept all three Go heuristic diagnostics explicit about heuristic behavior and limited evidence to extracted facts."

patterns-established:
  - "Go heuristic rule diagnostics include labels for the inputs that caused a warning."
  - "Companion Go test evidence is matched through RuleCtx related-file helpers rather than global fuzzy matching."
  - "Rule tests assert structured evidence fields rather than substring-only output checks."

requirements-completed: [RULE-05, RULE-06, RULE-07, TEST-01]

# Metrics
duration: 5 min
completed: 2026-04-30
---

# Phase 06 Plan 04: Go Heuristic Example Rules Summary

**SDK-facing Go heuristic rules with companion test evidence, weighted suite scores, and assertion evidence labels**

## Performance

- **Duration:** 5 min
- **Started:** 2026-04-30T09:47:33Z
- **Completed:** 2026-04-30T09:53:26Z
- **Tasks:** 3
- **Files modified:** 1

## Accomplishments

- Hardened `examples/go-branch-obligations` to use `RuleCtx::branches()` and `RuleCtx::go_tests_for_related_file()`, including companion `_test.go` evidence suppression and `condition`, `edge`, and `branch_fingerprint` labels.
- Replaced the Go test-suite size rule with the configured integer maintainability score `1 + (subtest_count * 4) + (table_rows * 2) + assertion_count`, default max `24`, and score-component evidence.
- Added assertion-after-action evidence for test name, assertion count, and comma-joined evidence terms while preserving explicit heuristic wording.

## Task Commits

Each task was committed atomically. TDD tasks include RED and GREEN commits.

1. **Task 1: Harden Go branch-obligation diagnostics**
   - `e3cdeb2` test: add failing branch obligation heuristic tests
   - `1b19668` feat: harden branch obligation heuristics
2. **Task 2: Implement weighted Go test-suite maintainability score**
   - `d464c9b` test: add failing test suite score tests
   - `6499f23` feat: implement weighted test suite score
3. **Task 3: Harden Go assertion-after-action warnings**
   - `43b0cec` test: add failing assertion heuristic tests
   - `ea2e300` feat: add assertion heuristic evidence
4. **Verification cleanup**
   - `6c3dc3b` style: format heuristic rule tests

## Files Created/Modified

- `crates/polint-rules/src/lib.rs` - Hardened Go heuristic rule implementations and added focused TDD unit tests.
- `.planning/phases/06-sdk-and-example-rules/06-04-SUMMARY.md` - Execution summary.

## Decisions Made

- Used SDK-facing `RuleCtx` helpers for branch and related-test access so built-in example rules continue to dogfood the public authoring surface.
- Kept branch evidence matching bounded to same-file and same-directory companion `_test.go` facts through the existing `go_tests_for_related_file` helper.
- Reported only extracted fact data in diagnostics, with heuristic wording so the rules do not claim exact coverage or semantic proof.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Applied rustfmt after verification found formatting drift**
- **Found during:** Plan-wide verification
- **Issue:** `cargo fmt -- --check` failed on the new assertion formatting in `crates/polint-rules/src/lib.rs`.
- **Fix:** Ran `cargo fmt` and committed the formatting-only changes.
- **Files modified:** `crates/polint-rules/src/lib.rs`
- **Verification:** `cargo fmt -- --check` passed after formatting.
- **Committed in:** `6c3dc3b`

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Formatting-only cleanup required for verification. No behavior or scope change.

## Issues Encountered

- The first final `cargo fmt -- --check` failed on test assertion formatting. Rustfmt was applied and all final checks passed.

## Known Stubs

None.

## User Setup Required

None - no external service configuration required.

## Verification

Passed:

- `cargo fmt -- --check`
- `cargo test -p polint-rules --lib branch_obligations`
- `cargo test -p polint-rules --lib test_suite_size`
- `cargo test -p polint-rules --lib assertion_after_action`
- `cargo clippy -p polint-rules --all-targets -- -D warnings`

## Next Phase Readiness

The remaining Phase 6 plans can build CLI and snapshot proof on top of all eight SDK-facing example rules, with the three Go heuristic rules now carrying deterministic evidence and honest wording.

## Self-Check: PASSED

- Confirmed created summary file exists.
- Confirmed modified rules file exists.
- Confirmed task and cleanup commits exist: `e3cdeb2`, `1b19668`, `d464c9b`, `6499f23`, `43b0cec`, `ea2e300`, `6c3dc3b`.

---
*Phase: 06-sdk-and-example-rules*
*Completed: 2026-04-30*

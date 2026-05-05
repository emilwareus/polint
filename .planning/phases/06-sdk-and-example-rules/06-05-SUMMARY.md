---
phase: 06-sdk-and-example-rules
plan: "05"
subsystem: testing
tags: [rust, cli, fixtures, integration-tests, tdd]

# Dependency graph
requires:
  - phase: 06-sdk-and-example-rules
    provides: SDK-facing example rules and Go heuristic diagnostics from Plans 06-01 through 06-04
  - phase: 04-go-adapter
    provides: Go parser facts, branch obligations, test facts, imports, literals, and complexity
  - phase: 05-typescript-adapter
    provides: TS/JS parser facts, string/regex literals, JSX attributes, and complexity
provides:
  - parsed JSON CLI integration proof for all eight requested example rule IDs
  - clean fixture profile coverage proving examples/* diagnostics can be suppressed with reasonable config and evidence
  - failing Go and TS fixture inputs for complexity, imports, raw colors, branch obligations, test-suite score, assertion heuristics, and denied literals
affects: [06-sdk-and-example-rules, polint-cli, fixtures]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - temp-repo CLI integration tests using an exact phase6 profile rule list
    - parsed JSON assertions for rule IDs, evidence, help text, severity overrides, allow-lists, and denied literals
    - clean Go branch evidence expressed through test case names for syntax-level heuristic suppression

key-files:
  created:
    - tests/fixtures/go/failing/payment_test.go
    - .planning/phases/06-sdk-and-example-rules/06-05-SUMMARY.md
  modified:
    - crates/polint-cli/tests/cli.rs
    - tests/fixtures/go/clean/payment_test.go
    - tests/fixtures/go/failing/payment.go
    - tests/fixtures/ts/failing/component.tsx

key-decisions:
  - "Used a small fixture expectation test as the Task 1 RED step before creating the missing failing Go test fixture."
  - "Kept Phase 6 CLI proof in temp repos with exact profile rule IDs and parsed JSON assertions."
  - "Fixed clean branch-obligation suppression through realistic Go test case evidence instead of weakening heuristic rule behavior."

patterns-established:
  - "Phase-specific CLI integration tests should assert parsed JSON fields rather than stdout substrings."
  - "Option coverage tests should assert both positive diagnostics and suppressions for allow and allow_files."
  - "Clean heuristic fixtures should carry explicit evidence terms for syntax-level matching."

requirements-completed: [RULE-01, RULE-02, RULE-03, RULE-04, RULE-05, RULE-06, RULE-07, RULE-08, TEST-01]

# Metrics
duration: 7 min
completed: 2026-04-30
---

# Phase 06 Plan 05: CLI Example Rule Proof Summary

**Parsed JSON CLI integration coverage for all eight SDK example rules with clean and failing fixture proof**

## Performance

- **Duration:** 7 min
- **Started:** 2026-04-30T09:56:33Z
- **Completed:** 2026-04-30T10:04:11Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments

- Added Phase 6 CLI integration tests that run `polint check --profile phase6 --format json --fail-on none` and assert all eight requested `examples/...` rule IDs through parsed JSON.
- Expanded failing Go and TS fixtures with denied literals, raw color/regex inputs, forbidden import coverage, complexity inputs, oversized test-suite evidence, and no-assertion test evidence.
- Added clean fixture coverage proving `examples/*` diagnostics are suppressed under reasonable thresholds, allow-lists, and branch-evidence test names.

## Task Commits

Each task was committed atomically. TDD tasks include RED and GREEN commits.

1. **Task 1: Expand Phase 6 clean and failing fixtures**
   - `f134180` test: add failing fixture coverage expectations
   - `84cf1a0` feat: expand phase 6 rule fixtures
2. **Task 2: Add CLI JSON tests for all Phase 6 example rules**
   - `27fe640` test: add failing phase 6 CLI coverage
   - `9562649` feat: align clean Go fixture evidence
3. **Task 3: Verify Phase 6 CLI fixture coverage**
   - `4d5ad28` chore: verify phase 6 CLI fixture coverage

## Files Created/Modified

- `crates/polint-cli/tests/cli.rs` - Added Phase 6 fixture helpers, exact rule ID list, parsed JSON CLI tests, and fixture expectation coverage.
- `tests/fixtures/go/clean/payment_test.go` - Added clean branch-evidence test case names for heuristic suppression.
- `tests/fixtures/go/failing/payment.go` - Added `legacy-token` and extra control flow for Phase 6 diagnostics.
- `tests/fixtures/go/failing/payment_test.go` - Added oversized suite and no-assertion test cases.
- `tests/fixtures/ts/failing/component.tsx` - Added `/legacy-testid/` regex literal while preserving raw color and denied string coverage.
- `.planning/phases/06-sdk-and-example-rules/06-05-SUMMARY.md` - Execution summary.

## Decisions Made

- Used `crates/polint-cli/tests/cli.rs` for the Task 1 RED fixture expectation because the fixture task needed a committed failing test before creating `payment_test.go`.
- Kept the CLI tests scoped to temp repos and exact `phase6` profiles rather than changing production CLI behavior.
- Preserved heuristic honesty by improving clean fixture evidence instead of loosening `examples/go-branch-obligations`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Clean Go fixture lacked branch evidence for heuristic suppression**
- **Found during:** Task 2 (Add CLI JSON tests for all Phase 6 example rules)
- **Issue:** The new clean fixture test failed because `examples/go-branch-obligations` reported four clean-branch diagnostics for `strings.TrimSpace(customer) == ""`, range iteration, negative charge amount, and `total == 0`.
- **Fix:** Added realistic clean test case names and rows that expose syntax-level evidence terms matching those branch conditions.
- **Files modified:** `tests/fixtures/go/clean/payment_test.go`
- **Verification:** `cargo test -p polint-cli --test cli phase6` passed.
- **Committed in:** `9562649`

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** The fix was limited to owned clean fixture evidence. No production rule, CLI, cache, graph, SARIF, or loader behavior changed.

## Issues Encountered

- The Task 2 RED test correctly exposed the clean branch-evidence gap documented above. It was resolved before final verification.

## Known Stubs

None. The stub scan matched intentional Go table-test literals (`cases := []struct`) and existing CLI test TOML snippets such as `rules = []` and `exclude = []`, not runtime stubs or placeholder behavior.

## User Setup Required

None - no external service configuration required.

## Verification

Passed:

- `cargo fmt -- --check`
- `cargo test -p polint-cli --test cli phase6`
- `cargo clippy -p polint-cli --all-targets -- -D warnings`
- `rg -n "examples/go-cyclomatic-complexity|examples/ts-cyclomatic-complexity|examples/go-import-boundaries|examples/ts-no-raw-colors|examples/go-branch-obligations|examples/go-test-suite-size|examples/go-assertion-after-action|examples/config-query-no-literal" crates/polint-cli/tests/cli.rs`

## Next Phase Readiness

Plan 06-06 can build representative diagnostic snapshots on top of the Phase 6 CLI fixture coverage. All eight requested example rule families now have configured CLI proof through parsed JSON.

## Self-Check: PASSED

- Confirmed created summary file exists.
- Confirmed all modified source and fixture files exist.
- Confirmed task commits exist: `f134180`, `84cf1a0`, `27fe640`, `9562649`, `4d5ad28`.

---
*Phase: 06-sdk-and-example-rules*
*Completed: 2026-04-30*

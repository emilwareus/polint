---
phase: 06-sdk-and-example-rules
plan: "03"
subsystem: sdk
tags: [rust, sdk, rules, go, typescript, diagnostics, tdd]

# Dependency graph
requires:
  - phase: 06-sdk-and-example-rules
    provides: SDK RuleCtx helpers, polint-sdk prelude, literal allow config, Go string facts, and TS regex literal facts from Plans 06-01 and 06-02
  - phase: 04-go-adapter
    provides: Go imports, functions, string literal facts, spans, and cyclomatic complexity facts
  - phase: 05-typescript-adapter
    provides: TS/JS functions, JSX attributes, string literal facts, regex literal syntax text, and cyclomatic complexity facts
provides:
  - SDK-facing Go and TS/JS complexity example rules with configured max thresholds
  - configured Go import boundary example rule with deterministic import evidence
  - TS raw-color example rule with exact literal and file allow-lists plus string/JSX dedupe
  - configured denied-literal example rule over Go, TS/JS string facts, and TS regex literal syntax text
affects: [06-sdk-and-example-rules, polint-rules, polint-sdk]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - built-in rules author through polint_sdk::prelude
    - diagnostics are collected from borrowed RuleCtx facts before reporting
    - syntax-level literal rules apply exact literal allow-lists before denial/reporting

key-files:
  created:
    - .planning/phases/06-sdk-and-example-rules/06-03-SUMMARY.md
  modified:
    - crates/polint-rules/src/lib.rs

key-decisions:
  - "Used polint_sdk::prelude::* for production built-in rule authoring while keeping run_rules access limited to focused unit tests."
  - "Deduped raw-color findings by file, byte range, and literal value so overlapping string and JSX facts produce one diagnostic."
  - "Kept denied regex literal handling syntax-level by reporting the available literal text and matched deny token only."

patterns-established:
  - "Example rule tests build synthetic AnalysisDb facts and execute rules through run_rules."
  - "Literal allow values are exact-match suppressions, separate from allow_files glob suppression."
  - "Literal diagnostics use stable evidence labels for literal, source or matched token, and language."

requirements-completed: [RULE-01, RULE-02, RULE-03, RULE-04, RULE-08, TEST-01]

# Metrics
duration: 10 min
completed: 2026-04-30
---

# Phase 06 Plan 03: SDK-Facing Example Rule Hardening Summary

**SDK-facing Go/TS complexity, import-boundary, raw-color, and denied-literal rules with configured thresholds, allow-lists, and deterministic evidence**

## Performance

- **Duration:** 10 min
- **Started:** 2026-04-30T09:31:49Z
- **Completed:** 2026-04-30T09:42:08Z
- **Tasks:** 3
- **Files modified:** 1

## Accomplishments

- Switched the built-in example rules to the public `polint_sdk::prelude::*` authoring surface and hardened Go/TS complexity plus Go import-boundary diagnostics.
- Added TS raw-color support for exact literal allow values, `allow_files`, JSX attributes, and deterministic duplicate suppression across string and JSX facts.
- Added configured denied-literal reporting across Go and TS/JS literal facts, including TS regex literal syntax text, with stable evidence for literal, match, and language.

## Task Commits

Each task was committed atomically. TDD tasks include RED and GREEN commits.

1. **Task 1: Harden complexity and import-boundary rules**
   - `0f03277` test: add failing tests for configured SDK rules
   - `7b1675d` feat: harden configured complexity and import rules
2. **Task 2: Harden TS raw-color detection with allow-list support**
   - `427a530` test: add failing raw color allow-list tests
   - `cb018c2` feat: harden TS raw color rule allow lists
3. **Task 3: Harden configured denied-literal queries**
   - `10ccc76` test: add failing denied literal query tests
   - `f3f0675` feat: harden configured denied literal queries
4. **Verification cleanup**
   - `49f7fde` refactor: reduce raw color helper arguments

## Files Created/Modified

- `crates/polint-rules/src/lib.rs` - SDK-facing rule implementations, literal filtering helpers, and focused synthetic-fact unit tests.
- `.planning/phases/06-sdk-and-example-rules/06-03-SUMMARY.md` - execution summary.

## Decisions Made

- Used the SDK prelude for production rule authoring and kept direct `polint_core::run_rules` usage inside tests only because `run_rules` is the existing execution helper.
- Preserved stable existing `examples/...` rule IDs and the `built_in_rules()` registration path.
- Treated raw colors and denied regex literals as syntax-level findings only, with help/evidence that avoids semantic CSS or regex claims.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Reduced raw-color helper arguments for clippy**
- **Found during:** Plan-wide verification
- **Issue:** `cargo clippy -p polint-rules --all-targets -- -D warnings` failed on `clippy::too_many_arguments` for the raw-color diagnostic helper introduced in Task 2.
- **Fix:** Grouped raw-color file/span/value/source data into a small `RawColorFinding` struct and preserved existing behavior.
- **Files modified:** `crates/polint-rules/src/lib.rs`
- **Verification:** `cargo test -p polint-rules --lib raw_color` and `cargo clippy -p polint-rules --all-targets -- -D warnings` passed.
- **Committed in:** `49f7fde`

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** The cleanup was limited to satisfying required verification. No scope or behavior change.

## Issues Encountered

- The first final clippy run failed on the raw-color helper argument count. The helper was refactored and all final checks passed.

## Known Stubs

None.

## User Setup Required

None - no external service configuration required.

## Verification

Passed:

- `cargo fmt -- --check`
- `cargo test -p polint-rules --lib complexity`
- `cargo test -p polint-rules --lib import_boundary`
- `cargo test -p polint-rules --lib raw_color`
- `cargo test -p polint-rules --lib config_query`
- `cargo clippy -p polint-rules --all-targets -- -D warnings`

## Next Phase Readiness

Plan 06-04 can build on these SDK-facing non-heuristic examples. The remaining heuristic Go example rules can use the same borrowed RuleCtx patterns and deterministic evidence style.

## Self-Check: PASSED

- Confirmed created summary file exists.
- Confirmed modified rules file exists.
- Confirmed task and cleanup commits exist: `0f03277`, `7b1675d`, `427a530`, `cb018c2`, `10ccc76`, `f3f0675`, `49f7fde`.

---
*Phase: 06-sdk-and-example-rules*
*Completed: 2026-04-30*

---
phase: 06-sdk-and-example-rules
plan: "02"
subsystem: sdk
tags: [rust, sdk, config, go, typescript, literals, tdd]

# Dependency graph
requires:
  - phase: 06-sdk-and-example-rules
    provides: SDK RuleCtx helpers and public string_literals access from Plan 06-01
  - phase: 04-go-adapter
    provides: tree-sitter-go parser traversal, import extraction, and Go unquote helpers
  - phase: 05-typescript-adapter
    provides: Oxc parser traversal and TS/JS string literal fact extraction
provides:
  - literal allow-list config support through RuleConfig and RuleOptions
  - Go string literal facts for SDK-facing rules
  - TS/JS regex literal syntax facts through StringLiteralFact
affects: [06-sdk-and-example-rules, polint-core, polint-config, polint-rules, polint-go, polint-ts]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - additive RuleOptions fields with serde-defaulted config mirrors
    - parser-node literal extraction with exact syntax values and spans
    - syntax-level regex literal reporting without regex semantic evaluation

key-files:
  created:
    - .planning/phases/06-sdk-and-example-rules/06-02-SUMMARY.md
  modified:
    - crates/polint-core/src/lib.rs
    - crates/polint-config/src/lib.rs
    - crates/polint-rules/src/lib.rs
    - crates/polint-go/src/lib.rs
    - crates/polint-ts/src/lib.rs

key-decisions:
  - "Kept literal `allow` separate from `allow_files` as an additive exact-value allow-list."
  - "Excluded Go import path string nodes from general string literal facts so ImportFact remains the import source of truth."
  - "Represented TS/JS regex literals as slash-delimited source syntax only, preserving flags without evaluating regex semantics."

patterns-established:
  - "Literal fact additions should preserve parser spans and exact observable syntax/value text."
  - "Parser fact tests should prove SDK visibility through AnalysisDb or RuleCtx helper access."

requirements-completed: [SDK-02, RULE-04, RULE-08, TEST-01]

# Metrics
duration: 6 min
completed: 2026-04-30
---

# Phase 06 Plan 02: Literal Config and Parser Facts Summary

**Exact literal allow-list config plus Go string literal and TS/JS regex literal facts for SDK rules**

## Performance

- **Duration:** 6 min
- **Started:** 2026-04-30T09:16:58Z
- **Completed:** 2026-04-30T09:23:19Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments

- Added `allow = [...]` TOML support and mapped it into `RuleOptions.allow` without changing existing `allow_files`, `deny`, `max`, or `forbidden_imports` behavior.
- Added Go parser-backed string literal facts for interpreted and raw string literals while keeping import path literals out of `StringLiteralFact`.
- Added TS/JS regex literal reporting as syntax-level `StringLiteralFact` values such as `/legacy-testid/` and `/^unsafe-/i`.

## Task Commits

Each task was committed atomically. TDD tasks include RED and GREEN commits.

1. **Task 1: Add exact literal allow-list config support**
   - `6cbcc08` test: add failing tests for literal allow config
   - `a75b2d5` feat: add literal allow config support
2. **Task 2: Extract Go string literal facts**
   - `1b5abba` test: add failing tests for Go string literal facts
   - `e345b7e` feat: extract Go string literal facts
3. **Task 3: Extract TS/JS regex literals as syntax-level literal facts**
   - `9329f2a` test: add failing tests for TS regex literal facts
   - `e002f07` feat: extract TS regex literal facts
4. **Verification cleanup**
   - `c92f1a1` style: format TS regex literal changes

## Files Created/Modified

- `crates/polint-core/src/lib.rs` - Added `RuleOptions.allow`.
- `crates/polint-config/src/lib.rs` - Added serde-defaulted `RuleConfig.allow` and config parsing/default tests.
- `crates/polint-rules/src/lib.rs` - Mapped config literal allow values into `RuleOptions`.
- `crates/polint-go/src/lib.rs` - Added Go string literal extraction and tests excluding import path duplication.
- `crates/polint-ts/src/lib.rs` - Added TS/JS regex literal extraction and RuleCtx visibility tests.
- `.planning/phases/06-sdk-and-example-rules/06-02-SUMMARY.md` - Execution summary.

## Decisions Made

- Kept literal allow-list support as a narrow additive config field rather than overloading `allow_files`.
- Used existing parser traversal and literal unquote/source-span helpers instead of whole-file line scanning.
- Preserved TS/JS regex literal source text exactly, including leading slash and flags, and did not add regex semantic evaluation.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Applied rustfmt after verification found formatting drift**
- **Found during:** Plan-wide verification
- **Issue:** `cargo fmt -- --check` failed on the TS regex literal import/helper formatting.
- **Fix:** Ran `cargo fmt` and committed the formatting-only changes.
- **Files modified:** `crates/polint-ts/src/lib.rs`
- **Verification:** `cargo fmt -- --check` passed after formatting.
- **Committed in:** `c92f1a1`

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Formatting-only cleanup required for verification. No behavior or scope change.

## Issues Encountered

- None beyond the formatting cleanup documented above.

## Known Stubs

None. The stub scan only matched intentional Go test fixture table literals such as `cases := []struct`, not placeholder runtime data.

## User Setup Required

None - no external service configuration required.

## Verification

Passed:

- `cargo fmt -- --check`
- `cargo test -p polint-config --lib allow_list`
- `cargo test -p polint-rules --lib rule_options_from_config_maps_literal_allow_list`
- `cargo test -p polint-go --lib string_literal`
- `cargo test -p polint-ts --lib regex_literal`
- `cargo clippy -p polint-core -p polint-config -p polint-rules -p polint-go -p polint-ts --all-targets -- -D warnings`

## Next Phase Readiness

Plan 06-03 can build the literal-based example rules on top of exact literal allow values plus Go and TS/JS literal facts exposed through `RuleCtx::string_literals()`.

## Self-Check: PASSED

- Confirmed created summary file exists.
- Confirmed all modified source files exist.
- Confirmed task and verification cleanup commits exist: `6cbcc08`, `a75b2d5`, `1b5abba`, `e345b7e`, `9329f2a`, `e002f07`, `c92f1a1`.

---
*Phase: 06-sdk-and-example-rules*
*Completed: 2026-04-30*

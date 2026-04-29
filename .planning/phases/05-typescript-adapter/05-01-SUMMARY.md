---
phase: 05-typescript-adapter
plan: "01"
subsystem: typescript-adapter
tags: [rust, oxc, parser-diagnostics, borrowed-source, typescript]

requires:
  - phase: 03-core-facts-and-diagnostics
    provides: AnalysisDb fact storage, Arc-backed SourceFile source, and byte-range span conversion
  - phase: 04-go-adapter
    provides: Adapter pattern for parser diagnostics and best-effort extraction after syntax errors
provides:
  - Oxc parser errors surfaced as stable parser/ts diagnostics
  - Best-effort TS import extraction continues after recoverable parser errors
  - Borrowed-source Oxc parse entry using SourceFile.source Arc text
  - AST helper boundaries for source type, program extraction, export detection, and Oxc span conversion
affects: [phase-05-typescript-adapter, phase-06-sdk-rules, phase-08-ci-output]

tech-stack:
  added: []
  patterns:
    - Parser diagnostics are converted from Oxc labels through core span_from_byte_range
    - TS parser entry clones SourceFile handles, not full source strings
    - Oxc module import/export spans feed ImportFact spans before lexical fallback helpers

key-files:
  created:
    - .planning/phases/05-typescript-adapter/05-01-SUMMARY.md
  modified:
    - crates/polint-ts/src/lib.rs

key-decisions:
  - "Kept parser/ts diagnostics local to polint-ts and used the stable TS/JS parser syntax-error prefix."
  - "Parsed TS-family files from SourceFile.source as borrowed Arc-backed text instead of cloning full source strings."
  - "Introduced narrow Oxc helper boundaries while preserving lexical extraction for fact families not yet AST-backed."

patterns-established:
  - "parse_ts_file returns per-file parser diagnostics while mutating AnalysisDb facts."
  - "span_from_oxc is the adapter-local bridge from Oxc byte spans to core Span values."
  - "extract_from_program is the single parser-backed dispatch point for TS fact extraction."

requirements-completed: [TS-01, TEST-01]

duration: 10min
completed: 2026-04-29
---

# Phase 05 Plan 01: TypeScript Adapter Foundation Summary

**Oxc-backed TS/JS parsing now emits parser/ts diagnostics, preserves recoverable import facts, and parses borrowed SourceFile text without full-source cloning.**

## Performance

- **Duration:** 10 min
- **Started:** 2026-04-29T16:28:46Z
- **Completed:** 2026-04-29T16:39:13Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments

- Converted Oxc parser errors into `parser/ts` diagnostics with the stable `TS/JS parser reported a syntax error` prefix and label-derived ranges.
- Kept best-effort extraction running after parser errors so recoverable malformed TS still records import facts.
- Reworked the parser path to borrow from `SourceFile.source` via `Arc<str>` instead of cloning the full source string.
- Added `parse_source_type`, `extract_from_program`, `statement_exported`, and `span_from_oxc` helper boundaries for later AST-backed extraction plans.
- Moved module import/export facts onto Oxc statement spans while preserving lexical fallback for unsupported or non-AST import forms.

## Task Commits

1. **Task 1 RED:** `f635938` test(05-01): add failing TS parser diagnostic tests
2. **Task 1 GREEN:** `b3ea389` feat(05-01): emit TS parser diagnostics
3. **Task 2 RED:** `71415ab` test(05-01): add failing borrowed-source TS tests
4. **Task 2 GREEN:** `b36227b` feat(05-01): borrow TS source for Oxc parsing

## Files Created/Modified

- `crates/polint-ts/src/lib.rs` - Added parser diagnostics, borrowed-source parsing, Oxc helper boundaries, AST import/export span extraction, and focused unit tests.
- `.planning/phases/05-typescript-adapter/05-01-SUMMARY.md` - Execution record for this plan.

## Verification

- `cargo test -p polint-ts --lib reports_oxc_parser_errors_as_parser_ts_diagnostics` - passed
- `cargo test -p polint-ts --lib clean_ts_family_sources_do_not_emit_parser_ts` - passed
- `cargo test -p polint-ts --lib continues_best_effort_ast_extraction_after_oxc_parse_error` - passed
- `cargo test -p polint-ts --lib parses_ts_source_from_shared_arc_without_full_source_clone` - passed
- `cargo test -p polint-ts --lib source_type_comes_from_file_path_for_ts_family` - passed
- `cargo test -p polint-ts --lib ast_helpers_preserve_source_byte_spans` - passed
- `cargo fmt -- --check` - passed
- `cargo test -p polint-ts --lib parser` - passed, 2 tests
- `cargo test -p polint-ts --lib oxc` - passed, 2 tests
- `cargo clippy -p polint-ts --all-targets -- -D warnings` - passed

## Decisions Made

- Kept `parser/ts` diagnostics in the adapter rather than changing core diagnostic behavior.
- Used `SourceType::from_path(path).unwrap_or_default()` behind a local helper so TS, TSX, JS, and JSX parsing follows file extension.
- Replaced import/export module specifier spans with Oxc declaration spans now, while leaving functions, strings, JSX attributes, and CommonJS fallback lexical until later Phase 5 plans.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Kept extraction running after panicked parser returns**
- **Found during:** Task 1 (Emit parser/ts diagnostics from Oxc parser errors)
- **Issue:** An early implementation returned immediately when `parsed.panicked && parsed.program.body.is_empty()`, which prevented the required best-effort import extraction path.
- **Fix:** Preserved the fallback parser diagnostic but continued through extraction helpers.
- **Files modified:** `crates/polint-ts/src/lib.rs`
- **Verification:** `cargo test -p polint-ts --lib continues_best_effort_ast_extraction_after_oxc_parse_error`
- **Committed in:** `b3ea389`

**2. [Rule 1 - Bug] Fixed source-inspection tests that matched their own literals**
- **Found during:** Task 2 (Parse borrowed source and establish AST helper boundaries)
- **Issue:** The new tests initially embedded exact implementation strings, so they could pass or fail by matching the test body instead of production code.
- **Fix:** Built searched strings from fragments inside tests and adjusted the malformed fixture to an Oxc-recoverable syntax error.
- **Files modified:** `crates/polint-ts/src/lib.rs`
- **Verification:** Task 2 focused tests and `cargo clippy -p polint-ts --all-targets -- -D warnings`
- **Committed in:** `b36227b`

---

**Total deviations:** 2 auto-fixed (2 bug fixes)
**Impact on plan:** Both fixes were required to make the planned tests meaningful and preserve the intended parser-recovery behavior. No scope was added beyond the adapter foundation.

## Issues Encountered

- `cargo fmt -- --check` found rustfmt layout drift after Task 2 implementation. Applied `cargo fmt` before committing `b36227b`.

## Known Stubs

None - stub scan returned no matches in files modified by this plan.

## Threat Flags

None - modified surface stayed within the planned repository-source-to-Oxc parser, Oxc-diagnostic-to-polint-diagnostic, and TS-adapter-to-core-DB trust boundaries.

## Auth Gates

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for `05-02`: the TS adapter now has a borrowed Oxc parser entry, controlled parser diagnostics, and helper boundaries for replacing remaining lexical fact families with AST-backed extraction.

---
*Phase: 05-typescript-adapter*
*Completed: 2026-04-29*

## Self-Check: PASSED

- Summary file exists at `.planning/phases/05-typescript-adapter/05-01-SUMMARY.md`.
- Verified task commits exist: `f635938`, `b3ea389`, `71415ab`, `b36227b`.
- Stub scan of files modified by this plan returned no matches.

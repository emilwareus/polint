---
phase: 05-typescript-adapter
plan: "03"
subsystem: typescript-adapter
tags: [rust, oxc, ast-traversal, jsx, complexity, import-graph]

requires:
  - phase: 05-typescript-adapter
    provides: Oxc parser entry, parser/ts diagnostics, borrowed source parsing, parser-backed imports, functions, classes, methods, component heuristics, and calls
provides:
  - Oxc AST-backed TS/JS string literal and template literal facts
  - Oxc AST-backed JSX attribute facts with stable simple expression values
  - Oxc AST-backed TS/JS cyclomatic complexity for functions, arrows, and methods
  - Unit proof that parser-backed TS import facts feed polint-graph ImportGraph
affects: [phase-05-typescript-adapter, phase-06-sdk-rules, phase-08-ci-output]

tech-stack:
  added:
    - polint-graph dev-dependency for polint-ts unit tests
  patterns:
    - Dynamic template literals emit only non-empty static quasi facts, not synthetic exact combined values
    - JSX spread attributes are traversed for nested literal expressions but do not create JsxAttributeFact records
    - TS/JS complexity is counted from AST control-flow nodes, not comments or string contents

key-files:
  created:
    - .planning/phases/05-typescript-adapter/05-03-SUMMARY.md
  modified:
    - Cargo.lock
    - crates/polint-ts/Cargo.toml
    - crates/polint-ts/src/lib.rs

key-decisions:
  - "Used Oxc AST traversal for literal, JSX, and complexity extraction instead of byte, word, or whitespace scanning."
  - "Kept dynamic template literals honest by recording static quasi text only."
  - "Added polint-graph as a polint-ts dev-dependency solely to prove ImportGraph consumption in unit tests."

patterns-established:
  - "extract_literals_and_jsx walks program statements in source order and pushes StringLiteralFact and JsxAttributeFact through AnalysisDb."
  - "ts_cyclomatic_complexity and arrow_cyclomatic_complexity start at 1 and add AST-derived control-flow increments."
  - "Import graph proof mirrors the Go adapter pattern by using ImportGraph::from_db in adapter unit tests."

requirements-progress: [TS-02, TS-03, TEST-01]

duration: interrupted/resumed
completed: 2026-04-30
---

# Phase 05 Plan 03: TypeScript Adapter Literal, JSX, Complexity, and Import Graph Summary

**Completed the remaining parser-backed Phase 5 TS/JS fact extraction and unit proof.**

## Accomplishments

- Added Oxc AST-backed extraction for string literals, static template literals, static template quasis from dynamic templates, tagged template quasis, JSX attributes, and JSX expression values where a stable simple string is practical.
- Added AST-backed TS/JS cyclomatic complexity for function declarations, function expressions, arrow functions, and class methods.
- Proved comments and string literal contents do not affect TS/JS complexity.
- Proved parser-backed TS import facts feed `polint_graph::ImportGraph::from_db`.
- Kept the graph crate unchanged; only a `polint-graph` dev-dependency was added to `polint-ts` tests.

## Task Commits

1. **Task 1 RED:** `232157d` test(05-03): add failing TS literal and JSX fact tests
2. **Task 1 GREEN:** `8fd3ce1` feat(05-03): extract TS literals and JSX attributes from Oxc AST
3. **Task 2 RED:** `637885e` test(05-03): add failing TS complexity and import graph tests
4. **Task 2 GREEN:** `0ebf250` feat(05-03): compute TS complexity from Oxc AST

## Files Created/Modified

- `crates/polint-ts/src/lib.rs` - Added AST literal/JSX traversal, AST complexity helpers, and focused unit tests.
- `crates/polint-ts/Cargo.toml` - Added `polint-graph` as a dev-dependency for the import graph unit proof.
- `Cargo.lock` - Recorded the `polint-ts` dev-dependency edge.
- `.planning/phases/05-typescript-adapter/05-03-SUMMARY.md` - Execution record for this plan.

## Verification

- `cargo test -p polint-ts --lib extracts_string_literals_and_static_templates_from_oxc_ast` - passed
- `cargo test -p polint-ts --lib extracts_jsx_attributes_from_oxc_ast` - passed
- `cargo test -p polint-ts --lib raw_color_literals_are_available_from_strings_and_jsx_attributes` - passed
- `cargo test -p polint-ts --lib computes_ts_complexity_from_oxc_control_flow` - passed
- `cargo test -p polint-ts --lib ts_complexity_does_not_count_words_inside_strings_or_comments` - passed
- `cargo test -p polint-ts --lib ts_import_facts_feed_import_graph` - passed
- `cargo test -p polint-ts --lib literals` - passed, 2 tests
- `cargo test -p polint-ts --lib complexity` - passed, 2 tests
- `cargo test -p polint-ts --lib import_graph` - passed, 1 test
- `cargo test -p polint-ts --lib` - passed, 15 tests
- `cargo fmt -- --check` - passed
- `cargo clippy -p polint-ts --all-targets -- -D warnings` - passed
- `rg -n "extract_literals_and_jsx|walk_expression_for_literals|push_string_literal_from_oxc|extract_jsx_element_attributes|JSXAttributeItem|TemplateLiteral" crates/polint-ts/src/lib.rs` - passed
- `! rg -n "while idx < bytes\\.len\\(\\)|line\\.split_whitespace\\(\\)|trim_matches\\(\\|ch\\| ch == '\\\"'" crates/polint-ts/src/lib.rs` - passed
- `rg -n "ts_cyclomatic_complexity|arrow_cyclomatic_complexity|complexity_from_statements|complexity_from_expression|is_and\\(\\)|is_or\\(\\)|ImportGraph::from_db" crates/polint-ts/src/lib.rs` - passed
- `! rg -n "fn cyclomatic_complexity|fn count_word|matches\\(\\\"&&\\\"\\)|matches\\(\\\"\\|\\|\\\"\\)" crates/polint-ts/src/lib.rs` - passed

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added polint-graph as a polint-ts dev-dependency**
- **Found during:** Task 2 (Compute TS/JS complexity from AST constructs and prove import graph facts)
- **Issue:** The plan required a `polint-ts` unit test using `polint_graph::ImportGraph::from_db`, but `polint-ts` did not depend on `polint-graph`.
- **Fix:** Added `polint-graph = { path = "../polint-graph" }` under `[dev-dependencies]`, matching the Go adapter's import graph test pattern.
- **Files modified:** `crates/polint-ts/Cargo.toml`, `Cargo.lock`
- **Verification:** `cargo test -p polint-ts --lib ts_import_facts_feed_import_graph`
- **Committed in:** `637885e`

---

**Total deviations:** 1 auto-fixed (1 blocking test-infrastructure fix)
**Impact on plan:** The dependency is test-only and does not change production graph behavior, CLI graph commands, DOT formatting, or module resolution.

## Issues Encountered

- Clippy rejected an intermediate widened `push_ts_function` signature. The implementation now uses `TsFunctionSpec` so complexity, calls, and component metadata remain grouped without violating the lint budget.
- Execution was interrupted after Task 1; Task 2 was completed inline through the sequential GSD fallback on `main`.

## Known Stubs

None - stub scan returned no matches in files modified by this plan.

## Threat Flags

None - modified surface stayed within parser-backed local facts and did not add resolver behavior, graph commands, CSS parsing, or exact semantic claims.

## Auth Gates

None.

## User Setup Required

None.

## Next Phase Readiness

Ready for `05-04`: TS/JS parser diagnostics, imports/exports, declarations, classes, methods, components, literals, JSX attributes, calls, complexity, and import graph fact proof are covered by adapter unit tests. The next plan can focus on fixtures, CLI integration tests, and full workspace verification.

---
*Phase: 05-typescript-adapter*
*Completed: 2026-04-30*

## Self-Check: PASSED

- Summary file exists at `.planning/phases/05-typescript-adapter/05-03-SUMMARY.md`.
- Verified task commits exist: `232157d`, `8fd3ce1`, `637885e`, `0ebf250`.
- Stub scan of files modified by this plan returned no matches.

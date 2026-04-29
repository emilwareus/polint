---
phase: 05-typescript-adapter
plan: "02"
subsystem: typescript-adapter
tags: [rust, oxc, ast-traversal, typescript, core-facts]

requires:
  - phase: 03-core-facts-and-diagnostics
    provides: AnalysisDb fact storage, RuleCtx queries, Capabilities, and byte-range span conversion
  - phase: 05-typescript-adapter
    provides: Oxc parser entry, parser/ts diagnostics, borrowed source parsing, and span_from_oxc helper
provides:
  - Narrow TsClassFact core contract with AnalysisDb, RuleCtx, and Capabilities access
  - Oxc AST-backed TS/JS import and export-from module specifier extraction
  - Oxc AST-backed function, arrow declaration, class, method, component, and call facts
  - Parser-recovery import fallback through Oxc module records
affects: [phase-05-typescript-adapter, phase-06-sdk-rules, phase-08-ci-output]

tech-stack:
  added: []
  patterns:
    - TsClassFact is an additive core fact rather than overloading FunctionFact or TsComponentFact
    - TS adapter extraction starts from Program.body and uses Oxc declarations, classes, methods, and CallExpression nodes
    - Component facts are explicitly labeled as a syntax-level component heuristic

key-files:
  created:
    - .planning/phases/05-typescript-adapter/05-02-SUMMARY.md
  modified:
    - crates/polint-core/src/lib.rs
    - crates/polint-ts/src/lib.rs

key-decisions:
  - "Added a narrow TsClassFact public contract with no class IDs, inheritance graph, resolver, or type information."
  - "Kept TS/JS module specifiers syntactic and parser-backed; no production Node or TypeScript resolution was added."
  - "Used Oxc module records only as a parser-backed fallback to preserve best-effort imports after unrecoverable parser errors."

patterns-established:
  - "push_ts_function returns the FunctionId used by TsComponentFact for function-backed component facts."
  - "FunctionFact.calls is sorted and deduped after collecting Oxc CallExpression callee names."
  - "Class methods are represented as FunctionFact names in ClassName.methodName form while classes use TsClassFact."

requirements-progress: [TS-02, TEST-01]

duration: 13min
completed: 2026-04-29
---

# Phase 05 Plan 02: TypeScript Adapter AST Facts Summary

**Oxc AST-backed TS/JS imports, functions, classes, methods, component heuristics, and call facts with a narrow core TsClassFact contract.**

## Performance

- **Duration:** 13 min
- **Started:** 2026-04-29T16:42:42Z
- **Completed:** 2026-04-29T16:55:20Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Added `TsClassFact`, append-only class fact storage, `AnalysisDb::push_ts_class`, `AnalysisDb::ts_classes`, `RuleCtx::ts_classes`, and `Capabilities::ts_classes`.
- Replaced line-oriented TS/JS import, export-from, declaration, class, component, and call extraction with Oxc AST traversal.
- Added parser-backed class facts and method functions, including `Dialog.render` style method names.
- Extracted call names from Oxc `CallExpression` nodes and stored sorted/deduped calls on `FunctionFact.calls`.
- Kept component detection honest with the `syntax-level component heuristic` code phrase and syntax-only PascalCase/JSX-return checks.

## Task Commits

1. **Task 1 RED:** `c5b6147` test(05-02): add failing TS class fact tests
2. **Task 1 GREEN:** `c610bf5` feat(05-02): add TS class fact core contract
3. **Task 2 RED:** `1199f0f` test(05-02): add failing Oxc AST TS extraction tests
4. **Task 2 GREEN:** `e13a70a` feat(05-02): extract TS syntax facts from Oxc AST

## Files Created/Modified

- `crates/polint-core/src/lib.rs` - Added `TsClassFact`, class fact storage/accessors, `RuleCtx::ts_classes`, `Capabilities::ts_classes`, and focused core tests.
- `crates/polint-ts/src/lib.rs` - Added Oxc AST import/export/declaration/class/method/component/call extraction, parser-recovery module-record fallback, and focused adapter tests.
- `.planning/phases/05-typescript-adapter/05-02-SUMMARY.md` - Execution record for this plan.

## Verification

- `cargo test -p polint-core --lib analysis_db_exposes_ts_class_facts` - passed
- `cargo test -p polint-core --lib rule_ctx_exposes_ts_classes` - passed
- `cargo test -p polint-core --lib capabilities_expose_ts_classes` - passed
- `cargo test -p polint-ts --lib extracts_imports_and_export_from_specifiers_from_oxc_ast` - passed
- `cargo test -p polint-ts --lib extracts_functions_arrows_classes_methods_and_calls_from_oxc_ast` - passed
- `cargo test -p polint-ts --lib detects_component_like_ts_facts_with_honest_heuristics` - passed
- `cargo test -p polint-ts --lib` - passed, 9 tests
- `cargo fmt -- --check` - passed
- `cargo test -p polint-core --lib ts_class` - passed, 3 tests
- `cargo test -p polint-ts --lib imports` - passed, 1 test
- `cargo test -p polint-ts --lib functions` - passed, 1 test
- `cargo clippy -p polint-core -p polint-ts --all-targets -- -D warnings` - passed

## Decisions Made

- Added only the narrow class fact surface required by Phase 5 and avoided inheritance, semantic type, resolver, or symbol APIs.
- Stored parser-backed module specifiers exactly as written in source rather than resolving Node/TS paths.
- Used Oxc module records as a fallback when parser recovery cannot provide a usable `Program.body`, preserving the Phase 5 parser diagnostic behavior without restoring line-oriented import extraction.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Preserved best-effort imports after unrecoverable parser errors**
- **Found during:** Task 2 (Extract imports, exports, declarations, classes, components, and calls from Oxc AST)
- **Issue:** Replacing the line-oriented fallback meant the existing malformed-source recovery test no longer produced an import fact when `Program.body` was empty.
- **Fix:** Added a parser-backed fallback from Oxc `module_record.requested_modules`, sorted by source span, only when AST import extraction produced no imports for the file.
- **Files modified:** `crates/polint-ts/src/lib.rs`
- **Verification:** `cargo test -p polint-ts --lib continues_best_effort_ast_extraction_after_oxc_parse_error`
- **Committed in:** `e13a70a`

---

**Total deviations:** 1 auto-fixed (1 bug fix)
**Impact on plan:** The fix preserved prior parser-recovery behavior using Oxc parser data and did not reintroduce line-oriented declaration, class, component, or call extraction.

## Issues Encountered

- The prior `ast_helpers_preserve_source_byte_spans` test expected import declaration spans from Plan 05-01. Plan 05-02 explicitly moved import/export facts to module specifier spans, so the test was updated to assert the Oxc string-literal span.
- Clippy rejected an initial `push_ts_function` helper shape for too many arguments and two one-arm matches. Introduced a small `TsAstCtx` and simplified the matches.

## Known Stubs

None - stub scan returned no matches in files modified by this plan.

## Threat Flags

None - modified surface stayed within the planned Oxc AST to core facts, TS adapter to SDK-facing core, and component heuristic boundaries.

## Auth Gates

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for `05-03`: TS/JS imports, export-from specifiers, functions, arrows, classes, methods, component heuristics, and calls now come from Oxc parser-backed facts. Later Phase 5 work can focus on JSX attributes, string/template literals, and complexity without depending on the old declaration extraction path.

---
*Phase: 05-typescript-adapter*
*Completed: 2026-04-29*

## Self-Check: PASSED

- Summary file exists at `.planning/phases/05-typescript-adapter/05-02-SUMMARY.md`.
- Verified task commits exist: `c5b6147`, `c610bf5`, `1199f0f`, `e13a70a`.
- Stub scan of files modified by this plan returned no matches.

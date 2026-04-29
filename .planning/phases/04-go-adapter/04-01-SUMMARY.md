---
phase: 04-go-adapter
plan: "01"
subsystem: go-adapter
tags: [rust, tree-sitter-go, parser-diagnostics, package-facts]

requires:
  - phase: 03-core-facts-and-diagnostics
    provides: Deterministic AnalysisDb IDs, span conversion, and diagnostic contracts
provides:
  - Narrow PackageFact storage in polint-core with insertion-order PackageId assignment
  - Parser-backed Go syntax-error diagnostics with stable parser/go messages and ranges
  - Tree-sitter package clause extraction into core package facts
  - Best-effort package extraction when malformed Go still has valid package subtrees
affects: [phase-04-go-adapter, phase-06-sdk-rules, phase-08-ci-output]

tech-stack:
  added: []
  patterns:
    - Vec-backed append-only AnalysisDb package fact storage
    - Go adapter parser diagnostics derived from tree-sitter error nodes
    - Tree-sitter named-child traversal helpers local to polint-go

key-files:
  created:
    - .planning/phases/04-go-adapter/04-01-SUMMARY.md
  modified:
    - crates/polint-core/src/lib.rs
    - crates/polint-go/src/lib.rs

key-decisions:
  - "Added only the narrow PackageFact core contract needed for Go package names."
  - "Kept Go parser diagnostics local to polint-go and emitted stable parser/go messages for malformed source."
  - "Kept existing import/function extraction in place while moving package extraction to tree-sitter nodes for this foundation plan."

patterns-established:
  - "Package facts follow the existing AnalysisDb push/accessor pattern: callers provide placeholder IDs and core assigns final IDs."
  - "Go parser diagnostics use tree-sitter node spans when available and fall back to line 1, column 1 only if no error node exists."

requirements-completed: [GO-01]
requirements-progress: [GO-02, TEST-01]

duration: 8min
completed: 2026-04-29
---

# Phase 04 Plan 01: Go Adapter Foundation Summary

**Tree-sitter-backed Go parser diagnostics and package-name facts now feed the core AnalysisDb without broad core refactors.**

## Performance

- **Duration:** 8 min
- **Started:** 2026-04-29T05:10:39Z
- **Completed:** 2026-04-29T05:18:50Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Added `PackageFact`, `AnalysisDb::push_package`, and `AnalysisDb::packages()` with deterministic insertion-order `PackageId` assignment.
- Refactored the Go adapter entry path to parse borrowed `&str` source with `tree-sitter-go` and emit `parser/go` diagnostics when the parse tree has errors.
- Added tree-sitter helpers for node spans, node text, deterministic named-child traversal, first error node lookup, and package clause extraction.
- Preserved best-effort extraction after parser errors so valid `package payment` subtrees still produce package facts.

## Task Commits

1. **Task 1 RED:** `2730302` test(04-01): add failing package fact tests
2. **Task 1 GREEN:** `a952414` feat(04-01): add package facts to core
3. **Task 2 RED:** `f9f2868` test(04-01): add failing Go parser foundation tests
4. **Task 2 GREEN:** `215e733` feat(04-01): parse Go packages with tree-sitter
5. **Verification cleanup:** `63546ca` test(04-01): cover parser verification filter

## Files Created/Modified

- `crates/polint-core/src/lib.rs` - Added package fact storage, insertion-order ID assignment, accessor, and core unit tests.
- `crates/polint-go/src/lib.rs` - Added parser diagnostics, tree-sitter package extraction helpers, parser/package tests, and plan-level parser verification coverage.
- `.planning/phases/04-go-adapter/04-01-SUMMARY.md` - Execution record for this plan.

## Verification

- `cargo test -p polint-core --lib analysis_db_assigns_package_ids_deterministically` - passed
- `cargo test -p polint-core --lib analysis_db_exposes_package_facts` - passed
- `cargo test -p polint-go --lib reports_tree_sitter_parse_errors_with_stable_range` - passed
- `cargo test -p polint-go --lib continues_best_effort_package_extraction_after_parse_error` - passed
- `cargo test -p polint-go --lib extracts_go_package_name_from_tree_sitter` - passed
- `cargo check -p polint-go -p polint-ts -p polint-rules` - passed
- `cargo fmt -- --check` - passed
- `cargo test -p polint-core --lib package` - passed, 2 tests
- `cargo test -p polint-go --lib parser` - passed, 1 test
- `cargo clippy -p polint-core -p polint-go --all-targets -- -D warnings` - passed

## Decisions Made

- Kept the core change additive and narrow: no query framework, package graph, resolver, or semantic sidecar was introduced.
- Used `tree_sitter_go::LANGUAGE.into()` as the parser source of truth for Go package extraction and parser diagnostics.
- Left the existing import/function line-oriented extraction untouched for later Phase 4 plans; this plan only moved the parser entry and package extraction path.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed tree-sitter child index type mismatch**
- **Found during:** Task 2 (Parse Go through tree-sitter and emit parser diagnostics)
- **Issue:** The new named-child traversal helper initially passed a `usize` index to `Node::named_child`, which expects `u32`.
- **Fix:** Iterated named-child indexes as `u32` in the traversal helpers.
- **Files modified:** `crates/polint-go/src/lib.rs`
- **Verification:** `cargo test -p polint-go --lib reports_tree_sitter_parse_errors_with_stable_range`; final clippy passed.
- **Committed in:** `215e733`

**2. [Rule 3 - Blocking] Made the plan-level parser test command exercise real tests**
- **Found during:** Overall verification
- **Issue:** `cargo test -p polint-go --lib parser` passed while running zero tests because the required parser tests used `parse`, not `parser`, in their names.
- **Fix:** Added a parser-named wrapper test that invokes the three required parser/package behavior tests.
- **Files modified:** `crates/polint-go/src/lib.rs`
- **Verification:** `cargo test -p polint-go --lib parser` now runs and passes 1 test.
- **Committed in:** `63546ca`

---

**Total deviations:** 2 auto-fixed (1 bug, 1 blocking verification issue)
**Impact on plan:** Both changes preserved the planned scope and strengthened the required verification path.

## Issues Encountered

- The documented plan-level Go parser test filter initially ran zero tests; resolved by adding the verification wrapper in `63546ca`.

## Known Stubs

None - stub scan returned no matches in files modified by this plan.

## Auth Gates

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for `04-02`: the Go adapter now has parser-backed package facts and explicit parser diagnostics, so import/declaration/call/test/complexity extraction can build on the same tree-sitter traversal approach.

---
*Phase: 04-go-adapter*
*Completed: 2026-04-29*

## Self-Check: PASSED

- Summary file exists at `.planning/phases/04-go-adapter/04-01-SUMMARY.md`.
- Verified task commits exist: `2730302`, `a952414`, `f9f2868`, `215e733`, `63546ca`.
- Stub scan of source files modified by this plan returned no matches.

---
phase: 06-sdk-and-example-rules
plan: "01"
subsystem: sdk
tags: [rust, sdk, rulectx, scaffolding, tdd]

# Dependency graph
requires:
  - phase: 03-core-facts-and-diagnostics
    provides: core rule traits, fact models, AnalysisDb, diagnostics, and runner behavior
  - phase: 05-typescript-adapter
    provides: TS/JS class, component, literal, JSX, and import facts
provides:
  - documented Rule, RuleMeta, Capabilities, RuleOptions, and RuleCtx authoring contract
  - borrowed RuleCtx query helpers for Phase 3-5 fact families and import edges
  - public polint-sdk prelude exports for normal rule authoring
  - SDK-oriented polint new-rule templates for Go, TS/JS, and generic rules
affects: [06-sdk-and-example-rules, polint-core, polint-sdk, polint-cli]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - borrowed RuleCtx iterators over AnalysisDb fact vectors
    - SDK-first generated rule templates using polint_sdk::prelude

key-files:
  created:
    - .planning/phases/06-sdk-and-example-rules/06-01-SUMMARY.md
  modified:
    - crates/polint-core/src/lib.rs
    - crates/polint-sdk/src/lib.rs
    - crates/polint-cli/src/main.rs
    - crates/polint-cli/tests/cli.rs

key-decisions:
  - "Kept the core Rule and RuleCtx contract additive while exposing new borrowed helper methods."
  - "Returned Vec<&TestFact> only for go_tests_for_related_file because it combines same-file and companion borrowed references."
  - "Kept polint new-rule scaffolds honest: SDK helper examples only, no dynamic loading claims."

patterns-established:
  - "RuleCtx file-scoped helpers return borrowed iterators over AnalysisDb order."
  - "SDK compile smoke tests use only crate::prelude::* to prevent accidental direct core imports."
  - "CLI scaffold tests assert exact generated helper calls and absence of polint_core imports."

requirements-completed: [SDK-01, SDK-02, TEST-01]

# Metrics
duration: 7 min
completed: 2026-04-30
---

# Phase 06 Plan 01: SDK Entry Point and Query Helpers Summary

**SDK-first RuleCtx helpers, prelude exports, and new-rule scaffolds for borrowed fact queries**

## Performance

- **Duration:** 7 min
- **Started:** 2026-04-30T09:04:42Z
- **Completed:** 2026-04-30T09:11:49Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Documented the core rule-authoring types and added RuleCtx helpers for packages, files, functions, imports, branch obligations, Go tests, TS components/classes, string literals, JSX attributes, source files, related Go tests, and import edges.
- Expanded `polint-sdk::prelude::*` to cover the public rule-authoring surface, including `PackageFact` and `TsClassFact`, with a compile smoke test that uses only the prelude.
- Updated `polint new-rule` templates so generated Go, TS/JS, and generic rules demonstrate the SDK helpers and keep `custom/{rule_name}` IDs.

## Task Commits

Each task was committed atomically. TDD tasks include RED and GREEN commits.

1. **Task 1: Add documented RuleCtx query helpers**
   - `3ed0bb8` test: add failing tests for RuleCtx helpers
   - `ae5dda9` feat: add RuleCtx SDK query helpers
2. **Task 2: Expand polint-sdk prelude and compile coverage**
   - `1786333` test: add failing SDK prelude smoke test
   - `5ad31eb` feat: expand SDK prelude exports
3. **Task 3: Align new-rule templates with the SDK helpers**
   - `8750b7c` test: add failing new-rule SDK helper assertions
   - `4da325e` feat: align new-rule templates with SDK helpers
4. **Verification cleanup**
   - `d72484b` style: format SDK helper changes

## Files Created/Modified

- `crates/polint-core/src/lib.rs` - RuleCtx helper methods, rustdoc, and focused core tests.
- `crates/polint-sdk/src/lib.rs` - crate docs, prelude exports, and SDK compile smoke test.
- `crates/polint-cli/src/main.rs` - SDK-oriented generated rule template examples.
- `crates/polint-cli/tests/cli.rs` - new-rule scaffold assertions for Go, TS/JS, and generic rules.
- `.planning/phases/06-sdk-and-example-rules/06-01-SUMMARY.md` - execution summary.

## Decisions Made

- Kept the SDK surface additive and source-compatible with the existing core trait and context shape.
- Used borrowed iterators for per-file helper methods so SDK convenience does not clone fact vectors.
- Used `go_tests_for_related_file(file) -> Vec<&TestFact>` for the only combining helper, preserving borrowed facts while gathering same-file and companion `_test.go` evidence.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Applied rustfmt after final verification found formatting drift**
- **Found during:** Plan-wide verification
- **Issue:** `cargo fmt -- --check` failed on the new RuleCtx helper tests and CLI template match arms.
- **Fix:** Ran `cargo fmt` and committed the formatting-only changes.
- **Files modified:** `crates/polint-core/src/lib.rs`, `crates/polint-cli/src/main.rs`
- **Verification:** `cargo fmt -- --check` passed after formatting.
- **Committed in:** `d72484b`

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Formatting-only cleanup required for verification. No scope change.

## Issues Encountered

- The first final `cargo fmt -- --check` failed after task commits. Formatting was applied and committed separately; all final checks passed.

## Known Stubs

None. The stub scan only matched existing test fixture TOML snippets such as `rules = []` and `exclude = []`, which are intentional test inputs rather than runtime stubs.

## User Setup Required

None - no external service configuration required.

## Verification

Passed:

- `cargo fmt -- --check`
- `cargo test -p polint-core --lib rule_ctx`
- `cargo test -p polint-sdk --lib`
- `cargo test -p polint-cli --test cli new_rule`
- `cargo clippy -p polint-core -p polint-sdk -p polint-cli --all-targets -- -D warnings`

## Next Phase Readiness

The public SDK entry point and scaffolding surface are ready for the next Phase 6 plan. Built-in example rules can now use the documented RuleCtx helpers instead of reaching through `AnalysisDb` directly.

## Self-Check: PASSED

- Confirmed created summary file exists.
- Confirmed all modified source/test files exist.
- Confirmed task and verification cleanup commits exist: `3ed0bb8`, `ae5dda9`, `1786333`, `5ad31eb`, `8750b7c`, `4da325e`, `d72484b`.

---
*Phase: 06-sdk-and-example-rules*
*Completed: 2026-04-30*

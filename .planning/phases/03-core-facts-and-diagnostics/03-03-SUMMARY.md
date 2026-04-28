---
phase: 03-core-facts-and-diagnostics
plan: "03"
subsystem: core-facts-diagnostics
tags: [rust, polint-fs, polint-cli, deterministic-output, diagnostics, proptest]

requires:
  - phase: 02-cli-config-and-discovery
    provides: CLI/config/discovery loop with `.gitignore`, include/exclude, supported language filtering, and JSON output
  - phase: 03-core-facts-and-diagnostics
    provides: Plans 03-01 and 03-02 hardened core facts, rule runner determinism, diagnostic identity, and rendering
provides:
  - Deterministic discovery proof for root-relative sorting after filtering
  - AnalysisDb file ID insertion-order proof from discovery output
  - Repeated-run CLI JSON determinism integration coverage
  - Phase 3 requirements and verification reconciliation
affects: [phase-04-go-adapter, phase-05-ts-adapter, phase-07-cache-performance, phase-08-ci-output]

tech-stack:
  added: [proptest workspace dev-dependency for polint-fs]
  patterns:
    - Pure include/exclude decision helper tested with generated paths
    - CLI determinism tests compare parsed JSON values and diagnostic file order
    - JSON diagnostic snapshots pin renderer output while separately verifying parseability

key-files:
  created:
    - .planning/phases/03-core-facts-and-diagnostics/03-03-SUMMARY.md
  modified:
    - Cargo.lock
    - crates/polint-fs/Cargo.toml
    - crates/polint-fs/src/lib.rs
    - crates/polint-cli/tests/cli.rs
    - crates/polint-diagnostics/src/lib.rs
    - .planning/PROJECT.md
    - .planning/REQUIREMENTS.md
    - .planning/ROADMAP.md
    - .planning/VERIFICATION.md

key-decisions:
  - "Kept discovery semantics unchanged and only extracted a testable include/exclude helper."
  - "Treated stable Phase 3 FileIds as deterministic within a run through sorted discovery and insertion order."
  - "Kept TEST-01, TEST-03, and TEST-04 in progress because later Go/TS, SARIF, cache, and broader rule scopes remain."
  - "Snapshot JSON renderer output directly to avoid serde_json feature-dependent Value key reserialization."

patterns-established:
  - "Filesystem tests assert exact normalized relative-path vectors after filtering."
  - "CLI determinism tests run the same temp repository multiple times and compare parsed JSON arrays."
  - "Planning closure docs separate completed source requirements from partial cross-phase test evidence."

requirements-completed: [FS-02, CORE-01, CORE-02, DIAG-01]
requirements-progress: [TEST-01, TEST-03, TEST-04]

duration: 8min
completed: 2026-04-28
---

# Phase 03 Plan 03: Deterministic Discovery and Closure Summary

**Deterministic discovery now has focused filesystem and CLI evidence, and Phase 3 status records reflect only the verified core/diagnostics scope.**

## Performance

- **Duration:** 8 min
- **Started:** 2026-04-28T11:46:20Z
- **Completed:** 2026-04-28T11:54:08Z
- **Tasks:** 3
- **Files modified:** 9

## Accomplishments

- Added filesystem tests for sorted normalized root-relative discovery output, filtering before sorting, and deterministic `AnalysisDb` `FileId` assignment.
- Added a `proptest` property proving include/exclude decisions are stable and explicit excludes win over includes.
- Added a CLI integration test that runs `polint check --profile phase3 --format json --fail-on none` three times over the same mixed temp repo and compares parsed diagnostic arrays plus file order.
- Reconciled Phase 3 planning records: `FS-02`, `CORE-01`, `CORE-02`, and `DIAG-01` are complete; `TEST-01`, `TEST-03`, and `TEST-04` remain in progress with Phase 3 evidence recorded.

## Task Commits

1. **Task 1 RED:** `efa809b` test(03-03): add failing discovery determinism tests
2. **Task 1 GREEN:** `fc1ba18` feat(03-03): prove deterministic file discovery order
3. **Task 2:** `01cac82` test(03-03): cover repeated CLI JSON determinism
4. **Task 3 blocking fix:** `c29dd82` test(03-03): stabilize JSON diagnostic snapshot
5. **Task 3 status:** `97718b9` docs(03-03): reconcile phase 3 status

## Files Created/Modified

- `Cargo.lock` - Locked `proptest` usage for `polint-fs` dev tests.
- `crates/polint-fs/Cargo.toml` - Added `proptest.workspace = true` under dev-dependencies.
- `crates/polint-fs/src/lib.rs` - Added the include/exclude helper and deterministic discovery/order tests.
- `crates/polint-cli/tests/cli.rs` - Added repeated-run parsed JSON determinism coverage.
- `crates/polint-diagnostics/src/lib.rs` - Stabilized the JSON snapshot assertion across workspace feature sets.
- `.planning/PROJECT.md` - Moved verified Phase 3 capabilities into validated status without overclaiming later work.
- `.planning/REQUIREMENTS.md` - Marked verified source requirements complete and scoped broad test requirements honestly.
- `.planning/ROADMAP.md` - Added the completed Phase 3 plan list.
- `.planning/VERIFICATION.md` - Appended Phase 3 verification evidence and no-worktree policy.

## Verification

- `cargo test -p polint-fs --lib discovery_order_is_root_relative_and_stable_with_nested_files` - passed
- `cargo test -p polint-fs --lib discovery_filters_before_sorting` - passed
- `cargo test -p polint-fs --lib load_analysis_files_preserves_discovery_order_in_file_ids` - passed
- `cargo test -p polint-fs --lib discovery_include_exclude_decision_is_stable` - passed
- `cargo test -p polint-cli --test cli check_json_output_is_deterministic_across_repeated_runs` - passed
- `cargo test -p polint-diagnostics --lib render_json_snapshot_is_stable` - passed
- `cargo fmt -- --check` - passed
- `cargo clippy --workspace --all-targets -- -D warnings` - passed
- `cargo test --workspace` - passed
- `rg -n "FS-02|CORE-01|CORE-02|DIAG-01|TEST-01|TEST-03|TEST-04" .planning/REQUIREMENTS.md .planning/PROJECT.md .planning/ROADMAP.md .planning/VERIFICATION.md` - passed

## Decisions Made

- Did not broaden discovery beyond Phase 2 semantics: `.gitignore`, include/exclude globs, default excludes, and supported Go/TS/TSX/JS/JSX extensions remain unchanged.
- Kept CLI determinism coverage focused on parsed JSON diagnostics and observable file ordering, not broader Phase 4/5 fixture breadth or Phase 8 CI command hardening.
- Kept broad test requirements in progress because Phase 3 provides evidence but does not close all later language, cache, SARIF, and command scopes.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Stabilized JSON diagnostic snapshot across workspace feature sets**
- **Found during:** Task 3 (full workspace verification)
- **Issue:** `cargo test --workspace` enabled `serde_json/preserve_order` through workspace dependencies, causing the diagnostic JSON snapshot test to fail on object key ordering after parsing and reserializing `serde_json::Value`.
- **Fix:** Kept parseability verification, then snapshotted the renderer output directly so the test pins the CLI-facing JSON contract.
- **Files modified:** `crates/polint-diagnostics/src/lib.rs`
- **Verification:** `cargo test -p polint-diagnostics --lib render_json_snapshot_is_stable`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` passed.
- **Committed in:** `c29dd82`

**Total deviations:** 1 auto-fixed blocking issue.
**Impact on plan:** Required to complete the planned full workspace verification; no product behavior or public API changed.

## Issues Encountered

- Task 2's new determinism test passed immediately because the existing Phase 3 discovery, `AnalysisDb`, diagnostic sorting, and CLI rendering chain already satisfied the behavior. No implementation change was needed for that task.
- Stub scan matched only requirement wording for "coverage placeholders", an explicit test fixture `exclude = []`, and a roadmap sentence about future TODO documentation. No product stubs were introduced.

## Known Stubs

None - stub scan found no hardcoded UI/data stubs or placeholder implementations introduced by this plan.

## Threat Flags

None - the plan added tests, a pure local path-filtering helper, and planning status updates; it did not introduce new endpoints, auth paths, schema changes, or external trust-boundary surfaces.

## Auth Gates

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 4 and Phase 5 can rely on deterministic file ordering and stable in-run file IDs when adding richer language facts. Phase 7 still owns cache/performance hardening, and Phase 8 still owns production SARIF/CI command hardening.

---
*Phase: 03-core-facts-and-diagnostics*
*Completed: 2026-04-28*

## Self-Check: PASSED

- Summary file exists at `.planning/phases/03-core-facts-and-diagnostics/03-03-SUMMARY.md`.
- Verified task commits exist: `efa809b`, `fc1ba18`, `01cac82`, `c29dd82`, `97718b9`.
- Stub scan found no blocking product stubs; matches were requirement wording, an explicit `exclude = []` test fixture, roadmap wording, and the summary's own stub-scan explanation.

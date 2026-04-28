---
phase: 02-cli-config-and-discovery
plan: 01
subsystem: cli-config-discovery
tags: [rust, cli, config, discovery, tests]

requires: []
provides:
  - Focused Phase 2 CLI integration coverage
  - Narrow config/discovery fixes required by the Phase 2 tests
  - Verification that init, new-rule, check, JSON/SARIF output, profiles, and discovery filters work together
affects: [polint-cli, polint-config, polint-fs]

tech-stack:
  added: []
  patterns:
    - Test the CLI as the user invokes it through assert_cmd
    - Keep JSON stdout parseable and route human guidance to human output only
    - Keep config defaults and explicit empty config values semantically distinct

key-files:
  created:
    - .planning/phases/02-cli-config-and-discovery/02-01-SUMMARY.md
  modified:
    - crates/polint-cli/tests/cli.rs
    - crates/polint-config/src/lib.rs
    - crates/polint-fs/src/lib.rs

key-decisions:
  - "Treated the existing main-branch CLI implementation as the Phase 2 baseline."
  - "Fixed only behavior directly exposed by the Phase 2 CLI/config/discovery tests."
  - "Used explicit per-extension rule file patterns in tests instead of relying on brace expansion semantics."

patterns-established:
  - "Explicit empty workspace excludes mean no excludes, not match all files."
  - "`src/**` style include patterns match direct child files as well as descendants."
  - "File discovery honors `.gitignore` in temporary and non-git repositories."

requirements-completed:
  - CLI-01
  - CLI-02
  - CLI-03
  - CFG-01
  - CFG-02
  - FS-01
  - DIAG-02
requirements-scoped:
  - TEST-02

duration: 18 min
completed: 2026-04-28
---

# Phase 2 Plan 01 Summary

Phase 2 CLI/config/discovery hardening added the focused integration coverage required before closing the phase.

## Performance

- **Duration:** 18 min
- **Completed:** 2026-04-28T09:08:42Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Added integration coverage for `polint init`, non-overwriting init behavior, Go and TS rule skeletons, missing-config defaults, profiles, severity overrides, JSON output, SARIF output, `--no-cache`, `rules.paths`, supported extensions, `.gitignore`, include/exclude filters, and default excludes.
- Fixed config glob behavior so an explicit empty `workspace.exclude` list no longer excludes every file.
- Fixed include glob handling so patterns like `src/**` match direct child files as well as deeper descendants.
- Fixed discovery so the `ignore` walker honors `.gitignore` in temp directories and other non-git roots.

## Commands

- `cargo test -p polint-cli --test cli`
- `git diff --check`

## Result

Passed. `cargo test -p polint-cli --test cli` reports 14 passed tests, including all focused Phase 2 coverage.

## Changes

- `crates/polint-cli/tests/cli.rs` now covers the Phase 2 CLI/config/discovery loop.
- `crates/polint-config/src/lib.rs` distinguishes default include behavior from explicit empty exclude behavior.
- `crates/polint-config/src/lib.rs` expands trailing `/**` patterns to cover direct child files consistently.
- `crates/polint-fs/src/lib.rs` sets `require_git(false)` so `.gitignore` files are honored outside initialized git repositories.

## Task Commits

1. **Task 1: Audit the existing Phase 2 baseline** - no separate commit; covered by source inspection and CLI help checks during execution.
2. **Task 2: Add focused Phase 2 CLI integration coverage** - `6f48e98` (test)
3. **Task 3: Fix behavior exposed by the focused tests** - `7fd7e9a` (fix)

## Files Created/Modified

- `.planning/phases/02-cli-config-and-discovery/02-01-SUMMARY.md` - execution summary for this plan.
- `crates/polint-cli/tests/cli.rs` - Phase 2 integration coverage.
- `crates/polint-config/src/lib.rs` - include/exclude glob behavior fixes.
- `crates/polint-fs/src/lib.rs` - `.gitignore` support for non-git roots.

## Deviations from Plan

- The plan's sample brace glob for supported extensions was replaced with explicit per-extension patterns in the test config because the current glob implementation does not treat brace expansion as a requirement.
- Source fixes were required: explicit empty excludes previously matched all files, `src/**` did not cover direct children, and `.gitignore` was not honored for temp roots without `.git`.

## Issues Encountered

- The first focused test run failed because `exclude = []` excluded all files. The fix keeps default includes as `**/*` while leaving empty excludes empty.

## User Setup Required

None.

## Next Phase Readiness

Plan 02-01 is complete. Plan 02-02 can run full workspace verification and reconcile Phase 2 GSD status records.

## Self-Check: PASSED

- Focused CLI integration suite passed.
- JSON stdout remained parseable in tests.
- Discovery behavior now covers default excludes, explicit include/exclude filters, `.gitignore`, and supported TS/JS extensions.
- No worktree was created or used.

---
*Phase: 02-cli-config-and-discovery*
*Completed: 2026-04-28*

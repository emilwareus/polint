---
phase: 08-ci-output-and-graph-commands
plan: "02"
subsystem: diagnostics
tags: [rust, sarif, ci-output]
requires:
  - phase: 08-ci-output-and-graph-commands
    plan: "01"
    provides: parseable machine-readable CLI stdout
provides:
  - SARIF-like renderer snapshot coverage for CI fields
  - CLI SARIF-like output coverage with fail-on threshold proof
affects: [diagnostics, cli, ci-output]
tech-stack:
  added: []
  patterns:
    - JSON pointer assertions for CI output contracts
key-files:
  created:
    - .planning/phases/08-ci-output-and-graph-commands/08-02-SUMMARY.md
  modified:
    - crates/polint-diagnostics/src/lib.rs
    - crates/polint-cli/tests/cli.rs
key-decisions:
  - "Kept the output described as SARIF-like; no full SARIF certification claim was added."
  - "Snapshot coverage lives in the diagnostics crate, while CLI coverage parses emitted SARIF-like JSON."
patterns-established:
  - "CI output assertions use JSON pointers for version, rule ID, severity, message, fingerprint, and URI fields."
requirements-completed:
  - DIAG-03
  - CLI-05
  - TEST-03
duration: 4 min
completed: 2026-05-01
tasks: 3
files: 2
---

# Phase 8 Plan 02: SARIF-Like CI Output Summary

**SARIF-like output now has renderer snapshot coverage and CLI integration proof for CI fields and fail thresholds.**

## Performance

- **Duration:** 4 min
- **Started:** 2026-05-01T11:29:00Z
- **Completed:** 2026-05-01T11:33:00Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments

- Added `render_sarif_snapshot_includes_ci_fields` to prove version, tool name, rule ID, level, message, fingerprint, and artifact URI fields.
- Added CLI integration coverage for `polint check --format sarif --fail-on none`.
- Added CLI proof that SARIF-like output still honors `--fail-on warn` with exit code 1.

## Task Commits

Each task was committed atomically:

1. **Task 1-3: SARIF-like renderer and CLI output tests** - `d22ed4e` (test)

## Verification

- `cargo test -p polint-diagnostics --lib render_sarif_snapshot_includes_ci_fields` - passed.
- `cargo test -p polint-cli --test cli sarif` - passed, 2 tests.

## Files Created/Modified

- `crates/polint-diagnostics/src/lib.rs` - Adds SARIF-like snapshot and JSON pointer assertions.
- `crates/polint-cli/tests/cli.rs` - Adds CLI SARIF-like CI field and fail-threshold assertions.
- `.planning/phases/08-ci-output-and-graph-commands/08-02-SUMMARY.md` - Records Plan 02 completion evidence.

## Decisions & Deviations

None - the existing renderer already emitted the required SARIF-like fields, so this plan was completed as targeted coverage.

## Next Phase Readiness

Plan 08-03 can focus on deterministic DOT graph export coverage without changing the CI output path.

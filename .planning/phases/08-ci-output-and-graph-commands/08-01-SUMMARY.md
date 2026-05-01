---
phase: 08-ci-output-and-graph-commands
plan: "01"
subsystem: cli
tags: [rust, cli, ci-output]
provides:
  - stable explain command behavior for built-in and unknown rule IDs
  - parseable machine output from test-rules JSON mode
  - documented exit-code contract coverage for fail-on thresholds and fatal config errors
affects: [cli, diagnostics, ci-output]
tech-stack:
  added: []
  patterns:
    - machine-readable formats must not print human prelude text to stdout
key-files:
  created:
    - .planning/phases/08-ci-output-and-graph-commands/08-01-SUMMARY.md
  modified:
    - crates/polint-cli/src/main.rs
    - crates/polint-cli/tests/cli.rs
key-decisions:
  - "Kept the existing `test-rules` human prelude for human output only."
  - "Exit code 2 remains reserved for fatal CLI errors surfaced through the top-level `run()` error handler."
patterns-established:
  - "CLI machine-output tests parse stdout as JSON to catch accidental human text."
requirements-completed:
  - CLI-04
  - CLI-05
  - TEST-02
duration: 5 min
completed: 2026-05-01
tasks: 3
files: 2
---

# Phase 8 Plan 01: CLI Command Contracts Summary

**CLI command surface and exit-code contracts are covered by integration tests, and `test-rules` no longer corrupts machine-readable stdout.**

## Performance

- **Duration:** 5 min
- **Started:** 2026-05-01T11:24:00Z
- **Completed:** 2026-05-01T11:29:00Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments

- Added integration tests for `polint explain` on known and unknown rule IDs.
- Added JSON parse proof for `polint test-rules --format json --fail-on none`.
- Added exit-code proof for `--fail-on warn|error|none` and fatal config parse errors.
- Updated `test-rules` so human prelude text is printed only for `--format human`.

## Task Commits

Each task was committed atomically:

1. **Task 1-3: CLI contract tests and machine-output fix** - `874c9b2` (fix)

## Verification

- `cargo test -p polint-cli --test cli` - passed, 44 tests.

## Files Created/Modified

- `crates/polint-cli/src/main.rs` - Guards the `test-rules` human prelude behind `FormatArg::Human`.
- `crates/polint-cli/tests/cli.rs` - Adds Phase 8 CLI command, JSON stdout, and exit-code integration coverage.
- `.planning/phases/08-ci-output-and-graph-commands/08-01-SUMMARY.md` - Records Plan 01 completion evidence.

## Decisions & Deviations

None - plan intent executed as specified. The test command was run as the full CLI integration target because Cargo accepts only one test-name filter at a time.

## Next Phase Readiness

Plan 08-02 can now build on parseable machine output and add SARIF-like CI output assertions without stdout contamination.

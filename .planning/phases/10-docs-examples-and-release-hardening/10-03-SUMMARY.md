---
phase: 10-docs-examples-and-release-hardening
plan: "03"
subsystem: testing
tags: [cli, integration-tests, fixtures, examples]

requires:
  - phase: 10-docs-examples-and-release-hardening
    provides: "Runnable example directories and hardened example docs"
provides:
  - "Mixed Go/TS fixture CLI integration proof"
  - "Configured example directory CLI smoke tests"
  - "Final property-test traceability evidence"
affects: [release-readiness, cli-tests, examples]

tech-stack:
  added: []
  patterns:
    - "CLI smoke tests write checked-in fixtures/configs into temp repos"
    - "Final test traceability uses existing property tests instead of artificial count-padding"

key-files:
  created: []
  modified:
    - crates/polint-cli/tests/cli.rs

key-decisions:
  - "Phase 10 smoke tests execute checked-in example configs instead of duplicating config text."
  - "Existing property tests remain the traceability source for spans, diagnostic sorting, discovery decisions, and cache keys."

patterns-established:
  - "Use include_str! fixtures plus temp repos for CLI-level release proof."
  - "Use parsed JSON assertions for diagnostics and DOT substring checks for graph smoke coverage."

requirements-completed: [TEST-01, TEST-02, TEST-04]

duration: 4 min
completed: 2026-05-01
---

# Phase 10 Plan 03: CLI Fixture and Example Tests Summary

**CLI integration coverage now proves mixed Go/TS fixtures and the configured Go/TS example directories run through `polint check`.**

## Performance

- **Duration:** 4 min
- **Started:** 2026-05-01T16:11:31Z
- **Completed:** 2026-05-01T16:14:09Z
- **Tasks:** 3
- **Files modified:** 1

## Accomplishments

- Added `check_mixed_fixture_handles_go_and_ts_sources` for the mixed Go/TS fixture, JSON `check`, and DOT import graph output.
- Added `example_go_branch_obligations_reports_expected_diagnostic` using the checked-in Go example source and `.polint.toml`.
- Added `example_ts_design_tokens_reports_raw_colors` using the checked-in TSX example source and `.polint.toml`.
- Located existing property tests for span byte ranges, diagnostic sorting determinism, discovery include/exclude behavior, and cache key participation.

## Verification Commands

```bash
cargo test -p polint-cli --test cli check_mixed_fixture_handles_go_and_ts_sources
cargo test -p polint-cli --test cli example_go_branch_obligations_reports_expected_diagnostic
cargo test -p polint-cli --test cli example_ts_design_tokens_reports_raw_colors
cargo test -p polint-cli --test cli example_
rg -n 'proptest!|span_from_byte_range_is_monotonic|sort_diagnostics_is_input_order_independent|discovery_include_exclude_decision_is_stable|cache_key_for_file_path_participates' crates/polint-core/src/lib.rs crates/polint-diagnostics/src/lib.rs crates/polint-fs/src/lib.rs crates/polint-cache/src/lib.rs
```

## Task Commits

1. **Tasks 1-3: Mixed fixture, configured examples, and test traceability** - `5d9d2b4` (test)

**Plan metadata:** this summary commit

## Files Created/Modified

- `crates/polint-cli/tests/cli.rs` - Added three phase 10 CLI integration tests.

## Decisions Made

The new smoke tests use the checked-in example source and config files directly. That lets the tests catch drift if the example directories move away from runnable v1 behavior.

## Deviations from Plan

None - plan executed as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for `10-04`: final release verification can rely on the added CLI smoke tests plus existing unit, integration, snapshot, and property-test coverage.

---
*Phase: 10-docs-examples-and-release-hardening*
*Completed: 2026-05-01*

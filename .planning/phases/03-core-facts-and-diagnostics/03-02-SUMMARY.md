---
phase: 03-core-facts-and-diagnostics
plan: "02"
subsystem: diagnostics
tags: [rust, polint-diagnostics, diagnostics, snapshots, proptest, serde-json]

requires:
  - phase: 03-core-facts-and-diagnostics
    provides: Plan 03-01 hardened core facts, spans, rule runner determinism, and diagnostic dedupe integration
provides:
  - Full diagnostic contract builders for labels, evidence, suggestions, fixes, help, and fingerprints
  - Stable default diagnostic fingerprints derived from rule, file, full range, and message
  - Deterministic diagnostic sort/dedupe invariant coverage
  - Human and JSON diagnostic rendering snapshots with JSON parseability verification
affects: [phase-04-go-adapter, phase-05-ts-adapter, phase-06-sdk-rules, phase-08-ci-output]

tech-stack:
  added: [insta workspace dev-dependency for polint-diagnostics, proptest workspace dev-dependency for polint-diagnostics]
  patterns:
    - Fluent diagnostic builders preserve public struct fields and existing constructors
    - Human diagnostics render the full contract while JSON remains serde_json-backed
    - Property tests compare sorted diagnostic keys across different input orders

key-files:
  created:
    - .planning/phases/03-core-facts-and-diagnostics/03-02-SUMMARY.md
  modified:
    - Cargo.lock
    - crates/polint-diagnostics/Cargo.toml
    - crates/polint-diagnostics/src/lib.rs

key-decisions:
  - "Kept diagnostic identity limited to rule ID, file path, full range, and message so severity and explanatory fields do not affect dedupe identity."
  - "Left JSON rendering on serde_json::to_string_pretty and proved parseability in the snapshot test."
  - "Kept SARIF behavior out of scope except for preserving compilation, consistent with Phase 8 ownership."

patterns-established:
  - "Diagnostic snapshots use inline insta snapshots in src/lib.rs for focused crate-local coverage."
  - "Contract fixture diagnostics use explicit fingerprints in render snapshots to keep renderer snapshots focused on formatting."

requirements-completed: [DIAG-01, TEST-01, TEST-03, TEST-04]

duration: 5min
completed: 2026-04-28
---

# Phase 03 Plan 02: Diagnostics Contract and Rendering Summary

**Diagnostics now carry the full Phase 3 contract, use stable full-range fingerprints, and have deterministic unit, property, and inline snapshot coverage.**

## Performance

- **Duration:** 5 min
- **Started:** 2026-04-28T11:39:39Z
- **Completed:** 2026-04-28T11:44:10Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Added `with_label`, `with_suggestion`, and `with_fix` fluent helpers while preserving `Diagnostic::error`, `Diagnostic::warning`, `Diagnostic::info`, `with_help`, `with_evidence`, and `with_fingerprint`.
- Changed default fingerprints to include rule ID, file path, start line/column, end line/column, and message.
- Added invariant tests for full diagnostic fields, fingerprint identity inputs, deterministic dedupe, and input-order-independent sorting.
- Expanded human rendering to include labels, evidence, suggestions, fixes, help text, and stable fingerprints.
- Added inline `insta` snapshots for human and JSON rendering, with JSON parsed through `serde_json::Value` before snapshotting.

## Task Commits

1. **Task 1 RED:** `5139d56` test(03-02): add failing diagnostic invariant tests
2. **Task 1 GREEN:** `90baf3f` feat(03-02): harden diagnostic identity contract
3. **Task 2 RED:** `6665adc` test(03-02): add failing diagnostic render snapshots
4. **Task 2 GREEN:** `51d8366` feat(03-02): render full human diagnostic contract

## Files Created/Modified

- `Cargo.lock` - Locked `insta` and its transitive dependencies for diagnostics snapshot tests.
- `crates/polint-diagnostics/Cargo.toml` - Added workspace `insta` and `proptest` as dev-dependencies.
- `crates/polint-diagnostics/src/lib.rs` - Added builders, full-range fingerprinting, human contract rendering, unit tests, property tests, and inline snapshots.
- `.planning/phases/03-core-facts-and-diagnostics/03-02-SUMMARY.md` - Execution record for this plan.

## Verification

- `cargo test -p polint-diagnostics --lib diagnostic_builders` - passed
- `cargo test -p polint-diagnostics --lib fingerprint_includes` - passed
- `cargo test -p polint-diagnostics --lib sort_diagnostics` - passed
- `cargo test -p polint-diagnostics --lib dedupe_diagnostics` - passed
- `cargo test -p polint-diagnostics --lib render_human_snapshot_includes_contract_fields` - passed
- `cargo test -p polint-diagnostics --lib render_json_snapshot_is_stable` - passed
- `cargo test -p polint-diagnostics --lib render_empty_human_output_is_stable` - passed
- `cargo fmt -- --check` - passed
- `cargo test -p polint-diagnostics --lib` - passed, 8 tests
- `cargo clippy -p polint-diagnostics --all-targets -- -D warnings` - passed

## Decisions Made

- Kept diagnostic sort order unchanged except for the now more precise fingerprint values.
- Did not include severity, labels, evidence, suggestions, fixes, help text, timestamps, or collection order in default fingerprints.
- Preserved SARIF-like rendering without adding snapshots, because final SARIF/CI hardening remains Phase 8.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- `cargo fmt -- --check` found formatting drift after the Task 1 GREEN edit. Ran `cargo fmt` before committing the implementation.
- The failed Task 2 RED snapshot generated an untracked `.pending-snap` file; removed that generated test artifact before committing the inline snapshot tests.

## Known Stubs

None - stub scan returned no matches in files modified by this plan.

## Auth Gates

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Go, TypeScript, SDK, and rule phases can now rely on additive diagnostic builders, stable full-range diagnostic identity, deterministic sort/dedupe behavior, and human/JSON rendering coverage. Phase 8 remains responsible for production SARIF-like CI output hardening.

---
*Phase: 03-core-facts-and-diagnostics*
*Completed: 2026-04-28*

## Self-Check: PASSED

- Summary file exists at `.planning/phases/03-core-facts-and-diagnostics/03-02-SUMMARY.md`.
- Verified task commits exist: `5139d56`, `90baf3f`, `6665adc`, `51d8366`.
- Stub scan of files modified by this plan returned no matches.

---
phase: 36-p0-type-value-place-alias-substrate
plan: 07
subsystem: analysis
tags: [rust, validation, debug, eval, no-leak, closeout]
requires:
  - phase: 36-p0-type-value-place-alias-substrate
    provides: Private type/value/place/alias substrate
provides:
  - Deterministic validation diagnostics for malformed type/value/alias facts
  - Debug snapshot coverage for provenance and alias statuses
  - Native Go and TS/JS eval fixtures for type/value/alias coverage
  - Public no-leak proof for private Phase 36 internals
affects: [phase-36, phase-37, validation, eval, public-api]
tech-stack:
  added: []
  patterns: [internal validation diagnostics, deterministic debug counts, public no-leak assertions]
key-files:
  created:
    - tests/eval-fixtures/type-value-alias/go-core/expected.polint-eval.toml
    - tests/eval-fixtures/type-value-alias/go-core/repo/.polint.toml
    - tests/eval-fixtures/type-value-alias/go-core/repo/main.go
    - tests/eval-fixtures/type-value-alias/ts-js-core/expected.polint-eval.toml
    - tests/eval-fixtures/type-value-alias/ts-js-core/repo/.polint.toml
    - tests/eval-fixtures/type-value-alias/ts-js-core/repo/src/app.ts
  modified:
    - crates/polint/src/analysis_kernel/validation.rs
    - crates/polint/src/analysis/types/debug.rs
    - crates/polint/src/core/mod.rs
    - crates/polint/tests/cli.rs
requirements-completed: [SAE-PREC-01]
duration: 19 min
completed: 2026-05-24
---

# Phase 36 Plan 07: Validation, Debug, Eval Fixtures, Public No-Leak Proof, And Roadmap Closeout Summary

**Phase 36 is closed with validation, deterministic debug/eval coverage, full regression, and no public API leak for the private type/value/place/alias substrate.**

## Performance

- **Duration:** 19 min
- **Started:** 2026-05-24T14:20:39Z
- **Completed:** 2026-05-24T14:39:27Z
- **Tasks:** 3
- **Files modified:** 11

## Accomplishments

- Added type/value/alias validation to the kernel metadata validator for duplicate stable keys, dangling place/access-path references, malformed access-path depth, points-to budget mismatches, and overconfident alias answers.
- Extended type/value/alias debug counts with native/extension provenance and added deterministic alias-status coverage for all alias outcomes.
- Added Go and TS/JS native eval fixtures under `tests/eval-fixtures/type-value-alias/` so the fixture suite covers Phase 36 native categories alongside extension precision.
- Added a public no-leak CLI test proving normal JSON output, CLI help, SDK/runner/crate-root surfaces, README, API visibility docs, and facts docs do not expose private Phase 36 internals.
- Kept the private alias metadata precision within the provider's setup-aware ceiling and cleaned stale dead-code expectations that clippy surfaced.

## Task Commits

1. **Task 1: Add validation diagnostics for Phase 36 facts** - `f28b864` (feat)
2. **Task 2: Add debug snapshots and eval observation** - `f28b864` (feat), `a77357c` (test)
3. **Task 3: Add public no-leak proof and close SAE-PREC-01** - `a77357c` (test)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/validation.rs` - Phase 36 fact validation and deterministic regression test.
- `crates/polint/src/analysis/types/debug.rs` - Provenance debug counts and alias status coverage.
- `crates/polint/src/core/mod.rs` - Precision ceiling alignment and dead-code expectation cleanup.
- `crates/polint/tests/cli.rs` - Public no-leak proof for type/value/alias internals.
- `tests/eval-fixtures/type-value-alias/go-core/` - Go native fixture.
- `tests/eval-fixtures/type-value-alias/ts-js-core/` - TS/JS native fixture.

## Decisions Made

- Base-only access paths remain valid; validation checks depth/projection consistency instead of rejecting empty projections.
- `ExactLocal` alias answers stay at `SetupAware` fact metadata precision because the provider's current public precision ceiling is setup-aware.
- Phase 36 internals remain private implementation details until later phases promote typed SDK views intentionally.

## Deviations from Plan

- The plan references `cargo test -p polint --test eval_fixtures --locked -- type_value_alias`, but this repository has no `eval_fixtures` test target. I used the internal lib fixture tests and full `cargo test -p polint --locked` regression instead.
- Eval observation support for extension precision was completed in 36-06; this plan added the remaining native Go and TS/JS fixture coverage.

## Issues Encountered

- Full regression initially caught two real closeout issues: exact alias metadata exceeded the provider precision ceiling, and base-only access paths were incorrectly treated as malformed. Both were fixed before final regression.

## Verification

- `cargo fmt --all --check` - passed.
- `cargo test -p polint --lib analysis::types::debug --locked` - passed.
- `cargo test -p polint --lib type_value_alias_validation --locked` - passed.
- `cargo test -p polint --test cli --locked -- type_value_alias_public_no_leak` - passed.
- `cargo test -p polint --lib eval_native_fixture_suite_covers_required_categories --locked` - passed.
- `cargo test -p polint --lib analysis_kernel::debug::tests::metadata_debug_json_contains_files_imports_symbols_and_references --locked` - passed.
- `cargo clippy -p polint -- -D warnings` - passed.
- `cargo test -p polint --locked` - passed.

## User Setup Required

None.

## Next Phase Readiness

Ready for Phase 37: refined call graph providers can consume the private Phase 36 type/value/place/alias substrate.

---
*Phase: 36-p0-type-value-place-alias-substrate*
*Completed: 2026-05-24*

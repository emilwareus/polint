---
phase: 36-p0-type-value-place-alias-substrate
plan: 06
subsystem: analysis
tags: [rust, extensions, type-value-alias, cache, quarantine, eval]
requires:
  - phase: 36-p0-type-value-place-alias-substrate
    provides: Native type/value/access-path/points-to/alias output
  - phase: 34
    provides: Validated extension fact sink
provides:
  - Phase 36 extension fact family validation and precision ceilings
  - Additive extension merge into private type/value/points-to/alias output
  - Extension-influenced cache/quarantine and native fixture proof
affects: [phase-36, phase-37, extension-sinks, cache, eval]
tech-stack:
  added: []
  patterns: [typed extension payload labels, additive merge, extension-influenced provider ordering]
key-files:
  created:
    - crates/polint/src/analysis/types/validate.rs
    - tests/eval-fixtures/type-value-alias/extension-precision/expected.polint-eval.toml
  modified:
    - crates/polint/src/analysis/extensions/sinks.rs
    - crates/polint/src/analysis/extensions/validate.rs
    - crates/polint/src/analysis/types/provider.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/provider.rs
    - crates/polint/src/analysis_kernel/incremental/quarantine.rs
requirements-completed: []
duration: 24 min
completed: 2026-05-24
---

# Phase 36 Plan 06: Extension Type/Value/Alias Facts, Merge Rules, And Quarantine Summary

**Validated extension facts can now add bounded type/value/alias precision to the private Phase 36 provider output.**

## Performance

- **Duration:** 24 min
- **Started:** 2026-05-24T13:57:36Z
- **Completed:** 2026-05-24T14:20:02Z
- **Tasks:** 3
- **Files modified:** 19

## Accomplishments

- Added typed extension sink families for Phase 36 facts and rejected malformed payloads before merge.
- Added additive merge logic for extension-provided type, value, allocation, access-path, points-to constraint, and alias-answer rows.
- Moved `polint.extensions` before `polint.type_value_alias` and made extension output an explicit type/value/alias input.
- Added extension cache digest coverage, type/value/alias quarantine coverage, and a real native fixture proving an accepted extension alias answer becomes provider output.

## Task Commits

1. **Task 1: Extend typed extension sink vocabulary for Phase 36 facts** - `fbced0f` (feat)
2. **Task 2: Merge validated extension facts additively** - `6ef2049` (feat)
3. **Task 3: Wire extension digests/quarantine/eval proof** - `db5543c` (test)

## Files Created/Modified

- `crates/polint/src/analysis/types/validate.rs` - Extension merge conversion and additive conflict handling.
- `crates/polint/src/analysis/extensions/sinks.rs` - Phase 36 fact family labels and payload shape checks.
- `crates/polint/src/analysis/extensions/validate.rs` - Malformed payload and precision ceiling rejection.
- `crates/polint/src/analysis/types/provider.rs` - Extension merge and extension output digest input wiring.
- `crates/polint/src/analysis_kernel/mod.rs` / `provider.rs` - Provider order and manifest dependency updates.
- `tests/eval-fixtures/type-value-alias/extension-precision/` - Real extension precision fixture.

## Decisions Made

- Extension rows are additive: native facts are not deleted, and stable-key conflicts are skipped/rejected rather than overwritten.
- Unvalidated extension `Exact` claims for Phase 36 facts are rejected by a precision ceiling; accepted extension facts map to conservative internal precision.
- Extension facts are merged inside the private provider output, not exposed as a public rule-author API.

## Deviations from Plan

- The documented `cargo test -p polint --test eval_fixtures` target does not exist in this repository. I used the internal lib fixture tests instead.

## Issues Encountered

- The new fixture initially used a hyphenated extension directory with an underscored handshake ID; the extension host correctly rejected the identity mismatch. The directory now matches the extension ID.

## Verification

- `cargo fmt --all --check` - passed.
- `cargo test -p polint --lib analysis::extensions --locked` - passed.
- `cargo test -p polint --lib analysis::types::validate --locked` - passed.
- `cargo test -p polint --lib analysis::types::cache_key --locked` - passed.
- `cargo test -p polint --lib analysis_kernel::incremental::quarantine --locked` - passed.
- `cargo test -p polint --lib eval_type_value_alias_extension_precision_fixture_passes --locked` - passed.
- `cargo test -p polint --lib eval_native_fixture_suite_covers_required_categories --locked` - passed.
- `cargo check -p polint --locked` - passed.

## User Setup Required

None.

## Next Phase Readiness

Ready for `36-07`: public/internal API boundaries and documentation can be tightened around the completed private Phase 36 fact substrate.

---
*Phase: 36-p0-type-value-place-alias-substrate*
*Completed: 2026-05-24*

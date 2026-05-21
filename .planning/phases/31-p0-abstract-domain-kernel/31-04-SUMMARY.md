---
phase: 31-p0-abstract-domain-kernel
plan: 04
subsystem: analysis
tags: [rust, abstract-interpretation, validation, debug-json, eval-fixtures]

requires:
  - phase: 31-p0-abstract-domain-kernel
    provides: Private domain facts, store, provider, and cache identity from Plan 31-03.
provides:
  - Abstract-domain row validation for references, metadata, status, precision, and reasons.
  - Crate-private debug JSON rows for abstract-domain observations, events, counts, and indexes.
  - Provider-order eval fixture proof for polint.abstract_domains after calls and before metrics.
affects: [phase-31, phase-32, phase-33, analysis-kernel, eval-fixtures]

tech-stack:
  added: []
  patterns:
    - Crate-private fact-family validator invoked from metadata validation after calls.
    - Test-facing debug/eval proof using stable keys, relative paths, labels, counts, and compact payload fragments.

key-files:
  created:
    - crates/polint/src/analysis/domains/validate.rs
  modified:
    - crates/polint/src/analysis/domains/mod.rs
    - crates/polint/src/analysis/domains/store.rs
    - crates/polint/src/analysis_kernel/validation.rs
    - crates/polint/src/analysis_kernel/debug.rs
    - crates/polint/src/core/mod.rs
    - crates/polint/src/eval/observed.rs
    - crates/polint/src/eval/fixtures.rs
    - tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml

key-decisions:
  - "Keep abstract-domain validation, debug JSON, and provider-order proof crate-private/test-facing with no SDK, runner, CLI, README, or docs/facts promotion."
  - "Represent domain bottom/no-info rows as explicit unknown top reasons before validation so malformed unknown rows fail closed."
  - "Record compact eval provider-output schema evidence for polint.abstract_domains without exposing a public provider surface."

patterns-established:
  - "validate_abstract_domains checks duplicate stable keys, required metadata, references, status/reason consistency, and provider precision ceilings."
  - "metadata_debug_json_for_test includes abstract_domains observations/events/counts/index_counts using stable keys and relative paths only."

requirements-completed: [SAE-INT-01]

duration: 14 min
completed: 2026-05-21
---

# Phase 31 Plan 04: Domain Validation Debug And Provider Order Proof Summary

**Fail-closed abstract-domain validation with safe debug snapshots and eval proof of provider order**

## Performance

- **Duration:** 14 min
- **Started:** 2026-05-21T11:44:20Z
- **Completed:** 2026-05-21T11:57:50Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments

- Added `validate_abstract_domains` and wired it into metadata validation after calls.
- Added test-facing `"abstract_domains"` debug JSON with observations, events, counts, and index counts.
- Updated eval observation and the provider-order fixture so `polint.abstract_domains` is proven after `polint.calls` and before `polint.metrics`.
- Verified exact-local payload precision maps to metadata no stronger than setup-aware, while exact metadata from the provider is rejected.

## Task Commits

Each task was committed atomically:

1. **Task 1: Validate and debug domain rows** - `48b92a4` (test), `99d44b4` (feat)
2. **Task 2: Prove abstract-domain provider order** - `0f79098` (test), `ec98254` (feat)

## Files Created/Modified

- `crates/polint/src/analysis/domains/validate.rs` - Abstract-domain row validator for references, metadata, status/reason consistency, duplicate stable keys, call-operation evidence, and precision ceilings.
- `crates/polint/src/analysis/domains/mod.rs` - Registers the private validation module.
- `crates/polint/src/analysis/domains/store.rs` - Emits bottom/no-info domain values as explicit unknown top reasons.
- `crates/polint/src/analysis_kernel/validation.rs` - Invokes domain validation after calls and adds focused tests.
- `crates/polint/src/analysis_kernel/debug.rs` - Adds abstract-domain debug rows, counts, and index-count evidence.
- `crates/polint/src/core/mod.rs` - Removes stale dead-code expectations after debug paths consumed domain event/store accessors.
- `crates/polint/src/eval/observed.rs` - Adds compact abstract-domain provider-output schema/status/count invariants and numeric provider-order test sorting.
- `crates/polint/src/eval/fixtures.rs` - Updates provider-order manifest assertions.
- `tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml` - Records abstract domains between calls and metrics plus schema evidence.

## Decisions Made

- Domain validation/debug/eval proof remains internal only; no SDK, runner, CLI, README, or docs/facts surface was added.
- Unknown/top/setup/unsupported/budget rows must carry a `TopReason`; present rows carry labels or digest fragments.
- Provider-order eval now records the abstract-domain provider schema as compact invariant evidence rather than exposing full provider metadata.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Normalized bottom domain rows to explicit unknown reasons**
- **Found during:** Task 1 (Validate and debug domain rows)
- **Issue:** Existing bottom/no-info domain values were serialized as `Unknown` label rows, which would violate the new validation requirement that unknown/top rows carry a `TopReason`.
- **Fix:** Mapped bottom values to `TopReason::UnknownValue` during domain output normalization.
- **Files modified:** `crates/polint/src/analysis/domains/store.rs`
- **Verification:** `cargo test -p polint --lib analysis_kernel::validation::abstract_domains --locked`
- **Committed in:** `99d44b4`

**2. [Rule 1 - Bug] Fixed provider-order test comparison for index 10**
- **Found during:** Task 2 (Prove abstract-domain provider order)
- **Issue:** Adding `provider_order.10` exposed lexicographic sorting in the observed provider-order test, causing index 10 to compare before index 2.
- **Fix:** Sorted provider-order assertions by parsed numeric suffix before comparing.
- **Files modified:** `crates/polint/src/eval/observed.rs`
- **Verification:** `cargo test -p polint --lib provider_order --locked`
- **Committed in:** `ec98254`

---

**Total deviations:** 2 auto-fixed (1 missing critical, 1 bug)
**Impact on plan:** Both fixes were required by the plan's validation/provider-order correctness goals and stayed inside private/test-facing internals.

## Issues Encountered

The plan command `cargo test -p polint --lib eval::fixtures::provider_order --locked` matched zero tests. I also ran `cargo test -p polint --lib eval_native_fixture_runner_provider_order --locked`, which exercised the concrete provider-order fixture and passed.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --lib analysis_kernel::validation::abstract_domains --locked` - passed
- `cargo test -p polint --lib analysis_kernel::debug::abstract_domains_debug_json --locked` - passed
- `cargo test -p polint --lib provider_order --locked` - passed
- `cargo test -p polint --lib eval::fixtures::provider_order --locked` - passed with 0 matched tests
- `cargo test -p polint --lib eval_native_fixture_runner_provider_order --locked` - passed
- `cargo test -p polint --lib analysis_kernel::validation --locked` - passed
- `cargo fmt --all -- --check` - passed

## Known Stubs

None. Stub scan hits were false positives in format strings and existing test fixture snippets using `exclude = []`.

## Threat Flags

None. The new validation, debug, and eval surfaces match the plan's threat register and remain crate-private/test-facing.

## Next Phase Readiness

Ready for Plan 31-05 to complete public-boundary/no-leak proof and final phase verification over the private abstract-domain kernel.

## Self-Check: PASSED

- Created file exists on disk.
- Task commits exist in git history.

---
*Phase: 31-p0-abstract-domain-kernel*
*Completed: 2026-05-21*

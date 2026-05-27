---
phase: 23-input-snapshots-and-cache-key-vocabulary
plan: 05
subsystem: eval-cache-verification
tags: [rust, eval-fixtures, input-snapshot, cache-stats, public-boundary]

# Dependency graph
requires:
  - phase: 23-input-snapshots-and-cache-key-vocabulary
    provides: input snapshot vocabulary, provider output metadata, cache stats, and kernel run reports from 23-01 through 23-04
  - phase: 22-internal-evaluation-harness-mvp
    provides: native eval fixture runner, observed item model, matcher, metrics, and deterministic report hashing
provides:
  - native cache/input-snapshots fixture proving Phase 23 snapshot/key/provider invariants
  - exact first-run current-cache counter expectations for Go and TS/JS syntax providers
  - public check JSON no-leak and determinism proof for input snapshot vocabulary
  - clippy-clean future-facing incremental vocabulary expectations
affects: [phase-23, phase-24, eval-fixtures, incremental-cache, public-api-boundary]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - observed eval invariants derived from crate-private KernelRunReport
    - native fixture expectations for exact current-cache counters
    - public-boundary integration tests combining JSON determinism and source-surface assertions

key-files:
  created:
    - tests/eval-fixtures/cache/input-snapshots/expected.polint-eval.toml
    - tests/eval-fixtures/cache/input-snapshots/repo/.polint.toml
    - tests/eval-fixtures/cache/input-snapshots/repo/goapp/go.mod
    - tests/eval-fixtures/cache/input-snapshots/repo/goapp/go.sum
    - tests/eval-fixtures/cache/input-snapshots/repo/goapp/payment.go
    - tests/eval-fixtures/cache/input-snapshots/repo/web/package.json
    - tests/eval-fixtures/cache/input-snapshots/repo/web/package-lock.json
    - tests/eval-fixtures/cache/input-snapshots/repo/web/tsconfig.json
    - tests/eval-fixtures/cache/input-snapshots/repo/web/src/app.ts
  modified:
    - crates/polint/src/eval/observed.rs
    - crates/polint/src/eval/fixtures.rs
    - crates/polint/tests/cli.rs
    - crates/polint/src/analysis_kernel/incremental/digest.rs
    - crates/polint/src/analysis_kernel/incremental/keys.rs
    - crates/polint/src/analysis_kernel/incremental/stats.rs

key-decisions:
  - "Emit Phase 23 snapshot/key/provider observations only through the internal eval harness, sourced from KernelRunReport."
  - "Keep exact current-cache counter proof in the new input-snapshots fixture while normalizing those rows out of the older cache-current-determinism comparison."
  - "Assert public JSON and SDK/runner/CLI source surfaces do not expose input snapshot, key, provider-output, cache-stat, or run-report vocabulary."

patterns-established:
  - "Observed invariants use stable string names and values so eval fixtures can assert cache identity coverage without exposing raw snapshot JSON."
  - "First-run Go and TS/JS syntax cache behavior is fixture-pinned with exact hit/miss/recompute/write counters."
  - "Future cache vocabulary uses scoped lint expectations with reasons until later phases wire all consumers."

requirements-completed: [SAE-FND-04]

# Metrics
duration: 17m
completed: 2026-05-18
---

# Phase 23 Plan 05: Input Snapshot Fixture and Public Boundary Summary

**Native eval fixture coverage for Phase 23 input snapshots, provider metadata, exact syntax cache counters, and public no-leak behavior.**

## Performance

- **Duration:** 17m
- **Started:** 2026-05-18T06:53:38Z
- **Completed:** 2026-05-18T07:10:50Z
- **Tasks:** 3
- **Files modified:** 16

## Accomplishments

- Added `snapshot_invariants`, `layer_key_invariants`, and `provider_output_invariants` in the eval observer, sourced from `KernelRunReport`.
- Added `cache/input-snapshots` as a native eval fixture with one Go file and one TS file, proving exact first-run cache counters for both syntax providers.
- Added `input_snapshots_stay_internal`, proving repeated public `polint check --format json` output stays byte-identical and free of internal snapshot/key/report vocabulary.
- Kept Phase 23 scoped to instrumentation and fixture proof; no public SDK, runner, CLI, persistent layer cache, dependency index, stale-reuse, or quarantine behavior was added.

## Task Commits

Each task was committed atomically:

1. **Task 1 RED: Observed invariant tests** - `b910c69` (test)
2. **Task 1 GREEN: Eval observed invariant emission** - `505dde2` (feat)
3. **Task 2 RED: Input-snapshots fixture expectation** - `f02349b` (test)
4. **Task 2 GREEN: Input-snapshots native fixture files** - `c8eeb93` (feat)
5. **Task 3 RED: Public no-leak test scaffold** - `10ea2ef` (test)
6. **Task 3 GREEN: Public no-leak assertions** - `c658353` (test)
7. **Verification fix: Incremental vocabulary clippy hygiene** - `e2e88f8` (fix)

## Files Created/Modified

- `crates/polint/src/eval/observed.rs` - Emits snapshot/key/provider/cache counter observed invariants from `KernelRunReport`.
- `crates/polint/src/eval/fixtures.rs` - Registers the input-snapshots fixture test and normalizes cache stat rows for the older cache-current-determinism comparison.
- `crates/polint/tests/cli.rs` - Adds public JSON determinism and no-leak assertions for Phase 23 internal vocabulary.
- `tests/eval-fixtures/cache/input-snapshots/expected.polint-eval.toml` - Expected native eval invariants and exact cache counter values.
- `tests/eval-fixtures/cache/input-snapshots/repo/` - Self-contained mixed Go and TS fixture repo.
- `crates/polint/src/analysis_kernel/incremental/{digest.rs,keys.rs,stats.rs}` - Scoped lint hygiene for future-facing internal vocabulary.

## Decisions Made

- Current-cache counters are internal eval observations, not public report fields.
- `verified_reuse` and `quarantines` remain explicit zero counters for this phase; no successful reuse or quarantine behavior is claimed.
- The older cache-current-determinism fixture compares public-equivalent behavior after removing internal cache-stat observation rows, while `input-snapshots` owns exact counter assertions.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Compatibility] Normalized cache-current-determinism around new counter rows**
- **Found during:** Task 2 (native fixture suite verification)
- **Issue:** Task 1 added internal cache counter observations that legitimately differ across cold, warm, and no-cache runs, causing the older cache-current-determinism fixture to miss its expected invariant.
- **Fix:** Removed `provider_output.polint.*.cache_stats.*` rows from that legacy comparison and recomputed matches/metrics for the normalized comparison.
- **Files modified:** `crates/polint/src/eval/fixtures.rs`
- **Verification:** `cargo test -p polint --lib eval_cache_current_determinism_fixture_passes --locked`; `cargo test -p polint --lib eval_native_fixture_suite_covers_required_categories --locked`
- **Committed in:** `c8eeb93`

**2. [Rule 3 - Blocking] Kept future-facing incremental vocabulary clippy-clean**
- **Found during:** Plan-level `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- **Issue:** Future Phase 23 vocabulary types still intentionally await later consumers, triggering dead-code and constructor-arity lints; one test used owned JSON strings only for ordering comparisons.
- **Fix:** Added scoped `#[expect]` attributes with reasons for intentional future vocabulary and changed the test comparison to inspect serialized digest values directly.
- **Files modified:** `crates/polint/src/analysis_kernel/incremental/digest.rs`, `crates/polint/src/analysis_kernel/incremental/keys.rs`, `crates/polint/src/analysis_kernel/incremental/stats.rs`
- **Verification:** `cargo test -p polint --lib incremental --locked`; `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- **Committed in:** `e2e88f8`

---

**Total deviations:** 2 auto-fixed issues (1 compatibility, 1 blocking verification)
**Impact on plan:** Both fixes preserve Phase 23 scope and public behavior. No persistent layer cache, public API, or unsupported cache semantics were added.

## Issues Encountered

- TDD RED runs failed as intended before implementation for each task.
- `.planning/config.json` was dirty before execution and remained untouched.
- Full verification initially failed on clippy warnings in Phase 23 future vocabulary; the blocking lint hygiene fix resolved it.

## User Setup Required

None - no external service configuration required.

## Known Stubs

None. Stub-pattern scan hits in `crates/polint/tests/cli.rs` are intentional test fixtures for TODO-literal and placeholder-literal rules, not unimplemented behavior.

## Threat Flags

None - new file access is limited to planned internal eval fixtures and test temp repos, and no public endpoint, auth path, schema migration, or public API surface was introduced.

## Verification

- `cargo test -p polint --lib observed_phase23_cache_counter_invariants --locked`
- `cargo test -p polint --lib eval_input_snapshot_fixture_passes --locked`
- `cargo test -p polint --lib eval_native_fixture_suite_covers_required_categories --locked`
- `cargo test -p polint --test cli input_snapshots_stay_internal --locked`
- `cargo test -p polint --test cli kernel_metadata_preserves_public_check_behavior --locked`
- `cargo test -p polint --lib eval --locked`
- `cargo test --workspace --all-features --locked`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo fmt --all -- --check`

## Next Phase Readiness

Phase 24 can consume internal snapshot/provider/cache-stat observations with fixture proof that current syntax cache counters are deterministic. Public `polint check` output remains vocabulary-free.

## Self-Check: PASSED

- Created/modified files exist, including the summary, eval observer/fixture updates, CLI no-leak test, fixture repo files, and incremental lint-hygiene files.
- Commits exist: `b910c69`, `505dde2`, `f02349b`, `c8eeb93`, `10ea2ef`, `c658353`, and `e2e88f8`.
- Final verification passed: targeted task tests, `cargo test -p polint --lib eval --locked`, `cargo test --workspace --all-features --locked`, `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`, and `cargo fmt --all -- --check`.
- `.planning/config.json` remained unstaged and untouched by this plan.

---
*Phase: 23-input-snapshots-and-cache-key-vocabulary*
*Completed: 2026-05-18*

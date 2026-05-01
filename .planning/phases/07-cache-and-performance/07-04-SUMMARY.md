---
phase: 07-cache-and-performance
plan: "04"
subsystem: performance
tags: [rust, cache, profiling, deterministic-output]
requires:
  - phase: 07-cache-and-performance
    provides: 07-01 cache key schema inputs
  - phase: 07-cache-and-performance
    provides: 07-02 cached parser facts
  - phase: 07-cache-and-performance
    provides: 07-03 deterministic Rayon execution
provides:
  - deterministic profile-rules timing rows
  - final cold/warm/no-cache equality proof
  - final cache key path property proof
  - full workspace formatting, clippy, and test verification
affects: [cli, cache, core]
tech-stack:
  added:
    - proptest dev-dependency in polint-cache
  patterns:
    - parseable non-snapshotted timing fields
    - cold/warm/no-cache byte-identical JSON assertions
key-files:
  created:
    - .planning/phases/07-cache-and-performance/07-04-SUMMARY.md
  modified:
    - crates/polint-cli/src/main.rs
    - crates/polint-cli/tests/cli.rs
    - crates/polint-cache/Cargo.toml
    - crates/polint-cache/src/lib.rs
    - Cargo.lock
key-decisions:
  - "Profile timings are user-facing local metadata only; tests parse shape and nonnegative values without asserting benchmark-grade speedups."
  - "The no-cache proof compares exact JSON output after a warm cache exists and asserts the cache file count does not increase."
patterns-established:
  - "Phase performance claims are proved through deterministic output and cache behavior tests, not fixed elapsed-time assertions."
requirements-completed:
  - PERF-01
  - PERF-02
  - PERF-03
  - TEST-01
  - TEST-04
duration: 6 min
completed: 2026-05-01
---

# Phase 7 Plan 04: Profiling and Final Proof Summary

**Closed Phase 7 with deterministic per-rule profiling output and end-to-end cache/performance verification.**

## Performance

- **Duration:** 6 min
- **Started:** 2026-05-01T08:11:24Z
- **Completed:** 2026-05-01T08:17:10Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments

- Hardened `polint profile-rules` so it emits deterministic rows in enabled rule order.
- Reused cached parallel analysis setup before per-rule timing so profiling observes the same analysis inputs as `check`.
- Added integration coverage for `profile-rules` row shape, parseable `elapsed_ms`, diagnostic counts, and `--fail-on warn` exit behavior.
- Added cache key property coverage proving relative path differences affect stable cache IDs.
- Added a cold/warm/no-cache CLI test proving exact JSON equality and no extra cache writes when `--no-cache` is used.
- Ran full Phase 7 verification across formatting, clippy, targeted tests, and the full workspace test suite.

## Task Commits

Each implementation task was committed atomically:

1. **Task 1: Harden profile-rules timing rows** - `0b78fa8` (perf)
2. **Task 2: Add final cache and determinism property coverage** - `44f4431` (test)
3. **Formatting: Format profile rule tests** - `011f722` (style)
4. **Task 3: Run full Phase 7 verification and update evidence** - recorded in this summary

## Evidence By Success Criterion

- **Cache key inputs:** `cargo test -p polint-cache --lib` passed 8 tests, including `cache_key_changes_with_config`, `cache_key_changes_with_rule_hash`, `cache_key_changes_with_schema`, `cache_key_changes_with_relative_path`, and `cache_key_for_file_path_participates_in_stable_id_proptest`.
- **`--no-cache` bypass:** `cargo test -p polint-cli --test cli cache` passed `check_no_cache_does_not_create_cache_directory`, `check_no_cache_bypasses_cache_writes`, and `check_no_cache_bypasses_cache_reads_and_writes`.
- **Rayon safe parallelism:** `cargo test -p polint-core --lib run_rules_parallel_matches_sequential` passed; full workspace tests also passed `go_parallel_analysis_matches_sequential`, `ts_parallel_analysis_matches_sequential`, and `load_analysis_files_parallel_matches_sequential_order`.
- **`profile-rules` timing rows:** `cargo test -p polint-cli --test cli profile_rules_reports_per_rule_timings` passed; the test parses every `elapsed_ms=` token as a nonnegative `f64` and asserts deterministic rule ordering and `diagnostics=` fields.
- **Repeated-run deterministic output:** `cargo test -p polint-cli --test cli cache` passed `check_cached_output_is_deterministic_across_repeated_runs`, `check_parallel_cached_output_is_deterministic_across_repeated_runs`, and the cold/warm/no-cache byte-identical JSON check.

## Verification Commands

- `cargo fmt -- --check` - passed after rustfmt formatting was applied and committed.
- `cargo test -p polint-cli --test cli profile_rules_reports_per_rule_timings` - passed.
- `cargo test -p polint-cli --test cli cache` - passed 7 tests.
- `cargo test -p polint-cache --lib` - passed 8 tests.
- `cargo test -p polint-core --lib run_rules_parallel_matches_sequential` - passed.
- `cargo clippy --workspace --all-targets -- -D warnings` - passed.
- `cargo test --workspace` - passed.

## Files Created/Modified

- `crates/polint-cli/src/main.rs` - `profile-rules` now reports deterministic per-rule timing rows after shared analysis/cache setup.
- `crates/polint-cli/tests/cli.rs` - Profile timing, fail-on, cache metadata, no-cache, and deterministic repeated-run integration coverage.
- `crates/polint-cache/src/lib.rs` - Final cache key property coverage.
- `crates/polint-cache/Cargo.toml` and `Cargo.lock` - `proptest` dev-dependency edge for cache invariants.

## Decisions Made

- Kept timing output honest by avoiding exact-duration assertions and fixed-speedup claims.
- Verified cache correctness through output equality and cache file counts instead of internal cache-hit counters.
- Kept per-rule timing sequential after shared analysis so rows remain meaningful and deterministic.

## Deviations from Plan

- `cargo fmt -- --check` found rustfmt wrapping changes in profile-rule tests. Fixed with `cargo fmt` and committed as `011f722`.

## Issues Encountered

None beyond formatting.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 7 is ready for phase-level review, verification, security closeout, and roadmap completion.

---
*Phase: 07-cache-and-performance*
*Completed: 2026-05-01*

---
phase: 23-input-snapshots-and-cache-key-vocabulary
plan: 03
subsystem: cache-instrumentation
tags: [rust, cache, incremental, go, typescript]

# Dependency graph
requires:
  - phase: 23-input-snapshots-and-cache-key-vocabulary
    provides: crate-private CacheStats vocabulary and existing cache key bridge
provides:
  - typed current-cache read/write status helpers
  - Go syntax provider cache-stat wrapper
  - TS/JS syntax provider cache-stat wrapper
  - deterministic unit coverage for disabled, hit, miss, invalid, recompute, and write outcomes
affects: [phase-23, phase-24, current-cache, provider-output-metadata]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - crate-private status-returning wrappers around compatibility cache APIs
    - provider analysis result wrappers that preserve diagnostic-only callers
    - per-file cache event aggregation into CacheStats counters

key-files:
  created: []
  modified:
    - crates/polint/src/cache/mod.rs
    - crates/polint/src/go/adapter.rs
    - crates/polint/src/go/mod.rs
    - crates/polint/src/go/tests.rs
    - crates/polint/src/ts/adapter.rs
    - crates/polint/src/ts/mod.rs
    - crates/polint/src/ts/tests.rs

key-decisions:
  - "Keep cache stats crate-private and route existing adapter entrypoints through diagnostic-only compatibility wrappers."
  - "Treat decoded cache payloads with a schema mismatch as current-cache misses/recomputes instead of successful hits."
  - "Leave verified_reuse and quarantines at zero; this plan records current cache behavior only."

patterns-established:
  - "Cache::read_json_or_miss and Cache::write_json delegate to status-aware helpers while preserving existing return contracts."
  - "ProviderAnalysisResult carries diagnostics plus CacheStats for internal consumers without changing public or bench-facing wrappers."
  - "FileCacheEvent records read/write outcomes per file before deterministic aggregation."

requirements-completed: [SAE-FND-04]

# Metrics
duration: 9m
completed: 2026-05-18
---

# Phase 23 Plan 03: Cache Status and Provider Stats Summary

**Current Go and TS/JS file-fact cache access now reports deterministic internal CacheStats without changing existing cache reuse behavior.**

## Performance

- **Duration:** 9m
- **Started:** 2026-05-18T06:27:25Z
- **Completed:** 2026-05-18T06:35:56Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments

- Added `CacheReadStatus`, `CacheReadOutcome<T>`, and `CacheWriteStatus` for current JSON cache reads and writes.
- Preserved compatibility APIs by routing `read_json_or_miss` and `write_json` through the new status-aware helpers.
- Added crate-private Go and TS/JS `analyze_with_plan_options_and_cache_stats` wrappers returning diagnostics plus `CacheStats`.
- Counted hits, misses, disabled bypasses, invalid evictions, recomputes, and successful writes while leaving verified reuse and quarantine semantics unimplemented.
- Added focused tests for cache statuses and provider stats while preserving existing diagnostic-only adapter wrappers.

## Task Commits

Each task was committed atomically:

1. **Task 1 RED: Cache status tests** - `18e766a` (test)
2. **Task 1 GREEN: Cache status helpers** - `24ae316` (feat)
3. **Task 2 RED: Provider cache stats tests** - `d964e19` (test)
4. **Task 2 GREEN: Provider cache stats wrappers** - `3b0038f` (feat)

## Files Created/Modified

- `crates/polint/src/cache/mod.rs` - Adds status-aware cache read/write helpers and focused tests.
- `crates/polint/src/go/adapter.rs` - Adds Go provider stats wrapper and cache event aggregation.
- `crates/polint/src/go/mod.rs` - Re-exports the crate-private Go stats wrapper.
- `crates/polint/src/go/tests.rs` - Covers Go miss/write/hit stats and disabled-cache recomputes.
- `crates/polint/src/ts/adapter.rs` - Adds TS/JS provider stats wrapper and cache event aggregation.
- `crates/polint/src/ts/mod.rs` - Re-exports the crate-private TS/JS stats wrapper.
- `crates/polint/src/ts/tests.rs` - Covers TS/JS miss/write/hit stats.

## Decisions Made

- Existing `analyze_with_plan_options` functions continue returning only `Vec<Diagnostic>` and now discard stats from the new internal wrapper.
- Current-cache stats treat valid JSON with an unexpected schema as a miss/recompute path, matching the existing behavior that does not reuse that payload.
- No persistent layer cache, dependency index, stale reuse verification, or quarantine behavior was introduced.

## Deviations from Plan

None - plan scope was executed as written. Warning hygiene stayed limited to preserving compatibility APIs and future-facing crate-private re-exports without changing behavior.

## Issues Encountered

- TDD RED runs failed as expected because the new helper types and provider wrappers did not exist yet.
- The integration-test build initially warned on future-facing re-exports and compatibility helpers after adapters moved to the status-aware APIs. Scoped lint allowances were added only for those compatibility surfaces.
- `cargo test -p polint --test cli check_cache_writes_fact_metadata --locked` still emits existing Phase 23 dead-code warnings from incremental vocabulary not yet consumed by production paths; the test itself passes.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --lib cache_read_status --locked`
- `cargo test -p polint --lib cache --locked`
- `cargo test -p polint --lib go:: --locked`
- `cargo test -p polint --lib ts:: --locked`
- `cargo test -p polint --test cli check_cache_writes_fact_metadata --locked`
- `cargo fmt --all -- --check`
- Acceptance greps confirmed status helpers, provider wrappers, crate-private re-exports, and no Phase 24 layer-cache terms.

## Known Stubs

None.

## Threat Flags

None - this plan adds internal cache/status instrumentation only and introduces no new endpoint, file-access trust boundary, schema migration, or public API surface.

## Next Phase Readiness

Phase 24 can consume deterministic current-cache counters while implementing persistent layer cache behavior separately. Existing Go/TS cache behavior and diagnostic-only adapter callers remain compatible.

## Self-Check: PASSED

- Created/modified files exist: cache helpers, Go/TS adapter wrappers, Go/TS module re-exports, Go/TS tests, and this summary.
- Commits exist: `18e766a`, `24ae316`, `d964e19`, and `3b0038f`.
- Final verification passed: cache status tests, cache filter, Go filter, TS filter, targeted CLI cache metadata test, and `cargo fmt --all -- --check`.

---
*Phase: 23-input-snapshots-and-cache-key-vocabulary*
*Completed: 2026-05-18*

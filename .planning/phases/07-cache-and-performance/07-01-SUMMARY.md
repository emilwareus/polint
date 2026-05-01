---
phase: 07-cache-and-performance
plan: "01"
subsystem: cache
tags: [rust, cache, cli, deterministic-hash]
requires:
  - phase: 06-sdk-and-example-rules
    provides: SDK examples and built-in rule registry used by rule hashing
provides:
  - cache keys with schema, config, rule, content, and path invalidation inputs
  - disabled cache read/write no-op behavior
  - deterministic CLI config and rule hash helpers
affects: [cache, cli, go-adapter, ts-adapter, performance]
tech-stack:
  added: []
  patterns:
    - source-free cache key construction with explicit schema strings
    - disabled cache writes verified by filesystem absence
key-files:
  created:
    - .planning/phases/07-cache-and-performance/07-01-SUMMARY.md
  modified:
    - crates/polint-cache/Cargo.toml
    - crates/polint-cache/src/lib.rs
    - crates/polint-cli/src/main.rs
    - crates/polint-cli/tests/cli.rs
    - Cargo.lock
key-decisions:
  - "Cache keys include a schema string so Go facts, TS facts, and future payloads cannot collide."
  - "The cache crate remains generic and serde-based; CLI code derives deterministic config/rule hashes separately."
patterns-established:
  - "Cache disabled means no reads, no writes, and no `.polint/cache` directory creation."
  - "Rule hash inputs are assembled from deterministic rule IDs, metadata, and ordered option fields."
requirements-completed:
  - PERF-01
  - TEST-01
  - TEST-04
duration: 10 min
completed: 2026-05-01
---

# Phase 7 Plan 01: Cache Foundation Summary

**Schema-aware cache keys and disabled-cache proof for the Phase 7 parser/fact cache.**

## Performance

- **Duration:** 10 min
- **Started:** 2026-05-01T07:48:00Z
- **Completed:** 2026-05-01T07:58:04Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments

- Added `CacheKey::for_file` with relative path, content hash, config hash, rule hash, version, and schema inputs.
- Added cache helper methods for enabled/root inspection and fallback reads that treat malformed JSON as a miss.
- Added CLI hash plumbing for deterministic config and rule cache inputs.
- Added CLI proof that `polint check --no-cache` does not create `.polint/cache`.

## Task Commits

Each task was committed atomically:

1. **Task 1: Make cache keys prove all invalidation inputs** - `894bf77` (perf)
2. **Task 2: Add safe cache read/write helpers and disabled no-op tests** - `4163318` (test)
3. **Task 3: Add deterministic config and rule hash plumbing in the CLI** - `faa84a9` (perf)

## Files Created/Modified

- `crates/polint-cache/src/lib.rs` - Cache key schema support, disabled-cache helpers, fallback reads, and cache tests.
- `crates/polint-cache/Cargo.toml` - Added `tempfile` dev-dependency for cache filesystem tests.
- `crates/polint-cli/src/main.rs` - Added deterministic config/rule hash helpers and early hash derivation.
- `crates/polint-cli/tests/cli.rs` - Added `check_no_cache_does_not_create_cache_directory`.
- `Cargo.lock` - Reflected the new `polint-cache` dev-dependency edge.

## Decisions Made

- Kept `CacheKey::new` backward-compatible by assigning default schema `analysis-facts-v1`.
- Used relative path plus content hash for per-file identity so identical contents in different files do not collide.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 07-02 can now wire adapters to cache using `CacheKey::for_file`, `read_json_or_miss`, and deterministic CLI config/rule hashes.

---
*Phase: 07-cache-and-performance*
*Completed: 2026-05-01*

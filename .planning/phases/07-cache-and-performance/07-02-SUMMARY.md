---
phase: 07-cache-and-performance
plan: "02"
subsystem: cache
tags: [rust, cache, go, typescript, analysis-db]
requires:
  - phase: 07-cache-and-performance
    provides: 07-01 cache key and disabled-cache primitives
provides:
  - source-free cached fact DTOs in polint-core
  - Go parser/fact cache read/write integration
  - TS/JS parser/fact cache read/write integration
  - CLI tests for cache creation, no-cache bypass, and deterministic repeated output
affects: [go-adapter, ts-adapter, cli, analysis-db]
tech-stack:
  added:
    - polint-cache dependency in polint-go
    - polint-cache dependency in polint-ts
  patterns:
    - adapters restore cached facts through AnalysisDb rather than trusting persisted IDs directly
    - malformed cache entries are treated as misses through read_json_or_miss
key-files:
  created:
    - .planning/phases/07-cache-and-performance/07-02-SUMMARY.md
  modified:
    - crates/polint-core/src/lib.rs
    - crates/polint-go/Cargo.toml
    - crates/polint-go/src/lib.rs
    - crates/polint-ts/Cargo.toml
    - crates/polint-ts/src/lib.rs
    - crates/polint-cli/src/main.rs
    - crates/polint-cli/tests/cli.rs
    - Cargo.lock
key-decisions:
  - "Cached fact DTOs live in polint-core so polint-cache stays generic and avoids a crate cycle."
  - "Adapters cache parser diagnostics and source-free facts only; source text and ASTs are not persisted."
patterns-established:
  - "Adapter cache schemas use explicit strings: go-facts-v1 and ts-facts-v1."
  - "Cache hits restore facts through AnalysisDb::restore_file_facts so current-run IDs remain deterministic."
requirements-completed:
  - PERF-01
  - TEST-01
  - TEST-04
duration: 12 min
completed: 2026-05-01
---

# Phase 7 Plan 02: Cached Parser/Facts Summary

**Source-free Go and TS/JS parser facts cached under `.polint/cache` with deterministic restoration.**

## Performance

- **Duration:** 12 min
- **Started:** 2026-05-01T07:53:00Z
- **Completed:** 2026-05-01T08:04:50Z
- **Tasks:** 3
- **Files modified:** 8

## Accomplishments

- Added `CachedFileAnalysis` and `CachedFileFacts` in `polint-core`.
- Added per-file fact export and restore helpers with function/branch ID remapping.
- Wired Go analysis through `analyze_with_cache` using `go-facts-v1` cache keys.
- Wired TS/JS analysis through `analyze_with_cache` using `ts-facts-v1` cache keys.
- Added CLI tests proving cache writes metadata, `--no-cache` bypasses writes, and repeated cache-enabled JSON output is byte-identical.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add source-free cached fact payloads** - `f311788` (perf)
2. **Task 2: Wire cache through the Go adapter** - `e87b3b8` (perf)
3. **Task 3: Wire cache through the TS/JS adapter** - `01d8af8` (perf)
4. **Formatting: Apply rustfmt to cached analysis changes** - `8f70429` (style)

## Files Created/Modified

- `crates/polint-core/src/lib.rs` - Cached fact DTOs plus export/restore helpers.
- `crates/polint-go/Cargo.toml` - Added `polint-cache` dependency.
- `crates/polint-go/src/lib.rs` - Go cache hit/miss/write path.
- `crates/polint-ts/Cargo.toml` - Added `polint-cache` dependency.
- `crates/polint-ts/src/lib.rs` - TS/JS cache hit/miss/write path.
- `crates/polint-cli/src/main.rs` - CLI now passes cache, config hash, and rule hash into both adapters.
- `crates/polint-cli/tests/cli.rs` - Cache metadata, no-cache bypass, and deterministic repeated-run tests.
- `Cargo.lock` - Reflected new adapter dependency edges.

## Decisions Made

- Kept `polint-cache` generic and serde-only; persisted fact DTOs live in `polint-core`.
- Cache write failures become `internal/cache` warnings rather than aborting source analysis.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- `cargo fmt -- --check` found formatting drift after the implementation. Fixed with rustfmt and committed as `8f70429`.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 07-03 can parallelize file loading and per-file adapter work using the source-free cached fact bundle as the deterministic merge boundary.

---
*Phase: 07-cache-and-performance*
*Completed: 2026-05-01*

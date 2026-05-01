---
phase: 07-cache-and-performance
plan: "03"
subsystem: performance
tags: [rust, rayon, deterministic-output, cache]
requires:
  - phase: 07-cache-and-performance
    provides: 07-02 source-free cached fact bundles
provides:
  - parallel file loading with deterministic AnalysisDb insertion
  - parallel Go parser/fact collection with deterministic merge
  - parallel TS/JS parser/fact collection with deterministic merge
  - CLI repeated-run proof for the default parallel cached check path
affects: [filesystem, go-adapter, ts-adapter, cli, core]
tech-stack:
  added:
    - rayon dependency in polint-fs
    - rayon dependency in polint-go
    - rayon dependency in polint-ts
  patterns:
    - per-file worker analysis merged by sorted FileId
    - worker-local AnalysisDb values share Arc-backed source text
key-files:
  created:
    - .planning/phases/07-cache-and-performance/07-03-SUMMARY.md
  modified:
    - crates/polint-fs/Cargo.toml
    - crates/polint-fs/src/lib.rs
    - crates/polint-go/Cargo.toml
    - crates/polint-go/src/lib.rs
    - crates/polint-ts/Cargo.toml
    - crates/polint-ts/src/lib.rs
    - crates/polint-core/src/lib.rs
    - crates/polint-cli/src/main.rs
    - crates/polint-cli/tests/cli.rs
    - Cargo.lock
key-decisions:
  - "Parser workers do not mutate the shared AnalysisDb; they return cached fact bundles that are restored sequentially."
  - "The CLI's existing `parallel` argument now controls file loading, adapter analysis, and rule execution."
patterns-established:
  - "Rayon work happens per file; observable IDs and diagnostics are produced by deterministic merge/sort boundaries."
  - "Temporary worker databases use `AnalysisDb::add_source_file` to avoid cloning full source strings."
requirements-completed:
  - PERF-02
  - TEST-01
  - TEST-04
duration: 7 min
completed: 2026-05-01
---

# Phase 7 Plan 03: Deterministic Parallel Execution Summary

**Rayon-backed file and adapter analysis with deterministic `AnalysisDb` merge and repeated CLI output proof.**

## Performance

- **Duration:** 7 min
- **Started:** 2026-05-01T08:04:50Z
- **Completed:** 2026-05-01T08:11:24Z
- **Tasks:** 3
- **Files modified:** 10

## Accomplishments

- Parallelized source file reads in `polint-fs` while preserving sorted file IDs.
- Added Rayon-backed per-file Go and TS/JS analysis collection.
- Added `AnalysisDb::add_source_file` so worker databases share `Arc<str>` source text.
- Threaded CLI `parallel` through adapter parsing and existing `run_rules` execution.
- Added repeated-run tests proving the default parallel cached CLI output is byte-identical.

## Task Commits

Each task was committed atomically:

1. **Task 1: Parallelize file reads while preserving file ID order** - `11ebad5` (perf)
2. **Task 2: Collect Go and TS/JS parser outputs in parallel and merge deterministically** - `aa97745` (perf)
3. **Task 3: Thread deterministic parallel analysis through CLI checks** - `6b4964a` (test)
4. **Formatting: Format parallel analysis helper** - `69af5f4` (style)

## Files Created/Modified

- `crates/polint-fs/src/lib.rs` - Parallel file reads and deterministic insertion tests.
- `crates/polint-go/src/lib.rs` - Parallel Go file analysis and sequential-equivalence test.
- `crates/polint-ts/src/lib.rs` - Parallel TS/JS file analysis and sequential-equivalence test.
- `crates/polint-core/src/lib.rs` - `add_source_file` helper for Arc-backed worker databases.
- `crates/polint-cli/src/main.rs` - Adapter analysis now receives the CLI parallel flag.
- `crates/polint-cli/tests/cli.rs` - Parallel cached repeated-run determinism test.
- `crates/*/Cargo.toml` and `Cargo.lock` - Rayon dependency edges for crates that now use it.

## Decisions Made

- Used per-file worker-local databases instead of locks around the shared `AnalysisDb`.
- Kept graph command analysis sequential through its existing `parallel = false` call path.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- `cargo fmt -- --check` found one formatting adjustment in the new `add_file` helper. Fixed with rustfmt and committed as `69af5f4`.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 07-04 can close profiling and final proof with cache, parallelism, and deterministic output already wired.

---
*Phase: 07-cache-and-performance*
*Completed: 2026-05-01*

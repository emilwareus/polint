---
phase: 24-persistent-layer-cache-for-existing-cheap-facts
plan: 02
subsystem: cache
tags: [rust, incremental-cache, syntax-cache, go, typescript, layer-cache]

requires:
  - phase: 24-persistent-layer-cache-for-existing-cheap-facts
    provides: Crate-private LayerCacheStore, LayerCacheManifest, dependency index, and managed layers cache layout from Plan 24-01
  - phase: 23-input-snapshots-and-cache-key-vocabulary
    provides: Digest, LayerKey, CacheStats, and provider output metadata vocabulary
provides:
  - Rule-independent syntax LayerKey construction for Go and TS/JS providers
  - Persistent Go syntax layer manifests and normalized payload reuse
  - Persistent TS/JS syntax layer manifests and normalized payload reuse
  - CLI regression proving unrelated rule edits do not invalidate syntax layers or leak internals
affects: [phase-24, incremental-cache, go-provider, ts-provider, analysis-kernel]

tech-stack:
  added: []
  patterns:
    - Provider-level syntax layer bridge around existing normalized parser facts
    - Manifest-validated cache hits with output digest reuse
    - Public-output compatibility tests for internal cache metadata

key-files:
  created: []
  modified:
    - crates/polint/src/analysis_kernel/incremental/keys.rs
    - crates/polint/src/analysis_kernel/incremental/mod.rs
    - crates/polint/src/analysis_kernel/incremental/stats.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/cache/mod.rs
    - crates/polint/src/go/adapter.rs
    - crates/polint/src/go/tests.rs
    - crates/polint/src/ts/adapter.rs
    - crates/polint/src/ts/tests.rs
    - crates/polint/tests/cli.rs

key-decisions:
  - "Syntax layer identity excludes rule code, rule options, and downstream diagnostic identity; parser reuse is keyed by source, config, lifecycle, provider, schema, toolchain, and parser parameters."
  - "Go and TS/JS syntax layer payloads store normalized facts and parser diagnostics, not raw source bodies or absolute temp roots."
  - "Adapter provider-output metadata reuses the validated layer read output digest on hits and computes the digest only after recompute misses."
  - "Cache hit/miss/reuse counters stay internal; the CLI regression verifies public JSON compatibility through PolintReport parsing and no-leak markers."

patterns-established:
  - "Provider adapters attempt a LayerCacheStore read before parsing, restore cached normalized facts on validated hits, and fail closed to recompute on corrupt or mismatched entries."
  - "Disabled syntax caching records bypasses without creating `.polint/cache/layers` files."
  - "Parallel cache tests use per-process atomic temp-root suffixes so corrupt-cache fixtures cannot interfere with one another."

requirements-completed: [SAE-FND-05]

duration: 20 min
completed: 2026-05-18
---

# Phase 24 Plan 02: Syntax Layer Cache Summary

**Persistent Go and TS/JS syntax layers with rule-independent keys, verified reuse stats, and public-output compatibility coverage**

## Performance

- **Duration:** 20 min
- **Started:** 2026-05-18T10:43:20Z
- **Completed:** 2026-05-18T11:03:29Z
- **Tasks:** 3
- **Files modified:** 10

## Accomplishments

- Added `LayerKey::syntax_layer_key` for Go and TS/JS syntax providers, with canonical source digest sorting and no rule-code or rule-options inputs.
- Wired Go and TS/JS adapters to persist provider-level syntax layer manifests and normalized fact payloads through `LayerCacheStore`.
- Recorded deterministic `misses`, `hits`, `recomputes`, `writes`, `bypasses_disabled`, `invalid_evicted_reads`, and `verified_reuse` counters for syntax layer cache paths.
- Added tests for cold/warm reuse, corrupt-cache fail-closed recompute, disabled bypasses, no raw source/temp-path payload leaks, and unrelated rule-edit stability.

## Task Commits

Each TDD task was committed atomically:

1. **Task 1: Add syntax layer keys that separate parser inputs from rule inputs**
   - `c54007e` test: add failing syntax layer key tests
   - `e5271da` feat: add syntax layer key constructor
2. **Task 2: Persist Go and TS/JS syntax layer manifests around existing fact payloads**
   - `f9d5359` test: add failing syntax layer cache tests
   - `07e2d32` feat: persist Go and TS syntax layers
   - `7388a1c` fix: isolate syntax cache test roots
3. **Task 3: Prove unrelated rule edits do not invalidate syntax layers**
   - `46c8eaf` test: prove syntax cache ignores rule edits

## Files Created/Modified

- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Syntax layer key helper and tests for rule independence, parser-input invalidation, and source digest sorting.
- `crates/polint/src/analysis_kernel/incremental/mod.rs` - Crate-private re-export of `LayerKind` for provider integration.
- `crates/polint/src/analysis_kernel/incremental/stats.rs` - Production-use `record_verified_reuse` method without stale dead-code expectation.
- `crates/polint/src/analysis_kernel/mod.rs` - Provider output metadata now carries adapter-provided layer hit digests when available.
- `crates/polint/src/cache/mod.rs` - Cache exposes enabled state and layer cache directory to crate-private provider adapters.
- `crates/polint/src/go/adapter.rs` - Go syntax layer read/write bridge, payload serialization, hit restore, corrupt recompute, and disabled bypass handling.
- `crates/polint/src/go/tests.rs` - Go syntax layer cold/warm, corrupt, disabled, payload privacy, and parallel-safe temp-root tests.
- `crates/polint/src/ts/adapter.rs` - TS/JS syntax layer read/write bridge, payload serialization, hit restore, corrupt recompute, and disabled bypass handling.
- `crates/polint/src/ts/tests.rs` - TS/JS syntax layer cold/warm, corrupt, disabled, payload privacy, and parallel-safe temp-root tests.
- `crates/polint/tests/cli.rs` - External public-SDK rule fixture proving unrelated rule edits leave syntax manifests stable and public JSON clean.

## Decisions Made

- Syntax cache identity intentionally does not include rule hash, rule code digest, or rule options; those inputs belong to downstream rule/query/diagnostic cache identity.
- Syntax payloads use the existing normalized fact structs and parser diagnostics as the first persistent layer payload rather than inventing a second fact representation.
- Public CLI output remains unchanged; tests inspect internal cache files for manifest stability rather than adding a user-facing cache-stat field.
- Corrupt or mismatched syntax layer entries count as invalid reads and recompute normally instead of surfacing cache corruption to users.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Removed stale verified-reuse dead-code expectation**
- **Found during:** Task 3 (Prove unrelated rule edits do not invalidate syntax layers)
- **Issue:** `CacheStats::record_verified_reuse` became production adapter behavior, so the previous non-test `expect(dead_code)` lint expectation was unfulfilled when compiling CLI integration tests.
- **Fix:** Removed the stale expectation and kept the method crate-private.
- **Files modified:** `crates/polint/src/analysis_kernel/incremental/stats.rs`
- **Verification:** `cargo test -p polint --test cli syntax_cache_ignores_unrelated_rule_edits --locked`
- **Committed in:** `46c8eaf`

**2. [Rule 1 - Bug] Made syntax cache test roots collision-proof under parallel tests**
- **Found during:** Overall verification
- **Issue:** The new Go/TS cache tests used process id plus timestamp for temp cache roots; the broad parallel `ts::` test filter exposed a possible root collision that could make corrupt-cache assertions inspect another test's layer files.
- **Fix:** Added per-module `AtomicU64` suffixes to Go and TS cache test root generation.
- **Files modified:** `crates/polint/src/go/tests.rs`, `crates/polint/src/ts/tests.rs`
- **Verification:** `cargo test -p polint --lib ts:: --locked`; `cargo test -p polint --lib go:: --locked`
- **Committed in:** `7388a1c`

---

**Total deviations:** 2 auto-fixed (2 bugs)
**Impact on plan:** Both fixes were required for reliable verification and did not widen public API or change user-facing behavior.

## Issues Encountered

- The first broad `cargo test -p polint --lib ts:: --locked` run failed in `ts_syntax_layer_cache_corrupt` because of the temp-root collision risk above. The isolation fix was committed and the same command passed afterward.

## Verification

- `cargo test -p polint --lib syntax_layer_key --locked`
- `cargo test -p polint --lib incremental::keys --locked`
- `cargo test -p polint --lib go_syntax_layer_cache --locked`
- `cargo test -p polint --lib ts_syntax_layer_cache --locked`
- `cargo test -p polint --test cli syntax_cache_ignores_unrelated_rule_edits --locked`
- `cargo test -p polint --lib go:: --locked`
- `cargo test -p polint --lib ts:: --locked`
- `cargo test -p polint --test cli input_snapshots_stay_internal --locked`
- `cargo fmt --all -- --check`

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 24-03 can build on crate-private, manifest-validated Go and TS/JS syntax layer reuse. Syntax cache internals remain private, corrupt entries fail closed, and unrelated rule edits are covered by an external-rule CLI regression.

## Self-Check: PASSED

- Verified summary and all modified source/test files exist on disk.
- Verified task commits exist: `c54007e`, `e5271da`, `f9d5359`, `07e2d32`, `46c8eaf`, `7388a1c`.

---
*Phase: 24-persistent-layer-cache-for-existing-cheap-facts*
*Completed: 2026-05-18*

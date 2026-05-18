---
phase: 24-persistent-layer-cache-for-existing-cheap-facts
plan: 01
subsystem: cache
tags: [rust, incremental-cache, layer-cache, invalidation, dependency-index]

requires:
  - phase: 23-input-snapshots-and-cache-key-vocabulary
    provides: Digest, LayerKey, CacheStats, and provider output metadata vocabulary
provides:
  - Crate-private dependency index and change-set vocabulary for cached analysis nodes
  - Conservative invalidation planner with fail-closed actions and stats
  - Crate-private layer cache manifest/store with digest-named blobs and manifest-last publication
  - Managed `.polint/cache/layers` layout category
affects: [phase-24, incremental-cache, analysis-kernel, provider-cache]

tech-stack:
  added: []
  patterns:
    - Crate-private serde manifest payloads
    - Digest-derived cache paths
    - Manifest-last cache publication

key-files:
  created:
    - crates/polint/src/analysis_kernel/incremental/dependency_index.rs
    - crates/polint/src/analysis_kernel/incremental/change_set.rs
    - crates/polint/src/analysis_kernel/incremental/invalidation.rs
    - crates/polint/src/analysis_kernel/incremental/layer_cache.rs
  modified:
    - crates/polint/src/analysis_kernel/incremental/mod.rs
    - crates/polint/src/analysis_kernel/incremental/keys.rs
    - crates/polint/src/cache/mod.rs

key-decisions:
  - "Layer-cache persistence remains crate-private under analysis_kernel::incremental with no SDK, runner, CLI, or public JSON surface."
  - "Layer payloads use digest-derived blob paths and manifests are published last under .polint/cache/layers."
  - "Invalidation planning fails closed for unknown, schema, provider, lifecycle, toolchain, model, extension, and missing dependency cases."
  - "Existing key structs derive ordering so CacheNode can support deterministic BTreeMap indexes."

patterns-established:
  - "Persistent layer manifests validate schema, key, payload digest, payload deserialization, and optional post-read output validation before Hit."
  - "Disabled layer-cache reads and writes return explicit bypass statuses without touching the filesystem."

requirements-completed: [SAE-FND-05]

duration: 13 min
completed: 2026-05-18
---

# Phase 24 Plan 01: Persistent Layer Cache Foundation Summary

**Crate-private layer cache foundation with deterministic dependency indexes, fail-closed invalidation, and safe manifest/blob persistence**

## Performance

- **Duration:** 13 min
- **Started:** 2026-05-18T10:25:45Z
- **Completed:** 2026-05-18T10:39:20Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments

- Added `DependencyIndex`, `CacheNode`, `DependencyEdge`, shape/kind vocabulary, and sorted forward/reverse edge storage.
- Added `ChangeSet` and `InvalidationPlan` with `Reuse`, `Verify`, `Recompute`, `Drop`, and `Quarantine` outcomes.
- Added `LayerCacheStore` and `LayerCacheManifest` with schema/key/payload validation, digest-named blobs, disabled bypasses, and manifest-last writes.
- Extended `CacheLayout` status/clean/prune handling with the managed `layers` category.

## Task Commits

Each TDD task was committed through RED and GREEN commits:

1. **Task 1: Dependency index, change set, and invalidation vocabulary**
   - `b399426` test: add failing tests for cache invalidation vocabulary
   - `6ab3f04` feat: implement cache invalidation vocabulary
2. **Task 2: Safe layer cache store and manifest format**
   - `853c82d` test: add failing tests for layer cache store
   - `3fde3db` feat: implement layer cache store

## Files Created/Modified

- `crates/polint/src/analysis_kernel/incremental/dependency_index.rs` - Versioned dependency index, cache node shapes, dependency edges, sorting, and deduplication.
- `crates/polint/src/analysis_kernel/incremental/change_set.rs` - Conservative change-kind vocabulary and sorted change rows.
- `crates/polint/src/analysis_kernel/incremental/invalidation.rs` - Fail-closed invalidation actions, reasons, stats, and planner.
- `crates/polint/src/analysis_kernel/incremental/layer_cache.rs` - Manifest format, layer cache store, safe read/write validation, disabled bypasses, and unit tests.
- `crates/polint/src/analysis_kernel/incremental/mod.rs` - Crate-private module wiring and re-exports.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Ordering derives for existing key types.
- `crates/polint/src/cache/mod.rs` - Managed `layers` cache directory, status, clean, and prune wiring.

## Decisions Made

- Layer cache internals stay crate-private and test-facing only; no public SDK, runner, CLI, or documented JSON contract was added.
- Layer cache payload filenames are derived only from validated digest values, and manifests are looked up from the requested `LayerKey`, not from untrusted manifest paths.
- Cache reads only return `Hit` after manifest schema, manifest key, payload digest, payload JSON, and optional post-deserialize validation pass.
- Disabled layer-cache mode returns explicit bypass statuses so providers can count `CacheStats::bypasses_disabled`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Made existing key structs orderable**
- **Found during:** Task 1 (Dependency index and invalidation vocabulary)
- **Issue:** `CacheNode` must store `LayerKey`, `QueryKey`, `SummaryKey`, and `DiagnosticKey` inside `BTreeMap` indexes, but those existing key structs did not implement ordering.
- **Fix:** Added `PartialOrd` and `Ord` derives to the existing key structs.
- **Files modified:** `crates/polint/src/analysis_kernel/incremental/keys.rs`
- **Verification:** `cargo test -p polint --lib dependency_index --locked`; `cargo test -p polint --lib invalidation --locked`; `cargo test -p polint --lib incremental --locked`
- **Committed in:** `6ab3f04`

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** The adjustment was required for deterministic dependency-index storage and did not widen public API.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 24-02 can build provider integration on top of the crate-private dependency index, invalidation vocabulary, layer manifest format, and managed `layers` cache layout.

## Self-Check: PASSED

- Verified created files exist on disk.
- Verified task commits exist: `b399426`, `6ab3f04`, `853c82d`, `3fde3db`.

---
*Phase: 24-persistent-layer-cache-for-existing-cheap-facts*
*Completed: 2026-05-18*

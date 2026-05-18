---
phase: 24-persistent-layer-cache-for-existing-cheap-facts
plan: 03
subsystem: cache
tags: [rust, incremental-cache, module-graph, layer-cache, invalidation]

requires:
  - phase: 24-persistent-layer-cache-for-existing-cheap-facts
    provides: LayerCacheStore, dependency index vocabulary, and syntax layer output digests from Plans 24-01 and 24-02
  - phase: 23-input-snapshots-and-cache-key-vocabulary
    provides: InputSnapshot, ProviderOutputMeta, CacheStats, and KernelRunReport
provides:
  - Module graph LayerKey construction using provider/schema, import, source/package, config, lifecycle, extension-absent, and upstream syntax digests
  - Module graph dependency edges for source text, import shape, lifecycle, config, provider schema, toolchain slot, and upstream syntax layers
  - Persistent module graph layer payloads with manifest validation, corrupt-cache recompute, disabled bypasses, and output digest reuse
  - Kernel run-report propagation of real `polint.module_graph` cache stats
affects: [phase-24, incremental-cache, module-graph, analysis-kernel]

tech-stack:
  added: []
  patterns:
    - Derived layer payload read/write through LayerCacheStore
    - Provider output digest reuse from validated layer-cache hits
    - Conservative module graph invalidation keyed by import, lifecycle, config, provider, source/package, and upstream syntax inputs

key-files:
  created: []
  modified:
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/incremental/dependency_index.rs
    - crates/polint/src/analysis_kernel/incremental/keys.rs
    - crates/polint/src/analysis_kernel/incremental/mod.rs
    - crates/polint/src/module_graph/mod.rs
    - crates/polint/src/module_graph/model.rs

key-decisions:
  - "Module graph cache identity includes provider/schema, import shape, source/package, config, Go lifecycle, TS/JS lifecycle, absent toolchain/extension slots, and upstream Go/TS syntax output digests."
  - "Module graph cache hits restore normalized facts through AnalysisDb::replace_module_graph_facts instead of bypassing metadata normalization."
  - "Disabled module graph caching records bypasses_disabled and recomputes without reading or writing layer-cache files."
  - "Module graph cache stats remain internal to KernelRunReport and do not change public check JSON."

patterns-established:
  - "Derived providers can expose a stats-returning cache path while preserving the existing compatibility derivation wrapper."
  - "Layer cache validators recompute payload/output metadata before merging cached facts into AnalysisDb."
  - "Kernel provider rows can carry provider-owned output digests when a layer cache hit is verified."

requirements-completed: [SAE-FND-05]

duration: 16 min
completed: 2026-05-18
---

# Phase 24 Plan 03: Module Graph Layer Cache Summary

**Persistent module graph layer cache with conservative import/lifecycle/config invalidation and internal run-report stats**

## Performance

- **Duration:** 16 min
- **Started:** 2026-05-18T11:08:25Z
- **Completed:** 2026-05-18T11:24:01Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments

- Added `LayerKey::module_graph_layer_key` and module graph dependency edges covering source text, import shape, package/source context, config, lifecycle, provider schema, toolchain, and upstream syntax layers.
- Added module graph layer payload persistence with manifest validation, output digest validation, corrupt-cache recompute, disabled bypass stats, and deterministic fact restore.
- Wired `AnalysisKernel::run` to pass Go/TS syntax output digests into the module graph cache identity and report real `polint.module_graph` cache stats.
- Added focused tests for cold/warm reuse, import and lifecycle invalidation, corrupt cache handling, disabled cache bypasses, and kernel report propagation.

## Task Commits

Each TDD task was committed atomically:

1. **Task 1: Add module graph layer key and dependency edges**
   - `31b59e4` test: add failing module graph cache identity tests
   - `bb0bf96` feat: add module graph cache identity inputs
2. **Task 2: Read and write cached module graph outputs through the kernel**
   - `1a81c17` test: add failing module graph layer cache tests
   - `2351a25` feat: persist module graph layer cache

## Files Created/Modified

- `crates/polint/src/analysis_kernel/mod.rs` - Passes upstream syntax output digests into module graph caching and records real module graph cache stats/output digest.
- `crates/polint/src/analysis_kernel/incremental/dependency_index.rs` - Adds dependency kinds needed for module graph source/import/schema/upstream edges.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Adds module graph layer key construction and upstream layer digest normalization.
- `crates/polint/src/analysis_kernel/incremental/mod.rs` - Re-exports the dependency-layer digest helper for crate-private provider wiring.
- `crates/polint/src/module_graph/mod.rs` - Adds cache key/dependency construction, layer read/write/restore path, stats accounting, and invalidation tests.
- `crates/polint/src/module_graph/model.rs` - Adds the serializable module graph layer payload shape.

## Decisions Made

- Module graph layer output digests include validated payload identity and layer key identity so lifecycle/config/upstream changes cannot stale-reuse an otherwise identical payload under the wrong dependencies.
- Cached module graph facts are restored via the existing `AnalysisDb::replace_module_graph_facts` normalization path to preserve deterministic IDs and metadata.
- Disabled cache mode uses the same derivation path but skips layer reads and writes, recording `bypasses_disabled` as internal cache stats only.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Known Stubs

None.

## Verification

- `cargo test -p polint --lib module_graph_layer_key --locked`
- `cargo test -p polint --lib module_graph_dependency_edges --locked`
- `cargo test -p polint --lib module_graph_layer_cache --locked`
- `cargo test -p polint --lib analysis_kernel --locked`
- `cargo test -p polint --lib module_graph --locked`
- `cargo test -p polint --lib incremental --locked`
- `cargo fmt --all -- --check`

## Next Phase Readiness

Plan 24-04 can layer symbol graph caching on top of verified syntax and module graph output digests. Module graph cache entries now fail closed on import, lifecycle, config, provider/schema, source/package, upstream syntax, disabled-cache, and corrupt-cache paths.

## Self-Check: PASSED

- Verified summary and all modified source files exist on disk.
- Verified task commits exist: `31b59e4`, `bb0bf96`, `1a81c17`, `2351a25`.

---
*Phase: 24-persistent-layer-cache-for-existing-cheap-facts*
*Completed: 2026-05-18*

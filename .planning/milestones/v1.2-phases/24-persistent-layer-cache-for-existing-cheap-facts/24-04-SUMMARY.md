---
phase: 24-persistent-layer-cache-for-existing-cheap-facts
plan: 04
subsystem: cache
tags: [rust, incremental-cache, symbol-graph, metrics, layer-cache]

requires:
  - phase: 24-persistent-layer-cache-for-existing-cheap-facts
    provides: LayerCacheStore, dependency edges, syntax output digests, and module graph output digests from Plans 24-01 through 24-03
  - phase: 23-input-snapshots-and-cache-key-vocabulary
    provides: Digest, LayerKey, ProviderOutputMeta, CacheStats, and KernelRunReport vocabulary
provides:
  - Persistent symbol/reference layer cache with conservative source/import/lifecycle/config/upstream invalidation
  - Persistent metrics layer cache with source/function/syntax/config invalidation
  - Kernel run-report propagation for polint.symbol_graph and polint.metrics cache stats and output digests
  - Public SDK compatibility proof for Symbols, References, FileMetrics, FunctionMetrics, and ComplexityMetrics
affects: [phase-24, incremental-cache, symbol-graph, metrics, analysis-kernel]

tech-stack:
  added: []
  patterns:
    - Derived layer payload read/write through LayerCacheStore
    - Provider-owned output digest reuse from validated layer-cache hits
    - Compatibility wrappers around stats-returning derived provider paths

key-files:
  created:
    - .planning/phases/24-persistent-layer-cache-for-existing-cheap-facts/24-04-SUMMARY.md
  modified:
    - crates/polint/src/analysis_kernel/incremental/digest.rs
    - crates/polint/src/analysis_kernel/incremental/keys.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/module_graph/mod.rs
    - crates/polint/src/symbol_graph/mod.rs
    - crates/polint/src/symbol_graph/model.rs
    - crates/polint/src/metrics.rs

key-decisions:
  - "Symbol graph cache identity includes source/function/package/import inputs, Go and TS/JS lifecycle digests, config, provider/schema, module graph output, syntax outputs, and absent extension slots."
  - "Metrics cache identity includes source text inputs, function fact inputs, config, upstream Go/TS syntax output digests, provider/schema, and absent lifecycle/toolchain/extension slots."
  - "Cached symbol/reference and metrics facts restore through existing AnalysisDb replacement methods so fact metadata is rebuilt through existing provider defaults."
  - "Derived layer cache stats remain internal to KernelRunReport and do not change public check JSON."

patterns-established:
  - "Symbol and metrics derived providers expose stats-returning cache paths while preserving compatibility wrappers."
  - "Layer cache hits use LayerCacheReadOutcome output digests as provider output metadata."
  - "Disabled derived-layer caching records bypasses_disabled and recomputes without touching layer-cache files."

requirements-completed: [SAE-FND-05]

duration: 19 min
completed: 2026-05-18
---

# Phase 24 Plan 04: Symbol and Metrics Layer Cache Summary

**Persistent symbol/reference and metrics layers with conservative upstream invalidation and internal run-report reuse stats**

## Performance

- **Duration:** 19 min
- **Started:** 2026-05-18T11:28:10Z
- **Completed:** 2026-05-18T11:47:02Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments

- Added `LayerKey::symbol_graph_layer_key` and symbol graph cache dependencies for source, function, package, import, lifecycle, config, module graph output, syntax output, provider/schema, toolchain, and extension inputs.
- Added symbol/reference layer payload persistence with manifest validation, corrupt-cache recompute, disabled bypass stats, deterministic restore, and provider output digest reuse.
- Added `LayerKey::metrics_layer_key` plus metrics payload persistence and invalidation over source text, function facts, syntax output digests, config, provider/schema, and absent extension slots.
- Wired `AnalysisKernel::run` to report real `polint.symbol_graph` and `polint.metrics` cache stats/output digests while preserving public SDK behavior.

## Task Commits

Each TDD task was committed atomically:

1. **Task 1: Add symbol/reference layer cache identity and persistence**
   - `c44a2f4` test: add failing symbol graph layer cache tests
   - `0b29a83` feat: persist symbol graph layer cache
2. **Task 2: Add metrics layer cache identity and persistence**
   - `ff6ac48` test: add failing metrics layer cache tests
   - `7905fd8` feat: persist metrics layer cache

## Files Created/Modified

- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Adds symbol graph and metrics layer key constructors and key invalidation tests.
- `crates/polint/src/analysis_kernel/mod.rs` - Passes upstream output digests into symbol/metrics cache paths and records provider-owned stats/output digests.
- `crates/polint/src/symbol_graph/mod.rs` - Adds symbol graph cache read/write/restore path, dependency edges, digest construction, stats accounting, and layer cache tests.
- `crates/polint/src/symbol_graph/model.rs` - Adds serializable symbol graph layer payload schema.
- `crates/polint/src/metrics.rs` - Adds metrics cache read/write/restore path, dependency edges, digest construction, stats accounting, and layer cache tests.
- `crates/polint/src/analysis_kernel/incremental/digest.rs` - Removes a stale dead-code expectation now that unordered digest construction is production cache-key behavior.
- `crates/polint/src/module_graph/mod.rs` - Marks the compatibility wrapper as intentional after the stats-returning cache path became the production kernel path.

## Decisions Made

- Symbol graph and metrics cache payloads store normalized facts only; no raw source, absolute paths, temp roots, timestamps, or run-local cache truth were introduced.
- Cache hits restore through `AnalysisDb::replace_symbol_graph_facts` and `AnalysisDb::replace_metric_facts`, preserving existing metadata refresh and public fact views.
- Corrupt or mismatched payloads are invalid reads followed by recompute, matching the existing module graph fail-closed pattern.
- Public CLI JSON and SDK contracts remain unchanged; cache observability is available through crate-private run reports and tests only.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Removed stale dead-code lint expectations exposed by derived cache wiring**
- **Found during:** Task 1 (Add symbol/reference layer cache identity and persistence)
- **Issue:** Once derived layer cache paths used `Digest::from_unordered` and stats-returning wrappers from integration builds, stale dead-code expectations and unannotated compatibility wrappers emitted warnings.
- **Fix:** Removed the stale `Digest::from_unordered` dead-code expectation and documented the module/symbol compatibility wrappers with scoped `expect(dead_code)` attributes.
- **Files modified:** `crates/polint/src/analysis_kernel/incremental/digest.rs`, `crates/polint/src/module_graph/mod.rs`, `crates/polint/src/symbol_graph/mod.rs`
- **Verification:** `cargo test -p polint --test cli kernel_metadata_preserves_public_check_behavior --locked`; `cargo test -p polint --lib symbol_graph_layer_cache --locked`
- **Committed in:** `0b29a83`

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** The cleanup keeps integration builds warning-clean without widening public API or changing behavior.

## Issues Encountered

- The first symbol graph cache test filter matched zero tests because the tests were under the broader derivation module. They were moved under a `symbol_graph_layer_cache` test module so the planned filter executes the cache cases.

## User Setup Required

None - no external service configuration required.

## Known Stubs

None.

## Verification

- `cargo test -p polint --lib symbol_graph_layer_cache --locked`
- `cargo test -p polint --lib symbol_graph_layer_key --locked`
- `cargo test -p polint --test cli kernel_metadata_preserves_public_check_behavior --locked`
- `cargo test -p polint --lib metrics_layer_cache --locked`
- `cargo test -p polint --lib metrics_layer_key --locked`
- `cargo test -p polint --lib kernel_run_report_metrics_row_carries_layer_cache_stats --locked`
- `cargo test -p polint --test cli external_rule_consumes_derived_metric_signals_through_public_sdk --locked`
- `cargo test -p polint --lib symbol_graph --locked`
- `cargo test -p polint --lib metrics --locked`
- `cargo test -p polint --lib analysis_kernel --locked`
- `cargo fmt --all -- --check`

## Next Phase Readiness

Plan 24-05 can build on persistent syntax, module graph, symbol/reference, and metrics layer caches with internal stats and fail-closed restore semantics. Existing Phase 24 dependencies remain intact.

## Self-Check: PASSED

- Verified summary and all modified source files exist on disk.
- Verified task commits exist: `c44a2f4`, `0b29a83`, `ff6ac48`, `7905fd8`.

---
*Phase: 24-persistent-layer-cache-for-existing-cheap-facts*
*Completed: 2026-05-18*

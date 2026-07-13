---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 12
subsystem: analysis-kernel
tags: [layer-cache, run-metadata, provider-output, cache-parity, typed-dependencies]

# Dependency graph
requires:
  - phase: 65-generation-manifest-and-metadata-mirroring
    plan: 11
    provides: Typed dependency edges and the manifest-relative layer sentinel
provides:
  - Exact compact LayerRunMetadata projected from normalized current-run manifests
  - Cache-path-independent semantic rows for hit, miss, disabled, invalid-read, and failed-write outcomes
  - Deterministic provider output metadata with sorted and deduplicated layer rows outside cache telemetry
affects: [phase-65-validation-events, phase-65-generation-records, phase-65-store-mirroring]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Build one normalized manifest before optional persistence and retain its semantic row regardless of cache outcome"
    - "Expand the manifest-relative dependency sentinel through the existing dependency-edge expansion path"
    - "Keep cache status and counters in telemetry while provider/layer semantic projections retain only identity-bearing metadata"

key-files:
  created:
    - .planning/phases/65-generation-manifest-and-metadata-mirroring/65-12-SUMMARY.md
  modified:
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/incremental/layer_cache.rs
    - crates/polint/src/analysis_kernel/incremental/stats.rs
    - crates/polint/src/analysis_kernel/incremental/run_report.rs
    - crates/polint/src/analysis_kernel/incremental/mod.rs
    - crates/polint/src/go/adapter.rs
    - crates/polint/src/ts/adapter.rs
    - crates/polint/src/module_graph/mod.rs
    - crates/polint/src/symbol_graph/mod.rs
    - crates/polint/src/metrics.rs

key-decisions:
  - "Use normalized LayerCacheManifest as the sole source for current-run layer metadata on every cache outcome"
  - "Disabled cache mode skips persistence only; failed writes retain the same pre-write manifest-derived semantic row"
  - "Sort and deduplicate LayerRunMetadata at ProviderOutputMeta construction while excluding CacheStats from semantic projections"
  - "Represent manifest validation with ProviderValidationStatus while preserving the native_trusted wire label"

patterns-established:
  - "Cache-path parity: equivalent provider outputs produce identical layer keys, payload/output digests, validation, warnings, and typed dependency edges"
  - "Compact retention: payload_digest is retained, while source, payload bodies, AST/MIR/CFG blobs, summaries, and graph adjacency are forbidden"
  - "Provider handoff: cache-capable derivations pass their current-run layer rows directly to ProviderOutputMeta without cache-directory discovery"

requirements-completed: [META-01, META-04]

# Metrics
duration: 25min
completed: 2026-07-13
---

# Phase 65 Plan 12: Exact Current-Run Layer Metadata Summary

**Normalized manifests now yield compact typed layer rows that reach provider run metadata identically across successful reuse, recomputation, disabled persistence, invalid reads, and failed writes.**

## Performance

- **Duration:** 25 min
- **Started:** 2026-07-13T17:38:09Z
- **Completed:** 2026-07-13T18:03:19Z
- **Tasks:** 1
- **Files modified:** 10 implementation files

## Accomplishments

- Added `LayerRunMetadata` and its semantic projection with exact `LayerKey`, output and payload digests, typed expanded dependencies, precision, typed validation, and deterministic warning codes.
- Refactored Go syntax, TS/JS syntax, module graph, module topology, symbol graph, and metrics so every recompute branch constructs one manifest before optional persistence; disabled and failed-write paths now retain the same semantic row as miss and hit paths.
- Propagated layer rows through each provider result into sorted/deduplicated `ProviderOutputMeta` rows while keeping cache counters and path status outside semantic projections.
- Added adapter-local five-path fixtures that prove exact branch selection through complete `CacheStats`, require failed-write diagnostics, and compare full semantic rows across all outcomes.
- Added typed validation wire round-trips, payload-digest-only retention checks, exact forbidden-field assertions, counter-independence coverage, 24 completion-order permutations, and kernel aggregation assertions for all six layer families.

## Task Commits

Each task was committed atomically:

1. **Task 1: Bubble exact compact layer metadata through every cache outcome** - `542d5275` (feat)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/incremental/layer_cache.rs` - Typed manifest validation plus the single normalized-manifest conversion into compact semantic layer metadata.
- `crates/polint/src/analysis_kernel/incremental/stats.rs` - Provider layer rows, semantic projection separation, deterministic sorting/deduplication, counter independence, and completion-order tests.
- `crates/polint/src/analysis_kernel/incremental/run_report.rs` - Real-provider construction with retained layer rows and a test-only empty-layer fixture helper.
- `crates/polint/src/analysis_kernel/incremental/mod.rs` - Curated internal re-exports for layer metadata and provider construction.
- `crates/polint/src/analysis_kernel/mod.rs` - Direct aggregation of all six cache-capable provider layer results into run metadata, with kernel handoff assertions.
- `crates/polint/src/go/adapter.rs` - Manifest-first Go syntax cache outcomes and the exact five-path parity fixture.
- `crates/polint/src/ts/adapter.rs` - Manifest-first TS/JS syntax cache outcomes and the exact five-path parity fixture.
- `crates/polint/src/module_graph/mod.rs` - Manifest-first module graph and module topology outcomes, including disabled and failed-write retention.
- `crates/polint/src/symbol_graph/mod.rs` - Manifest-first symbol graph outcomes with compact serialized-payload digest retention.
- `crates/polint/src/metrics.rs` - Manifest-first metrics outcomes with failed-write metadata retained through the kernel report.

## Decisions Made

- A normalized in-memory manifest is the authoritative handoff object. Cache reads return it, and recomputation constructs it before deciding whether persistence is enabled or succeeds.
- `LayerRunMetadata::from_manifest` reuses the existing dependency expansion function so the stored manifest-relative sentinel becomes the exact current `CacheNode::Layer(key)` without a second edge builder.
- Layer rows include validation because it is semantic trust metadata, but cache read/write status and every `CacheStats` counter remain telemetry and cannot affect semantic projections.
- Providers sort and deduplicate retained layer rows only at the `ProviderOutputMeta` boundary, making completion order irrelevant without changing producer behavior.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- The plan's literal `symbol_graph::tests` filter selects zero tests in the current module layout. The command passed, and the effective `symbol_graph::symbol_graph_derivation` filter was run separately with 10/10 passing.

## User Setup Required

None - this change is private analysis-kernel and cache-run metadata infrastructure.

## Verification

- Exact Go five-path metadata test: 1 passed.
- Exact TS/JS five-path metadata test: 1 passed.
- Layer-cache tests: 16 passed.
- Provider stats/projection tests: 7 passed, including all 24 completion-order permutations.
- Module graph tests: 34 passed.
- Symbol graph derivation tests: 10 passed; the plan's literal filter also passed with zero selected tests.
- Metrics tests: 30 passed.
- Kernel run-report aggregation tests: 11 passed; metrics write-error handoff test: 1 passed.
- `cargo check -p polint --all-features --locked`: passed.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy -p polint --all-features --locked -- -D warnings`: passed.
- Repository pre-commit hook passed, including workspace/all-target/all-feature Clippy with warnings denied.
- `git diff --check` passed; only the ten owned implementation files changed before summary creation, and no production cache-directory enumeration was introduced.

## Next Phase Readiness

- Exact layer identity, typed dependencies, and trust metadata now reach `ProviderOutputMeta`, ready for Plan 13 to add structured validation events to the same private run handoff.
- Cache telemetry remains isolated from semantic identity, so later generation/store records can consume the semantic projections without path-dependent drift.
- No blockers.

---
*Phase: 65-generation-manifest-and-metadata-mirroring*
*Completed: 2026-07-13*

## Self-Check: PASSED

The task commit exists; all ten scoped implementation files and this summary exist; every cache-capable result family reaches `ProviderOutputMeta`; exact Go/TS five-path parity, typed validation and payload-digest round-trips, forbidden-field exclusion, 24 completion orders, focused producer suites, all-feature compilation, strict lint, and the repository hook pass.

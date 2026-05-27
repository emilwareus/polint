---
phase: 32-summary-kernel-and-direct-summaries
plan: 04
subsystem: analysis
tags: [summary-provider, cache-key, provider-manifest, kernel-wiring, output-digest, layer-kind]

requires:
  - phase: 32-summary-kernel-and-direct-summaries
    plan: 01
    provides: "SummaryDomain trait, four core domain types, fact vocabulary enums"
  - phase: 32-summary-kernel-and-direct-summaries
    plan: 02
    provides: "SummaryOutput, SummaryStore, AnalysisDb summary storage and metadata refresh"
  - phase: 32-summary-kernel-and-direct-summaries
    plan: 03
    provides: "DirectSummaryBuilder::build producing four-domain SummaryOutput"
  - phase: 31-p0-abstract-domain-kernel
    provides: "Domain solver results consumed by summary builder"
  - phase: 30-direct-call-facts
    provides: "Direct call facts consumed by summary builder"
provides:
  - "direct_summaries_provider_parameter_digest() including all four domain IDs/versions"
  - "LayerKind::DirectSummaries variant and direct_summaries_layer_key constructor"
  - "polint.direct_summaries provider manifest after abstract_domains and before metrics"
  - "derive_direct_summaries_with_cache_stats calling builder, computing output digest, storing results"
  - "Kernel run sequence wiring for direct summaries provider"
affects: [32-summary-kernel-and-direct-summaries, 33-demand-queries-and-summary-scc-cache]

tech-stack:
  added: []
  patterns: ["Provider pattern: derive -> normalize -> output_digest -> store -> metadata", "Cache identity includes all upstream digests and absent future extension/model/toolchain slots per D-14"]

key-files:
  created:
    - "crates/polint/src/analysis/summaries/cache_key.rs"
    - "crates/polint/src/analysis/summaries/provider.rs"
  modified:
    - "crates/polint/src/analysis/summaries/mod.rs"
    - "crates/polint/src/analysis_kernel/provider.rs"
    - "crates/polint/src/analysis_kernel/incremental/keys.rs"
    - "crates/polint/src/analysis_kernel/mod.rs"
    - "crates/polint/src/analysis_kernel/incremental/run_report.rs"
    - "crates/polint/src/eval/fixtures.rs"
    - "crates/polint/src/eval/observed.rs"
    - "tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml"

key-decisions:
  - "Output digest includes abstract_domains_output_digest as a new upstream input alongside MIR, CFG, calls, symbol_graph, module_topology"
  - "Provider uses callable stable key map from mir_bodies for output digest row identity instead of function IDs"
  - "Cache identity uses absent extension/model/toolchain slots per D-14 for forward compatibility"

patterns-established:
  - "Direct summaries provider follows exact domains/provider.rs pattern: derive, build key maps, compute output digest, record cache stats, store in db"
  - "Provider ordering: polint.direct_summaries runs at position 10, between abstract_domains (9) and metrics (11)"

requirements-completed: [SAE-INT-02]

duration: 12min
completed: 2026-05-21
---

# Phase 32 Plan 04: Summary Provider and Kernel Wiring Summary

**Summary provider wired into kernel with parameter digest, output digest, LayerKind::DirectSummaries, and provider order between abstract_domains and metrics**

## Performance

- **Duration:** 12 min
- **Started:** 2026-05-21T18:59:40Z
- **Completed:** 2026-05-21T19:11:28Z
- **Tasks:** 2
- **Files modified:** 10

## Accomplishments
- Created summary provider parameter digest including all four domain IDs and versions (ControlEffects, CallEffects, MemoryEffects, DataFlowTito)
- Added LayerKind::DirectSummaries and direct_summaries_layer_key constructor with all upstream digest inputs and absent future slots
- Added polint.direct_summaries manifest with domain_observations/domain_events as inputs and five summary outputs
- Created provider.rs with derive_direct_summaries_with_cache_stats calling DirectSummaryBuilder::build, computing output digest, and storing results
- Wired provider into kernel run sequence between abstract_domains and metrics with correct dependency chain
- Updated all provider order assertions across tests, eval fixtures, and eval TOML (12 provider order vectors)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add summary provider manifest, parameter digest, and LayerKind::DirectSummaries** - `7884084` (feat)
2. **Task 2: Wire summary provider into kernel run sequence with output digest and run-report** - `3311262` (feat)

## Files Created/Modified
- `crates/polint/src/analysis/summaries/cache_key.rs` - Provider parameter digest with four domain IDs/versions and schema label
- `crates/polint/src/analysis/summaries/provider.rs` - derive_direct_summaries_with_cache_stats, output digest computation, provider order test, empty output determinism test
- `crates/polint/src/analysis/summaries/mod.rs` - Added cache_key and provider module declarations
- `crates/polint/src/analysis_kernel/provider.rs` - Added DIRECT_SUMMARIES_SCHEMA, polint.direct_summaries manifest, and manifest test
- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Added LayerKind::DirectSummaries variant and direct_summaries_layer_key constructor
- `crates/polint/src/analysis_kernel/mod.rs` - Wired direct summaries provider into kernel run method, updated provider order test assertions
- `crates/polint/src/analysis_kernel/incremental/run_report.rs` - Updated provider order assertion
- `crates/polint/src/eval/fixtures.rs` - Updated provider order indices for eval fixture runner
- `crates/polint/src/eval/observed.rs` - Updated provider order indices for observed kernel tests
- `tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml` - Added polint.direct_summaries at index 10

## Decisions Made
- Output digest includes abstract_domains_output_digest as upstream input, establishing the dependency chain for cache invalidation when domain results change
- Callable stable key map uses MirBodyId as the key for consistent identity between provider.rs and builder.rs
- Clone added to abstract_domains call for upstream dependency digests (semantic_mir, cfg, calls, symbol_graph, module_topology) since they are now shared between abstract_domains and direct_summaries providers

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added .clone() calls for dependency digests shared between abstract_domains and direct_summaries**
- **Found during:** Task 2
- **Issue:** The abstract_domains provider call at kernel mod.rs consumed (moved) dependency digest values that the new direct_summaries provider also needed
- **Fix:** Added .clone() to semantic_mir_dependency_output_digest, cfg_dependency_output_digest, calls_dependency_output_digest, symbol_dependency_output_digest, and module_topology_dependency_output_digest at the abstract_domains call site
- **Files modified:** `crates/polint/src/analysis_kernel/mod.rs`
- **Verification:** Full test suite passes (979 tests)
- **Committed in:** 3311262

**2. [Rule 3 - Blocking] Updated 5 additional provider order assertions across codebase**
- **Found during:** Task 1
- **Issue:** Adding polint.direct_summaries to PROVIDER_MANIFESTS broke existing provider order assertions in run_report.rs, kernel mod.rs tests, eval/fixtures.rs, eval/observed.rs, and the eval TOML fixture
- **Fix:** Updated all provider order vector assertions to include polint.direct_summaries at index 10 and shifted polint.metrics to index 11
- **Files modified:** `crates/polint/src/analysis_kernel/incremental/run_report.rs`, `crates/polint/src/analysis_kernel/mod.rs`, `crates/polint/src/eval/fixtures.rs`, `crates/polint/src/eval/observed.rs`, `tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml`
- **Verification:** All 979 tests pass
- **Committed in:** 7884084 and 3311262

---

**Total deviations:** 2 auto-fixed (2 blocking issues)
**Impact on plan:** Both were necessary mechanical fixes from adding a new provider to the manifest. No scope creep.

## Issues Encountered
None beyond the deviations documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Summary provider is fully wired into the kernel execution pipeline
- Direct summaries are computed after abstract domains and before metrics
- Cache identity includes all upstream digests and absent future slots per D-14
- Ready for Plan 32-05 (validation, debug, and eval integration)
- The polint.direct_summaries provider output is available in run reports for later observation and eval fixture consumption

## Self-Check: PASSED

- [x] `crates/polint/src/analysis/summaries/cache_key.rs` exists
- [x] `crates/polint/src/analysis/summaries/provider.rs` exists
- [x] Commit 7884084 verified
- [x] Commit 3311262 verified
- [x] All 979 tests pass
- [x] Formatting clean

---
*Phase: 32-summary-kernel-and-direct-summaries*
*Completed: 2026-05-21*

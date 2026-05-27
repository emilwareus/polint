---
phase: 35-framework-entrypoints-and-trust-boundaries
plan: 02
subsystem: analysis-entrypoints
tags: [provider-wiring, kernel-integration, cache-identity, output-digest]
dependency_graph:
  requires: [entrypoint-facts, entrypoint-store, entrypoint-ids]
  provides: [entrypoints-provider-kernel-wiring, entrypoints-cache-identity, entrypoints-output-digest]
  affects: [analysis-kernel-mod, analysis-kernel-run-report, eval-fixtures]
tech_stack:
  added: []
  patterns: [provider-output-with-optional-digest, upstream-digest-cloning, kernel-run-sequence-insertion]
key_files:
  created: []
  modified:
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/incremental/run_report.rs
    - crates/polint/src/eval/fixtures.rs
    - tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml
    - crates/polint/src/analysis/entrypoints/provider.rs
    - crates/polint/src/analysis/entrypoints/cache_key.rs
    - crates/polint/src/analysis/entrypoints/mod.rs
    - crates/polint/src/analysis_kernel/provider.rs
decisions:
  - Use provider_output_for_with_optional_digest for direct_summaries instead of metadata fallback after SCC closure
  - Clone upstream dependency digests before direct_summaries consumes them so entrypoints can reuse them
  - Direct summaries test updated from metadata-fallback assertion to deterministic cold/warm equality check
metrics:
  duration: 7 min
  completed: 2026-05-24
---

# Phase 35 Plan 02: Entrypoints Provider Manifest, Cache Key, and Kernel Wiring Summary

Provider manifest entry with schema, cache key with recognizer labels, provider function skeleton with empty output and deterministic digest, and kernel run-sequence integration after SCC closure.

## What Was Done

### Task 1: Create provider manifest entry, cache key, and provider function skeleton
- Added ENTRYPOINTS_SCHEMA constant to analysis_kernel/provider.rs with name "entrypoints-facts-1", version 1
- Inserted polint.entrypoints ProviderManifest in PROVIDER_MANIFESTS after polint.direct_summaries and before polint.extensions
- Manifest declares WholeRepoDerived kind, MultiLanguage scope, InMemoryDerived cache policy, SetupAware precision ceiling
- Manifest inputs include source_files, functions, symbols, references, semantic_imports, resolved_imports, import_to_package_edges, mir_bodies, mir_operations, places, call_sites, call_targets, unresolved_calls
- Manifest outputs: entrypoints, trust_boundaries, dispatch_edges, unresolved_framework
- Created entrypoints_provider_parameter_digest() in cache_key.rs with schema, output families, and recognizer labels
- Created derive_entrypoints_with_cache_stats() skeleton producing empty EntrypointOutput
- Created entrypoints_output_digest() following calls provider pattern with provider metadata, upstream digests, lifecycle components, and per-fact stable payload lines
- Created provider_error_diagnostic helper
- Created ENTRYPOINTS_PROVIDER_ID constant
- Updated provider_order test to include polint.entrypoints
- Updated entrypoints/mod.rs to declare cache_key and provider modules
- Pin test for parameter digest determinism
- Tests for empty output determinism, manifest declaration, and populated output with deterministic digest

### Task 2: Wire entrypoints provider into kernel run sequence
- Inserted derive_entrypoints_with_cache_stats call in AnalysisKernel::run after SCC closure and before polint.extensions
- Cloned upstream dependency digests (semantic_mir, cfg, calls, symbol, module_topology) before direct_summaries consumes them
- Pushed polint.entrypoints provider output with provider-computed digest via provider_output_for_with_optional_digest
- Updated direct_summaries to use provider_output_for_with_optional_digest with its actual digest (previously was using metadata fallback which diverged after SCC closure)
- Updated kernel_run_report_records_input_snapshot_and_provider_outputs test to include polint.entrypoints
- Updated provider_manifests_cover_existing_kernel_providers test to include polint.entrypoints
- Updated run_report test expected provider order to include polint.entrypoints
- Updated eval fixture expected provider order TOML to include polint.entrypoints at position 11
- Updated eval fixture Rust test to include polint.entrypoints at position 11
- Updated direct_summaries test from metadata-fallback assertion to deterministic cold/warm equality check

## Verification Results

- `cargo test -p polint --lib analysis_kernel::provider::tests::provider_order_matches_behavior_preserving_kernel_sequence` -- 1 passed
- `cargo test -p polint --lib analysis_kernel` -- 187 passed
- `cargo test -p polint --lib analysis::entrypoints` -- 16 passed
- `cargo check -p polint` -- succeeds (1 dead_code warning for accessor methods pending recognizer plans)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed direct_summaries provider output digest divergence after SCC closure**
- **Found during:** Task 2
- **Issue:** The original code stored `_direct_summaries_output_digest` (underscore-prefixed, unused in some code paths) and passed it via `provider_output_for_with_optional_digest`. Moving the provider_output push after SCC closure caused a pre-existing test to fail because the metadata-derived fallback digest changed after SCC closure modified the db.
- **Fix:** Restored `provider_output_for_with_optional_digest` with the actual provider-computed digest. Updated the test to verify deterministic cold/warm equality instead of metadata-fallback match.
- **Files modified:** crates/polint/src/analysis_kernel/mod.rs
- **Commit:** 59ad937

**2. [Rule 3 - Blocking] Plan referenced `_direct_summaries_output_digest` variable that did not exist in current code**
- **Found during:** Task 2
- **Issue:** Plan's Task 2 action step 1 said to remove the underscore from `_direct_summaries_output_digest` but this variable did not exist in the current code (direct_summaries.output_digest was not captured at all).
- **Fix:** Added `let direct_summaries_output_digest = direct_summaries.output_digest;` and passed it to `provider_output_for_with_optional_digest`.
- **Files modified:** crates/polint/src/analysis_kernel/mod.rs
- **Commit:** 59ad937

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | 6ca0e3f | feat(35-02): add entrypoints provider manifest, cache key, and provider function skeleton |
| 2 | 59ad937 | feat(35-02): wire entrypoints provider into kernel run sequence |

## Self-Check: PASSED

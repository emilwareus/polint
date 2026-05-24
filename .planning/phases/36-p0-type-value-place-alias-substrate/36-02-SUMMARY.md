---
phase: 36-p0-type-value-place-alias-substrate
plan: 02
subsystem: type-value-alias-provider
tags: [provider-manifest, analysis-db, cache-key, metadata, debug]
dependency_graph:
  requires: [private-type-value-alias-contracts]
  provides: [type-value-alias-provider, type-value-alias-storage, type-value-alias-output-digest, type-value-alias-metadata]
  affects: [analysis-kernel, analysis-db, provider-manifest, incremental-keys]
tech_stack:
  added: []
  patterns: [provider-output-digest, in-memory-derived-provider, fact-metadata-refresh, deterministic-debug-json]
key_files:
  created:
    - crates/polint/src/analysis/types/provider.rs
    - crates/polint/src/analysis/types/cache_key.rs
    - crates/polint/src/analysis/types/debug.rs
    - crates/polint/src/analysis/points_to/store.rs
  modified:
    - crates/polint/src/analysis/types/mod.rs
    - crates/polint/src/analysis/types/store.rs
    - crates/polint/src/analysis/points_to/mod.rs
    - crates/polint/src/analysis_kernel/provider.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/incremental/keys.rs
    - crates/polint/src/core/mod.rs
decisions:
  - `polint.type_value_alias` runs after `polint.entrypoints` and before `polint.extensions`, matching the Phase 36 extension-aware ordering decision.
  - The provider is initially deterministic empty-output wiring; later language/provider plans populate the private rows without changing the kernel contract.
  - Cache identity includes schema, config, Go/TS lifecycle components, upstream digests, extension/model/tool sentinels, and budget/precision parameters.
  - Phase 36 fact metadata uses `polint.type_value_alias` as both producer and layer id.
requirements-completed: []
metrics:
  completed: 2026-05-24
---

# Phase 36 Plan 02: Type/Value/Alias Provider, Storage, Cache Identity, and Metadata Summary

Wired the Phase 36 substrate into the private kernel as deterministic engine data.

## What Was Done

### Task 1: Add AnalysisDb storage and provider output replacement
- Added `AnalysisDb::replace_type_value_alias_facts` plus crate-private read accessors for type facts, narrowed type facts, value facts, allocation tokens, access paths, points-to constraints/sets, and alias answers.
- Added the aggregate `TypeValueAliasOutput` and `PointsToOutput`/`PointsToStore` normalization path.
- Added `analysis/types/provider.rs` with `derive_type_value_alias_with_cache_stats`, deterministic empty output, recompute stats, output digesting, and `AnalysisDb` storage.

### Task 2: Register provider manifest and kernel run step
- Added manifest `polint.type_value_alias` with `WholeRepoDerived`, `MultiLanguage`, `InMemoryDerived`, `SetupAware`, schema `type-value-alias-facts-1:1`, and all Phase 36 private outputs.
- Inserted the provider after `polint.entrypoints` and before `polint.extensions`.
- Updated provider order and kernel run-report expectations to include the new provider.

### Task 3: Add cache key, metadata, and debug scaffolding
- Added provider parameter digest code covering schema, outputs, precision tier, alias/points-to budgets, and extension/model/tool slots.
- Added snapshot-aware digest input coverage for config, lifecycle, upstream output digests, and extension/model/tool components.
- Added metadata attachment for Phase 36 fact families using `polint.type_value_alias` producer/layer id.
- Added deterministic test-facing debug JSON with counts by fact family, language, status, precision, alias status, and points-to budget.

## Verification Results

- `cargo test -p polint --lib analysis::types::provider --locked` -- 3 passed
- `cargo test -p polint --lib analysis::types::cache_key --locked` -- 1 passed
- `cargo test -p polint --lib analysis::types::debug --locked` -- 1 passed
- `cargo test -p polint --lib analysis_kernel::provider --locked` -- 12 passed
- `cargo test -p polint --lib analysis_kernel::metadata --locked` -- 6 passed
- `cargo check -p polint --locked` -- passed with expected dead-code warnings for Phase 36 private rows introduced before later provider population
- Public no-leak check passed for SDK, runner, README, and docs.

## Deviations from Plan

- Added `crates/polint/src/analysis/points_to/store.rs` because `AnalysisDb` storage needs a normalized points-to output/store alongside the other Phase 36 families.
- The plan's combined multi-filter cargo command was split into per-module `cargo test` invocations because `cargo test` accepts one test-name filter.

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1-3 | 741f168 | feat(36-02): wire type value alias provider |

## Self-Check: PASSED

---
phase: 38-local-plus-summary-projected-data-flow
plan: 02
subsystem: analysis-kernel
tags: [rust, data-flow, provider, cache]
requires:
  - phase: 38-local-plus-summary-projected-data-flow
    provides: data-flow facts and store
provides:
  - Data-flow provider manifest, cache parameter digest, output digest, and kernel execution order
affects: [analysis-kernel, incremental-cache, eval]
tech-stack:
  added: []
  patterns: [provider manifest after refined calls, deterministic provider digests]
key-files:
  created:
    - crates/polint/src/analysis/data_flow/cache_key.rs
    - crates/polint/src/analysis/data_flow/provider.rs
  modified:
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/provider.rs
    - crates/polint/src/analysis_kernel/incremental/run_report.rs
key-decisions:
  - "Run `polint.data_flow` after `polint.refined_calls` and before `polint.metrics`."
patterns-established:
  - "Data-flow provider output digest includes upstream provider digests, lifecycle inputs, model inputs, extension inputs, and serialized fact payloads."
requirements-completed: [SAE-PREC-03]
duration: 12min
completed: 2026-05-25
---

# Phase 38 Plan 02 Summary

**Data-flow provider wired into the kernel with deterministic cache identity**

## Accomplishments
- Added `DATA_FLOW_SCHEMA_LABEL` and provider parameter digest helpers.
- Added `polint.data_flow` to the provider manifest order and kernel run sequence.
- Updated provider-order eval expectations to account for the new provider position.

## Task Commits
1. **Provider, cache, and kernel wiring** - `bf41e6c` (feat)

## Verification
- `cargo check -p polint`
- `cargo test -p polint provider_order --lib`

## Deviations from Plan
None.

## Issues Encountered
Provider-order eval fixtures initially failed because they still expected metrics directly after refined calls; fixed the expected invariant order.

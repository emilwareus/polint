---
phase: 38-local-plus-summary-projected-data-flow
plan: 01
subsystem: analysis
tags: [rust, data-flow, facts, metadata]
requires:
  - phase: 36-type-value-alias-refined-calls
    provides: refined call and type/value fact patterns
provides:
  - Private data-flow fact IDs, row vocabulary, store normalization, and AnalysisDb metadata families
affects: [data-flow, analysis-kernel, metadata]
tech-stack:
  added: []
  patterns: [crate-private semantic fact families, stable-key metadata rows]
key-files:
  created:
    - crates/polint/src/analysis/data_flow/facts.rs
    - crates/polint/src/analysis/data_flow/store.rs
  modified:
    - crates/polint/src/analysis/ids.rs
    - crates/polint/src/analysis/mod.rs
    - crates/polint/src/analysis_kernel/metadata.rs
    - crates/polint/src/core/mod.rs
key-decisions:
  - "Keep data-flow facts crate-private and absent from SDK/runner surfaces."
patterns-established:
  - "DataFlowOutput normalizes by stable key and remaps run-local dense IDs before storage."
requirements-completed: [SAE-PREC-03]
duration: 10min
completed: 2026-05-25
---

# Phase 38 Plan 01 Summary

**Crate-private data-flow fact contracts with stable-key identity and AnalysisDb metadata**

## Accomplishments
- Added `DataFlowNodeId`, `DataFlowEdgeId`, `DataFlowModelId`, `DataFlowBudgetId`, and `DataFlowPathId`.
- Added private data-flow node, edge, model, status, precision, confidence, provenance, validation, and budget vocabulary.
- Added `DataFlowStore` normalization, endpoint validation, and indexes for later query work.
- Added `AnalysisDb` storage/accessors and metadata rows for all data-flow fact families.

## Task Commits
1. **Private data-flow contracts and store** - `bf41e6c` (feat)

## Verification
- `cargo check -p polint`
- `cargo test -p polint data_flow --lib`

## Deviations from Plan
None.

## Issues Encountered
None.

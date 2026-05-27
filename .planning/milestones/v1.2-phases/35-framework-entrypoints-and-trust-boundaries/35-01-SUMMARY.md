---
phase: 35-framework-entrypoints-and-trust-boundaries
plan: 01
subsystem: analysis-entrypoints
tags: [framework-facts, dense-ids, vocabulary-enums, store, validation]
dependency_graph:
  requires: []
  provides: [entrypoint-facts, trust-boundary-facts, dispatch-edge-facts, unresolved-framework-facts, entrypoint-store, entrypoint-ids]
  affects: [analysis-kernel-metadata, analysis-ids, analysis-mod]
tech_stack:
  added: []
  patterns: [dense-id-newtypes, normalized-output-container, validated-store-with-indexes, referential-integrity-checking]
key_files:
  created:
    - crates/polint/src/analysis/entrypoints/facts.rs
    - crates/polint/src/analysis/entrypoints/store.rs
    - crates/polint/src/analysis/entrypoints/mod.rs
  modified:
    - crates/polint/src/analysis/ids.rs
    - crates/polint/src/analysis_kernel/metadata.rs
    - crates/polint/src/analysis/mod.rs
decisions:
  - EntrypointOutput normalized() sorts by stable_key then reassigns sequential IDs starting from 0
  - EntrypointStore validates referential integrity: trust boundaries and dispatch edges must reference existing entrypoint stable keys
  - Four new FactFamily variants placed after ExtensionFact: Entrypoint, TrustBoundary, DispatchEdge, UnresolvedFramework
  - TriggerMetadata is a separate struct with optional fields rather than an enum to support mixed metadata across entrypoint kinds
metrics:
  duration: 5 min
  completed: 2026-05-23
---

# Phase 35 Plan 01: Framework Fact Types, Dense IDs, and EntrypointStore Summary

Four framework fact families with vocabulary enums, dense IDs, normalized output container, validated store with referential integrity indexes, and FactFamily enum extension.

## What Was Done

### Task 1: Define dense IDs, fact types, vocabulary enums, and FactFamily variants
- Added four dense ID newtypes to `analysis/ids.rs`: `EntrypointId`, `TrustBoundaryId`, `DispatchEdgeId`, `UnresolvedFrameworkId` with full derive set
- Created `analysis/entrypoints/facts.rs` with four fact structs:
  - `EntrypointFact` with 16 fields including kind, target_function, trigger_metadata, precision, provenance, confidence, status
  - `TrustBoundaryFact` with 13 fields including entrypoint_stable_key, source_kind, target_parameter
  - `FrameworkDispatchEdgeFact` with 13 fields including from_source, to_target, edge_kind
  - `UnresolvedFrameworkFact` with 11 fields including reason, evidence, scope_description
- Added 8 vocabulary enums: `EntrypointKind` (13 variants), `EntrypointPrecision` (5), `EntrypointProvenance` (3), `EntrypointConfidence` (3), `EntrypointStatus` (5), `TrustBoundarySourceKind` (14), `DispatchEdgeKind` (7), `UnresolvedFrameworkReason` (8)
- Added `TriggerMetadata` struct with optional method, path, tool_name, event_name, test_name fields
- Extended `FactFamily` enum with Entrypoint, TrustBoundary, DispatchEdge, UnresolvedFramework variants and labels
- Wired `pub(crate) mod entrypoints` in `analysis/mod.rs`

### Task 2: Create EntrypointOutput container and EntrypointStore with validated indexes
- Created `EntrypointOutput` with four Vec fields and `normalized()` method that sorts each vector by stable key and reassigns sequential IDs
- Created `EntrypointStore` with `from_output()` that normalizes, validates referential integrity (trust boundaries and dispatch edges must reference existing entrypoint stable keys), and builds BTreeMap indexes
- Six indexes: `entrypoints_by_kind`, `entrypoints_by_file`, `entrypoints_by_framework`, `trust_boundaries_by_entrypoint_key`, `dispatch_edges_by_entrypoint_key`, `unresolved_by_reason`
- Added accessor methods: `entrypoints()`, `trust_boundaries()`, `dispatch_edges()`, `unresolved()`, `output()`
- 7 unit tests covering normalization, index building, dangling reference rejection, and empty output

## Verification Results

- `cargo test -p polint --lib analysis::ids` -- 2 passed
- `cargo test -p polint --lib analysis::entrypoints` -- 12 passed
- `cargo check -p polint` -- succeeds with no errors (only expected dead_code warnings for new FactFamily variants)

## Deviations from Plan

None - plan executed exactly as written.

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | 869e0e6 | feat(35-01): define dense IDs, fact types, vocabulary enums, and FactFamily variants |
| 2 | 4397484 | feat(35-01): create EntrypointOutput container and EntrypointStore with validated indexes |

## Self-Check: PASSED

---
phase: 36-p0-type-value-place-alias-substrate
plan: 01
subsystem: private-type-value-alias-substrate
tags: [type-facts, value-facts, access-paths, points-to, aliases, stable-keys]
dependency_graph:
  requires: []
  provides: [private-type-facts, private-value-facts, private-access-path-facts, private-points-to-facts, private-alias-facts, phase-36-fact-families]
  affects: [analysis-ids, analysis-mod, analysis-kernel-metadata]
tech_stack:
  added: []
  patterns: [dense-id-newtypes, crate-private-fact-contracts, normalized-output-container, stable-key-sorting]
key_files:
  created:
    - crates/polint/src/analysis/types/facts.rs
    - crates/polint/src/analysis/types/store.rs
    - crates/polint/src/analysis/types/mod.rs
    - crates/polint/src/analysis/values/facts.rs
    - crates/polint/src/analysis/values/store.rs
    - crates/polint/src/analysis/values/mod.rs
    - crates/polint/src/analysis/access_paths/facts.rs
    - crates/polint/src/analysis/access_paths/store.rs
    - crates/polint/src/analysis/access_paths/mod.rs
    - crates/polint/src/analysis/points_to/facts.rs
    - crates/polint/src/analysis/points_to/mod.rs
    - crates/polint/src/analysis/aliases/facts.rs
    - crates/polint/src/analysis/aliases/store.rs
    - crates/polint/src/analysis/aliases/mod.rs
  modified:
    - crates/polint/src/analysis/ids.rs
    - crates/polint/src/analysis/mod.rs
    - crates/polint/src/analysis_kernel/metadata.rs
decisions:
  - Phase 36 type/value/access-path/points-to/alias contracts are private under `analysis`, with no SDK, runner, CLI, README, or docs exposure.
  - Existing `PlaceId` remains the semantic place identity; access paths layer projection facts over places rather than replacing place facts.
  - Output containers normalize by stable key and reassign dense IDs deterministically before later provider/storage wiring.
  - Alias answer vocabulary is exactly `NoAlias`, `MayAlias`, `MustAlias`, `PartialAlias`, and `Unknown`.
requirements-completed: []
metrics:
  completed: 2026-05-24
---

# Phase 36 Plan 01: Private Type, Value, Allocation, Access Path, and Alias Contracts Summary

Defined the private Phase 36 substrate contracts for type, value, allocation, access-path, points-to, and alias facts.

## What Was Done

### Task 1: Add dense IDs and crate-private modules
- Added 12 dense ID newtypes to `analysis/ids.rs`: `TypeFactId`, `TypeSetId`, `NarrowedTypeId`, `ValueFactId`, `AbstractValueId`, `AllocationTokenId`, `AccessPathId`, `PointsToConstraintId`, `PointsToSetId`, `PtVarId`, `ObjectTokenId`, and `AliasAnswerId`.
- Registered crate-private `analysis` modules for `types`, `values`, `access_paths`, `points_to`, and `aliases`.
- Left crate root, SDK, runner, CLI, README, and fact docs untouched.

### Task 2: Define fact rows and vocabulary enums
- Added `TypeFact` and `NarrowedTypeFact` with explicit type evidence phases for declared, inferred, resolved, flow-narrowed, extension-provided, unknown, unsupported, and setup-missing states.
- Added type shape vocabulary for primitive/literal/nullish, callable, object/class/module, nominal, structural, union/intersection, generic placeholder, Any, Unknown, and Unsupported distinctions.
- Added `ValueFact`, `AllocationTokenFact`, `AccessPathFact`, `PointsToConstraintFact`, `PointsToSetFact`, and `AliasAnswerFact`.
- Added alias status tests proving the required five-status vocabulary.

### Task 3: Add normalized stores and FactFamily variants
- Added normalized output containers and stores for type, value/allocation, access path, and alias facts.
- Each output container sorts by stable key and reassigns dense IDs from zero before storage.
- Extended `FactFamily` with Phase 36 variants and labels for type, narrowed type, value, allocation token, access path, points-to constraints/sets, alias answers, and type/value/alias events.

## Verification Results

- `cargo test -p polint --lib analysis::ids --locked` -- 2 passed
- `cargo test -p polint --lib analysis::types --locked` -- 3 passed
- `cargo test -p polint --lib analysis::values --locked` -- 2 passed
- `cargo test -p polint --lib analysis::access_paths --locked` -- 2 passed
- `cargo test -p polint --lib analysis::points_to --locked` -- 1 passed
- `cargo test -p polint --lib analysis::aliases --locked` -- 2 passed
- `cargo test -p polint --lib analysis_kernel::metadata --locked` -- 6 passed
- `cargo check -p polint --locked` -- passed with expected dead-code warnings for newly introduced private substrate variants before provider wiring
- Public no-leak check passed: no `Types<'_>`, `Values<'_>`, `Aliases<'_>`, or public module exports were found in SDK, runner, README, or docs.

## Deviations from Plan

- The plan's combined multi-filter cargo command was split into per-module `cargo test` invocations because `cargo test` accepts a single test-name filter.
- Points-to currently has fact contracts but no store module in this plan; solver storage/query wiring is intentionally covered by later Phase 36 plans.

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1-3 | b7e1971 | feat(36-01): add private type value alias contracts |

## Self-Check: PASSED

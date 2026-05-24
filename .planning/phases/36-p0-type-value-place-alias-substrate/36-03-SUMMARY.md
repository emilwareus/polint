---
phase: 36-p0-type-value-place-alias-substrate
plan: 03
subsystem: go-type-value-alias-seeds
tags: [go, type-facts, value-facts, access-paths, allocation-tokens, cache-key]
dependency_graph:
  requires: [type-value-alias-provider]
  provides: [go-type-seed-facts, go-value-seed-facts, go-access-path-facts, go-allocation-tokens]
  affects: [type-value-alias-provider, go-mir-lowering, cache-identity]
tech_stack:
  added: []
  patterns: [mir-derived-facts, place-backed-access-paths, unsupported-as-unknown, lifecycle-digest-sentinel]
key_files:
  created:
    - crates/polint/src/analysis/types/go.rs
  modified:
    - crates/polint/src/analysis/types/mod.rs
    - crates/polint/src/analysis/types/provider.rs
    - crates/polint/src/analysis/types/cache_key.rs
    - crates/polint/src/analysis/mir/lower_go.rs
decisions:
  - Go extraction is first-tier native seed extraction from existing MIR/place/unsupported rows, not an exact go/types claim.
  - Unsupported Go constructs produce Unknown or Unsupported Phase 36 rows, never Present/Exact rows.
  - Composite literals and function literals are exposed through existing Go MIR literal lowering so value/allocation rows can be derived deterministically.
  - Official Go lifecycle/toolchain participation is represented in the Phase 36 provider digest with explicit digest-changing sentinels; no repo lifecycle files are written.
requirements-completed: []
metrics:
  completed: 2026-05-24
---

# Phase 36 Plan 03: Go Type, Value, Allocation, Access Path, and Narrowing Facts Summary

Added the first Go population path for the Phase 36 private substrate.

## What Was Done

### Task 1: Extract Go type and access-path seed facts
- Added `analysis/types/go.rs` with `derive_go_type_value_alias`.
- Emits Go `TypeFact` rows from existing MIR places, using `PlaceId` as the subject and preserving `Language::Go`.
- Emits `AccessPathFact` rows for every Go place, including selector fields, indexes, and root receiver/parameter places.
- Maps unsupported or unknown Go place status to Unknown/Unsupported type/access-path facts rather than exact claims.

### Task 2: Add Go value and allocation tokens
- Derives Go `ValueFact` rows from MIR bind/assign/write/call operations.
- Classifies nil, booleans, strings, numbers, call returns, function objects, and unknown values.
- Added Go MIR lowering for composite literals and function literals as literal value evidence.
- Emits allocation tokens for composite literals and function objects with deterministic operation-based stable keys.

### Task 3: Include official-tool input digests when used
- Extended Phase 36 cache-key tests to prove Go lifecycle, Go tool invocation, and upstream provider digest changes alter the provider input digest.
- Kept official Go tooling inactive for this plan; no `go.work`, sidecar files, or repo lifecycle files are written.

## Verification Results

- `cargo test -p polint --lib analysis::types::go --locked` -- 3 passed
- `cargo test -p polint --lib analysis::types::cache_key --locked` -- 2 passed
- `cargo test -p polint --test cli --locked` -- 124 passed
- `cargo check -p polint --locked` -- passed with expected dead-code warnings for Phase 36 private accessors/event rows that later plans consume

## Deviations from Plan

- Eval fixture files were not added in this plan; the Go coverage is currently in focused unit tests plus the full CLI regression suite. Plan 36-07 remains responsible for consolidated eval/no-leak proof.
- No official Go tool provider is activated yet, so the implementation adds digest sentinels and lifecycle/tool tests rather than consuming live go/types output.

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1-3 | 43d2ac3 | feat(36-03): derive Go type value alias seeds |

## Self-Check: PASSED

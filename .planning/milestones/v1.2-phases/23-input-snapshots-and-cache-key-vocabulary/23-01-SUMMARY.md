---
phase: 23-input-snapshots-and-cache-key-vocabulary
plan: 01
subsystem: analysis-kernel-cache
tags: [rust, cache-identity, incremental, serde, analysis-kernel]

# Dependency graph
requires:
  - phase: 20-private-analysis-kernel-facade
    provides: crate-private analysis kernel and provider manifests
  - phase: 21-provenance-precision-and-validation-metadata
    provides: crate-private validation and precision vocabulary
  - phase: 22-internal-evaluation-harness-mvp
    provides: internal deterministic verification patterns
provides:
  - crate-private incremental module under analysis_kernel
  - typed Digest and DigestKind identity helpers
  - typed LayerKey, QueryKey, SummaryKey, and DiagnosticKey vocabulary
  - CacheStats and ProviderOutputMeta counters and provider metadata
affects: [phase-23, phase-24, incremental-cache, provider-output-metadata]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - crate-private serde structs for internal cache identity
    - canonical sorting of variable digest lists at construction boundaries
    - compatibility bridge from existing CacheKey fields into typed digests

key-files:
  created:
    - crates/polint/src/analysis_kernel/incremental/mod.rs
    - crates/polint/src/analysis_kernel/incremental/digest.rs
    - crates/polint/src/analysis_kernel/incremental/keys.rs
    - crates/polint/src/analysis_kernel/incremental/stats.rs
  modified:
    - crates/polint/src/analysis_kernel/mod.rs

key-decisions:
  - "Keep the incremental cache vocabulary crate-private under analysis_kernel with no SDK, runner, crate-root public, or CLI export."
  - "Bridge existing CacheKey fields into typed digest components rather than replacing current cache behavior."
  - "Sort every variable digest list at construction time so key equality is traversal-order independent."

patterns-established:
  - "Digest construction uses kind-aware, length-prefixed labeled parts before stable_hash."
  - "Compatibility cache fields are preserved as typed inputs for later invalidation work."
  - "Provider metadata starts as internal serializable vocabulary, not public report output."

requirements-completed: [SAE-FND-04]

# Metrics
duration: 9m
completed: 2026-05-18
---

# Phase 23 Plan 01: Input Snapshot and Cache-Key Vocabulary Summary

**Crate-private incremental cache identity vocabulary with deterministic digest helpers, typed keys, cache counters, and provider output metadata.**

## Performance

- **Duration:** 9m
- **Started:** 2026-05-18T06:02:03Z
- **Completed:** 2026-05-18T06:11:19Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- Added `analysis_kernel::incremental` as a crate-private module only.
- Implemented deterministic `Digest` construction, canonical unordered digest hashing, and explicit absent/unsupported digest helpers.
- Added `LayerKey`, `QueryKey`, `SummaryKey`, and `DiagnosticKey` with sorted digest-list constructors.
- Added `CacheStats` counters and `ProviderOutputMeta` serialization for future provider run reporting.
- Preserved existing cache compatibility by wrapping `CacheKey` fields into typed digest inputs.

## Task Commits

Each task was committed atomically:

1. **Task 1 RED: Digest and CacheStats tests** - `77e091f` (test)
2. **Task 1 GREEN: Digest and CacheStats implementation** - `3a65876` (feat)
3. **Task 2 RED: Key and provider metadata tests** - `c137b3b` (test)
4. **Task 2 GREEN: Key and provider metadata implementation** - `36be7e0` (feat)
5. **Refactor: rustfmt cleanup** - `d81391d` (refactor)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/mod.rs` - Declares the crate-private `incremental` module.
- `crates/polint/src/analysis_kernel/incremental/mod.rs` - Internal module boundary and crate-private re-exports for later consumers.
- `crates/polint/src/analysis_kernel/incremental/digest.rs` - Typed digest kind/value model, deterministic helpers, serde, display, and unit tests.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Layer/query/summary/diagnostic key types, constructors, cache bridge, and unit tests.
- `crates/polint/src/analysis_kernel/incremental/stats.rs` - Cache counters, provider output metadata, and unit tests.

## Decisions Made

- New identity types remain internal to `crates/polint`; no public SDK, runner, crate-root, CLI, or stable JSON contract was added.
- `LayerKey::from_existing_file_cache` includes `file_hash`, `config_hash`, `rule_hash`, `plan_hash`, `version`, and `schema` as typed digest inputs while leaving the existing cache key behavior intact.
- Re-export warnings are handled with scoped `#[expect(unused_imports)]` because this plan intentionally creates vocabulary before later plans wire production consumers.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- `cargo fmt --all -- --check` initially reported formatting changes. Applied `cargo fmt --all`, verified tests still passed, and committed the formatting-only refactor.
- `.planning/config.json` was dirty before summary creation with orchestrator-owned auto-chain state. It was not staged or modified by this plan's commits.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --lib incremental::digest --locked`
- `cargo test -p polint --lib incremental::stats --locked`
- `cargo test -p polint --lib incremental::keys --locked`
- `cargo test -p polint --lib incremental --locked`
- `cargo fmt --all -- --check`
- Public boundary greps confirmed no `incremental` exports from `lib.rs`, `sdk`, `runner`, or `cli`.
- `rg -n "^pub " crates/polint/src/analysis_kernel/incremental` returned no matches.

## Next Phase Readiness

Phase 23 can continue with input snapshot construction and broader cache vocabulary wiring. The typed key and provider metadata substrate is available internally without changing current cache reuse behavior.

## Self-Check: PASSED

- Created files exist: `incremental/mod.rs`, `digest.rs`, `keys.rs`, `stats.rs`, and this summary.
- Commits exist: `77e091f`, `3a65876`, `c137b3b`, `36be7e0`, and `d81391d`.
- Final verification passed: `cargo test -p polint --lib incremental --locked` and `cargo fmt --all -- --check`.

---
*Phase: 23-input-snapshots-and-cache-key-vocabulary*
*Completed: 2026-05-18*

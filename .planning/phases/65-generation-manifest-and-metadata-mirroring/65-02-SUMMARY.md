---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 02
subsystem: analysis-kernel
tags: [provider-metadata, query-identity, telemetry, digests, serde, eval]

# Dependency graph
requires:
  - phase: 65-generation-manifest-and-metadata-mirroring
    plan: 01
    provides: Canonical digest purposes, stable enum codecs, and run-ID-free provider output rows
provides:
  - Closed canonical provider-validation and demand-cache status vocabularies
  - Borrowed provider and query semantic projections that exclude execution telemetry
  - Full typed QueryKey retention in demand traces with deterministic semantic digests
  - One dependency-free QueryKey factory for cross-module renderer and eval fixtures
affects: [phase-65-layer-metadata, phase-65-query-dependencies, phase-65-store-commit-plan]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Closed status enums own canonical label parsing and serde rejection"
    - "Semantic projections whitelist identity fields while telemetry remains renderable separately"

key-files:
  created: []
  modified:
    - crates/polint/src/analysis_kernel/incremental/stats.rs
    - crates/polint/src/analysis_kernel/incremental/demand.rs
    - crates/polint/src/analysis_kernel/incremental/run_report.rs
    - crates/polint/src/analysis_kernel/incremental/mod.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/debug.rs
    - crates/polint/src/analysis/summaries/provider.rs
    - crates/polint/src/eval/performance.rs
    - crates/polint/src/eval/observed.rs

key-decisions:
  - "Provider metadata has exactly native_trusted and provider_failed validation states; metadata conflicts are provider failures rather than a third open label"
  - "Provider cache counters and query cache status/duration never enter semantic rows or semantic digests"
  - "Legacy debug and eval schemas render typed values explicitly, preserving established field names and strings"
  - "Cross-module demand-trace fixtures construct QueryKey only through the cfg(test) dependency_free_test_query_key seam"

patterns-established:
  - "Semantic/telemetry split: provider and query identity projections borrow canonical typed fields and omit runtime counters/status/time"
  - "Typed trace retention: DemandQueryTraceEntry owns QueryKey, result digest, result precision, and provenance"
  - "Source-boundary assertions accompany mutation tests so telemetry cannot silently re-enter semantic digest builders"

requirements-completed: [META-01, META-04]

# Metrics
duration: 29min
completed: 2026-07-12
---

# Phase 65 Plan 02: Provider and Query Semantic Boundary Summary

**Closed provider/query statuses and typed semantic projections now retain complete deterministic identity while keeping cache counters, cache outcomes, and timing as telemetry only.**

## Performance

- **Duration:** ~29 min
- **Started:** 2026-07-12T18:55:33Z
- **Completed:** 2026-07-12T19:24:30Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments

- Replaced provider validation strings with `ProviderValidationStatus`, including exact stable labels, typed unknown-label errors, serde compatibility, and exhaustive round-trip tests.
- Added a borrowed `ProviderSemanticProjection` that carries provider identity, output digest, precision, validation, and declared dependency inputs while structurally excluding `CacheStats`.
- Changed `DemandQueryTraceEntry` to retain the full typed `QueryKey`, typed result digest/precision/provenance, closed `DemandCacheStatus`, and separate duration telemetry.
- Added sorted/deduplicated semantic query projections and a canonical `DigestKind::Query` aggregate whose identity changes for key/result/precision/provenance mutations but not status or duration changes.
- Centralized dependency-free demand-trace fixture keys in one cfg(test), crate-private factory and preserved metadata-debug, eval performance, markdown/report, and observed output contracts through explicit renderers.

## Task Commits

Each task was committed atomically:

1. **Task 1: Type provider validation and isolate cache counters** - `d45457b8` (feat)
2. **Task 2: Retain typed query identity while excluding execution status and time** - `e4f5d733` (feat)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/incremental/stats.rs` - Closed provider status codec, borrowed semantic projection, and counter-isolation tests.
- `crates/polint/src/analysis_kernel/incremental/demand.rs` - Closed demand status codec, full typed trace rows, semantic query projection/digest, and sole cross-module test key factory.
- `crates/polint/src/analysis_kernel/incremental/run_report.rs` - Typed status aggregation and provider digest source-boundary proof.
- `crates/polint/src/analysis_kernel/incremental/mod.rs` - Curated private status exports and cfg(test)-only factory re-export.
- `crates/polint/src/analysis_kernel/mod.rs` - Provider output construction now assigns closed validation states.
- `crates/polint/src/analysis_kernel/debug.rs` - Legacy demand debug fields render from retained typed rows.
- `crates/polint/src/analysis/summaries/provider.rs` - SCC trace assertions consume typed query identity and status.
- `crates/polint/src/eval/performance.rs` - Provider/query eval rows explicitly render semantic fields and telemetry without changing schema.
- `crates/polint/src/eval/observed.rs` - Existing observed validation invariant renders the canonical provider label.

## Decisions Made

- Kept `ProviderOutputMeta.cache_stats` on the runtime metadata row for existing telemetry consumers, but made it impossible to obtain through `semantic_projection()`.
- Used the existing canonical digest builder and `DigestKind::Query` for semantic query aggregation; no alternate hash or debug-string identity path was introduced.
- Preserved the historical `SetupAware` demand-trace renderer spelling with explicit debug formatting while status labels remain canonical snake_case strings.
- Retained `DemandQueryResult.was_cached` as execution state but did not copy it into trace semantic projections or digests.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - AGENTS compliance] Replaced delivery-history comments with enduring demand-query invariants**

- **Found during:** Task 2 demand trace migration.
- **Issue:** Existing demand module comments and lint reasons referenced a numbered delivery plan, which the shipped-code comment policy forbids in touched source.
- **Fix:** Reworded the comments to describe run-scoped memoization, private consumers, and the semantic/telemetry boundary without roadmap chronology.
- **Files modified:** `crates/polint/src/analysis_kernel/incremental/demand.rs`, `crates/polint/src/analysis_kernel/incremental/mod.rs`
- **Verification:** Added-line scan found no phase, plan, milestone, or decision-number chronology in shipped changes; formatting and strict workspace Clippy passed.
- **Committed in:** `e4f5d733`

---

**Total deviations:** 1 auto-fixed (AGENTS policy compliance)
**Impact on plan:** Comment-only alignment in files already modified by the task; no API, schema, or product behavior changed.

## Issues Encountered

- The first Task 1 pre-commit run correctly rejected unused canonical parser/projection seams. Provider status deserialization now routes through the typed parser, and eval performance consumes the provider semantic projection before attaching cache telemetry. Focused Clippy and the retried full workspace hook then passed with zero warnings.

## User Setup Required

None - all changes are private kernel and evaluation vocabulary with no public configuration or external service requirements.

## Verification

- Provider status/projection tests: 6 passed.
- Provider output digest/run-report tests: 6 passed.
- Demand engine/status/semantic projection tests: 10 passed.
- Metadata-debug demand trace compatibility test: 1 passed.
- Eval performance compatibility tests: 6 passed.
- Eval observed compatibility tests: 13 passed.
- Eval markdown/report compatibility tests: 24 passed.
- `cargo check -p polint --all-features --locked`: passed.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed through both task commit hooks, including every workspace example rule crate.
- Acceptance scans: provider/query semantic builders contain no cache counters, cache status, `was_cached`, duration, or timestamp contributions; debug/eval consumer files contain no direct `QueryKey` constructor; exactly one cfg(test) dependency-free factory and one cfg(test) re-export exist.
- Stable eval fixture proof: `eval/performance.rs` still constructs explicit `StableFactMetaRow` values and calls the canonical provider digest without an opaque string-summary overload.
- Threat review: no new network, authentication, file-write, SQL, payload-body, or public API surface was introduced; typed parsers reject status spoofing and field-whitelisted projections prevent telemetry tampering from changing semantic identity.

## Next Phase Readiness

- Later layer metadata can reuse `ProviderValidationStatus` without reopening an open string surface.
- Query dependency vocabulary can extend the single dependency-free test factory in place while debug and eval consumers remain untouched.
- Store commit planning can consume deterministic provider/query semantic projections independently of cache telemetry.
- No blockers.

---
*Phase: 65-generation-manifest-and-metadata-mirroring*
*Completed: 2026-07-12*

## Self-Check: PASSED

All nine planned source files and this summary exist; task commits `d45457b8` and `e4f5d733` are present; every plan-focused, renderer-compatibility, compilation, formatting, strict-Clippy, semantic/telemetry mutation, and constructor-boundary check listed above passes.

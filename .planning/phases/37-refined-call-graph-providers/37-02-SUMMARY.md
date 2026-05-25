---
phase: 37-refined-call-graph-providers
plan: 02
subsystem: static-analysis
tags: [rust, provider, kernel, cache]
requires:
  - phase: 37-01
    provides: private refined call facts and store
provides:
  - polint.refined_calls provider manifest
  - Kernel execution step after polint.type_value_alias and before polint.metrics
  - Deterministic refined-call provider digest and provider-order eval updates
affects: [analysis-kernel, eval-fixtures, SAE-PREC-02]
tech-stack:
  added: []
  patterns: [provider manifest ordering, upstream digest cache identity, private provider output]
key-files:
  created:
    - crates/polint/src/analysis/refined_calls/cache_key.rs
    - crates/polint/src/analysis/refined_calls/provider.rs
    - crates/polint/src/analysis/refined_calls/validate.rs
  modified:
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/provider.rs
    - crates/polint/src/analysis_kernel/validation.rs
    - crates/polint/src/analysis_kernel/incremental/run_report.rs
    - crates/polint/src/eval/fixtures.rs
    - crates/polint/src/eval/observed.rs
    - tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml
key-decisions:
  - "Run polint.refined_calls after type/value/alias so later refinement tiers can consume those facts without adding cycles."
  - "Seed the provider by mirroring base call targets as DirectOnly refined edges, preserving baseline call behavior."
patterns-established:
  - "Refined-call cache identity includes upstream provider digests plus lifecycle/model/extension/tool inputs."
  - "Provider-order fixtures must include private providers when kernel order changes."
requirements-completed: [SAE-PREC-02]
duration: 50min
completed: 2026-05-24
---

# Phase 37 Plan 02: Refined Call Provider, Cache Identity, And Kernel Wiring Summary

**Deterministic private refined-call provider wired into the kernel after type/value/alias analysis**

## Performance

- **Duration:** 50 min
- **Started:** 2026-05-24T21:12:00Z
- **Completed:** 2026-05-24T22:02:33Z
- **Tasks:** 3
- **Files modified:** 14

## Accomplishments

- Added `REFINED_CALLS_SCHEMA_LABEL`, provider parameter digest, and deterministic provider-output digest.
- Added `derive_refined_calls_with_cache_stats`, initially emitting normalized DirectOnly refined edges from base call targets.
- Added `polint.refined_calls` to provider manifests after `polint.type_value_alias` and before `polint.metrics`.
- Wired kernel execution, provider output metadata, refined-call validation, and provider-order eval expectations.

## Task Commits

1. **Tasks 1-3: Provider, cache identity, and kernel wiring** - `dfff859` (feat)

## Files Created/Modified

- `crates/polint/src/analysis/refined_calls/cache_key.rs` - Schema label and parameter/input digest.
- `crates/polint/src/analysis/refined_calls/provider.rs` - Provider entrypoint and stable DirectOnly edge projection.
- `crates/polint/src/analysis/refined_calls/validate.rs` - Referential validation for refined edges.
- `crates/polint/src/analysis_kernel/provider.rs` - Manifest and provider-order test updates.
- `crates/polint/src/analysis_kernel/mod.rs` - Kernel execution step and digest threading.
- `tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml` - Provider-order fixture update.

## Decisions Made

The provider starts with DirectOnly baseline edges instead of an empty output. This makes the private layer immediately inspectable and testable while preserving direct call facts as the source of truth.

## Deviations from Plan

The manifest currently declares the refinement inputs used by this first slice plus planned downstream inputs. Framework, summary, Go, TS/JS, and extension refinement modules are present as empty private hooks for later Plan 37 slices.

## Issues Encountered

Provider-order tests initially failed because `eval::observed` had a separate hardcoded order expectation. Updated it alongside the fixture and manifest tests.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for Plan 37-03 framework dispatch refinements. Plans 37-03 through 37-06 remain incomplete.

---
*Phase: 37-refined-call-graph-providers*
*Completed: 2026-05-24*

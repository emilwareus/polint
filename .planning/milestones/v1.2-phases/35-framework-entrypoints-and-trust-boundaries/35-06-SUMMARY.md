---
phase: 35-framework-entrypoints-and-trust-boundaries
plan: 06
subsystem: analysis-entrypoints
tags: [validation, extension-merge, precision-ceiling, referential-integrity]
dependency_graph:
  requires: [entrypoint-facts, entrypoint-store, entrypoints-provider-kernel-wiring]
  provides: [entrypoints-validation, extension-framework-merge-awareness]
  affects: [analysis-kernel-validation, extension-validation, eval-observed]
tech_stack:
  added: []
  patterns: [metadata-validation-pipeline, precision-ceiling-enforcement, extension-merge-policy]
key_files:
  created:
    - crates/polint/src/analysis/entrypoints/validate.rs
  modified:
    - crates/polint/src/analysis/entrypoints/mod.rs
    - crates/polint/src/analysis_kernel/validation.rs
    - crates/polint/src/core/mod.rs
    - crates/polint/src/analysis/extensions/validate.rs
    - crates/polint/src/eval/observed.rs
decisions:
  - Entrypoint fact accessors promoted from #[cfg(test)] to production for validation use
  - Framework precision ceiling uses FrameworkPrecisionCeiling rejection reason separate from MissingProvenance
  - Framework fact families for extension validation identified by string matching against four family names
  - Conflicting entrypoint registrations detected by same target_function with different framework_ids
metrics:
  duration: 8 min
  completed: 2026-05-24
---

# Phase 35 Plan 06: Framework Fact Validation and Extension Merge Awareness Summary

Metadata validation for all four framework fact families with referential integrity, precision ceiling enforcement, conflicting registration detection, and extension overlay merge policy for framework-specific precision and native conflict handling.

## What Was Done

### Task 1: Create entrypoints validation and wire into kernel validation
- Created `analysis/entrypoints/validate.rs` with `validate_entrypoints(db, diagnostics)` function
- Validates duplicate stable keys for all four framework fact families (Entrypoint, TrustBoundary, DispatchEdge, UnresolvedFramework)
- Validates dangling function/symbol/file references on EntrypointFact (target_function, target_symbol, registration_file)
- Validates entrypoint_stable_key existence on TrustBoundaryFact (defense-in-depth behind store validation)
- Validates DispatchEdge references: to_target function, to_symbol, file, non-empty from_source
- Validates UnresolvedFrameworkFact: file reference, span validity
- Validates precision ceiling: rejects FactPrecision::Exact from polint.entrypoints producer rows
- Detects conflicting entrypoint registrations: multiple entrypoints targeting same function with different framework_ids
- Wired into kernel validation pipeline via `validate_fact_metadata` after `validate_summaries`
- Promoted entrypoint fact accessors from `#[cfg(test)]` to production visibility for validation access
- 9 unit tests: dangling function, dangling symbol, invalid span, duplicate stable key, precision ceiling, trust boundary reference, conflicting registrations, valid entrypoints, kernel integration

### Task 2: Add extension overlay merge awareness for framework facts
- Added `FrameworkPrecisionCeiling` variant to `ExtensionRejectionReason` enum
- Added framework fact family detection (entrypoint, trust_boundary, dispatch_edge, unresolved_framework)
- Extension facts with framework families and Exact precision are unconditionally rejected per D-18
- Native conflict detection for extension framework facts uses existing NativeConflict path
- Non-conflicting extension framework facts pass validation with their extension precision ceiling (SetupAware, Heuristic, GeneratedUnvalidated)
- Updated eval/observed.rs `extension_rejection_reason_label` match for new variant
- 6 new tests: exact precision rejection for entrypoint family, exact rejection for all framework families, native conflict rejection, non-conflicting acceptance with extension precision, non-exact precision acceptance for all three valid tiers

## Verification Results

- `cargo test -p polint --lib analysis::entrypoints::validate` -- 9 passed
- `cargo test -p polint --lib analysis::extensions::validate` -- 9 passed
- `cargo test -p polint --lib analysis_kernel::validation` -- 24 passed
- `cargo check -p polint` -- succeeds with no errors

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Entrypoint fact accessors were #[cfg(test)] only**
- **Found during:** Task 1
- **Issue:** The `entrypoint_facts()`, `trust_boundary_facts()`, `dispatch_edge_facts()`, and `unresolved_framework_facts()` methods on AnalysisDb were behind `#[cfg(test)]`, but the validation code runs in production and needs to access these fact vectors.
- **Fix:** Removed `#[cfg(test)]` attributes from all four entrypoint fact accessor methods in core/mod.rs.
- **Files modified:** crates/polint/src/core/mod.rs
- **Commit:** ef3f73d

**2. [Rule 3 - Blocking] ExtensionRejectionReason match exhaustiveness**
- **Found during:** Task 2
- **Issue:** Adding `FrameworkPrecisionCeiling` to `ExtensionRejectionReason` broke the exhaustive match in `eval/observed.rs`.
- **Fix:** Added the new variant to the match in `extension_rejection_reason_label`.
- **Files modified:** crates/polint/src/eval/observed.rs
- **Commit:** 8e4fd28

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | ef3f73d | feat(35-06): add entrypoints validation and wire into kernel validation |
| 2 | 8e4fd28 | feat(35-06): add extension overlay merge awareness for framework facts |

## Self-Check: PASSED

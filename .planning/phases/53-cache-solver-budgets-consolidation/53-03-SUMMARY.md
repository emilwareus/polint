---
phase: 53-cache-solver-budgets-consolidation
plan: 03
subsystem: analysis
tags: [rust, solver, budgets, unknown-taxonomy]

provides:
  - Stable crate-private solver budget reason labels
  - Solver cache invalidation when budget reason taxonomy changes
affects: [phase-53, solver, cache-invalidation, budget-taxonomy]

key-files:
  modified:
    - crates/polint/src/analysis/solver/budget.rs
    - crates/polint/src/analysis/solver/cache_key.rs

requirements-completed: [CACHE-02]
completed: 2026-06-05
---

# Phase 53 Plan 03 Summary

Added a crate-private `BudgetReason` taxonomy for solver, Go, JS token, object model, and adaptation budget ceilings. The solver provider digest now includes the ordered reason labels, so changing the taxonomy invalidates solver-derived caches deliberately.

## Verification

- `cargo test -p polint --lib budget_reason_labels_are_stable_and_specific` - passed
- `cargo test -p polint --lib cache_key` - passed
- `cargo test -p polint --lib` - passed

## Deviations

The existing unknown taxonomy path already carries `BudgetExceeded` categorization and reason strings for solver unknowns. This slice did not thread the new enum into every producing solver path because doing so would require broader driver-specific plumbing than Phase 53 needs for cache consolidation.

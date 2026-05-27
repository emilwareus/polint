---
phase: 31-p0-abstract-domain-kernel
fixed_at: 2026-05-21T13:05:50Z
review_path: .planning/phases/31-p0-abstract-domain-kernel/31-REVIEW.md
iteration: 1
findings_in_scope: 4
fixed: 4
skipped: 0
status: all_fixed
---

# Phase 31: Code Review Fix Report

**Fixed at:** 2026-05-21T13:05:50Z
**Source review:** .planning/phases/31-p0-abstract-domain-kernel/31-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 4
- Fixed: 4
- Skipped: 0

## Fixed Issues

### WR-01: Top Joins Are Not Commutative For Different Reasons

**Files modified:** `crates/polint/src/analysis/domains/core.rs`
**Commit:** 123fd56
**Applied fix:** Canonicalized different top-reason joins to `ConflictingFacts`, made `ConflictingFacts` an upper bound in `leq`, and added a regression test for commutative top joins. Status: fixed, requires human verification.

### WR-02: Overwrite And Copy Transfers Leave Stale Target Facts

**Files modified:** `crates/polint/src/analysis/domains/transfer.rs`
**Commit:** 1e652d3
**Applied fix:** Added strong-update clearing for assignment targets before literal, copy, and unknown writes while preserving self-copy behavior, with stale-slot regression tests. Status: fixed, requires human verification.

### WR-03: Branch Refinement Treats Predicate IDs As Place IDs

**Files modified:** `crates/polint/src/analysis/mir/op.rs`, `crates/polint/src/analysis/mir/lower_go.rs`, `crates/polint/src/analysis/mir/lower_ts.rs`, `crates/polint/src/analysis/provider.rs`, `crates/polint/src/analysis/domains/solver.rs`, `crates/polint/src/analysis/domains/transfer.rs`, `crates/polint/src/analysis/cfg/lower_go.rs`, `crates/polint/src/analysis/cfg/lower_ts.rs`
**Commit:** 360f120
**Applied fix:** Added an optional branch predicate place to the private MIR branch operation and made edge transfer refine only when a real place is available, with a conservative no-place regression test. Status: fixed, requires human verification.

### WR-04: Validation Failure Diagnostics Leak Private Domain Names Publicly

**Files modified:** `crates/polint/src/analysis/domains/validate.rs`, `crates/polint/src/analysis_kernel/validation.rs`
**Commit:** b74ea18
**Applied fix:** Replaced public abstract-domain validation details with a generic internal diagnostic and moved private family/key/field/reason data to `tracing::debug!`.

## Skipped Issues

None.

---

_Fixed: 2026-05-21T13:05:50Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_

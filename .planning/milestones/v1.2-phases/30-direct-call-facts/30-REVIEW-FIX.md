---
phase: 30-direct-call-facts
fixed_at: 2026-05-21T10:08:34Z
review_path: .planning/phases/30-direct-call-facts/30-REVIEW.md
iteration: 1
findings_in_scope: 4
fixed: 4
skipped: 0
status: all_fixed
---

# Phase 30: Code Review Fix Report

**Fixed at:** 2026-05-21T10:08:34Z
**Source review:** .planning/phases/30-direct-call-facts/30-REVIEW.md
**Iteration:** 1

**Summary:**
- Findings in scope: 4
- Fixed: 4
- Skipped: 0

## Fixed Issues

### WR-01: Constructor calls are recorded as direct references

**Files modified:** `crates/polint/src/analysis/calls/direct.rs`, `crates/polint/src/eval/fixtures.rs`, `tests/eval-fixtures/direct-calls/core/expected.polint-eval.toml`
**Commit:** 1329a1c
**Status:** fixed: requires human verification
**Applied fix:** Classified `Constructor` and `New` call sites as `ConstructorBinding`, added focused unit coverage, and added direct-call eval coverage for the constructor-binding algorithm count.

### WR-02: Resolved instance member calls are mislabeled as static members

**Files modified:** `crates/polint/src/analysis/calls/direct.rs`
**Commit:** 963a879
**Status:** fixed: requires human verification
**Applied fix:** Split static and instance member classification so resolved `Member` sites use `DirectMember`, with unit coverage asserting `DirectMember` and `MethodDirect`.

### WR-03: Calls manifest omits fact families the provider actually reads

**Files modified:** `crates/polint/src/analysis_kernel/provider.rs`
**Commit:** dd90984
**Status:** fixed
**Applied fix:** Added `semantic_imports` and `unsupported_semantics` to the private calls provider manifest and updated manifest metadata assertions.

### WR-04: Calls output digest ignores target function identity

**Files modified:** `crates/polint/src/analysis/calls/provider.rs`
**Commit:** b7e57e8
**Status:** fixed: requires human verification
**Applied fix:** Included stable target function and symbol keys in calls output digest rows and added regressions proving target identity changes affect the digest while dense ID-only shifts remain stable.

---

_Fixed: 2026-05-21T10:08:34Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_

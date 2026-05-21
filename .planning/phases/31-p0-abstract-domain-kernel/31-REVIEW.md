---
phase: 31-p0-abstract-domain-kernel
reviewed: 2026-05-21T12:55:54Z
depth: standard
files_reviewed: 38
files_reviewed_list:
  - crates/polint/src/analysis/domains/cache_key.rs
  - crates/polint/src/analysis/domains/core.rs
  - crates/polint/src/analysis/domains/facts.rs
  - crates/polint/src/analysis/domains/lattice.rs
  - crates/polint/src/analysis/domains/mod.rs
  - crates/polint/src/analysis/domains/provider.rs
  - crates/polint/src/analysis/domains/results.rs
  - crates/polint/src/analysis/domains/solver.rs
  - crates/polint/src/analysis/domains/state.rs
  - crates/polint/src/analysis/domains/store.rs
  - crates/polint/src/analysis/domains/transfer.rs
  - crates/polint/src/analysis/domains/validate.rs
  - crates/polint/src/analysis/ids.rs
  - crates/polint/src/analysis/mod.rs
  - crates/polint/src/analysis_kernel/debug.rs
  - crates/polint/src/analysis_kernel/incremental/keys.rs
  - crates/polint/src/analysis_kernel/incremental/run_report.rs
  - crates/polint/src/analysis_kernel/metadata.rs
  - crates/polint/src/analysis_kernel/mod.rs
  - crates/polint/src/analysis_kernel/provider.rs
  - crates/polint/src/analysis_kernel/validation.rs
  - crates/polint/src/core/mod.rs
  - crates/polint/src/eval/fixtures.rs
  - crates/polint/src/eval/matcher.rs
  - crates/polint/src/eval/metrics.rs
  - crates/polint/src/eval/mod.rs
  - crates/polint/src/eval/model.rs
  - crates/polint/src/eval/observed.rs
  - crates/polint/src/eval/report.rs
  - crates/polint/tests/cli.rs
  - tests/eval-fixtures/abstract-domains/core/expected.polint-eval.toml
  - tests/eval-fixtures/abstract-domains/core/repo/.polint.toml
  - tests/eval-fixtures/abstract-domains/core/repo/domain.go
  - tests/eval-fixtures/abstract-domains/core/repo/go.mod
  - tests/eval-fixtures/abstract-domains/core/repo/web/package.json
  - tests/eval-fixtures/abstract-domains/core/repo/web/src/domain.ts
  - tests/eval-fixtures/abstract-domains/core/repo/web/tsconfig.json
  - tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml
findings:
  critical: 0
  warning: 4
  info: 0
  total: 4
status: issues_found
---

# Phase 31: Code Review Report

**Reviewed:** 2026-05-21T12:55:54Z
**Depth:** standard
**Files Reviewed:** 38
**Status:** issues_found

## Summary

Reviewed the Phase 31 abstract-domain kernel, provider/cache identity, validation/debug/eval wiring, public no-leak tests, and fixture expectations. The new surfaces remain crate-private in normal public SDK/CLI/docs paths, but the domain kernel has correctness risks in lattice joins and transfer state updates that can materialize wrong observations. Validation failure diagnostics also expose private abstract-domain names on the public diagnostic path.

## Warnings

### WR-01: Top Joins Are Not Commutative For Different Reasons

**File:** `crates/polint/src/analysis/domains/core.rs:138`
**Issue:** Each domain returns whichever `Top(reason)` appears first when joining top values. For example, `Top(SetupMissing).join(Top(UnsupportedSemantic))` returns `SetupMissing`, while the reverse returns `UnsupportedSemantic`. Because `leq` only treats equal top reasons as comparable, this result is not an upper bound for the other top value. That violates the lattice contract and can make diagnostics/statuses depend on merge order.
**Fix:** Make differing top reasons join to a canonical upper bound, or store a deterministic set of reasons.

```rust
fn join_top_reasons(left: TopReason, right: TopReason) -> TopReason {
    if left == right {
        left
    } else {
        TopReason::ConflictingFacts
    }
}
```

Use that helper in all domain `join` implementations before handling non-top cases.

### WR-02: Overwrite And Copy Transfers Leave Stale Target Facts

**File:** `crates/polint/src/analysis/domains/transfer.rs:213`
**Issue:** `assign_value` marks initializedness and updates only the slot touched by the incoming value. `copy_place` only copies source slots that exist. If `target` previously had constant/string/truthiness facts and is overwritten from a source without those facts, the old target facts remain and are later emitted as current observations. Literal overwrites have the same stale-slot problem for derived slots such as `strings`.
**Fix:** Strong-update the target place before applying an overwrite/copy, then populate absent source slots as unknown/top or leave them absent consistently.

```rust
fn clear_place_facts(state: &mut ProductState, place: PlaceId) {
    state.core.nilness.remove(&place);
    state.core.truthiness.remove(&place);
    state.core.constants.remove(&place);
    state.core.strings.remove(&place);
    state.core.initializedness.remove(&place);
}
```

Call this before overwrite/bind/copy writes, then insert the new facts for the assignment.

### WR-03: Branch Refinement Treats Predicate IDs As Place IDs

**File:** `crates/polint/src/analysis/domains/transfer.rs:188`
**Issue:** `apply_branch_assumption` converts `MirPredicateId` directly into `PlaceId`. These are separate run-local ID spaces; predicate IDs are derived from syntax/operation ordinals, not from place allocation. When the numeric values happen to overlap, the solver can refine an unrelated place as truthy/falsy/non-nil. When they do not overlap, fake places are added to product state and may create dangling or duplicate observations.
**Fix:** Do not synthesize `PlaceId` from `MirPredicateId`. Carry predicate operand information in MIR/CFG, look up the predicate's source place explicitly, and only refine when the predicate resolves to a real `PlaceId`. Otherwise keep the branch state conservative.

### WR-04: Validation Failure Diagnostics Leak Private Domain Names

**File:** `crates/polint/src/analysis/domains/validate.rs:491`
**Issue:** Validation failures are appended to the normal diagnostic stream with messages/evidence containing `Abstract-domain`, `DomainObservation`, `DomainEvent`, stable keys, fields, and reasons. The public no-leak tests cover successful runs, but a malformed internal row would expose private abstract-domain internals through CLI JSON/SARIF output.
**Fix:** Keep public diagnostics generic and move private details to an internal debug/log channel.

```rust
Diagnostic::error(
    "polint/internal",
    "<workspace>",
    TextRange::point(1, 1),
    "Internal analysis validation failed.",
)
```

Avoid attaching private family names or stable keys to public diagnostic evidence.

---

_Reviewed: 2026-05-21T12:55:54Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

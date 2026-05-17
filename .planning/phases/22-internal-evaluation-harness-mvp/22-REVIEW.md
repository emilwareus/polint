---
phase: 22-internal-evaluation-harness-mvp
reviewed: 2026-05-17T18:04:00Z
depth: standard
files_reviewed: 23
files_reviewed_list:
  - crates/polint/src/eval/fixtures.rs
  - crates/polint/src/eval/matcher.rs
  - crates/polint/src/eval/metrics.rs
  - crates/polint/src/eval/mod.rs
  - crates/polint/src/eval/model.rs
  - crates/polint/src/eval/observed.rs
  - crates/polint/src/eval/report.rs
  - crates/polint/src/lib.rs
  - crates/polint/tests/cli.rs
  - tests/eval-fixtures/README.md
  - tests/eval-fixtures/cache/current-determinism/expected.polint-eval.toml
  - tests/eval-fixtures/cache/current-determinism/repo/.polint.toml
  - tests/eval-fixtures/cache/current-determinism/repo/component.tsx
  - tests/eval-fixtures/cache/current-determinism/repo/payment.go
  - tests/eval-fixtures/extension/rejection-delta/expected.polint-eval.toml
  - tests/eval-fixtures/extension/rejection-delta/repo/.polint.toml
  - tests/eval-fixtures/extension/rejection-delta/repo/src/app.ts
  - tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml
  - tests/eval-fixtures/kernel/provider-order/repo/.polint.toml
  - tests/eval-fixtures/kernel/provider-order/repo/src/app.ts
  - tests/eval-fixtures/provenance/metadata/expected.polint-eval.toml
  - tests/eval-fixtures/provenance/metadata/repo/.polint.toml
  - tests/eval-fixtures/provenance/metadata/repo/src/app.ts
findings:
  critical: 0
  warning: 1
  info: 0
  total: 1
status: issues_found
---

# Phase 22: Code Review Report

**Reviewed:** 2026-05-17T18:04:00Z
**Depth:** standard
**Files Reviewed:** 23
**Status:** issues_found

## Summary

Reviewed the internal eval model, matcher, metrics, fixture loader, observed-data collection, report normalization, crate visibility guard, CLI regression test, and native fixture manifests/repos. The eval module remains crate-private and the fixture path handling rejects absolute paths, parent traversal, and fixture symlinks. One determinism bug remains in report normalization.

Verification run during review:

```bash
cargo test -p polint eval_ -- --nocapture
cargo clippy -p polint --all-targets --all-features --locked -- -D warnings
```

Both commands passed.

## Warnings

### WR-01: Report Normalization Leaves Equal-Key Items In Caller Order

**File:** `crates/polint/src/eval/report.rs:94`

**Issue:** `normalize_run` sorts `case.expected` and `case.observed` with keys that omit fields still serialized into the eval JSON. Examples include `ExpectedDiagnostic.false_positive_trap`, `ExpectedFact.false_positive_trap`, and observed `provenance` across item kinds; observed diagnostics, graph edges, paths, and invariants also omit `precision`. Because `sort_by_key` is stable, two same-identity items that differ only in an omitted serialized field keep their input order. If a provider emits those rows in a different order, `to_deterministic_json_pretty` and `output_hash` can differ even though normalization is intended to make output order-independent.

**Fix:** Add a full serialized-field tie-breaker, or include every serialized field in the item sort keys. Keep intentionally ignored runtime durations out of the hash path after normalization.

```rust
case.expected.sort_by(|left, right| {
    expected_item_key(left)
        .cmp(&expected_item_key(right))
        .then_with(|| canonical_expected_item_key(left).cmp(&canonical_expected_item_key(right)))
});
case.observed.sort_by(|left, right| {
    observed_item_key(left)
        .cmp(&observed_item_key(right))
        .then_with(|| canonical_observed_item_key(left).cmp(&canonical_observed_item_key(right)))
});
```

Add a regression test that reverses two rows with the same current key but different `provenance`, `precision`, or `false_positive_trap`, then asserts normalized JSON and hash equality.

---

_Reviewed: 2026-05-17T18:04:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

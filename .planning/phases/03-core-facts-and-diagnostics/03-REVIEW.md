---
phase: 03-core-facts-and-diagnostics
reviewed: 2026-04-28T11:59:05Z
depth: standard
files_reviewed: 7
files_reviewed_list:
  - crates/polint-core/Cargo.toml
  - crates/polint-core/src/lib.rs
  - crates/polint-diagnostics/Cargo.toml
  - crates/polint-diagnostics/src/lib.rs
  - crates/polint-fs/Cargo.toml
  - crates/polint-fs/src/lib.rs
  - crates/polint-cli/tests/cli.rs
findings:
  critical: 0
  warning: 3
  info: 0
  total: 3
status: issues_found
---

# Phase 03: Code Review Report

**Reviewed:** 2026-04-28T11:59:05Z
**Depth:** standard
**Files Reviewed:** 7
**Status:** issues_found

## Summary

Reviewed Phase 03 core facts, diagnostics, filesystem discovery, and CLI determinism test changes. The implementation is mostly coherent and workspace verification passes, but three reliability/API risks should be addressed before treating the Phase 3 contracts as stable.

Verification run during review:

- `cargo test --workspace` - passed
- `cargo clippy --workspace --all-targets -- -D warnings` - passed

## Warnings

### WR-01: Rule metadata panics bypass runner containment

**File:** `crates/polint-core/src/lib.rs:547`

**Issue:** `run_rules` calls `rule.meta()` before entering `catch_unwind`. A custom rule that panics while producing metadata will crash the whole run instead of becoming a controlled internal diagnostic. That leaves a gap in the Phase 3 contract that rule panics are contained.

**Fix:**

```rust
let meta = match catch_unwind(AssertUnwindSafe(|| rule.meta())) {
    Ok(meta) => meta,
    Err(_) => {
        return vec![internal_rule_error_for_id(
            db,
            "unknown",
            "rule metadata panicked".to_string(),
        )];
    }
};
```

Then refactor `internal_rule_error` to accept a fallback rule id, and add a test rule whose `meta()` panics.

### WR-02: Fingerprint dedupe only removes adjacent duplicates

**File:** `crates/polint-diagnostics/src/lib.rs:209-212`

**Issue:** `dedupe_diagnostics` sorts by file, location, rule, message, and then fingerprint, but `dedup_by` only compares adjacent elements. If two diagnostics intentionally share a `stable_fingerprint` through `with_fingerprint` but differ in earlier sort fields, another diagnostic can sort between them and both duplicates survive. This violates the fingerprint dedupe contract and can make output noisier than expected.

**Fix:**

```rust
pub fn dedupe_diagnostics(mut diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    use std::collections::BTreeSet;

    sort_diagnostics(&mut diagnostics);
    let mut seen = BTreeSet::new();
    diagnostics
        .into_iter()
        .filter(|diagnostic| seen.insert(diagnostic.stable_fingerprint.clone()))
        .collect()
}
```

Add a regression test where two diagnostics with the same explicit fingerprint are separated by a third diagnostic after sorting.

### WR-03: Public `Diagnostic` shape is still brittle for downstream rule crates

**File:** `crates/polint-diagnostics/src/lib.rs:70`

**Issue:** `Diagnostic` exposes all fields publicly and derives `Deserialize` without defaults. Adding the Phase 3 fields is not source-compatible for downstream crates using struct literals, and older serialized diagnostics that lack the new fields will not deserialize. This is a public API compatibility risk for SDK users and future cache/CI consumers.

**Fix:** Choose the compatibility stance explicitly before release. If constructor-based use is the intended contract, mark `Diagnostic` and nested diagnostic data structs `#[non_exhaustive]` and document the builder API. If old serialized diagnostics must remain readable, add serde defaults for new optional/vector fields and a custom deserialization path or normalization step for missing `stable_fingerprint`.

---

_Reviewed: 2026-04-28T11:59:05Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

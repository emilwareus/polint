---
phase: 03-core-facts-and-diagnostics
fixed: 2026-04-28T12:08:07Z
source_review: 03-REVIEW.md
status: fixed
findings_fixed: 3
---

# Phase 03: Code Review Fix Report

## Fixed Findings

### WR-01: Rule metadata panics bypass runner containment

Fixed in `crates/polint-core/src/lib.rs`.

- Wrapped `Rule::meta()` in `catch_unwind`.
- Added `internal_rule_error_for_id` so metadata panics can report through `internal/unknown`.
- Added `run_rules_contains_meta_panics`.

### WR-02: Fingerprint dedupe only removes adjacent duplicates

Fixed in `crates/polint-diagnostics/src/lib.rs`.

- Changed `dedupe_diagnostics` to keep a global `BTreeSet` of seen fingerprints after deterministic sorting.
- Added `dedupe_diagnostics_removes_non_adjacent_duplicate_fingerprints`.

### WR-03: Public `Diagnostic` shape is still brittle for downstream rule crates

Fixed in `crates/polint-diagnostics/src/lib.rs`.

- Marked `Diagnostic` and nested diagnostic data structs as `#[non_exhaustive]`.
- Documented the constructor and fluent-helper API as the intended construction path.
- Replaced derived `Deserialize` for `Diagnostic` with a compatibility shim that defaults additive Phase 3 fields.
- Recomputed `stable_fingerprint` when deserializing older diagnostics that do not include it.
- Added `diagnostic_deserializes_missing_phase3_fields_with_computed_fingerprint`.

## Verification

- `cargo fmt -- --check`
- `cargo test -p polint-core --lib run_rules_contains_meta_panics`
- `cargo test -p polint-core --lib run_rules_contains_rule_errors_and_panics`
- `cargo test -p polint-core --lib`
- `cargo test -p polint-diagnostics --lib dedupe_diagnostics_removes_non_adjacent_duplicate_fingerprints`
- `cargo test -p polint-diagnostics --lib diagnostic_deserializes_missing_phase3_fields_with_computed_fingerprint`
- `cargo test -p polint-diagnostics --lib`

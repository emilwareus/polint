---
phase: 30-direct-call-facts
reviewed: 2026-05-21T09:58:57Z
depth: standard
files_reviewed: 33
files_reviewed_list:
  - crates/polint/src/analysis/calls/cache_key.rs
  - crates/polint/src/analysis/calls/direct.rs
  - crates/polint/src/analysis/calls/extract.rs
  - crates/polint/src/analysis/calls/facts.rs
  - crates/polint/src/analysis/calls/mod.rs
  - crates/polint/src/analysis/calls/provider.rs
  - crates/polint/src/analysis/calls/store.rs
  - crates/polint/src/analysis/calls/unresolved.rs
  - crates/polint/src/analysis/calls/validate.rs
  - crates/polint/src/analysis/ids.rs
  - crates/polint/src/analysis/mod.rs
  - crates/polint/src/analysis_kernel/debug.rs
  - crates/polint/src/analysis_kernel/incremental/keys.rs
  - crates/polint/src/analysis_kernel/metadata.rs
  - crates/polint/src/analysis_kernel/mod.rs
  - crates/polint/src/analysis_kernel/provider.rs
  - crates/polint/src/analysis_kernel/validation.rs
  - crates/polint/src/analysis_plan.rs
  - crates/polint/src/core/mod.rs
  - crates/polint/src/eval/fixtures.rs
  - crates/polint/src/eval/mod.rs
  - crates/polint/src/eval/model.rs
  - crates/polint/src/eval/observed.rs
  - crates/polint/tests/cli.rs
  - tests/eval-fixtures/direct-calls/core/expected.polint-eval.toml
  - tests/eval-fixtures/direct-calls/core/repo/.polint.toml
  - tests/eval-fixtures/direct-calls/core/repo/go.mod
  - tests/eval-fixtures/direct-calls/core/repo/service.go
  - tests/eval-fixtures/direct-calls/core/repo/web/package.json
  - tests/eval-fixtures/direct-calls/core/repo/web/src/app.ts
  - tests/eval-fixtures/direct-calls/core/repo/web/src/helper.ts
  - tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml
findings:
  critical: 0
  warning: 4
  info: 0
  total: 4
status: issues_found
---

# Phase 30: Code Review Report

**Reviewed:** 2026-05-21T09:58:57Z
**Depth:** standard
**Files Reviewed:** 33
**Status:** issues_found

## Summary

Reviewed the direct-call fact provider, storage/metadata paths, validation, eval observation, CLI privacy tests, and direct-call fixtures. No security issue or public direct-call API leak was found. The main risks are call-resolution classification errors and incomplete provider/cache identity for direct-call outputs.

## Warnings

### WR-01: Constructor calls are recorded as direct references

**File:** `crates/polint/src/analysis/calls/direct.rs:44`
**Issue:** `resolve_direct_call_targets` only distinguishes import bindings and static/member calls before falling back to `CallAlgorithm::DirectReference`. A resolved constructor call (`CallSyntaxKind::Constructor` or `New`) therefore produces a constructor edge kind but the wrong algorithm. This loses the intended `ConstructorBinding` taxonomy already present in `CallAlgorithm` and the provider parameter digest.
**Fix:**
```rust
let algorithm = if index.is_import_binding(site, reference) {
    CallAlgorithm::ImportBinding
} else if matches!(site.kind, CallSyntaxKind::Constructor | CallSyntaxKind::New) {
    CallAlgorithm::ConstructorBinding
} else if matches!(site.kind, CallSyntaxKind::StaticMember) {
    CallAlgorithm::StaticMember
} else if matches!(site.kind, CallSyntaxKind::Member) {
    CallAlgorithm::DirectMember
} else {
    CallAlgorithm::DirectReference
};
```
Add unit coverage asserting constructor calls emit `ConstructorBinding`, and update the direct-call eval fixture to assert that algorithm specifically.

### WR-02: Resolved instance member calls are mislabeled as static members

**File:** `crates/polint/src/analysis/calls/direct.rs:46`
**Issue:** The same branch treats both `CallSyntaxKind::StaticMember` and `CallSyntaxKind::Member` as `CallAlgorithm::StaticMember`. If semantic references resolve an instance method/property call, the provider will report a static-member algorithm for a non-static call. This makes downstream algorithm counts and stable target keys misleading.
**Fix:** Split `StaticMember` and `Member`, using `CallAlgorithm::DirectMember` for resolved non-static member calls. Add a test with a precise reference on a `CallSyntaxKind::Member` site and assert `DirectMember` plus `CallEdgeKind::MethodDirect`.

### WR-03: Calls manifest omits fact families the provider actually reads

**File:** `crates/polint/src/analysis_kernel/provider.rs:362`
**Issue:** The `polint.calls` manifest inputs do not list `semantic_imports` or `unsupported_semantics`, but the provider reads both: `semantic_imports()` in direct import classification and `unsupported_semantics()` in direct blocking/unresolved derivation. The layer digest currently includes upstream provider output digests, so cache invalidation is mostly protected, but the manifest is an inaccurate dependency contract and provider-order/eval metadata can miss real inputs.
**Fix:** Add the missing inputs to the calls manifest and update the provider-order fixture/tests:
```rust
inputs: &[
    "source_files",
    "functions",
    "symbols",
    "references",
    "semantic_imports",
    "unsupported_semantics",
    "resolved_imports",
    "import_to_package_edges",
    "mir_bodies",
    "mir_operations",
    "places",
    "cfg_functions",
    "cfg_edges",
],
```

### WR-04: Calls output digest ignores target function identity

**File:** `crates/polint/src/analysis/calls/provider.rs:138`
**Issue:** `calls_output_digest` records only whether `target_function` and `target_symbol` are present, not their stable identities. `target.stable_key` includes the target symbol for native direct targets, but the output row also exposes `target_function` and indexes incoming calls by function. A change from one target function to another can be invisible in the calls output digest if the target stable key and upstream digest inputs do not change accordingly.
**Fix:** Build stable-key maps for functions and symbols and include those stable keys rather than presence labels:
```rust
let function_keys = function_key_map(db);
let symbol_keys = symbol_key_map(db);

format!(
    "call_target={} site={} ... target_function={} target_symbol={}",
    target.stable_key,
    stable_site_key(&site_keys, target.site),
    stable_function_key(&function_keys, target.target_function),
    stable_symbol_key(&symbol_keys, target.target_symbol),
)
```
Then add a digest regression where two outputs differ only by target function stable key and assert the digest changes, while dense ID-only changes remain stable.

---

_Reviewed: 2026-05-21T09:58:57Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

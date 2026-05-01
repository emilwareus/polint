---
phase: 06-sdk-and-example-rules
reviewed: 2026-05-01T06:13:14Z
depth: standard
files_reviewed: 15
files_reviewed_list:
  - Cargo.lock
  - crates/polint-cli/src/main.rs
  - crates/polint-cli/tests/cli.rs
  - crates/polint-config/src/lib.rs
  - crates/polint-core/src/lib.rs
  - crates/polint-go/src/lib.rs
  - crates/polint-rules/Cargo.toml
  - crates/polint-rules/src/lib.rs
  - crates/polint-rules/tests/snapshots.rs
  - crates/polint-sdk/src/lib.rs
  - crates/polint-ts/src/lib.rs
  - tests/fixtures/go/clean/payment_test.go
  - tests/fixtures/go/failing/payment.go
  - tests/fixtures/go/failing/payment_test.go
  - tests/fixtures/ts/failing/component.tsx
findings:
  critical: 0
  warning: 2
  info: 0
  total: 2
status: issues_found
---

# Phase 6: Code Review Report

**Reviewed:** 2026-05-01T06:13:14Z
**Depth:** standard
**Files Reviewed:** 15
**Status:** issues_found

## Summary

Reviewed the Phase 6 SDK, parser fact, built-in rule, CLI fixture, and snapshot changes. The public SDK additions and panic containment approach look consistent with the project constraints, and no critical security issues were found.

Two rule behavior issues need fixes before treating the example rules as hardened: JSX raw-color findings can be duplicated for one source literal, and configured file filters are ignored by some Go example rules.

## Warnings

### WR-01: JSX raw-color attributes can be reported twice

**File:** `crates/polint-rules/src/lib.rs:462`
**Issue:** `TsNoRawColors` dedupes raw-color findings by exact `(file, start_byte, end_byte, value)`. The TS parser records a quoted JSX attribute twice: once as a `JsxAttributeFact` spanning the whole attribute at `crates/polint-ts/src/lib.rs:2597`, and once as a `StringLiteralFact` spanning only the quoted literal at `crates/polint-ts/src/lib.rs:2951`. Because the ranges differ, `<button data-color="#00ff00" />` emits two diagnostics for the same literal. I reproduced this against `tests/fixtures/ts/failing/component.tsx`: line 41 produced both a `jsx-attribute` and `string-literal` diagnostic for `#00ff00`.
**Fix:** Deduplicate same-file, same-value findings whose byte ranges overlap, or suppress the JSX attribute diagnostic when an equivalent string literal fact is already inside the attribute span. Add a parser-backed test or CLI assertion for a real JSX attribute, not only the synthetic same-span case.

```rust
fn overlaps(a_start: u32, a_end: u32, b_start: u32, b_end: u32) -> bool {
    a_start < b_end && b_start < a_end
}

// Store seen ranges, then skip same file/value if spans overlap.
if seen.iter().any(|seen| {
    seen.file == finding.file
        && seen.value == finding.value
        && overlaps(seen.start, seen.end, finding.span.start_byte, finding.span.end_byte)
}) {
    return;
}
```

### WR-02: Some Go rules ignore configured file filters

**File:** `crates/polint-rules/src/lib.rs:301`
**Issue:** `examples/go-test-suite-size` iterates every Go test without applying `RuleOptions.files` or `allow_files`; `examples/go-assertion-after-action` has the same issue at line 346. `examples/go-import-boundaries` also relies only on `forbidden_imports` source globs at line 147 and ignores the rule-level `files` filter copied from config. This causes false positives when users scope these rules to a subset of files. I reproduced the issue with `files = ["no-match/**"]`: both Go test rules still reported diagnostics for `payment_test.go`.
**Fix:** Apply the shared file selection logic before emitting diagnostics for these rules, and add negative tests proving non-matching `files` patterns suppress diagnostics.

```rust
let file = ctx.file_path(test.file);
if !file_selected(ctx.options(), &file) || file_allowed(ctx.options(), &file) {
    continue;
}
```

---

_Reviewed: 2026-05-01T06:13:14Z_
_Reviewer: Codex (gsd-code-reviewer)_
_Depth: standard_

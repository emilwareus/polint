---
phase: 13-symbols-and-references
reviewed: 2026-05-13T07:56:23Z
depth: standard
files_reviewed: 29
files_reviewed_list:
  - crates/polint-macros/src/lib.rs
  - crates/polint/src/analysis_plan.rs
  - crates/polint/src/cache/keys.rs
  - crates/polint/src/cli/mod.rs
  - crates/polint/src/cli/skill.rs
  - crates/polint/src/core/mod.rs
  - crates/polint/src/diagnostics/mod.rs
  - crates/polint/src/lib.rs
  - crates/polint/src/module_graph/mod.rs
  - crates/polint/src/runner/mod.rs
  - crates/polint/src/sdk/facts.rs
  - crates/polint/src/sdk/mod.rs
  - crates/polint/src/symbol_graph/go.rs
  - crates/polint/src/symbol_graph/mod.rs
  - crates/polint/src/symbol_graph/model.rs
  - crates/polint/src/symbol_graph/query.rs
  - crates/polint/src/symbol_graph/stable_id.rs
  - crates/polint/src/symbol_graph/ts.rs
  - crates/polint/tests/cli.rs
  - docs/facts/README.md
  - docs/facts/capability-plans.md
  - docs/facts/symbols-and-references.md
  - go.work
  - go.work.sum
  - tools/polint-go-symbols/go.mod
  - tools/polint-go-symbols/go.sum
  - tools/polint-go-symbols/internal/symbols/emit.go
  - tools/polint-go-symbols/internal/symbols/emit_test.go
  - tools/polint-go-symbols/main.go
findings:
  critical: 0
  warning: 3
  info: 0
  total: 3
status: issues_found
---

# Phase 13: Code Review Report

**Reviewed:** 2026-05-13T07:56:23Z
**Depth:** standard
**Files Reviewed:** 29
**Status:** issues_found

## Summary

Reviewed the listed Rust symbol/reference implementation, SDK and macro surfaces, CLI skill text, documentation, tests, and Go sidecar files at standard depth. The main concerns are correctness and truthfulness issues in the newly public symbol/reference contract: IDs can depend on transient file order, and Go reference facts can overstate precision or advertised write coverage.

Tests were not run for this review; findings are from source inspection.

## Warnings

### WR-01: Stable IDs include transient FileId values

**File:** `crates/polint/src/symbol_graph/stable_id.rs:266`
**Issue:** `span_part` serializes `span.file.0` into the stable key material. `FileId` is assigned from the current `AnalysisDb` insertion index (`crates/polint/src/core/mod.rs:593`), so adding, removing, or reordering any discovered file can change `SymbolId`, `DefinitionId`, and `ReferenceId` values for unchanged source. That conflicts with the public documentation that says these IDs are derived from normalized semantic key material, not vector positions.
**Fix:**
```rust
fn span_part(span: &Span) -> String {
    format!(
        "{}-{}:{}:{}-{}:{}",
        span.start_byte,
        span.end_byte,
        span.start_line,
        span.start_col,
        span.end_line,
        span.end_col
    )
}
```
Use the existing `file_key` fields for file identity, and add a regression that compares IDs for the same repo-relative file after inserting an unrelated file earlier in discovery order.

### WR-02: Unknown Go sidecar precision is upgraded to ExactSemantic

**File:** `crates/polint/src/symbol_graph/go.rs:663`
**Issue:** `reference_precision` maps any unrecognized precision string to `SymbolPrecision::ExactSemantic`. Because this value crosses the Go sidecar JSON boundary, a sidecar typo or schema drift would silently overclaim exact semantic precision instead of exposing unsupported or invalid data.
**Fix:**
```rust
fn reference_precision(precision: &str) -> SymbolPrecision {
    match precision {
        "exact_semantic" => SymbolPrecision::ExactSemantic,
        "exact_local" => SymbolPrecision::ExactLocal,
        "module_linked" => SymbolPrecision::ModuleLinked,
        "heuristic" => SymbolPrecision::Heuristic,
        "setup_missing" => SymbolPrecision::SetupMissing,
        "unsupported" => SymbolPrecision::Unsupported,
        _ => SymbolPrecision::Unsupported,
    }
}
```
If unknown values should fail closed, make the conversion return a `Result` and emit a `polint/capability` diagnostic instead.

### WR-03: Go write references are advertised but never emitted

**File:** `tools/polint-go-symbols/internal/symbols/emit.go:577`
**Issue:** `referenceKind` returns `read` for every non-package, non-type, non-call identifier, and `selectionReferenceKind` only returns `call`, `field`, `method`, or `member`. Assignment targets and increment/decrement expressions are therefore not classified as writes or read-writes, even though `docs/facts/symbols-and-references.md:298` advertises Go reads and writes. Rules looking for `ReferenceKind::Write` will silently miss Go writes.
**Fix:**
```go
func (e *emitter) referenceKind(ident *ast.Ident, obj types.Object) string {
	if _, ok := obj.(*types.PkgName); ok {
		return "package"
	}
	if _, ok := obj.(*types.TypeName); ok {
		return "type"
	}
	if e.isCallIdentifier(ident) {
		return "call"
	}
	if e.isAssignedIdentifier(ident) {
		return "write"
	}
	return "read"
}
```
Add LHS detection for identifiers and selectors, cover `AssignStmt` and `IncDecStmt`, and add a sidecar test for assignment/reference kinds. If write support is intentionally deferred, remove the Go write claim from the public docs until it is implemented.

---

_Reviewed: 2026-05-13T07:56:23Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

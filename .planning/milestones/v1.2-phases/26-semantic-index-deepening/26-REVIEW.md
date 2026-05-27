---
phase: 26-semantic-index-deepening
reviewed: 2026-05-19T08:43:29Z
depth: standard
files_reviewed: 29
files_reviewed_list:
  - crates/polint/go-sidecar/polint-go-symbols/internal/symbols/emit.go
  - crates/polint/src/analysis_kernel/debug.rs
  - crates/polint/src/analysis_kernel/incremental/keys.rs
  - crates/polint/src/analysis_kernel/incremental/mod.rs
  - crates/polint/src/analysis_kernel/metadata.rs
  - crates/polint/src/analysis_kernel/provider.rs
  - crates/polint/src/analysis_kernel/validation.rs
  - crates/polint/src/core/mod.rs
  - crates/polint/src/eval/fixtures.rs
  - crates/polint/src/eval/matcher.rs
  - crates/polint/src/eval/metrics.rs
  - crates/polint/src/eval/model.rs
  - crates/polint/src/eval/observed.rs
  - crates/polint/src/eval/report.rs
  - crates/polint/src/symbol_graph/go.rs
  - crates/polint/src/symbol_graph/mod.rs
  - crates/polint/src/symbol_graph/model.rs
  - crates/polint/src/symbol_graph/semantic.rs
  - crates/polint/src/symbol_graph/ts.rs
  - crates/polint/tests/cli.rs
  - docs/facts/symbols-and-references.md
  - tests/eval-fixtures/semantic-index/core/expected.polint-eval.toml
  - tests/eval-fixtures/semantic-index/core/repo/.polint.toml
  - tests/eval-fixtures/semantic-index/core/repo/go.mod
  - tests/eval-fixtures/semantic-index/core/repo/service.go
  - tests/eval-fixtures/semantic-index/core/repo/src/app.ts
  - tests/eval-fixtures/semantic-index/core/repo/src/lib.ts
  - tools/polint-go-symbols/internal/symbols/emit.go
  - tools/polint-go-symbols/internal/symbols/emit_test.go
findings:
  critical: 0
  warning: 2
  info: 0
  total: 2
status: issues_found
---

# Phase 26: Code Review Report

**Reviewed:** 2026-05-19T08:43:29Z
**Depth:** standard
**Files Reviewed:** 29
**Status:** issues_found

## Summary

Reviewed the semantic-index, symbol/reference, eval fixture, docs, and Go sidecar changes at standard depth. No critical security issues were found. Two correctness issues need attention before the new symbol/reference facts can be treated as stable: TypeScript default exports expressed as identifiers are not connected to public symbol/reference resolution, and Go scope stable keys depend on `token.Pos` values that can shift when unrelated files change.

## Warnings

### WR-01: Default export identifiers are not connected to module-linked reference resolution

**File:** `crates/polint/src/symbol_graph/ts.rs:1774`
**Issue:** `collect_export_names` only records `"default"` when `default_export_symbol` returns a declaration symbol, and `default_export_symbol` returns `None` for expression default exports. That misses common valid forms such as `export default defaultHandler;`, which is now present in `tests/eval-fixtures/semantic-index/core/repo/src/lib.ts:4`. The semantic export row is still emitted for `"default"` at `crates/polint/src/symbol_graph/ts.rs:387`, but `import_alias_candidates` resolves default imports through `exports.get("default")` at `crates/polint/src/symbol_graph/ts.rs:1344`. The result is a valid default import being reported as unresolved or missing from `References<'_>` even though the module has a default export.
**Fix:**
```rust
fn default_export_symbol(
    declaration: &ExportDefaultDeclarationKind<'_>,
    scoping: &Scoping,
) -> Option<OxcSymbolId> {
    match declaration {
        ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
            function.id.as_ref()?.symbol_id.get()
        }
        ExportDefaultDeclarationKind::ClassDeclaration(class) => class.id.as_ref()?.symbol_id.get(),
        ExportDefaultDeclarationKind::TSInterfaceDeclaration(interface) => {
            interface.id.symbol_id.get()
        }
        ExportDefaultDeclarationKind::Expression(expression) => {
            expression_identifier_reference(expression)
                .and_then(|reference| reference.reference_id.get())
                .and_then(|reference| scoping.get_reference(reference).symbol_id())
        }
        _ => None,
    }
}
```
Add a module-linked fixture for `const defaultHandler = () => handler; export default defaultHandler;` plus `import defaultHandler from "./lib";` and assert the default import reference resolves to the exported symbol. For anonymous default expressions, either create a generated default-export symbol or explicitly document/report the precision limit instead of emitting an apparently resolved export with no resolvable symbol.

### WR-02: Go semantic scope stable keys use `token.Pos` absolute offsets

**File:** `tools/polint-go-symbols/internal/symbols/emit.go:1351`
**Issue:** `scopeKey` includes `fmt.Sprintf("pos:%d", pos)`, and most callers pass raw AST `token.Pos` values such as `n.Pos()` at `tools/polint-go-symbols/internal/symbols/emit.go:577`. `token.Pos` is an absolute position in the package `FileSet`, so positions for an unchanged file can change when another file is parsed before it or when an earlier file's size changes. These keys are emitted as semantic scope stable keys, so adding an unrelated Go file can churn scope identities, metadata, generated semantic rows, and cache/eval comparisons for unchanged scopes. The embedded sidecar copy has the same code at `crates/polint/go-sidecar/polint-go-symbols/internal/symbols/emit.go:1351`.
**Fix:**
```go
func scopeKey(packagePath string, file string, kind string, startByte int, name string) string {
	return strings.Join([]string{
		"go:scope",
		"package:" + packagePath,
		"file:" + file,
		"kind:" + kind,
		"name:" + name,
		fmt.Sprintf("start:%d", startByte),
	}, "|")
}
```
Convert each `token.Pos` through the owning `*token.File` to a file-relative byte offset before building the key, and keep `tools/polint-go-symbols/...` and `crates/polint/go-sidecar/...` in sync. Add a sidecar test that records scope keys for a file, adds an unrelated earlier-sorted Go file, and asserts the original file's scope keys are unchanged.

---

_Reviewed: 2026-05-19T08:43:29Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

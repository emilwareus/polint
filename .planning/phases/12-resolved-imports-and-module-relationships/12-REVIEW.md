---
phase: 12-resolved-imports-and-module-relationships
reviewed: 2026-05-11T17:04:02Z
depth: standard
files_reviewed: 23
files_reviewed_list:
  - crates/polint-macros/src/lib.rs
  - crates/polint/src/analysis_plan.rs
  - crates/polint/src/cli/mod.rs
  - crates/polint/src/cli/skill.rs
  - crates/polint/src/core/mod.rs
  - crates/polint/src/lib.rs
  - crates/polint/src/module_graph/go.rs
  - crates/polint/src/module_graph/mod.rs
  - crates/polint/src/module_graph/model.rs
  - crates/polint/src/module_graph/paths.rs
  - crates/polint/src/module_graph/query.rs
  - crates/polint/src/module_graph/ts.rs
  - crates/polint/src/runner/mod.rs
  - crates/polint/src/sdk/facts.rs
  - crates/polint/src/sdk/mod.rs
  - crates/polint/src/ts/adapter.rs
  - crates/polint/src/ts/mod.rs
  - crates/polint/src/ts/tests.rs
  - crates/polint/tests/cli.rs
  - docs/facts/README.md
  - docs/facts/capability-plans.md
  - docs/facts/imports.md
  - docs/facts/resolved-imports.md
findings:
  critical: 0
  warning: 3
  info: 0
  total: 3
status: issues_found
---

# Phase 12: Code Review Report

**Reviewed:** 2026-05-11T17:04:02Z
**Depth:** standard
**Files Reviewed:** 23
**Status:** issues_found

## Summary

Reviewed the macro capability derivation, analysis plan, runner capability blocking, module graph providers, Go and TS/JS resolution paths, public SDK fact views, generated skill text, CLI integration tests, and public fact docs. The SDK visibility shape mostly follows the rule-authoring contract, and resolver commands use fixed argv rather than shell interpolation. I found three correctness/documentation issues that should be fixed before treating the phase as clean.

## Warnings

### WR-01: Dotless Go module paths can turn missing local imports into external dependencies

**File:** `crates/polint/src/module_graph/go.rs:175`

**Issue:** `import_is_external_dependency` checks `is_go_stdlib_import_path` before checking whether the import is inside the current module path. For a valid dotless module path such as `mycorp/app`, a missing local import like `mycorp/app/internal/missing` has first segment `mycorp`, so line 176 classifies it as stdlib/external. `resolve_go_import` then returns `External` at line 413 instead of `Unresolved(NotFound)`, hiding broken local imports from architecture rules.

**Fix:**

```rust
fn import_is_external_dependency(&self, import_path: &str) -> bool {
    if let Some(module_path) = self.module_path.as_deref() {
        return !import_is_within_module(import_path, module_path);
    }
    is_go_stdlib_import_path(import_path)
}
```

Add a regression test with `module mycorp/app` and an import of `mycorp/app/internal/missing`.

### WR-02: Fallback package nodes merge unrelated same-name packages

**File:** `crates/polint/src/module_graph/model.rs:473`

**Issue:** `package_label` keys fallback package nodes only by language and package name, and `ensure_package_node_with_label` dedupes by that label. When Go metadata is unavailable and `derive_requested_module_graph` falls back to `builder.ensure_package_node(package)` at `crates/polint/src/module_graph/mod.rs:89`, two directories with the same package name, such as `cmd/api/main.go` and `cmd/worker/main.go`, collapse into one package node. That produces incorrect containment and dependency edges.

**Fix:** Include the package file directory, or another stable package scope key, in the fallback package-node key/label while preserving Go import-path labels when `go list` metadata is available.

```rust
fn fallback_package_label(language: Language, relative_path: &str, name: &str) -> String {
    let dir = std::path::Path::new(relative_path)
        .parent()
        .and_then(|path| path.to_str())
        .unwrap_or(".");
    format!("{}:{dir}:{name}", language_label(language))
}
```

Add a module graph test with two same-name Go packages in different directories and assert that they produce distinct package nodes.

### WR-03: Docs say setup-missing relationship facts are inspectable by rules that are actually blocked

**File:** `docs/facts/resolved-imports.md:111`

**Issue:** The docs say setup-missing imports remain visible as facts, and the generated skill text says to treat `SetupMissing` as meaningful rule data (`crates/polint/src/cli/skill.rs:338`). In the implementation, any rule whose requested capability has non-supported support is skipped before execution (`crates/polint/src/core/mod.rs:1247` and `crates/polint/src/core/mod.rs:1280`). Rule authors therefore cannot inspect `SetupMissing` relationship facts; they receive `polint/capability` diagnostics instead.

**Fix:** Change the docs and generated skill text to say that `Unresolved`, `Dynamic`, and `Unsupported` statuses are inspectable by running relationship rules, while `SetupMissing` is surfaced through capability diagnostics and blocks execution.

---

_Reviewed: 2026-05-11T17:04:02Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

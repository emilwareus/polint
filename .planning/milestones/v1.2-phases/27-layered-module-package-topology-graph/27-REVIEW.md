---
phase: 27-layered-module-package-topology-graph
reviewed: "2026-05-19T13:53:37Z"
depth: standard
files_reviewed: 41
files_reviewed_list:
  - crates/polint/src/analysis_kernel/incremental/keys.rs
  - crates/polint/src/analysis_kernel/incremental/mod.rs
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
  - crates/polint/src/module_graph/formats/go_mod.rs
  - crates/polint/src/module_graph/formats/go_work.rs
  - crates/polint/src/module_graph/formats/mod.rs
  - crates/polint/src/module_graph/formats/package_json.rs
  - crates/polint/src/module_graph/formats/package_lock.rs
  - crates/polint/src/module_graph/go.rs
  - crates/polint/src/module_graph/mod.rs
  - crates/polint/src/module_graph/model.rs
  - crates/polint/src/module_graph/topology.rs
  - crates/polint/src/module_graph/ts.rs
  - crates/polint/tests/cli.rs
  - docs/CAPABILITY-FULFILLMENT-RESEARCH.md
  - docs/facts/capability-plans.md
  - docs/facts/imports.md
  - docs/roadmap/00_ROADMAP.md
  - tests/eval-fixtures/cache/layer-cache/expected.polint-eval.toml
  - tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml
  - tests/eval-fixtures/module-topology/core/expected.polint-eval.toml
  - tests/eval-fixtures/module-topology/core/repo/.polint.toml
  - tests/eval-fixtures/module-topology/core/repo/services/api/go.mod
  - tests/eval-fixtures/module-topology/core/repo/services/api/go.sum
  - tests/eval-fixtures/module-topology/core/repo/services/api/main.go
  - tests/eval-fixtures/module-topology/core/repo/web/generated/client.generated.ts
  - tests/eval-fixtures/module-topology/core/repo/web/package.json
  - tests/eval-fixtures/module-topology/core/repo/web/src/app.test.ts
  - tests/eval-fixtures/module-topology/core/repo/web/src/app.ts
findings:
  critical: 0
  warning: 3
  info: 0
  total: 3
status: issues_found
---

# Phase 27: Code Review Report

**Reviewed:** 2026-05-19T13:53:37Z
**Depth:** standard
**Files Reviewed:** 41
**Status:** issues_found

## Summary

Reviewed the layered module/package topology implementation, eval fixtures, docs, and CLI test updates. I found three correctness issues: Go import-to-package rows cannot reliably use Go module requirements, malformed or unsupported package-lock files can silently suppress dependency evidence, and an empty-import topology cache path can leave stale facts in a reused database.

## Warnings

### WR-01: Go import-to-package classification is disconnected from Go module requirements

**File:** `crates/polint/src/module_graph/go.rs:324`

**Issue:** Go topology creates a module package row for the `go.mod` module at lines 324-349 and attaches `require` rows to that package at lines 467-494. It then creates local Go package rows at lines 353-375 and overwrites `package_ids_by_import_path`, so source sets for files use the local package id at lines 393-397. Later, import-to-package classification only treats an external import as declared when the requirement's `from_package` equals the source set package id in `crates/polint/src/module_graph/mod.rs:831-840`. For a normal Go file in module `github.com/acme/api` importing required module `github.com/acme/lib`, the source package is the local package row while the requirement belongs to the module row, so the import is classified as `Undeclared` even though `go.mod` declares it. Local Go package imports have a similar gap because Go topology rows use `module_node: None` at lines 329 and 364 while `package_candidates_for_node` only matches `TopologyPackageFact.module_node` in `crates/polint/src/module_graph/mod.rs:759-763`.

**Fix:** Link Go topology package rows to the module graph package nodes created by `seed_go_module_nodes`, or make `declared_requirement_exists` resolve the owning Go module package before checking requirements. Add a temp-repo/eval fixture with a Go file importing both a declared external module and another local package, and assert the resulting `ImportToPackage` statuses are `external` and `resolved` respectively.

### WR-02: Unsupported or malformed package-lock files suppress lockfile evidence

**File:** `crates/polint/src/module_graph/formats/package_lock.rs:40`

**Issue:** `parse_package_lock` records malformed JSON in `manifest.unsupported` at lines 40-45, and it labels `lockfileVersion = 1` as `package-lock-v1` at lines 70-75 while only reading the v2/v3 `packages` map at lines 91-95. `emit_package_lock_edges` in `crates/polint/src/module_graph/ts.rs:878-918` ignores `manifest.unsupported`, so malformed lockfiles produce no unsupported row. `emit_missing_lockfile_edges` then returns early for any present JS lockfile at lines 957-964 and `has_any_js_lockfile` includes `package-lock.json` at lines 1004-1018. The result is that malformed package-lock files, and v1 lockfiles without a `packages` map, can suppress both exact resolved edges and `MissingLockfile` evidence.

**Fix:** Either parse npm lockfile v1's dependency tree or mark v1 as unsupported until it is implemented. Also emit unsupported `ResolvedDependencyEdgeFact` rows for `manifest.unsupported`, and only suppress `MissingLockfile` when a supported lockfile produced usable resolution evidence. Add fixtures for malformed `package-lock.json` and lockfileVersion 1.

### WR-03: Empty-import topology cache path does not clear stale import-to-package facts

**File:** `crates/polint/src/module_graph/mod.rs:599`

**Issue:** `derive_module_topology_with_cache_stats` returns early when `db.imports().is_empty()` at lines 599-611, but unlike the uncached path at lines 696-709 it never calls `db.replace_import_to_package_facts(Vec::new())`. In the normal fresh-run path this starts empty, but any reused `AnalysisDb` that previously had import-to-package rows will keep stale topology facts while the provider reports an empty output digest.

**Fix:**
```rust
if db.imports().is_empty() {
    db.replace_import_to_package_facts(Vec::new());
    return ModuleTopologyDerivation {
        output_digest: Some(module_topology_output_digest_for_payload(
            &ModuleTopologyLayerPayload {
                schema: MODULE_TOPOLOGY_LAYER_SCHEMA.to_string(),
                diagnostics: Vec::new(),
                capability_support: Vec::new(),
                import_to_package_edges: Vec::new(),
            },
            None,
        )),
        ..ModuleTopologyDerivation::default()
    };
}
```

Add a regression test that seeds an `AnalysisDb` with one import-to-package row, clears imports, runs `derive_module_topology_with_cache_stats`, and asserts `db.import_to_package_edges().is_empty()`.

---

_Reviewed: 2026-05-19T13:53:37Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

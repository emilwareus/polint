---
phase: 21-provenance-precision-and-validation-metadata
reviewed: 2026-05-17T08:10:11Z
depth: standard
files_reviewed: 9
files_reviewed_list:
  - crates/polint/src/analysis_kernel/debug.rs
  - crates/polint/src/analysis_kernel/metadata.rs
  - crates/polint/src/analysis_kernel/mod.rs
  - crates/polint/src/analysis_kernel/validation.rs
  - crates/polint/src/core/mod.rs
  - crates/polint/src/metrics.rs
  - crates/polint/src/module_graph/mod.rs
  - crates/polint/src/symbol_graph/mod.rs
  - crates/polint/tests/cli.rs
findings:
  critical: 0
  warning: 2
  info: 0
  total: 2
status: issues_found
---

# Phase 21: Code Review Report

**Reviewed:** 2026-05-17T08:10:11Z
**Depth:** standard
**Files Reviewed:** 9
**Status:** issues_found

## Summary

Reviewed the analysis kernel metadata path, validation logic, core fact storage, metric/module/symbol graph derivation, and the related CLI integration coverage. The implementation generally follows the crate visibility model: the reviewed implementation details remain behind `pub(crate)` modules, with public rule-author surfaces kept in `sdk` and `runner`.

Two warning-level correctness issues were found. Both affect reliability of the new provenance/validation path rather than public API shape. Tests were not run; this was a read-only code review.

## Warnings

### WR-01: Module Graph ID Normalization Can Corrupt Cross-References

**File:** `crates/polint/src/core/mod.rs:690`
**Issue:** `replace_module_graph_facts` renumbers `ResolvedImportFact.id`, `ModuleNode.id`, and `ModuleEdge.id`, but it does not remap cross-reference fields that point at those IDs (`ResolvedImportFact.target_node`, `ModuleEdge.from`, `ModuleEdge.to`, and `ModuleEdge.resolved_import`). A caller that provides an internally consistent graph using producer-local IDs will have only the primary IDs rewritten, leaving references pointing at stale IDs. That can surface as internal validation diagnostics or, worse, as rule-facing module graph facts that point at the wrong node/import after normalization. Current tests pass because their references are already pre-normalized, which makes the replacement contract easy to violate.
**Fix:** Either stop rewriting graph IDs in this method and require callers to provide final IDs, or build old-to-new ID maps before assignment and rewrite every dependent reference in the same pass. For example:

```rust
let node_ids = module_nodes
    .iter()
    .enumerate()
    .map(|(index, node)| (node.id, ModuleNodeId(index as u64)))
    .collect::<BTreeMap<_, _>>();
let resolved_ids = resolved_imports
    .iter()
    .enumerate()
    .map(|(index, fact)| (fact.id, ResolvedImportId(index as u64)))
    .collect::<BTreeMap<_, _>>();

for (index, node) in module_nodes.iter_mut().enumerate() {
    node.id = ModuleNodeId(index as u64);
}
for (index, fact) in resolved_imports.iter_mut().enumerate() {
    fact.id = ResolvedImportId(index as u64);
    fact.target_node = fact
        .target_node
        .and_then(|id| node_ids.get(&id).copied());
}
for (index, edge) in module_edges.iter_mut().enumerate() {
    edge.id = ModuleEdgeId(index as u64);
    edge.from = *node_ids
        .get(&edge.from)
        .expect("module edge source node must exist");
    edge.to = *node_ids
        .get(&edge.to)
        .expect("module edge target node must exist");
    edge.resolved_import = edge
        .resolved_import
        .and_then(|id| resolved_ids.get(&id).copied());
}
```

Add a regression test where nodes/imports use non-contiguous source IDs and edges/resolved imports reference those source IDs, then assert the replacement preserves reference integrity after normalization.

### WR-02: Debug Assertion Can Panic Before Metadata Validation Emits Diagnostics

**File:** `crates/polint/src/analysis_kernel/mod.rs:77`
**Issue:** `AnalysisKernel::run` calls `debug_assert!(db.missing_fact_metadata().is_empty())` immediately before `validate_fact_metadata`. In debug/test builds, a provider metadata gap will panic before validation can convert the problem into controlled `polint/internal` diagnostics. That contradicts the reliability goal that parser/provider failures and internal analysis gaps should become diagnostics or controlled errors rather than crashes.
**Fix:** Remove the runtime assertion and rely on the validation pass to report missing metadata. If a hard assertion is still useful, keep it in a dedicated test helper instead of the kernel execution path.

```rust
crate::metrics::derive_requested_metrics(&mut db, input.plan);
let validation_diagnostics =
    validation::validate_fact_metadata(&db, Self::provider_manifests());
diagnostics.extend(validation_diagnostics);
```

---

_Reviewed: 2026-05-17T08:10:11Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

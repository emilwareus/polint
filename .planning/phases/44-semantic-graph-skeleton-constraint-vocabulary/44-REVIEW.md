---
phase: 44-semantic-graph-skeleton-constraint-vocabulary
reviewed: 2026-05-30T11:34:16Z
depth: standard
files_reviewed: 19
files_reviewed_list:
  - crates/polint/src/analysis/semantic_graph/mod.rs
  - crates/polint/src/analysis/semantic_graph/facts.rs
  - crates/polint/src/analysis/semantic_graph/constraints.rs
  - crates/polint/src/analysis/semantic_graph/build.rs
  - crates/polint/src/analysis/semantic_graph/store.rs
  - crates/polint/src/analysis/semantic_graph/provider.rs
  - crates/polint/src/analysis/semantic_graph/cache_key.rs
  - crates/polint/src/analysis/semantic_graph/validate.rs
  - crates/polint/src/analysis/semantic_graph/debug.rs
  - crates/polint/src/analysis/ids.rs
  - crates/polint/src/analysis/mod.rs
  - crates/polint/src/analysis_kernel/provider.rs
  - crates/polint/src/analysis_kernel/validation.rs
  - crates/polint/src/analysis_kernel/mod.rs
  - crates/polint/src/core/mod.rs
  - crates/polint/src/eval/semantic_graph_snapshot.rs
  - crates/polint/src/eval/observed.rs
  - crates/polint/src/eval/fixtures.rs
  - crates/polint/src/eval/mod.rs
findings:
  critical: 0
  warning: 5
  info: 4
  total: 9
status: issues_found
---

# Phase 44: Code Review Report

**Reviewed:** 2026-05-30T11:34:16Z
**Depth:** standard
**Files Reviewed:** 19
**Status:** issues_found

## Summary

Phase 44 adds the private `analysis::semantic_graph` module: typed `NodeKind`/`EdgeKind`/`ConstraintKind` taxonomies, the `SemanticNodeFact`/`SemanticEdgeFact`/`ConstraintFact` families, a `normalized()` densify-after-stable-key-sort pass, the `SemanticGraphStore` with four deterministic indexes, the `polint.semantic_graph` provider entry point with output digest, a parameter/schema cache key, a validation pass, and snapshot fixtures. All graph/constraint types are `pub(crate)` and nothing is promoted to the public SDK surface, so the Phase 42 leak gate stays intact (the `eval/observed.rs` `public_boundary_no_leak` markers do not include semantic-graph identifiers, but the module is correctly never re-exported).

The core determinism machinery is sound: stable keys are length-prefixed (`<len>:<value>`, prefix-free — see `analysis_kernel/metadata.rs:370`), so cross-identity collisions are impossible; all builder maps are `BTreeMap`; and `normalized()` sorts every family by `(stable_key, id)` before assigning dense IDs, with edge endpoints and constraint node-refs remapped through the densification map. No nondeterministic `HashMap` iteration feeds sorted output, and no unstable sort was found.

No BLOCKER-tier correctness or security defects were proven. The findings below are robustness/maintainability concerns: a silent reverse-lookup fallback, a sentinel-value pattern that depends on a guard staying in lockstep, a documented-but-inaccurate "never dense IDs in the digest" claim, an asymmetry between store-time and validate-time precision-ceiling enforcement, and a few quality items. No structural-findings block was supplied with this review.

## Warnings

### WR-01: `node_key_for` silently returns an empty stable key on lookup miss

**File:** `crates/polint/src/analysis/semantic_graph/build.rs:141-146`
**Issue:** `node_key_for` reverse-scans `node_by_key` for a matching `SemanticNodeId` and falls back to `unwrap_or_default()` — i.e. an empty `String` — when no node matches. It is called from `push_edge` to build the edge stable key from its endpoint keys. If an unmapped id ever reaches `push_edge` (today the callers always pass interned ids, but the `u64::MAX` sentinel from `function_node` is one stray edit away from arriving here), the edge would be emitted with a stable key composed from `""` source/target. That is a malformed-but-validation-passing edge (the dangling-endpoint check in `store.rs` keys on the numeric id, not the stable key), which can silently corrupt the byte-stable digest rather than failing loudly. The honesty contract (D-06/D-15) wants a hard failure here, not a blank key.
**Fix:** Make the miss an explicit hard error instead of a blank-key degrade, e.g.:
```rust
fn node_key_for(&self, id: SemanticNodeId) -> String {
    self.node_by_key
        .iter()
        .find_map(|(key, &node_id)| (node_id == id).then(|| key.clone()))
        .unwrap_or_else(|| panic!("push_edge endpoint {id:?} was never interned"))
}
```
or thread a `Result`/skip so a missing endpoint never produces an edge with an empty composed key.

### WR-02: `u64::MAX` sentinel for an unresolved function node is fragile

**File:** `crates/polint/src/analysis/semantic_graph/build.rs:169-179, 198-202`
**Issue:** `function_node` returns `SemanticNodeId(u64::MAX)` when a function has no interned node, and the only thing stopping that poison value from being pushed as a real `Call` edge source is the explicit `&& caller_node != SemanticNodeId(u64::MAX)` guard in `project_call_edges_and_constraints`. A magic sentinel guarded at exactly one call site is a latent dangling-endpoint bug: any future caller of `function_node` that forgets the guard emits an edge whose source resolves to no node. Note that every function IS interned in `project_nodes` before this runs, so the sentinel is unreachable today — which makes the dead guard easy to drop in a later refactor without noticing.
**Fix:** Return `Option<SemanticNodeId>` from `function_node` and let the call site `if let Some(caller_node) = ...` naturally, removing the sentinel and the magic-value comparison entirely.

### WR-03: Output digest folds in dense `SemanticNodeId`s via edge/constraint payloads, contradicting the "never dense IDs" contract

**File:** `crates/polint/src/analysis/semantic_graph/provider.rs:33-34, 60, 95, 136-153`; `crates/polint/src/analysis/semantic_graph/facts.rs:144-145`
**Issue:** The provider docs and the `semantic_graph_output_digest` comment assert the digest is computed "over EXACTLY the stored stable serde payloads (`#[serde(skip)]` strips dense IDs … never dense IDs)". That is only true for each fact's own `id` field. `SemanticEdgeFact.source`/`.target` and the `SemanticNodeId`s inside `ConstraintKind` payloads (`CopyEdge.dst/src`, `CallConstraint.callsite`, etc.) are NOT `#[serde(skip)]`, so `stable_fact_payload` serializes the post-`normalized()` dense node IDs straight into the digest parts. The values are deterministic (assigned by stable-key sort), so this is not a determinism break, but the digest is in fact sensitive to dense node numbering, not purely to stable keys. The stated invariant ("never dense IDs") is therefore inaccurate, and a future change to the densification order would silently change the digest even when no stable identity changed.
**Fix:** Either (a) correct the docs to say the digest covers stable keys plus the deterministic post-sort adjacency numbering, or (b) digest edges/constraints over their stable keys (and the endpoints' stable keys) rather than the serialized dense IDs, so the digest is genuinely dense-ID-free as claimed.

### WR-04: Precision ceiling is enforced at validate-time but NOT at store-time, unlike the dangling-reference check

**File:** `crates/polint/src/analysis/semantic_graph/store.rs:162-204`; `crates/polint/src/analysis/semantic_graph/validate.rs:66-100, 126-136`
**Issue:** `SemanticGraphStore::from_output` rejects dangling edge endpoints and dangling constraint node-refs with `AnalysisError::InvalidFact` (a hard store-time failure). The D-07 precision-ceiling rule ("a derived node/edge must never claim `ResolvedStatic`") is enforced only in the separate `validate_semantic_graph` diagnostic pass that runs later over already-stored facts. The result is asymmetric: a graph containing an exact-equivalent-precision row stores cleanly and yields a present output digest, and is only flagged as a non-fatal diagnostic afterward. A producer bug that emits `ResolvedStatic` would still persist facts and certify a cache hit, defeating the "facts the digest certifies were not persisted → return `output_digest: None`" guarantee for this class of violation.
**Fix:** Move (or mirror) the `ResolvedStatic` ceiling check into `from_output` alongside the dangling-endpoint checks so a precision-ceiling violation is a store-time `InvalidFact` and the provider returns `output_digest: None`, consistent with the referential checks.

### WR-05: Constraint-node referential validation logic is duplicated across three sites and can drift

**File:** `crates/polint/src/analysis/semantic_graph/store.rs:84-135` and `crates/polint/src/analysis/semantic_graph/validate.rs:154-167`
**Issue:** The match over `ConstraintKind` that enumerates which payload fields are `SemanticNodeId` node references appears three times with the same arm structure: `remap_constraint_nodes` (store.rs:84), `constraint_referenced_nodes` (store.rs:125), and a second copy of `constraint_referenced_nodes` (validate.rs:157). Because `ConstraintKind` is a closed enum that will gain fields in later phases (FieldLoad/FieldStore already carry a `field: String` that is correctly skipped, TypeConstraint carries a non-node `type_fact`), any added node-bearing variant or field must be updated in all three matches or the remap/validation silently misses a node — producing stale post-sort references (if `remap_constraint_nodes` misses) or false-clean validation (if either `constraint_referenced_nodes` misses). The enums are exhaustive matches, so a *new variant* fails to compile, but adding a node *field* to an existing variant does not.
**Fix:** Define one canonical `fn constraint_node_refs_mut(&mut ConstraintKind) -> impl Iterator<&mut SemanticNodeId>` (and a `&`-borrow sibling) on `ConstraintKind` in `constraints.rs`, and have remap + both referential checks consume it, so the field enumeration lives in exactly one place next to the type definition.

## Info

### IN-01: `reject_exact_precision` (FactPrecision) helper is dead in the runtime path

**File:** `crates/polint/src/analysis/semantic_graph/validate.rs:143-152`
**Issue:** `reject_exact_precision` takes a `FactPrecision` and is only exercised by the unit test `precision_ceiling_helper_rejects_fact_precision_exact`. The semantic-graph fact types carry no `FactPrecision`-typed field (they use `SemanticPrecision`/`PointsToPrecision`), so the helper is never called from `validate_semantic_graph`. The doc comment acknowledges this ("The semantic-graph fact types carry no `FactPrecision`-typed precision field of their own"). It is a test-only contract placeholder.
**Fix:** Either wire it into the producer-level precision-ceiling check it documents, or drop it and rely on the kernel-wide `validate_precision_ceilings` pass; otherwise mark intent clearly so it is not mistaken for a live guard.

### IN-02: `node_key_for` is an O(n) reverse scan invoked per edge

**File:** `crates/polint/src/analysis/semantic_graph/build.rs:141-146`
**Issue:** `node_key_for` linearly scans `node_by_key` for every `push_edge` call (two scans per edge). This is a correctness-neutral inefficiency (performance is out of v1 scope) but is flagged because the natural fix also resolves WR-01: maintaining a parallel `id -> stable_key` reverse map, or passing the endpoint keys into `push_edge` directly (the callers already hold them), removes both the scan and the empty-string fallback.
**Fix:** Pass the already-known source/target stable keys into `push_edge` from the call sites, eliminating the reverse lookup entirely.

### IN-03: Provider digest helpers carry repeated `#[allow(clippy::too_many_arguments)]` and 10 positional `Digest` params

**File:** `crates/polint/src/analysis/semantic_graph/provider.rs:39-52, 96-109`
**Issue:** `derive_semantic_graph_with_cache_stats` and `semantic_graph_output_digest` each take ten positional upstream `Digest` arguments suppressed with `#[allow(clippy::too_many_arguments)]`. Positional same-typed `Digest` parameters are easy to transpose at a call site with no compiler help (e.g. swapping `symbol_output_digest` and `module_topology_output_digest` would still typecheck and silently mislabel a digest part), which is a real correctness hazard for a cache key.
**Fix:** Group the upstream digests into a named struct (e.g. `SemanticGraphUpstreamDigests { calls, identity, … }`) so each input is named at the call site and transposition is impossible.

### IN-04: `stable_fact_payload` swallows serialization failure into a `Debug` fallback inside a digest input

**File:** `crates/polint/src/analysis/semantic_graph/provider.rs:172-177`
**Issue:** `stable_fact_payload` does `serde_json::to_string(fact).unwrap_or_else(|_| format!("{fact:?}"))`. If serialization ever failed, the digest part would silently switch from the JSON form to the `Debug` form, changing the digest without any signal — and `Debug` output for these types is not guaranteed byte-stable across compiler/representation changes the way the serde form is. For these `#[derive(Serialize)]` plain-data types serialization cannot realistically fail, so this is informational, but a silent format swap in a byte-stability-critical path is worth tightening.
**Fix:** `.expect("semantic graph fact serializes")` (matching the `debug.rs` snapshot path, which already `expect`s) so a serialization regression fails loudly rather than mutating the digest.

---

_Reviewed: 2026-05-30T11:34:16Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

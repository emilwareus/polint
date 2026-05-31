# Phase 44: Semantic Graph Skeleton & Constraint Vocabulary - Pattern Map

**Mapped:** 2026-05-30
**Files analyzed:** 12 new + 4 modified
**Analogs found:** 16 / 16 (every new file has a strong in-codebase analog)

This phase creates a NEW private module `crates/polint/src/analysis/semantic_graph/` (all `pub(crate)`), new ID newtypes, a provider manifest entry, and snapshot fixtures. Every pattern below is drawn from a real analog already in `crates/polint`. The single strongest whole-module analog is **`analysis::reachability/`** (Phase 43 — most recent brand-new private analysis module with full provider/cache-key/validate/store/digest wiring and closed-enum byte-stability). The strongest fact-shape analog is **`analysis::points_to::facts::PointsToConstraintFact`** (the exact `{ id, kind, status, precision, stable_key }` shape D-10 mandates).

> **CRITICAL CORRECTION for planner (D-02/D-03/D-08 `#[repr(u8)]`):** CONTEXT.md repeatedly says "explicit `#[repr(u8)]` ordinals." The **actual codebase byte-stability convention** in every existing closed enum (`RootKind` at `reachability/facts.rs:48-57`, `IdentityKind`/`LanguageTag` at `identity/facts.rs:16-34`) does **NOT** use `#[repr(u8)]`. They achieve byte-stability purely via: pinned declaration order + derived `Ord` + `#[serde(rename_all = "snake_case")]` + an `as_str()` label method + a `*_sorts_in_pinned_declaration_order` test + a `*_has_exactly_N_variants` exhaustive-match test. `reachability/facts.rs:40-47` documents this explicitly: *"No explicit integer-ordinal representation attribute is used."* The planner should follow the **established codebase convention** (pinned order + serde rename + label method + lock tests), NOT add a novel `#[repr(u8)]` that no sibling enum uses. The byte-stability *outcome* CONTEXT.md wants is correct; the mechanism it names is not what the repo does.

> **CRITICAL CORRECTION for planner (D-04 module-node ID type):** CONTEXT.md D-04 writes `Module(ModuleId)` and asks the planner to confirm the type. The actual module-node identity is **`core::ModuleNodeId(pub u64)`** (`crates/polint/src/core/mod.rs:145`) — there is no `ModuleId` type. `ModuleNodeId` is the ID carried by `ModuleNode` (`core/mod.rs:316`) and module edges. Use `Module(ModuleNodeId)`.

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `analysis/semantic_graph/mod.rs` | module | — | `analysis/reachability/mod.rs` | exact |
| `analysis/semantic_graph/facts.rs` (nodes/edges) | model (fact family) | transform/aggregate | `analysis/reachability/facts.rs` + `analysis/calls/facts.rs` | exact |
| `analysis/semantic_graph/constraints.rs` | model (fact family) | transform/aggregate | `analysis/points_to/facts.rs` (`PointsToConstraintFact`) | exact |
| `analysis/semantic_graph/store.rs` | store (indexes) | CRUD/index | `analysis/points_to/store.rs` + `analysis/reachability/store.rs` | exact |
| `analysis/semantic_graph/build.rs` (population) | service | transform/aggregate | `analysis/reachability/discover.rs` + `traverse.rs` | role-match |
| `analysis/semantic_graph/provider.rs` | provider | request-response | `analysis/reachability/provider.rs` | exact |
| `analysis/semantic_graph/cache_key.rs` | config (digest) | transform | `analysis/reachability/cache_key.rs` | exact |
| `analysis/semantic_graph/validate.rs` | validator | transform | `analysis/reachability/validate.rs` | exact |
| `analysis/semantic_graph/debug.rs` (optional) | utility | transform | `analysis/reachability/debug.rs` | exact |
| `analysis/ids.rs` (MODIFIED — add `SemanticNodeId`/`SemanticEdgeId`/`SemanticConstraintId`) | model | — | `analysis/ids.rs:84-138` existing newtypes | exact |
| `analysis/mod.rs` (MODIFIED — register module) | config | — | `analysis/mod.rs` existing `pub(crate) mod` lines | exact |
| `analysis_kernel/provider.rs` (MODIFIED — manifest + order test) | config | — | `provider.rs:532-552` (`polint.reachability`) + `:573-620` (`polint.type_value_alias`) | exact |
| `tests/eval-fixtures/semantic-graph/<go>/...` | test fixture | — | `tests/eval-fixtures/determinism/go_reachable/` | exact |
| `tests/eval-fixtures/semantic-graph/<ts>/...` | test fixture | — | `tests/eval-fixtures/determinism/ts_reachable/` | exact |
| `tests/public_surface_leak.rs` (NOT modified — must stay green) | gate | — | existing `ALLOWED_PRELUDE` (do NOT extend) | constraint |

---

## Shared Patterns (apply across all `semantic_graph/` files)

### S1. Closed-enum byte-stability template (`NodeKind`, `EdgeKind`, `ConstraintKind`)
**Source:** `crates/polint/src/analysis/reachability/facts.rs:40-71` (`RootKind`) and `crates/polint/src/analysis/identity/facts.rs:16-45` (`IdentityKind`/`LanguageTag`).
**Apply to:** `facts.rs`, `constraints.rs`.

The exact, repo-canonical recipe (NO `#[repr(u8)]`):
```rust
/// Closed taxonomy ... (D-02). Pinned declaration order so derived `Ord` and serde
/// are declaration-driven and byte-stable, matching `RootKind`/`IdentityCategory`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NodeKind {
    Function,
    Callsite,
    Scope,
    Place,
    AbstractObject,
    Module,
    Package,
}

impl NodeKind {
    /// Stable lowercase label used in stable keys and digest payloads.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Callsite => "callsite",
            // ...
        }
    }
}
```
**Mandatory lock tests** (copy from `reachability/facts.rs:350-397`): `node_kind_sorts_in_pinned_declaration_order` (sort a permuted vec, assert it returns to declaration order) and `node_kind_has_exactly_7_variants` (exhaustive `match` + array-length assertion that fails to compile if a variant is added). Do the same for `EdgeKind` (4 variants) and `ConstraintKind` (7 variants).

> Note: a `NodeKind`/`EdgeKind`/`ConstraintKind` that **carry payloads** (D-04/D-08) cannot be `Copy` and cannot derive `Ord` if a payload field is non-`Ord`. See V1 below — model the kind *tag* (the `Copy` ordinal enum above) separately from the payload-carrying variant data, exactly as `points_to::PointsToConstraintKind` (`points_to/facts.rs:27-74`) carries struct payloads while still deriving `Ord` because all its payload fields (`PtVarId`, `String`) are `Ord`. The planner must keep every payload field `Ord` to preserve the derive, or split tag from payload.

### S2. Stable-key recipe (length-prefixed labeled parts) — D-06
**Source:** `crates/polint/src/analysis_kernel/metadata.rs:370-385` (`stable_key_from_parts`) and the thin wrapper `crates/polint/src/analysis/stable_key.rs:16-18` (`semantic_stable_key`).
**Apply to:** every node/edge/constraint stable key.

```rust
// analysis_kernel/metadata.rs:370 — sorts parts by label, length-prefixes family
// + each label + each value, normalizes backslashes to '/'.
pub(crate) fn stable_key_from_parts(family: FactFamily, parts: &[(&str, String)]) -> String { ... }

// analysis/stable_key.rs:16 — the crate-facing wrapper returning a typed StableFactKey:
pub(crate) fn semantic_stable_key(family: FactFamily, parts: &[(&str, String)]) -> StableFactKey
```
**D-06 composition rule** — keys are built from *referenced existing stable identity*, NEVER run-local dense IDs:
- **Node key** = `(node-kind label, referenced existing stable identity string)` — e.g. for a `Function` node, the referenced function's stable key / `package.Name`, NOT `FunctionId(n)`.
- **Edge key** = `(edge-kind label, source node stable key, target node stable key)`.

`reachability/facts.rs:235-251` (`compute_reachability_root_stable_key`) is the concrete precedent for a hand-rolled `format!`-based variant with `escape_field` (`facts.rs:255-257`) guarding `|` boundaries; `identity/facts.rs:164-182` is the same pattern. The planner may use either `stable_key_from_parts` (preferred, already sorts+length-prefixes) or the hand-rolled escaped-`format!` style — both exist in the codebase. The hand-rolled style **requires** a `*_disambiguates_field_boundaries` test (`reachability/facts.rs:400-424`).

### S3. Dense-IDs-after-sort + `normalized()` — D-05, D-14
**Source:** `crates/polint/src/analysis/points_to/store.rs:15-33` (`PointsToOutput::normalized`) and `crates/polint/src/analysis/reachability/store.rs:53-65`.
**Apply to:** `store.rs`.

```rust
// points_to/store.rs:16 — sort by (stable_key, id), THEN assign dense IDs by index.
pub(crate) fn normalized(mut self) -> Self {
    self.constraints.sort_by(|l, r| (l.stable_key.as_str(), l.id).cmp(&(r.stable_key.as_str(), r.id)));
    for (index, row) in self.constraints.iter_mut().enumerate() {
        row.id = PointsToConstraintId(index as u64);   // dense IDs assigned AFTER sort
    }
    self
}
```
`SemanticNodeId`/`SemanticEdgeId`/`SemanticConstraintId` are assigned **only after** sorting by stable key (D-05). Indexes (nodes-by-`NodeKind`, edges-by-`EdgeKind`, forward adjacency, constraints-by-`ConstraintKind`) are built in `from_output` **after** `normalized()`, mirroring `points_to/store.rs:45-80` (BTreeMap-keyed `Vec<usize>` index sidecars) and `reachability/store.rs:68-78`.

### S4. Provider output digest recipe — D-17
**Source:** `crates/polint/src/analysis/reachability/provider.rs:144-215` (`reachability_output_digest`) + `cache_key.rs:12-27` (`reachability_provider_parameter_digest`).
**Apply to:** `provider.rs`, `cache_key.rs`.

Digest parts (sorted, then `Digest::from_parts`):
```
provider_id=...        provider_version=...   schema=...   parameters=<param-digest>
config=<input_snapshot.config.digest>
calls_output=<digest>  identity_output=<digest>  abstract_domains_output=<digest>
entrypoints_output=<digest>  reachability_output=<digest>  type_value_alias_output=<digest>
symbol_graph=<digest>  module_topology=<digest>
node=<stable serde payload>  edge=<...>  constraint=<...>    // never dense IDs
<empty sentinel> if all fact sets empty
```
Key disciplines lifted verbatim from `reachability/provider.rs`:
- **Digest over stable serde payloads, never dense IDs** (`provider.rs:226-231` `stable_fact_payload` → `serde_json::to_string`). To keep dense `id` out of the digest, mark it `#[serde(skip)]` exactly like `ReachabilityRootFact.id` (`reachability/facts.rs:24-25`) — and because of that skip, the dense-ID newtype must derive `Default` (`ids.rs:132-138` documents why `ReachabilityRootId` derives `Default`).
- **Empty-output sentinel** so an empty graph's digest differs from a populated one (`provider.rs:205-210`).
- **Parameter digest = a frozen `&[&str]` algorithm-version list** with a lock test that trips on any bump (`cache_key.rs:12-27` + the `*_locks_parts_list` / `algorithm_version_bump_invalidates` tests at `cache_key.rs:33-74`). Use a schema label const like `SEMANTIC_GRAPH_SCHEMA_LABEL = "semantic-graph-facts-1"` (mirror `REACHABILITY_SCHEMA_LABEL`, `cache_key.rs:4`).

### S5. Provider run pipeline shape — D-11, D-13
**Source:** `crates/polint/src/analysis/reachability/provider.rs:36-142` (`derive_reachability_with_cache_stats`).
**Apply to:** `provider.rs`.

The 7-phase pipeline to mirror: (1) extract/project from existing facts (no mutation), (2) partition storable vs unstorable, (3) emit derived rows, (4) `normalized()` the storable set, (5) compute digest over the stored payloads, (6) assign dense IDs as a post-digest read concern, (7) store via a `db.replace_*` call returning `Result`; on store error return `output_digest: None` so a cache layer never records a hit for un-persisted state (`provider.rs:122-141`). The run-output struct shape is `provider.rs:16-21` (`{ diagnostics, cache_stats, output_digest: Option<Digest> }`).

### S6. Honest precision ceiling — D-07, D-15
**Source:** `crates/polint/src/analysis/reachability/validate.rs:122-131` (`reject_exact_precision`).
**Apply to:** `validate.rs`. Derived/aggregated graph edges must reject `FactPrecision::Exact`; reuse the precision vocabulary from `points_to/facts.rs:85-93` (`PointsToPrecision`) or `values/facts.rs:82-90` (`ValuePrecision`) as the field type. `ModelEdge` is reserved-but-empty (D-11) — emit zero, document the honest emptiness, exactly as Phase 43 reserved empty marks.

---

## Pattern Assignments

### `analysis/semantic_graph/mod.rs` (module)
**Analog:** `crates/polint/src/analysis/reachability/mod.rs:1-29`.
Top-of-file `//!` doc-comment with the **mandatory D-09 naming-collision guard** distinguishing the unified frontend `ConstraintKind` from the points-to sub-domain's `PointsToConstraintKind`. Model it on the `reachability/mod.rs:9-20` D-02 guard (which distinguishes whole-program vs block-level reachability). Then `pub(crate) mod` declarations:
```rust
pub(crate) mod build;
pub(crate) mod cache_key;
pub(crate) mod constraints;
pub(crate) mod debug;
pub(crate) mod facts;
pub(crate) mod provider;
pub(crate) mod store;
pub(crate) mod validate;
```

### `analysis/semantic_graph/facts.rs` (node + edge fact families)
**Analog:** `crates/polint/src/analysis/reachability/facts.rs:16-38` (fact-struct shape) + `crates/polint/src/analysis/calls/facts.rs:6-24` (richer composing fact). Node/edge fact shape to mirror:
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SemanticNodeFact {
    #[serde(skip)]                       // dense id never enters digest (S4)
    pub(crate) id: SemanticNodeId,
    pub(crate) kind: NodeKind,           // payload-carrying closed enum (S1, V1)
    pub(crate) precision: ...,           // S6 precision ceiling, if nodes carry it (D-07)
    pub(crate) status: ...,              // planner's discretion (D-07)
    pub(crate) stable_key: String,       // S2, composed from referenced identity
}
```
`NodeKind` variants carry **existing** IDs (D-04, composition over duplication): `Function(core::FunctionId)`, `Callsite(analysis::ids::CallSiteId)`, `Scope(symbol_graph::semantic::ScopeId)`, `Place(analysis::ids::PlaceId)`, `AbstractObject(analysis::ids::ObjectTokenId)`, `Module(core::ModuleNodeId)`, `Package(core::PackageId)`. `EdgeKind` is the `Copy` tag enum (`Call`/`MemberOf`/`Alloc`/`Flow`) per S1; the `SemanticEdgeFact` carries `source: SemanticNodeId`, `target: SemanticNodeId`, the `EdgeKind`, precision, and a `stable_key` built from source/target node stable keys (S2). `ValueSubject` (`values/facts.rs:39-46`) and `CallCallee` (`calls/facts.rs:74-100`) are the precedent for payload-carrying enums whose variants wrap existing IDs.

### `analysis/semantic_graph/constraints.rs` (constraint fact family)
**Analog (EXACT shape, D-10):** `crates/polint/src/analysis/points_to/facts.rs:7-14`:
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ConstraintFact {
    pub(crate) id: SemanticConstraintId,
    pub(crate) kind: ConstraintKind,
    pub(crate) status: ...,           // reuse a status enum à la PointsToStatus (facts.rs:76-83)
    pub(crate) precision: ...,        // S6 ceiling
    pub(crate) stable_key: String,
}
```
`ConstraintKind` (closed, S1) has exactly 7 variants: `CopyEdge`, `Alloc`, `FieldLoad`, `FieldStore`, `CallConstraint`, `ModelEdge`, `TypeConstraint`. Payload shapes follow `PointsToConstraintKind` (`points_to/facts.rs:27-74`), but reference **semantic-graph node IDs** (e.g. `CopyEdge { dst: SemanticNodeId, src: SemanticNodeId }`) or existing fact IDs (`CallConstraint` → a callsite node; `TypeConstraint` → `analysis::ids::TypeFactId`; `ModelEdge` → reserved, no producer until Phase 49). **D-09 GUARD:** this enum is the unified vocabulary *above* `points_to::PointsToConstraintKind`; do NOT merge/rename/delete the points-to enum (that is Phase 47). Document the conceptual map (`CopyEdge` ↔ points-to `Copy`, `Alloc` ↔ `AddressOf`) in a comment but introduce no code coupling.

### `analysis/semantic_graph/store.rs` (output + indexes)
**Analog:** `crates/polint/src/analysis/points_to/store.rs:9-89` + `crates/polint/src/analysis/reachability/store.rs:44-118`.
- `SemanticGraphOutput { nodes, edges, constraints }` with `normalized()` (S3) sorting each by stable key and reassigning dense IDs.
- `SemanticGraphStore::from_output(...)` building BTreeMap index sidecars (nodes-by-`NodeKind`, edges-by-`EdgeKind`, forward adjacency `source SemanticNodeId → Vec<edge idx>`, constraints-by-`ConstraintKind`) — D-14. Referential validation on construction (dangling endpoint → `AnalysisError::InvalidFact`) mirrors `reachability/store.rs:84-112`.
- `pub(crate) const SEMANTIC_GRAPH_PROVIDER_ID: &str = "polint.semantic_graph";` (mirror `reachability/store.rs:8`).

### `analysis/semantic_graph/build.rs` (population from existing facts) — D-11
**Analog:** `crates/polint/src/analysis/reachability/discover.rs` (project roots from facts) + `traverse.rs` (derive marks by walking edges, composition not mutation).
Emit a real-but-minimal graph from already-available v1.2 families: functions/callsites/scopes/places/modules/packages → nodes; `analysis::calls` direct-call edges (`calls/facts.rs` `CallSiteFact`/`CallTargetFact`) → `Call` edges + `CallConstraint`; `analysis::values`/`access_paths` facts (`values/facts.rs` `AllocationTokenFact`/`ValueFact`; `access_paths/facts.rs` `AccessPathProjection::Field`/`Property`) → `Alloc`/`CopyEdge`/`FieldLoad`/`FieldStore`; scope/place/object containment → `MemberOf`. `ModelEdge`: emit **zero**, documented (D-11). Reference all sources by stable key; mutate none (D-13).

### `analysis/semantic_graph/provider.rs`
**Analog:** `crates/polint/src/analysis/reachability/provider.rs` in full (run-output struct `:16-21`, pipeline `:36-142`, digest `:150-215`, error/diagnostic helpers `:233-268`, manifest-lookup test helper `:289-294`). Apply S4 + S5. Consumes the upstream digests listed in D-17.

### `analysis/semantic_graph/cache_key.rs`
**Analog:** `crates/polint/src/analysis/reachability/cache_key.rs` in full. Define `SEMANTIC_GRAPH_SCHEMA_LABEL` + `semantic_graph_provider_parameter_digest()` over a frozen algorithm-version `&[&str]`, plus the two lock tests (S4).

### `analysis/semantic_graph/validate.rs` — D-15
**Analog:** `crates/polint/src/analysis/reachability/validate.rs:14-187`.
Mirror: `check_duplicate_stable_keys` (`validate.rs:133-150`), per-row referential checks (every edge endpoint / constraint ref resolves; `validate.rs:52-115`), `reject_exact_precision` ceiling (`validate.rs:122-131`), dense-IDs-contiguous-and-sorted assertion, and the evidence-bearing `push_diagnostic` helper (`validate.rs:152-171`) — failures surface as structured `Diagnostic`s with `family`/`stable_key`/`field`/`reason` evidence, never silent drops.

### `analysis/ids.rs` (MODIFIED)
**Analog:** existing newtypes at `crates/polint/src/analysis/ids.rs:84-138`. Add:
```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct SemanticNodeId(pub(crate) u64);
// ...SemanticEdgeId, SemanticConstraintId identically.
```
Derive `Default` (needed for the `#[serde(skip)]`-on-`id` digest discipline, S4 — see the documented `ReachabilityRootId` precedent `ids.rs:132-138`). Register each in the `assert_small_id_contract::<...>()` list inside the existing `semantic_id_newtypes_are_...` test (`ids.rs:164-210`).

### `analysis/mod.rs` (MODIFIED)
Add `pub(crate) mod semantic_graph;` alongside the existing private analysis module declarations (same `pub(crate) mod` style as the sibling `reachability`/`identity`/`points_to` entries).

### `analysis_kernel/provider.rs` (MODIFIED) — D-16
**Analog:** the `polint.reachability` manifest `provider.rs:532-552` and `polint.type_value_alias` manifest `provider.rs:573-620`.
1. Add a `const SEMANTIC_GRAPH_SCHEMA: &[SchemaVersion]` next to `REACHABILITY_SCHEMA` (`provider.rs:205-208`), pointing at `crate::analysis::semantic_graph::cache_key::SEMANTIC_GRAPH_SCHEMA_LABEL`.
2. Insert a `ProviderManifest { id: "polint.semantic_graph", kind: ProviderKind::WholeRepoDerived, inputs: &[...], outputs: &["semantic_nodes","semantic_edges","semantic_constraints"], language_scope: MultiLanguage, cache_policy: InMemoryDerived, schema_versions: SEMANTIC_GRAPH_SCHEMA, precision_ceiling: PrecisionCeiling::SetupAware }` — recommended slot **between `polint.type_value_alias` (`provider.rs:573-620`) and `polint.refined_calls` (`provider.rs:621-644`)**, i.e. right after line 620.
3. **Update the three identical `provider_order_for_test()` assertion vectors** (`provider.rs:761-785`, `:790-814`, `:847-870`) and the `provider_order_report_for_test` `ProviderOrderRow` block (around `:1208-1274`) to insert `"polint.semantic_graph"` between `"polint.type_value_alias"` and `"polint.refined_calls"`. Note the real order around the slot is `... reachability, extensions, type_value_alias, [semantic_graph], refined_calls, data_flow ...` — `polint.extensions` sits between reachability and type_value_alias, so confirm against the live vector, not CONTEXT's abbreviated list. The Phase 43 determinism gate auto-enrolls the new provider via `provider_manifests()` (D-18) — no gate edit needed.

### `tests/eval-fixtures/semantic-graph/<go>/` and `<ts>/` — D-12
**Analog:** `tests/eval-fixtures/determinism/go_reachable/` and `.../ts_reachable/`. Each case is a directory with:
- `expected.polint-eval.toml` — header `schema_version = "polint-eval-fixture-1"`, `case_id`, `area = "facts"`, a doc comment block, `[repo] path = "repo"`, `[budget] max_runtime_ms = 120000` (copy `determinism/go_reachable/expected.polint-eval.toml`).
- `repo/.polint.toml` (workspace include + language config), `repo/go.mod` or `repo/package.json`, and a minimal source file (`main.go` / `src/*.ts`) exercising calls + allocs + field ops so the graph emits ≥1 of each node/edge/constraint kind. Snapshots assert byte-stable serialized nodes/edges/constraints, total-ordered by stable key, byte-identical cross-platform and across provider-order shuffles.

---

## Variant / divergence notes for the planner

### V1. Payload-carrying closed enums cannot be `Copy`
`RootKind`/`IdentityKind` are fieldless and `Copy`. `NodeKind`/`EdgeKind`/`ConstraintKind` carry payload IDs (D-04/D-08), so they drop `Copy` (and `Clone`/`PartialOrd`/`Ord` survive only if every payload field is `Ord`). The right precedent is the **payload-carrying** family: `points_to::PointsToConstraintKind` (`points_to/facts.rs:27-74`, derives `Clone, PartialEq, Eq, PartialOrd, Ord, Hash` — NOT `Copy`) and `ValueKind`/`CallCallee`. Keep every payload field `Ord` (all the ID newtypes are) so the `Ord` derive that drives byte-stable ordering survives. If a payload field is non-`Ord`, split a `Copy` tag enum (for the ordinal/`as_str`/index keys) from the payload struct — that is the cleaner option and matches how stores index by a `Copy` kind tag.

### V2. `#[repr(u8)]` is NOT the codebase convention — see the top-of-file correction. Follow pinned-order + serde-rename + `as_str()` + lock tests.

### V3. `Module(ModuleNodeId)` not `Module(ModuleId)` — `core::ModuleNodeId` (`core/mod.rs:145`) is the confirmed module-node identity; there is no `ModuleId`.

### V4. Public-surface-leak gate (`tests/public_surface_leak.rs`)
Do NOT modify it and do NOT extend `ALLOWED_PRELUDE` (`public_surface_leak.rs:42-90`). Every new `semantic_graph` type stays `pub(crate)`; the gate stays green automatically. This is a constraint, not a file to edit.

---

## No Analog Found

None. Every new file maps to a concrete in-codebase analog (most to the Phase 43 `reachability/` sibling module, the constraint fact family to `points_to/`). Use RESEARCH.md only for cross-checking the constraint-vocabulary *concepts*; the *code shapes* are all present in `crates/polint`.

## Metadata

**Analog search scope:** `crates/polint/src/analysis/{reachability,points_to,identity,calls,values,access_paths,ids.rs,stable_key.rs}`, `crates/polint/src/analysis_kernel/{provider.rs,metadata.rs}`, `crates/polint/src/core/mod.rs`, `crates/polint/src/symbol_graph/semantic.rs`, `crates/polint/tests/public_surface_leak.rs`, `tests/eval-fixtures/{determinism,identity}/`.
**Files scanned:** ~20 source files read in full or targeted.
**Pattern extraction date:** 2026-05-30

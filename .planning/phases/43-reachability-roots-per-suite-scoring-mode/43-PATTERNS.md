# Phase 43: Reachability, Roots & Per-Suite Scoring Mode - Pattern Map

**Mapped:** 2026-05-29
**Files analyzed:** 18 (12 new, 6 modified)
**Analogs found:** 18 / 18 (every file has an exact or strong role-match analog in the v1.2/Phase 42 substrate)

This is a Rust static-analysis engine (`crates/polint/`). There are no controllers/components/services — the role taxonomy is mapped to this codebase's fact-family / provider / kernel architecture:

| Engine role | Equivalent of |
|-------------|---------------|
| `facts` | model (typed `pub(crate)` records + closed enums) |
| `provider` | service (extract → dedup → assign-dense-IDs → normalize → digest pipeline) |
| `cache_key` | config (provider parameter digest, schema label) |
| `store` | model+repository (normalized output + read indexes + referential validation) |
| `validate` | middleware (dangling-ref + precision-ceiling + duplicate-key diagnostics) |
| `discover`/`traverse` | utility (pure derivation over existing facts) |
| `eval::*` | request-response pipeline (suite manifest → runner → metrics → report JSON) |
| determinism gate | test harness (parametric over `provider_manifests()`) |

**Naming-collision guard (D-02, MANDATORY):** the existing `polint.domain.reachability` (`crates/polint/src/analysis/domains/core.rs:106`) is the **block-level** abstract domain (`Reachable`/`Unreachable`/`Ambiguous` inside one function body). The new module is **whole-program reachability-from-roots**. Use module `analysis::reachability` + provider id `polint.reachability`; never reuse `polint.domain.reachability`. The top-of-module doc comment stating this distinction is required.

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `analysis/reachability/mod.rs` (NEW) | module-root | n/a | `analysis/identity/mod.rs` | exact |
| `analysis/reachability/facts.rs` (NEW) | model | transform | `analysis/identity/facts.rs` + `analysis/entrypoints/facts.rs` | exact |
| `analysis/reachability/discover.rs` (NEW) | service (extract) | transform | `analysis/identity/provider.rs` (`extract_identity_records`) | role-match |
| `analysis/reachability/traverse.rs` (NEW) | utility | event-driven (BFS/DFS) | `analysis/calls/store.rs` (edge indexes) + new | role-match |
| `analysis/reachability/provider.rs` (NEW) | service | transform | `analysis/identity/provider.rs` + `analysis/entrypoints/provider.rs` | exact |
| `analysis/reachability/cache_key.rs` (NEW) | config | n/a | `analysis/identity/cache_key.rs` + `analysis/entrypoints/cache_key.rs` | exact |
| `analysis/reachability/store.rs` (NEW) | model+repo | transform | `analysis/identity/store.rs` | exact |
| `analysis/reachability/validate.rs` (NEW) | middleware | request-response | `analysis/entrypoints/validate.rs` | exact |
| `analysis/reachability/debug.rs` (NEW, optional) | utility | transform | `analysis/entrypoints/debug.rs` (`#[cfg(test)]`) | role-match |
| `analysis/ids.rs` (MODIFY: add `ReachabilityRootId` + marking IDs) | model | n/a | `analysis/ids.rs` (existing newtypes) | exact |
| `analysis_kernel/provider.rs` (MODIFY: add `polint.reachability` manifest after `polint.entrypoints`) | config | n/a | existing `polint.identity`/`polint.entrypoints` manifest entries | exact |
| `analysis_kernel/mod.rs` (MODIFY: wire provider call) | service | request-response | identity wiring at `analysis_kernel/mod.rs:277-291` | exact |
| `eval/suite.rs` (MODIFY: add `scoring_mode: ScoringMode`) | model+config | request-response | `SuiteManifest` + `SuiteKind`/`SuiteTier` closed enums | exact |
| `eval/runner.rs` + `eval/metrics.rs` (MODIFY: mode-aware scoring) | service | request-response | `eval/metrics.rs` (`compute_metrics`, `categorized_failures_from_db`) | role-match |
| `eval/report.rs` + `eval/observed.rs` (MODIFY: reserve `solver_step_count` / `budget_exceeded_reasons`) | model | request-response | `MetricSummary` + `MetricSections` `#[serde(default)]` discipline | exact |
| `eval/determinism_gate.rs` (NEW) | test harness | batch | `analysis_kernel/provider.rs` (`provider_manifests`/`provider_order_for_test`) + `eval/observed.rs` (`provider_order_invariants`) | role-match |
| `research/evaluation-harness/suites/*.toml` (MODIFY ×4) | config | n/a | `go-x-tools-rta-callgraph.toml` | exact |
| `config/mod.rs` (MODIFY: minimal configured-roots input) | config | request-response | `PolintConfig` `#[serde(default)]` table sections | exact |

> **Note on the "polint-config crate":** the CONTEXT references `crates/polint-config/src/`. No such crate exists in the workspace (`crates/` contains only `polint`, `polint-bench`, `polint-macros`). The `.polint.toml` configured-roots input (D-13) belongs in `crates/polint/src/config/mod.rs` — add a `#[serde(default)]` `[reachability]` table to `PolintConfig`, following the existing `workspace`/`rules`/`languages` section pattern. The planner should treat `config/mod.rs` as the config touch point.

---

## Pattern Assignments

### `analysis/reachability/mod.rs` (module-root)

**Analog:** `analysis/identity/mod.rs` (only 11 lines — flat `pub(crate) mod` list).

Copy the flat module declaration shape and prepend the **mandatory D-02 doc comment**. Identity's `mod.rs` declares each submodule `pub(crate) mod`:

```rust
pub(crate) mod cache_key;
pub(crate) mod dedup;
pub(crate) mod facts;
pub(crate) mod provider;
pub(crate) mod render;
pub(crate) mod categorize;
pub(crate) mod store;
pub(crate) mod validate;
```

For reachability the Claude's-Discretion layout (D, CONTEXT line 65) is `facts.rs`, `provider.rs`, `discover.rs`, `traverse.rs`, `cache_key.rs`, `validate.rs`, `store.rs`, `debug.rs`. Add a top doc comment distinguishing whole-program-from-roots reachability from the block-level `polint.domain.reachability` domain.

---

### `analysis/reachability/facts.rs` (model, transform)

**Analogs:** `analysis/identity/facts.rs` (record shape, ID newtype placement, length-prefixed stable-key recipe, byte-stability tests) and `analysis/entrypoints/facts.rs` (closed status/precision/provenance vocabulary the bridge must match loss-lessly).

**Record + closed-enum derive discipline** (`identity/facts.rs:12-21`, `entrypoints/facts.rs:51-98`):

```rust
// Run-local dense ID newtype — co-located with the records it identifies
// (identity/facts.rs:12). D-06 says ReachabilityRootId goes in analysis::ids;
// follow whichever convention the planner picks but keep this exact derive set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct ReachabilityRootId(pub(crate) u64);

// Closed enum — pinned source order so serde + Ord are declaration-driven and
// byte-stable (D-04). The entrypoint vocabulary enums (entrypoints/facts.rs:68-98)
// are the loss-free bridge targets: RootStatus mirrors EntrypointStatus exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum RootKind {
    Main,
    Init,
    Exported,
    Test,
    FrameworkEntrypoint,
    ConfiguredEntrypoint,
}
```

> **D-04 note on `#[repr(u8)]`:** the CONTEXT D-04 text asks for "explicit `#[repr(u8)]` ordinals … mirror the Phase 42 `IdentityCategory` discipline." The *actual* Phase 42 `IdentityCategory` and `EntrypointKind`/`EntrypointStatus` enums in this codebase do **NOT** carry `#[repr(u8)]` — byte-stability is achieved purely by pinned declaration order + the derived `Ord`/serde `#[serde(rename_all = ...)]`. The planner should match the **established codebase pattern** (pinned order + derived `Ord` + serde rename), which already delivers the D-04 byte-stability guarantee. If `#[repr(u8)]` is added it is additive and harmless, but it is not what the existing code does — do not let the literal D-04 wording override the observed convention. Confirm with a variant-count + sort-order test like `entrypoints/facts.rs:242-260` and `identity/facts.rs:346-369`.

**Record struct shape** — compose v1.2 IDs by reference, never duplicate (D-03; mirror `IdentityRecord` at `identity/facts.rs:82-100` and `EntrypointFact` at `entrypoints/facts.rs:10-28`):

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ReachabilityRootFact {
    pub(crate) id: ReachabilityRootId,
    pub(crate) kind: RootKind,
    pub(crate) language: Language,
    pub(crate) target_function: FunctionId,
    pub(crate) target_symbol: Option<SymbolId>,
    pub(crate) originating_entrypoint: Option<EntrypointId>, // set for Test/FrameworkEntrypoint bridge (D-12)
    pub(crate) file: FileId,
    pub(crate) span: Span,
    pub(crate) precision: RootPrecision,
    pub(crate) provenance: RootProvenance,
    pub(crate) status: RootStatus,
    pub(crate) provider_id: String,
    pub(crate) stable_key: String,
}
```

**Stable-key recipe** — copy the length-prefixed labeled-parts shape with `|` separators + `escape_field` boundary disambiguation (`identity/facts.rs:164-195`). The root `stable_key` is built from `(language, kind, function stable identity)`, **never** run-local IDs (D-06):

```rust
// identity/facts.rs:172-182 — single line, no whitespace, explicit | separators,
// each field escaped so a value containing | cannot forge a boundary.
format!(
    "reachability_root|{}|{}|{}|{}|{}..{}",
    root_kind_label(kind), language_label, escape_field(function_identity),
    file_id.0, span.start_byte, span.end_byte,
)

fn escape_field(value: &str) -> String {
    value.replace('\\', "\\\\").replace('|', "\\|")  // identity/facts.rs:193-195
}
```

**Required co-located tests** (copy from `identity/facts.rs:319-441` and `entrypoints/facts.rs:230-296`): serde round-trip, `assert_copy_ord_hash::<RootKind>()`, sort-order assertion proving pinned declaration order, stable-key boundary disambiguation (`a|b`,`c` ≠ `a`,`b|c`), variant-count lock.

---

### `analysis/reachability/discover.rs` (service: extract; REACH-01)

**Analog:** `analysis/identity/provider.rs:75-130` (`extract_identity_records` / `function_identity_record` / `callsite_identity_record`) — project existing facts into new records, returning `None` to skip rather than fabricating.

The discovery sources (D-07..D-13) all read **existing facts only**:
- **Go `Main`/`Init`** (D-08): `db.functions()` filtered by `FunctionFact.name == "main"`/`"init"` + matching `PackageFact.name`. The Go package-name resolution helper is `identity/provider.rs:232-237` (`package_name_for_go_file` scanning `db.packages()`). `FunctionFact` fields available: `name`, `is_test`, `is_exported`, `language`, `file`, `span` (`core/mod.rs:246-256`).
- **Go/TS `Exported`** (D-09/D-10): `FunctionFact.is_exported` + symbol/module-graph export facts.
- **`Test`/`FrameworkEntrypoint` bridge** (D-12): map `db.entrypoint_facts()` — `EntrypointKind::Test → RootKind::Test`, every other `EntrypointKind` (`entrypoints/facts.rs:52-66`) `→ RootKind::FrameworkEntrypoint`. Carry `originating_entrypoint = Some(ep.id)` and inherit `ep.precision`/`ep.status` so the bridge is loss-free.
- **`ConfiguredEntrypoint`** (D-13): from the new `.polint.toml` `[reachability] roots = [...]` input; unresolvable entries become `RootStatus::Unresolved` roots, never silent drops.

Honest-label pattern (D-07): the `language_tag(...) -> Option<...>` skip at `identity/provider.rs:243-250` returns `None` for `Language::Unknown` — copy this; setup-missing inputs yield `SetupMissing`/`Unresolved` status roots, never fabricated `Resolved` ones.

---

### `analysis/reachability/traverse.rs` (utility: BFS/DFS over direct-call edges; REACH-02)

**Analog:** `analysis/calls/store.rs:51-62` (the `CallStore` edge indexes `outgoing_by_function`, `targets_by_site`) gives the adjacency the BFS walks. There is no existing graph-traversal module, so this is partly net-new, but the edge set is fixed: `CallTargetFact` (`calls/facts.rs:27-40`) with `caller: FunctionId`, `target_function: Option<FunctionId>`, `status: CallTargetStatus`.

BFS/DFS from each `ReachabilityRootFact.target_function` over **direct-call resolved-target edges only** — the only pre-solver edge set (D-18). Mark each call site by its **stable key** via a separate `CallReachabilityFact` family (composition, not mutation of `analysis::calls`):

```rust
// Mirror the IdentityRecord composition-by-reference discipline
// (identity/facts.rs:77-100 references call IDs; never rewrites call facts).
pub(crate) struct CallReachabilityFact {
    pub(crate) call_site_stable_key: String,   // keyed by call-site stable key (D-18)
    pub(crate) in_reachable_graph: bool,
    pub(crate) reason: String,                  // compact root-path/reason
    pub(crate) provider_id: String,
    pub(crate) stable_key: String,
}
```

Determinism: the reachable set / BFS frontier must iterate in a sorted, ID-independent order (sort roots and edges by stable key before traversal) so the marking output is byte-identical regardless of insertion order (D-20/D-21). Document that Phases 47/48 swap the edge set for solver-derived edges behind this same marking contract.

---

### `analysis/reachability/provider.rs` (service; REACH-01/02)

**Analog:** `analysis/identity/provider.rs:31-72` (`derive_identity_with_cache_stats`) — the canonical five-phase pipeline. This is the single most load-bearing pattern to copy:

```rust
// identity/provider.rs:38-71 — copy this five-phase pipeline verbatim in shape:
// Phase 1: extract  (discover.rs — project existing facts, no mutation)
let mut roots = discover_reachability_roots(db);
// Phase 2: dedup    (if needed; identity uses dedup_identity_records)
// Phase 3: assign dense IDs AFTER sort+dedup (D-06 determinism rule)
for (index, root) in roots.iter_mut().enumerate() {
    root.id = ReachabilityRootId(index as u64);
}
// Phase 4: normalize (single-source the sort contract through the Output type)
let output = ReachabilityProviderOutput { roots, marks }.normalized();
// Phase 5: digest over STABLE payloads, never dense IDs (Pattern F)
let output_digest = reachability_output_digest(manifest, input_snapshot, &deps, &output);
// then db.replace_reachability_facts(output) with diagnostic-on-error
```

**Output digest discipline** (D-19) — copy `identity/provider.rs:253-285` AND `entrypoints/provider.rs:67-146`. The digest parts list digests provider id/version/schema/parameters/config + **every upstream provider output digest** the reachability provider consumes, then sorts the parts and feeds `Digest::from_parts(DigestKind::ProviderOutput, ...)`:

```rust
// identity/provider.rs:259-284 shape — extend with the upstream digests D-19 lists:
let mut parts = vec![
    format!("provider_id={}", manifest.id),
    format!("provider_version={}", manifest.provider_version()),
    format!("schema={}", manifest.primary_schema_label()),
    format!("parameters={}", reachability_provider_parameter_digest()),
    format!("config={}", input_snapshot.config.digest),         // configured-roots input rides here (D-13/D-19)
    format!("calls_output={calls_output_digest}"),              // direct-call edges
    format!("entrypoints_output={entrypoints_output_digest}"),  // Test/Framework bridge
    format!("identity_output={identity_output_digest}"),        // root identity
    // + symbol/module-graph digests (D-19)
];
parts.extend(output.roots.iter().map(|r| format!("root={}", stable_payload(r))));
parts.extend(output.marks.iter().map(|m| format!("mark={}", stable_payload(m))));
if output.roots.is_empty() && output.marks.is_empty() { parts.push("reachability_output=empty".to_string()); }
parts.sort();
Digest::from_parts(DigestKind::ProviderOutput, "reachability_output", &refs)
```

The empty-output sentinel (`identity/provider.rs:279-281`, `entrypoints/provider.rs:135-141`) and `*_output_digest_for_test` helper (`identity/provider.rs:312-314`) must be carried over. The `valid_call_site_ids(db)` accessor (`identity/provider.rs:307-309`) is the model for any referential-integrity ID set the store needs.

---

### `analysis/reachability/cache_key.rs` (config)

**Analog:** `analysis/identity/cache_key.rs` (schema label const + parameter digest + locked-parts trip-wire test) and `analysis/entrypoints/cache_key.rs:3-23`.

```rust
// identity/cache_key.rs:3 + 18-31 — schema label const + frozen parameter parts list.
pub(crate) const REACHABILITY_SCHEMA_LABEL: &str = "reachability-facts-1";

pub(crate) fn reachability_provider_parameter_digest() -> Digest {
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "reachability_provider_parameters",
        &["reachability-facts-1", "reachability_roots", "call_marks",
          "go_main_init_v1", "exported_v1", "entrypoint_bridge_v1", "configured_roots_v1", "bfs_v1"],
    )
}
```

Copy the `*_locks_parts_list` test (`identity/cache_key.rs:37-54`) — it is the intended trip-wire: any algorithm-version bump must extend this list and deterministically invalidate the cache. Note (D-19): the **precision ceiling rejects `FactPrecision::Exact`** — reachability over direct calls is setup-aware/conservative.

---

### `analysis/reachability/store.rs` (model+repository)

**Analog:** `analysis/identity/store.rs` (entire file is the template).

- `ReachabilityProviderOutput { roots, marks }` with `empty()` + `normalized()` that sorts by a locked sort key (`identity/store.rs:15-26`). The sort key is the determinism contract — copy `record_sort_key` / total-order discipline from `dedup.rs:69-78` (sort by stable identity, then extend with remaining fields so no two distinct records tie — `dedup.rs:41-51`, `TotalOrderKey`).
- Typed store with `BTreeMap` read indexes (`by_kind`, `by_language`, `by_function`) — `identity/store.rs:30-105`.
- `from_output(...)` validates every composed reference (`originating_entrypoint`, `target_function`) against supplied valid ID sets and returns `AnalysisError::InvalidFact` on a dangling ref — copy `identity/store.rs:40-88` exactly (the dangling-call-site rejection there is the model for dangling-entrypoint rejection here).

---

### `analysis/reachability/validate.rs` (middleware)

**Analog:** `analysis/entrypoints/validate.rs` (the richer of the two validators — duplicate-key, dangling-ref, span, AND precision-ceiling checks).

Copy the structure of `validate_entrypoints` (`entrypoints/validate.rs:8-264`):
1. `check_duplicate_stable_keys` per fact family (`:266-283`).
2. `check_ref` against `db.functions()`/`db.symbols()`/`db.files()`/entrypoint-key sets (`:285-297`).
3. Span byte-range sanity (`start_byte > end_byte`).
4. **Precision-ceiling check** (`:213-234`) — reject `FactPrecision::Exact` for the `polint.reachability` producer, exactly as entrypoints rejects Exact for framework facts (D-19).
5. Diagnostics via `Diagnostic::error(...).with_evidence("family"/"stable_key"/"field"/"reason", ...)` (`:299-318`).

The co-located tests (`entrypoints/validate.rs:345-770`) show the full positive/negative coverage shape to replicate (dangling fn, dangling symbol, invalid span, duplicate key, precision ceiling, kernel-integrated validation).

---

### `analysis/ids.rs` (MODIFY — add `ReachabilityRootId` + any marking IDs)

**Analog:** the existing newtype block (`ids.rs:3-130`). Append following the exact one-line pattern; every ID is the same derive set:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct ReachabilityRootId(pub(crate) u64);
```

Add the new IDs to the `assert_small_id_contract::<...>()` roster in the test at `ids.rs:156-201`. (Note: identity chose to co-locate `IdentityRecordId` in `facts.rs` instead — `identity/facts.rs:12`; D-06 explicitly says put `ReachabilityRootId` in `ids.rs`, so follow the `ids.rs` convention here.)

---

### `analysis_kernel/provider.rs` (MODIFY — add `polint.reachability` manifest)

**Analog:** the `polint.identity` (`:428-443`) and `polint.entrypoints` (`:498-526`) manifest entries in `PROVIDER_MANIFESTS`. Insert the new manifest **immediately after `polint.entrypoints`** (D-19). Copy the `ProviderManifest` literal shape:

```rust
// after the polint.entrypoints entry (provider.rs:498-526):
ProviderManifest {
    id: "polint.reachability",
    kind: ProviderKind::WholeRepoDerived,
    inputs: &[
        "source_files", "functions", "symbols", "references",
        "call_sites", "call_targets", "unresolved_calls",   // direct-call edge set
        "entrypoints",                                        // Test/Framework bridge
        "identity_records",                                   // root identity
        "exports", /* + module-graph export inputs */
    ],
    outputs: &["reachability_roots", "call_reachability"],
    language_scope: LanguageScope::MultiLanguage,
    cache_policy: CachePolicy::InMemoryDerived,
    schema_versions: REACHABILITY_SCHEMA,    // const built like IDENTITY_SCHEMA (provider.rs:185-188)
    precision_ceiling: PrecisionCeiling::SetupAware,   // NOT Exact (D-19)
},
```

Add a `const REACHABILITY_SCHEMA` referencing `crate::analysis::reachability::cache_key::REACHABILITY_SCHEMA_LABEL` (mirror `IDENTITY_SCHEMA` at `:185-188`). Then **update the three order-assertion tests** that list the full provider order (`:733-759`, `:761-786`, `:816-841`) and the `provider_order_report_for_test` golden (`:889-1311`) to splice `"polint.reachability"` after `"polint.entrypoints"`. These tests are the determinism anchor for D-21/D-22 — the gate harness reads `provider_manifests()` so the new provider auto-enrolls.

---

### `analysis_kernel/mod.rs` (MODIFY — wire the provider call)

**Analog:** the identity provider wiring at `analysis_kernel/mod.rs:277-291`:

```rust
let identity = crate::analysis::identity::provider::derive_identity_with_cache_stats(
    &mut db, &input_snapshot,
    Self::provider_manifest("polint.identity"),
    calls_dependency_output_digest.clone(),
);
let identity_output_digest = identity.output_digest;
diagnostics.extend(identity.diagnostics);
provider_outputs.push(Self::provider_output_for_with_optional_digest(
    "polint.identity", &db, polint_identity_cache_stats, identity_output_digest,
));
```

Call `derive_reachability_with_cache_stats(...)` in the same shape, sequenced **after** the entrypoints provider call, passing the calls/entrypoints/identity/symbol output digests it digests (D-19).

---

### `eval/suite.rs` (MODIFY — add required `scoring_mode: ScoringMode`)

**Analog:** `SuiteManifest` (`suite.rs:66-101`) + the closed serde-rename enums `SuiteKind`/`SuiteTier`/`ExpectedSourceFormat` (`suite.rs:11-64`).

```rust
// suite.rs:11-21 closed-enum-with-wire-strings pattern — D-14 wire strings via serde rename.
// D-14 requires explicit kebab-case wire strings, so use #[serde(rename = "...")] per variant
// (NOT rename_all = "snake_case", which would emit "oracle_rta"):
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ScoringMode {
    #[serde(rename = "oracle-rta")]   OracleRta,
    #[serde(rename = "oracle-jelly")] OracleJelly,
    #[serde(rename = "whole-repo")]   WholeRepo,
}
```

Add `pub(crate) scoring_mode: ScoringMode,` to `SuiteManifest` (`suite.rs:68-83`) as a **non-`Option`** field. Gate-fails-if-missing is then two-layered (D-15): (a) `SuiteManifest` already has `#[serde(rename_all = "snake_case", deny_unknown_fields)]` (`suite.rs:67`) so a missing non-`Option` field fails TOML deserialization, and (b) add an explicit check in `SuiteManifest::validate()` (`suite.rs:86-100`).

Tests to add, modeled on existing suite tests:
- A **byte-for-byte wire-string assertion** (`oracle-rta`/`oracle-jelly`/`whole-repo`) — like `suite_manifest_denies_unknown_fields` (`suite.rs:264-295`).
- A **negative test** that a manifest missing `scoring_mode` is rejected (explicit verification artifact — Specific Ideas / D-15).
- The `committed_evaluation_suite_manifests_parse_and_validate` test (`suite.rs:241-262`) already loops every committed suite TOML — it will enforce the field is present in all four manifests after they are updated.
- Update the `manifest(...)` test helper (`suite.rs:297-346`) to include `scoring_mode`.

---

### `eval/runner.rs` + `eval/metrics.rs` (MODIFY — mode-aware scoring; D-17)

**Analog:** `eval/metrics.rs` (`compute_metrics` at `:115-253`, `categorized_failures_from_db` at `:360-404`) — the projection-from-facts + match-outcome counting path.

Mode semantics (D-17) — **getting `oracle-rta` vs `oracle-jelly` backwards silently tanks recall** (Specific Ideas):
- `oracle-rta`: filter scored edges to those whose source function is in the reachable-from-roots set (consult `CallReachabilityFact.in_reachable_graph`). Edges outside the reachable graph are excluded from precision/recall but retained as facts marked unreachable.
- `oracle-jelly`: reachability marking recorded but does **not** filter scoring — score the full enumerated set.
- `whole-repo`: no reachability filtering.

The filter consults the reachable-graph marking by call-site stable key — the same composition-by-stable-key discipline `categorized_failures_from_db` uses to join `db.identity_records()` to oracle spans (`metrics.rs:386-401`).

---

### `eval/report.rs` + `eval/observed.rs` (MODIFY — reserve solver fields; D-23)

**Analog:** `MetricSummary` (`report.rs:64-94`) + `MetricSections` (`report.rs:96-112`) + the layout-lock destructure test (`report.rs:723-758`).

D-23 says reserve `solver_step_count` (default 0) and `budget_exceeded_reasons` (default empty) NOW. Follow the frozen-`MetricSummary`-shape discipline:
- `MetricSummary` is layout-locked by the destructure test (`report.rs:730-757`) — adding a field there means updating that test too. Per the comment at `report.rs:725-728`, **extensions live on `MetricSections`, not `MetricSummary`**. So add the new fields as a new `#[serde(default)]` section on `MetricSections` (sibling of `categorized_failures` / `jelly_oracle_coverage` at `report.rs:108-111`), NOT on `MetricSummary` directly:

```rust
// report.rs:96-112 MetricSections — add a sibling reserved section with #[serde(default)]
// so older v1.2 JSON still deserializes (Pattern M, like jelly_oracle_coverage / categorized_failures).
#[serde(default)]
pub(crate) solver: SolverMetricSection,   // { solver_step_count: u64 (=0), budget_exceeded_reasons: Vec<String> (=[]) }
```

The section struct copies the `#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]` + `#[serde(rename_all = "snake_case", deny_unknown_fields)]` shape of `CategorizedFailureSection` (`report.rs:144-152`). Add a destructure layout-lock test for the new section mirroring `metric_summary_layout_unchanged`. Byte-identity of the full normalized observed JSON then transitively covers (b) solver step counts and (c) budget reasons (D-20).

---

### `eval/determinism_gate.rs` (NEW — REACH-03; D-20..D-25)

**Analogs:**
- `analysis_kernel/provider.rs` `provider_manifests()` / `provider_order_for_test()` (`:84-94`) — the parametric source the gate is driven by so future solver providers auto-enroll (D-22, "near-zero-maintenance").
- `eval/observed.rs` `provider_order_invariants()` (`:484-500`) — already iterates `AnalysisKernel::provider_manifests()` and projects deterministic observed rows; the gate's provider-set enumeration copies this iteration.
- `eval/report.rs` `eval_report_normalization_makes_json_order_independent` (`:760-766`) + `deterministic_output_hash` — the existing byte-identity-under-reordering proof the gate generalizes across N=10 seeded permutations.
- `analysis/domains/solver.rs:435` `deterministic_shuffled_rows_produce_byte_identical_result_digests` — the existing single-provider seeded-shuffle precedent.

Contract (fixed, D-20/D-21): run the eval observation **N=10 times under seeded distinct permutations** of (1) provider execution order where the DAG allows and (2) provider output-row / fact-insertion order; assert byte-identical normalized observed JSON across all 10. Place at `crates/polint/src/eval/determinism_gate.rs` (`pub(crate)`/test-facing) — add `pub(crate) mod determinism_gate;` to `eval/mod.rs:9-26`.

D-24: add fixtures under `tests/eval-fixtures/determinism/` — the structural precedent is `tests/eval-fixtures/identity/{dedup,crlf_normalization,jelly_oracle_coverage,categorized_failures}/` (each is a `repo/` + `.polint.toml` + `expected.polint-eval.toml`). Include a Go case AND a TS/JS case, each with roots + direct calls + ≥1 unreachable call. Gate runs in fast CI on Linux + macOS, both must pass independently (no averaging) — same contract as the Phase 42 leak gate (`tests/public_surface_leak.rs:11-13`).

D-25: document in the gate file the per-phase obligation that every subsequent solver phase (44–54) keeps the fixture green as a named acceptance gate.

---

### `research/evaluation-harness/suites/*.toml` (MODIFY ×4 — D-16)

**Analog:** `go-x-tools-rta-callgraph.toml` (full structure shown below). Add a top-level `scoring_mode = "..."` key (alongside `kind`, `languages`, `adapter_id`):

| Suite file | `scoring_mode` |
|------------|----------------|
| `go-x-tools-rta-callgraph.toml` | `"oracle-rta"` |
| `jelly-callgraph-micro.toml` | `"oracle-jelly"` |
| `gosec-samples.toml` | `"whole-repo"` |
| `secbench-js-smoke.toml` | `"whole-repo"` |

```toml
# go-x-tools-rta-callgraph.toml:1-10 — add scoring_mode in this top-level block:
schema_version = "polint-eval-suite-1"
id = "go-x-tools-rta-callgraph"
name = "Go x/tools RTA callgraph"
kind = "call_graph_precision"
languages = ["go"]
adapter_id = "go_x_tools_rta_callgraph"
scoring_mode = "oracle-rta"        # <-- NEW (D-14/D-16)
# ... source_url, license, language_support, [checkout], [expected], [scoring], [tiers.*] unchanged
```

---

### `config/mod.rs` (MODIFY — minimal configured-roots input; D-13)

**Analog:** `PolintConfig` (`config/mod.rs:19-35`) — every section is a `#[serde(default)]` field on the root struct, each its own `#[derive(Debug, Clone, Default, Serialize, Deserialize)]` struct.

```rust
// config/mod.rs:19-35 PolintConfig — add a defaulted [reachability] section:
#[serde(default)]
pub(crate) reachability: ReachabilityConfig,

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct ReachabilityConfig {
    #[serde(default)]
    pub(crate) roots: Vec<String>,   // e.g. ["pkg/path.Func", "src/x.ts#handler"] (D-13; exact shape planner's choice)
}
```

Keep it minimal and honest (D-13): an unresolvable configured root becomes a `RootStatus::Unresolved` root fact in discovery, never a silent drop. This input must participate in the reachability cache key (D-19) — it rides on `input_snapshot.config.digest`, which is already digested in every provider's output-digest parts list (`identity/provider.rs:264`).

---

## Shared Patterns

### Composition over mutation (D-03, D-18)
**Source:** `analysis/identity/facts.rs:77-100` (`IdentityRecord` references `CallSiteId`/`CallTargetId` by composition; call facts never mutated).
**Apply to:** `ReachabilityRootFact` (references `FunctionId`/`SymbolId`/`EntrypointId`) and `CallReachabilityFact` (keys call sites by stable key). Never mutate `analysis::calls` or `analysis::entrypoints`.

### Length-prefixed labeled-parts stable key (D-06)
**Source:** `analysis/identity/facts.rs:164-210` (`compute_identity_stable_key` + `escape_field` + `push_length_prefixed`).
**Apply to:** every reachability fact `stable_key` and any digest payload. Single line, `|`-separated, each field escaped, built from stable identity not run-local IDs.

### Provider output digest over stable payloads (D-19)
**Source:** `analysis/identity/provider.rs:253-285` + `analysis/entrypoints/provider.rs:67-146`. Parts = provider id/version/schema/parameters/config + every upstream output digest + per-record stable payloads; `parts.sort()`; `Digest::from_parts(DigestKind::ProviderOutput, ...)`; empty-output sentinel. Provider parameter digest with a locked-parts trip-wire test: `analysis/identity/cache_key.rs:18-54`.
**Apply to:** `reachability/provider.rs` + `reachability/cache_key.rs`.

### Sort-then-assign-dense-IDs determinism (D-06, D-20)
**Source:** `analysis/identity/provider.rs:42-48` (assign `IdentityRecordId(index)` only AFTER sort+dedup) + `analysis/identity/store.rs:22-26` (`normalized()` sorts by locked key) + `analysis/identity/dedup.rs:41-78` (total-order key so distinct records never tie).
**Apply to:** `reachability/provider.rs` (assign `ReachabilityRootId` after sort) + `reachability/store.rs` (`normalized()`).

### Closed-enum byte-stability (D-04, D-14)
**Source:** `analysis/entrypoints/facts.rs:51-98` + `analysis/identity/facts.rs:16-21` + `eval/suite.rs:11-21`. Pinned declaration order + derived `Ord` + serde rename — NO `#[repr(u8)]` in the existing code (see D-04 note above). Variant-count + sort-order tests: `entrypoints/facts.rs:230-296`, `identity/facts.rs:346-369`.
**Apply to:** `RootKind`/`RootStatus`/`RootPrecision`/`RootProvenance` and `ScoringMode`.

### Honest status/precision, never silent drop or fake Exact (D-07, D-13, D-19)
**Source:** `analysis/identity/provider.rs:243-250` (`language_tag` returns `None` to skip Unknown) + `analysis/entrypoints/validate.rs:213-234` (precision-ceiling rejects `FactPrecision::Exact`).
**Apply to:** `reachability/discover.rs` (SetupMissing/Unresolved roots) + `reachability/validate.rs` (reject Exact) + `polint.reachability` manifest `precision_ceiling: PrecisionCeiling::SetupAware`.

### Referential-integrity store validation (D-03)
**Source:** `analysis/identity/store.rs:40-88` (rejects dangling `originating_call_site_id` with `AnalysisError::InvalidFact`) + `analysis/entrypoints/validate.rs:62-107` (`check_ref` against `db.functions()`/`db.symbols()`).
**Apply to:** `reachability/store.rs::from_output` (validate `originating_entrypoint`, `target_function`).

### Frozen report shape via `#[serde(default)]` section (D-23)
**Source:** `eval/report.rs:96-112` (`MetricSections` adds sections via `#[serde(default)]`) + `eval/report.rs:723-758` (destructure layout-lock test) + `eval/report.rs:144-152` (`CategorizedFailureSection` derive shape).
**Apply to:** reserved `solver` section (`solver_step_count`, `budget_exceeded_reasons`) on `MetricSections`.

### `deny_unknown_fields` + non-Option = structural gate (D-15)
**Source:** `eval/suite.rs:66-67` (`SuiteManifest` `#[serde(deny_unknown_fields)]`) + `eval/suite.rs:264-295` (denies-unknown-fields test) + `eval/suite.rs:86-100` (`validate()` explicit checks).
**Apply to:** required `scoring_mode` field + explicit `validate()` check + negative test.

### Parametric harness driven by `provider_manifests()` (D-22)
**Source:** `analysis_kernel/provider.rs:84-94` (`provider_manifests`/`provider_order_for_test`) + `eval/observed.rs:484-500` (`provider_order_invariants` iterates the manifests).
**Apply to:** `eval/determinism_gate.rs` provider-set enumeration so phases 44–54 auto-enroll.

### Cross-platform byte-identical CI gate (D-24)
**Source:** `tests/public_surface_leak.rs:11-13` (Linux + macOS, both pass independently, no averaging).
**Apply to:** the determinism gate fixtures.

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `analysis/reachability/traverse.rs` (BFS/DFS body) | utility | event-driven | No whole-program graph-traversal module exists yet — the abstract-domain `polint.domain.reachability` is block-level only. The **edge set** (`analysis/calls`) and the **adjacency index shape** (`calls/store.rs:51-62`) are established; the BFS frontier walk itself is net-new. Determinism (sorted frontier, stable-key keying) follows the shared determinism patterns above. |
| `eval/determinism_gate.rs` (N=10 permutation harness) | test harness | batch | No multi-provider permutation harness exists. The single-provider seeded-shuffle precedent (`domains/solver.rs:435`) and the report order-independence proof (`report.rs:760`) are the closest building blocks; the parametric multi-provider gate is net-new but assembled entirely from `provider_manifests()` + existing normalized-JSON byte-identity. |

Everything else has an exact or strong role-match analog; the planner should copy patterns directly from the cited file:line ranges rather than inventing new shapes.

## Metadata

**Analog search scope:** `crates/polint/src/analysis/{identity,entrypoints,calls,domains}/`, `crates/polint/src/analysis/ids.rs`, `crates/polint/src/analysis_kernel/{provider.rs,mod.rs}`, `crates/polint/src/eval/{suite.rs,metrics.rs,observed.rs,report.rs,mod.rs}`, `crates/polint/src/config/mod.rs`, `crates/polint/src/core/mod.rs`, `crates/polint/tests/public_surface_leak.rs`, `research/evaluation-harness/suites/`, `tests/eval-fixtures/identity/`.
**Files scanned:** ~20 source files + 4 suite manifests + fixture tree listing.
**Pattern extraction date:** 2026-05-29

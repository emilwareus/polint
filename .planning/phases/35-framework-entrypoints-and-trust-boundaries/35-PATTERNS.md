# Phase 35: Framework Entrypoints and Trust Boundaries - Pattern Map

**Mapped:** 2026-05-23
**Files analyzed:** 14 new/modified files
**Analogs found:** 14 / 14

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `analysis/entrypoints/facts.rs` | model | CRUD | `analysis/calls/facts.rs` | exact |
| `analysis/entrypoints/store.rs` | store | CRUD | `analysis/calls/store.rs` | exact |
| `analysis/entrypoints/provider.rs` | provider | transform | `analysis/calls/provider.rs` | exact |
| `analysis/entrypoints/cache_key.rs` | config | transform | `analysis/calls/cache_key.rs` | exact |
| `analysis/entrypoints/validate.rs` | middleware | transform | `analysis/calls/validate.rs` | exact |
| `analysis/entrypoints/extract.rs` | service | transform | `analysis/calls/extract.rs` | exact |
| `analysis/entrypoints/unresolved.rs` | service | transform | `analysis/calls/unresolved.rs` | exact |
| `analysis/entrypoints/mod.rs` | config | CRUD | `analysis/calls/mod.rs` | exact |
| `analysis/ids.rs` (modified) | model | CRUD | `analysis/ids.rs` | exact |
| `analysis_kernel/metadata.rs` (modified) | model | CRUD | `analysis_kernel/metadata.rs` | exact |
| `analysis_kernel/provider.rs` (modified) | config | CRUD | `analysis_kernel/provider.rs` | exact |
| `analysis_kernel/validation.rs` (modified) | middleware | transform | `analysis_kernel/validation.rs` | exact |
| `analysis/mod.rs` (modified) | config | CRUD | `analysis/mod.rs` | exact |
| `analysis/extensions/sinks.rs` (modified) | model | CRUD | `analysis/extensions/sinks.rs` | exact |

## Pattern Assignments

### `analysis/entrypoints/facts.rs` (model, CRUD)

**Analog:** `crates/polint/src/analysis/calls/facts.rs`

**Imports pattern** (lines 1-2):
```rust
use serde::{Deserialize, Serialize};

use crate::analysis::ids::{CallSiteId, CallTargetId, MirBodyId, MirOpId, PlaceId};
use crate::core::{FileId, FunctionId, Language, ReferenceId, Span, SymbolId};
```

Entrypoints facts should use the same import style. New IDs (`EntrypointId`, `TrustBoundaryId`, `DispatchEdgeId`, `UnresolvedFrameworkId`) are defined in `analysis/ids.rs` and imported here.

**Core fact struct pattern** (lines 7-24):
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CallSiteFact {
    pub(crate) id: CallSiteId,
    pub(crate) language: Language,
    pub(crate) file: FileId,
    pub(crate) caller: FunctionId,
    // ... domain-specific fields ...
    pub(crate) status: CallTargetStatus,
    pub(crate) precision: CallPrecision,
    pub(crate) stable_key: String,
}
```

Pattern: every fact struct has `id`, `language`, domain fields, `precision`, and `stable_key`. The four new fact types (`EntrypointFact`, `TrustBoundaryFact`, `FrameworkDispatchEdgeFact`, `UnresolvedFrameworkFact`) must follow this layout.

**Enum pattern** (lines 54-72, 102-135, 148-167, 169-178, 180-190):
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum CallSyntaxKind {
    Function,
    Method,
    // ...
    Unknown,
}
```

All vocabulary enums derive the full `Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize` trait set. New enums: `EntrypointKind` (HttpRoute, HttpMiddleware, McpTool, McpResource, McpPrompt, CliCommand, Test, Job, QueueConsumer, ServerlessHandler, LifecycleCallback, EventListener, GeneratedDispatch), `TrustBoundarySourceKind`, `DispatchEdgeKind`, `UnresolvedFrameworkReason`, `EntrypointPrecision`, `EntrypointProvenance`.

**Test pattern** (lines 193-290):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{FileId, FunctionId, Language, SymbolId};
    use serde::Serialize;
    use serde::de::DeserializeOwned;
    use std::fmt::Debug;
    use std::hash::Hash;

    fn assert_small_id_contract<T>()
    where
        T: Debug + Clone + Copy + PartialEq + Eq + PartialOrd + Ord + Hash + Serialize + DeserializeOwned,
    { }

    #[test]
    fn call_facts_keep_dense_ids_and_stable_keys_separate() {
        // Construct facts, verify stable_key != id, verify cross-references
    }
}
```

Pattern: test module verifies ID contract traits, stable key separation, and vocabulary completeness.

---

### `analysis/entrypoints/store.rs` (store, CRUD)

**Analog:** `crates/polint/src/analysis/calls/store.rs`

**Output container pattern** (lines 10-15):
```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CallOutput {
    pub(crate) sites: Vec<CallSiteFact>,
    pub(crate) targets: Vec<CallTargetFact>,
    pub(crate) unresolved: Vec<UnresolvedCallFact>,
}
```

New: `EntrypointOutput` with `entrypoints: Vec<EntrypointFact>`, `trust_boundaries: Vec<TrustBoundaryFact>`, `dispatch_edges: Vec<FrameworkDispatchEdgeFact>`, `unresolved: Vec<UnresolvedFrameworkFact>`.

**Normalization pattern** (lines 22-48):
```rust
impl CallOutput {
    pub(crate) fn normalized(mut self) -> Self {
        self.sites.sort_by(|left, right| {
            (left.stable_key.as_str(), left.id).cmp(&(right.stable_key.as_str(), right.id))
        });
        // ... sort all vectors by stable_key ...
        self
    }
}
```

Pattern: `normalized()` sorts each fact vector by `(stable_key, id)` or `(parent_key, stable_key, id)` for deterministic output.

**Store with indexes pattern** (lines 51-62):
```rust
#[derive(Debug, Clone, Default)]
pub(crate) struct CallStore {
    output: CallOutput,
    sites_by_caller: BTreeMap<FunctionId, Vec<usize>>,
    targets_by_site: BTreeMap<CallSiteId, Vec<usize>>,
    // ... index maps ...
}
```

New: `EntrypointStore` with indexes like `entrypoints_by_kind`, `entrypoints_by_file`, `trust_boundaries_by_entrypoint`, `dispatch_edges_by_entrypoint`, `unresolved_by_reason`.

**from_output validation pattern** (lines 65-99):
```rust
impl CallStore {
    pub(crate) fn from_output(output: CallOutput) -> Result<Self, AnalysisError> {
        let output = output.normalized();
        // validate referential integrity (dangling references)
        for target in &output.targets {
            if !site_ids.contains(&target.site) {
                return Err(AnalysisError::InvalidFact { ... });
            }
        }
        // build indexes
        Ok(store)
    }
}
```

Pattern: `from_output` normalizes, validates referential integrity (trust boundaries reference entrypoints, dispatch edges reference entrypoints), then builds BTreeMap indexes. Returns `AnalysisError::InvalidFact` on validation failure.

---

### `analysis/entrypoints/provider.rs` (provider, transform)

**Analog:** `crates/polint/src/analysis/calls/provider.rs`

**Provider output struct pattern** (lines 17-22):
```rust
#[derive(Debug, Clone, Default)]
pub(crate) struct CallsProviderOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}
```

New: `EntrypointsProviderOutput` with identical shape.

**Main derive function pattern** (lines 25-84):
```rust
#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_calls_with_cache_stats(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    semantic_mir_output_digest: Digest,
    // ... upstream digests ...
) -> CallsProviderOutput {
    let mut sites = extract_call_sites(db);
    let targets = resolve_direct_call_targets(db, &sites);
    // ... derive, normalize, compute output digest, store ...
    let output = CallOutput { sites, targets, unresolved }.normalized();
    let output_digest = calls_output_digest(db, manifest, input_snapshot, ..., &output);
    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();
    match db.replace_call_facts(output) {
        Ok(()) => CallsProviderOutput { diagnostics: Vec::new(), cache_stats, output_digest: Some(output_digest) },
        Err(error) => CallsProviderOutput { diagnostics: vec![provider_error_diagnostic(error.to_string())], cache_stats, output_digest: Some(output_digest) },
    }
}
```

Pattern: extract -> normalize -> output_digest -> store -> return. The entrypoints provider does: run Go recognizers -> run TS/JS recognizers -> derive trust boundaries -> derive dispatch edges -> derive unresolved -> normalize -> output_digest -> store.

**Output digest pattern** (lines 87-178):
```rust
fn calls_output_digest(
    db: &AnalysisDb,
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    // ... upstream digests ...
    output: &CallOutput,
) -> Digest {
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!("parameters={}", calls_provider_parameter_digest()),
        format!("config={}", input_snapshot.config.digest),
        // ... upstream digests ...
    ];
    extend_component_parts(&mut parts, "go_lifecycle", &input_snapshot.go_lifecycle.components);
    extend_component_parts(&mut parts, "ts_js_lifecycle", &input_snapshot.ts_js_lifecycle.components);
    extend_component_parts(&mut parts, "model", &input_snapshot.models);
    extend_component_parts(&mut parts, "extension", &input_snapshot.extensions);
    extend_component_parts(&mut parts, "tool", &input_snapshot.tool_invocations);
    // ... output-specific parts ...
    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "calls_output", &refs)
}
```

Pattern: output digest includes provider metadata, upstream digests, lifecycle components, extension components, and per-fact stable payload lines. All parts are sorted before hashing. Uses `Digest::from_parts(DigestKind::ProviderOutput, ...)`.

**Provider error diagnostic pattern** (lines 294-301):
```rust
fn provider_error_diagnostic(message: String) -> Diagnostic {
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        TextRange::point(1, 1),
        format!("Calls provider failed: {message}"),
    )
}
```

**Test pattern** (lines 303-715): Tests verify deterministic digest, manifest declarations, output population with indexes, and cold/warm equality.

---

### `analysis/entrypoints/cache_key.rs` (config, transform)

**Analog:** `crates/polint/src/analysis/calls/cache_key.rs`

**Parameter digest pattern** (lines 3-22):
```rust
pub(crate) fn calls_provider_parameter_digest() -> Digest {
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "calls_provider_parameters",
        &[
            "calls-facts-1",
            "call_sites",
            "call_targets",
            "unresolved_calls",
            "direct_binding",
            // ... algorithm labels ...
        ],
    )
}
```

New: `entrypoints_provider_parameter_digest()` with schema label, output families, and recognizer labels.

**Test pattern** (lines 24-52): Pin-test that reproduces the exact same digest from the same inputs.

---

### `analysis/entrypoints/validate.rs` (middleware, transform)

**Analog:** `crates/polint/src/analysis/calls/validate.rs`

**Validation function pattern** (lines 8-286):
```rust
pub(crate) fn validate_calls(db: &AnalysisDb, diagnostics: &mut Vec<Diagnostic>) {
    // 1. Collect valid ID sets from db
    let files = db.files().iter().map(|row| row.id).collect::<BTreeSet<_>>();
    let functions = db.functions().iter().map(|row| row.id).collect::<BTreeSet<_>>();
    // ...
    
    // 2. Check duplicate stable keys
    check_duplicate_stable_keys(diagnostics, "CallSite", db.call_sites().iter().map(|row| row.stable_key.as_str()));

    // 3. Check dangling references per fact
    for site in db.call_sites() {
        check_ref(diagnostics, &files, site.file, "CallSite", &site.stable_key, "file", "dangling call file reference");
        // ...
        if site.span.start_byte > site.span.end_byte {
            push_call_diagnostic(diagnostics, "CallSite", &site.stable_key, "span", "invalid span byte range");
        }
    }

    // 4. Check precision ceilings via FactFamily metadata
    for family in [FactFamily::CallSite, FactFamily::CallTarget, FactFamily::UnresolvedCall] {
        for (reference, metadata) in db.fact_meta().rows() {
            if reference.family != family || metadata.producer_id != "polint.calls" { continue; }
            if metadata.precision == FactPrecision::Exact {
                push_call_diagnostic(diagnostics, family.label(), &metadata.stable_key, "precision", "precision ceiling exceeded: ...");
            }
        }
    }
}
```

Pattern: (1) collect valid IDs, (2) check duplicate stable keys, (3) check dangling refs per fact, (4) check precision ceilings. The entrypoints validator adds framework-specific checks: trust boundary entrypoint references exist, dispatch edge target references exist, conflicting entrypoint registrations for same handler, precision ceiling is never Exact.

**Diagnostic helper pattern** (lines 288-351):
```rust
fn check_duplicate_stable_keys<'a>(diagnostics: &mut Vec<Diagnostic>, family: &'static str, keys: impl Iterator<Item = &'a str>) { ... }

fn check_ref<T: Ord + Copy>(diagnostics: &mut Vec<Diagnostic>, valid: &BTreeSet<T>, value: T, family: &'static str, stable_key: &str, field: &'static str, reason: &'static str) { ... }

fn push_call_diagnostic(diagnostics: &mut Vec<Diagnostic>, family: &'static str, stable_key: &str, field: &'static str, reason: &'static str) {
    diagnostics.push(
        Diagnostic::error("polint/internal", "<workspace>", TextRange::point(1, 1), format!("Calls validation failed for {family} stable key."))
            .with_evidence("family", family)
            .with_evidence("stable_key", stable_key.to_string())
            .with_evidence("field", field)
            .with_evidence("reason", reason),
    );
}
```

Reuse `check_duplicate_stable_keys` and `check_ref` helpers. Replace diagnostic message prefix with "Entrypoints validation failed for {family} stable key."

---

### `analysis/entrypoints/extract.rs` (service, transform)

**Analog:** `crates/polint/src/analysis/calls/extract.rs`

**Extraction function pattern** (lines 14-108):
```rust
pub(crate) fn extract_call_sites(db: &AnalysisDb) -> Vec<CallSiteFact> {
    let bodies = db.mir_bodies().iter().map(|body| (body.id, body)).collect::<BTreeMap<_, _>>();
    let places = db.mir_places().iter().map(|place| (place.id, place)).collect::<BTreeMap<_, _>>();
    let functions = db.functions().iter().map(|function| (function.id, function)).collect::<BTreeMap<_, _>>();
    
    // ... filter, sort by stable keys for determinism ...
    
    // Build facts with stable keys using semantic_stable_key
    sites.push(CallSiteFact { ... stable_key: call_site_stable_key(db, body, operation, kind, &callee_shape, &operation_stable_key), });
    
    sites.sort_by(|left, right| (left.stable_key.as_str(), left.id).cmp(&(right.stable_key.as_str(), right.id)));
    sites
}
```

Pattern: build lookup maps from db, iterate over relevant facts, construct new facts with stable keys from `semantic_stable_key()`, sort by stable key at end. The entrypoints extractor dispatches to language-specific recognizer functions.

**Stable key generation pattern** (lines 404-425):
```rust
fn call_site_stable_key(db: &AnalysisDb, body: &MirBody, operation: &MirOperation, kind: CallSyntaxKind, callee_shape: &str, operation_stable_key: &str) -> String {
    semantic_stable_key(
        FactFamily::CallSite,
        &[
            ("language", format!("{:?}", body.language)),
            ("file_key", file_key(db, body.file)),
            ("caller_key", caller_key(db, body.function)),
            ("span", span_key(&operation.span)),
            ("callee_shape", callee_shape.to_string()),
            ("operation_key", operation_stable_key.to_string()),
            ("call_kind", format!("{kind:?}")),
        ],
    )
    .into_string()
}
```

Pattern: use `semantic_stable_key(FactFamily::Entrypoint, &[...])` with language, file_key, function binding key, framework_id, kind, and trigger metadata as parts.

---

### `analysis/entrypoints/unresolved.rs` (service, transform)

**Analog:** `crates/polint/src/analysis/calls/unresolved.rs`

**Derivation function pattern** (lines 13-42):
```rust
pub(crate) fn derive_unresolved_calls(db: &AnalysisDb, sites: &[CallSiteFact]) -> Vec<UnresolvedCallFact> {
    let mut rows = BTreeMap::new();
    for site in sites {
        if let Some(reason) = reason_for_site(site) {
            insert_unresolved(&mut rows, site, reason, "call-site-shape");
        }
    }
    // ... also check unsupported semantics ...
    rows.into_values().collect()
}
```

Pattern: derive unresolved facts from recognized but unresolvable patterns. For entrypoints: detect framework imports that cannot be resolved (unknown version, dynamic registration, unsupported framework). Use BTreeMap for dedup by stable key.

**Reason/status/precision mapping pattern** (lines 177-209):
```rust
fn status_for_reason(reason: UnresolvedCallReason) -> CallTargetStatus { ... }
fn precision_for_reason(reason: UnresolvedCallReason) -> CallPrecision { ... }
fn algorithm_for_reason(reason: UnresolvedCallReason) -> CallAlgorithm { ... }
```

Pattern: map each `UnresolvedFrameworkReason` to a status and precision value via exhaustive match.

---

### `analysis/entrypoints/mod.rs` (config, CRUD)

**Analog:** `crates/polint/src/analysis/calls/mod.rs`

**Module layout pattern** (lines 1-8):
```rust
pub(crate) mod cache_key;
pub(crate) mod direct;
pub(crate) mod extract;
pub(crate) mod facts;
pub(crate) mod provider;
pub(crate) mod store;
pub(crate) mod unresolved;
pub(crate) mod validate;
```

Pattern: all submodules are `pub(crate)`. The entrypoints module adds recognizer submodules (e.g., `go_recognizer`, `ts_recognizer` or `recognizers/go.rs`, `recognizers/ts.rs`).

---

### `analysis/ids.rs` (modified, model)

**Analog:** `crates/polint/src/analysis/ids.rs`

**Dense ID pattern** (lines 4-46):
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct MirBodyId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct CallSiteId(pub(crate) u64);
```

Add four new IDs following the exact same derive set:
- `EntrypointId(pub(crate) u64)`
- `TrustBoundaryId(pub(crate) u64)`
- `DispatchEdgeId(pub(crate) u64)`
- `UnresolvedFrameworkId(pub(crate) u64)`

---

### `analysis_kernel/metadata.rs` (modified, model)

**Analog:** `crates/polint/src/analysis_kernel/metadata.rs`

**FactFamily enum extension pattern** (lines 6-75):
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum FactFamily {
    // ... existing variants ...
    CallSite,
    CallTarget,
    UnresolvedCall,
    // ...
}
```

Add four new variants after `ExtensionFact`:
- `Entrypoint`
- `TrustBoundary`
- `DispatchEdge`
- `UnresolvedFramework`

**Label method extension** (lines 77-142):
```rust
impl FactFamily {
    pub(crate) fn label(self) -> &'static str {
        match self {
            // ...
            Self::CallSite => "CallSite",
            Self::CallTarget => "CallTarget",
            Self::UnresolvedCall => "UnresolvedCall",
            // ...
        }
    }
}
```

Add labels: `"Entrypoint"`, `"TrustBoundary"`, `"DispatchEdge"`, `"UnresolvedFramework"`.

---

### `analysis_kernel/provider.rs` (modified, config)

**Analog:** `crates/polint/src/analysis_kernel/provider.rs`

**Provider manifest insertion pattern** (lines 205-478):

Insert a new `ProviderManifest` entry for `"polint.entrypoints"` after `"polint.calls"` (position 9) and before `"polint.abstract_domains"` (current position 9, shifted to 10). The manifest follows the established pattern:

```rust
ProviderManifest {
    id: "polint.calls",
    kind: ProviderKind::WholeRepoDerived,
    inputs: &["source_files", "functions", "symbols", "references", ...],
    outputs: &["call_sites", "call_targets", "unresolved_calls"],
    language_scope: LanguageScope::MultiLanguage,
    cache_policy: CachePolicy::InMemoryDerived,
    schema_versions: CALLS_SCHEMA,
    precision_ceiling: PrecisionCeiling::SetupAware,
},
```

New entry uses `ProviderKind::WholeRepoDerived`, `LanguageScope::MultiLanguage`, `CachePolicy::InMemoryDerived`, `PrecisionCeiling::SetupAware`. Inputs include call-related outputs. Outputs: `["entrypoints", "trust_boundaries", "dispatch_edges", "unresolved_framework"]`.

A new schema constant follows the pattern (lines 180-183):
```rust
const CALLS_SCHEMA: &[SchemaVersion] = &[SchemaVersion {
    name: "calls-facts-1",
    version: 1,
}];
```

Add `ENTRYPOINTS_SCHEMA` with `name: "entrypoints-facts-1"`, `version: 1`.

**Provider order test pattern** (lines 502-519): The `provider_order_matches_behavior_preserving_kernel_sequence` test must be updated to include `"polint.entrypoints"` in the expected order.

---

### `analysis_kernel/validation.rs` (modified, middleware)

**Analog:** `crates/polint/src/analysis_kernel/validation.rs`

**Validation dispatch pattern** (lines 6-55):
```rust
use crate::analysis::calls::validate::validate_calls;

pub(crate) fn validate_fact_metadata(db: &AnalysisDb, manifests: &[ProviderManifest]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    // ...
    validate_calls(db, &mut diagnostics);
    // ...
    diagnostics.sort_by(diagnostic_order);
    diagnostics
}
```

Add `use crate::analysis::entrypoints::validate::validate_entrypoints;` and call `validate_entrypoints(db, &mut diagnostics);` after `validate_calls`.

---

### `analysis/mod.rs` (modified, config)

**Analog:** `crates/polint/src/analysis/mod.rs`

**Module declaration pattern** (lines 1-24):
```rust
pub(crate) mod calls;
pub(crate) mod cfg;
// ...
```

Add `pub(crate) mod entrypoints;` after `pub(crate) mod domains;` (alphabetical or logical ordering).

---

### `analysis/extensions/sinks.rs` (modified, model)

**Analog:** `crates/polint/src/analysis/extensions/sinks.rs`

No structural change needed to the sink types. Extension-emitted entrypoint/trust-boundary/dispatch-edge/unresolved-framework facts use `ExtensionFactCandidate` with `fact_family` set to `"entrypoint"`, `"trust_boundary"`, `"dispatch_edge"`, or `"unresolved_framework"`. The existing `ExtensionFactPrecision` enum (lines 25-32) already covers the needed precision tiers:

```rust
pub(crate) enum ExtensionFactPrecision {
    Exact,
    SetupAware,
    Heuristic,
    GeneratedUnvalidated,
}
```

Extension merge logic in `analysis/extensions/validate.rs` may need awareness of framework fact families to apply framework-specific validation (precision ceiling: no Exact for framework facts).

---

## Shared Patterns

### Stable Key Generation
**Source:** `crates/polint/src/analysis/stable_key.rs` lines 1-18
**Apply to:** All entrypoint fact construction (facts.rs, extract.rs, unresolved.rs)
```rust
use crate::analysis_kernel::{FactFamily, stable_key_from_parts};

pub(crate) fn semantic_stable_key(family: FactFamily, parts: &[(&str, String)]) -> StableFactKey {
    StableFactKey(stable_key_from_parts(family, parts))
}
```

### Dense ID Declaration
**Source:** `crates/polint/src/analysis/ids.rs` lines 4-7
**Apply to:** `ids.rs` modification for new entrypoint IDs
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct MirBodyId(pub(crate) u64);
```

### Diagnostic Error Pattern
**Source:** `crates/polint/src/analysis/calls/validate.rs` lines 332-351
**Apply to:** `entrypoints/validate.rs`
```rust
fn push_call_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    family: &'static str,
    stable_key: &str,
    field: &'static str,
    reason: &'static str,
) {
    diagnostics.push(
        Diagnostic::error("polint/internal", "<workspace>", TextRange::point(1, 1),
            format!("Calls validation failed for {family} stable key."))
            .with_evidence("family", family)
            .with_evidence("stable_key", stable_key.to_string())
            .with_evidence("field", field)
            .with_evidence("reason", reason),
    );
}
```

### Output Normalization Pattern
**Source:** `crates/polint/src/analysis/calls/store.rs` lines 22-48
**Apply to:** `entrypoints/store.rs`
```rust
pub(crate) fn normalized(mut self) -> Self {
    self.sites.sort_by(|left, right| {
        (left.stable_key.as_str(), left.id).cmp(&(right.stable_key.as_str(), right.id))
    });
    // repeat for each fact vector
    self
}
```

### Cache Digest Pattern
**Source:** `crates/polint/src/analysis/calls/provider.rs` lines 87-178
**Apply to:** `entrypoints/provider.rs` and `entrypoints/cache_key.rs`

Key elements:
1. Include `provider_id`, `provider_version()`, `primary_schema_label()`, parameter digest, config digest
2. Include all upstream output digests
3. Include lifecycle components via `extend_component_parts`
4. Include per-fact stable payload lines (sorted)
5. Sort all parts before hashing
6. Use `Digest::from_parts(DigestKind::ProviderOutput, "entrypoints_output", &refs)`

### Extension Overlay Merge Pattern
**Source:** `crates/polint/src/analysis/extensions/store.rs` lines 51-101
**Apply to:** Extension integration in entrypoints provider

Extension facts use `ExtensionOutput.normalized()` for deterministic ordering. Accepted extension facts carry `payload_digest`. The entrypoints provider should merge extension-emitted framework facts after native facts using additive set union by stable key, keeping native facts on conflict.

### AnalysisError Pattern
**Source:** `crates/polint/src/analysis/calls/store.rs` lines 78-83
**Apply to:** `entrypoints/store.rs`
```rust
return Err(AnalysisError::InvalidFact {
    provider: "polint.calls",
    reason: format!("dangling call site {:?} for target `{}`", target.site, target.stable_key),
});
```

Use `provider: "polint.entrypoints"` for framework fact validation errors.

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| Go recognizer logic (inside `extract.rs` or separate module) | service | transform | No existing framework-aware Go recognizers; pattern detection for `http.HandleFunc`, `chi.Get/Post`, `cobra.Command`, `Test*` naming is new logic. Uses call sites + imports + function facts as inputs. |
| TS/JS recognizer logic (inside `extract.rs` or separate module) | service | transform | No existing framework-aware TS/JS recognizers; pattern detection for Express `app.get/post`, MCP `server.tool/resource/prompt`, `describe/it/test` is new logic. Uses call sites + imports + function facts as inputs. |

For recognizer implementation, use the `extract.rs` pattern of consuming `AnalysisDb` and building BTreeMap indexes, then matching known framework patterns against call sites and import facts. The closest partial analog for pattern matching against imports is in `analysis/calls/direct.rs` lines 17-54, which shows how to resolve callees by looking up references and symbols.

---

## Metadata

**Analog search scope:** `crates/polint/src/analysis/calls/`, `crates/polint/src/analysis/extensions/`, `crates/polint/src/analysis_kernel/`, `crates/polint/src/analysis/`
**Files scanned:** 20+ source files across analysis and analysis_kernel modules
**Pattern extraction date:** 2026-05-23

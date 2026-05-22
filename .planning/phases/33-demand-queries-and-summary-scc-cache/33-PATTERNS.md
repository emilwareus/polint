# Phase 33: Demand Queries and Summary SCC Cache - Pattern Map

**Mapped:** 2026-05-22
**Files analyzed:** 14 new/modified files
**Analogs found:** 14 / 14

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/polint/src/analysis_kernel/incremental/keys.rs` | config | transform | self (activate `direct_summaries_layer_key`) | exact |
| `crates/polint/src/analysis_kernel/incremental/layer_cache.rs` | service | CRUD | self (extend for query cache entries) | exact |
| `crates/polint/src/analysis_kernel/incremental/invalidation.rs` | service | transform | self (extend quarantine logic) | exact |
| `crates/polint/src/analysis_kernel/incremental/dependency_index.rs` | service | transform | self (Query node already exists) | exact |
| `crates/polint/src/analysis_kernel/incremental/mod.rs` | config | re-export | self (add new module re-exports) | exact |
| `crates/polint/src/analysis/summaries/provider.rs` | service | request-response | `crates/polint/src/metrics.rs` | exact |
| `crates/polint/src/analysis/summaries/store.rs` | store | CRUD | self (add SCC-aware lookup) | exact |
| `crates/polint/src/analysis/summaries/scc.rs` (NEW) | service | batch/transform | `crates/polint/src/graph/mod.rs` + `crates/polint/src/analysis/calls/store.rs` | role-match |
| `crates/polint/src/analysis/summaries/demand.rs` (NEW) | service | request-response | `crates/polint/src/analysis_kernel/incremental/layer_cache.rs` | role-match |
| `crates/polint/src/analysis/summaries/validate.rs` | utility | transform | self (extend for demand query validation) | exact |
| `crates/polint/src/analysis_kernel/provider.rs` | config | CRUD | self (no structural change, schema ref only) | exact |
| `crates/polint/src/analysis_kernel/mod.rs` | controller | request-response | self (wire demand queries after direct_summaries) | exact |
| `crates/polint/src/analysis_kernel/validation.rs` | utility | transform | self (add demand query validation call) | exact |
| `crates/polint/src/analysis_kernel/incremental/run_report.rs` | model | transform | self (extend with demand query trace) | exact |

## Pattern Assignments

### `crates/polint/src/analysis_kernel/incremental/keys.rs` (config, activate reserved function)

**Analog:** Self -- the `direct_summaries_layer_key()` function is already fully implemented at lines 675-747. Phase 33 removes the `#[expect(dead_code)]` attribute and wires it into the summaries provider.

**Dead code attribute to remove** (lines 676-680):
```rust
    #[expect(dead_code, reason = "reserved for Phase 33 persistent layer cache")]
    #[expect(
        clippy::too_many_arguments,
        reason = "Direct summaries layer cache identity is intentionally explicit so every upstream digest input remains visible."
    )]
```

**Function signature** (lines 681-695):
```rust
    pub(crate) fn direct_summaries_layer_key(
        manifest: &ProviderManifest,
        source_function_digests: Vec<Digest>,
        config_digest: Digest,
        go_lifecycle_digest: Digest,
        ts_js_lifecycle_digest: Digest,
        upstream_syntax_output_digests: Vec<Digest>,
        semantic_mir_output_digest: Digest,
        cfg_output_digest: Digest,
        calls_output_digest: Digest,
        abstract_domains_output_digest: Digest,
        symbol_graph_output_digest: Digest,
        module_topology_output_digest: Digest,
        direct_summaries_parameter_digest: Digest,
    ) -> Self {
```

**LayerKind::DirectSummaries variant** (line 53):
```rust
    DirectSummaries,
```

**Existing `QueryKey` struct** (lines 82-89) -- demand queries will use this:
```rust
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct QueryKey {
    pub(crate) query_kind: String,
    pub(crate) query_version: String,
    pub(crate) parameter_digest: Digest,
    pub(crate) layer_digests: Vec<Digest>,
    pub(crate) budget_digest: Digest,
    pub(crate) precision_tier: PrecisionTier,
}
```

**Existing `SummaryKey` struct** (lines 91-99) -- SCC cache will use this:
```rust
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct SummaryKey {
    pub(crate) callable_stable_key: String,
    pub(crate) summary_domain: String,
    pub(crate) summary_version: String,
    pub(crate) body_shape_digest: Digest,
    pub(crate) dependency_summary_digests: Vec<Digest>,
    pub(crate) extension_digest: Digest,
}
```

---

### `crates/polint/src/analysis/summaries/provider.rs` (service, layer cache activation)

**Analog:** `crates/polint/src/metrics.rs` lines 1-121 -- the metrics provider is the best pattern for how to wire layer cache read/write into a provider.

**Imports pattern** (metrics.rs lines 1-15):
```rust
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{
    CacheNode, CacheStats, DependencyEdge, DependencyKind, Digest, DigestKind, InputSnapshot,
    LayerCacheManifest, LayerCacheReadStatus, LayerCacheStore, LayerCacheWriteStatus, LayerKey,
    LayerKind, PrecisionTier, ShapeKind, dependency_layer_digest,
};
use crate::cache::Cache;
use crate::core::AnalysisDb;
use crate::diagnostics::{Diagnostic, TextRange};
use serde::{Deserialize, Serialize};
```

**Layer cache read pattern** (metrics.rs lines 58-70):
```rust
    let layer_key = metrics_layer_key(
        db,
        manifest,
        config_digest.clone(),
        upstream_syntax_output_digests.clone(),
    );
    let store = cache.layer_cache_store();
    let mut cache_stats = CacheStats::default();
    let read = store
        .read_json_validated::<MetricsLayerPayload, _>(&layer_key, |payload, manifest| {
            validate_metrics_layer_payload(payload, manifest)
        });
```

**Cache hit restore pattern** (metrics.rs lines 72-85):
```rust
    match read.status {
        LayerCacheReadStatus::Hit => {
            cache_stats.record_hit();
            cache_stats.record_verified_reuse();
            let payload = read
                .value
                .expect("layer cache hit should include metrics payload");
            restore_metrics_layer_payload(db, &payload);
            MetricsDerivation {
                cache_stats,
                diagnostics: Vec::new(),
                output_digest: read.output_digest,
            }
        }
```

**Cache miss recompute + write pattern** (metrics.rs lines 93-120):
```rust
        LayerCacheReadStatus::Miss | LayerCacheReadStatus::InvalidEvicted => {
            if read.status == LayerCacheReadStatus::Miss {
                cache_stats.record_miss();
            } else {
                cache_stats.record_invalid_evicted_read();
            }
            cache_stats.record_recompute();
            let mut derivation = derive_requested_metrics_uncached(db, plan);
            let payload = metrics_layer_payload(db);
            let dependencies = metrics_layer_dependency_edges(
                db,
                &layer_key,
                manifest,
                &upstream_syntax_output_digests,
                config_digest,
            );
            derivation.output_digest = write_metrics_layer_payload(
                &store,
                layer_key,
                &payload,
                dependencies,
                &mut cache_stats,
                &mut derivation.diagnostics,
            );
            derivation.cache_stats = cache_stats;
            derivation
        }
```

**Write helper pattern** (metrics.rs lines 379-416):
```rust
fn write_metrics_layer_payload(
    store: &LayerCacheStore,
    layer_key: LayerKey,
    payload: &MetricsLayerPayload,
    dependencies: Vec<DependencyEdge>,
    stats: &mut CacheStats,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Digest> {
    let payload_digest = match LayerCacheStore::payload_digest_for_json(payload) {
        Ok(digest) => digest,
        Err(error) => {
            diagnostics.push(cache_write_diagnostic("metrics layer", error));
            return None;
        }
    };
    let output_digest = metrics_output_digest(&layer_key, &payload_digest);
    let manifest = LayerCacheManifest::new(
        layer_key,
        output_digest.clone(),
        payload_digest,
        dependencies,
        PrecisionTier::Syntax,
        "native_trusted",
        Vec::new(),
    );
    match store.write_json(&manifest, payload) {
        Ok(LayerCacheWriteStatus::Written) => {
            stats.record_write();
            Some(output_digest)
        }
        Ok(LayerCacheWriteStatus::BypassedDisabled) => {
            stats.record_disabled_bypass();
            Some(output_digest)
        }
        Err(error) => {
            diagnostics.push(cache_write_diagnostic("metrics layer", error));
            None
        }
    }
}
```

**Existing provider function to extend** (summaries/provider.rs lines 20-56):
```rust
#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_direct_summaries_with_cache_stats(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    semantic_mir_output_digest: Digest,
    cfg_output_digest: Digest,
    calls_output_digest: Digest,
    abstract_domains_output_digest: Digest,
    symbol_graph_output_digest: Digest,
    module_topology_output_digest: Digest,
    upstream_syntax_output_digests: Vec<Digest>,
) -> DirectSummariesProviderOutput {
    let output = DirectSummaryBuilder::build(db);
    // ... compute output digest, record stats, replace summary facts ...
}
```

---

### `crates/polint/src/analysis/summaries/scc.rs` (NEW -- SCC discovery and scheduling)

**Analog 1 -- petgraph usage:** `crates/polint/src/graph/mod.rs` lines 1-55

**Imports pattern** (graph/mod.rs lines 1-4):
```rust
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::BTreeMap;
```

**Graph construction pattern** (graph/mod.rs lines 29-47):
```rust
    pub(crate) fn from_db(db: &AnalysisDb) -> Self {
        let mut graph = DiGraph::<String, ()>::new();
        let mut nodes: BTreeMap<&str, NodeIndex> = BTreeMap::new();

        for file in db.files() {
            ensure_node(&mut nodes, &mut graph, file.relative_path.as_str());
        }

        for import in db.imports() {
            let from_path = db
                .file(import.file)
                .map_or("<unknown>", |file| file.relative_path.as_str());
            let from_idx = ensure_node(&mut nodes, &mut graph, from_path);
            let to_idx = ensure_node(&mut nodes, &mut graph, import.path.as_str());
            graph.add_edge(from_idx, to_idx, ());
        }

        Self { graph }
    }
```

**Analog 2 -- call target data for graph edges:** `crates/polint/src/analysis/calls/store.rs` lines 52-62 and 164-195

**CallStore index pattern** (calls/store.rs lines 52-62):
```rust
#[derive(Debug, Clone, Default)]
pub(crate) struct CallStore {
    output: CallOutput,
    sites_by_caller: BTreeMap<FunctionId, Vec<usize>>,
    targets_by_site: BTreeMap<CallSiteId, Vec<usize>>,
    outgoing_by_function: BTreeMap<FunctionId, Vec<usize>>,
    outgoing_by_symbol: BTreeMap<SymbolId, Vec<usize>>,
    incoming_by_symbol: BTreeMap<SymbolId, Vec<usize>>,
    incoming_by_function: BTreeMap<FunctionId, Vec<usize>>,
    unresolved_by_reason: BTreeMap<UnresolvedCallReason, Vec<usize>>,
    unresolved_by_status: BTreeMap<CallTargetStatus, Vec<usize>>,
}
```

**Call target accessor pattern** (calls/store.rs lines 184-195):
```rust
    pub(crate) fn outgoing_by_function(&self, caller: FunctionId) -> Vec<&CallTargetFact> {
        self.target_refs(self.outgoing_by_function.get(&caller))
    }

    pub(crate) fn incoming_by_function(&self, target_function: FunctionId) -> Vec<&CallTargetFact> {
        self.target_refs(self.incoming_by_function.get(&target_function))
    }
```

**SCC computation pattern** -- use `petgraph::algo::tarjan_scc`:
```rust
// Expected pattern for SCC module:
use petgraph::algo::tarjan_scc;
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::BTreeMap;

pub(crate) struct SccSchedule {
    pub(crate) sccs: Vec<Scc>,  // in reverse topological order (leaf callees first)
    pub(crate) total_functions: usize,
}

pub(crate) struct Scc {
    pub(crate) members: Vec<FunctionId>,  // sorted for determinism
    pub(crate) is_recursive: bool,
}
```

---

### `crates/polint/src/analysis/summaries/demand.rs` (NEW -- demand query memoization and cache)

**Analog:** `crates/polint/src/analysis_kernel/incremental/layer_cache.rs` lines 84-98 and 226-246

**Cache read outcome pattern** (layer_cache.rs lines 84-98):
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LayerCacheReadStatus {
    Hit,
    Miss,
    InvalidEvicted,
    BypassedDisabled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LayerCacheReadOutcome<T> {
    pub(crate) status: LayerCacheReadStatus,
    pub(crate) manifest: Option<LayerCacheManifest>,
    pub(crate) output_digest: Option<Digest>,
    pub(crate) payload_digest: Option<Digest>,
    pub(crate) value: Option<T>,
}
```

**LayerCacheManifest construction pattern** (layer_cache.rs lines 46-73):
```rust
impl LayerCacheManifest {
    pub(crate) fn new(
        key: LayerKey,
        output_digest: Digest,
        payload_digest: Digest,
        mut dependencies: Vec<DependencyEdge>,
        precision: PrecisionTier,
        validation: impl Into<String>,
        mut warnings: Vec<String>,
    ) -> Self {
        dependencies.sort();
        dependencies.dedup();
        warnings.sort();
        warnings.dedup();
        // ...
    }
}
```

**QueryKey construction pattern** (keys.rs lines 82-89):
```rust
pub(crate) struct QueryKey {
    pub(crate) query_kind: String,
    pub(crate) query_version: String,
    pub(crate) parameter_digest: Digest,
    pub(crate) layer_digests: Vec<Digest>,
    pub(crate) budget_digest: Digest,
    pub(crate) precision_tier: PrecisionTier,
}
```

---

### `crates/polint/src/analysis_kernel/incremental/invalidation.rs` (service, quarantine extension)

**Analog:** Self -- existing quarantine infrastructure.

**QuarantineReason enum** (lines 52-55):
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum QuarantineReason {
    ExtensionChanged,
}
```

**Extension quarantine action** (lines 248-250):
```rust
        ChangeKind::ExtensionCode | ChangeKind::ExtensionDeclaredInput => Some(
            InvalidationAction::Quarantine(node.clone(), QuarantineReason::ExtensionChanged),
        ),
```

**Node digest containment for Query/Summary** (lines 286-295):
```rust
fn query_key_contains_digest(key: &QueryKey, digest: &Digest) -> bool {
    key.parameter_digest == *digest
        || key.budget_digest == *digest
        || key.layer_digests.contains(digest)
}

fn summary_key_contains_digest(key: &SummaryKey, digest: &Digest) -> bool {
    key.body_shape_digest == *digest
        || key.extension_digest == *digest
        || key.dependency_summary_digests.contains(digest)
}
```

---

### `crates/polint/src/analysis_kernel/incremental/dependency_index.rs` (service, Query node support)

**Analog:** Self -- `CacheNode::Query` variant already exists.

**CacheNode enum** (lines 17-27):
```rust
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CacheNode {
    Input(String),
    Layer(LayerKey),
    Query(QueryKey),
    Summary(SummaryKey),
    Diagnostic(DiagnosticKey),
    Extension(String),
    ToolInvocation(String),
}
```

**DependencyEdge struct** (lines 68-74):
```rust
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct DependencyEdge {
    pub(crate) from: CacheNode,
    pub(crate) to: CacheNode,
    pub(crate) kind: DependencyKind,
    pub(crate) required_shape: ShapeKind,
}
```

---

### `crates/polint/src/analysis_kernel/mod.rs` (controller, wire demand queries)

**Analog:** Self -- the existing direct_summaries wiring block (lines 308-332).

**Direct summaries invocation pattern** (lines 308-332):
```rust
        let direct_summaries =
            crate::analysis::summaries::provider::derive_direct_summaries_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.direct_summaries"),
                semantic_mir_dependency_output_digest,
                cfg_dependency_output_digest,
                calls_dependency_output_digest,
                abstract_domains_dependency_output_digest,
                symbol_dependency_output_digest,
                module_topology_dependency_output_digest,
                vec![
                    go_dependency_output_digest.clone(),
                    ts_dependency_output_digest.clone(),
                ],
            );
        let polint_direct_summaries_cache_stats = direct_summaries.cache_stats.clone();
        let _direct_summaries_output_digest = direct_summaries.output_digest;
        diagnostics.extend(direct_summaries.diagnostics);
        provider_outputs.push(Self::provider_output_for_with_optional_digest(
            "polint.direct_summaries",
            &db,
            polint_direct_summaries_cache_stats,
            _direct_summaries_output_digest,
        ));
```

Note: The `_direct_summaries_output_digest` is currently discarded (leading underscore). Phase 33 activates it by passing it through the layer cache and using it for demand query input digests.

---

### `crates/polint/src/analysis/summaries/validate.rs` (utility, extend validation)

**Analog:** Self -- existing validation pattern.

**Validation function signature** (lines 9-10):
```rust
pub(crate) fn validate_summaries(db: &AnalysisDb, diagnostics: &mut Vec<Diagnostic>) {
```

**Per-fact validation pattern** (lines 56-58):
```rust
    for fact in db.summary_facts() {
        validate_summary_fact(db, diagnostics, &functions, fact);
    }
```

**Metadata check pattern** (lines 203-241):
```rust
fn check_metadata(
    db: &AnalysisDb,
    diagnostics: &mut Vec<Diagnostic>,
    family: FactFamily,
    run_id: u64,
    family_label: &'static str,
    stable_key: &str,
) {
    let Some(metadata) = db.metadata_for(FactRef::new(family, run_id)) else {
        push_summary_diagnostic(diagnostics, family_label, stable_key, "metadata", "required metadata is missing");
        return;
    };
    // ... further checks ...
}
```

---

### `crates/polint/src/analysis_kernel/incremental/run_report.rs` (model, demand query trace)

**Analog:** Self -- existing `KernelRunReport` and `ProviderOutputMeta`.

**Report construction** (lines 4-9):
```rust
pub(crate) struct KernelRunReport {
    pub(crate) input_snapshot: InputSnapshot,
    pub(crate) provider_outputs: Vec<ProviderOutputMeta>,
    pub(crate) cache_stats: CacheStats,
}
```

**Aggregate cache stats** (lines 101-114):
```rust
fn aggregate_cache_stats(provider_outputs: &[ProviderOutputMeta]) -> CacheStats {
    let mut aggregate = CacheStats::default();
    for output in provider_outputs {
        aggregate.hits += output.cache_stats.hits;
        aggregate.misses += output.cache_stats.misses;
        aggregate.recomputes += output.cache_stats.recomputes;
        aggregate.writes += output.cache_stats.writes;
        aggregate.bypasses_disabled += output.cache_stats.bypasses_disabled;
        aggregate.invalid_evicted_reads += output.cache_stats.invalid_evicted_reads;
        aggregate.verified_reuse += output.cache_stats.verified_reuse;
        aggregate.quarantines += output.cache_stats.quarantines;
    }
    aggregate
}
```

---

### `crates/polint/src/analysis/summaries/store.rs` (store, SCC-aware lookup)

**Analog:** Self -- existing `SummaryStore` pattern.

**Store index pattern** (lines 37-42):
```rust
#[derive(Debug, Clone, Default)]
pub(crate) struct SummaryStore {
    output: SummaryOutput,
    summaries_by_callable: BTreeMap<String, Vec<usize>>,
    summaries_by_domain: BTreeMap<SummaryDomainKind, Vec<usize>>,
    summaries_by_function: BTreeMap<FunctionId, Vec<usize>>,
}
```

**Query accessor pattern** (lines 74-84):
```rust
    pub(crate) fn summaries_by_callable(&self, callable_key: &str) -> Vec<&SummaryFact> {
        self.summary_refs(self.summaries_by_callable.get(callable_key))
    }

    pub(crate) fn summaries_by_function(&self, function: FunctionId) -> Vec<&SummaryFact> {
        self.summary_refs(self.summaries_by_function.get(&function))
    }
```

---

### `crates/polint/src/analysis/summaries/cache_key.rs` (config, provider parameter digest)

**Analog:** Self -- existing digest construction.

**Parameter digest pattern** (lines 7-28):
```rust
pub(crate) fn direct_summaries_provider_parameter_digest() -> Digest {
    let parts = [
        format!("schema={DIRECT_SUMMARIES_SCHEMA_LABEL}:1"),
        format!("domain={}:{}", ControlEffects::ID, ControlEffects::VERSION),
        // ... more domain version entries ...
    ];
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "direct_summaries_parameters",
        &refs,
    )
}
```

---

### `crates/polint/src/analysis_kernel/validation.rs` (utility, wire new validation)

**Analog:** Self -- existing validation orchestration.

**Validation orchestration** (lines 28-54):
```rust
pub(crate) fn validate_fact_metadata(
    db: &AnalysisDb,
    manifests: &[ProviderManifest],
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    // ... existing validators ...
    validate_summaries(db, &mut diagnostics);
    validate_metadata_providers(db, &manifests_by_id, &mut diagnostics);
    validate_precision_ceilings(db, &manifests_by_id, &mut diagnostics);
    diagnostics.sort_by(diagnostic_order);
    diagnostics
}
```

---

### `crates/polint/src/analysis/summaries/domain.rs` (model, summary domain traits)

**Analog:** Self -- existing domain algebra.

**`Changed` enum for backdating** (lines 1-13):
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Changed {
    Yes,
    No,
}

impl Changed {
    pub(crate) fn is_yes(self) -> bool {
        matches!(self, Self::Yes)
    }
}
```

**`SummaryTopReason` for budget exceeded** (lines 33-56):
```rust
pub(crate) enum SummaryTopReason {
    UnresolvedCallee,
    UnsupportedSemantic,
    DynamicWrite,
    SetupMissing,
    BudgetExceeded,     // <-- already defined for SCC iteration budget
    MissingDependency,
    ConflictingFacts,
}
```

**SummaryDomain trait** (lines 58-60):
```rust
pub(crate) trait SummaryDomain: Clone + Send + Sync + Eq + 'static {
    const ID: &'static str;
    const VERSION: u32;
```

---

## Shared Patterns

### Digest Construction
**Source:** `crates/polint/src/analysis_kernel/incremental/digest.rs`
**Apply to:** All new files that produce cache keys or output digests.
```rust
// Ordered digest from parts
Digest::from_parts(DigestKind::ProviderOutput, "label", &refs)

// Unordered digest (order-independent)
Digest::from_unordered(DigestKind::ProviderParameters, "label", vec![...])

// Absent sentinel for future/unused slots
Digest::absent(DigestKind::ExtensionCode, "extension_digest_absent")
```

### Layer Cache Identity Pattern
**Source:** `crates/polint/src/analysis_kernel/incremental/keys.rs` (all `*_layer_key` functions)
**Apply to:** Direct summaries layer key activation, SCC-level cache keys.
```rust
// Every layer key includes:
// 1. provider_id, provider_version, schema_version
// 2. parameter_digest (domain versions, algorithm config)
// 3. lifecycle_digest (go/ts lifecycle merged)
// 4. config_digest
// 5. toolchain_digest (absent if not applicable)
// 6. input_digests (source texts, lifecycle inputs, parameter inputs)
// 7. dependency_layer_digests (all upstream provider output digests)
// 8. extension_digests (absent sentinel slots for future extensions)
```

### Crate-Private Visibility
**Source:** `crates/polint/src/analysis_kernel/incremental/mod.rs`
**Apply to:** All new types and functions.
```rust
// All new structs, enums, functions use pub(crate) visibility
pub(crate) struct DemandQueryResult { ... }
pub(crate) fn compute_scc_schedule(...) -> SccSchedule { ... }

// Dead code attributes for reserved infrastructure
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "Reserved for Phase XX ...")
)]
```

### Deterministic Normalization
**Source:** `crates/polint/src/analysis/summaries/store.rs` lines 19-33
**Apply to:** All new output types (SCC schedule, demand query results).
```rust
impl SummaryOutput {
    pub(crate) fn normalized(mut self) -> Self {
        self.summaries.sort_by(|left, right| {
            (left.stable_key.as_str(), left.id).cmp(&(right.stable_key.as_str(), right.id))
        });
        // ... reassign IDs sequentially ...
        self
    }
}
```

### Test Construction Pattern
**Source:** `crates/polint/src/analysis_kernel/incremental/invalidation.rs` lines 306-353
**Apply to:** All new test modules.
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::incremental::{
        CacheNode, Digest, DigestKind, LayerKey, PrecisionTier,
    };

    fn digest(kind: DigestKind, label: &str) -> Digest {
        Digest::from_parts(kind, label, &[label])
    }

    fn layer(provider_id: &str, input_label: &str) -> LayerKey {
        LayerKey::new(
            crate::analysis_kernel::incremental::keys::LayerKind::TsSyntax,
            provider_id,
            "1",
            "ts-facts-v1",
            Digest::absent(DigestKind::ProviderParameters, "none"),
            // ... all other absent digests ...
            Vec::new(),
        )
    }
}
```

### Provider Output Wiring in Kernel
**Source:** `crates/polint/src/analysis_kernel/mod.rs` lines 308-332
**Apply to:** SCC computation step wired after direct_summaries.
```rust
// Pattern: call provider, capture stats + digest, push to provider_outputs
let result = crate::analysis::module::derive_with_cache_stats(
    &mut db,
    &input_snapshot,
    Self::provider_manifest("polint.X"),
    // ... dependency digests ...
);
let cache_stats = result.cache_stats.clone();
let output_digest = result.output_digest;
diagnostics.extend(result.diagnostics);
provider_outputs.push(Self::provider_output_for_with_optional_digest(
    "polint.X",
    &db,
    cache_stats,
    output_digest,
));
```

### Debug JSON Pattern
**Source:** `crates/polint/src/analysis_kernel/debug.rs` (crate-private test-only debug module)
**Apply to:** Demand query trace and SCC scheduling debug output.
```rust
// The debug module is gated with #[cfg(test)] at the module declaration
// Debug JSON is constructed with serde_json::json!({...}) and returned through
// test-only accessor methods on AnalysisKernel.
```

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| (none) | -- | -- | All files have strong analogs in the existing codebase. |

All 14 files either modify existing code (with self-analog) or follow established patterns from metrics layer cache, graph module, or incremental infrastructure.

## Metadata

**Analog search scope:** `crates/polint/src/` (analysis_kernel/, analysis/, metrics.rs, graph/)
**Files scanned:** ~30 source files examined for pattern extraction
**Pattern extraction date:** 2026-05-22

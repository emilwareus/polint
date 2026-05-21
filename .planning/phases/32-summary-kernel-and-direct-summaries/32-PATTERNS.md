# Phase 32: Summary Kernel and Direct Summaries - Pattern Map

**Mapped:** 2026-05-21
**Files analyzed:** 16 new/modified files
**Analogs found:** 16 / 16

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `analysis/summaries/mod.rs` | module-root | N/A | `analysis/domains/mod.rs` | exact |
| `analysis/summaries/facts.rs` | model | CRUD | `analysis/domains/facts.rs` | exact |
| `analysis/summaries/domain.rs` | model (trait) | transform | `analysis/domains/lattice.rs` | exact |
| `analysis/summaries/core.rs` | model (domain impls) | transform | `analysis/domains/core.rs` | exact |
| `analysis/summaries/store.rs` | store | CRUD | `analysis/calls/store.rs` | exact |
| `analysis/summaries/builder.rs` | service | transform | `analysis/domains/solver.rs` + `analysis/calls/extract.rs` | role-match |
| `analysis/summaries/provider.rs` | provider | request-response | `analysis/domains/provider.rs` | exact |
| `analysis/summaries/validate.rs` | middleware | transform | `analysis/domains/validate.rs` | exact |
| `analysis/summaries/cache_key.rs` | utility | transform | `analysis/domains/cache_key.rs` | exact |
| `analysis/mod.rs` | module-root (modify) | N/A | self | exact |
| `analysis/ids.rs` | model (modify) | N/A | self | exact |
| `analysis_kernel/metadata.rs` | model (modify) | N/A | self (FactFamily enum) | exact |
| `analysis_kernel/provider.rs` | config (modify) | N/A | self (PROVIDER_MANIFESTS) | exact |
| `analysis_kernel/mod.rs` | service (modify) | request-response | self (kernel run sequence) | exact |
| `analysis_kernel/validation.rs` | middleware (modify) | transform | self (validate_fact_metadata) | exact |
| `analysis_kernel/debug.rs` | utility (modify) | transform | self (MetadataDebugReport) | exact |

## Pattern Assignments

### `analysis/summaries/mod.rs` (module-root)

**Analog:** `crates/polint/src/analysis/domains/mod.rs` (lines 1-11)

**Module declaration pattern:**
```rust
pub(crate) mod cache_key;
pub(crate) mod core;
pub(crate) mod domain;
pub(crate) mod facts;
pub(crate) mod builder;
pub(crate) mod provider;
pub(crate) mod store;
pub(crate) mod validate;
```

All sub-modules use `pub(crate)` visibility -- never `pub`.

---

### `analysis/summaries/facts.rs` (model, CRUD)

**Analog:** `crates/polint/src/analysis/domains/facts.rs` (lines 1-139)

**Imports pattern** (lines 1-4):
```rust
use serde::{Deserialize, Serialize};

use crate::analysis::ids::{MirBodyId, PlaceId, CallSiteId};
use crate::core::FunctionId;
```

**Enum pattern -- status/precision/slot** (lines 6-14, 29-48, 67-98):
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum DomainSlot {
    Reachability,
    Nilness,
    // ...
}

impl DomainSlot {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Reachability => "reachability",
            // ...
        }
    }
}
```

Apply this pattern for: `SummaryDomainKind`, `SummaryStatus`, `SummaryPrecision`, `SummaryProvenance`, `FlowKind`, `AccessKind`, `ExternalEffectKind`, `ExitKind`.

**Fact struct pattern** (lines 113-126):
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DomainObservationFact {
    pub(crate) id: DomainObservationId,
    pub(crate) body: MirBodyId,
    pub(crate) block: Option<BasicBlockId>,
    pub(crate) operation: Option<MirOpId>,
    pub(crate) place: Option<PlaceId>,
    pub(crate) slot: DomainSlot,
    pub(crate) location: DomainLocation,
    pub(crate) value: DomainValue,
    pub(crate) status: DomainStatus,
    pub(crate) precision: DomainPrecision,
    pub(crate) stable_key: String,
}
```

Summary fact structs follow this pattern: dense ID + typed fields + status + precision + stable_key.

**Test pattern** (lines 141-182):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fact_keeps_fields_separate() {
        let fact = FactStruct { /* all fields */ };
        assert_eq!(fact.id.0, 7);
        assert_eq!(fact.status, Status::Present);
    }
}
```

---

### `analysis/summaries/domain.rs` (model trait, transform)

**Analog:** `crates/polint/src/analysis/domains/lattice.rs` (lines 34-97)

**TopReason pattern** (lines 34-59):
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum TopReason {
    UnknownValue,
    UnsupportedSemantic,
    DynamicWrite,
    UnresolvedCall,
    SetupMissing,
    BudgetExceeded,
    Widened,
    ConflictingFacts,
}

impl TopReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self { /* ... */ }
    }
}
```

**Trait pattern** (lines 71-97):
```rust
pub(crate) trait AbstractDomain: Clone + Send + Sync + Eq + 'static {
    const ID: &'static str;
    const VERSION: u32;

    fn bottom() -> Self;
    fn top(reason: TopReason) -> Self;
    fn is_bottom(&self) -> bool;
    fn is_top(&self) -> bool;
    fn leq(&self, other: &Self) -> bool;
    fn join(&self, other: &Self) -> Self;

    fn join_into(&mut self, incoming: &Self) -> Changed {
        let joined = self.join(incoming);
        if joined == *self {
            Changed::No
        } else {
            *self = joined;
            Changed::Yes
        }
    }

    fn stable_digest_parts(&self) -> Vec<String>;
}
```

The `SummaryDomain` trait should mirror this pattern. Key differences:
- Operates at callable granularity, not per-place
- Include `unknown_top(reason: SummaryTopReason) -> Self` as explicit top constructor
- Include `stable_digest() -> Digest` or `stable_digest_parts() -> Vec<String>`

**Test pattern for trait laws** (lines 103-188):
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum SampleDomain {
        Bottom,
        Value(&'static str),
        Top(TopReason),
    }

    impl AbstractDomain for SampleDomain {
        const ID: &'static str = "test.sample";
        const VERSION: u32 = 1;
        // ... all trait methods
    }

    #[test]
    fn join_into_changes_only_when_canonical_state_changes() { /* ... */ }
}
```

---

### `analysis/summaries/core.rs` (domain implementations, transform)

**Analog:** `crates/polint/src/analysis/domains/core.rs` (lines 1-100)

**Dead code allow pattern** (lines 1-4):
```rust
#![expect(
    dead_code,
    reason = "Phase 32 introduces summary domains before later plans consume them."
)]
```

**Domain enum pattern** (lines 12-70):
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReachabilityDomain {
    Unreachable,
    Reachable,
    Ambiguous,
    Top(TopReason),
}
```

Apply this pattern for all four summary domains:
- `ControlEffects` -- exits, async_kind, cleanup_effects, top
- `CallEffects` -- callee_edges, unresolved, callback_use, top
- `MemoryEffects` -- per-resource access kinds, external flag, top
- `DataFlowTito` -- param-to-return flows, mutation edges, top

Each domain implements `SummaryDomain` (or `AbstractDomain`-derived) with `bottom()`, `unknown_top(reason)`, `leq`, `join`, `stable_digest_parts`.

**Impl pattern with factory constructors** (lines 72-100):
```rust
impl ConstantDomain {
    pub(crate) fn from_literal(literal: ConstantLiteral) -> Self {
        Self::Values(BTreeSet::from([literal]))
    }

    pub(crate) fn from_literals(literals: impl IntoIterator<Item = ConstantLiteral>) -> Self {
        let values = BTreeSet::from_iter(literals);
        if values.len() > LITERAL_SET_CAP {
            Self::Top(TopReason::BudgetExceeded)
        } else {
            Self::Values(values)
        }
    }
}
```

---

### `analysis/summaries/store.rs` (store, CRUD)

**Analog:** `crates/polint/src/analysis/calls/store.rs` (lines 1-162)

**Output struct pattern** (lines 10-48):
```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CallOutput {
    pub(crate) sites: Vec<CallSiteFact>,
    pub(crate) targets: Vec<CallTargetFact>,
    pub(crate) unresolved: Vec<UnresolvedCallFact>,
}

impl CallOutput {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    pub(crate) fn normalized(mut self) -> Self {
        self.sites.sort_by(|left, right| {
            (left.stable_key.as_str(), left.id).cmp(&(right.stable_key.as_str(), right.id))
        });
        // ... sort each collection by stable_key ...
        self
    }
}
```

**Store struct with BTreeMap indexes pattern** (lines 51-162):
```rust
#[derive(Debug, Clone, Default)]
pub(crate) struct CallStore {
    output: CallOutput,
    sites_by_caller: BTreeMap<FunctionId, Vec<usize>>,
    targets_by_site: BTreeMap<CallSiteId, Vec<usize>>,
    // ... additional indexes ...
}

impl CallStore {
    pub(crate) fn from_output(output: CallOutput) -> Result<Self, AnalysisError> {
        let output = output.normalized();
        // Validate referential integrity before indexing
        for target in &output.targets {
            if !site_ids.contains(&target.site) {
                return Err(AnalysisError::InvalidFact {
                    provider: "polint.calls",
                    reason: format!("dangling call site {:?}", target.site),
                });
            }
        }
        // Build indexes
        let mut store = Self { output, ..Self::default() };
        for (index, site) in store.output.sites.iter().enumerate() {
            store.sites_by_caller.entry(site.caller).or_default().push(index);
        }
        Ok(store)
    }

    // Accessor methods returning Vec<&Fact>
    pub(crate) fn sites_by_caller(&self, caller: FunctionId) -> Vec<&CallSiteFact> {
        self.site_refs(self.sites_by_caller.get(&caller))
    }

    fn site_refs(&self, indexes: Option<&Vec<usize>>) -> Vec<&CallSiteFact> {
        indexes.map_or_else(Vec::new, |indexes| {
            indexes.iter().map(|&index| &self.output.sites[index]).collect()
        })
    }
}
```

The `SummaryStore` should be indexed by `SummaryKey` (callable_stable_key + domain). The store rejects: missing digests, mismatched domain versions, precision exceeding ceilings, conflicting higher-trust summaries.

**Test pattern** (lines 242-419):
```rust
#[cfg(test)]
mod tests {
    // Helper factory functions
    fn site(id: u64, caller: u64, stable_key: &str) -> CallSiteFact { /* ... */ }
    fn target(id: u64, site: u64, caller: u64, stable_key: &str) -> CallTargetFact { /* ... */ }

    #[test]
    fn normalized_sorts_rows_without_dropping_duplicates() { /* ... */ }

    #[test]
    fn from_output_builds_deterministic_indexes() { /* ... */ }

    #[test]
    fn from_output_rejects_targets_without_matching_sites() { /* ... */ }

    #[test]
    fn empty_output_builds_empty_store() { /* ... */ }
}
```

---

### `analysis/summaries/builder.rs` (service, transform)

**Analog 1:** `crates/polint/src/analysis/domains/solver.rs` (for the local-analysis consumption pattern)
**Analog 2:** `crates/polint/src/analysis/calls/extract.rs` (for the MIR-walking extraction pattern)

No single close analog exists. The builder is a new component that:
1. Reads domain solver results (Phase 31) for control-effect lifting
2. Walks MIR/CFG/places for TITO and memory effects
3. Reads direct call facts for call-effect summaries

**Provider input consumption pattern** from `analysis/domains/provider.rs` (lines 32-33):
```rust
let solver = LocalDomainSolver::new(SolverPolicy::deterministic());
let result = solver.solve(SolverInput::from(&*db));
```

The summary builder should similarly consume `AnalysisDb` accessors:
```rust
// Read upstream facts from db
let mir_bodies = db.mir_bodies();
let cfg_blocks = db.cfg_blocks();
let call_sites = db.call_sites();
let domain_observations = db.abstract_domain_observations();
// Build summaries per function/callable
```

---

### `analysis/summaries/provider.rs` (provider, request-response)

**Analog:** `crates/polint/src/analysis/domains/provider.rs` (lines 1-63)

**Provider output struct** (lines 13-18):
```rust
#[derive(Debug, Clone, Default)]
pub(crate) struct AbstractDomainsProviderOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}
```

**Provider derive function signature** (lines 20-31):
```rust
#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_abstract_domains_with_cache_stats(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    semantic_mir_output_digest: Digest,
    cfg_output_digest: Digest,
    calls_output_digest: Digest,
    symbol_graph_output_digest: Digest,
    module_topology_output_digest: Digest,
    upstream_syntax_output_digests: Vec<Digest>,
) -> AbstractDomainsProviderOutput {
```

**Provider body pattern** (lines 32-63):
```rust
    // 1. Derive results
    let solver = LocalDomainSolver::new(SolverPolicy::deterministic());
    let result = solver.solve(SolverInput::from(&*db));
    // 2. Build stable key maps
    let body_keys = body_stable_key_map(db);
    // 3. Build output from results
    let output = DomainOutput::from_results_with_place_keys(result.results(), &place_keys);
    // 4. Compute output digest
    let output_digest = abstract_domains_output_digest(manifest, input_snapshot, ...);
    // 5. Record cache stats
    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();
    // 6. Store results in db
    db.replace_abstract_domain_facts(output);
    // 7. Return provider output
    AbstractDomainsProviderOutput {
        diagnostics: Vec::new(),
        cache_stats,
        output_digest: Some(output_digest),
    }
```

**Output digest function pattern** (lines 66-159):
```rust
fn abstract_domains_output_digest(
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    // ... upstream digests ...
    output: &DomainOutput,
) -> Digest {
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!("parameters={}", parameter_digest()),
        format!("config={}", input_snapshot.config.digest),
        // ... upstream digest lines ...
    ];
    // ... extend with output rows using stable keys ...
    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "output_label", &refs)
}
```

**Test pattern** (lines 208-264):
```rust
#[cfg(test)]
mod provider_tests {
    #[test]
    fn provider_accepts_empty_output_with_deterministic_digest() {
        // Create db, config, input snapshot
        // Call derive function
        // Assert diagnostics empty, digest present, cache_stats.recomputes == 1
    }

    #[test]
    fn provider_manifest_declares_private_outputs() {
        // Find manifest by id
        // Assert schema label, outputs list
    }
}
```

---

### `analysis/summaries/validate.rs` (middleware, transform)

**Analog:** `crates/polint/src/analysis/domains/validate.rs` (lines 1-78)

**Validation entrypoint pattern** (lines 11-78):
```rust
pub(crate) fn validate_abstract_domains(db: &AnalysisDb, diagnostics: &mut Vec<Diagnostic>) {
    // 1. Collect valid ID sets from upstream fact families
    let bodies = db.mir_bodies().iter().map(|row| row.id).collect();
    let blocks = db.cfg_blocks().iter().map(|row| row.id).collect();
    // ...

    // 2. Check for duplicate stable keys
    check_duplicate_stable_keys(diagnostics, "FamilyName",
        db.facts().iter().map(|row| row.stable_key.as_str()),
    );

    // 3. Validate each row
    for row in db.facts() {
        validate_row(db, diagnostics, &bodies, &blocks, row);
    }

    // 4. Check metadata precision ceilings
    for family in [FactFamily::SummaryControl, /* ... */] {
        for (reference, metadata) in db.fact_meta().rows() {
            if reference.family != family { continue; }
            if metadata.precision == FactPrecision::Exact {
                push_diagnostic(diagnostics, ...);
            }
        }
    }
}
```

**Analog 2:** `crates/polint/src/analysis/calls/validate.rs` (lines 1-352)

Same structural pattern but for call facts. Both follow:
1. Collect valid ID sets
2. Check duplicate stable keys
3. Validate each row against ID sets
4. Check metadata precision ceilings
5. Push diagnostics via helper function

**Diagnostic push helper** (lines 483-503 from domains/validate.rs):
```rust
fn push_domain_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    family: &'static str,
    stable_key: &str,
    field: &'static str,
    reason: &'static str,
) {
    tracing::debug!(family, stable_key, field, reason, "validation failed");
    diagnostics.push(Diagnostic::error(
        "polint/internal",
        "<workspace>",
        TextRange::point(1, 1),
        "Internal analysis validation failed.",
    ));
}
```

---

### `analysis/summaries/cache_key.rs` (utility, transform)

**Analog:** `crates/polint/src/analysis/domains/cache_key.rs` (lines 1-93)

**Parameter digest pattern** (lines 12-66):
```rust
pub(crate) fn abstract_domains_provider_parameter_digest() -> Digest {
    let policy = SolverPolicy::deterministic();
    abstract_domains_provider_parameter_digest_for_policy(
        policy.reduction_rounds,
        policy.budget.widening_fuel,
        policy.budget.max_iterations,
    )
}

fn abstract_domains_provider_parameter_digest_for_policy(
    max_reduction_rounds: u32,
    widening_fuel: u32,
    iteration_budget: u32,
) -> Digest {
    let parts = [
        format!("schema={SCHEMA_LABEL}:1"),
        format!("domain={}:{}", DomainType::ID, DomainType::VERSION),
        // ... per-domain version lines ...
    ];
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "parameter_label",
        &refs,
    )
}
```

For summaries, include all four summary domain IDs/versions plus builder policy parameters.

**Test pattern** (lines 68-93):
```rust
#[cfg(test)]
mod provider_parameters {
    use super::*;

    #[test]
    fn parameters_change_when_policy_inputs_change() {
        let baseline = provider_parameter_digest();
        let changed = provider_parameter_digest_for_test(99, 8, 10_000);
        assert_ne!(baseline, changed);
    }
}
```

---

### `analysis/ids.rs` (modify -- add summary IDs)

**Analog:** self -- `crates/polint/src/analysis/ids.rs` (lines 1-41)

**ID newtype pattern** (lines 3-4):
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct MirBodyId(pub(crate) u64);
```

Add new summary IDs following this exact derive set:
- `SummaryId(pub(crate) u64)`
- `SummaryEventId(pub(crate) u64)`

Add to `assert_small_id_contract` test (line 68).

---

### `analysis_kernel/metadata.rs` (modify -- extend FactFamily)

**Analog:** self -- `crates/polint/src/analysis_kernel/metadata.rs` (lines 5-69)

**FactFamily enum extension pattern** (lines 56-68):
```rust
    DomainObservation,
    DomainEvent,
    // Add after DomainEvent:
    SummaryControl,
    SummaryCall,
    SummaryMemory,
    SummaryTito,
    SummaryMeta,
```

Each new variant needs a `label()` match arm in the `impl FactFamily` block.

---

### `analysis_kernel/provider.rs` (modify -- add provider manifest)

**Analog:** self -- `crates/polint/src/analysis_kernel/provider.rs` (lines 388-410)

**Provider manifest entry pattern:**
```rust
    ProviderManifest {
        id: "polint.abstract_domains",
        kind: ProviderKind::WholeRepoDerived,
        inputs: &[
            "source_files", "functions", "mir_bodies", "mir_operations",
            "places", "unsupported_semantics", "cfg_functions", "basic_blocks",
            "cfg_edges", "call_sites", "call_targets", "unresolved_calls",
        ],
        outputs: &["domain_observations", "domain_events"],
        language_scope: LanguageScope::MultiLanguage,
        cache_policy: CachePolicy::InMemoryDerived,
        schema_versions: ABSTRACT_DOMAINS_SCHEMA,
        precision_ceiling: PrecisionCeiling::SetupAware,
    },
```

The summaries provider manifest follows this pattern, placed after `polint.abstract_domains` and before `polint.metrics`. Add `LayerKind::DirectSummaries` to the `LayerKind` enum (or reuse per planner discretion).

---

### `analysis_kernel/mod.rs` (modify -- wire provider into kernel run)

**Analog:** self -- `crates/polint/src/analysis_kernel/mod.rs` (lines 276-299)

**Provider wiring pattern:**
```rust
        let calls_dependency_output_digest = calls_output_digest.unwrap_or_else(|| {
            incremental::Digest::absent(incremental::DigestKind::ProviderOutput, "polint.calls")
        });
        let abstract_domains =
            crate::analysis::domains::provider::derive_abstract_domains_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.abstract_domains"),
                semantic_mir_dependency_output_digest,
                cfg_dependency_output_digest,
                calls_dependency_output_digest,
                symbol_dependency_output_digest,
                module_topology_dependency_output_digest,
                vec![go_dependency_output_digest.clone(), ts_dependency_output_digest.clone()],
            );
        let polint_abstract_domains_cache_stats = abstract_domains.cache_stats.clone();
        let abstract_domains_output_digest = abstract_domains.output_digest;
        diagnostics.extend(abstract_domains.diagnostics);
        provider_outputs.push(Self::provider_output_for_with_optional_digest(
            "polint.abstract_domains",
            &db,
            polint_abstract_domains_cache_stats,
            abstract_domains_output_digest,
        ));
```

Add a similar block for `polint.direct_summaries` immediately after the abstract_domains block, consuming the abstract_domains_output_digest as an additional upstream input.

---

### `analysis_kernel/validation.rs` (modify -- wire validation)

**Analog:** self -- `crates/polint/src/analysis_kernel/validation.rs` (lines 27-47)

**Validation wiring pattern:**
```rust
pub(crate) fn validate_fact_metadata(
    db: &AnalysisDb,
    manifests: &[ProviderManifest],
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    // ...
    validate_semantic_mir(db, &ids, &mut diagnostics);
    validate_cfg(db, &mut diagnostics);
    validate_calls(db, &mut diagnostics);
    validate_abstract_domains(db, &mut diagnostics);
    // Add: validate_summaries(db, &mut diagnostics);
```

---

### `analysis_kernel/debug.rs` (modify -- add debug report section)

**Analog:** self -- `crates/polint/src/analysis_kernel/debug.rs` (lines 26-52)

**Debug report struct pattern** (lines 41-52):
```rust
#[derive(Serialize)]
struct MetadataDebugReport<'a> {
    files: Vec<FileDebugRow<'a>>,
    // ...
    calls: CallDebugReport,
    abstract_domains: AbstractDomainDebugReport,
    // Add: summaries: SummaryDebugReport,
}
```

**Debug report section pattern** (from calls_report function and test at lines 2330-2467):
```rust
// Report struct
#[derive(Serialize)]
struct CallDebugReport { sites: Vec<...>, targets: Vec<...>, counts: CallCounts, index_counts: CallIndexCounts }

// Test
#[test]
fn metadata_debug_json_contains_call_rows_counts_and_indexes() {
    // Build db, populate facts
    let report = AnalysisKernel::metadata_debug_json_for_test(&db);
    let calls = report["calls"].as_object().unwrap();
    for key in ["sites", "targets", "counts", "index_counts"] {
        assert!(calls.get(key).is_some());
    }
}

// No-leak test
#[test]
fn debug_json_omits_source_ast_absolute_paths_and_dense_identity() {
    let encoded = report["calls"].to_string();
    assert!(!encoded.contains(env!("CARGO_MANIFEST_DIR")));
    assert!(!encoded.contains("export function"));
    assert!(!encoded.contains("parser"));
    assert!(!encoded.contains("\"id\""));
}
```

---

## Shared Patterns

### Stable Key Construction
**Source:** `crates/polint/src/analysis_kernel/metadata.rs` (stable_key_from_parts function, re-exported)
**Apply to:** All summary fact construction (facts.rs, store.rs, builder.rs)
```rust
use crate::analysis_kernel::{FactFamily, stable_key_from_parts};

stable_key_from_parts(
    FactFamily::SummaryControl,
    &[
        ("callable", callable_stable_key.clone()),
        ("domain", "control_effects".to_string()),
    ],
)
```

### Error Handling
**Source:** `crates/polint/src/analysis/error.rs` (lines 1-12)
**Apply to:** store.rs (from_output validation), builder.rs
```rust
#[derive(Debug, thiserror::Error)]
pub(crate) enum AnalysisError {
    #[error("invalid semantic fact from `{provider}`: {reason}")]
    InvalidFact {
        provider: &'static str,
        reason: String,
    },
}
```

### Diagnostic Push Pattern
**Source:** `crates/polint/src/analysis/domains/validate.rs` (lines 483-503) and `crates/polint/src/analysis/calls/validate.rs` (lines 332-351)
**Apply to:** validate.rs
```rust
fn push_summary_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    family: &'static str,
    stable_key: &str,
    field: &'static str,
    reason: &'static str,
) {
    tracing::debug!(family, stable_key, field, reason, "summary validation failed");
    diagnostics.push(Diagnostic::error(
        "polint/internal",
        "<workspace>",
        TextRange::point(1, 1),
        "Internal analysis validation failed.",
    ));
}
```

### Normalization and Deterministic Sort
**Source:** `crates/polint/src/analysis/domains/store.rs` (lines 147-201) and `crates/polint/src/analysis/calls/store.rs` (lines 22-48)
**Apply to:** store.rs (SummaryOutput::normalized)
```rust
pub(crate) fn normalized(mut self) -> Self {
    self.facts.sort_by(|left, right| {
        (left.stable_key.as_str(), left.id).cmp(&(right.stable_key.as_str(), right.id))
    });
    for (index, fact) in self.facts.iter_mut().enumerate() {
        fact.id = SummaryId(index as u64);
    }
    self
}
```

### Digest Construction
**Source:** `crates/polint/src/analysis_kernel/incremental/` (Digest::from_parts, DigestKind)
**Apply to:** cache_key.rs, provider.rs
```rust
use crate::analysis_kernel::incremental::{Digest, DigestKind};

Digest::from_parts(DigestKind::ProviderOutput, "summaries_output", &refs)
```

### Crate-Private Visibility
**Source:** All `analysis/` module files
**Apply to:** All new summary files
- All modules: `pub(crate) mod`
- All structs, enums, traits, functions: `pub(crate)`
- Never `pub` -- Phase 32 summaries are crate-private per D-04

### SummaryKey Reuse
**Source:** `crates/polint/src/analysis_kernel/incremental/keys.rs` (lines 91-98)
**Apply to:** store.rs (as the identity key for the summary store)
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

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| (none) | -- | -- | All files have close analogs in existing codebase |

All 16 files have exact or role-match analogs. The codebase has strong precedent for every component Phase 32 needs.

## Metadata

**Analog search scope:** `crates/polint/src/analysis/`, `crates/polint/src/analysis_kernel/`
**Files scanned:** 35+
**Pattern extraction date:** 2026-05-21

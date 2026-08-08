//! Analysis database types.
//!
//! Extracted from the core monolith without behaviour changes.

use crate::analysis::access_paths::facts::AccessPathFact;
use crate::analysis::access_paths::store::AccessPathStore;
use crate::analysis::adaptation::facts::{AcceptedModelFact, RejectedModelFact};
use crate::analysis::aliases::facts::AliasAnswerFact;
use crate::analysis::aliases::store::AliasStore;
use crate::analysis::calls::facts::{
    CallSiteFact, CallTargetFact, CallTargetStatus, UnresolvedCallFact, UnresolvedCallReason,
};
use crate::analysis::calls::store::{CallOutput, CallStore};
use crate::analysis::cfg::facts::{
    BasicBlockFact, CfgEdgeFact, CfgFunctionFact, CfgNodeFact, ControlDependenceFact,
    DominatorFact, PostDominatorFact, ReachabilityFact, UnsupportedControlFlowFact,
};
use crate::analysis::cfg::store::CfgOutput;
use crate::analysis::data_flow::facts::{
    DataFlowBudgetFact, DataFlowConfidence, DataFlowEdgeFact, DataFlowModelFact, DataFlowNodeFact,
    DataFlowPrecision, DataFlowStatus, DataFlowValidation,
};
use crate::analysis::data_flow::provider::DATA_FLOW_PROVIDER_ID;
use crate::analysis::data_flow::store::{DataFlowOutput, DataFlowStore};
use crate::analysis::domains::facts::{DomainEventFact, DomainObservationFact};
use crate::analysis::domains::store::{DomainOutput, DomainStore};
use crate::analysis::entrypoints::facts::{
    EntrypointFact, EntrypointStatus, FrameworkDispatchEdgeFact, TrustBoundaryFact,
    UnresolvedFrameworkFact,
};
use crate::analysis::entrypoints::store::{EntrypointOutput, EntrypointStore};
use crate::analysis::error::AnalysisError;
use crate::analysis::evidence::facts::{
    EvidenceBundleFact, EvidenceEdgeFact, EvidenceNodeFact, EvidenceOmittedRegionFact,
    EvidencePathFact, EvidencePrecision, EvidenceReplayKeyFact, EvidenceSliceFact,
    EvidenceUnknownFact,
};
use crate::analysis::evidence::provider::EVIDENCE_PROVIDER_ID;
use crate::analysis::evidence::store::{EvidenceOutput, EvidenceStore};
use crate::analysis::extensions::store::{
    AcceptedExtensionFact, ExtensionActivationRow, ExtensionOutput, RejectedExtensionFact,
};
use crate::analysis::identity::facts::IdentityRecord;
use crate::analysis::identity::provider::valid_call_site_ids;
use crate::analysis::identity::store::{IdentityProviderOutput, IdentityStore};
use crate::analysis::ids::CallSiteId;
use crate::analysis::mir::body::{MirBody, MirOutput};
use crate::analysis::mir::op::{MirOperation, UnsupportedSemanticFact};
use crate::analysis::places::PlaceFact;
use crate::analysis::points_to::facts::{PointsToConstraintFact, PointsToSetFact};
use crate::analysis::points_to::store::PointsToStore;
use crate::analysis::reachability::facts::{CallReachabilityFact, ReachabilityRootFact};
use crate::analysis::reachability::store::{ReachabilityProviderOutput, ReachabilityStore};
use crate::analysis::refined_calls::facts::RefinedCallEdgeFact;
use crate::analysis::refined_calls::provider::REFINED_CALLS_PROVIDER_ID;
use crate::analysis::refined_calls::store::{RefinedCallOutput, RefinedCallStore};
use crate::analysis::semantic_graph::constraints::ConstraintFact;
use crate::analysis::semantic_graph::facts::{SemanticEdgeFact, SemanticNodeFact};
use crate::analysis::semantic_graph::store::SemanticGraphOutput;
use crate::analysis::solver::budget::BudgetStatus;
use crate::analysis::solver::facts::DerivedEdgeFact;
use crate::analysis::solver::store::{SolverOutput, SolverStore};
use crate::analysis::store::SemanticStore;
use crate::analysis::summaries::facts::{SummaryEventFact, SummaryFact};
use crate::analysis::summaries::store::{SummaryOutput, SummaryStore};
use crate::analysis::types::facts::{NarrowedTypeFact, TypeFact};
use crate::analysis::types::provider::TYPE_VALUE_ALIAS_PROVIDER_ID;
use crate::analysis::types::store::{TypeStore, TypeValueAliasOutput};
use crate::analysis::values::facts::{AllocationTokenFact, ValueFact};
use crate::analysis::values::store::ValueStore;
use crate::analysis_kernel::{
    FactConfidence, FactFamily, FactMeta, FactMetaStore, FactPrecision, FactRef, MissingFactMeta,
    ValidationStatus, resolution_metadata, resolution_status_metadata, stable_key_from_parts,
    symbol_metadata,
};
use crate::diagnostics::fingerprint;
use crate::go::semantic::facts::{
    GoSemanticAddressTakenFact, GoSemanticCallsiteFact, GoSemanticDynamicDispatchFact,
    GoSemanticFunctionFact, GoSemanticInstantiatedTypeFact, GoSemanticMethodSetFact,
    GoSemanticPackageErrorFact, GoSemanticPackageFact,
};
use crate::go::semantic::store::{GoSemanticFactsOutput, GoSemanticStore, GoSemanticStoreReport};
use crate::module_graph::topology::{
    DependencyRequirementFact, ImportToPackageFact, RepoTopologyOverlayFact,
    ResolvedDependencyEdgeFact, SourceSetFact, TopologyOutput, TopologyPackageFact,
    WorkspaceRootFact,
};
use crate::symbol_graph::semantic::{
    AliasFact, AliasId, ExportFact, ExportId, GeneratedSymbolFact, GeneratedSymbolId,
    ResolutionFact, ResolutionId, ScopeFact, ScopeId, SemanticImportFact, SemanticImportId,
    SemanticStatus, StableExportId, StableExportIdentity,
};
use crate::ts::object_model::facts::{
    TsObjectAllocationFact, TsPropertyReadFact, TsPropertyWriteFact, TsPrototypeLinkFact,
    TsReceiverBindingFact,
};
use crate::ts::object_model::store::{TsObjectModelOutput, TsObjectModelStore};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use super::fact_store::{
    CALL_STORE_FAMILY, CFG_STORE_FAMILY, CfgFactStore, FactStore, FactStoreEntry,
    GO_SEMANTIC_STORE_FAMILY, GO_SYNTAX_STORE_FAMILY, GoSyntaxStore, TS_SYNTAX_STORE_FAMILY,
    TsSyntaxStore,
};
use super::facts::{
    BranchObligation, CachedFileFacts, ComplexityMetricFact, CoverageFact, DefinitionFact,
    FileMetricFact, FunctionFact, FunctionMetricFact, ImportFact, JsxAttributeFact, ModuleEdge,
    ModuleNode, PackageFact, ReferenceFact, ResolvedImportFact, SourceFile, StringLiteralFact,
    SymbolFact, TestFact, TsClassFact, TsComponentFact,
};
use super::ids::{
    BranchId, FileId, FunctionId, ImportId, ModuleEdgeId, ModuleNodeId, PackageId,
    ResolvedImportId, SymbolId,
};
use super::labels::*;
use super::lang::Language;
use super::metadata::*;
use super::review::ReviewChangeset;
use super::span::Span;
use super::{
    CALLS_PROVIDER_ID, CFG_PROVIDER_ID, CYCLOMATIC_COMPLEXITY_METRIC_NAME, ENTRYPOINTS_PROVIDER_ID,
    FUNCTION_SIZE_METRIC_NAME, GO_SYNTAX_PROVIDER_ID, METRICS_PROVIDER_ID,
    MODULE_GRAPH_PROVIDER_ID, MODULE_TOPOLOGY_PROVIDER_ID, POLINT_ABSTRACT_DOMAINS_PROVIDER_ID,
    POLINT_DIRECT_SUMMARIES_PROVIDER_ID, SEMANTIC_MIR_PROVIDER_ID, SYMBOL_GRAPH_PROVIDER_ID,
    TS_SYNTAX_PROVIDER_ID,
};

#[derive(Debug, Clone)]
pub struct AnalysisDb {
    pub(crate) files: Vec<SourceFile>,
    pub(crate) fact_meta: FactMetaStore,
    /// Provider-owned stores keyed by primary [`FactFamily`]. Iteration stays ordered.
    pub(crate) fact_stores: BTreeMap<FactFamily, FactStoreEntry>,
    pub(crate) resolved_imports: Vec<ResolvedImportFact>,
    pub(crate) module_nodes: Vec<ModuleNode>,
    pub(crate) module_edges: Vec<ModuleEdge>,
    pub(crate) workspace_roots: Vec<WorkspaceRootFact>,
    pub(crate) topology_packages: Vec<TopologyPackageFact>,
    pub(crate) source_sets: Vec<SourceSetFact>,
    pub(crate) dependency_requirements: Vec<DependencyRequirementFact>,
    pub(crate) resolved_dependency_edges: Vec<ResolvedDependencyEdgeFact>,
    pub(crate) import_to_package_edges: Vec<ImportToPackageFact>,
    pub(crate) repo_topology_overlays: Vec<RepoTopologyOverlayFact>,
    pub(crate) scopes: Vec<ScopeFact>,
    pub(crate) semantic_imports: Vec<SemanticImportFact>,
    pub(crate) exports: Vec<ExportFact>,
    pub(crate) aliases: Vec<AliasFact>,
    pub(crate) resolution_facts: Vec<ResolutionFact>,
    pub(crate) generated_symbols: Vec<GeneratedSymbolFact>,
    pub(crate) stable_exports: Vec<StableExportIdentity>,
    pub(crate) scopes_by_id: BTreeMap<ScopeId, usize>,
    pub(crate) semantic_imports_by_id: BTreeMap<SemanticImportId, usize>,
    pub(crate) exports_by_id: BTreeMap<ExportId, usize>,
    pub(crate) aliases_by_id: BTreeMap<AliasId, usize>,
    pub(crate) resolution_facts_by_id: BTreeMap<ResolutionId, usize>,
    pub(crate) generated_symbols_by_id: BTreeMap<GeneratedSymbolId, usize>,
    pub(crate) stable_exports_by_id: BTreeMap<StableExportId, usize>,
    pub(crate) symbols: Vec<SymbolFact>,
    pub(crate) definitions: Vec<DefinitionFact>,
    pub(crate) references: Vec<ReferenceFact>,
    pub(crate) symbols_by_id: BTreeMap<SymbolId, usize>,
    pub(crate) definitions_by_symbol: BTreeMap<SymbolId, Vec<usize>>,
    pub(crate) references_by_target: BTreeMap<SymbolId, Vec<usize>>,
    pub(crate) symbols_by_file: BTreeMap<FileId, Vec<usize>>,
    pub(crate) references_by_file: BTreeMap<FileId, Vec<usize>>,
    pub(crate) symbols_by_name: BTreeMap<String, Vec<usize>>,
    pub(crate) coverage: Vec<CoverageFact>,
    pub(crate) file_metrics: Vec<FileMetricFact>,
    pub(crate) function_metrics: Vec<FunctionMetricFact>,
    pub(crate) complexity_metrics: Vec<ComplexityMetricFact>,
    pub(crate) semantic: Option<SemanticStore>,
    pub(crate) identity_records: Vec<IdentityRecord>,
    pub(crate) identity_store: Option<IdentityStore>,
    pub(crate) refined_call_edges: Vec<RefinedCallEdgeFact>,
    pub(crate) refined_call_store: Option<RefinedCallStore>,
    pub(crate) data_flow_nodes: Vec<DataFlowNodeFact>,
    pub(crate) data_flow_edges: Vec<DataFlowEdgeFact>,
    pub(crate) data_flow_models: Vec<DataFlowModelFact>,
    pub(crate) data_flow_budgets: Vec<DataFlowBudgetFact>,
    pub(crate) data_flow_store: Option<DataFlowStore>,
    pub(crate) evidence_nodes: Vec<EvidenceNodeFact>,
    pub(crate) evidence_edges: Vec<EvidenceEdgeFact>,
    pub(crate) evidence_bundles: Vec<EvidenceBundleFact>,
    pub(crate) evidence_paths: Vec<EvidencePathFact>,
    pub(crate) evidence_slices: Vec<EvidenceSliceFact>,
    pub(crate) evidence_unknowns: Vec<EvidenceUnknownFact>,
    pub(crate) evidence_omitted_regions: Vec<EvidenceOmittedRegionFact>,
    pub(crate) evidence_replay_keys: Vec<EvidenceReplayKeyFact>,
    pub(crate) evidence_store: Option<EvidenceStore>,
    pub(crate) abstract_domain_store: Option<DomainStore>,
    pub(crate) summary_facts: Vec<SummaryFact>,
    pub(crate) summary_events: Vec<SummaryEventFact>,
    pub(crate) summary_store: Option<SummaryStore>,
    pub(crate) extension_activations: Vec<ExtensionActivationRow>,
    pub(crate) extension_facts: Vec<AcceptedExtensionFact>,
    #[allow(
        dead_code,
        reason = "Rejected extension audit rows are surfaced by the extension provider/debug wiring in the next plan."
    )]
    pub(crate) rejected_extension_facts: Vec<RejectedExtensionFact>,
    pub(crate) adaptation_model_facts: Vec<AcceptedModelFact>,
    pub(crate) rejected_adaptation_model_facts: Vec<RejectedModelFact>,
    pub(crate) entrypoint_facts: Vec<EntrypointFact>,
    pub(crate) trust_boundary_facts: Vec<TrustBoundaryFact>,
    pub(crate) dispatch_edge_facts: Vec<FrameworkDispatchEdgeFact>,
    pub(crate) unresolved_framework_facts: Vec<UnresolvedFrameworkFact>,
    pub(crate) entrypoint_store: Option<EntrypointStore>,
    pub(crate) reachability_roots: Vec<ReachabilityRootFact>,
    pub(crate) reachability_marks: Vec<CallReachabilityFact>,
    pub(crate) semantic_nodes: Vec<SemanticNodeFact>,
    pub(crate) semantic_edges: Vec<SemanticEdgeFact>,
    pub(crate) semantic_constraints: Vec<ConstraintFact>,
    #[allow(
        dead_code,
        reason = " stores TS object-model rows before semantic-graph lowering consumes them."
    )]
    pub(crate) ts_object_allocations: Vec<TsObjectAllocationFact>,
    #[allow(
        dead_code,
        reason = " stores TS object-model rows before semantic-graph lowering consumes them."
    )]
    pub(crate) ts_property_writes: Vec<TsPropertyWriteFact>,
    #[allow(
        dead_code,
        reason = " stores TS object-model rows before semantic-graph lowering consumes them."
    )]
    pub(crate) ts_property_reads: Vec<TsPropertyReadFact>,
    #[allow(
        dead_code,
        reason = " stores TS object-model rows before semantic-graph lowering consumes them."
    )]
    pub(crate) ts_receiver_bindings: Vec<TsReceiverBindingFact>,
    #[allow(
        dead_code,
        reason = " stores TS object-model rows before semantic-graph lowering consumes them."
    )]
    pub(crate) ts_prototype_links: Vec<TsPrototypeLinkFact>,
    #[allow(
        dead_code,
        reason = " stores TS object-model rows before semantic-graph lowering consumes them."
    )]
    pub(crate) ts_object_model_store: Option<TsObjectModelStore>,
    pub(crate) solver_derived_edges: Vec<DerivedEdgeFact>,
    pub(crate) solver_budget_status: BudgetStatus,
    pub(crate) solver_budget_reasons: BTreeSet<String>,
    pub(crate) type_facts: Vec<TypeFact>,
    pub(crate) narrowed_type_facts: Vec<NarrowedTypeFact>,
    pub(crate) value_facts: Vec<ValueFact>,
    pub(crate) allocation_tokens: Vec<AllocationTokenFact>,
    pub(crate) access_path_facts: Vec<AccessPathFact>,
    pub(crate) points_to_constraints: Vec<PointsToConstraintFact>,
    pub(crate) points_to_sets: Vec<PointsToSetFact>,
    pub(crate) alias_answers: Vec<AliasAnswerFact>,
    pub(crate) type_store: Option<TypeStore>,
    pub(crate) value_store: Option<ValueStore>,
    pub(crate) access_path_store: Option<AccessPathStore>,
    pub(crate) points_to_store: Option<PointsToStore>,
    pub(crate) alias_store: Option<AliasStore>,
    pub(crate) path_contexts: Option<crate::path_context::PathContextIndex>,
    /// Diff-to-target-ref facts, injected by the host for `polint review`.
    ///
    /// This is the first externally injected fact family: it is set by the
    /// runner via [`AnalysisDb::set_changeset`] after the kernel runs, not
    /// derived by a provider. It is `None` under `polint check` (so the
    /// `ChangedFiles` view is empty there) and excluded from all cache digests.
    pub(crate) changeset: Option<ReviewChangeset>,
}

impl Default for AnalysisDb {
    fn default() -> Self {
        let mut fact_stores = BTreeMap::new();
        let mut go_syntax = GoSyntaxStore::default();
        // Trait method is part of the store contract (eviction later). Calling on an
        // empty store at construction keeps the method on the product path.
        FactStore::clear(&mut go_syntax);
        fact_stores.insert(GO_SYNTAX_STORE_FAMILY, FactStoreEntry::new(go_syntax));
        let mut ts_syntax = TsSyntaxStore::default();
        FactStore::clear(&mut ts_syntax);
        fact_stores.insert(TS_SYNTAX_STORE_FAMILY, FactStoreEntry::new(ts_syntax));
        let mut cfg_store = CfgFactStore::default();
        FactStore::clear(&mut cfg_store);
        fact_stores.insert(CFG_STORE_FAMILY, FactStoreEntry::new(cfg_store));
        let mut call_store = CallStore::default();
        FactStore::clear(&mut call_store);
        fact_stores.insert(CALL_STORE_FAMILY, FactStoreEntry::new(call_store));
        let mut go_semantic = GoSemanticStore::default();
        FactStore::clear(&mut go_semantic);
        fact_stores.insert(GO_SEMANTIC_STORE_FAMILY, FactStoreEntry::new(go_semantic));
        Self {
            files: Vec::new(),
            fact_meta: FactMetaStore::default(),
            fact_stores,
            resolved_imports: Vec::new(),
            module_nodes: Vec::new(),
            module_edges: Vec::new(),
            workspace_roots: Vec::new(),
            topology_packages: Vec::new(),
            source_sets: Vec::new(),
            dependency_requirements: Vec::new(),
            resolved_dependency_edges: Vec::new(),
            import_to_package_edges: Vec::new(),
            repo_topology_overlays: Vec::new(),
            scopes: Vec::new(),
            semantic_imports: Vec::new(),
            exports: Vec::new(),
            aliases: Vec::new(),
            resolution_facts: Vec::new(),
            generated_symbols: Vec::new(),
            stable_exports: Vec::new(),
            scopes_by_id: BTreeMap::new(),
            semantic_imports_by_id: BTreeMap::new(),
            exports_by_id: BTreeMap::new(),
            aliases_by_id: BTreeMap::new(),
            resolution_facts_by_id: BTreeMap::new(),
            generated_symbols_by_id: BTreeMap::new(),
            stable_exports_by_id: BTreeMap::new(),
            symbols: Vec::new(),
            definitions: Vec::new(),
            references: Vec::new(),
            symbols_by_id: BTreeMap::new(),
            definitions_by_symbol: BTreeMap::new(),
            references_by_target: BTreeMap::new(),
            symbols_by_file: BTreeMap::new(),
            references_by_file: BTreeMap::new(),
            symbols_by_name: BTreeMap::new(),
            coverage: Vec::new(),
            file_metrics: Vec::new(),
            function_metrics: Vec::new(),
            complexity_metrics: Vec::new(),
            semantic: None,
            identity_records: Vec::new(),
            identity_store: None,
            refined_call_edges: Vec::new(),
            refined_call_store: None,
            data_flow_nodes: Vec::new(),
            data_flow_edges: Vec::new(),
            data_flow_models: Vec::new(),
            data_flow_budgets: Vec::new(),
            data_flow_store: None,
            evidence_nodes: Vec::new(),
            evidence_edges: Vec::new(),
            evidence_bundles: Vec::new(),
            evidence_paths: Vec::new(),
            evidence_slices: Vec::new(),
            evidence_unknowns: Vec::new(),
            evidence_omitted_regions: Vec::new(),
            evidence_replay_keys: Vec::new(),
            evidence_store: None,
            abstract_domain_store: None,
            summary_facts: Vec::new(),
            summary_events: Vec::new(),
            summary_store: None,
            extension_activations: Vec::new(),
            extension_facts: Vec::new(),
            rejected_extension_facts: Vec::new(),
            adaptation_model_facts: Vec::new(),
            rejected_adaptation_model_facts: Vec::new(),
            entrypoint_facts: Vec::new(),
            trust_boundary_facts: Vec::new(),
            dispatch_edge_facts: Vec::new(),
            unresolved_framework_facts: Vec::new(),
            entrypoint_store: None,
            reachability_roots: Vec::new(),
            reachability_marks: Vec::new(),
            semantic_nodes: Vec::new(),
            semantic_edges: Vec::new(),
            semantic_constraints: Vec::new(),
            ts_object_allocations: Vec::new(),
            ts_property_writes: Vec::new(),
            ts_property_reads: Vec::new(),
            ts_receiver_bindings: Vec::new(),
            ts_prototype_links: Vec::new(),
            ts_object_model_store: None,
            solver_derived_edges: Vec::new(),
            solver_budget_status: BudgetStatus::NotRun,
            solver_budget_reasons: BTreeSet::new(),
            type_facts: Vec::new(),
            narrowed_type_facts: Vec::new(),
            value_facts: Vec::new(),
            allocation_tokens: Vec::new(),
            access_path_facts: Vec::new(),
            points_to_constraints: Vec::new(),
            points_to_sets: Vec::new(),
            alias_answers: Vec::new(),
            type_store: None,
            value_store: None,
            access_path_store: None,
            points_to_store: None,
            alias_store: None,
            path_contexts: None,
            changeset: None,
        }
    }
}

impl AnalysisDb {
    pub fn new() -> Self {
        Self::default()
    }

    fn go_syntax_store(&self) -> &GoSyntaxStore {
        self.fact_store(GO_SYNTAX_STORE_FAMILY)
            .expect("GoSyntaxStore is installed when AnalysisDb is constructed")
    }

    fn go_syntax_store_mut(&mut self) -> &mut GoSyntaxStore {
        self.fact_store_mut(GO_SYNTAX_STORE_FAMILY)
            .expect("GoSyntaxStore is installed when AnalysisDb is constructed")
    }

    fn ts_syntax_store(&self) -> &TsSyntaxStore {
        self.fact_store(TS_SYNTAX_STORE_FAMILY)
            .expect("TsSyntaxStore is installed when AnalysisDb is constructed")
    }

    fn ts_syntax_store_mut(&mut self) -> &mut TsSyntaxStore {
        self.fact_store_mut(TS_SYNTAX_STORE_FAMILY)
            .expect("TsSyntaxStore is installed when AnalysisDb is constructed")
    }

    fn cfg_store(&self) -> &CfgFactStore {
        self.fact_store(CFG_STORE_FAMILY)
            .expect("CfgFactStore is installed when AnalysisDb is constructed")
    }

    fn cfg_store_mut(&mut self) -> &mut CfgFactStore {
        self.fact_store_mut(CFG_STORE_FAMILY)
            .expect("CfgFactStore is installed when AnalysisDb is constructed")
    }

    fn calls_store(&self) -> &CallStore {
        self.fact_store(CALL_STORE_FAMILY)
            .expect("CallStore is installed when AnalysisDb is constructed")
    }

    fn calls_store_mut(&mut self) -> &mut CallStore {
        self.fact_store_mut(CALL_STORE_FAMILY)
            .expect("CallStore is installed when AnalysisDb is constructed")
    }

    fn go_semantic_store(&self) -> &GoSemanticStore {
        self.fact_store(GO_SEMANTIC_STORE_FAMILY)
            .expect("GoSemanticStore is installed when AnalysisDb is constructed")
    }

    fn go_semantic_store_mut(&mut self) -> &mut GoSemanticStore {
        self.fact_store_mut(GO_SEMANTIC_STORE_FAMILY)
            .expect("GoSemanticStore is installed when AnalysisDb is constructed")
    }

    /// Typed downcast helper for registry stores. Returns `None` when the family
    /// is absent or holds a different concrete store type.
    pub(crate) fn fact_store<T: 'static>(&self, family: FactFamily) -> Option<&T> {
        self.fact_stores
            .get(&family)
            .and_then(|entry| entry.as_store().as_any().downcast_ref::<T>())
    }

    /// Mutable typed downcast helper for registry stores.
    pub(crate) fn fact_store_mut<T: 'static>(&mut self, family: FactFamily) -> Option<&mut T> {
        self.fact_stores
            .get_mut(&family)
            .and_then(|entry| entry.as_store_mut().as_any_mut().downcast_mut::<T>())
    }

    /// Injects diff-to-target-ref facts for `polint review`.
    ///
    /// Called by the host runner after the kernel runs and before rules
    /// execute, so the `ChangedFiles` fact view can read the diff. The
    /// changeset is excluded from all cache digests by construction (it is set
    /// post-kernel), so a changing diff never busts the analysis cache.
    pub(crate) fn set_changeset(&mut self, changeset: ReviewChangeset) {
        self.changeset = Some(changeset);
    }

    /// Returns the injected changeset, or `None` under `polint check`.
    pub(crate) fn changeset(&self) -> Option<&ReviewChangeset> {
        self.changeset.as_ref()
    }

    pub fn add_file(&mut self, path: PathBuf, relative_path: String, source: String) -> FileId {
        let language = Language::from_path(&path);
        let content_hash = fingerprint(&[&source]);
        self.push_source_file(
            path,
            relative_path,
            language,
            Arc::from(source),
            content_hash,
        )
    }

    pub fn add_source_file(
        &mut self,
        path: PathBuf,
        relative_path: String,
        language: Language,
        source: Arc<str>,
        content_hash: String,
    ) -> FileId {
        self.push_source_file(path, relative_path, language, source, content_hash)
    }

    fn push_source_file(
        &mut self,
        path: PathBuf,
        relative_path: String,
        language: Language,
        source: Arc<str>,
        content_hash: String,
    ) -> FileId {
        let id = FileId(self.files.len() as u32);
        let metadata = source_file_metadata(&relative_path, language, &content_hash);
        self.files.push(SourceFile {
            id,
            path,
            relative_path,
            language,
            source,
            content_hash,
        });
        self.record_fact_meta(FactFamily::SourceFile, u64::from(id.0), metadata);
        id
    }

    pub fn push_package(&mut self, mut fact: PackageFact) -> PackageId {
        fact.id = PackageId(self.go_syntax_store().packages().len() as u64);
        let metadata = self.package_metadata(&fact);
        let id = self.go_syntax_store_mut().push_package(fact);
        self.record_fact_meta(FactFamily::Package, id.0, metadata);
        id
    }

    pub fn push_function(&mut self, mut fact: FunctionFact) -> FunctionId {
        fact.id = FunctionId(self.go_syntax_store().functions().len() as u64);
        let metadata = self.function_metadata(&fact);
        let id = self.go_syntax_store_mut().push_function(fact);
        self.record_fact_meta(FactFamily::Function, id.0, metadata);
        id
    }

    pub fn push_import(&mut self, mut fact: ImportFact) -> ImportId {
        fact.id = ImportId(self.go_syntax_store().imports().len() as u64);
        let metadata = self.import_metadata(&fact);
        let id = self.go_syntax_store_mut().push_import(fact);
        self.record_fact_meta(FactFamily::Import, id.0, metadata);
        id
    }

    pub fn push_branch(&mut self, mut fact: BranchObligation) -> BranchId {
        fact.id = BranchId(self.go_syntax_store().branches().len() as u64);
        let metadata = self.branch_metadata(&fact);
        let id = self.go_syntax_store_mut().push_branch(fact);
        self.record_fact_meta(FactFamily::BranchObligation, id.0, metadata);
        id
    }

    pub fn push_test(&mut self, fact: TestFact) {
        let metadata = self.test_metadata(&fact);
        let run_id = self.go_syntax_store_mut().push_test(fact);
        self.record_fact_meta(FactFamily::Test, run_id, metadata);
    }

    pub fn push_coverage(&mut self, fact: CoverageFact) {
        let run_id = self.coverage.len() as u64;
        let metadata = self.coverage_metadata(&fact);
        self.coverage.push(fact);
        self.record_fact_meta(FactFamily::Coverage, run_id, metadata);
    }

    pub(crate) fn replace_metric_facts(
        &mut self,
        file_metrics: Vec<FileMetricFact>,
        function_metrics: Vec<FunctionMetricFact>,
        complexity_metrics: Vec<ComplexityMetricFact>,
    ) {
        self.file_metrics = file_metrics;
        self.function_metrics = function_metrics;
        self.complexity_metrics = complexity_metrics;
        self.refresh_metric_metadata();
    }

    pub(crate) fn replace_module_graph_facts(
        &mut self,
        mut resolved_imports: Vec<ResolvedImportFact>,
        mut module_nodes: Vec<ModuleNode>,
        mut module_edges: Vec<ModuleEdge>,
    ) {
        let resolved_import_ids = resolved_imports
            .iter()
            .enumerate()
            .map(|(index, fact)| (fact.id, ResolvedImportId(index as u64)))
            .collect::<BTreeMap<_, _>>();
        let module_node_ids = module_nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id, ModuleNodeId(index as u64)))
            .collect::<BTreeMap<_, _>>();

        for (index, fact) in resolved_imports.iter_mut().enumerate() {
            fact.id = ResolvedImportId(index as u64);
            if let Some(target_node) = fact.target_node
                && let Some(remapped) = module_node_ids.get(&target_node)
            {
                fact.target_node = Some(*remapped);
            }
        }
        for (index, node) in module_nodes.iter_mut().enumerate() {
            node.id = ModuleNodeId(index as u64);
        }
        for (index, edge) in module_edges.iter_mut().enumerate() {
            edge.id = ModuleEdgeId(index as u64);
            if let Some(remapped) = module_node_ids.get(&edge.from) {
                edge.from = *remapped;
            }
            if let Some(remapped) = module_node_ids.get(&edge.to) {
                edge.to = *remapped;
            }
            if let Some(resolved_import) = edge.resolved_import
                && let Some(remapped) = resolved_import_ids.get(&resolved_import)
            {
                edge.resolved_import = Some(*remapped);
            }
        }

        self.resolved_imports = resolved_imports;
        self.module_nodes = module_nodes;
        self.module_edges = module_edges;
        self.refresh_module_graph_metadata();
    }

    pub(crate) fn replace_topology_facts(&mut self, output: TopologyOutput) {
        let output = output.normalized();
        self.workspace_roots = output.workspace_roots;
        self.topology_packages = output.packages;
        self.source_sets = output.source_sets;
        self.dependency_requirements = output.dependency_requirements;
        self.resolved_dependency_edges = output.resolved_dependency_edges;
        self.import_to_package_edges = output.import_to_package_edges;
        self.repo_topology_overlays = output.overlays;
        self.refresh_topology_metadata();
    }

    pub(crate) fn replace_import_to_package_facts(&mut self, edges: Vec<ImportToPackageFact>) {
        let output = TopologyOutput {
            import_to_package_edges: edges,
            ..TopologyOutput::default()
        }
        .normalized();
        self.import_to_package_edges = output.import_to_package_edges;
        self.refresh_import_to_package_metadata();
    }

    pub(crate) fn replace_symbol_graph_facts(
        &mut self,
        symbols: Vec<SymbolFact>,
        definitions: Vec<DefinitionFact>,
        references: Vec<ReferenceFact>,
    ) {
        self.symbols = symbols;
        self.definitions = definitions;
        self.references = references;
        self.rebuild_symbol_graph_indexes();
        self.refresh_symbol_graph_metadata();
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "semantic index replacement accepts every internal semantic row family explicitly"
    )]
    pub(crate) fn replace_semantic_index_facts(
        &mut self,
        mut scopes: Vec<ScopeFact>,
        mut semantic_imports: Vec<SemanticImportFact>,
        mut exports: Vec<ExportFact>,
        mut aliases: Vec<AliasFact>,
        mut resolutions: Vec<ResolutionFact>,
        mut generated_symbols: Vec<GeneratedSymbolFact>,
        mut stable_exports: Vec<StableExportIdentity>,
    ) {
        normalize_scope_facts(&mut scopes);
        let scope_ids = scopes
            .iter()
            .enumerate()
            .map(|(index, scope)| (scope.id, ScopeId(index as u64)))
            .collect::<BTreeMap<_, _>>();
        for (index, scope) in scopes.iter_mut().enumerate() {
            scope.id = ScopeId(index as u64);
            if let Some(parent) = scope.parent
                && let Some(remapped) = scope_ids.get(&parent)
            {
                scope.parent = Some(*remapped);
            }
        }

        normalize_semantic_import_facts(&mut semantic_imports);
        for (index, import) in semantic_imports.iter_mut().enumerate() {
            import.id = SemanticImportId(index as u64);
            if let Some(scope) = import.scope
                && let Some(remapped) = scope_ids.get(&scope)
            {
                import.scope = Some(*remapped);
            }
        }

        normalize_export_facts(&mut exports);
        let export_ids = exports
            .iter()
            .enumerate()
            .map(|(index, export)| (export.id, ExportId(index as u64)))
            .collect::<BTreeMap<_, _>>();
        for (index, export) in exports.iter_mut().enumerate() {
            export.id = ExportId(index as u64);
            if let Some(scope) = export.scope
                && let Some(remapped) = scope_ids.get(&scope)
            {
                export.scope = Some(*remapped);
            }
        }

        normalize_alias_facts(&mut aliases);
        for (index, alias) in aliases.iter_mut().enumerate() {
            alias.id = AliasId(index as u64);
        }

        normalize_resolution_facts(&mut resolutions);
        for (index, resolution) in resolutions.iter_mut().enumerate() {
            resolution.id = ResolutionId(index as u64);
        }

        normalize_generated_symbol_facts(&mut generated_symbols);
        for (index, generated) in generated_symbols.iter_mut().enumerate() {
            generated.id = GeneratedSymbolId(index as u64);
        }

        normalize_stable_export_identities(&mut stable_exports);
        for (index, stable_export) in stable_exports.iter_mut().enumerate() {
            stable_export.id = StableExportId(index as u64);
            if let Some(remapped) = export_ids.get(&stable_export.export) {
                stable_export.export = *remapped;
            }
        }

        self.scopes = scopes;
        self.semantic_imports = semantic_imports;
        self.exports = exports;
        self.aliases = aliases;
        self.resolution_facts = resolutions;
        self.generated_symbols = generated_symbols;
        self.stable_exports = stable_exports;
        self.rebuild_semantic_index_indexes();
        self.refresh_semantic_index_metadata();
    }

    pub(crate) fn replace_semantic_mir(&mut self, output: MirOutput) -> Result<(), AnalysisError> {
        self.semantic = Some(SemanticStore::from_output(output)?);
        self.refresh_semantic_mir_metadata();
        Ok(())
    }

    pub(crate) fn replace_cfg_facts(&mut self, output: CfgOutput) -> Result<(), AnalysisError> {
        let output = output.normalized();
        self.cfg_store_mut().replace(output);
        self.refresh_cfg_metadata();
        Ok(())
    }

    pub(crate) fn replace_call_facts(
        &mut self,
        mut output: CallOutput,
    ) -> Result<(), AnalysisError> {
        self.populate_call_owner_symbols(&mut output);
        let store = CallStore::from_output(output)?;
        *self.calls_store_mut() = store;
        self.refresh_call_metadata();
        Ok(())
    }

    pub(crate) fn replace_identity_facts(
        &mut self,
        output: IdentityProviderOutput,
    ) -> Result<(), AnalysisError> {
        let valid_sites = valid_call_site_ids(self);
        let valid_targets = self
            .call_targets()
            .iter()
            .map(|target| target.id)
            .collect::<BTreeSet<_>>();
        let store = IdentityStore::from_output(output, &valid_sites, &valid_targets)?;
        self.identity_records = store.records().to_vec();
        self.identity_store = Some(store);
        Ok(())
    }

    pub(crate) fn identity_records(&self) -> &[IdentityRecord] {
        &self.identity_records
    }

    /// Injects identity records directly, bypassing store-level reference
    /// validation, so validation diagnostics (the defense-in-depth layer) can be
    /// exercised even for records that the store would have rejected.
    #[cfg(test)]
    pub(crate) fn set_identity_records_for_test(&mut self, records: Vec<IdentityRecord>) {
        self.identity_records = records;
        self.identity_store = None;
    }

    #[allow(dead_code)]
    pub(crate) fn identity_store(&self) -> Option<&IdentityStore> {
        self.identity_store.as_ref()
    }

    #[allow(
        dead_code,
        reason = "Provider hot paths pass normalized output directly; tests and compatibility callers still use the normalizing entry point."
    )]
    pub(crate) fn replace_refined_call_facts(
        &mut self,
        output: RefinedCallOutput,
    ) -> Result<(), AnalysisError> {
        self.replace_normalized_refined_call_facts(output.normalized())
    }

    pub(crate) fn replace_normalized_refined_call_facts(
        &mut self,
        output: RefinedCallOutput,
    ) -> Result<(), AnalysisError> {
        let store = RefinedCallStore::from_normalized_output(output)?;
        self.refined_call_edges.clear();
        self.refined_call_store = Some(store);
        self.refresh_refined_call_metadata();
        Ok(())
    }

    pub(crate) fn replace_data_flow_facts(
        &mut self,
        output: DataFlowOutput,
    ) -> Result<(), AnalysisError> {
        let store = DataFlowStore::from_output(output)?;
        self.data_flow_nodes = store.nodes().to_vec();
        self.data_flow_edges = store.edges().to_vec();
        self.data_flow_models = store.models().to_vec();
        self.data_flow_budgets = store.budgets().to_vec();
        self.data_flow_store = Some(store);
        self.refresh_data_flow_metadata();
        Ok(())
    }

    pub(crate) fn replace_evidence_facts(
        &mut self,
        output: EvidenceOutput,
    ) -> Result<(), AnalysisError> {
        let store = EvidenceStore::from_output(output)?;
        self.evidence_nodes = store.nodes().to_vec();
        self.evidence_edges = store.edges().to_vec();
        self.evidence_bundles = store.bundles().to_vec();
        self.evidence_paths = store.paths().to_vec();
        self.evidence_slices = store.slices().to_vec();
        self.evidence_unknowns = store.unknowns().to_vec();
        self.evidence_omitted_regions = store.omitted_regions().to_vec();
        self.evidence_replay_keys = store.replay_keys().to_vec();
        self.evidence_store = Some(store);
        self.refresh_evidence_metadata();
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn replace_abstract_domain_facts(&mut self, output: DomainOutput) {
        let store = DomainStore::from_output(output);
        self.replace_abstract_domain_store(store);
    }

    pub(crate) fn replace_normalized_abstract_domain_facts(&mut self, output: DomainOutput) {
        let store = DomainStore::from_normalized_output(output);
        self.replace_abstract_domain_store(store);
    }

    fn replace_abstract_domain_store(&mut self, store: DomainStore) {
        self.abstract_domain_store = Some(store);
        self.refresh_abstract_domain_metadata();
    }

    fn populate_call_owner_symbols(&self, output: &mut CallOutput) {
        if output.sites.iter().all(|site| site.owner_symbol.is_some()) {
            return;
        }

        let function_symbols = self
            .functions()
            .iter()
            .filter_map(|function| {
                let symbol = self
                    .symbols
                    .iter()
                    .find(|symbol| {
                        symbol.file == Some(function.file)
                            && symbol.name == function.name
                            && symbol.primary_span.as_ref().is_some_and(|span| {
                                span == &function.span || Self::span_is_within(span, &function.span)
                            })
                    })
                    .map(|symbol| symbol.id)
                    .or_else(|| {
                        self.definitions
                            .iter()
                            .find(|definition| {
                                definition.file == Some(function.file)
                                    && definition.name == function.name
                                    && definition.primary_span.as_ref().is_some_and(|span| {
                                        span == &function.span
                                            || Self::span_is_within(span, &function.span)
                                    })
                            })
                            .map(|definition| definition.symbol)
                    });
                symbol.map(|symbol| (function.id, symbol))
            })
            .collect::<BTreeMap<_, _>>();

        for site in &mut output.sites {
            if site.owner_symbol.is_none() {
                site.owner_symbol = function_symbols.get(&site.caller).copied();
            }
        }
    }

    fn span_is_within(inner: &Span, outer: &Span) -> bool {
        inner.file == outer.file
            && inner.start_byte >= outer.start_byte
            && inner.end_byte <= outer.end_byte
    }

    pub(crate) fn call_sites(&self) -> &[CallSiteFact] {
        self.calls_store().sites()
    }

    pub(crate) fn call_targets(&self) -> &[CallTargetFact] {
        self.calls_store().targets()
    }

    pub(crate) fn unresolved_calls(&self) -> &[UnresolvedCallFact] {
        self.calls_store().unresolved()
    }

    pub(crate) fn call_store(&self) -> Option<&CallStore> {
        Some(self.calls_store())
    }

    pub(crate) fn refined_call_edges(&self) -> &[RefinedCallEdgeFact] {
        if let Some(store) = &self.refined_call_store {
            store.edges()
        } else {
            &self.refined_call_edges
        }
    }

    #[allow(dead_code)]
    pub(crate) fn refined_call_store(&self) -> Option<&RefinedCallStore> {
        self.refined_call_store.as_ref()
    }

    pub(crate) fn data_flow_nodes(&self) -> &[DataFlowNodeFact] {
        &self.data_flow_nodes
    }

    pub(crate) fn data_flow_edges(&self) -> &[DataFlowEdgeFact] {
        &self.data_flow_edges
    }

    pub(crate) fn data_flow_models(&self) -> &[DataFlowModelFact] {
        &self.data_flow_models
    }

    pub(crate) fn data_flow_budgets(&self) -> &[DataFlowBudgetFact] {
        &self.data_flow_budgets
    }

    #[allow(dead_code)]
    pub(crate) fn data_flow_store(&self) -> Option<&DataFlowStore> {
        self.data_flow_store.as_ref()
    }

    pub(crate) fn evidence_nodes(&self) -> &[EvidenceNodeFact] {
        &self.evidence_nodes
    }

    pub(crate) fn evidence_edges(&self) -> &[EvidenceEdgeFact] {
        &self.evidence_edges
    }

    pub(crate) fn evidence_bundles(&self) -> &[EvidenceBundleFact] {
        &self.evidence_bundles
    }

    pub(crate) fn evidence_paths(&self) -> &[EvidencePathFact] {
        &self.evidence_paths
    }

    pub(crate) fn evidence_slices(&self) -> &[EvidenceSliceFact] {
        &self.evidence_slices
    }

    pub(crate) fn evidence_unknowns(&self) -> &[EvidenceUnknownFact] {
        &self.evidence_unknowns
    }

    pub(crate) fn evidence_omitted_regions(&self) -> &[EvidenceOmittedRegionFact] {
        &self.evidence_omitted_regions
    }

    pub(crate) fn evidence_replay_keys(&self) -> &[EvidenceReplayKeyFact] {
        &self.evidence_replay_keys
    }

    #[allow(dead_code)]
    pub(crate) fn evidence_store(&self) -> Option<&EvidenceStore> {
        self.evidence_store.as_ref()
    }

    pub(crate) fn abstract_domain_observations(&self) -> &[DomainObservationFact] {
        self.abstract_domain_store
            .as_ref()
            .map(DomainStore::observations)
            .unwrap_or(&[])
    }

    pub(crate) fn abstract_domain_events(&self) -> &[DomainEventFact] {
        self.abstract_domain_store
            .as_ref()
            .map(DomainStore::events)
            .unwrap_or(&[])
    }

    #[allow(dead_code)]
    pub(crate) fn abstract_domain_store(&self) -> Option<&DomainStore> {
        self.abstract_domain_store.as_ref()
    }

    pub(crate) fn replace_summary_facts(&mut self, output: SummaryOutput) {
        self.replace_summary_facts_without_metadata(output);
        self.refresh_summary_metadata();
    }

    pub(crate) fn replace_summary_facts_without_metadata(&mut self, output: SummaryOutput) {
        let store =
            SummaryStore::from_output(output).expect("summary output should produce a valid store");
        self.summary_facts.clear();
        self.summary_events.clear();
        self.summary_store = Some(store);
    }

    pub(crate) fn merge_summary_facts_without_metadata(
        &mut self,
        summaries: &[SummaryFact],
        events: &[SummaryEventFact],
    ) {
        if let Some(store) = &mut self.summary_store {
            store.merge_updates(summaries, events);
            self.summary_facts.clear();
            self.summary_events.clear();
            return;
        }

        self.replace_summary_facts_without_metadata(SummaryOutput {
            summaries: summaries.to_vec(),
            events: events.to_vec(),
        });
    }

    pub(crate) fn refresh_summary_metadata_after_bulk_update(&mut self) {
        self.refresh_summary_metadata();
    }

    #[allow(
        dead_code,
        reason = "Extension fact replacement is wired into the kernel provider in the next plan."
    )]
    pub(crate) fn replace_extension_facts(&mut self, output: ExtensionOutput) {
        let output = output.normalized();
        self.extension_activations = output.activations;
        self.extension_facts = output.accepted;
        self.rejected_extension_facts = output.rejected;
        self.refresh_extension_metadata();
    }

    pub(crate) fn summary_facts(&self) -> &[SummaryFact] {
        if let Some(store) = &self.summary_store {
            store.all_summaries()
        } else {
            &self.summary_facts
        }
    }
    pub(crate) fn summary_events(&self) -> &[SummaryEventFact] {
        if let Some(store) = &self.summary_store {
            store.all_events()
        } else {
            &self.summary_events
        }
    }

    #[allow(dead_code)]
    pub(crate) fn summary_store(&self) -> Option<&SummaryStore> {
        self.summary_store.as_ref()
    }

    pub(crate) fn extension_facts(&self) -> &[AcceptedExtensionFact] {
        &self.extension_facts
    }

    pub(crate) fn extension_activations(&self) -> &[ExtensionActivationRow] {
        &self.extension_activations
    }

    #[allow(
        dead_code,
        reason = "Rejected extension audit rows are surfaced by the extension provider/debug wiring in the next plan."
    )]
    pub(crate) fn rejected_extension_facts(&self) -> &[RejectedExtensionFact] {
        &self.rejected_extension_facts
    }

    pub(crate) fn replace_adaptation_model_facts(
        &mut self,
        accepted: Vec<AcceptedModelFact>,
        rejected: Vec<RejectedModelFact>,
    ) {
        self.adaptation_model_facts = accepted;
        self.rejected_adaptation_model_facts = rejected;
        self.refresh_adaptation_model_metadata();
    }

    pub(crate) fn adaptation_model_facts(&self) -> &[AcceptedModelFact] {
        &self.adaptation_model_facts
    }

    #[allow(
        dead_code,
        reason = "Rejected adaptation model audit rows are surfaced by eval fixture observation wiring."
    )]
    pub(crate) fn rejected_adaptation_model_facts(&self) -> &[RejectedModelFact] {
        &self.rejected_adaptation_model_facts
    }

    pub(crate) fn replace_entrypoint_facts(
        &mut self,
        output: EntrypointOutput,
    ) -> Result<(), AnalysisError> {
        let store = EntrypointStore::from_output(output)?;
        self.entrypoint_facts = store.entrypoints().to_vec();
        self.trust_boundary_facts = store.trust_boundaries().to_vec();
        self.dispatch_edge_facts = store.dispatch_edges().to_vec();
        self.unresolved_framework_facts = store.unresolved().to_vec();
        self.entrypoint_store = Some(store);
        self.refresh_entrypoint_metadata();
        Ok(())
    }

    pub(crate) fn entrypoint_facts(&self) -> &[EntrypointFact] {
        &self.entrypoint_facts
    }

    #[allow(
        dead_code,
        reason = "Reachability fact replacement is wired into the kernel provider in the next  task (provider/kernel splice)."
    )]
    pub(crate) fn replace_reachability_facts(
        &mut self,
        output: ReachabilityProviderOutput,
    ) -> Result<(), AnalysisError> {
        let valid_function_ids = self.functions().iter().map(|row| row.id).collect();
        let valid_entrypoint_ids = self.entrypoint_facts.iter().map(|row| row.id).collect();
        let store =
            ReachabilityStore::from_output(output, &valid_function_ids, &valid_entrypoint_ids)?;
        self.reachability_roots = store.roots().to_vec();
        self.reachability_marks = store.marks().to_vec();
        Ok(())
    }

    #[allow(
        dead_code,
        reason = "Reachability roots are consumed by validation, debug, and the kernel provider wiring in ."
    )]
    pub(crate) fn reachability_roots(&self) -> &[ReachabilityRootFact] {
        &self.reachability_roots
    }

    #[allow(
        dead_code,
        reason = "Reachability marks are populated by the marking traversal in  and read by debug/eval."
    )]
    pub(crate) fn reachability_marks(&self) -> &[CallReachabilityFact] {
        &self.reachability_marks
    }

    /// Stores the normalized semantic-graph nodes/edges/constraints (GRAPH-01),
    /// mirroring [`Self::replace_reachability_facts`]. Construction runs through
    /// [`SemanticGraphStore::from_output`], which normalizes (stable-key sort + dense
    /// ID assignment) and referentially validates every edge endpoint and constraint
    /// node reference — a dangling reference returns [`AnalysisError::InvalidFact`] so
    /// the db is never left holding a malformed graph.
    #[allow(
        dead_code,
        reason = "Provider hot paths pass normalized output directly; tests and compatibility callers still use the normalizing entry point."
    )]
    pub(crate) fn replace_semantic_graph_facts(
        &mut self,
        output: SemanticGraphOutput,
    ) -> Result<(), AnalysisError> {
        self.replace_normalized_semantic_graph_facts(output.normalized())
    }

    pub(crate) fn replace_normalized_semantic_graph_facts(
        &mut self,
        output: SemanticGraphOutput,
    ) -> Result<(), AnalysisError> {
        output.validate_references()?;
        self.semantic_nodes = output.nodes;
        self.semantic_edges = output.edges;
        self.semantic_constraints = output.constraints;
        Ok(())
    }

    pub(crate) fn semantic_nodes(&self) -> &[SemanticNodeFact] {
        &self.semantic_nodes
    }

    pub(crate) fn semantic_edges(&self) -> &[SemanticEdgeFact] {
        &self.semantic_edges
    }

    pub(crate) fn semantic_constraints(&self) -> &[ConstraintFact] {
        &self.semantic_constraints
    }

    /// Stores the private TS object/property/prototype/receiver rows used by the
    /// current semantic-graph lowering. Construction runs through
    /// [`TsObjectModelStore::try_from_output`], which preserves deterministic
    /// normalization and rejects duplicate stable keys before stale rows are replaced.
    #[allow(
        dead_code,
        reason = " stores TS object-model rows before semantic-graph lowering consumes them."
    )]
    pub(crate) fn replace_ts_object_model_facts(
        &mut self,
        output: TsObjectModelOutput,
    ) -> Result<(), AnalysisError> {
        let store = TsObjectModelStore::try_from_output(output)?;
        self.ts_object_allocations = store.allocations().to_vec();
        self.ts_property_writes = store.property_writes().to_vec();
        self.ts_property_reads = store.property_reads().to_vec();
        self.ts_receiver_bindings = store.receiver_bindings().to_vec();
        self.ts_prototype_links = store.prototype_links().to_vec();
        self.ts_object_model_store = Some(store);
        Ok(())
    }

    #[allow(
        dead_code,
        reason = " stores TS object-model rows before semantic-graph lowering consumes them."
    )]
    pub(crate) fn ts_object_allocations(&self) -> &[TsObjectAllocationFact] {
        &self.ts_object_allocations
    }

    #[allow(
        dead_code,
        reason = " stores TS object-model rows before semantic-graph lowering consumes them."
    )]
    pub(crate) fn ts_property_writes(&self) -> &[TsPropertyWriteFact] {
        &self.ts_property_writes
    }

    #[allow(
        dead_code,
        reason = " stores TS object-model rows before semantic-graph lowering consumes them."
    )]
    pub(crate) fn ts_property_reads(&self) -> &[TsPropertyReadFact] {
        &self.ts_property_reads
    }

    #[allow(
        dead_code,
        reason = " stores TS object-model rows before semantic-graph lowering consumes them."
    )]
    pub(crate) fn ts_receiver_bindings(&self) -> &[TsReceiverBindingFact] {
        &self.ts_receiver_bindings
    }

    #[allow(
        dead_code,
        reason = " stores TS object-model rows before semantic-graph lowering consumes them."
    )]
    pub(crate) fn ts_prototype_links(&self) -> &[TsPrototypeLinkFact] {
        &self.ts_prototype_links
    }

    #[allow(
        dead_code,
        reason = " stores TS object-model rows before semantic-graph lowering consumes them."
    )]
    pub(crate) fn ts_object_model_store(&self) -> Option<&TsObjectModelStore> {
        self.ts_object_model_store.as_ref()
    }

    /// Stores the normalized solver-derived edges (GRAPH-03/GRAPH-04), mirroring
    /// [`Self::replace_semantic_graph_facts`]. Construction runs through
    /// [`SolverStore::from_output`], which normalizes (stable-key sort + dense ID
    /// assignment) and referentially validates duplicate stable keys + the precision
    /// ceiling (D-06) — a malformed row returns [`AnalysisError::InvalidFact`] so the
    /// db is never left holding a malformed solver output.
    pub(crate) fn replace_solver_facts(
        &mut self,
        output: SolverOutput,
    ) -> Result<(), AnalysisError> {
        let store = SolverStore::from_output(output)?;
        self.solver_derived_edges = store.derived_edges().to_vec();
        self.solver_budget_status = store.budget_status();
        self.solver_budget_reasons = store.budget_reasons().clone();
        Ok(())
    }

    /// The stored solver-derived edges. Consumed by the provider tests today and by
    /// the GRAPH-05 refined_calls rework (which projects over solver output);
    /// no production read exists yet, so the accessor is dead-code in a non-test build
    /// until that consumer lands (the facts are stored unconditionally so the
    /// determinism gate observes them).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn solver_derived_edges(&self) -> &[DerivedEdgeFact] {
        &self.solver_derived_edges
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn solver_budget_status(&self) -> BudgetStatus {
        self.solver_budget_status
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn solver_budget_reasons(&self) -> &BTreeSet<String> {
        &self.solver_budget_reasons
    }

    /// Store the Go semantic facts, returning the resilience report (malformed RTA-signal
    /// harvest rows dropped, FIX 3; plus duplicate structural rows collapsed keep-first,
    /// FIX-08) so the provider can surface observable diagnostics. All counts are zero on a
    /// clean frontend run.
    pub(crate) fn replace_go_semantic_facts(
        &mut self,
        output: GoSemanticFactsOutput,
    ) -> Result<GoSemanticStoreReport, AnalysisError> {
        let store = GoSemanticStore::from_output(output)?;
        let report = store.report();
        *self.go_semantic_store_mut() = store;
        Ok(report)
    }

    /// The normalized Go semantic output currently stored in the database.
    ///
    /// Used by the provider after `replace_go_semantic_facts` so its output digest certifies
    /// the rows that survived store-time resilience passes (invalid harvest-row drops and
    /// duplicate structural-key collapse), not the raw sidecar/lowering rows.
    pub(crate) fn go_semantic_facts_output(&self) -> GoSemanticFactsOutput {
        self.go_semantic_store().output().clone()
    }

    pub(crate) fn go_semantic_packages(&self) -> &[GoSemanticPackageFact] {
        &self.go_semantic_store().output().packages
    }

    pub(crate) fn go_semantic_functions(&self) -> &[GoSemanticFunctionFact] {
        &self.go_semantic_store().output().functions
    }

    pub(crate) fn go_semantic_callsites(&self) -> &[GoSemanticCallsiteFact] {
        &self.go_semantic_store().output().callsites
    }

    #[allow(
        dead_code,
        reason = "Method-set facts are stored privately for receiver/RTA expansion."
    )]
    pub(crate) fn go_semantic_method_sets(&self) -> &[GoSemanticMethodSetFact] {
        &self.go_semantic_store().output().method_sets
    }

    #[allow(
        dead_code,
        reason = "Address-taken facts are stored privately for the Plan 2 go_rta dispatch-candidate set (GO-05)."
    )]
    pub(crate) fn go_semantic_address_taken(&self) -> &[GoSemanticAddressTakenFact] {
        &self.go_semantic_store().output().address_taken
    }

    #[allow(
        dead_code,
        reason = "Instantiated-type facts are stored privately for the Plan 2 go_rta rapid-type filter (GO-05)."
    )]
    pub(crate) fn go_semantic_instantiated_types(&self) -> &[GoSemanticInstantiatedTypeFact] {
        &self.go_semantic_store().output().instantiated_types
    }

    #[allow(
        dead_code,
        reason = "Dynamic-dispatch detail is stored privately for the Plan 2 go_rta method-set matching (GO-05)."
    )]
    pub(crate) fn go_semantic_dynamic_dispatch(&self) -> &[GoSemanticDynamicDispatchFact] {
        &self.go_semantic_store().output().dynamic_dispatch
    }

    #[cfg(test)]
    pub(crate) fn go_semantic_rta_edges(
        &self,
    ) -> &[crate::go::semantic::facts::GoSemanticRtaEdgeFact] {
        &self.go_semantic_store().output().rta_edges
    }

    #[allow(
        dead_code,
        reason = "Package-load errors are stored privately for capability diagnostics once the provider is kernel-wired."
    )]
    pub(crate) fn go_semantic_package_errors(&self) -> &[GoSemanticPackageErrorFact] {
        &self.go_semantic_store().output().package_errors
    }

    pub(crate) fn trust_boundary_facts(&self) -> &[TrustBoundaryFact] {
        &self.trust_boundary_facts
    }

    pub(crate) fn dispatch_edge_facts(&self) -> &[FrameworkDispatchEdgeFact] {
        &self.dispatch_edge_facts
    }

    pub(crate) fn unresolved_framework_facts(&self) -> &[UnresolvedFrameworkFact] {
        &self.unresolved_framework_facts
    }

    #[allow(
        dead_code,
        reason = "Compatibility callers can still pass unnormalized aggregate output; providers use the normalized fast path."
    )]
    pub(crate) fn replace_type_value_alias_facts(&mut self, output: TypeValueAliasOutput) {
        self.replace_normalized_type_value_alias_facts(output.normalized());
    }

    pub(crate) fn replace_normalized_type_value_alias_facts(
        &mut self,
        output: TypeValueAliasOutput,
    ) {
        let type_store = TypeStore::from_normalized_output(output.types);
        let value_store = ValueStore::from_normalized_output(output.values);
        let access_path_store = AccessPathStore::from_normalized_output(output.access_paths);
        let points_to_store = PointsToStore::from_normalized_output(output.points_to);
        let alias_store = AliasStore::from_normalized_output(output.aliases);

        self.type_facts = type_store.types().to_vec();
        self.narrowed_type_facts = type_store.narrowed().to_vec();
        self.value_facts = value_store.values().to_vec();
        self.allocation_tokens = value_store.allocations().to_vec();
        self.access_path_facts = access_path_store.access_paths().to_vec();
        self.points_to_constraints = points_to_store.constraints().to_vec();
        self.points_to_sets = points_to_store.sets().to_vec();
        self.alias_answers = alias_store.answers().to_vec();
        self.type_store = Some(type_store);
        self.value_store = Some(value_store);
        self.access_path_store = Some(access_path_store);
        self.points_to_store = Some(points_to_store);
        self.alias_store = Some(alias_store);
        self.refresh_type_value_alias_metadata();
    }

    pub(crate) fn type_facts(&self) -> &[TypeFact] {
        &self.type_facts
    }

    #[allow(dead_code)]
    pub(crate) fn narrowed_type_facts(&self) -> &[NarrowedTypeFact] {
        &self.narrowed_type_facts
    }

    pub(crate) fn value_facts(&self) -> &[ValueFact] {
        &self.value_facts
    }

    #[allow(dead_code)]
    pub(crate) fn allocation_tokens(&self) -> &[AllocationTokenFact] {
        &self.allocation_tokens
    }

    pub(crate) fn access_path_facts(&self) -> &[AccessPathFact] {
        &self.access_path_facts
    }

    #[allow(dead_code)]
    pub(crate) fn points_to_constraints(&self) -> &[PointsToConstraintFact] {
        &self.points_to_constraints
    }

    pub(crate) fn points_to_sets(&self) -> &[PointsToSetFact] {
        &self.points_to_sets
    }

    pub(crate) fn alias_answers(&self) -> &[AliasAnswerFact] {
        &self.alias_answers
    }

    #[allow(dead_code)]
    pub(crate) fn call_sites_by_caller(&self, caller: FunctionId) -> Vec<&CallSiteFact> {
        self.calls_store().sites_by_caller(caller)
    }

    #[allow(dead_code)]
    pub(crate) fn call_targets_by_site(&self, site: CallSiteId) -> Vec<&CallTargetFact> {
        self.calls_store().targets_by_site(site)
    }

    #[allow(dead_code)]
    pub(crate) fn outgoing_calls_by_function(&self, caller: FunctionId) -> Vec<&CallTargetFact> {
        self.calls_store().outgoing_by_function(caller)
    }

    #[allow(dead_code)]
    pub(crate) fn outgoing_calls_by_symbol(&self, caller: SymbolId) -> Vec<&CallTargetFact> {
        self.calls_store().outgoing_by_symbol(caller)
    }

    #[allow(dead_code)]
    pub(crate) fn incoming_calls_by_symbol(&self, target: SymbolId) -> Vec<&CallTargetFact> {
        self.calls_store().incoming_by_symbol(target)
    }

    #[allow(dead_code)]
    pub(crate) fn incoming_calls_by_function(&self, target: FunctionId) -> Vec<&CallTargetFact> {
        self.calls_store().incoming_by_function(target)
    }

    #[allow(dead_code)]
    pub(crate) fn unresolved_calls_by_reason(
        &self,
        reason: UnresolvedCallReason,
    ) -> Vec<&UnresolvedCallFact> {
        self.calls_store().unresolved_by_reason(reason)
    }

    #[allow(dead_code)]
    pub(crate) fn unresolved_calls_by_status(
        &self,
        status: CallTargetStatus,
    ) -> Vec<&UnresolvedCallFact> {
        self.calls_store().unresolved_by_status(status)
    }

    fn refresh_call_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::CallSite);
        self.fact_meta.remove_family(FactFamily::CallTarget);
        self.fact_meta.remove_family(FactFamily::UnresolvedCall);

        let call_sites = self.call_sites().to_vec();
        let call_targets = self.call_targets().to_vec();
        let unresolved_calls = self.unresolved_calls().to_vec();

        for fact in &call_sites {
            let metadata = self.call_site_metadata(fact);
            self.record_fact_meta(FactFamily::CallSite, fact.id.0, metadata);
        }

        for fact in &call_targets {
            let metadata = self.call_target_metadata(fact);
            self.record_fact_meta(FactFamily::CallTarget, fact.id.0, metadata);
        }

        for (index, fact) in unresolved_calls.iter().enumerate() {
            let metadata = self.unresolved_call_metadata(fact);
            self.record_fact_meta(FactFamily::UnresolvedCall, index as u64, metadata);
        }

        self.finish_fact_meta_insertions(&[
            FactFamily::CallSite,
            FactFamily::CallTarget,
            FactFamily::UnresolvedCall,
        ]);
    }

    fn refresh_refined_call_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::RefinedCallEdge);

        if let Some(store) = self.refined_call_store.take() {
            for fact in store.edges() {
                let run_id = fact.id.0;
                let metadata = self.refined_call_edge_metadata(fact);
                self.record_fact_meta(FactFamily::RefinedCallEdge, run_id, metadata);
            }
            self.refined_call_store = Some(store);
            self.finish_fact_meta_insertions(&[FactFamily::RefinedCallEdge]);
            return;
        }

        for index in 0..self.refined_call_edges.len() {
            let (run_id, metadata) = {
                let fact = &self.refined_call_edges[index];
                (fact.id.0, self.refined_call_edge_metadata(fact))
            };
            self.record_fact_meta(FactFamily::RefinedCallEdge, run_id, metadata);
        }
        self.finish_fact_meta_insertions(&[FactFamily::RefinedCallEdge]);
    }

    fn refresh_data_flow_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::DataFlowNode);
        self.fact_meta.remove_family(FactFamily::DataFlowEdge);
        self.fact_meta.remove_family(FactFamily::DataFlowModel);
        self.fact_meta.remove_family(FactFamily::DataFlowBudget);

        let node_metadata = self
            .data_flow_nodes
            .iter()
            .map(|fact| (fact.id.0, self.data_flow_node_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in node_metadata {
            self.record_fact_meta(FactFamily::DataFlowNode, run_id, metadata);
        }

        let edge_metadata = self
            .data_flow_edges
            .iter()
            .map(|fact| (fact.id.0, self.data_flow_edge_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in edge_metadata {
            self.record_fact_meta(FactFamily::DataFlowEdge, run_id, metadata);
        }

        let model_metadata = self
            .data_flow_models
            .iter()
            .map(|fact| (fact.id.0, self.data_flow_model_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in model_metadata {
            self.record_fact_meta(FactFamily::DataFlowModel, run_id, metadata);
        }

        let budget_metadata = self
            .data_flow_budgets
            .iter()
            .map(|fact| (fact.id.0, self.data_flow_budget_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in budget_metadata {
            self.record_fact_meta(FactFamily::DataFlowBudget, run_id, metadata);
        }

        self.finish_fact_meta_insertions(&[
            FactFamily::DataFlowNode,
            FactFamily::DataFlowEdge,
            FactFamily::DataFlowModel,
            FactFamily::DataFlowBudget,
        ]);
    }

    fn refresh_evidence_metadata(&mut self) {
        for family in [
            FactFamily::EvidenceNode,
            FactFamily::EvidenceEdge,
            FactFamily::EvidenceBundle,
            FactFamily::EvidencePath,
            FactFamily::EvidenceSlice,
            FactFamily::EvidenceUnknown,
            FactFamily::EvidenceOmittedRegion,
            FactFamily::EvidenceReplayKey,
        ] {
            self.fact_meta.remove_family(family);
        }

        let node_metadata = self
            .evidence_nodes
            .iter()
            .map(|fact| (fact.id.0, self.evidence_node_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in node_metadata {
            self.record_fact_meta(FactFamily::EvidenceNode, run_id, metadata);
        }

        let edge_metadata = self
            .evidence_edges
            .iter()
            .map(|fact| (fact.id.0, self.evidence_edge_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in edge_metadata {
            self.record_fact_meta(FactFamily::EvidenceEdge, run_id, metadata);
        }

        let bundle_metadata = self
            .evidence_bundles
            .iter()
            .map(|fact| (fact.id.0, self.evidence_bundle_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in bundle_metadata {
            self.record_fact_meta(FactFamily::EvidenceBundle, run_id, metadata);
        }

        let path_metadata = self
            .evidence_paths
            .iter()
            .map(|fact| (fact.id.0, self.evidence_path_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in path_metadata {
            self.record_fact_meta(FactFamily::EvidencePath, run_id, metadata);
        }

        let slice_metadata = self
            .evidence_slices
            .iter()
            .map(|fact| (fact.id.0, self.evidence_slice_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in slice_metadata {
            self.record_fact_meta(FactFamily::EvidenceSlice, run_id, metadata);
        }

        let unknown_metadata = self
            .evidence_unknowns
            .iter()
            .enumerate()
            .map(|(index, fact)| (index as u64, self.evidence_unknown_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in unknown_metadata {
            self.record_fact_meta(FactFamily::EvidenceUnknown, run_id, metadata);
        }

        let omitted_metadata = self
            .evidence_omitted_regions
            .iter()
            .map(|fact| (fact.id.0, self.evidence_omitted_region_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in omitted_metadata {
            self.record_fact_meta(FactFamily::EvidenceOmittedRegion, run_id, metadata);
        }

        let replay_metadata = self
            .evidence_replay_keys
            .iter()
            .enumerate()
            .map(|(index, fact)| (index as u64, self.evidence_replay_key_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in replay_metadata {
            self.record_fact_meta(FactFamily::EvidenceReplayKey, run_id, metadata);
        }

        self.finish_fact_meta_insertions(&[
            FactFamily::EvidenceNode,
            FactFamily::EvidenceEdge,
            FactFamily::EvidenceBundle,
            FactFamily::EvidencePath,
            FactFamily::EvidenceSlice,
            FactFamily::EvidenceUnknown,
            FactFamily::EvidenceOmittedRegion,
            FactFamily::EvidenceReplayKey,
        ]);
    }
    fn refresh_abstract_domain_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::DomainObservation);
        self.fact_meta.remove_family(FactFamily::DomainEvent);

        let Some(store) = self.abstract_domain_store.take() else {
            return;
        };

        for fact in store.observations() {
            let run_id = fact.id.0;
            let metadata = self.domain_observation_metadata(fact);
            self.record_fact_meta(FactFamily::DomainObservation, run_id, metadata);
        }

        for fact in store.events() {
            let run_id = fact.id.0;
            let metadata = self.domain_event_metadata(fact);
            self.record_fact_meta(FactFamily::DomainEvent, run_id, metadata);
        }

        self.abstract_domain_store = Some(store);
        self.finish_fact_meta_insertions(&[FactFamily::DomainObservation, FactFamily::DomainEvent]);
    }

    fn refresh_summary_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::SummaryControl);
        self.fact_meta.remove_family(FactFamily::SummaryCall);
        self.fact_meta.remove_family(FactFamily::SummaryMemory);
        self.fact_meta.remove_family(FactFamily::SummaryTito);
        self.fact_meta.remove_family(FactFamily::SummaryEvent);

        if let Some(store) = self.summary_store.take() {
            for fact in store.all_summaries() {
                let family = summary_domain_to_fact_family(fact.domain);
                let run_id = fact.id.0;
                let metadata = self.summary_fact_metadata(fact);
                self.record_fact_meta(family, run_id, metadata);
            }

            for fact in store.all_events() {
                let run_id = fact.id.0;
                let metadata = self.summary_event_metadata(fact);
                self.record_fact_meta(FactFamily::SummaryEvent, run_id, metadata);
            }

            self.summary_store = Some(store);
            self.finish_fact_meta_insertions(&[
                FactFamily::SummaryControl,
                FactFamily::SummaryCall,
                FactFamily::SummaryMemory,
                FactFamily::SummaryTito,
                FactFamily::SummaryEvent,
            ]);
            return;
        }

        for index in 0..self.summary_facts.len() {
            let (family, run_id, metadata) = {
                let fact = &self.summary_facts[index];
                (
                    summary_domain_to_fact_family(fact.domain),
                    fact.id.0,
                    self.summary_fact_metadata(fact),
                )
            };
            self.record_fact_meta(family, run_id, metadata);
        }

        for index in 0..self.summary_events.len() {
            let (run_id, metadata) = {
                let fact = &self.summary_events[index];
                (fact.id.0, self.summary_event_metadata(fact))
            };
            self.record_fact_meta(FactFamily::SummaryEvent, run_id, metadata);
        }
        self.finish_fact_meta_insertions(&[
            FactFamily::SummaryControl,
            FactFamily::SummaryCall,
            FactFamily::SummaryMemory,
            FactFamily::SummaryTito,
            FactFamily::SummaryEvent,
        ]);
    }

    #[allow(
        dead_code,
        reason = "Extension metadata refresh is reached through extension provider wiring in the next plan."
    )]
    fn refresh_extension_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::ExtensionFact);
        let metadata = self
            .extension_facts
            .iter()
            .enumerate()
            .map(|(index, fact)| (index as u64, extension_fact_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in metadata {
            self.record_fact_meta(FactFamily::ExtensionFact, run_id, metadata);
        }
        self.finish_fact_meta_insertions(&[FactFamily::ExtensionFact]);
    }

    fn refresh_adaptation_model_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::AdaptationModel);
        let metadata = self
            .adaptation_model_facts
            .iter()
            .enumerate()
            .map(|(index, fact)| (index as u64, adaptation_model_fact_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in metadata {
            self.record_fact_meta(FactFamily::AdaptationModel, run_id, metadata);
        }
        self.finish_fact_meta_insertions(&[FactFamily::AdaptationModel]);
    }

    fn refresh_entrypoint_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::Entrypoint);
        self.fact_meta.remove_family(FactFamily::TrustBoundary);
        self.fact_meta.remove_family(FactFamily::DispatchEdge);
        self.fact_meta
            .remove_family(FactFamily::UnresolvedFramework);

        let entrypoint_metadata = self
            .entrypoint_facts
            .iter()
            .map(|fact| (fact.id.0, self.entrypoint_fact_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in entrypoint_metadata {
            self.record_fact_meta(FactFamily::Entrypoint, run_id, metadata);
        }

        let trust_boundary_metadata = self
            .trust_boundary_facts
            .iter()
            .map(|fact| (fact.id.0, self.trust_boundary_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in trust_boundary_metadata {
            self.record_fact_meta(FactFamily::TrustBoundary, run_id, metadata);
        }

        let dispatch_edge_metadata = self
            .dispatch_edge_facts
            .iter()
            .map(|fact| (fact.id.0, self.dispatch_edge_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in dispatch_edge_metadata {
            self.record_fact_meta(FactFamily::DispatchEdge, run_id, metadata);
        }

        let unresolved_metadata = self
            .unresolved_framework_facts
            .iter()
            .map(|fact| (fact.id.0, self.unresolved_framework_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in unresolved_metadata {
            self.record_fact_meta(FactFamily::UnresolvedFramework, run_id, metadata);
        }
        self.finish_fact_meta_insertions(&[
            FactFamily::Entrypoint,
            FactFamily::TrustBoundary,
            FactFamily::DispatchEdge,
            FactFamily::UnresolvedFramework,
        ]);
    }

    fn refresh_type_value_alias_metadata(&mut self) {
        for family in [
            FactFamily::Type,
            FactFamily::NarrowedType,
            FactFamily::Value,
            FactFamily::AllocationToken,
            FactFamily::AccessPath,
            FactFamily::PointsToConstraint,
            FactFamily::PointsToSet,
            FactFamily::AliasAnswer,
        ] {
            self.fact_meta.remove_family(family);
        }

        for index in 0..self.type_facts.len() {
            let (run_id, metadata) = {
                let fact = &self.type_facts[index];
                (fact.id.0, self.type_fact_metadata(fact))
            };
            self.record_fact_meta(FactFamily::Type, run_id, metadata);
        }

        for index in 0..self.narrowed_type_facts.len() {
            let (run_id, metadata) = {
                let fact = &self.narrowed_type_facts[index];
                (fact.id.0, self.narrowed_type_metadata(fact))
            };
            self.record_fact_meta(FactFamily::NarrowedType, run_id, metadata);
        }

        for index in 0..self.value_facts.len() {
            let (run_id, metadata) = {
                let fact = &self.value_facts[index];
                (fact.id.0, self.value_fact_metadata(fact))
            };
            self.record_fact_meta(FactFamily::Value, run_id, metadata);
        }

        for index in 0..self.allocation_tokens.len() {
            let (run_id, metadata) = {
                let fact = &self.allocation_tokens[index];
                (fact.id.0, self.allocation_token_metadata(fact))
            };
            self.record_fact_meta(FactFamily::AllocationToken, run_id, metadata);
        }

        for index in 0..self.access_path_facts.len() {
            let (run_id, metadata) = {
                let fact = &self.access_path_facts[index];
                (fact.id.0, self.access_path_metadata(fact))
            };
            self.record_fact_meta(FactFamily::AccessPath, run_id, metadata);
        }

        for index in 0..self.points_to_constraints.len() {
            let (run_id, metadata) = {
                let fact = &self.points_to_constraints[index];
                (fact.id.0, self.points_to_constraint_metadata(fact))
            };
            self.record_fact_meta(FactFamily::PointsToConstraint, run_id, metadata);
        }

        for index in 0..self.points_to_sets.len() {
            let (run_id, metadata) = {
                let fact = &self.points_to_sets[index];
                (fact.id.0, self.points_to_set_metadata(fact))
            };
            self.record_fact_meta(FactFamily::PointsToSet, run_id, metadata);
        }

        for index in 0..self.alias_answers.len() {
            let (run_id, metadata) = {
                let fact = &self.alias_answers[index];
                (fact.id.0, self.alias_answer_metadata(fact))
            };
            self.record_fact_meta(FactFamily::AliasAnswer, run_id, metadata);
        }

        self.finish_fact_meta_insertions(&[
            FactFamily::Type,
            FactFamily::NarrowedType,
            FactFamily::Value,
            FactFamily::AllocationToken,
            FactFamily::AccessPath,
            FactFamily::PointsToConstraint,
            FactFamily::PointsToSet,
            FactFamily::AliasAnswer,
        ]);
    }

    fn type_fact_metadata(&self, fact: &TypeFact) -> FactMeta {
        let (precision, confidence) =
            type_metadata_precision(fact.status, fact.precision, Some(fact.confidence));
        fact_meta_from_stable_key(
            FactFamily::Type,
            TYPE_VALUE_ALIAS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", format!("{:?}", fact.status)),
                ("precision", format!("{:?}", fact.precision)),
                ("phase", format!("{:?}", fact.phase)),
                ("language", language_label(fact.language).to_string()),
                ("file_key", self.option_source_file_key(fact.file)),
                (
                    "place_key",
                    fact.place
                        .map(|place| self.fact_stable_key(FactFamily::Place, place.0))
                        .unwrap_or_else(none_value),
                ),
                ("subject", format!("{:?}", fact.subject)),
                ("shape", format!("{:?}", fact.shape)),
                ("provenance", format!("{:?}", fact.provenance)),
            ]),
        )
    }

    fn narrowed_type_metadata(&self, fact: &NarrowedTypeFact) -> FactMeta {
        let (precision, confidence) = type_metadata_precision(fact.status, fact.precision, None);
        fact_meta_from_stable_key(
            FactFamily::NarrowedType,
            TYPE_VALUE_ALIAS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", format!("{:?}", fact.status)),
                ("precision", format!("{:?}", fact.precision)),
                ("language", language_label(fact.language).to_string()),
                (
                    "place_key",
                    self.fact_stable_key(FactFamily::Place, fact.place.0),
                ),
                (
                    "operation_key",
                    fact.operation
                        .map(|operation| {
                            self.fact_stable_key(FactFamily::MirOperation, operation.0)
                        })
                        .unwrap_or_else(none_value),
                ),
                ("evidence", fact.evidence.clone()),
            ]),
        )
    }

    fn value_fact_metadata(&self, fact: &ValueFact) -> FactMeta {
        let (precision, confidence) = value_metadata_precision(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::Value,
            TYPE_VALUE_ALIAS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", format!("{:?}", fact.status)),
                ("precision", format!("{:?}", fact.precision)),
                ("language", language_label(fact.language).to_string()),
                ("subject", format!("{:?}", fact.subject)),
                ("kind", format!("{:?}", fact.kind)),
                ("provenance", format!("{:?}", fact.provenance)),
            ]),
        )
    }

    fn allocation_token_metadata(&self, fact: &AllocationTokenFact) -> FactMeta {
        fact_meta_from_stable_key(
            FactFamily::AllocationToken,
            TYPE_VALUE_ALIAS_PROVIDER_ID,
            FactPrecision::SetupAware,
            FactConfidence::Medium,
            fact.stable_key.clone(),
            stable_parts([
                ("kind", format!("{:?}", fact.kind)),
                ("language", language_label(fact.language).to_string()),
                (
                    "source_place",
                    fact.source_place
                        .map(|place| self.fact_stable_key(FactFamily::Place, place.0))
                        .unwrap_or_else(none_value),
                ),
                ("provenance", format!("{:?}", fact.provenance)),
            ]),
        )
    }

    fn access_path_metadata(&self, fact: &AccessPathFact) -> FactMeta {
        let precision = match fact.status {
            crate::analysis::access_paths::facts::AccessPathStatus::Resolved => {
                FactPrecision::SetupAware
            }
            crate::analysis::access_paths::facts::AccessPathStatus::Partial => {
                FactPrecision::Ambiguous
            }
            crate::analysis::access_paths::facts::AccessPathStatus::Unknown => {
                FactPrecision::Unresolved
            }
            crate::analysis::access_paths::facts::AccessPathStatus::Unsupported => {
                FactPrecision::Unsupported
            }
            crate::analysis::access_paths::facts::AccessPathStatus::BudgetExceeded => {
                FactPrecision::Heuristic
            }
        };
        fact_meta_from_stable_key(
            FactFamily::AccessPath,
            TYPE_VALUE_ALIAS_PROVIDER_ID,
            precision,
            FactConfidence::Medium,
            fact.stable_key.clone(),
            stable_parts([
                ("status", format!("{:?}", fact.status)),
                ("language", language_label(fact.language).to_string()),
                (
                    "base_key",
                    self.fact_stable_key(FactFamily::Place, fact.base.0),
                ),
                ("projection_count", fact.projections.len().to_string()),
            ]),
        )
    }

    fn points_to_constraint_metadata(&self, fact: &PointsToConstraintFact) -> FactMeta {
        let (precision, confidence) = points_to_metadata_precision(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::PointsToConstraint,
            TYPE_VALUE_ALIAS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", format!("{:?}", fact.status)),
                ("precision", format!("{:?}", fact.precision)),
                ("kind", format!("{:?}", fact.kind)),
            ]),
        )
    }

    fn points_to_set_metadata(&self, fact: &PointsToSetFact) -> FactMeta {
        let (precision, confidence) = points_to_metadata_precision(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::PointsToSet,
            TYPE_VALUE_ALIAS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", format!("{:?}", fact.status)),
                ("precision", format!("{:?}", fact.precision)),
                ("budget", format!("{:?}", fact.budget)),
                ("variable", format!("{:?}", fact.variable)),
                ("objects", format!("{:?}", fact.objects)),
            ]),
        )
    }

    fn alias_answer_metadata(&self, fact: &AliasAnswerFact) -> FactMeta {
        let (precision, confidence) = alias_metadata_precision(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::AliasAnswer,
            TYPE_VALUE_ALIAS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", format!("{:?}", fact.status)),
                ("precision", format!("{:?}", fact.precision)),
                ("reason", format!("{:?}", fact.reason)),
                ("left", format!("{:?}", fact.left)),
                ("right", format!("{:?}", fact.right)),
                ("evidence", fact.evidence.join("\n")),
            ]),
        )
    }

    fn entrypoint_fact_metadata(&self, fact: &EntrypointFact) -> FactMeta {
        let (precision, confidence) = entrypoint_precision_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::Entrypoint,
            ENTRYPOINTS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", format!("{:?}", fact.status)),
                ("precision", format!("{:?}", fact.precision)),
                ("kind", format!("{:?}", fact.kind)),
                ("language", language_label(fact.language).to_string()),
                ("framework", fact.framework_id.clone()),
                ("file_key", self.source_file_key(fact.registration_file)),
                (
                    "function_key",
                    self.function_key(
                        fact.target_function,
                        &fact.framework_id,
                        &fact.registration_span,
                    ),
                ),
                ("provenance", format!("{:?}", fact.provenance)),
            ]),
        )
    }

    fn trust_boundary_metadata(&self, fact: &TrustBoundaryFact) -> FactMeta {
        let (precision, confidence) =
            entrypoint_precision_metadata(EntrypointStatus::Resolved, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::TrustBoundary,
            ENTRYPOINTS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("source_kind", format!("{:?}", fact.source_kind)),
                ("entrypoint_key", fact.entrypoint_stable_key.clone()),
                ("precision", format!("{:?}", fact.precision)),
                ("language", language_label(fact.language).to_string()),
                ("file_key", self.source_file_key(fact.file)),
            ]),
        )
    }

    fn dispatch_edge_metadata(&self, fact: &FrameworkDispatchEdgeFact) -> FactMeta {
        let (precision, confidence) =
            entrypoint_precision_metadata(EntrypointStatus::Resolved, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::DispatchEdge,
            ENTRYPOINTS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("edge_kind", format!("{:?}", fact.edge_kind)),
                ("from_source", fact.from_source.clone()),
                ("precision", format!("{:?}", fact.precision)),
                ("language", language_label(fact.language).to_string()),
                ("file_key", self.source_file_key(fact.file)),
            ]),
        )
    }

    fn unresolved_framework_metadata(&self, fact: &UnresolvedFrameworkFact) -> FactMeta {
        fact_meta_from_stable_key(
            FactFamily::UnresolvedFramework,
            ENTRYPOINTS_PROVIDER_ID,
            FactPrecision::SetupAware,
            FactConfidence::Medium,
            fact.stable_key.clone(),
            stable_parts([
                ("reason", format!("{:?}", fact.reason)),
                ("framework", fact.framework_id.clone()),
                ("precision", format!("{:?}", fact.precision)),
                ("language", language_label(fact.language).to_string()),
                ("file_key", self.source_file_key(fact.file)),
            ]),
        )
    }

    fn summary_fact_metadata(&self, fact: &SummaryFact) -> FactMeta {
        let (precision, confidence) = summary_precision_metadata(fact.status, fact.precision);
        FactMeta {
            stable_key: fact.stable_key.clone(),
            producer_id: POLINT_DIRECT_SUMMARIES_PROVIDER_ID,
            layer_id: POLINT_DIRECT_SUMMARIES_PROVIDER_ID,
            precision,
            confidence,
            validation: ValidationStatus::NativeTrusted,
            payload_digest: summary_fact_payload_metadata_digest(fact),
        }
    }

    fn summary_event_metadata(&self, fact: &SummaryEventFact) -> FactMeta {
        let (precision, confidence) = summary_precision_metadata(fact.status, fact.precision);
        FactMeta {
            stable_key: fact.stable_key.clone(),
            producer_id: POLINT_DIRECT_SUMMARIES_PROVIDER_ID,
            layer_id: POLINT_DIRECT_SUMMARIES_PROVIDER_ID,
            precision,
            confidence,
            validation: ValidationStatus::NativeTrusted,
            payload_digest: summary_event_payload_metadata_digest(fact),
        }
    }

    fn refresh_cfg_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::CfgFunction);
        self.fact_meta.remove_family(FactFamily::CfgNode);
        self.fact_meta.remove_family(FactFamily::BasicBlock);
        self.fact_meta.remove_family(FactFamily::CfgEdge);
        self.fact_meta.remove_family(FactFamily::CfgReachability);
        self.fact_meta.remove_family(FactFamily::CfgDominator);
        self.fact_meta.remove_family(FactFamily::CfgPostDominator);
        self.fact_meta
            .remove_family(FactFamily::CfgControlDependence);
        self.fact_meta
            .remove_family(FactFamily::UnsupportedControlFlow);

        let cfg_functions = self.cfg_functions().to_vec();
        let cfg_nodes = self.cfg_nodes().to_vec();
        let cfg_blocks = self.cfg_blocks().to_vec();
        let cfg_edges = self.cfg_edges().to_vec();
        let cfg_reachability = self.cfg_reachability().to_vec();
        let cfg_dominators = self.cfg_dominators().to_vec();
        let cfg_postdominators = self.cfg_postdominators().to_vec();
        let cfg_control_dependence = self.cfg_control_dependence().to_vec();
        let unsupported_control_flow = self.unsupported_control_flow().to_vec();

        for fact in &cfg_functions {
            let metadata = self.cfg_function_metadata(fact);
            self.record_fact_meta(FactFamily::CfgFunction, fact.id.0, metadata);
        }

        for fact in &cfg_nodes {
            let metadata = self.cfg_node_metadata(fact);
            self.record_fact_meta(FactFamily::CfgNode, fact.id.0, metadata);
        }

        for fact in &cfg_blocks {
            let metadata = self.cfg_block_metadata(fact);
            self.record_fact_meta(FactFamily::BasicBlock, fact.id.0, metadata);
        }

        for fact in &cfg_edges {
            let metadata = self.cfg_edge_metadata(fact);
            self.record_fact_meta(FactFamily::CfgEdge, fact.id.0, metadata);
        }

        for fact in &cfg_reachability {
            let metadata = self.cfg_reachability_metadata(fact);
            self.record_fact_meta(FactFamily::CfgReachability, fact.id.0, metadata);
        }

        for fact in &cfg_dominators {
            let metadata = self.cfg_dominator_metadata(fact);
            self.record_fact_meta(FactFamily::CfgDominator, fact.id.0, metadata);
        }

        for fact in &cfg_postdominators {
            let metadata = self.cfg_postdominator_metadata(fact);
            self.record_fact_meta(FactFamily::CfgPostDominator, fact.id.0, metadata);
        }

        for fact in &cfg_control_dependence {
            let metadata = self.cfg_control_dependence_metadata(fact);
            self.record_fact_meta(FactFamily::CfgControlDependence, fact.id.0, metadata);
        }

        for fact in &unsupported_control_flow {
            let metadata = self.unsupported_control_flow_metadata(fact);
            self.record_fact_meta(FactFamily::UnsupportedControlFlow, fact.id.0, metadata);
        }

        self.finish_fact_meta_insertions(&[
            FactFamily::CfgFunction,
            FactFamily::CfgNode,
            FactFamily::BasicBlock,
            FactFamily::CfgEdge,
            FactFamily::CfgReachability,
            FactFamily::CfgDominator,
            FactFamily::CfgPostDominator,
            FactFamily::CfgControlDependence,
            FactFamily::UnsupportedControlFlow,
        ]);
    }

    fn refresh_semantic_mir_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::MirBody);
        self.fact_meta.remove_family(FactFamily::MirOperation);
        self.fact_meta.remove_family(FactFamily::Place);
        self.fact_meta
            .remove_family(FactFamily::UnsupportedSemantic);

        for index in 0..self.mir_bodies().len() {
            let (run_id, metadata) = {
                let body = &self.mir_bodies()[index];
                (body.id.0, self.mir_body_metadata(body))
            };
            self.record_fact_meta(FactFamily::MirBody, run_id, metadata);
        }

        for index in 0..self.mir_operations().len() {
            let (run_id, metadata) = {
                let operation = &self.mir_operations()[index];
                (operation.id.0, self.mir_operation_metadata(operation))
            };
            self.record_fact_meta(FactFamily::MirOperation, run_id, metadata);
        }

        for index in 0..self.mir_places().len() {
            let (run_id, metadata) = {
                let place = &self.mir_places()[index];
                (place.id.0, self.place_metadata(place))
            };
            self.record_fact_meta(FactFamily::Place, run_id, metadata);
        }

        for index in 0..self.unsupported_semantics().len() {
            let (run_id, metadata) = {
                let row = &self.unsupported_semantics()[index];
                (row.id.0, self.unsupported_semantic_metadata(row))
            };
            self.record_fact_meta(FactFamily::UnsupportedSemantic, run_id, metadata);
        }

        self.finish_fact_meta_insertions(&[
            FactFamily::MirBody,
            FactFamily::MirOperation,
            FactFamily::Place,
            FactFamily::UnsupportedSemantic,
        ]);
    }
}

impl AnalysisDb {
    fn refresh_module_graph_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::ModuleNode);
        self.fact_meta.remove_family(FactFamily::ResolvedImport);
        self.fact_meta.remove_family(FactFamily::ModuleEdge);

        let node_metadata = self
            .module_nodes
            .iter()
            .map(|node| (node.id.0, self.module_node_metadata(node)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in node_metadata {
            self.record_fact_meta(FactFamily::ModuleNode, run_id, metadata);
        }

        let resolved_metadata = self
            .resolved_imports
            .iter()
            .map(|fact| (fact.id.0, self.resolved_import_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in resolved_metadata {
            self.record_fact_meta(FactFamily::ResolvedImport, run_id, metadata);
        }

        let edge_metadata = self
            .module_edges
            .iter()
            .map(|edge| (edge.id.0, self.module_edge_metadata(edge)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in edge_metadata {
            self.record_fact_meta(FactFamily::ModuleEdge, run_id, metadata);
        }
        self.finish_fact_meta_insertions(&[
            FactFamily::ModuleNode,
            FactFamily::ResolvedImport,
            FactFamily::ModuleEdge,
        ]);
    }

    fn refresh_topology_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::WorkspaceRoot);
        self.fact_meta.remove_family(FactFamily::TopologyPackage);
        self.fact_meta.remove_family(FactFamily::SourceSet);
        self.fact_meta
            .remove_family(FactFamily::DependencyRequirement);
        self.fact_meta
            .remove_family(FactFamily::ResolvedDependencyEdge);
        self.fact_meta
            .remove_family(FactFamily::RepoTopologyOverlay);
        self.refresh_import_to_package_metadata();

        let root_metadata = self
            .workspace_roots
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    topology_fact_metadata(
                        FactFamily::WorkspaceRoot,
                        MODULE_GRAPH_PROVIDER_ID,
                        fact.precision,
                        &fact.stable_key,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in root_metadata {
            self.record_fact_meta(FactFamily::WorkspaceRoot, run_id, metadata);
        }

        let package_metadata = self
            .topology_packages
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    topology_fact_metadata(
                        FactFamily::TopologyPackage,
                        MODULE_GRAPH_PROVIDER_ID,
                        fact.precision,
                        &fact.stable_key,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in package_metadata {
            self.record_fact_meta(FactFamily::TopologyPackage, run_id, metadata);
        }

        let source_set_metadata = self
            .source_sets
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    topology_fact_metadata(
                        FactFamily::SourceSet,
                        MODULE_GRAPH_PROVIDER_ID,
                        fact.precision,
                        &fact.stable_key,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in source_set_metadata {
            self.record_fact_meta(FactFamily::SourceSet, run_id, metadata);
        }

        let requirement_metadata = self
            .dependency_requirements
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    topology_fact_metadata(
                        FactFamily::DependencyRequirement,
                        MODULE_GRAPH_PROVIDER_ID,
                        fact.precision,
                        &fact.stable_key,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in requirement_metadata {
            self.record_fact_meta(FactFamily::DependencyRequirement, run_id, metadata);
        }

        let resolved_metadata = self
            .resolved_dependency_edges
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    topology_fact_metadata(
                        FactFamily::ResolvedDependencyEdge,
                        MODULE_GRAPH_PROVIDER_ID,
                        fact.precision,
                        &fact.stable_key,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in resolved_metadata {
            self.record_fact_meta(FactFamily::ResolvedDependencyEdge, run_id, metadata);
        }

        let overlay_metadata = self
            .repo_topology_overlays
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    topology_fact_metadata(
                        FactFamily::RepoTopologyOverlay,
                        MODULE_GRAPH_PROVIDER_ID,
                        fact.precision,
                        &fact.stable_key,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in overlay_metadata {
            self.record_fact_meta(FactFamily::RepoTopologyOverlay, run_id, metadata);
        }
        self.finish_fact_meta_insertions(&[
            FactFamily::WorkspaceRoot,
            FactFamily::TopologyPackage,
            FactFamily::SourceSet,
            FactFamily::DependencyRequirement,
            FactFamily::ResolvedDependencyEdge,
            FactFamily::RepoTopologyOverlay,
        ]);
    }

    fn refresh_import_to_package_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::ImportToPackage);

        let metadata = self
            .import_to_package_edges
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    topology_fact_metadata(
                        FactFamily::ImportToPackage,
                        MODULE_TOPOLOGY_PROVIDER_ID,
                        fact.precision,
                        &fact.stable_key,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in metadata {
            self.record_fact_meta(FactFamily::ImportToPackage, run_id, metadata);
        }
        self.finish_fact_meta_insertions(&[FactFamily::ImportToPackage]);
    }

    fn refresh_semantic_index_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::Scope);
        self.fact_meta.remove_family(FactFamily::SemanticImport);
        self.fact_meta.remove_family(FactFamily::Export);
        self.fact_meta.remove_family(FactFamily::Alias);
        self.fact_meta.remove_family(FactFamily::Resolution);
        self.fact_meta.remove_family(FactFamily::GeneratedSymbol);
        self.fact_meta.remove_family(FactFamily::StableExport);

        let scope_metadata = self
            .scopes
            .iter()
            .map(|scope| {
                (
                    scope.id.0,
                    self.semantic_fact_metadata(FactFamily::Scope, &scope.stable_key, scope.status),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in scope_metadata {
            self.record_fact_meta(FactFamily::Scope, run_id, metadata);
        }

        let import_metadata = self
            .semantic_imports
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    self.semantic_fact_metadata(
                        FactFamily::SemanticImport,
                        &fact.stable_key,
                        fact.status,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in import_metadata {
            self.record_fact_meta(FactFamily::SemanticImport, run_id, metadata);
        }

        let export_metadata = self
            .exports
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    self.semantic_fact_metadata(FactFamily::Export, &fact.stable_key, fact.status),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in export_metadata {
            self.record_fact_meta(FactFamily::Export, run_id, metadata);
        }

        let alias_metadata = self
            .aliases
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    self.semantic_fact_metadata(FactFamily::Alias, &fact.stable_key, fact.status),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in alias_metadata {
            self.record_fact_meta(FactFamily::Alias, run_id, metadata);
        }

        let resolution_metadata = self
            .resolution_facts
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    self.semantic_fact_metadata(
                        FactFamily::Resolution,
                        &fact.stable_key,
                        fact.status,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in resolution_metadata {
            self.record_fact_meta(FactFamily::Resolution, run_id, metadata);
        }

        let generated_metadata = self
            .generated_symbols
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    self.semantic_fact_metadata(
                        FactFamily::GeneratedSymbol,
                        &fact.stable_key,
                        fact.status,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in generated_metadata {
            self.record_fact_meta(FactFamily::GeneratedSymbol, run_id, metadata);
        }

        let stable_export_metadata = self
            .stable_exports
            .iter()
            .map(|fact| {
                (
                    fact.id.0,
                    self.semantic_fact_metadata(
                        FactFamily::StableExport,
                        &fact.stable_key,
                        fact.status,
                    ),
                )
            })
            .collect::<Vec<_>>();
        for (run_id, metadata) in stable_export_metadata {
            self.record_fact_meta(FactFamily::StableExport, run_id, metadata);
        }
        self.finish_fact_meta_insertions(&[
            FactFamily::Scope,
            FactFamily::SemanticImport,
            FactFamily::Export,
            FactFamily::Alias,
            FactFamily::Resolution,
            FactFamily::GeneratedSymbol,
            FactFamily::StableExport,
        ]);
    }

    fn refresh_symbol_graph_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::Symbol);
        self.fact_meta.remove_family(FactFamily::Definition);
        self.fact_meta.remove_family(FactFamily::Reference);

        for index in 0..self.symbols.len() {
            let (run_id, metadata) = {
                let symbol = &self.symbols[index];
                (symbol.id.0, self.symbol_fact_metadata(symbol))
            };
            self.record_fact_meta(FactFamily::Symbol, run_id, metadata);
        }

        for index in 0..self.definitions.len() {
            let (run_id, metadata) = {
                let definition = &self.definitions[index];
                (definition.id.0, self.definition_fact_metadata(definition))
            };
            self.record_fact_meta(FactFamily::Definition, run_id, metadata);
        }

        for index in 0..self.references.len() {
            let (run_id, metadata) = {
                let reference = &self.references[index];
                (reference.id.0, self.reference_fact_metadata(reference))
            };
            self.record_fact_meta(FactFamily::Reference, run_id, metadata);
        }
        self.finish_fact_meta_insertions(&[
            FactFamily::Symbol,
            FactFamily::Definition,
            FactFamily::Reference,
        ]);
    }

    fn refresh_metric_metadata(&mut self) {
        self.fact_meta.remove_family(FactFamily::FileMetric);
        self.fact_meta.remove_family(FactFamily::FunctionMetric);
        self.fact_meta.remove_family(FactFamily::ComplexityMetric);

        let file_metadata = self
            .file_metrics
            .iter()
            .enumerate()
            .map(|(index, fact)| (index as u64, self.file_metric_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in file_metadata {
            self.record_fact_meta(FactFamily::FileMetric, run_id, metadata);
        }

        let function_metadata = self
            .function_metrics
            .iter()
            .enumerate()
            .map(|(index, fact)| (index as u64, self.function_metric_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in function_metadata {
            self.record_fact_meta(FactFamily::FunctionMetric, run_id, metadata);
        }

        let complexity_metadata = self
            .complexity_metrics
            .iter()
            .enumerate()
            .map(|(index, fact)| (index as u64, self.complexity_metric_metadata(fact)))
            .collect::<Vec<_>>();
        for (run_id, metadata) in complexity_metadata {
            self.record_fact_meta(FactFamily::ComplexityMetric, run_id, metadata);
        }
        self.finish_fact_meta_insertions(&[
            FactFamily::FileMetric,
            FactFamily::FunctionMetric,
            FactFamily::ComplexityMetric,
        ]);
    }

    fn rebuild_symbol_graph_indexes(&mut self) {
        self.symbols_by_id.clear();
        self.definitions_by_symbol.clear();
        self.references_by_target.clear();
        self.symbols_by_file.clear();
        self.references_by_file.clear();
        self.symbols_by_name.clear();

        for (index, symbol) in self.symbols.iter().enumerate() {
            self.symbols_by_id.insert(symbol.id, index);
            if let Some(file) = symbol.file {
                self.symbols_by_file.entry(file).or_default().push(index);
            }
            self.symbols_by_name
                .entry(symbol.name.clone())
                .or_default()
                .push(index);
        }

        for (index, definition) in self.definitions.iter().enumerate() {
            self.definitions_by_symbol
                .entry(definition.symbol)
                .or_default()
                .push(index);
        }

        for (index, reference) in self.references.iter().enumerate() {
            if let Some(target) = reference.target {
                self.references_by_target
                    .entry(target)
                    .or_default()
                    .push(index);
            }
            if let Some(file) = reference.file {
                self.references_by_file.entry(file).or_default().push(index);
            }
        }

        let symbols = &self.symbols;
        for indexes in self.symbols_by_file.values_mut() {
            indexes.sort_by_key(|index| symbols[*index].id);
        }
        for indexes in self.symbols_by_name.values_mut() {
            indexes.sort_by_key(|index| symbols[*index].id);
        }

        let definitions = &self.definitions;
        for indexes in self.definitions_by_symbol.values_mut() {
            indexes.sort_by_key(|index| definitions[*index].id);
        }

        let references = &self.references;
        for indexes in self.references_by_target.values_mut() {
            indexes.sort_by_key(|index| references[*index].id);
        }
        for indexes in self.references_by_file.values_mut() {
            indexes.sort_by_key(|index| references[*index].id);
        }
    }

    fn rebuild_semantic_index_indexes(&mut self) {
        self.scopes_by_id.clear();
        self.semantic_imports_by_id.clear();
        self.exports_by_id.clear();
        self.aliases_by_id.clear();
        self.resolution_facts_by_id.clear();
        self.generated_symbols_by_id.clear();
        self.stable_exports_by_id.clear();

        for (index, scope) in self.scopes.iter().enumerate() {
            self.scopes_by_id.insert(scope.id, index);
        }
        for (index, import) in self.semantic_imports.iter().enumerate() {
            self.semantic_imports_by_id.insert(import.id, index);
        }
        for (index, export) in self.exports.iter().enumerate() {
            self.exports_by_id.insert(export.id, index);
        }
        for (index, alias) in self.aliases.iter().enumerate() {
            self.aliases_by_id.insert(alias.id, index);
        }
        for (index, resolution) in self.resolution_facts.iter().enumerate() {
            self.resolution_facts_by_id.insert(resolution.id, index);
        }
        for (index, generated) in self.generated_symbols.iter().enumerate() {
            self.generated_symbols_by_id.insert(generated.id, index);
        }
        for (index, stable_export) in self.stable_exports.iter().enumerate() {
            self.stable_exports_by_id.insert(stable_export.id, index);
        }
    }

    pub fn push_ts_component(&mut self, fact: TsComponentFact) {
        let metadata = self.ts_component_metadata(&fact);
        let run_id = self.ts_syntax_store_mut().push_ts_component(fact);
        self.record_fact_meta(FactFamily::TsComponent, run_id, metadata);
    }

    pub fn push_ts_class(&mut self, fact: TsClassFact) {
        let metadata = self.ts_class_metadata(&fact);
        let run_id = self.ts_syntax_store_mut().push_ts_class(fact);
        self.record_fact_meta(FactFamily::TsClass, run_id, metadata);
    }

    pub fn push_string_literal(&mut self, fact: StringLiteralFact) {
        let metadata = self.string_literal_metadata(&fact);
        let run_id = self.ts_syntax_store_mut().push_string_literal(fact);
        self.record_fact_meta(FactFamily::StringLiteral, run_id, metadata);
    }

    pub fn push_jsx_attribute(&mut self, fact: JsxAttributeFact) {
        let metadata = self.jsx_attribute_metadata(&fact);
        let run_id = self.ts_syntax_store_mut().push_jsx_attribute(fact);
        self.record_fact_meta(FactFamily::JsxAttribute, run_id, metadata);
    }

    pub(crate) fn fact_meta(&self) -> &FactMetaStore {
        &self.fact_meta
    }

    #[cfg(test)]
    pub(crate) fn fact_meta_mut_for_test(&mut self) -> &mut FactMetaStore {
        &mut self.fact_meta
    }

    #[cfg(test)]
    pub(crate) fn remove_fact_metadata_for_test(&mut self, fact_ref: FactRef) -> Option<FactMeta> {
        self.fact_meta.remove_for_test(fact_ref)
    }

    pub(crate) fn metadata_for(&self, fact_ref: FactRef) -> Option<&FactMeta> {
        self.fact_meta().get(fact_ref)
    }
}

impl AnalysisDb {
    pub(crate) fn missing_fact_metadata(&self) -> Vec<MissingFactMeta> {
        let mut missing = Vec::new();

        for file in self.files() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::SourceFile,
                u64::from(file.id.0),
            );
        }
        for package in self.packages() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Package, package.id.0);
        }
        for function in self.functions() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Function, function.id.0);
        }
        for import in self.imports() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Import, import.id.0);
        }
        for resolved_import in self.resolved_imports() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::ResolvedImport,
                resolved_import.id.0,
            );
        }
        for module_node in self.module_nodes() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::ModuleNode, module_node.id.0);
        }
        for module_edge in self.module_edges() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::ModuleEdge, module_edge.id.0);
        }
        for root in self.workspace_roots() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::WorkspaceRoot, root.id.0);
        }
        for package in self.topology_packages() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::TopologyPackage,
                package.id.0,
            );
        }
        for source_set in self.source_sets() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::SourceSet, source_set.id.0);
        }
        for requirement in self.dependency_requirements() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::DependencyRequirement,
                requirement.id.0,
            );
        }
        for edge in self.resolved_dependency_edges() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::ResolvedDependencyEdge,
                edge.id.0,
            );
        }
        for edge in self.import_to_package_edges() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::ImportToPackage, edge.id.0);
        }
        for overlay in self.repo_topology_overlays() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::RepoTopologyOverlay,
                overlay.id.0,
            );
        }
        for scope in self.scopes() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Scope, scope.id.0);
        }
        for import in self.semantic_imports() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::SemanticImport, import.id.0);
        }
        for export in self.exports() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Export, export.id.0);
        }
        for alias in self.aliases() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Alias, alias.id.0);
        }
        for resolution in self.resolution_facts() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Resolution, resolution.id.0);
        }
        for generated in self.generated_symbols() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::GeneratedSymbol,
                generated.id.0,
            );
        }
        for stable_export in self.stable_exports() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::StableExport,
                stable_export.id.0,
            );
        }
        for body in self.mir_bodies() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::MirBody, body.id.0);
        }
        for operation in self.mir_operations() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::MirOperation, operation.id.0);
        }
        for place in self.mir_places() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Place, place.id.0);
        }
        for row in self.unsupported_semantics() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::UnsupportedSemantic,
                row.id.0,
            );
        }
        for site in self.call_sites() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::CallSite, site.id.0);
        }
        for target in self.call_targets() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::CallTarget, target.id.0);
        }
        for (run_id, _unresolved) in self.unresolved_calls().iter().enumerate() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::UnresolvedCall,
                run_id as u64,
            );
        }
        for edge in self.refined_call_edges() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::RefinedCallEdge, edge.id.0);
        }
        for node in self.data_flow_nodes() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::DataFlowNode, node.id.0);
        }
        for edge in self.data_flow_edges() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::DataFlowEdge, edge.id.0);
        }
        for model in self.data_flow_models() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::DataFlowModel, model.id.0);
        }
        for budget in self.data_flow_budgets() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::DataFlowBudget, budget.id.0);
        }
        for node in self.evidence_nodes() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::EvidenceNode, node.id.0);
        }
        for edge in self.evidence_edges() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::EvidenceEdge, edge.id.0);
        }
        for bundle in self.evidence_bundles() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::EvidenceBundle, bundle.id.0);
        }
        for path in self.evidence_paths() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::EvidencePath, path.id.0);
        }
        for slice in self.evidence_slices() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::EvidenceSlice, slice.id.0);
        }
        for (run_id, _unknown) in self.evidence_unknowns().iter().enumerate() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::EvidenceUnknown,
                run_id as u64,
            );
        }
        for omitted in self.evidence_omitted_regions() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::EvidenceOmittedRegion,
                omitted.id.0,
            );
        }
        for (run_id, _replay) in self.evidence_replay_keys().iter().enumerate() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::EvidenceReplayKey,
                run_id as u64,
            );
        }
        for symbol in self.symbols() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Symbol, symbol.id.0);
        }
        for definition in self.definitions() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Definition, definition.id.0);
        }
        for reference in self.references() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Reference, reference.id.0);
        }
        for branch in self.branches() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::BranchObligation,
                branch.id.0,
            );
        }
        for (run_id, _test) in self.tests().iter().enumerate() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Test, run_id as u64);
        }
        for (run_id, _coverage) in self.coverage().iter().enumerate() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::Coverage, run_id as u64);
        }
        for (run_id, _file_metric) in self.file_metrics().iter().enumerate() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::FileMetric, run_id as u64);
        }
        for (run_id, _function_metric) in self.function_metrics().iter().enumerate() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::FunctionMetric,
                run_id as u64,
            );
        }
        for (run_id, _complexity_metric) in self.complexity_metrics().iter().enumerate() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::ComplexityMetric,
                run_id as u64,
            );
        }
        for (run_id, _component) in self.ts_components().iter().enumerate() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::TsComponent, run_id as u64);
        }
        for (run_id, _class) in self.ts_classes().iter().enumerate() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::TsClass, run_id as u64);
        }
        for (run_id, _literal) in self.string_literals().iter().enumerate() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::StringLiteral, run_id as u64);
        }
        for (run_id, _attribute) in self.jsx_attributes().iter().enumerate() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::JsxAttribute, run_id as u64);
        }
        for (run_id, _fact) in self.extension_facts().iter().enumerate() {
            self.push_missing_fact_metadata(&mut missing, FactFamily::ExtensionFact, run_id as u64);
        }
        for (run_id, _fact) in self.adaptation_model_facts().iter().enumerate() {
            self.push_missing_fact_metadata(
                &mut missing,
                FactFamily::AdaptationModel,
                run_id as u64,
            );
        }

        missing.sort_by(|left, right| {
            (left.family.label(), left.run_id).cmp(&(right.family.label(), right.run_id))
        });
        missing
    }

    fn push_missing_fact_metadata(
        &self,
        missing: &mut Vec<MissingFactMeta>,
        family: FactFamily,
        run_id: u64,
    ) {
        if self.metadata_for(FactRef::new(family, run_id)).is_none() {
            missing.push(MissingFactMeta { family, run_id });
        }
    }

    pub fn file(&self, id: FileId) -> Option<&SourceFile> {
        self.files.get(id.0 as usize)
    }

    pub(crate) fn set_path_contexts(&mut self, index: crate::path_context::PathContextIndex) {
        self.path_contexts = Some(index);
    }

    /// Repo-relative paths paired with `relative_path` (see `.polint.toml` `[path_contexts]`).
    pub fn path_context_related(&self, pair_name: &str, relative_path: &str) -> Vec<String> {
        self.path_contexts
            .as_ref()
            .map(|ix| ix.related_paths(pair_name, relative_path))
            .unwrap_or_default()
    }

    pub fn files(&self) -> &[SourceFile] {
        &self.files
    }

    /// Relative paths as in diagnostics (`SourceFile.relative_path`) → full source text.
    pub fn sources_by_relative_path(&self) -> BTreeMap<String, Arc<str>> {
        self.files
            .iter()
            .map(|file| (file.relative_path.clone(), Arc::clone(&file.source)))
            .collect()
    }

    pub fn packages(&self) -> &[PackageFact] {
        self.go_syntax_store().packages()
    }

    pub fn functions(&self) -> &[FunctionFact] {
        self.go_syntax_store().functions()
    }

    pub fn imports(&self) -> &[ImportFact] {
        self.go_syntax_store().imports()
    }

    pub fn resolved_imports(&self) -> &[ResolvedImportFact] {
        &self.resolved_imports
    }

    pub fn module_nodes(&self) -> &[ModuleNode] {
        &self.module_nodes
    }

    pub fn module_edges(&self) -> &[ModuleEdge] {
        &self.module_edges
    }

    pub(crate) fn workspace_roots(&self) -> &[WorkspaceRootFact] {
        &self.workspace_roots
    }

    pub(crate) fn topology_packages(&self) -> &[TopologyPackageFact] {
        &self.topology_packages
    }

    pub(crate) fn source_sets(&self) -> &[SourceSetFact] {
        &self.source_sets
    }

    pub(crate) fn dependency_requirements(&self) -> &[DependencyRequirementFact] {
        &self.dependency_requirements
    }

    pub(crate) fn resolved_dependency_edges(&self) -> &[ResolvedDependencyEdgeFact] {
        &self.resolved_dependency_edges
    }

    pub(crate) fn import_to_package_edges(&self) -> &[ImportToPackageFact] {
        &self.import_to_package_edges
    }

    pub(crate) fn repo_topology_overlays(&self) -> &[RepoTopologyOverlayFact] {
        &self.repo_topology_overlays
    }

    pub(crate) fn scopes(&self) -> &[ScopeFact] {
        &self.scopes
    }

    pub(crate) fn semantic_imports(&self) -> &[SemanticImportFact] {
        &self.semantic_imports
    }

    pub(crate) fn exports(&self) -> &[ExportFact] {
        &self.exports
    }

    pub(crate) fn aliases(&self) -> &[AliasFact] {
        &self.aliases
    }

    pub(crate) fn resolution_facts(&self) -> &[ResolutionFact] {
        &self.resolution_facts
    }

    pub(crate) fn generated_symbols(&self) -> &[GeneratedSymbolFact] {
        &self.generated_symbols
    }

    pub(crate) fn stable_exports(&self) -> &[StableExportIdentity] {
        &self.stable_exports
    }

    pub(crate) fn semantic_store(&self) -> Option<&SemanticStore> {
        self.semantic.as_ref()
    }

    pub(crate) fn mir_bodies(&self) -> &[MirBody] {
        self.semantic_store().map_or(&[], SemanticStore::mir_bodies)
    }

    pub(crate) fn mir_operations(&self) -> &[MirOperation] {
        self.semantic_store()
            .map_or(&[], SemanticStore::mir_operations)
    }

    pub(crate) fn mir_places(&self) -> &[PlaceFact] {
        self.semantic_store().map_or(&[], SemanticStore::places)
    }

    pub(crate) fn unsupported_semantics(&self) -> &[UnsupportedSemanticFact] {
        self.semantic_store()
            .map_or(&[], SemanticStore::unsupported_semantics)
    }

    pub(crate) fn cfg_functions(&self) -> &[CfgFunctionFact] {
        self.cfg_store().functions()
    }

    pub(crate) fn cfg_nodes(&self) -> &[CfgNodeFact] {
        self.cfg_store().nodes()
    }

    pub(crate) fn cfg_blocks(&self) -> &[BasicBlockFact] {
        self.cfg_store().blocks()
    }

    pub(crate) fn cfg_edges(&self) -> &[CfgEdgeFact] {
        self.cfg_store().edges()
    }

    pub(crate) fn cfg_reachability(&self) -> &[ReachabilityFact] {
        self.cfg_store().reachability()
    }

    pub(crate) fn cfg_dominators(&self) -> &[DominatorFact] {
        self.cfg_store().dominators()
    }

    pub(crate) fn cfg_postdominators(&self) -> &[PostDominatorFact] {
        self.cfg_store().postdominators()
    }

    pub(crate) fn cfg_control_dependence(&self) -> &[ControlDependenceFact] {
        self.cfg_store().control_dependence()
    }

    pub(crate) fn unsupported_control_flow(&self) -> &[UnsupportedControlFlowFact] {
        self.cfg_store().unsupported()
    }

    pub fn symbols(&self) -> &[SymbolFact] {
        &self.symbols
    }

    pub fn definitions(&self) -> &[DefinitionFact] {
        &self.definitions
    }

    pub fn references(&self) -> &[ReferenceFact] {
        &self.references
    }

    pub(crate) fn symbol_by_id(&self, id: SymbolId) -> Option<&SymbolFact> {
        self.symbols_by_id
            .get(&id)
            .and_then(|index| self.symbols.get(*index))
    }

    pub(crate) fn symbols_for_file(&self, file: FileId) -> impl Iterator<Item = &SymbolFact> + '_ {
        self.symbols_by_file
            .get(&file)
            .into_iter()
            .flat_map(|indexes| indexes.iter().filter_map(|index| self.symbols.get(*index)))
    }

    pub(crate) fn symbols_by_name(&self, name: &str) -> impl Iterator<Item = &SymbolFact> + '_ {
        self.symbols_by_name
            .get(name)
            .into_iter()
            .flat_map(|indexes| indexes.iter().filter_map(|index| self.symbols.get(*index)))
    }

    pub(crate) fn definition_for_symbol(&self, symbol: SymbolId) -> Option<&DefinitionFact> {
        let mut definitions = self.definitions_for_symbol(symbol);
        let first = definitions.next();
        first
            .filter(|definition| definition.is_primary)
            .or_else(|| definitions.find(|definition| definition.is_primary))
            .or(first)
    }

    pub(crate) fn definitions_for_symbol(
        &self,
        symbol: SymbolId,
    ) -> impl Iterator<Item = &DefinitionFact> + '_ {
        self.definitions_by_symbol
            .get(&symbol)
            .into_iter()
            .flat_map(|indexes| {
                indexes
                    .iter()
                    .filter_map(|index| self.definitions.get(*index))
            })
    }

    pub(crate) fn references_to_symbol(
        &self,
        symbol: SymbolId,
    ) -> impl Iterator<Item = &ReferenceFact> + '_ {
        self.references_by_target
            .get(&symbol)
            .into_iter()
            .flat_map(|indexes| {
                indexes
                    .iter()
                    .filter_map(|index| self.references.get(*index))
            })
    }

    pub(crate) fn references_for_file(
        &self,
        file: FileId,
    ) -> impl Iterator<Item = &ReferenceFact> + '_ {
        self.references_by_file
            .get(&file)
            .into_iter()
            .flat_map(|indexes| {
                indexes
                    .iter()
                    .filter_map(|index| self.references.get(*index))
            })
    }

    pub fn branches(&self) -> &[BranchObligation] {
        self.go_syntax_store().branches()
    }

    pub fn tests(&self) -> &[TestFact] {
        self.go_syntax_store().tests()
    }

    pub fn coverage(&self) -> &[CoverageFact] {
        &self.coverage
    }

    pub fn file_metrics(&self) -> &[FileMetricFact] {
        &self.file_metrics
    }

    pub fn function_metrics(&self) -> &[FunctionMetricFact] {
        &self.function_metrics
    }

    pub fn complexity_metrics(&self) -> &[ComplexityMetricFact] {
        &self.complexity_metrics
    }

    pub fn ts_components(&self) -> &[TsComponentFact] {
        self.ts_syntax_store().ts_components()
    }

    pub fn ts_classes(&self) -> &[TsClassFact] {
        self.ts_syntax_store().ts_classes()
    }

    pub fn string_literals(&self) -> &[StringLiteralFact] {
        self.ts_syntax_store().string_literals()
    }

    pub fn jsx_attributes(&self) -> &[JsxAttributeFact] {
        self.ts_syntax_store().jsx_attributes()
    }

    pub fn path_for(&self, file: FileId) -> String {
        self.file(file)
            .map(|file| file.relative_path.clone())
            .unwrap_or_else(|| "<unknown>".to_string())
    }
}

impl AnalysisDb {
    pub fn facts_for_file(&self, file: FileId) -> CachedFileFacts {
        let branch_ids = self
            .branches()
            .iter()
            .filter(|branch| branch.file == file)
            .map(|branch| branch.id)
            .collect::<BTreeSet<_>>();
        CachedFileFacts {
            packages: self
                .packages()
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            functions: self
                .functions()
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            imports: self
                .imports()
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            branches: self
                .branches()
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            tests: self
                .tests()
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            coverage: self
                .coverage
                .iter()
                .filter(|fact| branch_ids.contains(&fact.branch))
                .cloned()
                .collect(),
            ts_components: self
                .ts_components()
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            ts_classes: self
                .ts_classes()
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            string_literals: self
                .string_literals()
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            jsx_attributes: self
                .jsx_attributes()
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
        }
    }

    pub fn restore_file_facts(&mut self, file: FileId, facts: CachedFileFacts) {
        let mut function_ids = BTreeMap::new();
        let mut branch_ids = BTreeMap::new();

        for mut package in facts.packages {
            package.file = file;
            package.span.file = file;
            self.push_package(package);
        }

        for mut function in facts.functions {
            let cached_id = function.id;
            function.file = file;
            function.span.file = file;
            let restored_id = self.push_function(function);
            function_ids.insert(cached_id, restored_id);
        }

        for mut import in facts.imports {
            import.file = file;
            import.span.file = file;
            self.push_import(import);
        }

        for mut branch in facts.branches {
            let cached_id = branch.id;
            branch.file = file;
            branch.function = branch
                .function
                .and_then(|function| function_ids.get(&function).copied());
            branch.decision_span.file = file;
            let restored_id = self.push_branch(branch);
            branch_ids.insert(cached_id, restored_id);
        }

        for mut test in facts.tests {
            test.file = file;
            test.function = test
                .function
                .and_then(|function| function_ids.get(&function).copied());
            test.span.file = file;
            self.push_test(test);
        }

        for mut coverage in facts.coverage {
            if let Some(branch) = branch_ids.get(&coverage.branch).copied() {
                coverage.branch = branch;
                self.push_coverage(coverage);
            }
        }

        for mut component in facts.ts_components {
            component.file = file;
            component.function = component
                .function
                .and_then(|function| function_ids.get(&function).copied());
            component.span.file = file;
            self.push_ts_component(component);
        }

        for mut class in facts.ts_classes {
            class.file = file;
            class.span.file = file;
            self.push_ts_class(class);
        }

        for mut literal in facts.string_literals {
            literal.file = file;
            literal.span.file = file;
            self.push_string_literal(literal);
        }

        for mut attribute in facts.jsx_attributes {
            attribute.file = file;
            attribute.span.file = file;
            self.push_jsx_attribute(attribute);
        }
    }

    fn record_fact_meta(&mut self, family: FactFamily, run_id: u64, meta: FactMeta) {
        let reference = FactRef::new(family, run_id);
        let _insert = self.fact_meta.insert(reference, meta);
        debug_assert!(self.metadata_for(reference).is_some());
    }

    fn finish_fact_meta_insertions(&mut self, families: &[FactFamily]) {
        for family in families {
            self.fact_meta.finish_family_insertions(*family);
        }
    }

    pub(crate) fn finish_all_fact_meta_insertions(&mut self) {
        self.fact_meta.finish_all_insertions();
    }

    fn package_metadata(&self, fact: &PackageFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::Package,
            syntax_provider_for_language(fact.language),
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("language", language_label(fact.language).to_string()),
                ("name", fact.name.clone()),
                ("span", span_metadata_value(&fact.span)),
            ]),
            stable_parts([]),
        )
    }

    fn function_metadata(&self, fact: &FunctionFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::Function,
            syntax_provider_for_language(fact.language),
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("language", language_label(fact.language).to_string()),
                ("name", fact.name.clone()),
                ("span", span_metadata_value(&fact.span)),
            ]),
            stable_parts([
                ("is_test", fact.is_test.to_string()),
                ("is_exported", fact.is_exported.to_string()),
                (
                    "cyclomatic_complexity",
                    fact.cyclomatic_complexity.to_string(),
                ),
                ("calls", fact.calls.join("\n")),
            ]),
        )
    }

    fn import_metadata(&self, fact: &ImportFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::Import,
            syntax_provider_for_language(fact.language),
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("language", language_label(fact.language).to_string()),
                ("import_path", fact.path.clone()),
                ("span", span_metadata_value(&fact.span)),
            ]),
            stable_parts([(
                "package",
                fact.package.clone().unwrap_or_else(|| "none".to_string()),
            )]),
        )
    }

    fn branch_metadata(&self, fact: &BranchObligation) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::BranchObligation,
            GO_SYNTAX_PROVIDER_ID,
            FactPrecision::Heuristic,
            FactConfidence::Medium,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("stable_fingerprint", fact.stable_fingerprint.clone()),
            ]),
            stable_parts([
                ("function", option_function_id(fact.function)),
                ("span", span_metadata_value(&fact.decision_span)),
                ("condition_text", fact.condition_text.clone()),
                ("edge_label", fact.edge_label.clone()),
                ("is_error_path", fact.is_error_path.to_string()),
            ]),
        )
    }

    fn test_metadata(&self, fact: &TestFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::Test,
            GO_SYNTAX_PROVIDER_ID,
            FactPrecision::Heuristic,
            FactConfidence::Medium,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("name", fact.name.clone()),
                ("span", span_metadata_value(&fact.span)),
            ]),
            stable_parts([
                ("function", option_function_id(fact.function)),
                ("evidence_terms", fact.evidence_terms.join("\n")),
                ("assertion_count", fact.assertion_count.to_string()),
                ("subtest_count", fact.subtest_count.to_string()),
                ("subtest_names", fact.subtest_names.join("\n")),
                ("table_rows", fact.table_rows.to_string()),
            ]),
        )
    }

    fn coverage_metadata(&self, fact: &CoverageFact) -> FactMeta {
        let branch = self
            .branches()
            .iter()
            .find(|branch| branch.id == fact.branch);
        let (path, branch_fingerprint, precision, confidence) = if let Some(branch) = branch {
            (
                self.path_for(branch.file),
                branch.stable_fingerprint.clone(),
                FactPrecision::SetupAware,
                FactConfidence::Medium,
            )
        } else {
            (
                "<unknown>".to_string(),
                format!("unresolved:{}", fact.branch.0),
                FactPrecision::Unsupported,
                FactConfidence::Low,
            )
        };

        fact_meta_from_parts(
            FactFamily::Coverage,
            branch
                .map(|branch| syntax_provider_for_file(self.file(branch.file)))
                .unwrap_or(GO_SYNTAX_PROVIDER_ID),
            precision,
            confidence,
            stable_parts([
                ("path", path),
                ("branch_fingerprint", branch_fingerprint),
                ("source", fact.source.clone()),
            ]),
            stable_parts([
                ("branch", fact.branch.0.to_string()),
                ("covered", option_bool(fact.covered)),
            ]),
        )
    }

    fn file_metric_metadata(&self, fact: &FileMetricFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::FileMetric,
            METRICS_PROVIDER_ID,
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([("file_key", self.source_file_key(fact.file))]),
            stable_parts([
                ("language", language_label(fact.language).to_string()),
                ("line_count", fact.line_count.to_string()),
                (
                    "non_empty_line_count",
                    fact.non_empty_line_count.to_string(),
                ),
                ("byte_count", fact.byte_count.to_string()),
                ("function_count", fact.function_count.to_string()),
            ]),
        )
    }

    fn function_metric_metadata(&self, fact: &FunctionMetricFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::FunctionMetric,
            METRICS_PROVIDER_ID,
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([
                (
                    "function_key",
                    self.function_key(fact.function, &fact.name, &fact.span),
                ),
                ("metric_name", FUNCTION_SIZE_METRIC_NAME.to_string()),
            ]),
            stable_parts([
                ("file_key", self.source_file_key(fact.file)),
                ("language", language_label(fact.language).to_string()),
                ("line_count", fact.line_count.to_string()),
                ("byte_count", fact.byte_count.to_string()),
            ]),
        )
    }

    fn complexity_metric_metadata(&self, fact: &ComplexityMetricFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::ComplexityMetric,
            METRICS_PROVIDER_ID,
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([
                (
                    "function_key",
                    self.function_key(fact.function, &fact.name, &fact.span),
                ),
                ("metric_name", CYCLOMATIC_COMPLEXITY_METRIC_NAME.to_string()),
            ]),
            stable_parts([
                ("file_key", self.source_file_key(fact.file)),
                ("language", language_label(fact.language).to_string()),
                (
                    "cyclomatic_complexity",
                    fact.cyclomatic_complexity.to_string(),
                ),
            ]),
        )
    }

    fn module_node_metadata(&self, node: &ModuleNode) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::ModuleNode,
            MODULE_GRAPH_PROVIDER_ID,
            FactPrecision::SetupAware,
            FactConfidence::High,
            stable_parts([
                ("kind", module_node_kind_label(node.kind).to_string()),
                ("label", node.label.clone()),
                ("path", option_file_path(self, node.file)),
                (
                    "package_key",
                    node.package
                        .map(|package| self.fact_stable_key(FactFamily::Package, package.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "language",
                    node.language
                        .map(|language| language_label(language).to_string())
                        .unwrap_or_else(none_value),
                ),
            ]),
            stable_parts([("id", node.id.0.to_string())]),
        )
    }

    fn resolved_import_metadata(&self, fact: &ResolvedImportFact) -> FactMeta {
        let (precision, confidence) = resolution_metadata(fact.precision, fact.status);
        fact_meta_from_parts(
            FactFamily::ResolvedImport,
            MODULE_GRAPH_PROVIDER_ID,
            precision,
            confidence,
            stable_parts([
                (
                    "import_key",
                    self.fact_stable_key(FactFamily::Import, fact.import.0),
                ),
                ("from_path", self.path_for(fact.from_file)),
                (
                    "target_node_key",
                    fact.target_node
                        .map(|node| self.fact_stable_key(FactFamily::ModuleNode, node.0))
                        .unwrap_or_else(none_value),
                ),
                ("status", resolution_status_label(fact.status).to_string()),
                (
                    "precision",
                    resolution_precision_label(fact.precision).to_string(),
                ),
                (
                    "reason",
                    fact.reason
                        .map(|reason| unresolved_reason_label(reason).to_string())
                        .unwrap_or_else(none_value),
                ),
            ]),
            stable_parts([
                ("id", fact.id.0.to_string()),
                ("import", fact.import.0.to_string()),
                ("from_file", u64::from(fact.from_file.0).to_string()),
                (
                    "target_node",
                    fact.target_node
                        .map(|node| node.0.to_string())
                        .unwrap_or_else(none_value),
                ),
            ]),
        )
    }

    fn module_edge_metadata(&self, edge: &ModuleEdge) -> FactMeta {
        let (precision, confidence) = resolution_status_metadata(edge.status);
        fact_meta_from_parts(
            FactFamily::ModuleEdge,
            MODULE_GRAPH_PROVIDER_ID,
            precision,
            confidence,
            stable_parts([
                (
                    "from_node_key",
                    self.fact_stable_key(FactFamily::ModuleNode, edge.from.0),
                ),
                (
                    "to_node_key",
                    self.fact_stable_key(FactFamily::ModuleNode, edge.to.0),
                ),
                (
                    "import_key",
                    edge.import
                        .map(|import| self.fact_stable_key(FactFamily::Import, import.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "resolved_import_key",
                    edge.resolved_import
                        .map(|resolved| {
                            self.fact_stable_key(FactFamily::ResolvedImport, resolved.0)
                        })
                        .unwrap_or_else(none_value),
                ),
                ("kind", module_edge_kind_label(edge.kind).to_string()),
                ("status", resolution_status_label(edge.status).to_string()),
            ]),
            stable_parts([
                ("id", edge.id.0.to_string()),
                ("from", edge.from.0.to_string()),
                ("to", edge.to.0.to_string()),
            ]),
        )
    }

    fn symbol_fact_metadata(&self, fact: &SymbolFact) -> FactMeta {
        let (precision, confidence) = symbol_metadata(fact.precision);
        fact_meta_from_stable_key(
            FactFamily::Symbol,
            SYMBOL_GRAPH_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("id", fact.id.0.to_string()),
                ("language", language_label(fact.language).to_string()),
                ("name", fact.name.clone()),
                ("qualified_name", fact.qualified_name.clone()),
                (
                    "precision",
                    symbol_precision_label(fact.precision).to_string(),
                ),
                ("path", option_file_path(self, fact.file)),
                (
                    "span",
                    option_span_metadata_value(fact.primary_span.as_ref()),
                ),
            ]),
        )
    }

    fn definition_fact_metadata(&self, fact: &DefinitionFact) -> FactMeta {
        let (precision, confidence) = symbol_metadata(fact.precision);
        fact_meta_from_stable_key(
            FactFamily::Definition,
            SYMBOL_GRAPH_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("id", fact.id.0.to_string()),
                ("symbol", fact.symbol.0.to_string()),
                ("language", language_label(fact.language).to_string()),
                ("name", fact.name.clone()),
                ("qualified_name", fact.qualified_name.clone()),
                (
                    "precision",
                    symbol_precision_label(fact.precision).to_string(),
                ),
                ("path", option_file_path(self, fact.file)),
                (
                    "span",
                    option_span_metadata_value(fact.primary_span.as_ref()),
                ),
            ]),
        )
    }

    fn reference_fact_metadata(&self, fact: &ReferenceFact) -> FactMeta {
        let (precision, confidence) = symbol_metadata(fact.precision);
        fact_meta_from_stable_key(
            FactFamily::Reference,
            SYMBOL_GRAPH_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("id", fact.id.0.to_string()),
                ("language", language_label(fact.language).to_string()),
                ("name", fact.name.clone()),
                ("qualified_name", fact.qualified_name.clone()),
                (
                    "precision",
                    symbol_precision_label(fact.precision).to_string(),
                ),
                (
                    "status",
                    symbol_resolution_status_label(fact.status).to_string(),
                ),
                ("path", option_file_path(self, fact.file)),
                (
                    "span",
                    option_span_metadata_value(fact.primary_span.as_ref()),
                ),
                (
                    "target",
                    fact.target
                        .map(|target| target.0.to_string())
                        .unwrap_or_else(none_value),
                ),
            ]),
        )
    }

    fn semantic_fact_metadata(
        &self,
        family: FactFamily,
        stable_key: &str,
        status: SemanticStatus,
    ) -> FactMeta {
        let (precision, confidence) = semantic_status_metadata(status);
        fact_meta_from_stable_key(
            family,
            SYMBOL_GRAPH_PROVIDER_ID,
            precision,
            confidence,
            stable_key.to_string(),
            stable_parts([("status", semantic_status_label(status).to_string())]),
        )
    }

    fn mir_body_metadata(&self, body: &MirBody) -> FactMeta {
        let (precision, confidence) = mir_status_metadata(body.status);
        fact_meta_from_stable_key(
            FactFamily::MirBody,
            SEMANTIC_MIR_PROVIDER_ID,
            precision,
            confidence,
            body.stable_key.clone(),
            stable_parts([
                ("status", mir_status_label(body.status).to_string()),
                ("language", language_label(body.language).to_string()),
                ("file_key", self.source_file_key(body.file)),
                (
                    "function_key",
                    self.function_key(body.function, "", &body.span),
                ),
                ("owner_stable_key", body.owner_stable_key.clone()),
                (
                    "package",
                    body.package
                        .map(|package| package.0.to_string())
                        .unwrap_or_else(none_value),
                ),
                (
                    "module",
                    body.module
                        .map(|module| module.0.to_string())
                        .unwrap_or_else(none_value),
                ),
                ("span", span_metadata_value(&body.span)),
            ]),
        )
    }

    fn mir_operation_metadata(&self, operation: &MirOperation) -> FactMeta {
        let (precision, confidence) = mir_status_metadata(operation.status);
        fact_meta_from_stable_key(
            FactFamily::MirOperation,
            SEMANTIC_MIR_PROVIDER_ID,
            precision,
            confidence,
            operation.stable_key.clone(),
            stable_parts([
                ("status", mir_status_label(operation.status).to_string()),
                (
                    "body_key",
                    self.fact_stable_key(FactFamily::MirBody, operation.body.0),
                ),
                ("ordinal", operation.ordinal.to_string()),
                ("span", span_metadata_value(&operation.span)),
            ]),
        )
    }
}

impl AnalysisDb {
    fn place_metadata(&self, place: &PlaceFact) -> FactMeta {
        let (precision, confidence) = place_status_metadata(place.status);
        fact_meta_from_stable_key(
            FactFamily::Place,
            SEMANTIC_MIR_PROVIDER_ID,
            precision,
            confidence,
            place.stable_key.clone(),
            stable_parts([
                ("status", place_status_label(place.status).to_string()),
                ("language", language_label(place.language).to_string()),
                ("path", option_file_path(self, place.file)),
                ("function", option_function_id(place.function)),
                ("projection_count", place.projections.len().to_string()),
            ]),
        )
    }

    fn unsupported_semantic_metadata(&self, row: &UnsupportedSemanticFact) -> FactMeta {
        let (precision, confidence) = mir_status_metadata(row.status);
        fact_meta_from_stable_key(
            FactFamily::UnsupportedSemantic,
            SEMANTIC_MIR_PROVIDER_ID,
            precision,
            confidence,
            row.stable_key.clone(),
            stable_parts([
                ("status", mir_status_label(row.status).to_string()),
                ("language", language_label(row.language).to_string()),
                ("path", self.path_for(row.file)),
                ("span", span_metadata_value(&row.span)),
                ("construct", row.construct.clone()),
                ("source_evidence", row.source_evidence.clone()),
                (
                    "body_key",
                    row.body
                        .map(|body| self.fact_stable_key(FactFamily::MirBody, body.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "operation_key",
                    row.operation
                        .map(|operation| {
                            self.fact_stable_key(FactFamily::MirOperation, operation.0)
                        })
                        .unwrap_or_else(none_value),
                ),
                (
                    "affected_places",
                    row.affected_places
                        .iter()
                        .map(|place| self.fact_stable_key(FactFamily::Place, place.0))
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
            ]),
        )
    }

    fn call_site_metadata(&self, fact: &CallSiteFact) -> FactMeta {
        let (precision, confidence) = call_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CallSite,
            CALLS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", call_status_label(fact.status).to_string()),
                (
                    "precision",
                    call_precision_label(fact.precision).to_string(),
                ),
                ("kind", call_syntax_kind_label(fact.kind).to_string()),
                ("language", language_label(fact.language).to_string()),
                ("file_key", self.source_file_key(fact.file)),
                ("caller_key", self.function_key(fact.caller, "", &fact.span)),
                (
                    "owner_symbol_key",
                    fact.owner_symbol
                        .map(|symbol| self.fact_stable_key(FactFamily::Symbol, symbol.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "body_key",
                    self.fact_stable_key(FactFamily::MirBody, fact.body.0),
                ),
                (
                    "operation_key",
                    self.fact_stable_key(FactFamily::MirOperation, fact.operation.0),
                ),
                ("span", span_metadata_value(&fact.span)),
            ]),
        )
    }

    fn call_target_metadata(&self, fact: &CallTargetFact) -> FactMeta {
        let (precision, confidence) = call_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CallTarget,
            CALLS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", call_status_label(fact.status).to_string()),
                (
                    "precision",
                    call_precision_label(fact.precision).to_string(),
                ),
                (
                    "algorithm",
                    call_algorithm_label(fact.algorithm).to_string(),
                ),
                (
                    "edge_kind",
                    call_edge_kind_label(fact.edge_kind).to_string(),
                ),
                (
                    "reason",
                    fact.reason
                        .map(call_unresolved_reason_label)
                        .map(str::to_string)
                        .unwrap_or_else(none_value),
                ),
                (
                    "site_key",
                    self.fact_stable_key(FactFamily::CallSite, fact.site.0),
                ),
                (
                    "caller_key",
                    self.fact_stable_key(FactFamily::Function, fact.caller.0),
                ),
                (
                    "target_function_key",
                    fact.target_function
                        .map(|function| self.fact_stable_key(FactFamily::Function, function.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "target_symbol_key",
                    fact.target_symbol
                        .map(|symbol| self.fact_stable_key(FactFamily::Symbol, symbol.0))
                        .unwrap_or_else(none_value),
                ),
            ]),
        )
    }

    fn refined_call_edge_metadata(&self, fact: &RefinedCallEdgeFact) -> FactMeta {
        let (precision, status_confidence) = call_status_metadata(fact.status, fact.precision);
        let confidence = refined_call_confidence_metadata(fact.confidence, status_confidence);
        let validation = refined_call_validation_metadata(fact.validation);
        fact_meta_from_stable_key_with_validation(
            FactFamily::RefinedCallEdge,
            REFINED_CALLS_PROVIDER_ID,
            precision,
            confidence,
            validation,
            fact.stable_key.clone(),
            stable_parts([
                ("status", call_status_label(fact.status).to_string()),
                (
                    "precision",
                    call_precision_label(fact.precision).to_string(),
                ),
                (
                    "algorithm",
                    call_algorithm_label(fact.algorithm).to_string(),
                ),
                (
                    "edge_kind",
                    call_edge_kind_label(fact.edge_kind).to_string(),
                ),
                ("tier", refined_call_tier_label(fact.tier).to_string()),
                (
                    "validation",
                    refined_call_validation_label(fact.validation).to_string(),
                ),
                (
                    "reason",
                    fact.reason
                        .map(call_unresolved_reason_label)
                        .map(str::to_string)
                        .unwrap_or_else(none_value),
                ),
                (
                    "site_key",
                    self.fact_stable_key(FactFamily::CallSite, fact.site.0),
                ),
                (
                    "base_target_key",
                    fact.base_target
                        .map(|target| self.fact_stable_key(FactFamily::CallTarget, target.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "caller_key",
                    self.fact_stable_key(FactFamily::Function, fact.caller.0),
                ),
                (
                    "target_function_key",
                    fact.target_function
                        .map(|function| self.fact_stable_key(FactFamily::Function, function.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "target_symbol_key",
                    fact.target_symbol
                        .map(|symbol| self.fact_stable_key(FactFamily::Symbol, symbol.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "synthetic_target",
                    fact.synthetic_target.clone().unwrap_or_else(none_value),
                ),
                ("evidence", fact.evidence.join("\n")),
                ("inputs", fact.input_stable_keys.join("\n")),
            ]),
        )
    }

    fn data_flow_node_metadata(&self, fact: &DataFlowNodeFact) -> FactMeta {
        let model = fact
            .model
            .and_then(|id| self.data_flow_models.iter().find(|model| model.id == id));
        let (status, data_flow_precision, data_flow_confidence, data_flow_validation, model_key) =
            model.map_or(
                (
                    DataFlowStatus::Present,
                    DataFlowPrecision::Syntax,
                    DataFlowConfidence::High,
                    DataFlowValidation::Native,
                    none_value(),
                ),
                |model| {
                    (
                        model.status,
                        model.precision,
                        model.confidence,
                        model.validation,
                        model.stable_key.clone(),
                    )
                },
            );
        let (precision, status_confidence) = data_flow_status_metadata(status, data_flow_precision);
        let confidence = data_flow_confidence_metadata(data_flow_confidence, status_confidence);
        let validation = data_flow_validation_metadata(data_flow_validation);
        fact_meta_from_stable_key_with_validation(
            FactFamily::DataFlowNode,
            DATA_FLOW_PROVIDER_ID,
            precision,
            confidence,
            validation,
            fact.stable_key.clone(),
            stable_parts([
                ("kind", format!("{:?}", fact.kind)),
                ("status", data_flow_status_label(status).to_string()),
                (
                    "precision",
                    data_flow_precision_label(data_flow_precision).to_string(),
                ),
                (
                    "validation",
                    data_flow_validation_label(data_flow_validation).to_string(),
                ),
                ("language", language_label(fact.language).to_string()),
                (
                    "file_key",
                    fact.file
                        .map(|file| self.source_file_key(file))
                        .unwrap_or_else(none_value),
                ),
                (
                    "function_key",
                    fact.function
                        .map(|function| self.fact_stable_key(FactFamily::Function, function.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "place_key",
                    fact.place
                        .map(|place| self.fact_stable_key(FactFamily::Place, place.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "symbol_key",
                    fact.symbol
                        .map(|symbol| self.fact_stable_key(FactFamily::Symbol, symbol.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "reference_key",
                    fact.reference
                        .map(|reference| self.fact_stable_key(FactFamily::Reference, reference.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "call_site_key",
                    fact.call_site
                        .map(|site| self.fact_stable_key(FactFamily::CallSite, site.0))
                        .unwrap_or_else(none_value),
                ),
                ("model_key", model_key),
                ("span", option_span_metadata_value(fact.span.as_ref())),
            ]),
        )
    }

    fn data_flow_edge_metadata(&self, fact: &DataFlowEdgeFact) -> FactMeta {
        let (precision, status_confidence) = data_flow_status_metadata(fact.status, fact.precision);
        let confidence = data_flow_confidence_metadata(fact.confidence, status_confidence);
        let validation = data_flow_validation_metadata(fact.validation);
        fact_meta_from_stable_key_with_validation(
            FactFamily::DataFlowEdge,
            DATA_FLOW_PROVIDER_ID,
            precision,
            confidence,
            validation,
            fact.stable_key.clone(),
            stable_parts([
                ("kind", format!("{:?}", fact.kind)),
                ("algorithm", format!("{:?}", fact.algorithm)),
                ("status", data_flow_status_label(fact.status).to_string()),
                (
                    "precision",
                    data_flow_precision_label(fact.precision).to_string(),
                ),
                (
                    "validation",
                    data_flow_validation_label(fact.validation).to_string(),
                ),
                (
                    "from_key",
                    self.fact_stable_key(FactFamily::DataFlowNode, fact.from.0),
                ),
                (
                    "to_key",
                    self.fact_stable_key(FactFamily::DataFlowNode, fact.to.0),
                ),
                (
                    "call_site_key",
                    fact.call_site
                        .map(|site| self.fact_stable_key(FactFamily::CallSite, site.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "call_target_key",
                    fact.call_target
                        .map(|target| self.fact_stable_key(FactFamily::CallTarget, target.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "refined_call_key",
                    fact.refined_call
                        .map(|edge| self.fact_stable_key(FactFamily::RefinedCallEdge, edge.0))
                        .unwrap_or_else(none_value),
                ),
                ("evidence", fact.evidence.join("\n")),
                ("inputs", fact.input_stable_keys.join("\n")),
            ]),
        )
    }

    fn data_flow_model_metadata(&self, fact: &DataFlowModelFact) -> FactMeta {
        let (precision, status_confidence) = data_flow_status_metadata(fact.status, fact.precision);
        let confidence = data_flow_confidence_metadata(fact.confidence, status_confidence);
        let validation = data_flow_validation_metadata(fact.validation);
        fact_meta_from_stable_key_with_validation(
            FactFamily::DataFlowModel,
            DATA_FLOW_PROVIDER_ID,
            precision,
            confidence,
            validation,
            fact.stable_key.clone(),
            stable_parts([
                ("kind", format!("{:?}", fact.kind)),
                ("language", language_label(fact.language).to_string()),
                ("provider_id", fact.provider_id.clone()),
                ("model_id", fact.model_id.clone().unwrap_or_else(none_value)),
                (
                    "source_key",
                    fact.source_stable_key.clone().unwrap_or_else(none_value),
                ),
                ("evidence", fact.evidence.join("\n")),
                ("payload_labels", fact.payload_labels.join("\n")),
            ]),
        )
    }

    fn data_flow_budget_metadata(&self, fact: &DataFlowBudgetFact) -> FactMeta {
        fact_meta_from_stable_key(
            FactFamily::DataFlowBudget,
            DATA_FLOW_PROVIDER_ID,
            FactPrecision::Heuristic,
            FactConfidence::Medium,
            fact.stable_key.clone(),
            stable_parts([
                ("reason", format!("{:?}", fact.reason)),
                ("status", data_flow_status_label(fact.status).to_string()),
                ("limit", fact.limit.to_string()),
                ("observed", fact.observed.to_string()),
            ]),
        )
    }

    fn evidence_node_metadata(&self, fact: &EvidenceNodeFact) -> FactMeta {
        let (precision, status_confidence) = evidence_status_metadata(fact.status, fact.precision);
        let confidence = evidence_confidence_metadata(fact.confidence, status_confidence);
        let validation = evidence_validation_metadata(fact.validation);
        fact_meta_from_stable_key_with_validation(
            FactFamily::EvidenceNode,
            EVIDENCE_PROVIDER_ID,
            precision,
            confidence,
            validation,
            fact.stable_key.clone(),
            stable_parts([
                ("kind", format!("{:?}", fact.kind)),
                ("status", evidence_status_label(fact.status).to_string()),
                (
                    "precision",
                    evidence_precision_label(fact.precision).to_string(),
                ),
                (
                    "provenance",
                    evidence_provenance_label(fact.provenance).to_string(),
                ),
                (
                    "validation",
                    evidence_validation_label(fact.validation).to_string(),
                ),
                ("language", language_label(fact.language).to_string()),
                (
                    "file_key",
                    fact.file
                        .map(|file| self.source_file_key(file))
                        .unwrap_or_else(none_value),
                ),
                (
                    "function_key",
                    fact.function
                        .map(|function| self.fact_stable_key(FactFamily::Function, function.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "place_key",
                    fact.place
                        .map(|place| self.fact_stable_key(FactFamily::Place, place.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "operation_key",
                    fact.operation
                        .map(|operation| {
                            self.fact_stable_key(FactFamily::MirOperation, operation.0)
                        })
                        .unwrap_or_else(none_value),
                ),
                ("span", option_span_metadata_value(fact.span.as_ref())),
                ("sources", fact.source_fact_stable_keys.join("\n")),
            ]),
        )
    }

    fn evidence_edge_metadata(&self, fact: &EvidenceEdgeFact) -> FactMeta {
        let (precision, status_confidence) = evidence_status_metadata(fact.status, fact.precision);
        let confidence = evidence_confidence_metadata(fact.confidence, status_confidence);
        let validation = evidence_validation_metadata(fact.validation);
        fact_meta_from_stable_key_with_validation(
            FactFamily::EvidenceEdge,
            EVIDENCE_PROVIDER_ID,
            precision,
            confidence,
            validation,
            fact.stable_key.clone(),
            stable_parts([
                ("kind", format!("{:?}", fact.kind)),
                ("query_mode", format!("{:?}", fact.query_mode)),
                ("status", evidence_status_label(fact.status).to_string()),
                (
                    "precision",
                    evidence_precision_label(fact.precision).to_string(),
                ),
                (
                    "provenance",
                    evidence_provenance_label(fact.provenance).to_string(),
                ),
                (
                    "validation",
                    evidence_validation_label(fact.validation).to_string(),
                ),
                (
                    "from_key",
                    self.fact_stable_key(FactFamily::EvidenceNode, fact.from.0),
                ),
                (
                    "to_key",
                    self.fact_stable_key(FactFamily::EvidenceNode, fact.to.0),
                ),
                (
                    "call_site_key",
                    fact.call_site
                        .map(|site| self.fact_stable_key(FactFamily::CallSite, site.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "summary_key",
                    fact.summary_stable_key.clone().unwrap_or_else(none_value),
                ),
                ("sources", fact.source_fact_stable_keys.join("\n")),
            ]),
        )
    }

    fn evidence_bundle_metadata(&self, fact: &EvidenceBundleFact) -> FactMeta {
        let (precision, status_confidence) = evidence_status_metadata(fact.status, fact.precision);
        let confidence = evidence_confidence_metadata(fact.confidence, status_confidence);
        let validation = evidence_validation_metadata(fact.validation);
        fact_meta_from_stable_key_with_validation(
            FactFamily::EvidenceBundle,
            EVIDENCE_PROVIDER_ID,
            precision,
            confidence,
            validation,
            fact.stable_key.clone(),
            stable_parts([
                ("diagnostic_key", fact.diagnostic_stable_key.clone()),
                ("query_mode", format!("{:?}", fact.query_mode)),
                ("status", evidence_status_label(fact.status).to_string()),
                (
                    "precision",
                    evidence_precision_label(fact.precision).to_string(),
                ),
                ("selected_paths", fact.selected_paths.len().to_string()),
                ("selected_slices", fact.selected_slices.len().to_string()),
                (
                    "replay_key",
                    fact.replay_key.clone().unwrap_or_else(none_value),
                ),
            ]),
        )
    }

    fn evidence_path_metadata(&self, fact: &EvidencePathFact) -> FactMeta {
        let (precision, confidence) =
            evidence_status_metadata(fact.status, EvidencePrecision::Heuristic);
        fact_meta_from_stable_key(
            FactFamily::EvidencePath,
            EVIDENCE_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("query_mode", format!("{:?}", fact.query_mode)),
                ("status", evidence_status_label(fact.status).to_string()),
                ("rank", fact.rank.to_string()),
                ("node_count", fact.nodes.len().to_string()),
                ("edge_count", fact.edges.len().to_string()),
                ("hidden_node_count", fact.hidden_node_count.to_string()),
            ]),
        )
    }

    fn evidence_slice_metadata(&self, fact: &EvidenceSliceFact) -> FactMeta {
        let (precision, confidence) =
            evidence_status_metadata(fact.status, EvidencePrecision::Heuristic);
        fact_meta_from_stable_key(
            FactFamily::EvidenceSlice,
            EVIDENCE_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("query_mode", format!("{:?}", fact.query_mode)),
                ("status", evidence_status_label(fact.status).to_string()),
                ("root_count", fact.root_nodes.len().to_string()),
                ("node_count", fact.nodes.len().to_string()),
                ("edge_count", fact.edges.len().to_string()),
            ]),
        )
    }

    fn evidence_unknown_metadata(&self, fact: &EvidenceUnknownFact) -> FactMeta {
        fact_meta_from_stable_key(
            FactFamily::EvidenceUnknown,
            EVIDENCE_PROVIDER_ID,
            FactPrecision::Unresolved,
            FactConfidence::Low,
            fact.stable_key.clone(),
            stable_parts([
                ("reason", format!("{:?}", fact.reason)),
                ("message", fact.message.clone()),
                ("sources", fact.source_fact_stable_keys.join("\n")),
            ]),
        )
    }

    fn evidence_omitted_region_metadata(&self, fact: &EvidenceOmittedRegionFact) -> FactMeta {
        fact_meta_from_stable_key(
            FactFamily::EvidenceOmittedRegion,
            EVIDENCE_PROVIDER_ID,
            FactPrecision::Unresolved,
            FactConfidence::Low,
            fact.stable_key.clone(),
            stable_parts([
                ("reason", format!("{:?}", fact.reason)),
                ("hidden_node_count", fact.hidden_node_count.to_string()),
                ("hidden_edge_count", fact.hidden_edge_count.to_string()),
                (
                    "budget_label",
                    fact.budget_label.clone().unwrap_or_else(none_value),
                ),
            ]),
        )
    }

    fn evidence_replay_key_metadata(&self, fact: &EvidenceReplayKeyFact) -> FactMeta {
        fact_meta_from_stable_key(
            FactFamily::EvidenceReplayKey,
            EVIDENCE_PROVIDER_ID,
            FactPrecision::Heuristic,
            FactConfidence::Medium,
            fact.stable_key.clone(),
            stable_parts([
                ("query_mode", format!("{:?}", fact.query_mode)),
                ("graph_schema", fact.graph_schema.clone()),
                ("max_paths", fact.query_budget.max_paths.to_string()),
                ("max_nodes", fact.query_budget.max_nodes.to_string()),
                ("max_edges", fact.query_budget.max_edges.to_string()),
                ("max_depth", fact.query_budget.max_depth.to_string()),
                ("ranking", format!("{:?}", fact.ranking)),
                ("renderer", format!("{:?}", fact.renderer)),
                ("upstream", fact.upstream_digest_keys.join("\n")),
            ]),
        )
    }
}

impl AnalysisDb {
    fn unresolved_call_metadata(&self, fact: &UnresolvedCallFact) -> FactMeta {
        let (precision, confidence) = call_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::UnresolvedCall,
            CALLS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", call_status_label(fact.status).to_string()),
                (
                    "precision",
                    call_precision_label(fact.precision).to_string(),
                ),
                (
                    "algorithm",
                    call_algorithm_label(fact.algorithm).to_string(),
                ),
                (
                    "reason",
                    call_unresolved_reason_label(fact.reason).to_string(),
                ),
                (
                    "site_key",
                    self.fact_stable_key(FactFamily::CallSite, fact.site.0),
                ),
                (
                    "caller_key",
                    self.fact_stable_key(FactFamily::Function, fact.caller.0),
                ),
            ]),
        )
    }

    fn domain_observation_metadata(&self, fact: &DomainObservationFact) -> FactMeta {
        let (precision, confidence) = domain_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::DomainObservation,
            POLINT_ABSTRACT_DOMAINS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", fact.status.as_str().to_string()),
                ("precision", fact.precision.as_str().to_string()),
                ("slot", fact.slot.as_str().to_string()),
                ("location", fact.location.as_str().to_string()),
                (
                    "body_key",
                    self.fact_stable_key(FactFamily::MirBody, fact.body.0),
                ),
                (
                    "block",
                    fact.block
                        .map(|block| block.0.to_string())
                        .unwrap_or_else(none_value),
                ),
                (
                    "operation_key",
                    fact.operation
                        .map(|operation| {
                            self.fact_stable_key(FactFamily::MirOperation, operation.0)
                        })
                        .unwrap_or_else(none_value),
                ),
                (
                    "place_key",
                    fact.place
                        .map(|place| self.fact_stable_key(FactFamily::Place, place.0))
                        .unwrap_or_else(none_value),
                ),
                ("value", fact.value.stable_parts().join("\n")),
            ]),
        )
    }

    fn domain_event_metadata(&self, fact: &DomainEventFact) -> FactMeta {
        let (precision, confidence) = domain_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::DomainEvent,
            POLINT_ABSTRACT_DOMAINS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", fact.status.as_str().to_string()),
                ("precision", fact.precision.as_str().to_string()),
                (
                    "slot",
                    fact.slot
                        .map(|slot| slot.as_str().to_string())
                        .unwrap_or_else(none_value),
                ),
                ("reason", fact.reason.clone()),
                (
                    "body_key",
                    self.fact_stable_key(FactFamily::MirBody, fact.body.0),
                ),
                (
                    "block",
                    fact.block
                        .map(|block| block.0.to_string())
                        .unwrap_or_else(none_value),
                ),
                (
                    "operation_key",
                    fact.operation
                        .map(|operation| {
                            self.fact_stable_key(FactFamily::MirOperation, operation.0)
                        })
                        .unwrap_or_else(none_value),
                ),
            ]),
        )
    }

    fn cfg_function_metadata(&self, fact: &CfgFunctionFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CfgFunction,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                (
                    "body_key",
                    self.fact_stable_key(FactFamily::MirBody, fact.body.0),
                ),
                ("language", language_label(fact.language).to_string()),
                ("path", self.path_for(fact.file)),
                ("span", span_metadata_value(&fact.span)),
            ]),
        )
    }

    fn cfg_node_metadata(&self, fact: &CfgNodeFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CfgNode,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                ("kind", cfg_node_kind_label(fact.kind).to_string()),
                (
                    "function_key",
                    self.fact_stable_key(FactFamily::CfgFunction, fact.cfg_function.0),
                ),
                ("operation_ordinal", fact.operation_ordinal.to_string()),
                ("span", option_span_metadata_value(fact.span.as_ref())),
            ]),
        )
    }

    fn cfg_block_metadata(&self, fact: &BasicBlockFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::BasicBlock,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                ("kind", basic_block_kind_label(fact.kind).to_string()),
                (
                    "function_key",
                    self.fact_stable_key(FactFamily::CfgFunction, fact.cfg_function.0),
                ),
                ("reachable", fact.reachable.to_string()),
                ("reverse_postorder", fact.reverse_postorder.to_string()),
            ]),
        )
    }

    fn cfg_edge_metadata(&self, fact: &CfgEdgeFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CfgEdge,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                ("view", cfg_view_label(fact.view).to_string()),
                ("kind", cfg_edge_kind_label(fact.kind).to_string()),
                (
                    "function_key",
                    self.fact_stable_key(FactFamily::CfgFunction, fact.cfg_function.0),
                ),
                ("from_block", fact.from_block.0.to_string()),
                ("to_block", fact.to_block.0.to_string()),
            ]),
        )
    }

    fn cfg_reachability_metadata(&self, fact: &ReachabilityFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CfgReachability,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                ("view", cfg_view_label(fact.view).to_string()),
                ("block", fact.block.0.to_string()),
                ("reachable", fact.reachable.to_string()),
            ]),
        )
    }

    fn cfg_dominator_metadata(&self, fact: &DominatorFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CfgDominator,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                ("view", cfg_view_label(fact.view).to_string()),
                ("dominator", fact.dominator.0.to_string()),
                ("dominated", fact.dominated.0.to_string()),
                ("immediate", fact.immediate.to_string()),
            ]),
        )
    }

    fn cfg_postdominator_metadata(&self, fact: &PostDominatorFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CfgPostDominator,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                ("view", cfg_view_label(fact.view).to_string()),
                ("postdominator", fact.postdominator.0.to_string()),
                ("postdominated", fact.postdominated.0.to_string()),
                ("immediate", fact.immediate.to_string()),
            ]),
        )
    }

    fn cfg_control_dependence_metadata(&self, fact: &ControlDependenceFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CfgControlDependence,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                ("view", cfg_view_label(fact.view).to_string()),
                ("edge", fact.controlling_edge.0.to_string()),
                (
                    "edge_kind",
                    cfg_edge_kind_label(fact.controlling_edge_kind).to_string(),
                ),
                ("controlled_block", fact.controlled_block.0.to_string()),
            ]),
        )
    }

    fn unsupported_control_flow_metadata(&self, fact: &UnsupportedControlFlowFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::UnsupportedControlFlow,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                ("language", language_label(fact.language).to_string()),
                ("path", self.path_for(fact.file)),
                ("span", span_metadata_value(&fact.span)),
                ("construct", fact.construct.clone()),
                ("source_evidence", fact.source_evidence.clone()),
            ]),
        )
    }

    fn fact_stable_key(&self, family: FactFamily, run_id: u64) -> String {
        self.metadata_for(FactRef::new(family, run_id))
            .map(|metadata| metadata.stable_key.clone())
            .unwrap_or_else(|| format!("<missing:{}:{run_id}>", family.label()))
    }

    fn source_file_key(&self, file: FileId) -> String {
        self.metadata_for(FactRef::new(FactFamily::SourceFile, u64::from(file.0)))
            .map(|metadata| metadata.stable_key.clone())
            .unwrap_or_else(|| self.path_for(file).replace('\\', "/"))
    }

    fn option_source_file_key(&self, file: Option<FileId>) -> String {
        file.map(|file| self.source_file_key(file))
            .unwrap_or_else(none_value)
    }

    fn function_key(&self, function: FunctionId, name: &str, span: &Span) -> String {
        self.metadata_for(FactRef::new(FactFamily::Function, function.0))
            .map(|metadata| metadata.stable_key.clone())
            .unwrap_or_else(|| {
                stable_key_from_parts(
                    FactFamily::Function,
                    &[
                        ("path", self.path_for(span.file)),
                        ("name", name.to_string()),
                        ("span", span_metadata_value(span)),
                    ],
                )
            })
    }

    fn ts_component_metadata(&self, fact: &TsComponentFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::TsComponent,
            TS_SYNTAX_PROVIDER_ID,
            FactPrecision::Heuristic,
            FactConfidence::Medium,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("name", fact.name.clone()),
                ("span", span_metadata_value(&fact.span)),
            ]),
            stable_parts([("function", option_function_id(fact.function))]),
        )
    }

    fn ts_class_metadata(&self, fact: &TsClassFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::TsClass,
            TS_SYNTAX_PROVIDER_ID,
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("name", fact.name.clone()),
                ("span", span_metadata_value(&fact.span)),
            ]),
            stable_parts([
                ("is_exported", fact.is_exported.to_string()),
                ("is_component_like", fact.is_component_like.to_string()),
            ]),
        )
    }

    fn string_literal_metadata(&self, fact: &StringLiteralFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::StringLiteral,
            syntax_provider_for_language(fact.language),
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("language", language_label(fact.language).to_string()),
                ("value", fact.value.clone()),
                ("span", span_metadata_value(&fact.span)),
            ]),
            stable_parts([]),
        )
    }

    fn jsx_attribute_metadata(&self, fact: &JsxAttributeFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::JsxAttribute,
            TS_SYNTAX_PROVIDER_ID,
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("name", fact.name.clone()),
                ("value", option_string(fact.value.as_deref())),
                ("span", span_metadata_value(&fact.span)),
            ]),
            stable_parts([]),
        )
    }
}

fn option_file_path(db: &AnalysisDb, file: Option<FileId>) -> String {
    file.map(|file| db.path_for(file))
        .unwrap_or_else(none_value)
}

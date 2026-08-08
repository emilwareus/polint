//! Analysis database types.
//!
//! Extracted from the core monolith without behaviour changes.

use crate::analysis::access_paths::facts::AccessPathFact;
use crate::analysis::access_paths::store::AccessPathStore;
use crate::analysis::adaptation::facts::{AcceptedModelFact, RejectedModelFact};
use crate::analysis::aliases::facts::AliasAnswerFact;
use crate::analysis::aliases::store::AliasStore;
use crate::analysis::calls::facts::{CallSiteFact, CallTargetFact, UnresolvedCallFact};
use crate::analysis::calls::store::{CallOutput, CallStore};
use crate::analysis::cfg::facts::{
    BasicBlockFact, CfgEdgeFact, CfgFunctionFact, CfgNodeFact, ControlDependenceFact,
    DominatorFact, PostDominatorFact, ReachabilityFact, UnsupportedControlFlowFact,
};
use crate::analysis::cfg::store::CfgOutput;
use crate::analysis::data_flow::facts::{
    DataFlowBudgetFact, DataFlowEdgeFact, DataFlowModelFact, DataFlowNodeFact,
};
use crate::analysis::data_flow::store::{DataFlowOutput, DataFlowStore};
use crate::analysis::domains::facts::{DomainEventFact, DomainObservationFact};
use crate::analysis::domains::store::{DomainOutput, DomainStore};
use crate::analysis::entrypoints::facts::{
    EntrypointFact, FrameworkDispatchEdgeFact, TrustBoundaryFact, UnresolvedFrameworkFact,
};
use crate::analysis::entrypoints::store::EntrypointStore;
use crate::analysis::error::AnalysisError;
use crate::analysis::evidence::facts::{
    EvidenceBundleFact, EvidenceEdgeFact, EvidenceNodeFact, EvidenceOmittedRegionFact,
    EvidencePathFact, EvidenceReplayKeyFact, EvidenceSliceFact, EvidenceUnknownFact,
};
use crate::analysis::evidence::store::{EvidenceOutput, EvidenceStore};
use crate::analysis::extensions::store::{
    AcceptedExtensionFact, ExtensionActivationRow, ExtensionOutput, RejectedExtensionFact,
};
use crate::analysis::identity::facts::IdentityRecord;
use crate::analysis::identity::provider::valid_call_site_ids;
use crate::analysis::identity::store::{IdentityProviderOutput, IdentityStore};
use crate::analysis::mir::body::MirOutput;
use crate::analysis::points_to::facts::{PointsToConstraintFact, PointsToSetFact};
use crate::analysis::points_to::store::PointsToStore;
use crate::analysis::reachability::facts::{CallReachabilityFact, ReachabilityRootFact};
use crate::analysis::refined_calls::facts::RefinedCallEdgeFact;
use crate::analysis::refined_calls::store::{RefinedCallOutput, RefinedCallStore};
use crate::analysis::semantic_graph::constraints::ConstraintFact;
use crate::analysis::semantic_graph::facts::{SemanticEdgeFact, SemanticNodeFact};
use crate::analysis::solver::budget::BudgetStatus;
use crate::analysis::solver::facts::DerivedEdgeFact;
use crate::analysis::store::SemanticStore;
use crate::analysis::summaries::facts::{SummaryEventFact, SummaryFact};
use crate::analysis::summaries::store::{SummaryOutput, SummaryStore};
use crate::analysis::types::facts::{NarrowedTypeFact, TypeFact};
use crate::analysis::types::store::TypeStore;
use crate::analysis::values::facts::{AllocationTokenFact, ValueFact};
use crate::analysis::values::store::ValueStore;
use crate::analysis_kernel::{FactFamily, FactMetaStore};
use crate::diagnostics::fingerprint;
use crate::go::semantic::facts::{
    GoSemanticAddressTakenFact, GoSemanticCallsiteFact, GoSemanticDynamicDispatchFact,
    GoSemanticFunctionFact, GoSemanticInstantiatedTypeFact, GoSemanticMethodSetFact,
    GoSemanticPackageErrorFact, GoSemanticPackageFact, GoSemanticRtaEdgeFact,
};
use crate::module_graph::topology::{
    DependencyRequirementFact, ImportToPackageFact, RepoTopologyOverlayFact,
    ResolvedDependencyEdgeFact, SourceSetFact, TopologyOutput, TopologyPackageFact,
    WorkspaceRootFact,
};
use crate::symbol_graph::semantic::{
    AliasFact, AliasId, ExportFact, ExportId, GeneratedSymbolFact, GeneratedSymbolId,
    ResolutionFact, ResolutionId, ScopeFact, ScopeId, SemanticImportFact, SemanticImportId,
    StableExportId, StableExportIdentity,
};
use crate::ts::object_model::facts::{
    TsObjectAllocationFact, TsPropertyReadFact, TsPropertyWriteFact, TsPrototypeLinkFact,
    TsReceiverBindingFact,
};
use crate::ts::object_model::store::TsObjectModelStore;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use super::facts::{
    BranchObligation, ComplexityMetricFact, CoverageFact, DefinitionFact, FileMetricFact,
    FunctionFact, FunctionMetricFact, ImportFact, JsxAttributeFact, ModuleEdge, ModuleNode,
    PackageFact, ReferenceFact, ResolvedImportFact, SourceFile, StringLiteralFact, SymbolFact,
    TestFact, TsClassFact, TsComponentFact,
};
use super::ids::{
    BranchId, FileId, FunctionId, ImportId, ModuleEdgeId, ModuleNodeId, PackageId,
    ResolvedImportId, SymbolId,
};
use super::lang::Language;
use super::metadata::*;
use super::review::ReviewChangeset;
use super::span::Span;

#[derive(Debug, Clone)]
pub struct AnalysisDb {
    pub(crate) files: Vec<SourceFile>,
    pub(crate) fact_meta: FactMetaStore,
    pub(crate) packages: Vec<PackageFact>,
    pub(crate) functions: Vec<FunctionFact>,
    pub(crate) imports: Vec<ImportFact>,
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
    pub(crate) branches: Vec<BranchObligation>,
    pub(crate) tests: Vec<TestFact>,
    pub(crate) coverage: Vec<CoverageFact>,
    pub(crate) file_metrics: Vec<FileMetricFact>,
    pub(crate) function_metrics: Vec<FunctionMetricFact>,
    pub(crate) complexity_metrics: Vec<ComplexityMetricFact>,
    pub(crate) ts_components: Vec<TsComponentFact>,
    pub(crate) ts_classes: Vec<TsClassFact>,
    pub(crate) string_literals: Vec<StringLiteralFact>,
    pub(crate) jsx_attributes: Vec<JsxAttributeFact>,
    pub(crate) semantic: Option<SemanticStore>,
    pub(crate) cfg_functions: Vec<CfgFunctionFact>,
    pub(crate) cfg_nodes: Vec<CfgNodeFact>,
    pub(crate) cfg_blocks: Vec<BasicBlockFact>,
    pub(crate) cfg_edges: Vec<CfgEdgeFact>,
    pub(crate) cfg_reachability: Vec<ReachabilityFact>,
    pub(crate) cfg_dominators: Vec<DominatorFact>,
    pub(crate) cfg_postdominators: Vec<PostDominatorFact>,
    pub(crate) cfg_control_dependence: Vec<ControlDependenceFact>,
    pub(crate) unsupported_control_flow: Vec<UnsupportedControlFlowFact>,
    pub(crate) call_sites: Vec<CallSiteFact>,
    pub(crate) call_targets: Vec<CallTargetFact>,
    pub(crate) unresolved_calls: Vec<UnresolvedCallFact>,
    pub(crate) call_store: Option<CallStore>,
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
    pub(crate) go_semantic_packages: Vec<GoSemanticPackageFact>,
    pub(crate) go_semantic_functions: Vec<GoSemanticFunctionFact>,
    pub(crate) go_semantic_callsites: Vec<GoSemanticCallsiteFact>,
    pub(crate) go_semantic_method_sets: Vec<GoSemanticMethodSetFact>,
    pub(crate) go_semantic_address_taken: Vec<GoSemanticAddressTakenFact>,
    pub(crate) go_semantic_instantiated_types: Vec<GoSemanticInstantiatedTypeFact>,
    pub(crate) go_semantic_dynamic_dispatch: Vec<GoSemanticDynamicDispatchFact>,
    pub(crate) go_semantic_rta_edges: Vec<GoSemanticRtaEdgeFact>,
    pub(crate) go_semantic_package_errors: Vec<GoSemanticPackageErrorFact>,
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
        Self {
            files: Vec::new(),
            fact_meta: FactMetaStore::default(),
            packages: Vec::new(),
            functions: Vec::new(),
            imports: Vec::new(),
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
            branches: Vec::new(),
            tests: Vec::new(),
            coverage: Vec::new(),
            file_metrics: Vec::new(),
            function_metrics: Vec::new(),
            complexity_metrics: Vec::new(),
            ts_components: Vec::new(),
            ts_classes: Vec::new(),
            string_literals: Vec::new(),
            jsx_attributes: Vec::new(),
            semantic: None,
            cfg_functions: Vec::new(),
            cfg_nodes: Vec::new(),
            cfg_blocks: Vec::new(),
            cfg_edges: Vec::new(),
            cfg_reachability: Vec::new(),
            cfg_dominators: Vec::new(),
            cfg_postdominators: Vec::new(),
            cfg_control_dependence: Vec::new(),
            unsupported_control_flow: Vec::new(),
            call_sites: Vec::new(),
            call_targets: Vec::new(),
            unresolved_calls: Vec::new(),
            call_store: None,
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
            go_semantic_packages: Vec::new(),
            go_semantic_functions: Vec::new(),
            go_semantic_callsites: Vec::new(),
            go_semantic_method_sets: Vec::new(),
            go_semantic_address_taken: Vec::new(),
            go_semantic_instantiated_types: Vec::new(),
            go_semantic_dynamic_dispatch: Vec::new(),
            go_semantic_rta_edges: Vec::new(),
            go_semantic_package_errors: Vec::new(),
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
        let id = PackageId(self.packages.len() as u64);
        fact.id = id;
        let metadata = self.package_metadata(&fact);
        self.packages.push(fact);
        self.record_fact_meta(FactFamily::Package, id.0, metadata);
        id
    }

    pub fn push_function(&mut self, mut fact: FunctionFact) -> FunctionId {
        let id = FunctionId(self.functions.len() as u64);
        fact.id = id;
        let metadata = self.function_metadata(&fact);
        self.functions.push(fact);
        self.record_fact_meta(FactFamily::Function, id.0, metadata);
        id
    }

    pub fn push_import(&mut self, mut fact: ImportFact) -> ImportId {
        let id = ImportId(self.imports.len() as u64);
        fact.id = id;
        let metadata = self.import_metadata(&fact);
        self.imports.push(fact);
        self.record_fact_meta(FactFamily::Import, id.0, metadata);
        id
    }

    pub fn push_branch(&mut self, mut fact: BranchObligation) -> BranchId {
        let id = BranchId(self.branches.len() as u64);
        fact.id = id;
        let metadata = self.branch_metadata(&fact);
        self.branches.push(fact);
        self.record_fact_meta(FactFamily::BranchObligation, id.0, metadata);
        id
    }

    pub fn push_test(&mut self, fact: TestFact) {
        let run_id = self.tests.len() as u64;
        let metadata = self.test_metadata(&fact);
        self.tests.push(fact);
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
        self.cfg_functions = output.functions;
        self.cfg_nodes = output.nodes;
        self.cfg_blocks = output.blocks;
        self.cfg_edges = output.edges;
        self.cfg_reachability = output.reachability;
        self.cfg_dominators = output.dominators;
        self.cfg_postdominators = output.postdominators;
        self.cfg_control_dependence = output.control_dependence;
        self.unsupported_control_flow = output.unsupported;
        self.refresh_cfg_metadata();
        Ok(())
    }

    pub(crate) fn replace_call_facts(
        &mut self,
        mut output: CallOutput,
    ) -> Result<(), AnalysisError> {
        self.populate_call_owner_symbols(&mut output);
        let store = CallStore::from_output(output)?;
        self.call_sites = store.sites().to_vec();
        self.call_targets = store.targets().to_vec();
        self.unresolved_calls = store.unresolved().to_vec();
        self.call_store = Some(store);
        self.refresh_call_metadata();
        Ok(())
    }

    pub(crate) fn replace_identity_facts(
        &mut self,
        output: IdentityProviderOutput,
    ) -> Result<(), AnalysisError> {
        let valid_sites = valid_call_site_ids(self);
        let valid_targets = self
            .call_targets
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
            .functions
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
        &self.call_sites
    }

    pub(crate) fn call_targets(&self) -> &[CallTargetFact] {
        &self.call_targets
    }

    pub(crate) fn unresolved_calls(&self) -> &[UnresolvedCallFact] {
        &self.unresolved_calls
    }

    #[allow(dead_code)]
    pub(crate) fn call_store(&self) -> Option<&CallStore> {
        self.call_store.as_ref()
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
}

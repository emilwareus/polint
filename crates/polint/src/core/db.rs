//! Analysis database types.
//!
//! Extracted from the core monolith without behaviour changes.

use crate::analysis::access_paths::facts::AccessPathFact;
use crate::analysis::access_paths::store::AccessPathStore;
use crate::analysis::adaptation::facts::{AcceptedModelFact, RejectedModelFact};
use crate::analysis::aliases::facts::AliasAnswerFact;
use crate::analysis::aliases::store::AliasStore;
use crate::analysis::calls::facts::{CallSiteFact, CallTargetFact, UnresolvedCallFact};
use crate::analysis::calls::store::CallStore;
use crate::analysis::cfg::facts::{
    BasicBlockFact, CfgEdgeFact, CfgFunctionFact, CfgNodeFact, ControlDependenceFact,
    DominatorFact, PostDominatorFact, ReachabilityFact, UnsupportedControlFlowFact,
};
use crate::analysis::data_flow::facts::{
    DataFlowBudgetFact, DataFlowEdgeFact, DataFlowModelFact, DataFlowNodeFact,
};
use crate::analysis::data_flow::store::DataFlowStore;
use crate::analysis::domains::store::DomainStore;
use crate::analysis::entrypoints::facts::{
    EntrypointFact, FrameworkDispatchEdgeFact, TrustBoundaryFact, UnresolvedFrameworkFact,
};
use crate::analysis::entrypoints::store::EntrypointStore;
use crate::analysis::evidence::facts::{
    EvidenceBundleFact, EvidenceEdgeFact, EvidenceNodeFact, EvidenceOmittedRegionFact,
    EvidencePathFact, EvidenceReplayKeyFact, EvidenceSliceFact, EvidenceUnknownFact,
};
use crate::analysis::evidence::store::EvidenceStore;
use crate::analysis::extensions::store::{
    AcceptedExtensionFact, ExtensionActivationRow, RejectedExtensionFact,
};
use crate::analysis::identity::facts::IdentityRecord;
use crate::analysis::identity::store::IdentityStore;
use crate::analysis::points_to::facts::{PointsToConstraintFact, PointsToSetFact};
use crate::analysis::points_to::store::PointsToStore;
use crate::analysis::reachability::facts::{CallReachabilityFact, ReachabilityRootFact};
use crate::analysis::refined_calls::facts::RefinedCallEdgeFact;
use crate::analysis::refined_calls::store::RefinedCallStore;
use crate::analysis::semantic_graph::constraints::ConstraintFact;
use crate::analysis::semantic_graph::facts::{SemanticEdgeFact, SemanticNodeFact};
use crate::analysis::solver::budget::BudgetStatus;
use crate::analysis::solver::facts::DerivedEdgeFact;
use crate::analysis::store::SemanticStore;
use crate::analysis::summaries::facts::{SummaryEventFact, SummaryFact};
use crate::analysis::summaries::store::SummaryStore;
use crate::analysis::types::facts::{NarrowedTypeFact, TypeFact};
use crate::analysis::types::store::TypeStore;
use crate::analysis::values::facts::{AllocationTokenFact, ValueFact};
use crate::analysis::values::store::ValueStore;
use crate::analysis_kernel::FactMetaStore;
use crate::go::semantic::facts::{
    GoSemanticAddressTakenFact, GoSemanticCallsiteFact, GoSemanticDynamicDispatchFact,
    GoSemanticFunctionFact, GoSemanticInstantiatedTypeFact, GoSemanticMethodSetFact,
    GoSemanticPackageErrorFact, GoSemanticPackageFact, GoSemanticRtaEdgeFact,
};
use crate::module_graph::topology::{
    DependencyRequirementFact, ImportToPackageFact, RepoTopologyOverlayFact,
    ResolvedDependencyEdgeFact, SourceSetFact, TopologyPackageFact, WorkspaceRootFact,
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

use super::facts::{
    BranchObligation, ComplexityMetricFact, CoverageFact, DefinitionFact, FileMetricFact,
    FunctionFact, FunctionMetricFact, ImportFact, JsxAttributeFact, ModuleEdge, ModuleNode,
    PackageFact, ReferenceFact, ResolvedImportFact, SourceFile, StringLiteralFact, SymbolFact,
    TestFact, TsClassFact, TsComponentFact,
};
use super::ids::{FileId, SymbolId};
use super::review::ReviewChangeset;

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

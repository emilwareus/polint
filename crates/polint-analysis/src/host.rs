//! Owner-side typed accessors over [`polint_analysis_api::FactDatabase`].
//!
//! Concrete composition roots (facade `AnalysisDb`, [`crate::LocalAnalysisDb`])
//! implement [`FactDatabase`]; analysis algorithms call these default methods so
//! they never name the facade database type.

use std::sync::Arc;

use polint_analysis_api::{
    FactConfidence, FactDatabase, FactFamily, FactMeta, FactMetaStore, FactPrecision, FactRef,
    FactStore, ValidationStatus,
};
use polint_core::StableKeyId;

use crate::summaries::facts::SummaryDomainKind;
use crate::{POLINT_ABSTRACT_DOMAINS_PROVIDER_ID, POLINT_DIRECT_SUMMARIES_PROVIDER_ID};

use crate::access_paths::facts::AccessPathFact;
use crate::access_paths::store::AccessPathStore;
use crate::aliases::facts::AliasAnswerFact;
use crate::aliases::store::AliasStore;
use crate::calls::facts::{CallSiteFact, CallTargetFact, UnresolvedCallFact};
use crate::calls::store::{CallOutput, CallStore};
use crate::cfg::facts::{
    BasicBlockFact, CfgEdgeFact, CfgFunctionFact, CfgNodeFact, ControlDependenceFact,
    DominatorFact, PostDominatorFact, ReachabilityFact, UnsupportedControlFlowFact,
};
use crate::cfg::store::CfgOutput;
use crate::data_flow::facts::{
    DataFlowBudgetFact, DataFlowEdgeFact, DataFlowModelFact, DataFlowNodeFact,
};
use crate::data_flow::store::{DataFlowOutput, DataFlowStore};
use crate::domains::facts::{DomainEventFact, DomainObservationFact};
use crate::domains::store::{DomainOutput, DomainStore};
use crate::entrypoints::facts::{
    EntrypointFact, FrameworkDispatchEdgeFact, TrustBoundaryFact, UnresolvedFrameworkFact,
};
use crate::entrypoints::store::{EntrypointOutput, EntrypointStore};
use crate::error::AnalysisError;
use crate::evidence::facts::{
    EvidenceBundleFact, EvidenceEdgeFact, EvidenceNodeFact, EvidenceOmittedRegionFact,
    EvidencePathFact, EvidenceReplayKeyFact, EvidenceSliceFact, EvidenceUnknownFact,
};
use crate::evidence::store::{EvidenceOutput, EvidenceStore};
use crate::extensions::store::{
    AcceptedExtensionFact, ExtensionActivationRow, ExtensionOutput, RejectedExtensionFact,
};
use crate::fact_store::{
    ACCESS_PATH_STORE_FAMILY, ADAPTATION_STORE_FAMILY, ALIAS_STORE_FAMILY, AdaptationFactStore,
    CALL_STORE_FAMILY, CFG_STORE_FAMILY, CfgFactStore, DATA_FLOW_STORE_FAMILY, DOMAIN_STORE_FAMILY,
    ENTRYPOINT_STORE_FAMILY, EVIDENCE_STORE_FAMILY, EXTENSION_STORE_FAMILY, ExtensionFactStore,
    IDENTITY_STORE_FAMILY, POINTS_TO_STORE_FAMILY, REACHABILITY_STORE_FAMILY,
    REFINED_CALL_STORE_FAMILY, SEMANTIC_GRAPH_STORE_FAMILY, SEMANTIC_MIR_STORE_FAMILY,
    SOLVER_STORE_FAMILY, SUMMARY_STORE_FAMILY, TYPE_STORE_FAMILY, VALUE_STORE_FAMILY,
};
use crate::identity::facts::IdentityRecord;
use crate::identity::store::IdentityStore;
use crate::ids::CallSiteId;
use crate::mir_body::MirOutput;
use crate::mir_body::{MirBlock, MirBody};
use crate::mir_body::{MirStatement, MirTerminator};
use crate::mir_op::MirOperation;
use crate::mir_op::UnsupportedSemanticFact;
use crate::places::{PlaceFact, PlaceTypeFact};
use crate::points_to::facts::{PointsToConstraintFact, PointsToSetFact};
use crate::points_to::store::PointsToStore;
use crate::reachability::facts::ReachabilityRootFact;
use crate::reachability::store::{ReachabilityProviderOutput, ReachabilityStore};
use crate::refined_calls::facts::RefinedCallEdgeFact;
use crate::refined_calls::store::{RefinedCallOutput, RefinedCallStore};
use crate::semantic_graph::constraints::ConstraintFact;
use crate::semantic_graph::facts::{SemanticEdgeFact, SemanticNodeFact};
use crate::semantic_graph::store::{SemanticGraphOutput, SemanticGraphStore};
use crate::solver::budget::BudgetStatus;
use crate::solver::facts::DerivedEdgeFact;
use crate::solver::store::{SolverOutput, SolverStore};
use crate::store::SemanticStore;
use crate::summaries::facts::{SummaryEventFact, SummaryFact};
use crate::summaries::store::{SummaryOutput, SummaryStore};
use crate::types::facts::{NarrowedTypeFact, TypeFact};
use crate::types::store::{TypeStore, TypeValueAliasOutput};
use crate::values::facts::{AllocationTokenFact, ValueFact};
use crate::values::store::ValueStore;

fn store_ref<T: FactStore + 'static>(db: &(impl FactDatabase + ?Sized), family: FactFamily) -> &T {
    db.store(family)
        .and_then(|entry| entry.as_any().downcast_ref::<T>())
        .unwrap_or_else(|| panic!("analysis store {family:?} is installed"))
}

fn store_mut<T: FactStore + 'static>(
    db: &mut (impl FactDatabase + ?Sized),
    family: FactFamily,
) -> &mut T {
    db.store_mut(family)
        .and_then(|entry| entry.as_any_mut().downcast_mut::<T>())
        .unwrap_or_else(|| panic!("analysis store {family:?} is installed"))
}

/// Typed analysis store access and replace helpers for any [`FactDatabase`].
pub trait AnalysisHost: FactDatabase {
    fn resolve_stable_key(&self, id: StableKeyId) -> Arc<str> {
        self.stable_key_interner().resolve(id)
    }

    fn fact_meta_mut_for_test(&mut self) -> &mut FactMetaStore {
        self.fact_meta_mut()
    }

    fn cfg_store(&self) -> &CfgFactStore {
        store_ref(self, CFG_STORE_FAMILY)
    }
    fn cfg_store_mut(&mut self) -> &mut CfgFactStore {
        store_mut(self, CFG_STORE_FAMILY)
    }
    fn calls_store(&self) -> &CallStore {
        store_ref(self, CALL_STORE_FAMILY)
    }
    fn calls_store_mut(&mut self) -> &mut CallStore {
        store_mut(self, CALL_STORE_FAMILY)
    }
    fn summary_store(&self) -> &SummaryStore {
        store_ref(self, SUMMARY_STORE_FAMILY)
    }
    fn summary_store_mut(&mut self) -> &mut SummaryStore {
        store_mut(self, SUMMARY_STORE_FAMILY)
    }
    fn semantic_mir_store(&self) -> &SemanticStore {
        store_ref(self, SEMANTIC_MIR_STORE_FAMILY)
    }
    fn semantic_mir_store_mut(&mut self) -> &mut SemanticStore {
        store_mut(self, SEMANTIC_MIR_STORE_FAMILY)
    }
    fn identity_store_inner(&self) -> &IdentityStore {
        store_ref(self, IDENTITY_STORE_FAMILY)
    }
    fn refined_call_store_inner(&self) -> &RefinedCallStore {
        store_ref(self, REFINED_CALL_STORE_FAMILY)
    }
    fn data_flow_store_inner(&self) -> &DataFlowStore {
        store_ref(self, DATA_FLOW_STORE_FAMILY)
    }
    fn evidence_store_inner(&self) -> &EvidenceStore {
        store_ref(self, EVIDENCE_STORE_FAMILY)
    }
    fn domain_store_inner(&self) -> &DomainStore {
        store_ref(self, DOMAIN_STORE_FAMILY)
    }
    fn entrypoint_store_inner(&self) -> &EntrypointStore {
        store_ref(self, ENTRYPOINT_STORE_FAMILY)
    }
    fn type_store_inner(&self) -> &TypeStore {
        store_ref(self, TYPE_STORE_FAMILY)
    }
    fn value_store_inner(&self) -> &ValueStore {
        store_ref(self, VALUE_STORE_FAMILY)
    }
    fn access_path_store_inner(&self) -> &AccessPathStore {
        store_ref(self, ACCESS_PATH_STORE_FAMILY)
    }
    fn points_to_store_inner(&self) -> &PointsToStore {
        store_ref(self, POINTS_TO_STORE_FAMILY)
    }
    fn alias_store_inner(&self) -> &AliasStore {
        store_ref(self, ALIAS_STORE_FAMILY)
    }
    fn extension_store_inner(&self) -> &ExtensionFactStore {
        store_ref(self, EXTENSION_STORE_FAMILY)
    }
    fn adaptation_store_inner(&self) -> &AdaptationFactStore {
        store_ref(self, ADAPTATION_STORE_FAMILY)
    }
    fn reachability_store_inner(&self) -> &ReachabilityStore {
        store_ref(self, REACHABILITY_STORE_FAMILY)
    }
    fn semantic_graph_store_inner(&self) -> &SemanticGraphStore {
        store_ref(self, SEMANTIC_GRAPH_STORE_FAMILY)
    }
    fn solver_store_inner(&self) -> &SolverStore {
        store_ref(self, SOLVER_STORE_FAMILY)
    }

    fn summary_facts(&self) -> &[SummaryFact] {
        self.summary_store().all_summaries()
    }
    fn summary_events(&self) -> &[SummaryEventFact] {
        self.summary_store().all_events()
    }

    fn unresolved_calls(&self) -> &[UnresolvedCallFact] {
        self.calls_store().unresolved()
    }

    fn unsupported_semantics(&self) -> &[UnsupportedSemanticFact] {
        self.semantic_mir_store().unsupported_semantics()
    }

    fn mir_bodies(&self) -> &[MirBody] {
        self.semantic_mir_store().mir_bodies()
    }

    fn mir_operations(&self) -> &[MirOperation] {
        self.semantic_mir_store().mir_operations()
    }

    fn mir_blocks(&self) -> &[MirBlock] {
        self.semantic_mir_store().mir_blocks()
    }

    fn mir_places(&self) -> &[PlaceFact] {
        self.semantic_mir_store().places()
    }

    fn mir_place_types(&self) -> &[PlaceTypeFact] {
        self.semantic_mir_store().place_types()
    }

    fn call_sites(&self) -> &[CallSiteFact] {
        self.calls_store().sites()
    }

    fn call_targets(&self) -> &[CallTargetFact] {
        self.calls_store().targets()
    }

    fn replace_summary_facts(&mut self, output: SummaryOutput) {
        let interner = self.stable_key_interner();
        let store = SummaryStore::from_output(output, &interner)
            .expect("summary output should produce a valid store");
        *self.summary_store_mut() = store;
        refresh_summary_metadata(self);
    }

    fn replace_call_facts(&mut self, output: CallOutput) -> Result<(), AnalysisError> {
        let interner = self.stable_key_interner();
        let store = CallStore::from_output(output, &interner)?;
        *self.calls_store_mut() = store;
        Ok(())
    }

    fn replace_cfg_facts(&mut self, output: CfgOutput) -> Result<(), AnalysisError> {
        self.cfg_store_mut().replace(output);
        Ok(())
    }

    fn replace_semantic_mir(&mut self, output: MirOutput) -> Result<(), AnalysisError> {
        let interner = self.stable_key_interner();
        *self.semantic_mir_store_mut() = SemanticStore::from_output(output, &interner)?;
        Ok(())
    }

    fn replace_refined_call_facts(
        &mut self,
        output: RefinedCallOutput,
    ) -> Result<(), AnalysisError> {
        let interner = self.stable_key_interner();
        *store_mut::<RefinedCallStore>(self, REFINED_CALL_STORE_FAMILY) =
            RefinedCallStore::from_output(output, &interner)?;
        Ok(())
    }

    fn replace_entrypoint_facts(&mut self, output: EntrypointOutput) -> Result<(), AnalysisError> {
        let interner = self.stable_key_interner();
        *store_mut::<EntrypointStore>(self, ENTRYPOINT_STORE_FAMILY) =
            EntrypointStore::from_output(output, &interner)?;
        Ok(())
    }

    fn replace_abstract_domain_facts(&mut self, output: DomainOutput) {
        let interner = self.stable_key_interner();
        *store_mut::<DomainStore>(self, DOMAIN_STORE_FAMILY) =
            DomainStore::from_output(output, &interner);
        refresh_abstract_domain_metadata(self);
    }

    fn mir_statements(&self) -> &[MirStatement] {
        self.semantic_mir_store().mir_statements()
    }

    fn mir_terminators(&self) -> &[MirTerminator] {
        self.semantic_mir_store().mir_terminators()
    }

    fn cfg_functions(&self) -> &[CfgFunctionFact] {
        self.cfg_store().functions()
    }
    fn cfg_nodes(&self) -> &[CfgNodeFact] {
        self.cfg_store().nodes()
    }
    fn cfg_blocks(&self) -> &[BasicBlockFact] {
        self.cfg_store().blocks()
    }
    fn cfg_edges(&self) -> &[CfgEdgeFact] {
        self.cfg_store().edges()
    }
    fn cfg_reachability(&self) -> &[ReachabilityFact] {
        self.cfg_store().reachability()
    }
    fn cfg_dominators(&self) -> &[DominatorFact] {
        self.cfg_store().dominators()
    }
    fn cfg_postdominators(&self) -> &[PostDominatorFact] {
        self.cfg_store().postdominators()
    }
    fn cfg_control_dependence(&self) -> &[ControlDependenceFact] {
        self.cfg_store().control_dependence()
    }
    fn unsupported_control_flow(&self) -> &[UnsupportedControlFlowFact] {
        self.cfg_store().unsupported()
    }

    fn refined_call_edges(&self) -> &[RefinedCallEdgeFact] {
        self.refined_call_store_inner().edges()
    }

    fn data_flow_nodes(&self) -> &[DataFlowNodeFact] {
        self.data_flow_store_inner().nodes()
    }
    fn data_flow_edges(&self) -> &[DataFlowEdgeFact] {
        self.data_flow_store_inner().edges()
    }
    fn data_flow_models(&self) -> &[DataFlowModelFact] {
        self.data_flow_store_inner().models()
    }
    fn data_flow_budgets(&self) -> &[DataFlowBudgetFact] {
        self.data_flow_store_inner().budgets()
    }

    fn evidence_nodes(&self) -> &[EvidenceNodeFact] {
        self.evidence_store_inner().nodes()
    }
    fn evidence_edges(&self) -> &[EvidenceEdgeFact] {
        self.evidence_store_inner().edges()
    }
    fn evidence_bundles(&self) -> &[EvidenceBundleFact] {
        self.evidence_store_inner().bundles()
    }
    fn evidence_paths(&self) -> &[EvidencePathFact] {
        self.evidence_store_inner().paths()
    }
    fn evidence_slices(&self) -> &[EvidenceSliceFact] {
        self.evidence_store_inner().slices()
    }
    fn evidence_unknowns(&self) -> &[EvidenceUnknownFact] {
        self.evidence_store_inner().unknowns()
    }
    fn evidence_omitted_regions(&self) -> &[EvidenceOmittedRegionFact] {
        self.evidence_store_inner().omitted_regions()
    }
    fn evidence_replay_keys(&self) -> &[EvidenceReplayKeyFact] {
        self.evidence_store_inner().replay_keys()
    }

    fn abstract_domain_observations(&self) -> &[DomainObservationFact] {
        self.domain_store_inner().observations()
    }
    fn abstract_domain_events(&self) -> &[DomainEventFact] {
        self.domain_store_inner().events()
    }

    fn entrypoint_facts(&self) -> &[EntrypointFact] {
        self.entrypoint_store_inner().entrypoints()
    }
    fn trust_boundary_facts(&self) -> &[TrustBoundaryFact] {
        self.entrypoint_store_inner().trust_boundaries()
    }
    fn dispatch_edge_facts(&self) -> &[FrameworkDispatchEdgeFact] {
        self.entrypoint_store_inner().dispatch_edges()
    }
    fn unresolved_framework_facts(&self) -> &[UnresolvedFrameworkFact] {
        self.entrypoint_store_inner().unresolved()
    }

    fn type_facts(&self) -> &[TypeFact] {
        self.type_store_inner().types()
    }
    fn narrowed_type_facts(&self) -> &[NarrowedTypeFact] {
        self.type_store_inner().narrowed()
    }
    fn value_facts(&self) -> &[ValueFact] {
        self.value_store_inner().values()
    }
    fn allocation_tokens(&self) -> &[AllocationTokenFact] {
        self.value_store_inner().allocations()
    }
    fn access_path_facts(&self) -> &[AccessPathFact] {
        self.access_path_store_inner().access_paths()
    }
    fn points_to_constraints(&self) -> &[PointsToConstraintFact] {
        self.points_to_store_inner().constraints()
    }
    fn points_to_sets(&self) -> &[PointsToSetFact] {
        self.points_to_store_inner().sets()
    }
    fn alias_answers(&self) -> &[AliasAnswerFact] {
        self.alias_store_inner().answers()
    }

    fn identity_records(&self) -> &[IdentityRecord] {
        self.identity_store_inner().records()
    }

    fn reachability_roots(&self) -> &[ReachabilityRootFact] {
        self.reachability_store_inner().roots()
    }

    fn semantic_nodes(&self) -> &[SemanticNodeFact] {
        self.semantic_graph_store_inner().nodes()
    }
    fn semantic_edges(&self) -> &[SemanticEdgeFact] {
        self.semantic_graph_store_inner().edges()
    }
    fn semantic_constraints(&self) -> &[ConstraintFact] {
        self.semantic_graph_store_inner().constraints()
    }

    fn solver_derived_edges(&self) -> &[DerivedEdgeFact] {
        self.solver_store_inner().derived_edges()
    }
    fn solver_budget_status(&self) -> BudgetStatus {
        self.solver_store_inner().budget_status()
    }
    fn solver_budget_reasons(&self) -> &std::collections::BTreeSet<String> {
        self.solver_store_inner().budget_reasons()
    }

    fn extension_facts(&self) -> &[AcceptedExtensionFact] {
        self.extension_store_inner().accepted.as_slice()
    }
    fn extension_activations(&self) -> &[ExtensionActivationRow] {
        self.extension_store_inner().activations.as_slice()
    }
    fn rejected_extension_facts(&self) -> &[RejectedExtensionFact] {
        self.extension_store_inner().rejected.as_slice()
    }

    fn call_sites_by_caller(&self, caller: polint_core::FunctionId) -> Vec<&CallSiteFact> {
        self.calls_store().sites_by_caller(caller)
    }
    fn call_targets_by_site(&self, site: CallSiteId) -> Vec<&CallTargetFact> {
        self.calls_store().targets_by_site(site)
    }

    fn metadata_for(&self, fact_ref: FactRef) -> Option<&FactMeta> {
        self.fact_meta().get(fact_ref)
    }

    fn replace_data_flow_facts(&mut self, output: DataFlowOutput) -> Result<(), AnalysisError> {
        let interner = self.stable_key_interner();
        *store_mut::<DataFlowStore>(self, DATA_FLOW_STORE_FAMILY) =
            DataFlowStore::from_output(output, &interner)?;
        Ok(())
    }

    fn replace_evidence_facts(&mut self, output: EvidenceOutput) -> Result<(), AnalysisError> {
        let interner = self.stable_key_interner();
        *store_mut::<EvidenceStore>(self, EVIDENCE_STORE_FAMILY) =
            EvidenceStore::from_output(output, &interner)?;
        Ok(())
    }

    fn replace_reachability_facts(
        &mut self,
        output: ReachabilityProviderOutput,
    ) -> Result<(), AnalysisError> {
        let interner = self.stable_key_interner();
        let valid_function_ids = self
            .functions()
            .iter()
            .map(|row| row.id)
            .collect::<std::collections::BTreeSet<_>>();
        let valid_entrypoint_ids = self
            .entrypoint_facts()
            .iter()
            .map(|row| row.id)
            .collect::<std::collections::BTreeSet<_>>();
        let store = ReachabilityStore::from_output(
            output,
            &interner,
            &valid_function_ids,
            &valid_entrypoint_ids,
        )?;
        *store_mut::<ReachabilityStore>(self, REACHABILITY_STORE_FAMILY) = store;
        Ok(())
    }

    fn replace_solver_facts(&mut self, output: SolverOutput) -> Result<(), AnalysisError> {
        let interner = self.stable_key_interner();
        *store_mut::<SolverStore>(self, SOLVER_STORE_FAMILY) =
            SolverStore::from_output(output, &interner)?;
        Ok(())
    }

    fn replace_type_value_alias_facts(&mut self, output: TypeValueAliasOutput) {
        let interner = self.stable_key_interner();
        let output = output.normalized(&interner);
        *store_mut::<TypeStore>(self, TYPE_STORE_FAMILY) =
            TypeStore::from_normalized_output(output.types);
        *store_mut::<ValueStore>(self, VALUE_STORE_FAMILY) =
            ValueStore::from_normalized_output(output.values);
        *store_mut::<AccessPathStore>(self, ACCESS_PATH_STORE_FAMILY) =
            AccessPathStore::from_normalized_output(output.access_paths);
        *store_mut::<PointsToStore>(self, POINTS_TO_STORE_FAMILY) =
            PointsToStore::from_normalized_output(output.points_to);
        *store_mut::<AliasStore>(self, ALIAS_STORE_FAMILY) =
            AliasStore::from_normalized_output(output.aliases);
    }

    fn replace_extension_facts(&mut self, output: ExtensionOutput) {
        let output = output.normalized(&self.stable_key_interner());
        let store = store_mut::<ExtensionFactStore>(self, EXTENSION_STORE_FAMILY);
        store.activations = output.activations;
        store.accepted = output.accepted;
        store.rejected = output.rejected;
    }

    fn replace_semantic_graph_facts(
        &mut self,
        output: SemanticGraphOutput,
    ) -> Result<(), AnalysisError> {
        let interner = self.stable_key_interner();
        *store_mut::<SemanticGraphStore>(self, SEMANTIC_GRAPH_STORE_FAMILY) =
            SemanticGraphStore::from_output(output, &interner)?;
        Ok(())
    }
}

impl<T: FactDatabase + ?Sized> AnalysisHost for T {}

fn host_fact_meta(
    producer_id: &'static str,
    stable_key: StableKeyId,
    payload_digest: String,
) -> FactMeta {
    FactMeta {
        stable_key,
        producer_id,
        layer_id: producer_id,
        precision: FactPrecision::Heuristic,
        confidence: FactConfidence::Medium,
        validation: ValidationStatus::NativeTrusted,
        payload_digest,
    }
}

fn summary_domain_family(domain: SummaryDomainKind) -> FactFamily {
    match domain {
        SummaryDomainKind::ControlEffects => FactFamily::SummaryControl,
        SummaryDomainKind::CallEffects => FactFamily::SummaryCall,
        SummaryDomainKind::MemoryEffects => FactFamily::SummaryMemory,
        SummaryDomainKind::DataFlowTito => FactFamily::SummaryTito,
    }
}

fn refresh_summary_metadata(db: &mut (impl AnalysisHost + ?Sized)) {
    let summaries = db.summary_facts().to_vec();
    let events = db.summary_events().to_vec();

    {
        let meta = db.fact_meta_mut();
        meta.remove_family(FactFamily::SummaryControl);
        meta.remove_family(FactFamily::SummaryCall);
        meta.remove_family(FactFamily::SummaryMemory);
        meta.remove_family(FactFamily::SummaryTito);
        meta.remove_family(FactFamily::SummaryEvent);
        for fact in &summaries {
            let family = summary_domain_family(fact.domain);
            meta.insert(
                FactRef::new(family, fact.id.0),
                host_fact_meta(
                    POLINT_DIRECT_SUMMARIES_PROVIDER_ID,
                    fact.stable_key,
                    format!("summary:{}", fact.id.0),
                ),
            );
        }
        for fact in &events {
            meta.insert(
                FactRef::new(FactFamily::SummaryEvent, fact.id.0),
                host_fact_meta(
                    POLINT_DIRECT_SUMMARIES_PROVIDER_ID,
                    fact.stable_key,
                    format!("summary-event:{}", fact.id.0),
                ),
            );
        }
        for family in [
            FactFamily::SummaryControl,
            FactFamily::SummaryCall,
            FactFamily::SummaryMemory,
            FactFamily::SummaryTito,
            FactFamily::SummaryEvent,
        ] {
            meta.finish_family_insertions(family);
        }
    }
}

fn refresh_abstract_domain_metadata(db: &mut (impl AnalysisHost + ?Sized)) {
    let observations = db.domain_store_inner().observations().to_vec();
    let events = db.domain_store_inner().events().to_vec();

    {
        let meta = db.fact_meta_mut();
        meta.remove_family(FactFamily::DomainObservation);
        meta.remove_family(FactFamily::DomainEvent);
        for fact in &observations {
            meta.insert(
                FactRef::new(FactFamily::DomainObservation, fact.id.0),
                host_fact_meta(
                    POLINT_ABSTRACT_DOMAINS_PROVIDER_ID,
                    fact.stable_key,
                    format!("domain-obs:{}", fact.id.0),
                ),
            );
        }
        for fact in &events {
            meta.insert(
                FactRef::new(FactFamily::DomainEvent, fact.id.0),
                host_fact_meta(
                    POLINT_ABSTRACT_DOMAINS_PROVIDER_ID,
                    fact.stable_key,
                    format!("domain-event:{}", fact.id.0),
                ),
            );
        }
        meta.finish_family_insertions(FactFamily::DomainObservation);
        meta.finish_family_insertions(FactFamily::DomainEvent);
    }
}

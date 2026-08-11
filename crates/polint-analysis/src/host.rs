//! Owner-side typed accessors over [`polint_analysis_api::FactDatabase`].
//!
//! Concrete composition roots (facade `AnalysisDb`, [`crate::LocalAnalysisDb`])
//! implement [`FactDatabase`]; analysis algorithms call these default methods so
//! they never name the facade database type.

use std::sync::Arc;

use polint_analysis_api::{FactDatabase, FactFamily, FactMetaStore, FactStore};
use polint_core::StableKeyId;

use crate::access_paths::store::AccessPathStore;
use crate::aliases::store::AliasStore;
use crate::calls::facts::{CallSiteFact, CallTargetFact, UnresolvedCallFact};
use crate::calls::store::{CallOutput, CallStore};
use crate::cfg::store::CfgOutput;
use crate::data_flow::store::DataFlowStore;
use crate::domains::store::{DomainOutput, DomainStore};
use crate::entrypoints::store::{EntrypointOutput, EntrypointStore};
use crate::error::AnalysisError;
use crate::evidence::store::EvidenceStore;
use crate::fact_store::{
    ACCESS_PATH_STORE_FAMILY, ADAPTATION_STORE_FAMILY, ALIAS_STORE_FAMILY, AdaptationFactStore,
    CALL_STORE_FAMILY, CFG_STORE_FAMILY, CfgFactStore, DATA_FLOW_STORE_FAMILY, DOMAIN_STORE_FAMILY,
    ENTRYPOINT_STORE_FAMILY, EVIDENCE_STORE_FAMILY, EXTENSION_STORE_FAMILY, ExtensionFactStore,
    IDENTITY_STORE_FAMILY, POINTS_TO_STORE_FAMILY, REACHABILITY_STORE_FAMILY,
    REFINED_CALL_STORE_FAMILY, SEMANTIC_GRAPH_STORE_FAMILY, SEMANTIC_MIR_STORE_FAMILY,
    SOLVER_STORE_FAMILY, SUMMARY_STORE_FAMILY, TYPE_STORE_FAMILY, VALUE_STORE_FAMILY,
};
use crate::identity::store::IdentityStore;
use crate::mir_body::MirOutput;
use crate::mir_body::{MirBlock, MirBody};
use crate::mir_op::MirOperation;
use crate::mir_op::UnsupportedSemanticFact;
use crate::places::{PlaceFact, PlaceTypeFact};
use crate::points_to::store::PointsToStore;
use crate::reachability::store::ReachabilityStore;
use crate::refined_calls::store::{RefinedCallOutput, RefinedCallStore};
use crate::semantic_graph::store::{SemanticGraphOutput, SemanticGraphStore};
use crate::solver::store::SolverStore;
use crate::store::SemanticStore;
use crate::summaries::facts::{SummaryEventFact, SummaryFact};
use crate::summaries::store::{SummaryOutput, SummaryStore};
use crate::types::store::TypeStore;
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

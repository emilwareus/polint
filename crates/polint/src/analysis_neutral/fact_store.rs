//! FactStore implementations and registry keys for language-neutral analysis stores.

use std::any::Any;

use crate::analysis_api::{FactFamily, FactStore};

use crate::analysis_neutral::access_paths::store::AccessPathStore;
use crate::analysis_neutral::adaptation::facts::{AcceptedModelFact, RejectedModelFact};
use crate::analysis_neutral::aliases::store::AliasStore;
use crate::analysis_neutral::calls::store::CallStore;
use crate::analysis_neutral::cfg::facts::{
    BasicBlockFact, CfgEdgeFact, CfgFunctionFact, CfgNodeFact, ControlDependenceFact,
    DominatorFact, PostDominatorFact, ReachabilityFact, UnsupportedControlFlowFact,
};
use crate::analysis_neutral::cfg::store::CfgOutput;
use crate::analysis_neutral::data_flow::store::DataFlowStore;
use crate::analysis_neutral::domains::store::DomainStore;
use crate::analysis_neutral::entrypoints::store::EntrypointStore;
use crate::analysis_neutral::evidence::store::EvidenceStore;
use crate::analysis_neutral::extensions::store::{
    AcceptedExtensionFact, ExtensionActivationRow, RejectedExtensionFact,
};
use crate::analysis_neutral::identity::store::IdentityStore;
use crate::analysis_neutral::points_to::store::PointsToStore;
use crate::analysis_neutral::reachability::store::ReachabilityStore;
use crate::analysis_neutral::refined_calls::store::RefinedCallStore;
use crate::analysis_neutral::semantic_graph::store::SemanticGraphStore;
use crate::analysis_neutral::solver::store::SolverStore;
use crate::analysis_neutral::store::SemanticStore;
use crate::analysis_neutral::summaries::store::SummaryStore;
use crate::analysis_neutral::types::store::TypeStore;
use crate::analysis_neutral::values::store::ValueStore;

/// CFG facts produced by `polint.cfg`.
#[derive(Debug, Clone, Default)]
pub struct CfgFactStore {
    output: CfgOutput,
}

impl CfgFactStore {
    pub fn replace(&mut self, output: CfgOutput) {
        self.output = output;
    }

    pub fn functions(&self) -> &[CfgFunctionFact] {
        &self.output.functions
    }

    pub fn nodes(&self) -> &[CfgNodeFact] {
        &self.output.nodes
    }

    pub fn blocks(&self) -> &[BasicBlockFact] {
        &self.output.blocks
    }

    pub fn edges(&self) -> &[CfgEdgeFact] {
        &self.output.edges
    }

    pub fn reachability(&self) -> &[ReachabilityFact] {
        &self.output.reachability
    }

    pub fn dominators(&self) -> &[DominatorFact] {
        &self.output.dominators
    }

    pub fn postdominators(&self) -> &[PostDominatorFact] {
        &self.output.postdominators
    }

    pub fn control_dependence(&self) -> &[ControlDependenceFact] {
        &self.output.control_dependence
    }

    pub fn unsupported(&self) -> &[UnsupportedControlFlowFact] {
        &self.output.unsupported
    }
}

impl FactStore for CfgFactStore {
    fn family(&self) -> FactFamily {
        FactFamily::CfgFunction
    }

    fn clear(&mut self) {
        self.output = CfgOutput::empty();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_box(&self) -> Box<dyn FactStore> {
        Box::new(self.clone())
    }
}

pub const CFG_STORE_FAMILY: FactFamily = FactFamily::CfgFunction;

macro_rules! impl_fact_store {
    ($ty:ty, $family:expr, $const:ident) => {
        impl FactStore for $ty {
            fn family(&self) -> FactFamily {
                $family
            }
            fn clear(&mut self) {
                *self = <$ty>::default();
            }
            fn as_any(&self) -> &dyn Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn Any {
                self
            }
            fn clone_box(&self) -> Box<dyn FactStore> {
                Box::new(self.clone())
            }
        }
        pub const $const: FactFamily = $family;
    };
}

impl_fact_store!(CallStore, FactFamily::CallSite, CALL_STORE_FAMILY);
impl_fact_store!(IdentityStore, FactFamily::Identity, IDENTITY_STORE_FAMILY);
impl_fact_store!(
    RefinedCallStore,
    FactFamily::RefinedCallEdge,
    REFINED_CALL_STORE_FAMILY
);
impl_fact_store!(
    DataFlowStore,
    FactFamily::DataFlowNode,
    DATA_FLOW_STORE_FAMILY
);
impl_fact_store!(
    EvidenceStore,
    FactFamily::EvidenceNode,
    EVIDENCE_STORE_FAMILY
);
impl_fact_store!(
    DomainStore,
    FactFamily::DomainObservation,
    DOMAIN_STORE_FAMILY
);
impl_fact_store!(
    SummaryStore,
    FactFamily::SummaryControl,
    SUMMARY_STORE_FAMILY
);
impl_fact_store!(
    EntrypointStore,
    FactFamily::Entrypoint,
    ENTRYPOINT_STORE_FAMILY
);
impl_fact_store!(TypeStore, FactFamily::Type, TYPE_STORE_FAMILY);
impl_fact_store!(ValueStore, FactFamily::Value, VALUE_STORE_FAMILY);
impl_fact_store!(
    AccessPathStore,
    FactFamily::AccessPath,
    ACCESS_PATH_STORE_FAMILY
);
impl_fact_store!(
    PointsToStore,
    FactFamily::PointsToSet,
    POINTS_TO_STORE_FAMILY
);
impl_fact_store!(AliasStore, FactFamily::AliasAnswer, ALIAS_STORE_FAMILY);
impl_fact_store!(
    ReachabilityStore,
    FactFamily::Reachability,
    REACHABILITY_STORE_FAMILY
);
impl_fact_store!(
    SemanticGraphStore,
    FactFamily::SemanticGraph,
    SEMANTIC_GRAPH_STORE_FAMILY
);
impl_fact_store!(
    SolverStore,
    FactFamily::SolverDerivedEdge,
    SOLVER_STORE_FAMILY
);
impl_fact_store!(
    SemanticStore,
    FactFamily::MirBody,
    SEMANTIC_MIR_STORE_FAMILY
);

/// Extension activations and accepted/rejected facts for `polint.extensions`.
#[derive(Debug, Clone, Default)]
pub struct ExtensionFactStore {
    pub activations: Vec<ExtensionActivationRow>,
    pub accepted: Vec<AcceptedExtensionFact>,
    pub rejected: Vec<RejectedExtensionFact>,
}

impl_fact_store!(
    ExtensionFactStore,
    FactFamily::ExtensionFact,
    EXTENSION_STORE_FAMILY
);

/// Accepted/rejected adaptation model facts for `polint.adaptation`.
#[derive(Debug, Clone, Default)]
pub struct AdaptationFactStore {
    pub accepted: Vec<AcceptedModelFact>,
    pub rejected: Vec<RejectedModelFact>,
}

impl_fact_store!(
    AdaptationFactStore,
    FactFamily::AdaptationModel,
    ADAPTATION_STORE_FAMILY
);

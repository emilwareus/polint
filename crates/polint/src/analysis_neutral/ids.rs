pub use crate::ir::{
    CallSiteId, MirBodyId, MirOpId, MirPredicateId, MirStatementId, MirTerminatorId, MirValueId,
    PlaceId, TypeSetId, UnsupportedId,
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CallTargetId(pub u64);

/// Scope handle shared with the symbol-graph semantic index (dense run-local id).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ScopeId(pub u64);

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TsBindingId(pub u64);
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TsDirectBindingId(pub u64);
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TsInventoryCallsiteId(pub u64);
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TsInventoryFunctionId(pub u64);
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TsScopeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AbstractStateId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DomainObservationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DomainEventId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SummaryId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SummaryEventId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntrypointId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TrustBoundaryId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DispatchEdgeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct UnresolvedFrameworkId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TypeFactId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NarrowedTypeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ValueFactId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AbstractValueId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AllocationTokenId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AccessPathId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PointsToConstraintId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PointsToSetId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PtVarId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ObjectTokenId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AliasAnswerId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RefinedCallEdgeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DataFlowNodeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DataFlowEdgeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DataFlowModelId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DataFlowBudgetId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DataFlowPathId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EvidenceNodeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EvidenceEdgeId(pub u64);

pub use crate::internal_core::EvidenceBundleId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EvidencePathId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EvidenceSliceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EvidenceOmittedRegionId(pub u64);

// `Default` is required because `ReachabilityRootFact.id` carries `#[serde(skip)]`
// (D-19: dense IDs must never enter a serialized stable-payload / digest part); serde
// reconstructs the skipped field via `Default`, yielding `ReachabilityRootId(0)`.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct ReachabilityRootId(pub u64);

// `Default` is required because the dense `id` fields on `SemanticNodeFact` /
// `SemanticEdgeFact` carry `#[serde(skip)]` (dense IDs must never enter a
// serialized stable-payload / digest part, D-06); serde reconstructs the skipped
// field via `Default`, yielding `SemanticNodeId(0)` / `SemanticEdgeId(0)`. The
// dense IDs are assigned only after the stable-key sort (D-05).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct SemanticNodeId(pub u64);

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct SemanticEdgeId(pub u64);

// `Default` is required because the dense `id` field on `ConstraintFact` carries
// `#[serde(skip)]` (dense IDs must never enter a serialized stable-payload / digest
// part, D-06); serde reconstructs the skipped field via `Default`, yielding
// `SemanticConstraintId(0)`. The dense IDs are assigned only after the stable-key
// sort (D-05), mirroring `SemanticNodeId`/`SemanticEdgeId`.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct SemanticConstraintId(pub u64);

// `Default` is required because the dense `id` field on the solver's derived-edge
// fact (`crate::analysis_neutral::solver::facts::DerivedEdgeFact`) carries `#[serde(skip)]`
// (dense IDs must never enter a serialized stable-payload / digest part, D-06);
// serde reconstructs the skipped field via `Default`, yielding `DerivedEdgeId(0)`.
// The dense IDs are assigned only after the stable-key sort (D-08),
// mirroring `SemanticConstraintId`/`SemanticNodeId`/`SemanticEdgeId`.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct DerivedEdgeId(pub u64);

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use serde::de::DeserializeOwned;
    use std::collections::{BTreeSet, HashSet};
    use std::fmt::Debug;
    use std::hash::Hash;

    fn assert_small_id_contract<T>()
    where
        T: Debug
            + Clone
            + Copy
            + PartialEq
            + Eq
            + PartialOrd
            + Ord
            + Hash
            + Serialize
            + DeserializeOwned,
    {
    }

    #[test]
    fn semantic_id_newtypes_are_copy_ordered_hashable_and_serializable() {
        assert_small_id_contract::<MirBodyId>();
        assert_small_id_contract::<MirOpId>();
        assert_small_id_contract::<MirStatementId>();
        assert_small_id_contract::<MirTerminatorId>();
        assert_small_id_contract::<MirValueId>();
        assert_small_id_contract::<MirPredicateId>();
        assert_small_id_contract::<PlaceId>();
        assert_small_id_contract::<UnsupportedId>();
        assert_small_id_contract::<CallSiteId>();
        assert_small_id_contract::<CallTargetId>();
        assert_small_id_contract::<TsInventoryFunctionId>();
        assert_small_id_contract::<TsInventoryCallsiteId>();
        assert_small_id_contract::<TsScopeId>();
        assert_small_id_contract::<TsBindingId>();
        assert_small_id_contract::<TsDirectBindingId>();
        assert_small_id_contract::<AbstractStateId>();
        assert_small_id_contract::<DomainObservationId>();
        assert_small_id_contract::<DomainEventId>();
        assert_small_id_contract::<SummaryId>();
        assert_small_id_contract::<SummaryEventId>();
        assert_small_id_contract::<EntrypointId>();
        assert_small_id_contract::<TrustBoundaryId>();
        assert_small_id_contract::<DispatchEdgeId>();
        assert_small_id_contract::<UnresolvedFrameworkId>();
        assert_small_id_contract::<TypeFactId>();
        assert_small_id_contract::<TypeSetId>();
        assert_small_id_contract::<NarrowedTypeId>();
        assert_small_id_contract::<ValueFactId>();
        assert_small_id_contract::<AbstractValueId>();
        assert_small_id_contract::<AllocationTokenId>();
        assert_small_id_contract::<AccessPathId>();
        assert_small_id_contract::<PointsToConstraintId>();
        assert_small_id_contract::<PointsToSetId>();
        assert_small_id_contract::<PtVarId>();
        assert_small_id_contract::<ObjectTokenId>();
        assert_small_id_contract::<AliasAnswerId>();
        assert_small_id_contract::<RefinedCallEdgeId>();
        assert_small_id_contract::<DataFlowNodeId>();
        assert_small_id_contract::<DataFlowEdgeId>();
        assert_small_id_contract::<DataFlowModelId>();
        assert_small_id_contract::<DataFlowBudgetId>();
        assert_small_id_contract::<DataFlowPathId>();
        assert_small_id_contract::<EvidenceNodeId>();
        assert_small_id_contract::<EvidenceEdgeId>();
        assert_small_id_contract::<EvidenceBundleId>();
        assert_small_id_contract::<EvidencePathId>();
        assert_small_id_contract::<EvidenceSliceId>();
        assert_small_id_contract::<EvidenceOmittedRegionId>();
        assert_small_id_contract::<ReachabilityRootId>();
        assert_small_id_contract::<SemanticNodeId>();
        assert_small_id_contract::<SemanticEdgeId>();
        assert_small_id_contract::<SemanticConstraintId>();
        assert_small_id_contract::<DerivedEdgeId>();
    }

    #[test]
    fn dense_ids_sort_and_hash_as_run_local_handles() {
        let mut ordered = BTreeSet::new();
        ordered.insert(PlaceId(2));
        ordered.insert(PlaceId(1));

        let mut hashed = HashSet::new();
        hashed.insert(PlaceId(1));

        assert_eq!(
            ordered.into_iter().collect::<Vec<_>>(),
            vec![PlaceId(1), PlaceId(2)]
        );
        assert!(hashed.contains(&PlaceId(1)));
    }
}

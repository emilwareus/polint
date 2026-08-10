use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct MirBodyId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct MirOpId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct MirStatementId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct MirTerminatorId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct MirValueId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct MirPredicateId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct PlaceId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct UnsupportedId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct CallSiteId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct CallTargetId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct TsInventoryFunctionId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct TsInventoryCallsiteId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct TsScopeId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct TsBindingId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct TsDirectBindingId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct AbstractStateId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct DomainObservationId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct DomainEventId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct SummaryId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct SummaryEventId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct EntrypointId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct TrustBoundaryId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct DispatchEdgeId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct UnresolvedFrameworkId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct TypeFactId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct TypeSetId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct NarrowedTypeId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct ValueFactId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct AbstractValueId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct AllocationTokenId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct AccessPathId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct PointsToConstraintId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct PointsToSetId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct PtVarId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct ObjectTokenId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct AliasAnswerId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct RefinedCallEdgeId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct DataFlowNodeId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct DataFlowEdgeId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct DataFlowModelId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct DataFlowBudgetId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct DataFlowPathId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct EvidenceNodeId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct EvidenceEdgeId(pub(crate) u64);

pub(crate) use polint_core::EvidenceBundleId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct EvidencePathId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct EvidenceSliceId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct EvidenceOmittedRegionId(pub(crate) u64);

// `Default` is required because `ReachabilityRootFact.id` carries `#[serde(skip)]`
// (D-19: dense IDs must never enter a serialized stable-payload / digest part); serde
// reconstructs the skipped field via `Default`, yielding `ReachabilityRootId(0)`.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub(crate) struct ReachabilityRootId(pub(crate) u64);

// `Default` is required because the dense `id` fields on `SemanticNodeFact` /
// `SemanticEdgeFact` carry `#[serde(skip)]` (dense IDs must never enter a
// serialized stable-payload / digest part, D-06); serde reconstructs the skipped
// field via `Default`, yielding `SemanticNodeId(0)` / `SemanticEdgeId(0)`. The
// dense IDs are assigned only after the stable-key sort (D-05).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub(crate) struct SemanticNodeId(pub(crate) u64);

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub(crate) struct SemanticEdgeId(pub(crate) u64);

// `Default` is required because the dense `id` field on `ConstraintFact` carries
// `#[serde(skip)]` (dense IDs must never enter a serialized stable-payload / digest
// part, D-06); serde reconstructs the skipped field via `Default`, yielding
// `SemanticConstraintId(0)`. The dense IDs are assigned only after the stable-key
// sort (D-05), mirroring `SemanticNodeId`/`SemanticEdgeId`.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub(crate) struct SemanticConstraintId(pub(crate) u64);

// `Default` is required because the dense `id` field on the solver's derived-edge
// fact (`crate::analysis::solver::facts::DerivedEdgeFact`) carries `#[serde(skip)]`
// (dense IDs must never enter a serialized stable-payload / digest part, D-06);
// serde reconstructs the skipped field via `Default`, yielding `DerivedEdgeId(0)`.
// The dense IDs are assigned only after the stable-key sort (D-08),
// mirroring `SemanticConstraintId`/`SemanticNodeId`/`SemanticEdgeId`.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub(crate) struct DerivedEdgeId(pub(crate) u64);

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

use serde::{Deserialize, Serialize};

use crate::analysis::ids::{AccessPathId, AliasAnswerId, PlaceId};
use crate::core::StableKeyId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AliasAnswerFact {
    pub(crate) id: AliasAnswerId,
    pub(crate) left: AliasOperand,
    pub(crate) right: AliasOperand,
    pub(crate) status: AliasStatus,
    pub(crate) reason: AliasReason,
    pub(crate) evidence: Vec<String>,
    pub(crate) precision: AliasPrecision,
    pub(crate) stable_key: StableKeyId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum AliasOperand {
    Place(PlaceId),
    AccessPath(AccessPathId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum AliasStatus {
    NoAlias,
    MayAlias,
    MustAlias,
    PartialAlias,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum AliasReason {
    SameStablePlace,
    DisjointLocals,
    DisjointAllocations,
    DisjointPointsToSets,
    OverlappingPointsToSets,
    SingletonEqualObject,
    CommonBaseDifferentProjection,
    UnsupportedDynamicConstruct,
    SetupMissing,
    BudgetExceeded,
    ExtensionProvided,
    MissingPointsTo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum AliasPrecision {
    ExactLocal,
    FlowInsensitive,
    SetupAware,
    Conservative,
    Heuristic,
    Unknown,
    Unsupported,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_status_preserves_required_vocabulary() {
        let statuses = [
            AliasStatus::NoAlias,
            AliasStatus::MayAlias,
            AliasStatus::MustAlias,
            AliasStatus::PartialAlias,
            AliasStatus::Unknown,
        ];

        assert_eq!(statuses.len(), 5);
    }
}

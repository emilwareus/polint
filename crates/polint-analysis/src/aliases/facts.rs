use serde::{Deserialize, Serialize};

use crate::ids::{AccessPathId, AliasAnswerId, PlaceId};
use polint_core::StableKeyId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AliasAnswerFact {
    pub id: AliasAnswerId,
    pub left: AliasOperand,
    pub right: AliasOperand,
    pub status: AliasStatus,
    pub reason: AliasReason,
    pub evidence: Vec<String>,
    pub precision: AliasPrecision,
    pub stable_key: StableKeyId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AliasOperand {
    Place(PlaceId),
    AccessPath(AccessPathId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AliasStatus {
    NoAlias,
    MayAlias,
    MustAlias,
    PartialAlias,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AliasReason {
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
pub enum AliasPrecision {
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

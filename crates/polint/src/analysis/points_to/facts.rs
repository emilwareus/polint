use serde::{Deserialize, Serialize};

use crate::analysis::ids::{
    ObjectTokenId, PointsToConstraintId, PointsToSetId, PtVarId, ValueFactId,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PointsToConstraintFact {
    pub(crate) id: PointsToConstraintId,
    pub(crate) kind: PointsToConstraintKind,
    pub(crate) status: PointsToStatus,
    pub(crate) precision: PointsToPrecision,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PointsToSetFact {
    pub(crate) id: PointsToSetId,
    pub(crate) variable: PtVarId,
    pub(crate) objects: Vec<ObjectTokenId>,
    pub(crate) status: PointsToStatus,
    pub(crate) precision: PointsToPrecision,
    pub(crate) budget: PointsToBudgetStatus,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum PointsToConstraintKind {
    AddressOf {
        dst: PtVarId,
        object: ObjectTokenId,
    },
    Copy {
        dst: PtVarId,
        src: PtVarId,
    },
    Load {
        dst: PtVarId,
        pointer: PtVarId,
    },
    Store {
        pointer: PtVarId,
        src: PtVarId,
    },
    FieldLoad {
        dst: PtVarId,
        base: PtVarId,
        field: String,
    },
    FieldStore {
        base: PtVarId,
        field: String,
        src: PtVarId,
    },
    ElementLoad {
        dst: PtVarId,
        base: PtVarId,
        index: String,
    },
    ElementStore {
        base: PtVarId,
        index: String,
        src: PtVarId,
    },
    CallReturn {
        dst: PtVarId,
        value: ValueFactId,
    },
    SummaryFlow {
        dst: PtVarId,
        src: PtVarId,
        summary_key: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum PointsToStatus {
    Present,
    Unknown,
    Unsupported,
    SetupMissing,
    BudgetExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum PointsToPrecision {
    FlowInsensitive,
    LocalFlowSensitive,
    SummaryProjected,
    Heuristic,
    Unknown,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum PointsToBudgetStatus {
    WithinBudget,
    BudgetExceeded,
    NotRun,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_to_constraints_include_field_and_call_flow() {
        let constraints = [
            PointsToConstraintKind::FieldLoad {
                dst: PtVarId(1),
                base: PtVarId(2),
                field: "name".to_string(),
            },
            PointsToConstraintKind::CallReturn {
                dst: PtVarId(1),
                value: ValueFactId(3),
            },
        ];

        assert_eq!(constraints.len(), 2);
    }
}

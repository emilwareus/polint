use serde::{Deserialize, Serialize};

use crate::analysis::ids::{AccessPathId, CallSiteId, MirBodyId, PlaceId};
use crate::core::{FileId, FunctionId, Language};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AccessPathFact {
    pub(crate) id: AccessPathId,
    pub(crate) base: PlaceId,
    pub(crate) projections: Vec<AccessPathProjection>,
    pub(crate) depth: u32,
    pub(crate) language: Language,
    pub(crate) file: Option<FileId>,
    pub(crate) function: Option<FunctionId>,
    pub(crate) body: Option<MirBodyId>,
    pub(crate) status: AccessPathStatus,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum AccessPathProjection {
    Field(String),
    Property(String),
    IndexKnown(String),
    IndexUnknown { evidence: String },
    Deref,
    AwaitResult,
    CallReturn(CallSiteId),
    Unknown { evidence: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum AccessPathStatus {
    Resolved,
    Partial,
    Unknown,
    Unsupported,
    BudgetExceeded,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_path_projection_distinguishes_known_and_unknown_indexes() {
        assert_ne!(
            AccessPathProjection::IndexKnown("name".to_string()),
            AccessPathProjection::IndexUnknown {
                evidence: "expr".to_string()
            }
        );
    }
}

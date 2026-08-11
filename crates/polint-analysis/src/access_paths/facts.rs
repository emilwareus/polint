use serde::{Deserialize, Serialize};

use crate::ids::{AccessPathId, CallSiteId, MirBodyId, PlaceId};
use polint_core::{FileId, FunctionId, Language, StableKeyId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessPathFact {
    pub id: AccessPathId,
    pub base: PlaceId,
    pub projections: Vec<AccessPathProjection>,
    pub depth: u32,
    pub language: Language,
    pub file: Option<FileId>,
    pub function: Option<FunctionId>,
    pub body: Option<MirBodyId>,
    pub status: AccessPathStatus,
    pub stable_key: StableKeyId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AccessPathProjection {
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
pub enum AccessPathStatus {
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

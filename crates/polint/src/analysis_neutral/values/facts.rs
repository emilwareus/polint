use serde::{Deserialize, Serialize};

use crate::analysis_neutral::ids::{
    AbstractValueId, AllocationTokenId, MirBodyId, MirOpId, PlaceId, ValueFactId,
};
use crate::internal_core::{FileId, FunctionId, Language, Span, StableKeyId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueFact {
    pub id: ValueFactId,
    pub subject: ValueSubject,
    pub value: AbstractValueId,
    pub kind: ValueKind,
    pub language: Language,
    pub file: Option<FileId>,
    pub function: Option<FunctionId>,
    pub body: Option<MirBodyId>,
    pub precision: ValuePrecision,
    pub status: ValueStatus,
    pub provenance: ValueProvenance,
    pub stable_key: StableKeyId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllocationTokenFact {
    pub id: AllocationTokenId,
    pub kind: AllocationKind,
    pub language: Language,
    pub file: Option<FileId>,
    pub function: Option<FunctionId>,
    pub body: Option<MirBodyId>,
    pub source_place: Option<PlaceId>,
    pub source_operation: Option<MirOpId>,
    pub span: Option<Span>,
    pub provenance: ValueProvenance,
    pub stable_key: StableKeyId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ValueSubject {
    Place(PlaceId),
    Operation(MirOpId),
    Allocation(AllocationTokenId),
    Synthetic(String),
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ValueKind {
    Null,
    Undefined,
    Nil,
    Bool(String),
    Number(String),
    String(String),
    Literal(String),
    FunctionObject,
    ClassObject,
    ModuleObject,
    PlaceRef(PlaceId),
    Object(AllocationTokenId),
    Array(AllocationTokenId),
    CompositeLiteral(AllocationTokenId),
    CallReturn(PlaceId),
    Unknown { evidence: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AllocationKind {
    ObjectLiteral,
    ArrayLiteral,
    CompositeLiteral,
    FunctionObject,
    ClassObject,
    ModuleNamespace,
    Closure,
    SyntheticFrameworkObject,
    ExtensionModeledObject,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ValuePrecision {
    ExactLocal,
    SetupAware,
    Conservative,
    Heuristic,
    Unknown,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ValueStatus {
    Present,
    Unknown,
    Unsupported,
    SetupMissing,
    BudgetExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ValueProvenance {
    Native,
    Extension { extension_id: String },
    Generated,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_kind_distinguishes_known_and_unknown_values() {
        assert_ne!(
            ValueKind::String("key".to_string()),
            ValueKind::Unknown {
                evidence: "dynamic property".to_string()
            }
        );
    }
}

use serde::{Deserialize, Serialize};

use crate::analysis::ids::{
    AbstractValueId, AllocationTokenId, MirBodyId, MirOpId, PlaceId, ValueFactId,
};
use crate::core::{FileId, FunctionId, Language, Span, StableKeyId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ValueFact {
    pub(crate) id: ValueFactId,
    pub(crate) subject: ValueSubject,
    pub(crate) value: AbstractValueId,
    pub(crate) kind: ValueKind,
    pub(crate) language: Language,
    pub(crate) file: Option<FileId>,
    pub(crate) function: Option<FunctionId>,
    pub(crate) body: Option<MirBodyId>,
    pub(crate) precision: ValuePrecision,
    pub(crate) status: ValueStatus,
    pub(crate) provenance: ValueProvenance,
    pub(crate) stable_key: StableKeyId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AllocationTokenFact {
    pub(crate) id: AllocationTokenId,
    pub(crate) kind: AllocationKind,
    pub(crate) language: Language,
    pub(crate) file: Option<FileId>,
    pub(crate) function: Option<FunctionId>,
    pub(crate) body: Option<MirBodyId>,
    pub(crate) source_place: Option<PlaceId>,
    pub(crate) source_operation: Option<MirOpId>,
    pub(crate) span: Option<Span>,
    pub(crate) provenance: ValueProvenance,
    pub(crate) stable_key: StableKeyId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum ValueSubject {
    Place(PlaceId),
    Operation(MirOpId),
    Allocation(AllocationTokenId),
    Synthetic(String),
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum ValueKind {
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
pub(crate) enum AllocationKind {
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
pub(crate) enum ValuePrecision {
    ExactLocal,
    SetupAware,
    Conservative,
    Heuristic,
    Unknown,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum ValueStatus {
    Present,
    Unknown,
    Unsupported,
    SetupMissing,
    BudgetExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum ValueProvenance {
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

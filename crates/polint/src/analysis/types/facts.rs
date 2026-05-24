use serde::{Deserialize, Serialize};

use crate::analysis::cfg::ids::BasicBlockId;
use crate::analysis::ids::{MirBodyId, MirOpId, NarrowedTypeId, PlaceId, TypeFactId, TypeSetId};
use crate::core::{FileId, FunctionId, Language, SymbolId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TypeFact {
    pub(crate) id: TypeFactId,
    pub(crate) subject: TypeSubject,
    pub(crate) type_set: TypeSetId,
    pub(crate) shape: TypeShape,
    pub(crate) phase: TypePhase,
    pub(crate) language: Language,
    pub(crate) file: Option<FileId>,
    pub(crate) function: Option<FunctionId>,
    pub(crate) body: Option<MirBodyId>,
    pub(crate) place: Option<PlaceId>,
    pub(crate) cfg_block: Option<BasicBlockId>,
    pub(crate) operation: Option<MirOpId>,
    pub(crate) precision: TypePrecision,
    pub(crate) confidence: TypeConfidence,
    pub(crate) status: TypeStatus,
    pub(crate) provenance: TypeProvenance,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct NarrowedTypeFact {
    pub(crate) id: NarrowedTypeId,
    pub(crate) place: PlaceId,
    pub(crate) type_set: TypeSetId,
    pub(crate) cfg_block: Option<BasicBlockId>,
    pub(crate) operation: Option<MirOpId>,
    pub(crate) predicate: Option<PlaceId>,
    pub(crate) evidence: String,
    pub(crate) language: Language,
    pub(crate) file: Option<FileId>,
    pub(crate) function: Option<FunctionId>,
    pub(crate) body: Option<MirBodyId>,
    pub(crate) precision: TypePrecision,
    pub(crate) status: TypeStatus,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum TypeSubject {
    Symbol(SymbolId),
    Place(PlaceId),
    Operation(MirOpId),
    Function(FunctionId),
    Synthetic(String),
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum TypeShape {
    Primitive(String),
    Literal(String),
    Nullish(String),
    Callable { signature: String },
    Class { name: Option<String> },
    Object { shape_id: Option<String> },
    Module { module_key: String },
    Nominal { type_id: String },
    Structural { shape_id: String },
    Union(Vec<TypeSetId>),
    Intersection(Vec<TypeSetId>),
    GenericPlaceholder(String),
    Any,
    Unknown { reason: String },
    Unsupported { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum TypePhase {
    Declared,
    Inferred,
    Resolved,
    FlowNarrowed,
    ExtensionProvided,
    Unknown,
    Unsupported,
    SetupMissing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum TypePrecision {
    ExactLocal,
    SetupAware,
    Conservative,
    Heuristic,
    Unknown,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum TypeConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum TypeStatus {
    Present,
    Unknown,
    Unsupported,
    SetupMissing,
    BudgetExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum TypeProvenance {
    Native,
    OfficialTool { tool: String },
    Extension { extension_id: String },
    Generated,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_phase_preserves_all_required_evidence_phases() {
        let phases = [
            TypePhase::Declared,
            TypePhase::Inferred,
            TypePhase::Resolved,
            TypePhase::FlowNarrowed,
            TypePhase::ExtensionProvided,
            TypePhase::Unknown,
            TypePhase::Unsupported,
            TypePhase::SetupMissing,
        ];

        assert_eq!(phases.len(), 8);
    }

    #[test]
    fn type_shape_preserves_any_unknown_and_unsupported_distinctions() {
        assert_ne!(
            TypeShape::Any,
            TypeShape::Unknown {
                reason: "dynamic".to_string()
            }
        );
        assert_ne!(
            TypeShape::Unknown {
                reason: "dynamic".to_string()
            },
            TypeShape::Unsupported {
                reason: "eval".to_string()
            }
        );
    }
}

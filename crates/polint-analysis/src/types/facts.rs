use serde::{Deserialize, Serialize};

use crate::cfg::ids::BasicBlockId;
use crate::ids::{MirBodyId, MirOpId, NarrowedTypeId, PlaceId, TypeFactId, TypeSetId};
use polint_core::{FileId, FunctionId, Language, StableKeyId, SymbolId};

pub use polint_ir::TypeShape;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeFact {
    pub id: TypeFactId,
    pub subject: TypeSubject,
    pub type_set: TypeSetId,
    pub shape: TypeShape,
    pub phase: TypePhase,
    pub language: Language,
    pub file: Option<FileId>,
    pub function: Option<FunctionId>,
    pub body: Option<MirBodyId>,
    pub place: Option<PlaceId>,
    pub cfg_block: Option<BasicBlockId>,
    pub operation: Option<MirOpId>,
    pub precision: TypePrecision,
    pub confidence: TypeConfidence,
    pub status: TypeStatus,
    pub provenance: TypeProvenance,
    pub stable_key: StableKeyId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NarrowedTypeFact {
    pub id: NarrowedTypeId,
    pub place: PlaceId,
    pub type_set: TypeSetId,
    pub cfg_block: Option<BasicBlockId>,
    pub operation: Option<MirOpId>,
    pub predicate: Option<PlaceId>,
    pub evidence: String,
    pub language: Language,
    pub file: Option<FileId>,
    pub function: Option<FunctionId>,
    pub body: Option<MirBodyId>,
    pub precision: TypePrecision,
    pub status: TypeStatus,
    pub stable_key: StableKeyId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TypeSubject {
    Symbol(SymbolId),
    Place(PlaceId),
    Operation(MirOpId),
    Function(FunctionId),
    Synthetic(String),
    Unknown(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TypePhase {
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
pub enum TypePrecision {
    ExactLocal,
    SetupAware,
    Conservative,
    Heuristic,
    Unknown,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TypeConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TypeStatus {
    Present,
    Unknown,
    Unsupported,
    SetupMissing,
    BudgetExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TypeProvenance {
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

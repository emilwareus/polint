use serde::{Deserialize, Serialize};

use crate::internal_core::{FileId, FunctionId, Language, StableKeyId, SymbolId};
use crate::ir::ids::{CallSiteId, MirBodyId, PlaceId};
use crate::ir::types::TypeShape;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceFact {
    pub id: PlaceId,
    pub language: Language,
    pub file: Option<FileId>,
    pub function: Option<FunctionId>,
    pub root: PlaceRoot,
    pub projections: Vec<PlaceProjection>,
    pub stable_key: StableKeyId,
    pub status: PlaceStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceTypeFact {
    pub place: PlaceId,
    pub ty: TypeShape,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PlaceRoot {
    Local {
        function: FunctionId,
        name: String,
    },
    Parameter {
        function: FunctionId,
        index: u32,
        name: Option<String>,
    },
    Global {
        symbol: Option<SymbolId>,
        name: String,
    },
    Temporary {
        body: MirBodyId,
        ordinal: u32,
    },
    CallReturn {
        call: CallSiteId,
    },
    Unknown {
        evidence: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PlaceProjection {
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
pub enum PlaceStatus {
    Resolved,
    Partial,
    Unknown,
    Unsupported,
}

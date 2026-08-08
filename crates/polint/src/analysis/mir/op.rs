use serde::{Deserialize, Serialize};

use crate::analysis::ids::{
    CallSiteId, MirBodyId, MirOpId, MirPredicateId, MirValueId, PlaceId, UnsupportedId,
};
use crate::analysis::mir::body::MirStatus;
use crate::core::{FileId, Language, Span};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MirOperation {
    pub(crate) id: MirOpId,
    pub(crate) body: MirBodyId,
    pub(crate) ordinal: u32,
    pub(crate) span: Span,
    pub(crate) kind: MirOperationKind,
    pub(crate) stable_key: String,
    pub(crate) status: MirStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum MirOperationKind {
    StorageLive {
        place: PlaceId,
    },
    Bind {
        place: PlaceId,
        value: MirValue,
    },
    Assign {
        place: PlaceId,
        value: MirValue,
        mode: AssignMode,
    },
    Read {
        place: PlaceId,
    },
    Write {
        place: PlaceId,
        value: MirValue,
    },
    Branch {
        predicate: MirPredicateId,
        predicate_place: Option<PlaceId>,
    },
    Call {
        site: CallSiteId,
        callee: MirValue,
        arguments: Vec<PlaceId>,
        return_place: PlaceId,
    },
    Return {
        value: Option<MirValue>,
    },
    Unsupported {
        unsupported: UnsupportedId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum AssignMode {
    DeclarationBinding,
    Overwrite,
    PartialWrite,
    Simultaneous,
    ProjectionMutation,
    UnknownWrite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum MirValue {
    Literal {
        value: String,
    },
    Place(PlaceId),
    Temporary(MirValueId),
    CallReturn(CallSiteId),
    BinOp {
        op: String,
        lhs: Box<MirValue>,
        rhs: Box<MirValue>,
    },
    Aggregate {
        kind: MirAggregateKind,
        fields: Vec<MirAggregateField>,
    },
    Closure {
        body: MirBodyId,
        captures: Vec<PlaceId>,
    },
    Unknown {
        evidence: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum MirAggregateKind {
    Array,
    Object,
    Composite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MirAggregateField {
    pub(crate) name: Option<String>,
    pub(crate) value: MirValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct UnsupportedSemanticFact {
    pub(crate) id: UnsupportedId,
    pub(crate) body: Option<MirBodyId>,
    pub(crate) operation: Option<MirOpId>,
    pub(crate) language: Language,
    pub(crate) file: FileId,
    pub(crate) span: Span,
    pub(crate) construct: String,
    pub(crate) source_evidence: String,
    pub(crate) affected_places: Vec<PlaceId>,
    pub(crate) affected_domains: Vec<UnsupportedDomain>,
    pub(crate) conservative_action: ConservativeAction,
    pub(crate) precision: UnsupportedPrecision,
    pub(crate) status: MirStatus,
    pub(crate) stable_key: String,
}

impl UnsupportedSemanticFact {
    pub(crate) fn is_complete(&self) -> bool {
        !self.construct.trim().is_empty()
            && !self.source_evidence.trim().is_empty()
            && !self.affected_domains.is_empty()
            && !matches!(self.status, MirStatus::Resolved)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum UnsupportedDomain {
    Mir,
    Cfg,
    Calls,
    Domains,
    Summaries,
    DataFlow,
    Aliases,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum ConservativeAction {
    SkipOperation,
    HavocAffectedPlaces,
    PreserveWithUnknownValue,
    StopLowering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum UnsupportedPrecision {
    Partial,
    Unknown,
    Unsupported,
}

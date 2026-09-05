use serde::{Deserialize, Serialize};

use crate::internal_core::{FileId, Language, Span, StableKeyId};
use crate::ir::body::MirStatus;
use crate::ir::ids::{
    CallSiteId, MirBodyId, MirOpId, MirPredicateId, MirValueId, PlaceId, UnsupportedId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirOperation {
    pub id: MirOpId,
    pub body: MirBodyId,
    pub ordinal: u32,
    pub span: Span,
    pub kind: MirOperationKind,
    pub stable_key: StableKeyId,
    pub status: MirStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MirOperationKind {
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
        nil_test: Option<BranchNilTest>,
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

/// A branch whose predicate compares a place against the language's nil value.
///
/// `nil_on_true` names the outgoing edge on which the place is nil. Whether the
/// opposite edge proves non-nil depends on how exhaustively the comparison
/// covers the nil values of the language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum BranchNilTest {
    /// The comparison covers every nil value, so the opposite edge proves
    /// non-nil: Go `== nil` / `!= nil`, and TypeScript `== null` / `!= null`,
    /// which match `undefined` as well as `null`.
    Exhaustive { nil_on_true: bool },
    /// The comparison covers only part of the nil space, so only the nil edge
    /// carries a conclusion: TypeScript `=== null` leaves `undefined` possible
    /// on the opposite edge.
    NilEdgeOnly { nil_on_true: bool },
}

impl BranchNilTest {
    /// The edge on which the compared place is nil.
    pub fn nil_on_true(self) -> bool {
        match self {
            Self::Exhaustive { nil_on_true } | Self::NilEdgeOnly { nil_on_true } => nil_on_true,
        }
    }

    /// Whether the edge that is not the nil edge proves the place is non-nil.
    pub fn proves_non_nil(self) -> bool {
        matches!(self, Self::Exhaustive { .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AssignMode {
    DeclarationBinding,
    Overwrite,
    PartialWrite,
    Simultaneous,
    ProjectionMutation,
    UnknownWrite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MirValue {
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
pub enum MirAggregateKind {
    Array,
    Object,
    Composite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MirAggregateField {
    pub name: Option<String>,
    pub value: MirValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedSemanticFact {
    pub id: UnsupportedId,
    pub body: Option<MirBodyId>,
    pub operation: Option<MirOpId>,
    pub language: Language,
    pub file: FileId,
    pub span: Span,
    pub construct: String,
    pub source_evidence: String,
    pub affected_places: Vec<PlaceId>,
    pub affected_domains: Vec<UnsupportedDomain>,
    pub conservative_action: ConservativeAction,
    pub precision: UnsupportedPrecision,
    pub status: MirStatus,
    pub stable_key: StableKeyId,
}

impl UnsupportedSemanticFact {
    pub fn is_complete(&self) -> bool {
        !self.construct.trim().is_empty()
            && !self.source_evidence.trim().is_empty()
            && !self.affected_domains.is_empty()
            && !matches!(self.status, MirStatus::Resolved)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum UnsupportedDomain {
    Mir,
    Cfg,
    Calls,
    Domains,
    Summaries,
    DataFlow,
    Aliases,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ConservativeAction {
    SkipOperation,
    HavocAffectedPlaces,
    PreserveWithUnknownValue,
    StopLowering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum UnsupportedPrecision {
    Partial,
    Unknown,
    Unsupported,
}

use serde::{Deserialize, Serialize};

use crate::analysis::ids::{CallSiteId, CallTargetId, MirBodyId, MirOpId, PlaceId};
use crate::core::{FileId, FunctionId, Language, ReferenceId, Span, SymbolId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CallSiteFact {
    pub(crate) id: CallSiteId,
    pub(crate) language: Language,
    pub(crate) file: FileId,
    pub(crate) caller: FunctionId,
    pub(crate) owner_symbol: Option<SymbolId>,
    pub(crate) body: MirBodyId,
    pub(crate) operation: MirOpId,
    pub(crate) span: Span,
    pub(crate) kind: CallSyntaxKind,
    pub(crate) callee: CallCallee,
    pub(crate) receiver: Option<PlaceId>,
    pub(crate) arguments: Vec<PlaceId>,
    pub(crate) result: Option<PlaceId>,
    pub(crate) status: CallTargetStatus,
    pub(crate) precision: CallPrecision,
    /// True when this call site is lexically inside a `throw` argument
    /// (`throw new E(... f() ...)`). Such calls sit on error paths that the
    /// demand-driven oracle does not exercise, so resolvers skip them to avoid
    /// false edges (e.g. express's `gettype(fn)` in a middleware-type-check throw).
    #[serde(default)]
    pub(crate) in_throw: bool,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CallTargetFact {
    pub(crate) id: CallTargetId,
    pub(crate) site: CallSiteId,
    pub(crate) caller: FunctionId,
    pub(crate) target_function: Option<FunctionId>,
    pub(crate) target_symbol: Option<SymbolId>,
    pub(crate) edge_kind: CallEdgeKind,
    pub(crate) algorithm: CallAlgorithm,
    pub(crate) status: CallTargetStatus,
    pub(crate) reason: Option<UnresolvedCallReason>,
    pub(crate) provenance: CallProvenance,
    pub(crate) precision: CallPrecision,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct UnresolvedCallFact {
    pub(crate) site: CallSiteId,
    pub(crate) caller: FunctionId,
    pub(crate) status: CallTargetStatus,
    pub(crate) reason: UnresolvedCallReason,
    pub(crate) algorithm: CallAlgorithm,
    pub(crate) provenance: CallProvenance,
    pub(crate) precision: CallPrecision,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum CallSyntaxKind {
    Function,
    Method,
    Constructor,
    StaticMember,
    Member,
    Index,
    Super,
    Import,
    New,
    TaggedTemplate,
    GoRoutine,
    Deferred,
    DynamicImport,
    Require,
    FunctionValue,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum CallCallee {
    Identifier {
        reference: Option<ReferenceId>,
        name: String,
    },
    Member {
        base: PlaceId,
        property: String,
    },
    Index {
        base: PlaceId,
        index: Option<PlaceId>,
    },
    Super,
    Import,
    FunctionValue {
        place: PlaceId,
    },
    Constructor {
        reference: Option<ReferenceId>,
        name: Option<String>,
    },
    Unknown {
        reason: UnresolvedCallReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum CallEdgeKind {
    Direct,
    Constructor,
    StaticMember,
    MethodDirect,
    Method,
    FunctionValue,
    Synthetic,
    Spawn,
    Deferred,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum CallAlgorithm {
    SyntaxOnly,
    DirectReference,
    ImportBinding,
    ConstructorBinding,
    StaticMember,
    DirectMember,
    GoStatic,
    GoCha,
    GoRta,
    GoVta,
    TypeHierarchy,
    PointsTo,
    SummaryAssisted,
    FrameworkModel,
    RepoModel,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum CallTargetStatus {
    Resolved,
    Ambiguous,
    Unresolved,
    Unsupported,
    SetupMissing,
    BudgetExceeded,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum UnresolvedCallReason {
    FunctionValue,
    DynamicProperty,
    InterfaceDispatch,
    Eval,
    CallApplyBind,
    FrameworkDispatch,
    Reflection,
    GoroutineBoundary,
    DynamicImport,
    ProxyOrAccessor,
    MissingSemanticReference,
    MissingImportResolution,
    SetupMissing,
    UnsupportedSyntax,
    BudgetExceeded,
    UnknownCallee,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum CallPrecision {
    Exact,
    SetupAware,
    Conservative,
    Heuristic,
    Ambiguous,
    Unknown,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum CallProvenance {
    NativeDirect,
    Native,
    SemanticReference,
    ImportBinding,
    MirShape,
    Topology,
    Extension,
    Model,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{FileId, FunctionId, Language, SymbolId};
    use serde::Serialize;
    use serde::de::DeserializeOwned;
    use std::fmt::Debug;
    use std::hash::Hash;

    fn assert_small_id_contract<T>()
    where
        T: Debug
            + Clone
            + Copy
            + PartialEq
            + Eq
            + PartialOrd
            + Ord
            + Hash
            + Serialize
            + DeserializeOwned,
    {
    }

    #[test]
    fn call_target_id_is_copy_ordered_hashable_serializable_and_distinct_from_call_site_id() {
        assert_small_id_contract::<CallTargetId>();

        let site = CallSiteId(7);
        let target = CallTargetId(7);

        assert_eq!(site.0, target.0);
    }

    #[test]
    fn call_facts_keep_dense_ids_and_stable_keys_separate() {
        let site = CallSiteFact {
            in_throw: false,
            id: CallSiteId(1),
            language: Language::TypeScript,
            file: FileId(2),
            caller: FunctionId(3),
            owner_symbol: Some(SymbolId(4)),
            body: MirBodyId(5),
            operation: MirOpId(6),
            span: Span::point(FileId(2), 10, 20),
            kind: CallSyntaxKind::Function,
            callee: CallCallee::Identifier {
                reference: None,
                name: "run".to_string(),
            },
            receiver: None,
            arguments: vec![PlaceId(7)],
            result: Some(PlaceId(8)),
            status: CallTargetStatus::Resolved,
            precision: CallPrecision::Exact,
            stable_key: "call-site:test".to_string(),
        };

        let target = CallTargetFact {
            id: CallTargetId(9),
            site: site.id,
            caller: site.caller,
            target_function: Some(FunctionId(10)),
            target_symbol: Some(SymbolId(11)),
            edge_kind: CallEdgeKind::Direct,
            algorithm: CallAlgorithm::DirectReference,
            status: CallTargetStatus::Resolved,
            reason: None,
            provenance: CallProvenance::Native,
            precision: CallPrecision::Exact,
            stable_key: "call-target:test".to_string(),
        };

        assert_ne!(site.stable_key, target.stable_key);
        assert_eq!(site.id, target.site);
    }

    #[test]
    fn call_status_and_reason_vocabulary_covers_direct_and_unknown_forms() {
        let statuses = [
            CallTargetStatus::Resolved,
            CallTargetStatus::Ambiguous,
            CallTargetStatus::Unresolved,
            CallTargetStatus::Unsupported,
            CallTargetStatus::SetupMissing,
        ];
        let reasons = [
            UnresolvedCallReason::FunctionValue,
            UnresolvedCallReason::DynamicProperty,
            UnresolvedCallReason::InterfaceDispatch,
            UnresolvedCallReason::Eval,
            UnresolvedCallReason::CallApplyBind,
            UnresolvedCallReason::FrameworkDispatch,
        ];

        assert_eq!(statuses.len(), 5);
        assert_eq!(reasons.len(), 6);
    }
}

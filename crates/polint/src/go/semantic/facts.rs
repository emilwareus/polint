use crate::core::{FileId, Span, StableKeyId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct GoSemanticPackageId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct GoSemanticFunctionId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct GoSemanticCallsiteId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct GoSemanticMethodSetId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct GoSemanticPackageErrorId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct GoSemanticAddressTakenId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct GoSemanticInstantiatedTypeId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct GoSemanticDynamicDispatchId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct GoSemanticRtaEdgeId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum GoSemanticFunctionKind {
    Function,
    Method,
    Init,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum GoSemanticCallStatus {
    ResolvedStatic,
    UnresolvedDynamic,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoSemanticPackageFact {
    pub(crate) id: GoSemanticPackageId,
    pub(crate) stable_key: StableKeyId,
    pub(crate) package_id: String,
    pub(crate) package_path: String,
    pub(crate) package_name: String,
    pub(crate) module_path: String,
    pub(crate) files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoSemanticFunctionFact {
    pub(crate) id: GoSemanticFunctionId,
    pub(crate) stable_key: StableKeyId,
    pub(crate) package_id: String,
    pub(crate) package_path: String,
    pub(crate) name: String,
    pub(crate) qualified: String,
    pub(crate) signature: String,
    pub(crate) kind: GoSemanticFunctionKind,
    pub(crate) receiver: Option<String>,
    pub(crate) relative_file: Option<String>,
    pub(crate) file: Option<FileId>,
    pub(crate) span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoSemanticCallsiteFact {
    pub(crate) id: GoSemanticCallsiteId,
    pub(crate) stable_key: StableKeyId,
    pub(crate) package_id: String,
    pub(crate) package_path: String,
    pub(crate) caller: String,
    pub(crate) static_callee: Option<String>,
    pub(crate) status: GoSemanticCallStatus,
    pub(crate) reason: Option<String>,
    pub(crate) relative_file: Option<String>,
    pub(crate) file: Option<FileId>,
    pub(crate) span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoSemanticMethodSetFact {
    pub(crate) id: GoSemanticMethodSetId,
    pub(crate) stable_key: StableKeyId,
    pub(crate) package_id: String,
    pub(crate) package_path: String,
    pub(crate) type_name: String,
    pub(crate) methods: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoSemanticPackageErrorFact {
    pub(crate) id: GoSemanticPackageErrorId,
    pub(crate) stable_key: StableKeyId,
    pub(crate) package_id: String,
    pub(crate) package_path: String,
    pub(crate) message: String,
}

/// An address-taken Go function — the RTA dispatch-candidate set for func-value
/// callsites (D-05). Harvested from `*ssa.MakeClosure` and `*ssa.Function` value
/// operands in the sidecar. `function` is the official `ssa.Function` `.String()`
/// identity; `stable_key` is length-prefixed from that identity (D-12/D-13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoSemanticAddressTakenFact {
    pub(crate) id: GoSemanticAddressTakenId,
    pub(crate) stable_key: StableKeyId,
    pub(crate) package_id: String,
    pub(crate) package_path: String,
    pub(crate) function: String,
}

/// An instantiated runtime type — the RTA "rapid type" set: a concrete type converted
/// to an interface via `*ssa.MakeInterface` in the reachable SSA program (D-05). The
/// instantiated-type filter is what distinguishes RTA from coarse CHA. `type_name` is the
/// official `go/types` `.String()` identity; `stable_key` is length-prefixed from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoSemanticInstantiatedTypeFact {
    pub(crate) id: GoSemanticInstantiatedTypeId,
    pub(crate) stable_key: StableKeyId,
    pub(crate) package_id: String,
    pub(crate) package_path: String,
    pub(crate) type_name: String,
}

/// Dynamic-callsite dispatch detail — the discriminant Plan 2's RTA driver needs to
/// resolve an `UnresolvedDynamic` callsite by method-set matching (D-05). For an interface
/// invoke, `interface_type` + `method` are set; for a func-value call, `signature` is set;
/// honest `None` otherwise (D-08/D-15 — no fabricated discriminant). `callsite_stable_key`
/// joins this detail back to the originating [`GoSemanticCallsiteFact`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoSemanticDynamicDispatchFact {
    pub(crate) id: GoSemanticDynamicDispatchId,
    pub(crate) stable_key: StableKeyId,
    pub(crate) package_id: String,
    pub(crate) package_path: String,
    pub(crate) caller: String,
    pub(crate) callsite_stable_key: StableKeyId,
    pub(crate) interface_type: Option<String>,
    pub(crate) method: Option<String>,
    pub(crate) signature: Option<String>,
}

/// A direct x/tools RTA call-graph edge emitted by the Go sidecar.
///
/// This is intentionally an internal evaluation fact, not a public rule-author API. The
/// existing source-backed solver/refined-call pipeline cannot represent synthetic SSA
/// functions such as `init$1`, generic instantiations, bound method wrappers, or
/// reflection synthetic calls. The external x/tools benchmark therefore consumes this
/// fact directly instead of forcing those oracle identities through source-only facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoSemanticRtaEdgeFact {
    pub(crate) id: GoSemanticRtaEdgeId,
    pub(crate) stable_key: StableKeyId,
    pub(crate) package_id: String,
    pub(crate) package_path: String,
    pub(crate) caller: String,
    pub(crate) callee: String,
    pub(crate) edge_kind: String,
}

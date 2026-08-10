use polint_core::{FileId, Span, StableKeyId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GoSemanticPackageId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GoSemanticFunctionId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GoSemanticCallsiteId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GoSemanticMethodSetId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GoSemanticPackageErrorId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GoSemanticAddressTakenId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GoSemanticInstantiatedTypeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GoSemanticDynamicDispatchId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GoSemanticRtaEdgeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GoSemanticFunctionKind {
    Function,
    Method,
    Init,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GoSemanticCallStatus {
    ResolvedStatic,
    UnresolvedDynamic,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoSemanticPackageFact {
    pub id: GoSemanticPackageId,
    pub stable_key: StableKeyId,
    pub package_id: String,
    pub package_path: String,
    pub package_name: String,
    pub module_path: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoSemanticFunctionFact {
    pub id: GoSemanticFunctionId,
    pub stable_key: StableKeyId,
    pub package_id: String,
    pub package_path: String,
    pub name: String,
    pub qualified: String,
    pub signature: String,
    pub kind: GoSemanticFunctionKind,
    pub receiver: Option<String>,
    pub relative_file: Option<String>,
    pub file: Option<FileId>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoSemanticCallsiteFact {
    pub id: GoSemanticCallsiteId,
    pub stable_key: StableKeyId,
    pub package_id: String,
    pub package_path: String,
    pub caller: String,
    pub static_callee: Option<String>,
    pub status: GoSemanticCallStatus,
    pub reason: Option<String>,
    pub relative_file: Option<String>,
    pub file: Option<FileId>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoSemanticMethodSetFact {
    pub id: GoSemanticMethodSetId,
    pub stable_key: StableKeyId,
    pub package_id: String,
    pub package_path: String,
    pub type_name: String,
    pub methods: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoSemanticPackageErrorFact {
    pub id: GoSemanticPackageErrorId,
    pub stable_key: StableKeyId,
    pub package_id: String,
    pub package_path: String,
    pub message: String,
}

/// An address-taken Go function — the RTA dispatch-candidate set for func-value
/// callsites (D-05). Harvested from `*ssa.MakeClosure` and `*ssa.Function` value
/// operands in the sidecar. `function` is the official `ssa.Function` `.String()`
/// identity; `stable_key` is length-prefixed from that identity (D-12/D-13).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoSemanticAddressTakenFact {
    pub id: GoSemanticAddressTakenId,
    pub stable_key: StableKeyId,
    pub package_id: String,
    pub package_path: String,
    pub function: String,
}

/// An instantiated runtime type — the RTA "rapid type" set: a concrete type converted
/// to an interface via `*ssa.MakeInterface` in the reachable SSA program (D-05). The
/// instantiated-type filter is what distinguishes RTA from coarse CHA. `type_name` is the
/// official `go/types` `.String()` identity; `stable_key` is length-prefixed from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoSemanticInstantiatedTypeFact {
    pub id: GoSemanticInstantiatedTypeId,
    pub stable_key: StableKeyId,
    pub package_id: String,
    pub package_path: String,
    pub type_name: String,
}

/// Dynamic-callsite dispatch detail — the discriminant Plan 2's RTA driver needs to
/// resolve an `UnresolvedDynamic` callsite by method-set matching (D-05). For an interface
/// invoke, `interface_type` + `method` are set; for a func-value call, `signature` is set;
/// honest `None` otherwise (D-08/D-15 — no fabricated discriminant). `callsite_stable_key`
/// joins this detail back to the originating [`GoSemanticCallsiteFact`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoSemanticDynamicDispatchFact {
    pub id: GoSemanticDynamicDispatchId,
    pub stable_key: StableKeyId,
    pub package_id: String,
    pub package_path: String,
    pub caller: String,
    pub callsite_stable_key: StableKeyId,
    pub interface_type: Option<String>,
    pub method: Option<String>,
    pub signature: Option<String>,
}

/// A direct x/tools RTA call-graph edge emitted by the Go sidecar.
///
/// This is intentionally an internal evaluation fact, not a public rule-author API. The
/// existing source-backed solver/refined-call pipeline cannot represent synthetic SSA
/// functions such as `init$1`, generic instantiations, bound method wrappers, or
/// reflection synthetic calls. The external x/tools benchmark therefore consumes this
/// fact directly instead of forcing those oracle identities through source-only facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoSemanticRtaEdgeFact {
    pub id: GoSemanticRtaEdgeId,
    pub stable_key: StableKeyId,
    pub package_id: String,
    pub package_path: String,
    pub caller: String,
    pub callee: String,
    pub edge_kind: String,
}

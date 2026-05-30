use serde::{Deserialize, Serialize};

use crate::analysis::ids::{CallSiteId, ObjectTokenId, PlaceId, SemanticEdgeId, SemanticNodeId};
use crate::core::{FunctionId, ModuleNodeId, PackageId};
use crate::symbol_graph::semantic::ScopeId;

// ---------------------------------------------------------------------------
// NodeKind
// ---------------------------------------------------------------------------

/// Closed taxonomy of semantic-graph node kinds (D-02, D-03).
///
/// Each variant **composes an existing v1.2 identity newtype by reference** (D-04)
/// — the graph invents no parallel identities. Pinned declaration order so the
/// derived `Ord` and serde representation are declaration-driven and byte-stable,
/// matching the established `RootKind`/`PointsToConstraintKind` convention in this
/// codebase. No explicit integer-ordinal representation attribute (`#[repr(u8)]`)
/// is used — byte-stability is achieved purely via pinned order + derived `Ord` +
/// serde rename + an `as_str()` label method, exactly as every other closed enum
/// here does.
///
/// Because the variants carry payloads they are NOT `Copy`; every payload field is
/// an `Ord` ID newtype so the derived `Ord` that drives byte-stable ordering
/// survives.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NodeKind {
    Function(FunctionId),
    Callsite(CallSiteId),
    Scope(ScopeId),
    Place(PlaceId),
    AbstractObject(ObjectTokenId),
    Module(ModuleNodeId),
    Package(PackageId),
}

impl NodeKind {
    /// Stable lowercase tag label used in stable keys and digest payloads.
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Function(_) => "function",
            Self::Callsite(_) => "callsite",
            Self::Scope(_) => "scope",
            Self::Place(_) => "place",
            Self::AbstractObject(_) => "abstract_object",
            Self::Module(_) => "module",
            Self::Package(_) => "package",
        }
    }
}

// ---------------------------------------------------------------------------
// EdgeKind
// ---------------------------------------------------------------------------

/// Closed taxonomy of semantic-graph edge kinds (D-02, D-03).
///
/// A fieldless `Copy` enum following the `RootKind` template exactly: pinned
/// declaration order + derived `Ord` + `#[serde(rename_all = "snake_case")]` + an
/// `as_str()` label, with NO `#[repr(u8)]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EdgeKind {
    Call,
    MemberOf,
    Alloc,
    Flow,
}

impl EdgeKind {
    /// Stable lowercase label used in stable keys and digest payloads.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Call => "call",
            Self::MemberOf => "member_of",
            Self::Alloc => "alloc",
            Self::Flow => "flow",
        }
    }
}

// ---------------------------------------------------------------------------
// Precision vocabulary
// ---------------------------------------------------------------------------

/// Honest precision tier carried by every node and edge (D-07).
///
/// Reuses the same shape as `points_to::facts::PointsToPrecision` — graph rows are
/// derived/aggregated, so the `Exact` ceiling is intentionally absent here; the
/// precision-ceiling enforcement itself lands in Plan 03 validation. Pinned order +
/// derived `Ord` + serde rename for byte-stability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SemanticPrecision {
    ResolvedStatic,
    SetupAware,
    FlowInsensitive,
    Heuristic,
    Conservative,
    Unknown,
}

impl SemanticPrecision {
    /// Stable lowercase label used in stable keys and digest payloads.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ResolvedStatic => "resolved_static",
            Self::SetupAware => "setup_aware",
            Self::FlowInsensitive => "flow_insensitive",
            Self::Heuristic => "heuristic",
            Self::Conservative => "conservative",
            Self::Unknown => "unknown",
        }
    }
}

// ---------------------------------------------------------------------------
// Fact families
// ---------------------------------------------------------------------------

/// One semantic-graph node. Mirrors the `ReachabilityRootFact` shape: the dense
/// `id` is a run-local post-normalization read concern only and MUST NOT enter any
/// serialized stable payload that feeds the output digest (D-06) — `#[serde(skip)]`
/// strips it; serde restores it via `SemanticNodeId::default()` (= 0).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SemanticNodeFact {
    #[serde(skip)]
    pub(crate) id: SemanticNodeId,
    pub(crate) kind: NodeKind,
    pub(crate) precision: SemanticPrecision,
    /// Built from the referenced existing identity (D-06), never run-local dense
    /// IDs. Populated by Plan 03's builder; the skeleton carries the field.
    pub(crate) stable_key: String,
}

/// One semantic-graph edge. `source`/`target` are dense `SemanticNodeId` handles
/// resolved after normalization; the persistent identity is carried by
/// `stable_key`, built from the source/target node stable keys (D-06). The dense
/// `id` carries `#[serde(skip)]` for the same digest discipline as the node fact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SemanticEdgeFact {
    #[serde(skip)]
    pub(crate) id: SemanticEdgeId,
    pub(crate) source: SemanticNodeId,
    pub(crate) target: SemanticNodeId,
    pub(crate) kind: EdgeKind,
    pub(crate) precision: SemanticPrecision,
    pub(crate) stable_key: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_kind_composes_existing_identity_newtypes() {
        // Each variant wraps an existing v1.2 identity newtype (D-04); no parallel
        // identity is invented.
        let nodes = [
            NodeKind::Function(FunctionId(1)),
            NodeKind::Callsite(CallSiteId(2)),
            NodeKind::Scope(ScopeId(3)),
            NodeKind::Place(PlaceId(4)),
            NodeKind::AbstractObject(ObjectTokenId(5)),
            NodeKind::Module(ModuleNodeId(6)),
            NodeKind::Package(PackageId(7)),
        ];
        assert_eq!(nodes.len(), 7);
    }

    #[test]
    fn node_kind_labels_are_stable_snake_case() {
        assert_eq!(NodeKind::Function(FunctionId(0)).as_str(), "function");
        assert_eq!(NodeKind::Callsite(CallSiteId(0)).as_str(), "callsite");
        assert_eq!(NodeKind::Scope(ScopeId(0)).as_str(), "scope");
        assert_eq!(NodeKind::Place(PlaceId(0)).as_str(), "place");
        assert_eq!(
            NodeKind::AbstractObject(ObjectTokenId(0)).as_str(),
            "abstract_object"
        );
        assert_eq!(NodeKind::Module(ModuleNodeId(0)).as_str(), "module");
        assert_eq!(NodeKind::Package(PackageId(0)).as_str(), "package");
    }

    #[test]
    fn node_kind_has_exactly_7_variants() {
        // Compile-time exhaustive match over every arm; the array length lock fails
        // to compile if a variant is added without updating this test.
        fn assert_all(kind: &NodeKind) -> &'static str {
            match kind {
                NodeKind::Function(_) => "function",
                NodeKind::Callsite(_) => "callsite",
                NodeKind::Scope(_) => "scope",
                NodeKind::Place(_) => "place",
                NodeKind::AbstractObject(_) => "abstract_object",
                NodeKind::Module(_) => "module",
                NodeKind::Package(_) => "package",
            }
        }
        let variants = [
            assert_all(&NodeKind::Function(FunctionId(0))),
            assert_all(&NodeKind::Callsite(CallSiteId(0))),
            assert_all(&NodeKind::Scope(ScopeId(0))),
            assert_all(&NodeKind::Place(PlaceId(0))),
            assert_all(&NodeKind::AbstractObject(ObjectTokenId(0))),
            assert_all(&NodeKind::Module(ModuleNodeId(0))),
            assert_all(&NodeKind::Package(PackageId(0))),
        ];
        assert_eq!(variants.len(), 7);
    }

    #[test]
    fn node_kind_module_variant_uses_module_node_id() {
        // V3 correction guard: the module node composes `core::ModuleNodeId`, not a
        // (non-existent) `ModuleId`.
        let module = NodeKind::Module(ModuleNodeId(42));
        match module {
            NodeKind::Module(id) => assert_eq!(id, ModuleNodeId(42)),
            _ => panic!("expected module variant"),
        }
    }

    #[test]
    fn edge_kind_sorts_in_pinned_declaration_order() {
        let mut kinds = vec![
            EdgeKind::Flow,
            EdgeKind::Alloc,
            EdgeKind::MemberOf,
            EdgeKind::Call,
        ];
        kinds.sort();
        assert_eq!(
            kinds,
            vec![
                EdgeKind::Call,
                EdgeKind::MemberOf,
                EdgeKind::Alloc,
                EdgeKind::Flow,
            ]
        );
    }

    #[test]
    fn edge_kind_has_exactly_4_variants() {
        fn assert_all(kind: EdgeKind) -> EdgeKind {
            match kind {
                EdgeKind::Call | EdgeKind::MemberOf | EdgeKind::Alloc | EdgeKind::Flow => kind,
            }
        }
        let variants = [
            assert_all(EdgeKind::Call),
            assert_all(EdgeKind::MemberOf),
            assert_all(EdgeKind::Alloc),
            assert_all(EdgeKind::Flow),
        ];
        assert_eq!(variants.len(), 4);
    }

    #[test]
    fn edge_kind_labels_are_stable_snake_case() {
        assert_eq!(EdgeKind::Call.as_str(), "call");
        assert_eq!(EdgeKind::MemberOf.as_str(), "member_of");
        assert_eq!(EdgeKind::Alloc.as_str(), "alloc");
        assert_eq!(EdgeKind::Flow.as_str(), "flow");
    }

    #[test]
    fn node_fact_round_trips_through_serde_json() {
        let node = SemanticNodeFact {
            id: SemanticNodeId(7),
            kind: NodeKind::Function(FunctionId(3)),
            precision: SemanticPrecision::SetupAware,
            stable_key: "node|function|pkg.F".to_string(),
        };
        let json = serde_json::to_string(&node).expect("serialize");
        // The dense id is skipped, so the round-tripped value carries the default
        // id (0) while every other field is preserved.
        let restored: SemanticNodeFact = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.id, SemanticNodeId(0));
        assert_eq!(restored.kind, node.kind);
        assert_eq!(restored.precision, node.precision);
        assert_eq!(restored.stable_key, node.stable_key);
        // The skipped dense id must not appear in the serialized payload.
        assert!(!json.contains("\"id\""));
    }

    #[test]
    fn edge_fact_round_trips_through_serde_json() {
        let edge = SemanticEdgeFact {
            id: SemanticEdgeId(9),
            source: SemanticNodeId(1),
            target: SemanticNodeId(2),
            kind: EdgeKind::Call,
            precision: SemanticPrecision::Conservative,
            stable_key: "edge|call|a|b".to_string(),
        };
        let json = serde_json::to_string(&edge).expect("serialize");
        let restored: SemanticEdgeFact = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.id, SemanticEdgeId(0));
        assert_eq!(restored.source, edge.source);
        assert_eq!(restored.target, edge.target);
        assert_eq!(restored.kind, edge.kind);
        assert_eq!(restored.stable_key, edge.stable_key);
    }

    #[test]
    fn semantic_precision_labels_are_stable_snake_case() {
        assert_eq!(
            SemanticPrecision::ResolvedStatic.as_str(),
            "resolved_static"
        );
        assert_eq!(SemanticPrecision::SetupAware.as_str(), "setup_aware");
        assert_eq!(
            SemanticPrecision::FlowInsensitive.as_str(),
            "flow_insensitive"
        );
        assert_eq!(SemanticPrecision::Unknown.as_str(), "unknown");
    }
}

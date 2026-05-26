use super::facts::{EvidenceEdgeFact, EvidenceEdgeKind, EvidenceNodeFact, EvidenceQueryMode};
use super::store::EvidenceStore;
use crate::analysis::ids::EvidenceNodeId;

#[allow(
    dead_code,
    reason = "Private evidence query helpers are consumed by subsequent Phase 39 path/rendering plans."
)]
pub(crate) fn incoming_edges(
    store: &EvidenceStore,
    node: EvidenceNodeId,
    mode: EvidenceQueryMode,
) -> Vec<&EvidenceEdgeFact> {
    store
        .incoming(node)
        .into_iter()
        .filter(|edge| edge.query_mode == mode || mode == EvidenceQueryMode::FullBackward)
        .collect()
}

#[allow(
    dead_code,
    reason = "Private evidence query helpers are consumed by subsequent Phase 39 path/rendering plans."
)]
pub(crate) fn outgoing_edges(
    store: &EvidenceStore,
    node: EvidenceNodeId,
    mode: EvidenceQueryMode,
) -> Vec<&EvidenceEdgeFact> {
    store
        .outgoing(node)
        .into_iter()
        .filter(|edge| edge.query_mode == mode || mode == EvidenceQueryMode::ForwardImpact)
        .collect()
}

#[allow(
    dead_code,
    reason = "Private evidence query helpers are consumed by subsequent Phase 39 path/rendering plans."
)]
pub(crate) fn nodes_by_edge_kind(
    store: &EvidenceStore,
    kind: EvidenceEdgeKind,
) -> Vec<&EvidenceNodeFact> {
    let mut nodes = store
        .by_edge_kind(kind)
        .into_iter()
        .flat_map(|edge| [edge.from, edge.to])
        .filter_map(|node| store.node(node))
        .collect::<Vec<_>>();
    nodes.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
    nodes.dedup_by(|left, right| left.id == right.id);
    nodes
}

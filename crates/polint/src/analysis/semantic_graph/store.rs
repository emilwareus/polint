use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::error::AnalysisError;
use crate::analysis::ids::{SemanticEdgeId, SemanticNodeId};
use crate::analysis::semantic_graph::facts::{
    EdgeKind, NodeKind, SemanticEdgeFact, SemanticNodeFact,
};

pub(crate) const SEMANTIC_GRAPH_PROVIDER_ID: &str = "polint.semantic_graph";

/// Provider output for `polint.semantic_graph` — the normalized node and edge sets.
///
/// The constraint vocabulary arrives in Plan 02; this struct is intentionally left
/// extensible but does NOT carry a `constraints` field yet, so Plan 02 can add it
/// without churning a contract this plan would otherwise pin prematurely.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SemanticGraphOutput {
    pub(crate) nodes: Vec<SemanticNodeFact>,
    pub(crate) edges: Vec<SemanticEdgeFact>,
}

impl SemanticGraphOutput {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    /// Sorts nodes and edges by `(stable_key, id)` THEN reassigns dense
    /// `SemanticNodeId`/`SemanticEdgeId` sequentially by index (D-05: dense IDs only
    /// after the stable-key sort). Edge `source`/`target` handles are remapped from
    /// the pre-sort node IDs to the post-sort dense node IDs so the adjacency stays
    /// consistent after re-densification.
    pub(crate) fn normalized(mut self) -> Self {
        self.nodes.sort_by(|left, right| {
            (left.stable_key.as_str(), left.id).cmp(&(right.stable_key.as_str(), right.id))
        });
        // Map each node's pre-sort dense id to its post-sort dense id so edges can
        // be rewritten to the new node numbering.
        let mut remap: BTreeMap<SemanticNodeId, SemanticNodeId> = BTreeMap::new();
        for (index, node) in self.nodes.iter_mut().enumerate() {
            let new_id = SemanticNodeId(index as u64);
            remap.insert(node.id, new_id);
            node.id = new_id;
        }
        // Rewrite edge endpoints to the post-sort node IDs before sorting edges, so
        // an edge's `source`/`target` always reference the densified node numbering.
        for edge in &mut self.edges {
            if let Some(&new_source) = remap.get(&edge.source) {
                edge.source = new_source;
            }
            if let Some(&new_target) = remap.get(&edge.target) {
                edge.target = new_target;
            }
        }
        self.edges.sort_by(|left, right| {
            (left.stable_key.as_str(), left.id).cmp(&(right.stable_key.as_str(), right.id))
        });
        for (index, edge) in self.edges.iter_mut().enumerate() {
            edge.id = SemanticEdgeId(index as u64);
        }
        self
    }
}

/// Typed semantic-graph store with the deterministic read indexes consumers use
/// (D-14): nodes-by-kind, edges-by-kind, outgoing (forward) adjacency, and incoming
/// (backward) adjacency. The incoming index is the one the Phase 47 unified solver's
/// reachability/RTA fixpoint traverses; it is built alongside the outgoing index in
/// the same post-normalization pass, never on demand.
#[derive(Debug, Clone, Default)]
pub(crate) struct SemanticGraphStore {
    nodes: Vec<SemanticNodeFact>,
    edges: Vec<SemanticEdgeFact>,
    nodes_by_kind: BTreeMap<&'static str, Vec<usize>>,
    edges_by_kind: BTreeMap<EdgeKind, Vec<usize>>,
    /// Forward/outgoing adjacency: edge source node -> edge IDs leaving it.
    outgoing: BTreeMap<SemanticNodeId, Vec<SemanticEdgeId>>,
    /// Backward/incoming adjacency: edge target node -> edge IDs entering it.
    incoming: BTreeMap<SemanticNodeId, Vec<SemanticEdgeId>>,
}

impl SemanticGraphStore {
    /// Builds the store after `normalized()`, validating that every edge
    /// `source`/`target` resolves to a stored node (dangling endpoint ->
    /// [`AnalysisError::InvalidFact`], mirroring the reachability store) and building
    /// the four deterministic index sidecars.
    pub(crate) fn from_output(output: SemanticGraphOutput) -> Result<Self, AnalysisError> {
        let output = output.normalized();

        let node_ids: BTreeSet<SemanticNodeId> = output.nodes.iter().map(|node| node.id).collect();

        for edge in &output.edges {
            if !node_ids.contains(&edge.source) {
                return Err(AnalysisError::InvalidFact {
                    provider: SEMANTIC_GRAPH_PROVIDER_ID,
                    reason: format!(
                        "dangling edge source {:?} for semantic edge `{}`",
                        edge.source, edge.stable_key
                    ),
                });
            }
            if !node_ids.contains(&edge.target) {
                return Err(AnalysisError::InvalidFact {
                    provider: SEMANTIC_GRAPH_PROVIDER_ID,
                    reason: format!(
                        "dangling edge target {:?} for semantic edge `{}`",
                        edge.target, edge.stable_key
                    ),
                });
            }
        }

        let mut store = Self {
            nodes: output.nodes,
            edges: output.edges,
            ..Self::default()
        };

        for (index, node) in store.nodes.iter().enumerate() {
            store
                .nodes_by_kind
                .entry(node.kind.as_str())
                .or_default()
                .push(index);
        }

        // Single post-normalization edge pass builds the by-kind index and BOTH
        // adjacency directions so they stay deterministic and order-independent.
        // Edges are already sorted by (stable_key, id), so each per-node edge-id
        // vector is appended in stable order.
        for (index, edge) in store.edges.iter().enumerate() {
            store
                .edges_by_kind
                .entry(edge.kind)
                .or_default()
                .push(index);
            store.outgoing.entry(edge.source).or_default().push(edge.id);
            store.incoming.entry(edge.target).or_default().push(edge.id);
        }

        Ok(store)
    }

    pub(crate) fn nodes(&self) -> &[SemanticNodeFact] {
        &self.nodes
    }

    pub(crate) fn edges(&self) -> &[SemanticEdgeFact] {
        &self.edges
    }

    pub(crate) fn nodes_for_kind(&self, kind: &NodeKind) -> &[usize] {
        self.nodes_by_kind
            .get(kind.as_str())
            .map_or(&[], Vec::as_slice)
    }

    pub(crate) fn edges_for_kind(&self, kind: EdgeKind) -> &[usize] {
        self.edges_by_kind.get(&kind).map_or(&[], Vec::as_slice)
    }

    /// Outgoing (forward) adjacency: the edge IDs whose `source` is `node`.
    pub(crate) fn outgoing_edges(&self, node: SemanticNodeId) -> &[SemanticEdgeId] {
        self.outgoing.get(&node).map_or(&[], Vec::as_slice)
    }

    /// Incoming (backward) adjacency: the edge IDs whose `target` is `node`. This
    /// is the index the unified solver's reachability/RTA fixpoint traverses.
    pub(crate) fn incoming_edges(&self, node: SemanticNodeId) -> &[SemanticEdgeId] {
        self.incoming.get(&node).map_or(&[], Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ids::{CallSiteId, ObjectTokenId};
    use crate::analysis::semantic_graph::facts::SemanticPrecision;
    use crate::core::FunctionId;

    fn node(id: u64, kind: NodeKind, stable_key: &str) -> SemanticNodeFact {
        SemanticNodeFact {
            id: SemanticNodeId(id),
            kind,
            precision: SemanticPrecision::SetupAware,
            stable_key: stable_key.to_string(),
        }
    }

    fn edge(
        id: u64,
        source: u64,
        target: u64,
        kind: EdgeKind,
        stable_key: &str,
    ) -> SemanticEdgeFact {
        SemanticEdgeFact {
            id: SemanticEdgeId(id),
            source: SemanticNodeId(source),
            target: SemanticNodeId(target),
            kind,
            precision: SemanticPrecision::Conservative,
            stable_key: stable_key.to_string(),
        }
    }

    fn sample_output() -> SemanticGraphOutput {
        // Two nodes and one edge from node-A (source) to node-B (target).
        SemanticGraphOutput {
            nodes: vec![
                node(0, NodeKind::Function(FunctionId(1)), "node|function|a"),
                node(1, NodeKind::Callsite(CallSiteId(2)), "node|callsite|b"),
            ],
            edges: vec![edge(0, 0, 1, EdgeKind::Call, "edge|call|a|b")],
        }
    }

    #[test]
    fn normalized_assigns_dense_ids_after_stable_key_sort() {
        let normalized = SemanticGraphOutput {
            nodes: vec![
                node(99, NodeKind::Callsite(CallSiteId(2)), "node|callsite|z"),
                node(7, NodeKind::Function(FunctionId(1)), "node|function|a"),
            ],
            edges: Vec::new(),
        }
        .normalized();
        // Sorted by stable_key: "node|callsite|z" < "node|function|a" ('c' < 'f'),
        // so the callsite node sorts first and is assigned dense id 0, regardless of
        // its larger pre-sort id (99). Dense IDs are assigned only after the sort.
        assert_eq!(normalized.nodes[0].stable_key, "node|callsite|z");
        assert_eq!(normalized.nodes[0].id, SemanticNodeId(0));
        assert_eq!(normalized.nodes[1].stable_key, "node|function|a");
        assert_eq!(normalized.nodes[1].id, SemanticNodeId(1));
    }

    #[test]
    fn normalized_is_shuffle_stable() {
        let base = sample_output();
        // Clone and shuffle node + edge row order.
        let mut shuffled = base.clone();
        shuffled.nodes.reverse();
        shuffled.edges.reverse();

        let a = base.normalized();
        let b = shuffled.normalized();

        // Byte-identical serialized output under shuffle: serialize node and edge
        // vectors (dense `id` is `#[serde(skip)]`, so this captures kind/precision/
        // stable_key/endpoints).
        let a_nodes = serde_json::to_string(&a.nodes).expect("serialize a nodes");
        let b_nodes = serde_json::to_string(&b.nodes).expect("serialize b nodes");
        assert_eq!(a_nodes, b_nodes);
        let a_edges = serde_json::to_string(&a.edges).expect("serialize a edges");
        let b_edges = serde_json::to_string(&b.edges).expect("serialize b edges");
        assert_eq!(a_edges, b_edges);
        // The dense IDs assigned are identical too.
        assert_eq!(
            a.nodes.iter().map(|n| n.id).collect::<Vec<_>>(),
            b.nodes.iter().map(|n| n.id).collect::<Vec<_>>()
        );
        assert_eq!(
            a.edges.iter().map(|e| e.id).collect::<Vec<_>>(),
            b.edges.iter().map(|e| e.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn from_output_builds_deterministic_kind_indexes() {
        let store = SemanticGraphStore::from_output(sample_output()).expect("store");
        assert_eq!(store.nodes().len(), 2);
        assert_eq!(store.edges().len(), 1);
        assert_eq!(
            store
                .nodes_for_kind(&NodeKind::Function(FunctionId(0)))
                .len(),
            1
        );
        assert_eq!(
            store
                .nodes_for_kind(&NodeKind::Callsite(CallSiteId(0)))
                .len(),
            1
        );
        assert!(
            store
                .nodes_for_kind(&NodeKind::AbstractObject(ObjectTokenId(0)))
                .is_empty()
        );
        assert_eq!(store.edges_for_kind(EdgeKind::Call).len(), 1);
        assert!(store.edges_for_kind(EdgeKind::Flow).is_empty());
    }

    #[test]
    fn incoming_adjacency_is_built_and_consistent_with_outgoing() {
        let store = SemanticGraphStore::from_output(sample_output()).expect("store");

        // Resolve the dense node ids after normalization by stable key.
        let source = store
            .nodes()
            .iter()
            .find(|n| n.stable_key == "node|function|a")
            .expect("source node")
            .id;
        let target = store
            .nodes()
            .iter()
            .find(|n| n.stable_key == "node|callsite|b")
            .expect("target node")
            .id;
        let edge_id = store.edges()[0].id;

        // The source node's outgoing entry contains the edge id, and the target
        // node's incoming entry contains the SAME edge id — the two indexes mirror
        // each other for this fixture edge.
        assert_eq!(store.outgoing_edges(source), &[edge_id]);
        assert_eq!(store.incoming_edges(target), &[edge_id]);
        // The target has no outgoing edges and the source has no incoming edges.
        assert!(store.outgoing_edges(target).is_empty());
        assert!(store.incoming_edges(source).is_empty());
    }

    #[test]
    fn from_output_rejects_dangling_edge_endpoint() {
        let output = SemanticGraphOutput {
            nodes: vec![node(
                0,
                NodeKind::Function(FunctionId(1)),
                "node|function|a",
            )],
            // Edge targets node id 5 which does not resolve to a stored node.
            edges: vec![edge(0, 0, 5, EdgeKind::Call, "edge|call|a|missing")],
        };
        let error =
            SemanticGraphStore::from_output(output).expect_err("dangling endpoint rejected");
        assert!(error.to_string().contains("dangling edge target"));
        assert!(error.to_string().contains("polint.semantic_graph"));
    }

    #[test]
    fn empty_output_builds_empty_store() {
        let store =
            SemanticGraphStore::from_output(SemanticGraphOutput::empty()).expect("empty store");
        assert!(store.nodes().is_empty());
        assert!(store.edges().is_empty());
    }
}

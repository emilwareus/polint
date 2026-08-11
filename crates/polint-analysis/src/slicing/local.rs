use std::collections::{BTreeSet, VecDeque};

use crate::evidence::facts::{
    EvidenceEdgeKind, EvidencePrecision, EvidenceQueryMode, EvidenceStatus,
};
use crate::evidence::store::EvidenceStore;
use crate::ids::{EvidenceEdgeId, EvidenceNodeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SliceQuery {
    pub root: EvidenceNodeId,
    pub direction: SliceDirection,
    pub mode: SliceMode,
    pub edge_filter: EdgeFilter,
    pub budget: SliceBudget,
}

#[allow(
    dead_code,
    reason = "Forward slicing is part of the private query contract before public callers exist."
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceDirection {
    Backward,
    Forward,
}

#[allow(
    dead_code,
    reason = "All slice modes are part of the private query contract before public callers exist."
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceMode {
    ThinBackward,
    FullBackward,
    FullLocal,
    ForwardImpact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeFilter {
    Thin,
    FullLocal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SliceBudget {
    pub max_nodes: usize,
    pub max_edges: usize,
    pub max_depth: usize,
}

impl Default for SliceBudget {
    fn default() -> Self {
        Self {
            max_nodes: 64,
            max_edges: 96,
            max_depth: 32,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceResult {
    pub nodes: Vec<EvidenceNodeId>,
    pub edges: Vec<EvidenceEdgeId>,
    pub omitted_regions: Vec<SliceOmittedRegion>,
    pub unknown_edges: Vec<EvidenceEdgeId>,
    pub status: EvidenceStatus,
    pub precision: EvidencePrecision,
    pub stats: SliceStats,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceOmittedRegion {
    pub reason: SliceOmittedReason,
    pub hidden_node_count: u32,
    pub hidden_edge_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceOmittedReason {
    NodeLimit,
    EdgeLimit,
    DepthLimit,
    FilteredEdges,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SliceStats {
    pub visited_nodes: usize,
    pub visited_edges: usize,
    pub filtered_edges: usize,
    pub max_depth_reached: usize,
}

pub fn local_slice(store: &EvidenceStore, query: SliceQuery) -> SliceResult {
    let mut nodes = BTreeSet::from([query.root]);
    let mut edges = BTreeSet::new();
    let mut unknown_edges = BTreeSet::new();
    let mut omitted_regions = Vec::new();
    let mut queue = VecDeque::from([(query.root, 0usize)]);
    let mut filtered_edges = 0usize;
    let mut depth_limit_hit = false;

    while let Some((node, depth)) = queue.pop_front() {
        if depth >= query.budget.max_depth {
            let has_more = candidate_edges(store, node, query.direction)
                .into_iter()
                .any(|edge| !nodes.contains(&next_node(edge, query.direction)));
            if has_more {
                depth_limit_hit = true;
            }
            continue;
        }

        for edge in candidate_edges(store, node, query.direction) {
            if !edge_allowed(edge.kind, query.edge_filter) {
                filtered_edges += 1;
                continue;
            }
            if edges.len() >= query.budget.max_edges {
                omitted_regions.push(SliceOmittedRegion {
                    reason: SliceOmittedReason::EdgeLimit,
                    hidden_node_count: 0,
                    hidden_edge_count: 1,
                });
                continue;
            }
            let next = next_node(edge, query.direction);
            if !nodes.contains(&next) {
                if nodes.len() >= query.budget.max_nodes {
                    omitted_regions.push(SliceOmittedRegion {
                        reason: SliceOmittedReason::NodeLimit,
                        hidden_node_count: 1,
                        hidden_edge_count: 0,
                    });
                    continue;
                }
                nodes.insert(next);
                queue.push_back((next, depth + 1));
            }
            edges.insert(edge.id);
            if edge.kind == EvidenceEdgeKind::Unknown || edge.status != EvidenceStatus::Present {
                unknown_edges.insert(edge.id);
            }
        }
    }

    if filtered_edges > 0 && query.edge_filter == EdgeFilter::Thin {
        omitted_regions.push(SliceOmittedRegion {
            reason: SliceOmittedReason::FilteredEdges,
            hidden_node_count: 0,
            hidden_edge_count: filtered_edges as u32,
        });
    }
    if depth_limit_hit {
        omitted_regions.push(SliceOmittedRegion {
            reason: SliceOmittedReason::DepthLimit,
            hidden_node_count: 0,
            hidden_edge_count: 1,
        });
    }

    let status = if !omitted_regions.is_empty() {
        EvidenceStatus::Partial
    } else if !unknown_edges.is_empty() {
        EvidenceStatus::Unknown
    } else {
        EvidenceStatus::Present
    };
    let precision = match query.edge_filter {
        EdgeFilter::Thin => EvidencePrecision::Syntax,
        EdgeFilter::FullLocal => EvidencePrecision::Conservative,
    };

    let nodes = nodes.into_iter().collect::<Vec<_>>();
    let edges = edges.into_iter().collect::<Vec<_>>();
    let unknown_edges = unknown_edges.into_iter().collect::<Vec<_>>();
    SliceResult {
        stats: SliceStats {
            visited_nodes: nodes.len(),
            visited_edges: edges.len(),
            filtered_edges,
            max_depth_reached: query.budget.max_depth,
        },
        nodes,
        edges,
        omitted_regions,
        unknown_edges,
        status,
        precision,
    }
}

impl SliceMode {
    #[allow(
        dead_code,
        reason = "Mode-to-evidence-key mapping is consumed by subsequent cache/debug plans."
    )]
    pub fn query_mode(self) -> EvidenceQueryMode {
        match self {
            Self::ThinBackward => EvidenceQueryMode::ThinBackward,
            Self::FullBackward | Self::FullLocal => EvidenceQueryMode::FullBackward,
            Self::ForwardImpact => EvidenceQueryMode::ForwardImpact,
        }
    }
}

fn candidate_edges(
    store: &EvidenceStore,
    node: EvidenceNodeId,
    direction: SliceDirection,
) -> Vec<&crate::evidence::facts::EvidenceEdgeFact> {
    match direction {
        SliceDirection::Backward => store.incoming(node),
        SliceDirection::Forward => store.outgoing(node),
    }
}

fn next_node(
    edge: &crate::evidence::facts::EvidenceEdgeFact,
    direction: SliceDirection,
) -> EvidenceNodeId {
    match direction {
        SliceDirection::Backward => edge.from,
        SliceDirection::Forward => edge.to,
    }
}

fn edge_allowed(kind: EvidenceEdgeKind, filter: EdgeFilter) -> bool {
    match filter {
        EdgeFilter::Thin => matches!(
            kind,
            EvidenceEdgeKind::DataValue | EvidenceEdgeKind::Summary | EvidenceEdgeKind::Model
        ),
        EdgeFilter::FullLocal => matches!(
            kind,
            EvidenceEdgeKind::DataValue
                | EvidenceEdgeKind::DataTaint
                | EvidenceEdgeKind::DataAddress
                | EvidenceEdgeKind::Control
                | EvidenceEdgeKind::Call
                | EvidenceEdgeKind::Return
                | EvidenceEdgeKind::ParameterIn
                | EvidenceEdgeKind::ParameterOut
                | EvidenceEdgeKind::Summary
                | EvidenceEdgeKind::Model
                | EvidenceEdgeKind::Alias
                | EvidenceEdgeKind::Unknown
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::facts::{
        EvidenceConfidence, EvidenceEdgeFact, EvidenceExpansion, EvidenceNodeFact,
        EvidenceNodeKind, EvidenceProvenance, EvidenceValidation,
    };
    use crate::evidence::store::EvidenceOutput;
    use polint_core::Language;

    #[test]
    fn thin_backward_is_subset_of_full_backward() {
        let store = store_with_value_and_control_edges();

        let thin = local_slice(&store, query(EdgeFilter::Thin));
        let full = local_slice(&store, query(EdgeFilter::FullLocal));

        assert!(thin.edges.iter().all(|edge| full.edges.contains(edge)));
        assert!(full.edges.contains(&EvidenceEdgeId(1)));
    }

    #[test]
    fn thin_backward_includes_direct_value_producers() {
        let store = store_with_value_and_control_edges();

        let result = local_slice(&store, query(EdgeFilter::Thin));

        assert!(result.edges.contains(&EvidenceEdgeId(0)));
        assert!(result.nodes.contains(&EvidenceNodeId(0)));
    }

    #[test]
    fn full_local_includes_control_dependencies() {
        let store = store_with_value_and_control_edges();

        let result = local_slice(&store, query(EdgeFilter::FullLocal));

        assert!(result.edges.contains(&EvidenceEdgeId(1)));
    }

    #[test]
    fn thin_filter_reports_omitted_regions_for_filtered_edges() {
        let store = store_with_value_and_control_edges();

        let result = local_slice(&store, query(EdgeFilter::Thin));

        assert!(
            result
                .omitted_regions
                .iter()
                .any(|region| region.reason == SliceOmittedReason::FilteredEdges)
        );
    }

    #[test]
    fn local_slice_omits_edge_when_endpoint_node_exceeds_budget() {
        let store = store_with_value_and_control_edges();
        let mut query = query(EdgeFilter::FullLocal);
        query.budget.max_nodes = 2;

        let result = local_slice(&store, query);

        assert!(result.nodes.contains(&EvidenceNodeId(0)));
        assert!(!result.nodes.contains(&EvidenceNodeId(1)));
        assert!(result.edges.contains(&EvidenceEdgeId(0)));
        assert!(!result.edges.contains(&EvidenceEdgeId(1)));
    }

    fn query(edge_filter: EdgeFilter) -> SliceQuery {
        SliceQuery {
            root: EvidenceNodeId(2),
            direction: SliceDirection::Backward,
            mode: SliceMode::ThinBackward,
            edge_filter,
            budget: SliceBudget::default(),
        }
    }

    fn store_with_value_and_control_edges() -> EvidenceStore {
        EvidenceStore::from_output(
            EvidenceOutput {
                nodes: vec![node(0), node(1), node(2)],
                edges: vec![
                    edge(0, 0, 2, EvidenceEdgeKind::DataValue),
                    edge(1, 1, 2, EvidenceEdgeKind::Control),
                ],
                bundles: Vec::new(),
                paths: Vec::new(),
                slices: Vec::new(),
                unknowns: Vec::new(),
                omitted_regions: Vec::new(),
                replay_keys: Vec::new(),
            },
            &polint_core::test_stable_key_interner(),
        )
        .expect("valid evidence store")
    }

    fn node(id: u64) -> EvidenceNodeFact {
        EvidenceNodeFact {
            id: EvidenceNodeId(id),
            kind: EvidenceNodeKind::Operation,
            language: Language::Go,
            file: None,
            function: None,
            body: None,
            operation: None,
            cfg_node: None,
            place: None,
            symbol: None,
            reference: None,
            call_site: None,
            span: None,
            status: EvidenceStatus::Present,
            precision: EvidencePrecision::Syntax,
            provenance: EvidenceProvenance::Native,
            validation: EvidenceValidation::Native,
            confidence: EvidenceConfidence::High,
            compact_label: None,
            source_fact_stable_keys: Vec::new(),
            stable_key: polint_core::stable_key_for_test(&format!("node:{id}")),
        }
    }

    fn edge(id: u64, from: u64, to: u64, kind: EvidenceEdgeKind) -> EvidenceEdgeFact {
        EvidenceEdgeFact {
            id: EvidenceEdgeId(id),
            from: EvidenceNodeId(from),
            to: EvidenceNodeId(to),
            kind,
            query_mode: EvidenceQueryMode::FullBackward,
            status: EvidenceStatus::Present,
            precision: EvidencePrecision::Syntax,
            provenance: EvidenceProvenance::Native,
            validation: EvidenceValidation::Native,
            confidence: EvidenceConfidence::High,
            call_site: None,
            summary_stable_key: None,
            expansion: EvidenceExpansion::None,
            compact_label: None,
            source_fact_stable_keys: Vec::new(),
            stable_key: polint_core::stable_key_for_test(&format!("edge:{id}")),
        }
    }
}

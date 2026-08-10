use std::collections::{BTreeSet, VecDeque};

use crate::analysis::evidence::facts::{EvidenceEdgeFact, EvidenceEdgeKind, EvidenceStatus};
use crate::analysis::evidence::rank::{PathRankScore, compare_scores, rank_score_for_edges};
use crate::analysis::evidence::store::EvidenceStore;
use crate::analysis::ids::{CallSiteId, EvidenceEdgeId, EvidenceNodeId};
use crate::analysis::slicing::paths::{
    PathBudget, PathMode, PathOmittedReason, PathOmittedRegion, path_edge_allowed,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InterproceduralPathQuery {
    pub(crate) source: EvidenceNodeId,
    pub(crate) sink: EvidenceNodeId,
    pub(crate) budget: PathBudget,
    pub(crate) max_interprocedural_depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InterproceduralPathResult {
    pub(crate) paths: Vec<InterproceduralPath>,
    pub(crate) omitted_regions: Vec<PathOmittedRegion>,
    pub(crate) unknown_edges: Vec<EvidenceEdgeId>,
    pub(crate) status: EvidenceStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InterproceduralPath {
    pub(crate) nodes: Vec<EvidenceNodeId>,
    pub(crate) edges: Vec<EvidenceEdgeId>,
    pub(crate) score: PathRankScore,
    pub(crate) stable_key_text: String,
}

pub(crate) fn find_interprocedural_paths(
    store: &EvidenceStore,
    query: InterproceduralPathQuery,
) -> InterproceduralPathResult {
    if query.budget.max_paths == 0 {
        return InterproceduralPathResult {
            paths: Vec::new(),
            omitted_regions: vec![PathOmittedRegion {
                reason: PathOmittedReason::PathCount,
                hidden_node_count: 0,
                hidden_edge_count: 1,
            }],
            unknown_edges: Vec::new(),
            status: EvidenceStatus::BudgetExceeded,
        };
    }

    if !node_available_for_path(store, query.source) || !node_available_for_path(store, query.sink)
    {
        return InterproceduralPathResult {
            paths: Vec::new(),
            omitted_regions: Vec::new(),
            unknown_edges: Vec::new(),
            status: unavailable_node_status(store, query.source, query.sink),
        };
    }

    let mut queue = VecDeque::from([PathFrame {
        node: query.source,
        nodes: vec![query.source],
        edges: Vec::new(),
        visited: BTreeSet::from([(query.source, PathContext::default())]),
        context: PathContext::default(),
    }]);
    let mut paths = Vec::new();
    let mut omitted_regions = Vec::new();
    let mut unknown_edges = BTreeSet::new();
    let mut expanded_edges = 0usize;

    while let Some(frame) = queue.pop_front() {
        if frame.node == query.sink {
            paths.push(path_from_frame(store, frame));
            if paths.len() >= query.budget.max_paths {
                if !queue.is_empty() {
                    omitted_regions.push(PathOmittedRegion {
                        reason: PathOmittedReason::PathCount,
                        hidden_node_count: 0,
                        hidden_edge_count: queue.len() as u32,
                    });
                }
                break;
            }
            continue;
        }
        if frame.edges.len() >= query.budget.max_depth {
            omitted_regions.push(PathOmittedRegion {
                reason: PathOmittedReason::DepthLimit,
                hidden_node_count: 0,
                hidden_edge_count: 1,
            });
            continue;
        }
        for edge in outgoing_edges(store, frame.node) {
            if edge.status == EvidenceStatus::BudgetExceeded {
                unknown_edges.insert(edge.id);
                omitted_regions.push(PathOmittedRegion {
                    reason: PathOmittedReason::EdgeLimit,
                    hidden_node_count: 0,
                    hidden_edge_count: 1,
                });
                continue;
            }
            if edge.status != EvidenceStatus::Present
                || edge.kind == EvidenceEdgeKind::Unknown
                || !node_available_for_path(store, edge.from)
                || !node_available_for_path(store, edge.to)
            {
                unknown_edges.insert(edge.id);
                continue;
            }
            if !path_edge_allowed(edge.kind, PathMode::SourceToSink) {
                continue;
            }
            if expanded_edges >= query.budget.max_edges {
                omitted_regions.push(PathOmittedRegion {
                    reason: PathOmittedReason::EdgeLimit,
                    hidden_node_count: 0,
                    hidden_edge_count: 1,
                });
                continue;
            }
            expanded_edges += 1;
            let Some(next_context) = transition_context(
                &frame.context,
                edge,
                query.max_interprocedural_depth,
                &mut omitted_regions,
            ) else {
                continue;
            };
            if frame.visited.contains(&(edge.to, next_context.clone())) {
                continue;
            }
            if frame.nodes.len() >= query.budget.max_nodes {
                omitted_regions.push(PathOmittedRegion {
                    reason: PathOmittedReason::NodeLimit,
                    hidden_node_count: 1,
                    hidden_edge_count: 0,
                });
                continue;
            }
            let mut next = frame.clone();
            next.node = edge.to;
            next.nodes.push(edge.to);
            next.edges.push(edge.id);
            next.visited.insert((edge.to, next_context.clone()));
            next.context = next_context;
            queue.push_back(next);
        }
    }

    paths.sort_by(|left, right| {
        compare_scores(left.score, right.score)
            .then_with(|| left.stable_key_text.cmp(&right.stable_key_text))
    });
    let status = if !omitted_regions.is_empty() {
        EvidenceStatus::BudgetExceeded
    } else if !unknown_edges.is_empty() || paths.is_empty() {
        EvidenceStatus::Unknown
    } else {
        EvidenceStatus::Present
    };
    InterproceduralPathResult {
        paths,
        omitted_regions,
        unknown_edges: unknown_edges.into_iter().collect(),
        status,
    }
}

fn node_available_for_path(store: &EvidenceStore, node: EvidenceNodeId) -> bool {
    store
        .node(node)
        .is_some_and(|node| node.status == EvidenceStatus::Present)
}

fn unavailable_node_status(
    store: &EvidenceStore,
    source: EvidenceNodeId,
    sink: EvidenceNodeId,
) -> EvidenceStatus {
    if [source, sink].into_iter().any(|node| {
        store
            .node(node)
            .is_some_and(|node| node.status == EvidenceStatus::BudgetExceeded)
    }) {
        EvidenceStatus::BudgetExceeded
    } else {
        EvidenceStatus::Unknown
    }
}

fn transition_context(
    context: &PathContext,
    edge: &EvidenceEdgeFact,
    max_interprocedural_depth: usize,
    omitted_regions: &mut Vec<PathOmittedRegion>,
) -> Option<PathContext> {
    match edge.kind {
        EvidenceEdgeKind::ParameterIn | EvidenceEdgeKind::Call => {
            let call_site = edge.call_site?;
            if context.call_sites.len() >= max_interprocedural_depth {
                omitted_regions.push(PathOmittedRegion {
                    reason: PathOmittedReason::DepthLimit,
                    hidden_node_count: 0,
                    hidden_edge_count: 1,
                });
                return None;
            }
            let mut next = context.clone();
            next.call_sites.push(call_site);
            Some(next)
        }
        EvidenceEdgeKind::ParameterOut | EvidenceEdgeKind::Return => {
            let call_site = edge.call_site?;
            if context.call_sites.last().copied() != Some(call_site) {
                return None;
            }
            let mut next = context.clone();
            next.call_sites.pop();
            Some(next)
        }
        EvidenceEdgeKind::Summary => Some(context.clone()),
        _ => Some(context.clone()),
    }
}

fn outgoing_edges(store: &EvidenceStore, node: EvidenceNodeId) -> Vec<&EvidenceEdgeFact> {
    let mut edges = store.outgoing(node);
    edges.sort_by(|left, right| {
        store
            .resolve_stable_key(left.stable_key)
            .cmp(&store.resolve_stable_key(right.stable_key))
    });
    edges
}

fn path_from_frame(store: &EvidenceStore, frame: PathFrame) -> InterproceduralPath {
    InterproceduralPath {
        stable_key_text: frame
            .edges
            .iter()
            .map(|edge| {
                store
                    .edge(*edge)
                    .map(|edge| store.resolve_stable_key(edge.stable_key))
                    .unwrap_or_else(|| std::sync::Arc::from("missing"))
            })
            .collect::<Vec<_>>()
            .join(">"),
        score: rank_score_for_edges(store, &frame.edges),
        nodes: frame.nodes,
        edges: frame.edges,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
struct PathContext {
    call_sites: Vec<CallSiteId>,
}

#[derive(Debug, Clone)]
struct PathFrame {
    node: EvidenceNodeId,
    nodes: Vec<EvidenceNodeId>,
    edges: Vec<EvidenceEdgeId>,
    visited: BTreeSet<(EvidenceNodeId, PathContext)>,
    context: PathContext,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::evidence::facts::{
        EvidenceConfidence, EvidenceEdgeFact, EvidenceExpansion, EvidenceNodeFact,
        EvidenceNodeKind, EvidencePrecision, EvidenceProvenance, EvidenceQueryMode,
        EvidenceValidation,
    };
    use crate::analysis::evidence::store::EvidenceOutput;
    use crate::core::Language;

    #[test]
    fn call_site_stack_allows_only_matching_caller_to_reach_sink() {
        let store = context_store();

        let result =
            find_interprocedural_paths(&store, query(EvidenceNodeId(0), EvidenceNodeId(3)));

        assert_eq!(result.paths.len(), 1);
        assert_eq!(result.paths[0].stable_key_text, "edge:a-in>edge:a-out");
    }

    #[test]
    fn mismatched_call_site_return_is_rejected() {
        let store = context_store();

        let result =
            find_interprocedural_paths(&store, query(EvidenceNodeId(0), EvidenceNodeId(5)));

        assert!(result.paths.is_empty());
        assert_eq!(result.status, EvidenceStatus::Unknown);
    }

    #[test]
    fn over_depth_traversal_reports_budget_omission() {
        let store = context_store();
        let mut query = query(EvidenceNodeId(0), EvidenceNodeId(3));
        query.max_interprocedural_depth = 0;

        let result = find_interprocedural_paths(&store, query);

        assert!(result.paths.is_empty());
        assert!(
            result
                .omitted_regions
                .iter()
                .any(|region| region.reason == PathOmittedReason::DepthLimit)
        );
        assert_eq!(result.status, EvidenceStatus::BudgetExceeded);
    }

    #[test]
    fn unresolved_dynamic_calls_are_visible_unknown_edges() {
        let store = unknown_store();

        let result =
            find_interprocedural_paths(&store, query(EvidenceNodeId(0), EvidenceNodeId(1)));

        assert!(result.paths.is_empty());
        assert_eq!(result.unknown_edges, vec![EvidenceEdgeId(0)]);
        assert_eq!(result.status, EvidenceStatus::Unknown);
    }

    #[test]
    fn max_paths_zero_returns_budget_without_paths() {
        let store = context_store();
        let mut query = query(EvidenceNodeId(0), EvidenceNodeId(3));
        query.budget.max_paths = 0;

        let result = find_interprocedural_paths(&store, query);

        assert!(result.paths.is_empty());
        assert_eq!(result.status, EvidenceStatus::BudgetExceeded);
        assert!(
            result
                .omitted_regions
                .iter()
                .any(|region| region.reason == PathOmittedReason::PathCount)
        );
    }

    #[test]
    fn callee_sink_is_reachable_before_returning_to_caller_context() {
        let store = context_store();

        let result =
            find_interprocedural_paths(&store, query(EvidenceNodeId(0), EvidenceNodeId(2)));

        assert_eq!(result.paths.len(), 1);
        assert_eq!(result.paths[0].stable_key_text, "edge:a-in");
        assert_eq!(result.status, EvidenceStatus::Present);
    }

    #[test]
    fn non_present_interprocedural_edge_is_not_materialized_as_path() {
        let store = interprocedural_store_with_edge_status(EvidenceStatus::SetupMissing);

        let result =
            find_interprocedural_paths(&store, query(EvidenceNodeId(0), EvidenceNodeId(1)));

        assert!(result.paths.is_empty());
        assert_eq!(result.unknown_edges, vec![EvidenceEdgeId(0)]);
        assert_eq!(result.status, EvidenceStatus::Unknown);
    }

    #[test]
    fn budget_interprocedural_edge_is_reported_before_unknown_kind_filtering() {
        let store = unknown_store_with_status(EvidenceStatus::BudgetExceeded);

        let result =
            find_interprocedural_paths(&store, query(EvidenceNodeId(0), EvidenceNodeId(1)));

        assert!(result.paths.is_empty());
        assert_eq!(result.unknown_edges, vec![EvidenceEdgeId(0)]);
        assert_eq!(result.status, EvidenceStatus::BudgetExceeded);
        assert!(
            result
                .omitted_regions
                .iter()
                .any(|region| region.reason == PathOmittedReason::EdgeLimit)
        );
    }

    #[test]
    fn control_only_interprocedural_route_is_not_materialized_as_path() {
        let store = EvidenceStore::from_output(
            EvidenceOutput {
                nodes: vec![node(0), node(1)],
                edges: vec![edge(
                    0,
                    0,
                    1,
                    EvidenceEdgeKind::Control,
                    None,
                    "edge:control",
                )],
                bundles: Vec::new(),
                paths: Vec::new(),
                slices: Vec::new(),
                unknowns: Vec::new(),
                omitted_regions: Vec::new(),
                replay_keys: Vec::new(),
            },
            &crate::core::test_stable_key_interner(),
        )
        .expect("valid evidence");

        let result =
            find_interprocedural_paths(&store, query(EvidenceNodeId(0), EvidenceNodeId(1)));

        assert!(result.paths.is_empty());
        assert_eq!(result.status, EvidenceStatus::Unknown);
    }

    #[test]
    fn interprocedural_search_applies_global_edge_expansion_budget() {
        let store = context_store();
        let mut query = query(EvidenceNodeId(0), EvidenceNodeId(3));
        query.budget.max_edges = 1;

        let result = find_interprocedural_paths(&store, query);

        assert!(result.paths.is_empty());
        assert_eq!(result.status, EvidenceStatus::BudgetExceeded);
        assert!(
            result
                .omitted_regions
                .iter()
                .any(|region| region.reason == PathOmittedReason::EdgeLimit)
        );
    }

    fn query(source: EvidenceNodeId, sink: EvidenceNodeId) -> InterproceduralPathQuery {
        InterproceduralPathQuery {
            source,
            sink,
            budget: PathBudget::default(),
            max_interprocedural_depth: 4,
        }
    }

    fn context_store() -> EvidenceStore {
        EvidenceStore::from_output(
            EvidenceOutput {
                nodes: (0..6).map(node).collect(),
                edges: vec![
                    edge(0, 0, 2, EvidenceEdgeKind::ParameterIn, Some(1), "edge:a-in"),
                    edge(
                        1,
                        2,
                        3,
                        EvidenceEdgeKind::ParameterOut,
                        Some(1),
                        "edge:a-out",
                    ),
                    edge(2, 1, 2, EvidenceEdgeKind::ParameterIn, Some(2), "edge:b-in"),
                    edge(
                        3,
                        2,
                        5,
                        EvidenceEdgeKind::ParameterOut,
                        Some(2),
                        "edge:b-out",
                    ),
                    edge(
                        4,
                        2,
                        5,
                        EvidenceEdgeKind::ParameterOut,
                        Some(2),
                        "edge:mismatch",
                    ),
                ],
                bundles: Vec::new(),
                paths: Vec::new(),
                slices: Vec::new(),
                unknowns: Vec::new(),
                omitted_regions: Vec::new(),
                replay_keys: Vec::new(),
            },
            &crate::core::test_stable_key_interner(),
        )
        .expect("valid evidence")
    }

    fn interprocedural_store_with_edge_status(status: EvidenceStatus) -> EvidenceStore {
        let mut edge = edge(
            0,
            0,
            1,
            EvidenceEdgeKind::DataValue,
            None,
            "edge:non-present",
        );
        edge.status = status;
        EvidenceStore::from_output(
            EvidenceOutput {
                nodes: vec![node(0), node(1)],
                edges: vec![edge],
                bundles: Vec::new(),
                paths: Vec::new(),
                slices: Vec::new(),
                unknowns: Vec::new(),
                omitted_regions: Vec::new(),
                replay_keys: Vec::new(),
            },
            &crate::core::test_stable_key_interner(),
        )
        .expect("valid evidence")
    }

    fn unknown_store() -> EvidenceStore {
        unknown_store_with_status(EvidenceStatus::Unknown)
    }

    fn unknown_store_with_status(status: EvidenceStatus) -> EvidenceStore {
        EvidenceStore::from_output(
            EvidenceOutput {
                nodes: vec![node(0), node(1)],
                edges: vec![EvidenceEdgeFact {
                    id: EvidenceEdgeId(0),
                    from: EvidenceNodeId(0),
                    to: EvidenceNodeId(1),
                    kind: EvidenceEdgeKind::Unknown,
                    query_mode: EvidenceQueryMode::Path,
                    status,
                    precision: EvidencePrecision::Unknown,
                    provenance: EvidenceProvenance::Native,
                    validation: EvidenceValidation::Native,
                    confidence: EvidenceConfidence::Low,
                    call_site: Some(CallSiteId(9)),
                    summary_stable_key: None,
                    expansion: EvidenceExpansion::None,
                    compact_label: Some("dynamic_call".to_string()),
                    source_fact_stable_keys: Vec::new(),
                    stable_key: crate::core::stable_key_for_test("edge:unknown-call"),
                }],
                bundles: Vec::new(),
                paths: Vec::new(),
                slices: Vec::new(),
                unknowns: Vec::new(),
                omitted_regions: Vec::new(),
                replay_keys: Vec::new(),
            },
            &crate::core::test_stable_key_interner(),
        )
        .expect("valid evidence")
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
            stable_key: crate::core::stable_key_for_test(&format!("node:{id}")),
        }
    }

    fn edge(
        id: u64,
        from: u64,
        to: u64,
        kind: EvidenceEdgeKind,
        call_site: Option<u64>,
        stable_key: &str,
    ) -> EvidenceEdgeFact {
        EvidenceEdgeFact {
            id: EvidenceEdgeId(id),
            from: EvidenceNodeId(from),
            to: EvidenceNodeId(to),
            kind,
            query_mode: EvidenceQueryMode::Path,
            status: EvidenceStatus::Present,
            precision: EvidencePrecision::Exact,
            provenance: EvidenceProvenance::Native,
            validation: EvidenceValidation::Native,
            confidence: EvidenceConfidence::High,
            call_site: call_site.map(CallSiteId),
            summary_stable_key: None,
            expansion: EvidenceExpansion::None,
            compact_label: None,
            source_fact_stable_keys: Vec::new(),
            stable_key: crate::core::stable_key_for_test(stable_key),
        }
    }
}

use std::collections::{BTreeSet, VecDeque};

use crate::analysis::evidence::facts::{EvidenceEdgeKind, EvidencePrecision, EvidenceStatus};
use crate::analysis::evidence::rank::{PathRankScore, compare_scores, rank_score_for_edges};
use crate::analysis::evidence::store::EvidenceStore;
use crate::analysis::ids::{EvidenceEdgeId, EvidenceNodeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PathQuery {
    pub(crate) source: EvidenceNodeId,
    pub(crate) sink: EvidenceNodeId,
    pub(crate) mode: PathMode,
    pub(crate) budget: PathBudget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PathMode {
    #[allow(
        dead_code,
        reason = "Local path mode is part of the private Phase 39 query contract and is exercised by later bundle rendering plans."
    )]
    Local,
    SourceToSink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PathBudget {
    pub(crate) max_paths: usize,
    pub(crate) max_nodes: usize,
    pub(crate) max_edges: usize,
    pub(crate) max_depth: usize,
}

impl Default for PathBudget {
    fn default() -> Self {
        Self {
            max_paths: 5,
            max_nodes: 64,
            max_edges: 96,
            max_depth: 32,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PathResult {
    pub(crate) paths: Vec<EvidencePath>,
    pub(crate) omitted_regions: Vec<PathOmittedRegion>,
    pub(crate) status: EvidenceStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EvidencePath {
    pub(crate) nodes: Vec<EvidenceNodeId>,
    pub(crate) edges: Vec<EvidenceEdgeId>,
    pub(crate) score: PathRankScore,
    pub(crate) status: EvidenceStatus,
    pub(crate) precision: EvidencePrecision,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PathOmittedRegion {
    pub(crate) reason: PathOmittedReason,
    pub(crate) hidden_node_count: u32,
    pub(crate) hidden_edge_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PathOmittedReason {
    PathCount,
    NodeLimit,
    EdgeLimit,
    DepthLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChopQuery {
    pub(crate) source: EvidenceNodeId,
    pub(crate) sink: EvidenceNodeId,
    pub(crate) budget: PathBudget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChopResult {
    pub(crate) nodes: Vec<EvidenceNodeId>,
    pub(crate) status: EvidenceStatus,
}

pub(crate) fn find_paths(store: &EvidenceStore, query: PathQuery) -> PathResult {
    if query.budget.max_paths == 0 {
        return PathResult {
            paths: Vec::new(),
            omitted_regions: vec![PathOmittedRegion {
                reason: PathOmittedReason::PathCount,
                hidden_node_count: 0,
                hidden_edge_count: 1,
            }],
            status: EvidenceStatus::BudgetExceeded,
        };
    }

    let mut queue = VecDeque::from([PathFrame {
        node: query.source,
        nodes: vec![query.source],
        edges: Vec::new(),
        visited: BTreeSet::from([query.source]),
    }]);
    let mut paths = Vec::new();
    let mut omitted_regions = Vec::new();

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
            if !outgoing_ranked_edges(store, frame.node).is_empty() {
                omitted_regions.push(PathOmittedRegion {
                    reason: PathOmittedReason::DepthLimit,
                    hidden_node_count: 0,
                    hidden_edge_count: 1,
                });
            }
            continue;
        }
        for edge in outgoing_ranked_edges(store, frame.node) {
            if !path_edge_allowed(edge.kind, query.mode) {
                continue;
            }
            let next = edge.to;
            if frame.visited.contains(&next) {
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
            if frame.edges.len() >= query.budget.max_edges {
                omitted_regions.push(PathOmittedRegion {
                    reason: PathOmittedReason::EdgeLimit,
                    hidden_node_count: 0,
                    hidden_edge_count: 1,
                });
                continue;
            }
            let mut next_frame = frame.clone();
            next_frame.node = next;
            next_frame.nodes.push(next);
            next_frame.edges.push(edge.id);
            next_frame.visited.insert(next);
            queue.push_back(next_frame);
        }
    }

    paths.sort_by(|left, right| {
        compare_scores(left.score, right.score).then_with(|| left.stable_key.cmp(&right.stable_key))
    });
    let status = if !omitted_regions.is_empty() {
        EvidenceStatus::BudgetExceeded
    } else if paths.is_empty() {
        EvidenceStatus::Unknown
    } else {
        EvidenceStatus::Present
    };
    PathResult {
        paths,
        omitted_regions,
        status,
    }
}

pub(crate) fn chop(store: &EvidenceStore, query: ChopQuery) -> ChopResult {
    let forward = reachable(store, query.source, Direction::Forward, query.budget);
    let backward = reachable(store, query.sink, Direction::Backward, query.budget);
    let nodes = forward
        .intersection(&backward)
        .copied()
        .collect::<Vec<EvidenceNodeId>>();
    ChopResult {
        status: if nodes.is_empty() {
            EvidenceStatus::Unknown
        } else {
            EvidenceStatus::Present
        },
        nodes,
    }
}

pub(crate) mod summary {
    use crate::analysis::evidence::facts::{
        EvidenceEdgeKind, EvidenceExpansion, EvidencePrecision, EvidenceProvenance, EvidenceStatus,
    };
    use crate::analysis::evidence::store::EvidenceStore;
    use crate::analysis::ids::{EvidenceEdgeId, EvidenceNodeId};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct EvidenceSummaryStep {
        pub(crate) edge: EvidenceEdgeId,
        pub(crate) stable_key: String,
        pub(crate) summary_stable_key: String,
        pub(crate) callable_stable_key: Option<String>,
        pub(crate) domain: Option<String>,
        pub(crate) input_endpoint: EvidenceNodeId,
        pub(crate) output_endpoint: EvidenceNodeId,
        pub(crate) status: EvidenceStatus,
        pub(crate) precision: EvidencePrecision,
        pub(crate) provenance: EvidenceProvenance,
        pub(crate) expansion: EvidenceExpansion,
    }

    #[allow(
        dead_code,
        reason = "Private compressed summary rendering is consumed by Phase 39 bundle renderers."
    )]
    pub(crate) fn compressed_steps(store: &EvidenceStore) -> Vec<EvidenceSummaryStep> {
        let mut steps = store
            .edges()
            .iter()
            .filter_map(|edge| compressed_step_for_edge(store, edge.id))
            .collect::<Vec<_>>();
        steps.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        steps
    }

    pub(crate) fn compressed_step_for_edge(
        store: &EvidenceStore,
        edge_id: EvidenceEdgeId,
    ) -> Option<EvidenceSummaryStep> {
        let edge = store.edge(edge_id)?;
        let summary_stable_key = edge.summary_stable_key.clone()?;
        if edge.kind != EvidenceEdgeKind::Summary
            && !matches!(
                edge.expansion,
                EvidenceExpansion::Opaque { .. }
                    | EvidenceExpansion::Expandable { .. }
                    | EvidenceExpansion::ExternalModel { .. }
            )
        {
            return None;
        }
        Some(EvidenceSummaryStep {
            edge: edge.id,
            stable_key: edge.stable_key.clone(),
            domain: edge
                .source_fact_stable_keys
                .iter()
                .find_map(|key| summary_domain_from_key(key))
                .or_else(|| {
                    edge.compact_label
                        .as_deref()
                        .and_then(summary_domain_from_key)
                }),
            callable_stable_key: edge
                .source_fact_stable_keys
                .iter()
                .find(|key| is_callable_key(key))
                .cloned(),
            summary_stable_key,
            input_endpoint: edge.from,
            output_endpoint: edge.to,
            status: edge.status,
            precision: edge.precision,
            provenance: edge.provenance,
            expansion: edge.expansion.clone(),
        })
    }

    fn is_callable_key(key: &str) -> bool {
        key.starts_with("callable:")
            || key.contains("Function")
            || key.contains("CallTarget")
            || key.contains("RefinedCallEdge")
    }

    fn summary_domain_from_key(key: &str) -> Option<String> {
        if key.contains("data_flow_tito") || key.contains("SummaryTito") {
            Some("data_flow_tito".to_string())
        } else if key.contains("control_effects") || key.contains("SummaryControl") {
            Some("control_effects".to_string())
        } else if key.contains("call_effects") || key.contains("SummaryCall") {
            Some("call_effects".to_string())
        } else if key.contains("memory_effects") || key.contains("SummaryMemory") {
            Some("memory_effects".to_string())
        } else {
            None
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::analysis::evidence::facts::{
            EvidenceConfidence, EvidenceEdgeFact, EvidenceNodeFact, EvidenceNodeKind,
            EvidenceQueryMode, EvidenceValidation,
        };
        use crate::analysis::evidence::store::EvidenceOutput;
        use crate::core::Language;

        #[test]
        fn summary_projected_edge_becomes_one_compressed_step() {
            let store = summary_store(EvidenceExpansion::Expandable {
                key: "evidence:expand:summary:tito".to_string(),
            });

            let steps = compressed_steps(&store);

            assert_eq!(steps.len(), 1);
            assert_eq!(steps[0].summary_stable_key, "summary:tito");
            assert_eq!(steps[0].callable_stable_key.as_deref(), Some("callable:fn"));
            assert_eq!(steps[0].domain.as_deref(), Some("data_flow_tito"));
            assert_eq!(steps[0].input_endpoint, EvidenceNodeId(0));
            assert_eq!(steps[0].output_endpoint, EvidenceNodeId(1));
        }

        #[test]
        fn expandable_summaries_carry_stable_expansion_key() {
            let store = summary_store(EvidenceExpansion::Expandable {
                key: "evidence:expand:summary:tito".to_string(),
            });

            let step = compressed_steps(&store).remove(0);

            assert_eq!(
                step.expansion,
                EvidenceExpansion::Expandable {
                    key: "evidence:expand:summary:tito".to_string()
                }
            );
        }

        #[test]
        fn opaque_summaries_carry_reason_and_unknown_status() {
            let store = summary_store(EvidenceExpansion::Opaque {
                reason: "summary_status=Unknown".to_string(),
            });

            let step = compressed_steps(&store).remove(0);

            assert_eq!(step.status, EvidenceStatus::Unknown);
            assert!(matches!(
                step.expansion,
                EvidenceExpansion::Opaque { ref reason } if !reason.is_empty()
            ));
        }

        fn summary_store(expansion: EvidenceExpansion) -> EvidenceStore {
            let status = if matches!(expansion, EvidenceExpansion::Opaque { .. }) {
                EvidenceStatus::Unknown
            } else {
                EvidenceStatus::Present
            };
            EvidenceStore::from_output(EvidenceOutput {
                nodes: vec![node(0), node(1)],
                edges: vec![EvidenceEdgeFact {
                    id: EvidenceEdgeId(0),
                    from: EvidenceNodeId(0),
                    to: EvidenceNodeId(1),
                    kind: EvidenceEdgeKind::Summary,
                    query_mode: EvidenceQueryMode::Path,
                    status,
                    precision: EvidencePrecision::SetupAware,
                    provenance: EvidenceProvenance::Summary,
                    validation: EvidenceValidation::ReferentiallyValidated,
                    confidence: EvidenceConfidence::Medium,
                    call_site: None,
                    summary_stable_key: Some("summary:tito".to_string()),
                    expansion,
                    compact_label: Some("data_flow_tito".to_string()),
                    source_fact_stable_keys: vec![
                        "summary:tito".to_string(),
                        "callable:fn".to_string(),
                    ],
                    stable_key: "edge:summary".to_string(),
                }],
                bundles: Vec::new(),
                paths: Vec::new(),
                slices: Vec::new(),
                unknowns: Vec::new(),
                omitted_regions: Vec::new(),
                replay_keys: Vec::new(),
            })
            .expect("valid evidence")
        }

        fn node(id: u64) -> EvidenceNodeFact {
            EvidenceNodeFact {
                id: EvidenceNodeId(id),
                kind: EvidenceNodeKind::Summary,
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
                stable_key: format!("node:{id}"),
            }
        }
    }
}

fn path_from_frame(store: &EvidenceStore, frame: PathFrame) -> EvidencePath {
    let score = rank_score_for_edges(store, &frame.edges);
    let precision = if frame.edges.iter().all(|edge| {
        store
            .edge(*edge)
            .is_some_and(|edge| edge.precision == EvidencePrecision::Exact)
    }) {
        EvidencePrecision::Exact
    } else {
        EvidencePrecision::Heuristic
    };
    EvidencePath {
        stable_key: frame
            .edges
            .iter()
            .map(|edge| {
                store
                    .edge(*edge)
                    .map(|edge| edge.stable_key.as_str())
                    .unwrap_or("missing")
            })
            .collect::<Vec<_>>()
            .join(">"),
        nodes: frame.nodes,
        edges: frame.edges,
        score,
        status: EvidenceStatus::Present,
        precision,
    }
}

fn outgoing_ranked_edges(
    store: &EvidenceStore,
    node: EvidenceNodeId,
) -> Vec<&crate::analysis::evidence::facts::EvidenceEdgeFact> {
    let mut edges = store.outgoing(node);
    edges.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
    edges
}

fn path_edge_allowed(kind: EvidenceEdgeKind, _mode: PathMode) -> bool {
    !matches!(kind, EvidenceEdgeKind::ExplanationOnly)
}

fn reachable(
    store: &EvidenceStore,
    start: EvidenceNodeId,
    direction: Direction,
    budget: PathBudget,
) -> BTreeSet<EvidenceNodeId> {
    let mut seen = BTreeSet::from([start]);
    let mut queue = VecDeque::from([(start, 0usize)]);
    while let Some((node, depth)) = queue.pop_front() {
        if seen.len() >= budget.max_nodes || depth >= budget.max_depth {
            continue;
        }
        let edges = match direction {
            Direction::Forward => store.outgoing(node),
            Direction::Backward => store.incoming(node),
        };
        for edge in edges {
            let next = match direction {
                Direction::Forward => edge.to,
                Direction::Backward => edge.from,
            };
            if seen.insert(next) {
                queue.push_back((next, depth + 1));
            }
        }
    }
    seen
}

#[derive(Debug, Clone)]
struct PathFrame {
    node: EvidenceNodeId,
    nodes: Vec<EvidenceNodeId>,
    edges: Vec<EvidenceEdgeId>,
    visited: BTreeSet<EvidenceNodeId>,
}

#[derive(Debug, Clone, Copy)]
enum Direction {
    Forward,
    Backward,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::evidence::facts::{
        EvidenceConfidence, EvidenceEdgeFact, EvidenceExpansion, EvidenceNodeFact,
        EvidenceNodeKind, EvidenceProvenance, EvidenceQueryMode, EvidenceValidation,
    };
    use crate::analysis::evidence::store::EvidenceOutput;
    use crate::core::Language;

    #[test]
    fn path_search_returns_bounded_path_for_direct_flow() {
        let store = path_store();

        let result = find_paths(&store, path_query(4));

        assert_eq!(result.paths[0].stable_key, "edge:direct");
    }

    #[test]
    fn path_search_respects_max_paths_with_deterministic_ordering() {
        let store = path_store();
        let mut query = path_query(4);
        query.budget.max_paths = 1;

        let result = find_paths(&store, query);

        assert_eq!(result.paths.len(), 1);
        assert_eq!(result.paths[0].stable_key, "edge:direct");
        assert_eq!(result.status, EvidenceStatus::BudgetExceeded);
    }

    #[test]
    fn path_search_reports_budget_truncation() {
        let store = path_store();
        let mut query = path_query(4);
        query.budget.max_depth = 0;

        let result = find_paths(&store, query);

        assert!(
            result
                .omitted_regions
                .iter()
                .any(|region| region.reason == PathOmittedReason::DepthLimit)
        );
    }

    #[test]
    fn chop_intersects_forward_and_backward_reachability() {
        let store = path_store();

        let result = chop(
            &store,
            ChopQuery {
                source: EvidenceNodeId(0),
                sink: EvidenceNodeId(3),
                budget: PathBudget::default(),
            },
        );

        assert!(result.nodes.contains(&EvidenceNodeId(1)));
        assert!(!result.nodes.contains(&EvidenceNodeId(4)));
    }

    fn path_query(max_paths: usize) -> PathQuery {
        PathQuery {
            source: EvidenceNodeId(0),
            sink: EvidenceNodeId(3),
            mode: PathMode::SourceToSink,
            budget: PathBudget {
                max_paths,
                ..PathBudget::default()
            },
        }
    }

    fn path_store() -> EvidenceStore {
        EvidenceStore::from_output(EvidenceOutput {
            nodes: (0..5).map(node).collect(),
            edges: vec![
                edge(0, 0, 3, "edge:direct"),
                edge(1, 0, 1, "edge:a"),
                edge(2, 1, 3, "edge:b"),
                edge(3, 0, 4, "edge:unrelated"),
            ],
            bundles: Vec::new(),
            paths: Vec::new(),
            slices: Vec::new(),
            unknowns: Vec::new(),
            omitted_regions: Vec::new(),
            replay_keys: Vec::new(),
        })
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
            stable_key: format!("node:{id}"),
        }
    }

    fn edge(id: u64, from: u64, to: u64, stable_key: &str) -> EvidenceEdgeFact {
        EvidenceEdgeFact {
            id: EvidenceEdgeId(id),
            from: EvidenceNodeId(from),
            to: EvidenceNodeId(to),
            kind: EvidenceEdgeKind::DataValue,
            query_mode: EvidenceQueryMode::Path,
            status: EvidenceStatus::Present,
            precision: EvidencePrecision::Exact,
            provenance: EvidenceProvenance::Native,
            validation: EvidenceValidation::Native,
            confidence: EvidenceConfidence::High,
            call_site: None,
            summary_stable_key: None,
            expansion: EvidenceExpansion::None,
            compact_label: None,
            source_fact_stable_keys: Vec::new(),
            stable_key: stable_key.to_string(),
        }
    }
}

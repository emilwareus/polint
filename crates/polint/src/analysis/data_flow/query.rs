use std::collections::{BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use super::facts::{DataFlowBudgetReason, DataFlowEdgeFact, DataFlowStatus};
use super::store::{DataFlowOutput, DataFlowStore};
use crate::analysis::ids::{DataFlowEdgeId, DataFlowNodeId, DataFlowPathId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DataFlowPath {
    pub(crate) id: DataFlowPathId,
    pub(crate) source: DataFlowNodeId,
    pub(crate) sink: DataFlowNodeId,
    pub(crate) edges: Vec<DataFlowEdgeId>,
    pub(crate) status: DataFlowPathStatus,
    pub(crate) budget: DataFlowSearchBudget,
    pub(crate) budget_reason: Option<DataFlowBudgetReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DataFlowSearchBudget {
    pub(crate) max_depth: usize,
    pub(crate) max_paths: usize,
}

impl Default for DataFlowSearchBudget {
    fn default() -> Self {
        Self {
            max_depth: 32,
            max_paths: 256,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum DataFlowPathStatus {
    Found,
    NotFound,
    Unknown,
    BudgetExceeded,
}

pub(crate) fn find_paths(
    store: &DataFlowStore,
    source: DataFlowNodeId,
    sink: DataFlowNodeId,
    budget: DataFlowSearchBudget,
) -> Vec<DataFlowPath> {
    if budget.max_paths == 0 {
        return vec![status_path(
            DataFlowPathId(0),
            source,
            sink,
            DataFlowPathStatus::BudgetExceeded,
            budget,
            Some(DataFlowBudgetReason::PathCount),
        )];
    }

    let mut paths = Vec::new();
    let mut queue = VecDeque::from([PathFrame {
        node: source,
        edges: Vec::new(),
        visited: BTreeSet::from([source]),
    }]);
    let mut budget_exceeded_reason = None;
    let mut saw_uncertain_edge = false;

    while let Some(frame) = queue.pop_front() {
        if frame.node == sink {
            paths.push(DataFlowPath {
                id: DataFlowPathId(paths.len() as u64),
                source,
                sink,
                edges: frame.edges,
                status: DataFlowPathStatus::Found,
                budget,
                budget_reason: None,
            });
            if paths.len() >= budget.max_paths {
                budget_exceeded_reason =
                    (!queue.is_empty()).then_some(DataFlowBudgetReason::PathCount);
                break;
            }
            continue;
        }
        let edges = outgoing_edges(store, frame.node);
        if frame.edges.len() >= budget.max_depth {
            let mut has_present_continuation = false;
            for edge in edges {
                if frame.visited.contains(&edge.to) {
                    continue;
                }
                match edge.status {
                    DataFlowStatus::BudgetExceeded => {
                        budget_exceeded_reason = Some(DataFlowBudgetReason::EdgeLimit);
                    }
                    DataFlowStatus::Present => has_present_continuation = true,
                    DataFlowStatus::Unknown
                    | DataFlowStatus::Unsupported
                    | DataFlowStatus::SetupMissing
                    | DataFlowStatus::Rejected => saw_uncertain_edge = true,
                }
            }
            if has_present_continuation && budget_exceeded_reason.is_none() {
                budget_exceeded_reason = Some(DataFlowBudgetReason::PathDepth);
            }
            continue;
        }
        for edge in edges {
            if edge.status == DataFlowStatus::BudgetExceeded {
                budget_exceeded_reason = Some(DataFlowBudgetReason::EdgeLimit);
                continue;
            }
            if edge.status != DataFlowStatus::Present {
                saw_uncertain_edge = true;
                continue;
            }
            if frame.visited.contains(&edge.to) {
                continue;
            }
            let mut next_edges = frame.edges.clone();
            next_edges.push(edge.id);
            let mut next_visited = frame.visited.clone();
            next_visited.insert(edge.to);
            queue.push_back(PathFrame {
                node: edge.to,
                edges: next_edges,
                visited: next_visited,
            });
        }
    }

    if paths.is_empty() {
        let status = if budget_exceeded_reason.is_some() {
            DataFlowPathStatus::BudgetExceeded
        } else if saw_uncertain_edge {
            DataFlowPathStatus::Unknown
        } else {
            DataFlowPathStatus::NotFound
        };
        paths.push(status_path(
            DataFlowPathId(0),
            source,
            sink,
            status,
            budget,
            budget_exceeded_reason,
        ));
    } else if let Some(reason) = budget_exceeded_reason {
        paths.push(status_path(
            DataFlowPathId(paths.len() as u64),
            source,
            sink,
            DataFlowPathStatus::BudgetExceeded,
            budget,
            Some(reason),
        ));
    }
    paths
}

pub(crate) fn store_budget_observations_for_paths(
    paths: &[DataFlowPath],
    context: &str,
    output: &mut DataFlowOutput,
) {
    for path in paths {
        if path.status != DataFlowPathStatus::BudgetExceeded {
            continue;
        }
        let reason = path
            .budget_reason
            .unwrap_or(DataFlowBudgetReason::PathCount);
        let (limit, observed) = match reason {
            DataFlowBudgetReason::PathDepth => (
                path.budget.max_depth as u64,
                path.budget.max_depth as u64 + 1,
            ),
            DataFlowBudgetReason::PathCount => (
                path.budget.max_paths as u64,
                path.budget.max_paths as u64 + 1,
            ),
            DataFlowBudgetReason::NodeLimit | DataFlowBudgetReason::EdgeLimit => (
                path.budget.max_paths as u64,
                path.budget.max_paths as u64 + 1,
            ),
        };
        super::local::budget_fact(reason, limit, observed, context, output);
    }
}

fn status_path(
    id: DataFlowPathId,
    source: DataFlowNodeId,
    sink: DataFlowNodeId,
    status: DataFlowPathStatus,
    budget: DataFlowSearchBudget,
    budget_reason: Option<DataFlowBudgetReason>,
) -> DataFlowPath {
    DataFlowPath {
        id,
        source,
        sink,
        edges: Vec::new(),
        status,
        budget,
        budget_reason,
    }
}

fn outgoing_edges(store: &DataFlowStore, node: DataFlowNodeId) -> Vec<&DataFlowEdgeFact> {
    store.outgoing(node)
}

#[derive(Debug, Clone)]
struct PathFrame {
    node: DataFlowNodeId,
    edges: Vec<DataFlowEdgeId>,
    visited: BTreeSet<DataFlowNodeId>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::data_flow::facts::{
        DataFlowAlgorithm, DataFlowConfidence, DataFlowEdgeFact, DataFlowEdgeKind,
        DataFlowNodeFact, DataFlowNodeKind, DataFlowPrecision, DataFlowProvenance,
        DataFlowValidation,
    };
    use crate::analysis::data_flow::store::DataFlowOutput;
    use crate::core::Language;

    #[test]
    fn search_returns_bounded_path_between_nodes() {
        let store = DataFlowStore::from_output(DataFlowOutput {
            nodes: vec![node(0), node(1), node(2)],
            edges: vec![edge(0, 0, 1), edge(1, 1, 2)],
            models: Vec::new(),
            budgets: Vec::new(),
        })
        .expect("valid store");

        let paths = find_paths(
            &store,
            DataFlowNodeId(0),
            DataFlowNodeId(2),
            DataFlowSearchBudget {
                max_depth: 4,
                max_paths: 4,
            },
        );

        assert_eq!(paths[0].status, DataFlowPathStatus::Found);
        assert_eq!(paths[0].edges, vec![DataFlowEdgeId(0), DataFlowEdgeId(1)]);
    }

    #[test]
    fn search_reports_budget_exceeded_when_depth_prevents_exhaustive_answer() {
        let store = DataFlowStore::from_output(DataFlowOutput {
            nodes: vec![node(0), node(1), node(2)],
            edges: vec![edge(0, 0, 1), edge(1, 1, 2)],
            models: Vec::new(),
            budgets: Vec::new(),
        })
        .expect("valid store");

        let paths = find_paths(
            &store,
            DataFlowNodeId(0),
            DataFlowNodeId(2),
            DataFlowSearchBudget {
                max_depth: 1,
                max_paths: 4,
            },
        );

        assert_eq!(paths[0].status, DataFlowPathStatus::BudgetExceeded);
        assert_eq!(
            paths[0].budget_reason,
            Some(DataFlowBudgetReason::PathDepth)
        );
    }

    #[test]
    fn search_reports_unknown_when_only_route_is_non_present() {
        let mut uncertain = edge(0, 0, 1);
        uncertain.status = DataFlowStatus::Unknown;
        let store = DataFlowStore::from_output(DataFlowOutput {
            nodes: vec![node(0), node(1)],
            edges: vec![uncertain],
            models: Vec::new(),
            budgets: Vec::new(),
        })
        .expect("valid store");

        let paths = find_paths(
            &store,
            DataFlowNodeId(0),
            DataFlowNodeId(1),
            DataFlowSearchBudget {
                max_depth: 4,
                max_paths: 4,
            },
        );

        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].status, DataFlowPathStatus::Unknown);
        assert!(paths[0].edges.is_empty());
    }

    #[test]
    fn search_reports_unknown_when_depth_reaches_uncertain_continuation() {
        let mut uncertain = edge(1, 1, 2);
        uncertain.status = DataFlowStatus::Unknown;
        let store = DataFlowStore::from_output(DataFlowOutput {
            nodes: vec![node(0), node(1), node(2)],
            edges: vec![edge(0, 0, 1), uncertain],
            models: Vec::new(),
            budgets: Vec::new(),
        })
        .expect("valid store");

        let paths = find_paths(
            &store,
            DataFlowNodeId(0),
            DataFlowNodeId(2),
            DataFlowSearchBudget {
                max_depth: 1,
                max_paths: 4,
            },
        );

        assert_eq!(paths[0].status, DataFlowPathStatus::Unknown);
        assert_eq!(paths[0].budget_reason, None);
    }

    #[test]
    fn search_reports_edge_limit_when_depth_reaches_budget_continuation() {
        let mut budget_edge = edge(1, 1, 2);
        budget_edge.status = DataFlowStatus::BudgetExceeded;
        let store = DataFlowStore::from_output(DataFlowOutput {
            nodes: vec![node(0), node(1), node(2)],
            edges: vec![edge(0, 0, 1), budget_edge],
            models: Vec::new(),
            budgets: Vec::new(),
        })
        .expect("valid store");

        let paths = find_paths(
            &store,
            DataFlowNodeId(0),
            DataFlowNodeId(2),
            DataFlowSearchBudget {
                max_depth: 1,
                max_paths: 4,
            },
        );

        assert_eq!(paths[0].status, DataFlowPathStatus::BudgetExceeded);
        assert_eq!(
            paths[0].budget_reason,
            Some(DataFlowBudgetReason::EdgeLimit)
        );
    }

    #[test]
    fn search_reports_budget_exceeded_when_path_count_cap_is_hit() {
        let store = DataFlowStore::from_output(DataFlowOutput {
            nodes: vec![node(0), node(1), node(2), node(3)],
            edges: vec![edge(0, 0, 1), edge(1, 1, 3), edge(2, 0, 2), edge(3, 2, 3)],
            models: Vec::new(),
            budgets: Vec::new(),
        })
        .expect("valid store");

        let paths = find_paths(
            &store,
            DataFlowNodeId(0),
            DataFlowNodeId(3),
            DataFlowSearchBudget {
                max_depth: 4,
                max_paths: 1,
            },
        );

        assert_eq!(
            paths.last().map(|path| path.status),
            Some(DataFlowPathStatus::BudgetExceeded)
        );
    }

    #[test]
    fn budget_exceeded_paths_convert_to_stored_budget_rows() {
        let mut output = DataFlowOutput {
            nodes: vec![node(0), node(1), node(2)],
            edges: vec![edge(0, 0, 1), edge(1, 1, 2)],
            models: Vec::new(),
            budgets: Vec::new(),
        };
        let store = DataFlowStore::from_output(output.clone()).expect("valid store");
        let paths = find_paths(
            &store,
            DataFlowNodeId(0),
            DataFlowNodeId(2),
            DataFlowSearchBudget {
                max_depth: 1,
                max_paths: 4,
            },
        );

        store_budget_observations_for_paths(&paths, "test-query", &mut output);

        assert_eq!(output.budgets.len(), 1);
        assert_eq!(output.budgets[0].status, DataFlowStatus::BudgetExceeded);
        assert!(output.budgets[0].stable_key.contains("test-query"));
    }

    #[test]
    fn budget_observation_uses_explicit_query_budget_reason() {
        let mut output = DataFlowOutput {
            nodes: vec![node(0), node(1), node(2)],
            edges: vec![edge(0, 0, 1), edge(1, 1, 2)],
            models: Vec::new(),
            budgets: Vec::new(),
        };
        let store = DataFlowStore::from_output(output.clone()).expect("valid store");
        let paths = find_paths(
            &store,
            DataFlowNodeId(0),
            DataFlowNodeId(2),
            DataFlowSearchBudget {
                max_depth: 1,
                max_paths: 4,
            },
        );

        store_budget_observations_for_paths(&paths, "depth-query", &mut output);

        assert_eq!(output.budgets[0].reason, DataFlowBudgetReason::PathDepth);
        assert!(
            output.budgets[0].observed > output.budgets[0].limit,
            "path-depth budgets must record observed > limit"
        );
    }

    fn node(id: u64) -> DataFlowNodeFact {
        DataFlowNodeFact {
            id: DataFlowNodeId(id),
            kind: DataFlowNodeKind::Synthetic,
            language: Language::Unknown,
            file: None,
            function: None,
            body: None,
            operation: None,
            cfg_node: None,
            place: None,
            symbol: None,
            reference: None,
            call_site: None,
            model: None,
            span: None,
            stable_key: format!("node:{id}"),
        }
    }

    fn edge(id: u64, from: u64, to: u64) -> DataFlowEdgeFact {
        DataFlowEdgeFact {
            id: DataFlowEdgeId(id),
            from: DataFlowNodeId(from),
            to: DataFlowNodeId(to),
            kind: DataFlowEdgeKind::LocalUse,
            algorithm: DataFlowAlgorithm::QuerySearch,
            status: DataFlowStatus::Present,
            precision: DataFlowPrecision::Syntax,
            validation: DataFlowValidation::Native,
            confidence: DataFlowConfidence::High,
            provenance: DataFlowProvenance::Query,
            call_site: None,
            call_target: None,
            refined_call: None,
            model: None,
            budget: None,
            evidence: Vec::new(),
            input_stable_keys: Vec::new(),
            stable_key: format!("edge:{id}"),
        }
    }
}

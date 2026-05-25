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
        )];
    }

    let mut paths = Vec::new();
    let mut queue = VecDeque::from([PathFrame {
        node: source,
        edges: Vec::new(),
        visited: BTreeSet::from([source]),
    }]);
    let mut budget_exceeded = false;

    while let Some(frame) = queue.pop_front() {
        if frame.node == sink {
            paths.push(DataFlowPath {
                id: DataFlowPathId(paths.len() as u64),
                source,
                sink,
                edges: frame.edges,
                status: DataFlowPathStatus::Found,
                budget,
            });
            if paths.len() >= budget.max_paths {
                budget_exceeded = !queue.is_empty();
                break;
            }
            continue;
        }
        let edges = traversable_edges(store, frame.node);
        if frame.edges.len() >= budget.max_depth {
            if edges.iter().any(|edge| !frame.visited.contains(&edge.to)) {
                budget_exceeded = true;
            }
            continue;
        }
        for edge in edges {
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
        let status = if budget_exceeded {
            DataFlowPathStatus::BudgetExceeded
        } else {
            DataFlowPathStatus::NotFound
        };
        paths.push(status_path(DataFlowPathId(0), source, sink, status, budget));
    } else if budget_exceeded {
        paths.push(status_path(
            DataFlowPathId(paths.len() as u64),
            source,
            sink,
            DataFlowPathStatus::BudgetExceeded,
            budget,
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
        let (reason, limit, observed) =
            if path.budget.max_depth == 0 || path.edges.len() >= path.budget.max_depth {
                (
                    DataFlowBudgetReason::PathDepth,
                    path.budget.max_depth as u64,
                    path.edges.len() as u64 + 1,
                )
            } else {
                (
                    DataFlowBudgetReason::PathCount,
                    path.budget.max_paths as u64,
                    path.budget.max_paths as u64 + 1,
                )
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
) -> DataFlowPath {
    DataFlowPath {
        id,
        source,
        sink,
        edges: Vec::new(),
        status,
        budget,
    }
}

fn traversable_edges(store: &DataFlowStore, node: DataFlowNodeId) -> Vec<&DataFlowEdgeFact> {
    store
        .outgoing(node)
        .into_iter()
        .filter(|edge| edge.status == DataFlowStatus::Present)
        .collect()
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

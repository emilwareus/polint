use std::collections::{BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use super::facts::{DataFlowEdgeFact, DataFlowStatus};
use super::store::DataFlowStore;
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
    let mut paths = Vec::new();
    let mut queue = VecDeque::from([PathFrame {
        node: source,
        edges: Vec::new(),
        visited: BTreeSet::from([source]),
    }]);

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
                break;
            }
            continue;
        }
        if frame.edges.len() >= budget.max_depth {
            continue;
        }
        for edge in traversable_edges(store, frame.node) {
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
        paths.push(DataFlowPath {
            id: DataFlowPathId(0),
            source,
            sink,
            edges: Vec::new(),
            status: DataFlowPathStatus::NotFound,
            budget,
        });
    }
    paths
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

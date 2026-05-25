use std::collections::{BTreeMap, BTreeSet};

use super::facts::{
    DataFlowBudgetFact, DataFlowEdgeFact, DataFlowEdgeKind, DataFlowModelFact, DataFlowNodeFact,
    DataFlowNodeKind, DataFlowProvenance, DataFlowStatus,
};
use crate::analysis::error::AnalysisError;
use crate::analysis::ids::{
    CallSiteId, DataFlowBudgetId, DataFlowEdgeId, DataFlowModelId, DataFlowNodeId, PlaceId,
};
use crate::core::FunctionId;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DataFlowOutput {
    pub(crate) nodes: Vec<DataFlowNodeFact>,
    pub(crate) edges: Vec<DataFlowEdgeFact>,
    pub(crate) models: Vec<DataFlowModelFact>,
    pub(crate) budgets: Vec<DataFlowBudgetFact>,
}

impl DataFlowOutput {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    pub(crate) fn normalized(mut self) -> Self {
        self.nodes.sort_by(|left, right| {
            (left.stable_key.as_str(), left.kind, left.id).cmp(&(
                right.stable_key.as_str(),
                right.kind,
                right.id,
            ))
        });
        self.models = self
            .models
            .into_iter()
            .map(DataFlowModelFact::normalized)
            .collect();
        self.models.sort_by(|left, right| {
            (left.stable_key.as_str(), left.kind, left.id).cmp(&(
                right.stable_key.as_str(),
                right.kind,
                right.id,
            ))
        });
        self.budgets.sort_by(|left, right| {
            (left.stable_key.as_str(), left.reason, left.id).cmp(&(
                right.stable_key.as_str(),
                right.reason,
                right.id,
            ))
        });

        let node_remap = self
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| (node.id, DataFlowNodeId(index as u64)))
            .collect::<BTreeMap<_, _>>();
        let model_remap = self
            .models
            .iter()
            .enumerate()
            .map(|(index, model)| (model.id, DataFlowModelId(index as u64)))
            .collect::<BTreeMap<_, _>>();
        let budget_remap = self
            .budgets
            .iter()
            .enumerate()
            .map(|(index, budget)| (budget.id, DataFlowBudgetId(index as u64)))
            .collect::<BTreeMap<_, _>>();

        for (index, node) in self.nodes.iter_mut().enumerate() {
            node.id = DataFlowNodeId(index as u64);
            if let Some(model) = node.model.and_then(|id| model_remap.get(&id).copied()) {
                node.model = Some(model);
            }
        }
        for (index, model) in self.models.iter_mut().enumerate() {
            model.id = DataFlowModelId(index as u64);
        }
        for (index, budget) in self.budgets.iter_mut().enumerate() {
            budget.id = DataFlowBudgetId(index as u64);
        }

        self.edges = self
            .edges
            .into_iter()
            .map(DataFlowEdgeFact::normalized)
            .map(|mut edge| {
                if let Some(remapped) = node_remap.get(&edge.from) {
                    edge.from = *remapped;
                }
                if let Some(remapped) = node_remap.get(&edge.to) {
                    edge.to = *remapped;
                }
                if let Some(model) = edge.model.and_then(|id| model_remap.get(&id).copied()) {
                    edge.model = Some(model);
                }
                if let Some(budget) = edge.budget.and_then(|id| budget_remap.get(&id).copied()) {
                    edge.budget = Some(budget);
                }
                edge
            })
            .collect();
        self.edges.sort_by(|left, right| {
            (
                left.stable_key.as_str(),
                left.from,
                left.to,
                left.kind,
                left.algorithm,
                left.id,
            )
                .cmp(&(
                    right.stable_key.as_str(),
                    right.from,
                    right.to,
                    right.kind,
                    right.algorithm,
                    right.id,
                ))
        });
        for (index, edge) in self.edges.iter_mut().enumerate() {
            edge.id = DataFlowEdgeId(index as u64);
        }
        self
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct DataFlowStore {
    output: DataFlowOutput,
    by_node_kind: BTreeMap<DataFlowNodeKind, Vec<usize>>,
    by_edge_kind: BTreeMap<DataFlowEdgeKind, Vec<usize>>,
    by_status: BTreeMap<DataFlowStatus, Vec<usize>>,
    by_provenance: BTreeMap<DataFlowProvenance, Vec<usize>>,
    by_place: BTreeMap<PlaceId, Vec<usize>>,
    by_call_site: BTreeMap<CallSiteId, Vec<usize>>,
    by_function: BTreeMap<FunctionId, Vec<usize>>,
    outgoing: BTreeMap<DataFlowNodeId, Vec<usize>>,
    incoming: BTreeMap<DataFlowNodeId, Vec<usize>>,
}

impl DataFlowStore {
    pub(crate) fn from_output(output: DataFlowOutput) -> Result<Self, AnalysisError> {
        let output = output.normalized();
        reject_duplicates(
            output.nodes.iter().map(|node| node.stable_key.as_str()),
            "data-flow node stable key",
        )?;
        reject_duplicates(
            output.edges.iter().map(|edge| edge.stable_key.as_str()),
            "data-flow edge stable key",
        )?;
        reject_duplicates(
            output.models.iter().map(|model| model.stable_key.as_str()),
            "data-flow model stable key",
        )?;
        reject_duplicates(
            output
                .budgets
                .iter()
                .map(|budget| budget.stable_key.as_str()),
            "data-flow budget stable key",
        )?;

        let nodes = output
            .nodes
            .iter()
            .map(|node| node.id)
            .collect::<BTreeSet<_>>();
        for edge in &output.edges {
            if !nodes.contains(&edge.from) || !nodes.contains(&edge.to) {
                return Err(AnalysisError::InvalidFact {
                    provider: "polint.data_flow",
                    reason: format!(
                        "data-flow edge `{}` references missing endpoint",
                        edge.stable_key
                    ),
                });
            }
        }

        let mut store = Self {
            output,
            ..Self::default()
        };
        for (index, node) in store.output.nodes.iter().enumerate() {
            store.by_node_kind.entry(node.kind).or_default().push(index);
            if let Some(place) = node.place {
                store.by_place.entry(place).or_default().push(index);
            }
            if let Some(call_site) = node.call_site {
                store.by_call_site.entry(call_site).or_default().push(index);
            }
            if let Some(function) = node.function {
                store.by_function.entry(function).or_default().push(index);
            }
        }
        for (index, edge) in store.output.edges.iter().enumerate() {
            store.by_edge_kind.entry(edge.kind).or_default().push(index);
            store.by_status.entry(edge.status).or_default().push(index);
            store
                .by_provenance
                .entry(edge.provenance)
                .or_default()
                .push(index);
            store.outgoing.entry(edge.from).or_default().push(index);
            store.incoming.entry(edge.to).or_default().push(index);
        }
        Ok(store)
    }

    pub(crate) fn nodes(&self) -> &[DataFlowNodeFact] {
        &self.output.nodes
    }

    pub(crate) fn edges(&self) -> &[DataFlowEdgeFact] {
        &self.output.edges
    }

    pub(crate) fn models(&self) -> &[DataFlowModelFact] {
        &self.output.models
    }

    pub(crate) fn budgets(&self) -> &[DataFlowBudgetFact] {
        &self.output.budgets
    }

    pub(crate) fn outgoing(&self, node: DataFlowNodeId) -> Vec<&DataFlowEdgeFact> {
        self.edge_refs(self.outgoing.get(&node))
    }

    #[allow(
        dead_code,
        reason = "Private query indexes are exposed to later data-flow plan slices."
    )]
    pub(crate) fn by_edge_kind(&self, kind: DataFlowEdgeKind) -> Vec<&DataFlowEdgeFact> {
        self.edge_refs(self.by_edge_kind.get(&kind))
    }

    fn edge_refs(&self, indexes: Option<&Vec<usize>>) -> Vec<&DataFlowEdgeFact> {
        indexes.map_or_else(Vec::new, |indexes| {
            indexes
                .iter()
                .map(|index| &self.output.edges[*index])
                .collect()
        })
    }
}

fn reject_duplicates<'a>(
    keys: impl Iterator<Item = &'a str>,
    label: &'static str,
) -> Result<(), AnalysisError> {
    let mut seen = BTreeSet::new();
    for key in keys {
        if !seen.insert(key.to_string()) {
            return Err(AnalysisError::InvalidFact {
                provider: "polint.data_flow",
                reason: format!("duplicate {label} `{key}`"),
            });
        }
    }
    Ok(())
}

pub(crate) fn next_data_flow_node_id(nodes: &[DataFlowNodeFact]) -> DataFlowNodeId {
    DataFlowNodeId(nodes.len() as u64)
}

pub(crate) fn next_data_flow_edge_id(edges: &[DataFlowEdgeFact]) -> DataFlowEdgeId {
    DataFlowEdgeId(edges.len() as u64)
}

pub(crate) fn next_data_flow_model_id(models: &[DataFlowModelFact]) -> DataFlowModelId {
    DataFlowModelId(models.len() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::data_flow::facts::{
        DataFlowAlgorithm, DataFlowConfidence, DataFlowPrecision, DataFlowProvenance,
        DataFlowValidation,
    };
    use crate::core::Language;

    #[test]
    fn output_sorts_and_reassigns_ids_with_edge_endpoint_remap() {
        let output = DataFlowOutput {
            nodes: vec![node(9, "node:z"), node(3, "node:a")],
            edges: vec![edge(7, 9, 3, "edge:z_to_a")],
            models: Vec::new(),
            budgets: Vec::new(),
        }
        .normalized();

        assert_eq!(output.nodes[0].stable_key, "node:a");
        assert_eq!(output.nodes[0].id, DataFlowNodeId(0));
        assert_eq!(output.edges[0].from, DataFlowNodeId(1));
        assert_eq!(output.edges[0].to, DataFlowNodeId(0));
        assert_eq!(output.edges[0].id, DataFlowEdgeId(0));
    }

    #[test]
    fn store_rejects_dangling_edge_endpoints() {
        let result = DataFlowStore::from_output(DataFlowOutput {
            nodes: vec![node(0, "node:a")],
            edges: vec![edge(0, 0, 4, "edge:dangling")],
            models: Vec::new(),
            budgets: Vec::new(),
        });

        assert!(result.is_err());
    }

    fn node(id: u64, stable_key: &str) -> DataFlowNodeFact {
        DataFlowNodeFact {
            id: DataFlowNodeId(id),
            kind: DataFlowNodeKind::Place,
            language: Language::TypeScript,
            file: None,
            function: None,
            body: None,
            operation: None,
            cfg_node: None,
            place: Some(PlaceId(id)),
            symbol: None,
            reference: None,
            call_site: None,
            model: None,
            span: None,
            stable_key: stable_key.to_string(),
        }
    }

    fn edge(id: u64, from: u64, to: u64, stable_key: &str) -> DataFlowEdgeFact {
        DataFlowEdgeFact {
            id: DataFlowEdgeId(id),
            from: DataFlowNodeId(from),
            to: DataFlowNodeId(to),
            kind: DataFlowEdgeKind::LocalAssignment,
            algorithm: DataFlowAlgorithm::LocalMir,
            status: DataFlowStatus::Present,
            precision: DataFlowPrecision::Syntax,
            validation: DataFlowValidation::Native,
            confidence: DataFlowConfidence::High,
            provenance: DataFlowProvenance::Native,
            call_site: None,
            call_target: None,
            refined_call: None,
            model: None,
            budget: None,
            evidence: Vec::new(),
            input_stable_keys: Vec::new(),
            stable_key: stable_key.to_string(),
        }
    }
}

#![cfg(test)]

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use super::facts::{DataFlowEdgeKind, DataFlowStatus};
use crate::core::AnalysisDb;

pub(crate) fn data_flow_debug_json_for_test(db: &AnalysisDb) -> Value {
    let mut edge_counts_by_kind = BTreeMap::<String, usize>::new();
    let mut edge_counts_by_status = BTreeMap::<String, usize>::new();
    for edge in db.data_flow_edges() {
        *edge_counts_by_kind
            .entry(format!("{:?}", edge.kind))
            .or_default() += 1;
        *edge_counts_by_status
            .entry(format!("{:?}", edge.status))
            .or_default() += 1;
    }

    let report = DataFlowDebugReport {
        nodes: db
            .data_flow_nodes()
            .iter()
            .map(|node| DataFlowNodeDebugRow {
                family: "DataFlowNode",
                stable_key: node.stable_key.clone(),
                producer_id: "polint.data_flow",
                kind: format!("{:?}", node.kind),
                status: "present".to_string(),
                precision: "syntax".to_string(),
                path: node
                    .file
                    .and_then(|file| db.file(file))
                    .map(|file| file.relative_path.clone()),
            })
            .collect(),
        edges: db
            .data_flow_edges()
            .iter()
            .map(|edge| DataFlowEdgeDebugRow {
                family: "DataFlowEdge",
                stable_key: edge.stable_key.clone(),
                producer_id: "polint.data_flow",
                kind: format!("{:?}", edge.kind),
                algorithm: format!("{:?}", edge.algorithm),
                status: format!("{:?}", edge.status),
                precision: format!("{:?}", edge.precision),
                provenance: format!("{:?}", edge.provenance),
                evidence_count: edge.evidence.len(),
                input_count: edge.input_stable_keys.len(),
                budget: edge.budget.map(|budget| budget.0),
            })
            .collect(),
        models: db
            .data_flow_models()
            .iter()
            .map(|model| DataFlowModelDebugRow {
                family: "DataFlowModel",
                stable_key: model.stable_key.clone(),
                producer_id: "polint.data_flow",
                kind: format!("{:?}", model.kind),
                status: format!("{:?}", model.status),
                precision: format!("{:?}", model.precision),
                provenance: format!("{:?}", model.provenance),
            })
            .collect(),
        budgets: db
            .data_flow_budgets()
            .iter()
            .map(|budget| DataFlowBudgetDebugRow {
                family: "DataFlowBudget",
                stable_key: budget.stable_key.clone(),
                producer_id: "polint.data_flow",
                reason: format!("{:?}", budget.reason),
                limit: budget.limit,
                observed: budget.observed,
                status: format!("{:?}", budget.status),
            })
            .collect(),
        counts: DataFlowDebugCounts {
            nodes: db.data_flow_nodes().len(),
            edges: db.data_flow_edges().len(),
            models: db.data_flow_models().len(),
            budgets: db.data_flow_budgets().len(),
            local_edges: db
                .data_flow_edges()
                .iter()
                .filter(|edge| edge.algorithm == super::facts::DataFlowAlgorithm::LocalMir)
                .count(),
            direct_call_edges: db
                .data_flow_edges()
                .iter()
                .filter(|edge| edge.algorithm == super::facts::DataFlowAlgorithm::DirectCall)
                .count(),
            summary_projected_edges: db
                .data_flow_edges()
                .iter()
                .filter(|edge| edge.algorithm == super::facts::DataFlowAlgorithm::SummaryProjection)
                .count(),
            unknown_or_havoc_edges: db
                .data_flow_edges()
                .iter()
                .filter(|edge| {
                    matches!(
                        edge.kind,
                        DataFlowEdgeKind::UnknownFlow | DataFlowEdgeKind::HavocFlow
                    ) || matches!(
                        edge.status,
                        DataFlowStatus::Unknown
                            | DataFlowStatus::Unsupported
                            | DataFlowStatus::SetupMissing
                    )
                })
                .count(),
            budget_rows: db.data_flow_budgets().len(),
            by_kind: edge_counts_by_kind,
            by_status: edge_counts_by_status,
        },
    };
    serde_json::to_value(report).expect("data-flow debug report should serialize")
}

#[derive(Serialize)]
struct DataFlowDebugReport {
    nodes: Vec<DataFlowNodeDebugRow>,
    edges: Vec<DataFlowEdgeDebugRow>,
    models: Vec<DataFlowModelDebugRow>,
    budgets: Vec<DataFlowBudgetDebugRow>,
    counts: DataFlowDebugCounts,
}

#[derive(Serialize)]
struct DataFlowNodeDebugRow {
    family: &'static str,
    stable_key: String,
    producer_id: &'static str,
    kind: String,
    status: String,
    precision: String,
    path: Option<String>,
}

#[derive(Serialize)]
struct DataFlowEdgeDebugRow {
    family: &'static str,
    stable_key: String,
    producer_id: &'static str,
    kind: String,
    algorithm: String,
    status: String,
    precision: String,
    provenance: String,
    evidence_count: usize,
    input_count: usize,
    budget: Option<u64>,
}

#[derive(Serialize)]
struct DataFlowModelDebugRow {
    family: &'static str,
    stable_key: String,
    producer_id: &'static str,
    kind: String,
    status: String,
    precision: String,
    provenance: String,
}

#[derive(Serialize)]
struct DataFlowBudgetDebugRow {
    family: &'static str,
    stable_key: String,
    producer_id: &'static str,
    reason: String,
    limit: u64,
    observed: u64,
    status: String,
}

#[derive(Serialize)]
struct DataFlowDebugCounts {
    nodes: usize,
    edges: usize,
    models: usize,
    budgets: usize,
    local_edges: usize,
    direct_call_edges: usize,
    summary_projected_edges: usize,
    unknown_or_havoc_edges: usize,
    budget_rows: usize,
    by_kind: BTreeMap<String, usize>,
    by_status: BTreeMap<String, usize>,
}

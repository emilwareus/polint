use std::collections::BTreeSet;

use super::facts::{DataFlowEdgeKind, DataFlowStatus, DataFlowValidation};
use super::store::DataFlowOutput;

#[allow(
    dead_code,
    reason = "Validation rows are used by the debug/eval surface in later data-flow slices."
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DataFlowValidationIssue {
    pub(crate) stable_key_text: String,
    pub(crate) reason: String,
}

#[allow(
    dead_code,
    reason = "Validation is kept as a reusable private hook before it is surfaced in debug output."
)]
pub(crate) fn validate_output(
    output: &DataFlowOutput,
    interner: &crate::core::StableKeyInterner,
) -> Vec<DataFlowValidationIssue> {
    let mut issues = Vec::new();
    let nodes = output
        .nodes
        .iter()
        .map(|node| node.id)
        .collect::<BTreeSet<_>>();
    let models = output
        .models
        .iter()
        .map(|model| model.id)
        .collect::<BTreeSet<_>>();
    let budgets = output
        .budgets
        .iter()
        .map(|budget| budget.id)
        .collect::<BTreeSet<_>>();
    let mut stable_keys = BTreeSet::new();

    for node in &output.nodes {
        if !stable_keys.insert(("node", node.stable_key)) {
            issues.push(DataFlowValidationIssue {
                stable_key_text: interner.resolve(node.stable_key).to_string(),
                reason: "duplicate node stable key".to_string(),
            });
        }
    }
    for edge in &output.edges {
        if !stable_keys.insert(("edge", edge.stable_key)) {
            issues.push(DataFlowValidationIssue {
                stable_key_text: interner.resolve(edge.stable_key).to_string(),
                reason: "duplicate edge stable key".to_string(),
            });
        }
        if !nodes.contains(&edge.from) || !nodes.contains(&edge.to) {
            issues.push(DataFlowValidationIssue {
                stable_key_text: interner.resolve(edge.stable_key).to_string(),
                reason: "dangling edge endpoint".to_string(),
            });
        }
        if edge.provenance == super::facts::DataFlowProvenance::Summary
            && edge
                .input_stable_keys
                .iter()
                .all(|key| key.trim().is_empty())
        {
            issues.push(DataFlowValidationIssue {
                stable_key_text: interner.resolve(edge.stable_key).to_string(),
                reason: "summary-projected edge missing summary stable-key evidence".to_string(),
            });
        }
        if edge.status == DataFlowStatus::BudgetExceeded && edge.budget.is_none() {
            issues.push(DataFlowValidationIssue {
                stable_key_text: interner.resolve(edge.stable_key).to_string(),
                reason: "budget-exceeded edge missing budget row".to_string(),
            });
        }
        if let Some(budget) = edge.budget
            && !budgets.contains(&budget)
        {
            issues.push(DataFlowValidationIssue {
                stable_key_text: interner.resolve(edge.stable_key).to_string(),
                reason: "edge references missing budget row".to_string(),
            });
        }
        if matches!(
            edge.kind,
            DataFlowEdgeKind::UnknownFlow | DataFlowEdgeKind::HavocFlow
        ) && edge.evidence.is_empty()
        {
            issues.push(DataFlowValidationIssue {
                stable_key_text: interner.resolve(edge.stable_key).to_string(),
                reason: "uncertainty edge missing evidence".to_string(),
            });
        }
    }
    for model in &output.models {
        if model.status == DataFlowStatus::Present
            && model.validation == DataFlowValidation::Rejected
        {
            issues.push(DataFlowValidationIssue {
                stable_key_text: interner.resolve(model.stable_key).to_string(),
                reason: "present model cannot be rejected".to_string(),
            });
        }
    }
    for node in &output.nodes {
        if let Some(model) = node.model
            && !models.contains(&model)
        {
            issues.push(DataFlowValidationIssue {
                stable_key_text: interner.resolve(node.stable_key).to_string(),
                reason: "node references missing model".to_string(),
            });
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::data_flow::facts::{
        DataFlowAlgorithm, DataFlowConfidence, DataFlowEdgeFact, DataFlowNodeFact,
        DataFlowNodeKind, DataFlowPrecision, DataFlowProvenance, DataFlowValidation,
    };
    use crate::analysis::ids::{DataFlowEdgeId, DataFlowNodeId};
    use crate::core::Language;

    #[test]
    fn validation_rejects_budget_exceeded_edge_without_budget_reference() {
        let mut edge = edge(0, 0, 1, "edge:budget");
        edge.status = DataFlowStatus::BudgetExceeded;
        edge.kind = DataFlowEdgeKind::BudgetTruncated;

        let issues = validate_output(
            &DataFlowOutput {
                nodes: vec![node(0), node(1)],
                edges: vec![edge],
                models: Vec::new(),
                budgets: Vec::new(),
            },
            &crate::core::test_stable_key_interner(),
        );

        assert!(
            issues
                .iter()
                .any(|issue| issue.reason.contains("missing budget row"))
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
            stable_key: crate::core::stable_key_for_test(&format!("node:{id}")),
        }
    }

    fn edge(id: u64, from: u64, to: u64, stable_key: &str) -> DataFlowEdgeFact {
        DataFlowEdgeFact {
            id: DataFlowEdgeId(id),
            from: DataFlowNodeId(from),
            to: DataFlowNodeId(to),
            kind: DataFlowEdgeKind::LocalUse,
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
            stable_key: crate::core::stable_key_for_test(stable_key),
        }
    }
}

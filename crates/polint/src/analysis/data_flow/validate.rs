use std::collections::BTreeSet;

use super::store::DataFlowOutput;

#[allow(
    dead_code,
    reason = "Validation rows are used by the debug/eval surface in later data-flow slices."
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DataFlowValidationIssue {
    pub(crate) stable_key: String,
    pub(crate) reason: String,
}

#[allow(
    dead_code,
    reason = "Validation is kept as a reusable private hook before it is surfaced in debug output."
)]
pub(crate) fn validate_output(output: &DataFlowOutput) -> Vec<DataFlowValidationIssue> {
    let mut issues = Vec::new();
    let nodes = output
        .nodes
        .iter()
        .map(|node| node.id)
        .collect::<BTreeSet<_>>();
    let mut stable_keys = BTreeSet::new();

    for node in &output.nodes {
        if !stable_keys.insert(("node", node.stable_key.clone())) {
            issues.push(DataFlowValidationIssue {
                stable_key: node.stable_key.clone(),
                reason: "duplicate node stable key".to_string(),
            });
        }
    }
    for edge in &output.edges {
        if !stable_keys.insert(("edge", edge.stable_key.clone())) {
            issues.push(DataFlowValidationIssue {
                stable_key: edge.stable_key.clone(),
                reason: "duplicate edge stable key".to_string(),
            });
        }
        if !nodes.contains(&edge.from) || !nodes.contains(&edge.to) {
            issues.push(DataFlowValidationIssue {
                stable_key: edge.stable_key.clone(),
                reason: "dangling edge endpoint".to_string(),
            });
        }
    }
    issues
}

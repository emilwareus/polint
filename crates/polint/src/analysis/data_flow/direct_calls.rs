use super::facts::{
    DataFlowAlgorithm, DataFlowConfidence, DataFlowEdgeFact, DataFlowEdgeKind, DataFlowNodeFact,
    DataFlowNodeKind, DataFlowPrecision, DataFlowProvenance, DataFlowStatus, DataFlowValidation,
};
use super::store::{DataFlowOutput, next_data_flow_edge_id, next_data_flow_node_id};
use crate::analysis::calls::facts::CallTargetStatus;
use crate::analysis::ids::DataFlowNodeId;
use crate::analysis::refined_calls::facts::{RefinedCallConfidence, RefinedCallEdgeFact};
use crate::analysis_kernel::{FactFamily, stable_key_from_parts};
use crate::core::AnalysisDb;

pub(crate) fn derive_direct_call_edges(db: &AnalysisDb, output: &mut DataFlowOutput) {
    for edge in db.refined_call_edges() {
        if edge.status == CallTargetStatus::Resolved {
            derive_resolved_call_edge(output, edge);
        } else {
            derive_unresolved_call_edge(output, edge);
        }
    }
}

fn derive_resolved_call_edge(output: &mut DataFlowOutput, edge: &RefinedCallEdgeFact) {
    let argument = call_node(
        output,
        DataFlowNodeKind::CallArgument,
        edge,
        format!("site:{}:argument", edge.site.0),
    );
    let receiver = call_node(
        output,
        DataFlowNodeKind::CallReceiver,
        edge,
        format!("site:{}:receiver", edge.site.0),
    );
    let returned = call_node(
        output,
        DataFlowNodeKind::CallReturn,
        edge,
        format!("site:{}:return", edge.site.0),
    );
    let callee_boundary = call_node(
        output,
        DataFlowNodeKind::SummaryInput,
        edge,
        format!(
            "site:{}:callee:{}",
            edge.site.0,
            edge.target_function
                .map(|function| function.0.to_string())
                .or_else(|| edge.synthetic_target.clone())
                .unwrap_or_else(|| "unknown".to_string())
        ),
    );

    push_edge(
        output,
        CallEdgeDraft {
            from: argument,
            to: callee_boundary,
            kind: DataFlowEdgeKind::CallArgumentToParameter,
            status: DataFlowStatus::Present,
            precision: precision(edge),
            evidence: vec!["direct_call_argument_boundary".to_string()],
            edge,
        },
    );
    push_edge(
        output,
        CallEdgeDraft {
            from: receiver,
            to: callee_boundary,
            kind: DataFlowEdgeKind::ReceiverToMethod,
            status: DataFlowStatus::Present,
            precision: precision(edge),
            evidence: vec!["direct_call_receiver_boundary".to_string()],
            edge,
        },
    );
    push_edge(
        output,
        CallEdgeDraft {
            from: callee_boundary,
            to: returned,
            kind: DataFlowEdgeKind::CallReturnToUse,
            status: DataFlowStatus::Present,
            precision: DataFlowPrecision::Conservative,
            evidence: vec!["direct_call_return_boundary".to_string()],
            edge,
        },
    );
}

fn derive_unresolved_call_edge(output: &mut DataFlowOutput, edge: &RefinedCallEdgeFact) {
    let source = call_node(
        output,
        DataFlowNodeKind::CallArgument,
        edge,
        format!("site:{}:unresolved-argument", edge.site.0),
    );
    let sink = call_node(
        output,
        DataFlowNodeKind::Synthetic,
        edge,
        format!("site:{}:unresolved-call", edge.site.0),
    );
    push_edge(
        output,
        CallEdgeDraft {
            from: source,
            to: sink,
            kind: DataFlowEdgeKind::UnknownFlow,
            status: if edge.status == CallTargetStatus::SetupMissing {
                DataFlowStatus::SetupMissing
            } else {
                DataFlowStatus::Unknown
            },
            precision: DataFlowPrecision::Unknown,
            evidence: vec![
                "refined_call_unresolved".to_string(),
                edge.reason
                    .map(|reason| format!("reason={reason:?}"))
                    .unwrap_or_else(|| "reason=none".to_string()),
            ],
            edge,
        },
    );
}

struct CallEdgeDraft<'a> {
    from: DataFlowNodeId,
    to: DataFlowNodeId,
    kind: DataFlowEdgeKind,
    status: DataFlowStatus,
    precision: DataFlowPrecision,
    evidence: Vec<String>,
    edge: &'a RefinedCallEdgeFact,
}

fn push_edge(output: &mut DataFlowOutput, draft: CallEdgeDraft<'_>) {
    let stable_key = stable_key_from_parts(
        FactFamily::DataFlowEdge,
        &[
            ("kind", format!("{:?}", draft.kind)),
            ("refined_call", draft.edge.stable_key.clone()),
            ("from", node_key(output, draft.from)),
            ("to", node_key(output, draft.to)),
            ("status", format!("{:?}", draft.status)),
        ],
    );
    if output
        .edges
        .iter()
        .any(|edge| edge.stable_key == stable_key)
    {
        return;
    }
    output.edges.push(DataFlowEdgeFact {
        id: next_data_flow_edge_id(&output.edges),
        from: draft.from,
        to: draft.to,
        kind: draft.kind,
        algorithm: DataFlowAlgorithm::DirectCall,
        status: draft.status,
        precision: draft.precision,
        validation: DataFlowValidation::ReferentiallyValidated,
        confidence: confidence(draft.edge),
        provenance: DataFlowProvenance::Native,
        call_site: Some(draft.edge.site),
        call_target: draft.edge.base_target,
        refined_call: Some(draft.edge.id),
        model: None,
        budget: None,
        evidence: draft.evidence,
        input_stable_keys: {
            let mut keys = draft.edge.input_stable_keys.clone();
            keys.push(draft.edge.stable_key.clone());
            keys
        },
        stable_key,
    });
}

fn node_key(output: &DataFlowOutput, node: DataFlowNodeId) -> String {
    output
        .nodes
        .iter()
        .find(|fact| fact.id == node)
        .map(|fact| fact.stable_key.clone())
        .unwrap_or_else(|| format!("node:{}", node.0))
}

fn call_node(
    output: &mut DataFlowOutput,
    kind: DataFlowNodeKind,
    edge: &RefinedCallEdgeFact,
    suffix: String,
) -> DataFlowNodeId {
    let stable_key = stable_key_from_parts(
        FactFamily::DataFlowNode,
        &[
            ("kind", format!("{kind:?}")),
            ("refined_call", edge.stable_key.clone()),
            ("node", suffix),
        ],
    );
    if let Some(existing) = output
        .nodes
        .iter()
        .find(|node| node.stable_key == stable_key)
        .map(|node| node.id)
    {
        return existing;
    }
    let id = next_data_flow_node_id(&output.nodes);
    output.nodes.push(DataFlowNodeFact {
        id,
        kind,
        language: edge.language,
        file: None,
        function: Some(edge.caller),
        body: None,
        operation: None,
        cfg_node: None,
        place: None,
        symbol: edge.target_symbol,
        reference: None,
        call_site: Some(edge.site),
        model: None,
        span: None,
        stable_key,
    });
    id
}

fn precision(edge: &RefinedCallEdgeFact) -> DataFlowPrecision {
    match edge.precision {
        crate::analysis::calls::facts::CallPrecision::Exact => DataFlowPrecision::Exact,
        crate::analysis::calls::facts::CallPrecision::SetupAware => DataFlowPrecision::SetupAware,
        crate::analysis::calls::facts::CallPrecision::Conservative => {
            DataFlowPrecision::Conservative
        }
        crate::analysis::calls::facts::CallPrecision::Heuristic => DataFlowPrecision::Heuristic,
        crate::analysis::calls::facts::CallPrecision::Ambiguous
        | crate::analysis::calls::facts::CallPrecision::Unknown => DataFlowPrecision::Unknown,
        crate::analysis::calls::facts::CallPrecision::Unsupported => DataFlowPrecision::Unknown,
    }
}

fn confidence(edge: &RefinedCallEdgeFact) -> DataFlowConfidence {
    match edge.confidence {
        RefinedCallConfidence::High => DataFlowConfidence::High,
        RefinedCallConfidence::Medium => DataFlowConfidence::Medium,
        RefinedCallConfidence::Low => DataFlowConfidence::Low,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::calls::facts::{
        CallAlgorithm, CallEdgeKind, CallPrecision, CallProvenance, UnresolvedCallReason,
    };
    use crate::analysis::ids::{CallSiteId, CallTargetId, RefinedCallEdgeId};
    use crate::analysis::refined_calls::facts::{
        RefinedCallEdgeFact, RefinedCallTier, RefinedCallValidation,
    };
    use crate::core::{FunctionId, Language};

    #[test]
    fn resolved_refined_call_creates_role_specific_edges() {
        let mut output = DataFlowOutput::empty();
        derive_resolved_call_edge(&mut output, &refined_edge(CallTargetStatus::Resolved));

        assert!(output.edges.iter().any(|edge| {
            edge.kind == DataFlowEdgeKind::CallArgumentToParameter
                && edge.refined_call == Some(RefinedCallEdgeId(1))
                && edge.call_site == Some(CallSiteId(2))
        }));
        assert!(
            output
                .edges
                .iter()
                .any(|edge| edge.kind == DataFlowEdgeKind::ReceiverToMethod)
        );
        assert!(
            output
                .edges
                .iter()
                .any(|edge| edge.kind == DataFlowEdgeKind::CallReturnToUse)
        );
    }

    #[test]
    fn unresolved_refined_call_creates_unknown_row() {
        let mut output = DataFlowOutput::empty();
        derive_unresolved_call_edge(&mut output, &refined_edge(CallTargetStatus::Unresolved));

        let edge = output
            .edges
            .iter()
            .find(|edge| edge.kind == DataFlowEdgeKind::UnknownFlow)
            .expect("unknown edge");
        assert_eq!(edge.status, DataFlowStatus::Unknown);
        assert!(edge.evidence.iter().any(|value| value.contains("reason=")));
    }

    fn refined_edge(status: CallTargetStatus) -> RefinedCallEdgeFact {
        RefinedCallEdgeFact {
            id: RefinedCallEdgeId(1),
            site: CallSiteId(2),
            base_target: Some(CallTargetId(3)),
            caller: FunctionId(4),
            target_function: Some(FunctionId(5)),
            target_symbol: None,
            synthetic_target: None,
            language: Language::TypeScript,
            edge_kind: CallEdgeKind::Direct,
            algorithm: CallAlgorithm::DirectReference,
            tier: RefinedCallTier::TypeValueFunctionToken,
            status,
            reason: if status == CallTargetStatus::Resolved {
                None
            } else {
                Some(UnresolvedCallReason::Unknown)
            },
            provenance: CallProvenance::NativeDirect,
            precision: CallPrecision::SetupAware,
            validation: RefinedCallValidation::ReferentiallyValidated,
            confidence: RefinedCallConfidence::High,
            evidence: vec!["test".to_string()],
            input_stable_keys: vec!["call-site".to_string()],
            stable_key: "refined:edge".to_string(),
        }
    }
}

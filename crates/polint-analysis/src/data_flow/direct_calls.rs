use super::facts::{
    DataFlowAlgorithm, DataFlowConfidence, DataFlowEdgeFact, DataFlowEdgeKind, DataFlowNodeFact,
    DataFlowNodeKind, DataFlowPrecision, DataFlowProvenance, DataFlowStatus, DataFlowValidation,
};
use super::store::{DataFlowOutput, next_data_flow_edge_id, next_data_flow_node_id};
use crate::calls::facts::{CallSiteFact, CallSyntaxKind, CallTargetStatus};
use crate::ids::{CallSiteId, DataFlowBudgetId, DataFlowNodeId, PlaceId};
use crate::refined_calls::facts::{RefinedCallConfidence, RefinedCallEdgeFact};
use crate::summaries::facts::{
    FlowRoot, SummaryDomainKind, SummaryFact, SummaryPrecision, SummaryStatus,
};
use polint_analysis_api::{FactFamily, stable_key_from_parts};
use crate::AnalysisHost;
use polint_core::{Language};

pub fn derive_direct_call_edges(db: &impl AnalysisHost, output: &mut DataFlowOutput) {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    for edge in db.refined_call_edges() {
        if edge.status == CallTargetStatus::Resolved {
            derive_resolved_call_edge(db, output, edge);
        } else {
            derive_unresolved_call_edge(interner, output, edge);
        }
    }
}

fn derive_resolved_call_edge(
    db: &impl AnalysisHost,
    output: &mut DataFlowOutput,
    edge: &RefinedCallEdgeFact,
) {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    let site = db.call_sites().iter().find(|site| site.id == edge.site);
    let site_id = site.map(|site| site.id);
    let callee_input = call_node(
        interner,
        output,
        DataFlowNodeKind::SummaryInput,
        edge,
        "callee-input".to_string(),
        site_id,
    );
    let callee_output = call_node(
        interner,
        output,
        DataFlowNodeKind::SummaryOutput,
        edge,
        "callee-output".to_string(),
        site_id,
    );

    let argument_nodes = argument_nodes(output, edge, site);
    for (index, argument) in argument_nodes.iter().copied() {
        push_edge(
            interner,
            output,
            CallEdgeDraft {
                from: argument,
                to: callee_input,
                kind: DataFlowEdgeKind::CallArgumentToParameter,
                status: DataFlowStatus::Present,
                precision: precision(edge),
                validation: DataFlowValidation::ReferentiallyValidated,
                budget: None,
                evidence: vec![
                    "direct_call_argument_boundary".to_string(),
                    format!("argument_index={index}"),
                ],
                extra_input_stable_keys: Vec::new(),
                call_site: site_id,
                edge,
            },
        );
    }
    if let Some(receiver) = receiver_node(output, site) {
        push_edge(
            interner,
            output,
            CallEdgeDraft {
                from: receiver,
                to: callee_input,
                kind: DataFlowEdgeKind::ReceiverToMethod,
                status: DataFlowStatus::Present,
                precision: precision(edge),
                validation: DataFlowValidation::ReferentiallyValidated,
                budget: None,
                evidence: vec!["direct_call_receiver_boundary".to_string()],
                extra_input_stable_keys: Vec::new(),
                call_site: site_id,
                edge,
            },
        );
    }
    bridge_target_summaries(db, output, edge, site, site_id);
    if let Some(returned) = return_node(output, site) {
        push_edge(
            interner,
            output,
            CallEdgeDraft {
                from: callee_output,
                to: returned,
                kind: DataFlowEdgeKind::CallReturnToUse,
                status: DataFlowStatus::Present,
                precision: DataFlowPrecision::Conservative,
                validation: DataFlowValidation::ReferentiallyValidated,
                budget: None,
                evidence: vec!["direct_call_return_boundary".to_string()],
                extra_input_stable_keys: Vec::new(),
                call_site: site_id,
                edge,
            },
        );
    }
}

fn bridge_target_summaries(
    db: &impl AnalysisHost,
    output: &mut DataFlowOutput,
    edge: &RefinedCallEdgeFact,
    site: Option<&CallSiteFact>,
    call_site: Option<CallSiteId>,
) {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    let Some(target_function) = edge.target_function else {
        return;
    };

    for summary in db.summary_facts().iter().filter(|summary| {
        summary.function == target_function
            && summary.domain == SummaryDomainKind::DataFlowTito
            && summary.status == SummaryStatus::Present
    }) {
        for flow in &summary.tito_flows {
            if !super::summary_edges::flow_kind_projects_as_tito(flow.kind) {
                continue;
            }
            let Some(from) = call_root_node(output, edge, site, flow.from) else {
                continue;
            };
            let Some(to) = call_root_node(output, edge, site, flow.to) else {
                continue;
            };
            let summary_input = summary_node(
                interner,
                output,
                summary,
                DataFlowNodeKind::SummaryInput,
                &super::summary_edges::root_role(flow.from),
                call_site,
            );
            let summary_output = summary_node(
                interner,
                output,
                summary,
                DataFlowNodeKind::SummaryOutput,
                &super::summary_edges::root_role(flow.to),
                call_site,
            );
            let summary_inputs = vec![
                interner.resolve(summary.stable_key).to_string(),
                interner.resolve(summary.callable_stable_key).to_string(),
            ];
            push_edge(
                interner,
                output,
                CallEdgeDraft {
                    from,
                    to: summary_input,
                    kind: DataFlowEdgeKind::SummaryProjected,
                    status: DataFlowStatus::Present,
                    precision: summary_precision(summary.precision),
                    validation: DataFlowValidation::ReferentiallyValidated,
                    budget: None,
                    evidence: vec![
                        "direct_call_summary_input_bridge".to_string(),
                        format!("summary={}", interner.resolve(summary.stable_key)),
                        format!("flow_from={}", super::summary_edges::root_role(flow.from)),
                    ],
                    extra_input_stable_keys: summary_inputs.clone(),
                    call_site,
                    edge,
                },
            );
            push_call_summary_tito_edge(
                interner,
                output,
                edge,
                summary,
                flow,
                summary_input,
                (summary_output, call_site),
            );
            push_edge(
                interner,
                output,
                CallEdgeDraft {
                    from: summary_output,
                    to,
                    kind: DataFlowEdgeKind::SummaryProjected,
                    status: DataFlowStatus::Present,
                    precision: summary_precision(summary.precision),
                    validation: DataFlowValidation::ReferentiallyValidated,
                    budget: None,
                    evidence: vec![
                        "direct_call_summary_output_bridge".to_string(),
                        format!("summary={}", interner.resolve(summary.stable_key)),
                        format!("flow_to={}", super::summary_edges::root_role(flow.to)),
                    ],
                    extra_input_stable_keys: summary_inputs,
                    call_site,
                    edge,
                },
            );
        }
    }
}

fn argument_nodes(
    output: &mut DataFlowOutput,
    _edge: &RefinedCallEdgeFact,
    site: Option<&CallSiteFact>,
) -> Vec<(usize, DataFlowNodeId)> {
    site.into_iter()
        .flat_map(|site| site.arguments.iter().copied())
        .enumerate()
        .filter_map(|(index, place)| place_node(output, place).map(|node| (index, node)))
        .collect::<Vec<_>>()
}

fn call_root_node(
    output: &mut DataFlowOutput,
    edge: &RefinedCallEdgeFact,
    site: Option<&CallSiteFact>,
    root: FlowRoot,
) -> Option<DataFlowNodeId> {
    match root {
        FlowRoot::Param(index) => {
            let site = site?;
            if edge.language == Language::Go
                && site.kind == CallSyntaxKind::Method
                && site.receiver.is_some()
            {
                if index == 0 {
                    return receiver_node(output, Some(site));
                }
                return site
                    .arguments
                    .get((index - 1) as usize)
                    .copied()
                    .and_then(|place| place_node(output, place));
            }
            site.arguments
                .get(index as usize)
                .copied()
                .and_then(|place| place_node(output, place))
        }
        FlowRoot::Receiver => receiver_node(output, site),
        FlowRoot::Return => return_node(output, site),
    }
}

fn receiver_node(
    output: &mut DataFlowOutput,
    site: Option<&CallSiteFact>,
) -> Option<DataFlowNodeId> {
    let receiver = site.and_then(|site| site.receiver)?;
    place_node(output, receiver)
}

fn return_node(output: &mut DataFlowOutput, site: Option<&CallSiteFact>) -> Option<DataFlowNodeId> {
    site.and_then(|site| site.result)
        .and_then(|place| place_node(output, place))
}

fn place_node(output: &DataFlowOutput, place: PlaceId) -> Option<DataFlowNodeId> {
    output
        .nodes
        .iter()
        .find(|node| node.place == Some(place))
        .map(|node| node.id)
}

fn derive_unresolved_call_edge(
    interner: &polint_core::StableKeyInterner,
    output: &mut DataFlowOutput,
    edge: &RefinedCallEdgeFact,
) {
    let source = call_node(
        interner,
        output,
        DataFlowNodeKind::CallArgument,
        edge,
        "unresolved-argument".to_string(),
        None,
    );
    let sink = call_node(
        interner,
        output,
        DataFlowNodeKind::Synthetic,
        edge,
        "unresolved-call".to_string(),
        None,
    );
    let status = unresolved_status(edge.status);
    let budget = (status == DataFlowStatus::BudgetExceeded).then(|| {
        super::local::budget_fact(
            interner,
            super::facts::DataFlowBudgetReason::PathCount,
            1,
            2,
            &interner.resolve(edge.stable_key),
            output,
        )
    });
    push_edge(
        interner,
        output,
        CallEdgeDraft {
            from: source,
            to: sink,
            kind: unresolved_kind(status),
            status,
            precision: DataFlowPrecision::Unknown,
            validation: unresolved_validation(status, budget),
            budget,
            evidence: vec![
                "refined_call_unresolved".to_string(),
                edge.reason
                    .map(|reason| format!("reason={reason:?}"))
                    .unwrap_or_else(|| "reason=none".to_string()),
            ],
            extra_input_stable_keys: Vec::new(),
            call_site: None,
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
    validation: DataFlowValidation,
    budget: Option<DataFlowBudgetId>,
    evidence: Vec<String>,
    extra_input_stable_keys: Vec<String>,
    call_site: Option<CallSiteId>,
    edge: &'a RefinedCallEdgeFact,
}

fn push_edge(
    interner: &polint_core::StableKeyInterner,
    output: &mut DataFlowOutput,
    draft: CallEdgeDraft<'_>,
) {
    let stable_key = stable_key_from_parts(
        interner,
        FactFamily::DataFlowEdge,
        &[
            ("kind", format!("{:?}", draft.kind)),
            (
                "refined_call",
                interner.resolve(draft.edge.stable_key).to_string(),
            ),
            ("from", node_key(interner, output, draft.from)),
            ("to", node_key(interner, output, draft.to)),
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
        validation: draft.validation,
        confidence: confidence(draft.edge),
        provenance: DataFlowProvenance::Native,
        call_site: draft.call_site,
        call_target: draft.edge.base_target,
        refined_call: Some(draft.edge.id),
        model: None,
        budget: draft.budget,
        evidence: draft.evidence,
        input_stable_keys: {
            let mut keys = draft.edge.input_stable_keys.clone();
            keys.push(interner.resolve(draft.edge.stable_key).to_string());
            keys.extend(draft.extra_input_stable_keys);
            keys
        },
        stable_key,
    });
}

fn summary_node(
    interner: &polint_core::StableKeyInterner,
    output: &mut DataFlowOutput,
    fact: &SummaryFact,
    kind: DataFlowNodeKind,
    role: &str,
    call_site: Option<CallSiteId>,
) -> DataFlowNodeId {
    let stable_key = stable_key_from_parts(
        interner,
        FactFamily::DataFlowNode,
        &[
            ("kind", format!("{kind:?}")),
            ("summary", interner.resolve(fact.stable_key).to_string()),
            ("role", role.to_string()),
            (
                "call_site",
                call_site
                    .map(|id| id.0.to_string())
                    .unwrap_or_else(|| "none".to_string()),
            ),
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
        language: Language::Unknown,
        file: None,
        function: Some(fact.function),
        body: None,
        operation: None,
        cfg_node: None,
        place: None,
        symbol: None,
        reference: None,
        call_site,
        model: None,
        span: None,
        stable_key,
    });
    id
}

fn push_call_summary_tito_edge(
    interner: &polint_core::StableKeyInterner,
    output: &mut DataFlowOutput,
    edge: &RefinedCallEdgeFact,
    summary: &SummaryFact,
    flow: &crate::summaries::facts::SummaryFlowEdge,
    from: DataFlowNodeId,
    pair: (DataFlowNodeId, Option<CallSiteId>),
) {
    let (to, call_site) = pair;
    let stable_key = stable_key_from_parts(
        interner,
        FactFamily::DataFlowEdge,
        &[
            ("kind", format!("{:?}", DataFlowEdgeKind::SummaryTito)),
            (
                "refined_call",
                interner.resolve(edge.stable_key).to_string(),
            ),
            ("summary", interner.resolve(summary.stable_key).to_string()),
            ("from", node_key(interner, output, from)),
            ("to", node_key(interner, output, to)),
            ("flow_from", super::summary_edges::root_role(flow.from)),
            ("flow_to", super::summary_edges::root_role(flow.to)),
            ("flow_kind", format!("{:?}", flow.kind)),
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
        from,
        to,
        kind: DataFlowEdgeKind::SummaryTito,
        algorithm: DataFlowAlgorithm::SummaryProjection,
        status: DataFlowStatus::Present,
        precision: summary_precision(summary.precision),
        validation: DataFlowValidation::ReferentiallyValidated,
        confidence: DataFlowConfidence::Medium,
        provenance: DataFlowProvenance::Summary,
        call_site,
        call_target: edge.base_target,
        refined_call: Some(edge.id),
        model: None,
        budget: None,
        evidence: vec![
            "direct_call_summary_data_flow_tito".to_string(),
            format!("summary={}", interner.resolve(summary.stable_key)),
            format!(
                "flow={}->{}:{:?}",
                super::summary_edges::root_role(flow.from),
                super::summary_edges::root_role(flow.to),
                flow.kind
            ),
        ],
        input_stable_keys: vec![
            interner.resolve(edge.stable_key).to_string(),
            interner.resolve(summary.stable_key).to_string(),
            interner.resolve(summary.callable_stable_key).to_string(),
        ],
        stable_key,
    });
}

fn unresolved_status(status: CallTargetStatus) -> DataFlowStatus {
    match status {
        CallTargetStatus::Resolved => DataFlowStatus::Present,
        CallTargetStatus::Ambiguous | CallTargetStatus::Unresolved => DataFlowStatus::Unknown,
        CallTargetStatus::Unsupported => DataFlowStatus::Unsupported,
        CallTargetStatus::SetupMissing => DataFlowStatus::SetupMissing,
        CallTargetStatus::BudgetExceeded => DataFlowStatus::BudgetExceeded,
        CallTargetStatus::Rejected => DataFlowStatus::Rejected,
    }
}

fn unresolved_kind(status: DataFlowStatus) -> DataFlowEdgeKind {
    match status {
        DataFlowStatus::BudgetExceeded => DataFlowEdgeKind::BudgetTruncated,
        DataFlowStatus::Unsupported => DataFlowEdgeKind::HavocFlow,
        DataFlowStatus::Present
        | DataFlowStatus::Unknown
        | DataFlowStatus::SetupMissing
        | DataFlowStatus::Rejected => DataFlowEdgeKind::UnknownFlow,
    }
}

fn unresolved_validation(
    status: DataFlowStatus,
    budget: Option<DataFlowBudgetId>,
) -> DataFlowValidation {
    match status {
        DataFlowStatus::BudgetExceeded if budget.is_some() => DataFlowValidation::BudgetValidated,
        DataFlowStatus::Rejected => DataFlowValidation::Rejected,
        _ => DataFlowValidation::Native,
    }
}

fn node_key(
    interner: &polint_core::StableKeyInterner,
    output: &DataFlowOutput,
    node: DataFlowNodeId,
) -> String {
    output
        .nodes
        .iter()
        .find(|fact| fact.id == node)
        .map(|fact| interner.resolve(fact.stable_key).to_string())
        .unwrap_or_else(|| format!("node:{}", node.0))
}

fn call_node(
    interner: &polint_core::StableKeyInterner,
    output: &mut DataFlowOutput,
    kind: DataFlowNodeKind,
    edge: &RefinedCallEdgeFact,
    suffix: String,
    call_site: Option<CallSiteId>,
) -> DataFlowNodeId {
    let stable_key = stable_key_from_parts(
        interner,
        FactFamily::DataFlowNode,
        &[
            ("kind", format!("{kind:?}")),
            (
                "refined_call",
                interner.resolve(edge.stable_key).to_string(),
            ),
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
        call_site,
        model: None,
        span: None,
        stable_key,
    });
    id
}

fn precision(edge: &RefinedCallEdgeFact) -> DataFlowPrecision {
    match edge.precision {
        crate::calls::facts::CallPrecision::Exact => DataFlowPrecision::Exact,
        crate::calls::facts::CallPrecision::SetupAware => DataFlowPrecision::SetupAware,
        crate::calls::facts::CallPrecision::Conservative => {
            DataFlowPrecision::Conservative
        }
        crate::calls::facts::CallPrecision::Heuristic => DataFlowPrecision::Heuristic,
        crate::calls::facts::CallPrecision::Ambiguous
        | crate::calls::facts::CallPrecision::Unknown => DataFlowPrecision::Unknown,
        crate::calls::facts::CallPrecision::Unsupported => DataFlowPrecision::Unknown,
    }
}

fn confidence(edge: &RefinedCallEdgeFact) -> DataFlowConfidence {
    match edge.confidence {
        RefinedCallConfidence::High => DataFlowConfidence::High,
        RefinedCallConfidence::Medium => DataFlowConfidence::Medium,
        RefinedCallConfidence::Low => DataFlowConfidence::Low,
    }
}

fn summary_precision(precision: SummaryPrecision) -> DataFlowPrecision {
    match precision {
        SummaryPrecision::Local => DataFlowPrecision::Syntax,
        SummaryPrecision::SetupAware => DataFlowPrecision::SetupAware,
        SummaryPrecision::Heuristic => DataFlowPrecision::Heuristic,
        SummaryPrecision::UnknownTop => DataFlowPrecision::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use crate::LocalAnalysisDb;
    use super::*;
    use crate::calls::facts::{
        CallAlgorithm, CallCallee, CallEdgeKind, CallPrecision, CallProvenance, CallSiteFact,
        CallSyntaxKind, UnresolvedCallReason,
    };
    use crate::calls::store::CallOutput;
    use crate::data_flow::store::DataFlowStore;
    use crate::ids::{
        CallSiteId, CallTargetId, DataFlowNodeId, MirBodyId, MirOpId, PlaceId, RefinedCallEdgeId,
        SummaryId,
    };
    use crate::ifds::{DataFlowPathStatus, DataFlowSearchBudget, find_taint_paths};
    use crate::refined_calls::facts::{
        RefinedCallEdgeFact, RefinedCallTier, RefinedCallValidation,
    };
    use crate::refined_calls::store::RefinedCallOutput;
    use crate::summaries::facts::{
        FlowKind, FlowRoot, SummaryDomainKind, SummaryFact, SummaryFlowEdge, SummaryPrecision,
        SummaryProvenance, SummaryStatus,
    };
    use crate::summaries::store::SummaryOutput;
    use polint_core::{FileId, FunctionId, Language, Span, stable_key_for_test};

    #[test]
    fn resolved_refined_call_creates_role_specific_edges() {
        let mut db = LocalAnalysisDb::default();
        db.replace_call_facts(CallOutput {
            sites: vec![call_site(Some(PlaceId(20)))],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("valid call facts");
        let mut output = DataFlowOutput {
            nodes: vec![
                place_node(10, PlaceId(10)),
                place_node(20, PlaceId(20)),
                place_node(30, PlaceId(30)),
            ],
            edges: Vec::new(),
            models: Vec::new(),
            budgets: Vec::new(),
        };
        derive_resolved_call_edge(&db, &mut output, &refined_edge(CallTargetStatus::Resolved));

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
                .any(|edge| edge.kind == DataFlowEdgeKind::CallReturnToUse
                    && edge.from != DataFlowNodeId(10)
                    && edge.from != DataFlowNodeId(20)
                    && edge.to == DataFlowNodeId(30))
        );
    }

    #[test]
    fn unresolved_refined_call_creates_unknown_row() {
        let mut output = DataFlowOutput::empty();
        derive_unresolved_call_edge(
            &crate::LocalAnalysisDb::new().stable_key_interner(),
            &mut output,
            &refined_edge(CallTargetStatus::Unresolved),
        );

        let edge = output
            .edges
            .iter()
            .find(|edge| edge.kind == DataFlowEdgeKind::UnknownFlow)
            .expect("unknown edge");
        assert_eq!(edge.status, DataFlowStatus::Unknown);
        assert!(edge.evidence.iter().any(|value| value.contains("reason=")));
    }

    #[test]
    fn direct_call_edges_use_real_call_site_place_nodes_and_skip_missing_receiver() {
        let mut db = LocalAnalysisDb::default();
        db.replace_call_facts(CallOutput {
            sites: vec![call_site(None)],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("valid call facts");
        db.replace_refined_call_facts(RefinedCallOutput {
            edges: vec![refined_edge(CallTargetStatus::Resolved)],
        })
        .expect("valid refined calls");
        let mut output = DataFlowOutput {
            nodes: vec![
                place_node(10, PlaceId(10)),
                place_node(20, PlaceId(20)),
                place_node(30, PlaceId(30)),
            ],
            edges: Vec::new(),
            models: Vec::new(),
            budgets: Vec::new(),
        };

        derive_direct_call_edges(&db, &mut output);

        assert!(output.edges.iter().any(|edge| {
            edge.kind == DataFlowEdgeKind::CallArgumentToParameter
                && edge.from == DataFlowNodeId(10)
        }));
        assert!(output.edges.iter().any(|edge| {
            edge.kind == DataFlowEdgeKind::CallReturnToUse && edge.to == DataFlowNodeId(30)
        }));
        assert!(
            !output
                .edges
                .iter()
                .any(|edge| edge.kind == DataFlowEdgeKind::ReceiverToMethod)
        );
    }

    #[test]
    fn direct_call_edges_emit_receiver_only_for_real_receiver_place() {
        let mut db = LocalAnalysisDb::default();
        db.replace_call_facts(CallOutput {
            sites: vec![call_site(Some(PlaceId(20)))],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("valid call facts");
        db.replace_refined_call_facts(RefinedCallOutput {
            edges: vec![refined_edge(CallTargetStatus::Resolved)],
        })
        .expect("valid refined calls");
        let mut output = DataFlowOutput {
            nodes: vec![
                place_node(10, PlaceId(10)),
                place_node(20, PlaceId(20)),
                place_node(30, PlaceId(30)),
            ],
            edges: Vec::new(),
            models: Vec::new(),
            budgets: Vec::new(),
        };

        derive_direct_call_edges(&db, &mut output);

        assert!(output.edges.iter().any(|edge| {
            edge.kind == DataFlowEdgeKind::ReceiverToMethod && edge.from == DataFlowNodeId(20)
        }));
    }

    #[test]
    fn resolved_refined_call_bridges_through_target_data_flow_summary() {
        let mut db = LocalAnalysisDb::default();
        db.replace_call_facts(CallOutput {
            sites: vec![call_site(None)],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("valid call facts");
        db.replace_refined_call_facts(RefinedCallOutput {
            edges: vec![refined_edge(CallTargetStatus::Resolved)],
        })
        .expect("valid refined calls");
        db.replace_summary_facts(SummaryOutput {
            summaries: vec![summary_fact()],
            events: Vec::new(),
        });
        let mut output = DataFlowOutput {
            nodes: vec![
                place_node(10, PlaceId(10)),
                place_node(20, PlaceId(20)),
                place_node(30, PlaceId(30)),
            ],
            edges: Vec::new(),
            models: Vec::new(),
            budgets: Vec::new(),
        };

        derive_direct_call_edges(&db, &mut output);
        super::super::summary_edges::derive_summary_projected_edges(&db, &mut output);
        let store = DataFlowStore::from_output(output, &polint_core::test_stable_key_interner())
            .expect("valid store");
        let source = store
            .nodes()
            .iter()
            .find(|node| node.place == Some(PlaceId(10)))
            .expect("argument node")
            .id;
        let sink = store
            .nodes()
            .iter()
            .find(|node| node.place == Some(PlaceId(30)))
            .expect("return node")
            .id;
        let paths = find_taint_paths(
            &db,
            &store,
            source,
            sink,
            &std::collections::BTreeSet::new(),
            DataFlowSearchBudget {
                max_depth: 8,
                max_paths: 4,
            },
        );

        assert_eq!(paths[0].status, DataFlowPathStatus::Found);
        let found_edges = paths[0]
            .edges
            .iter()
            .filter_map(|id| store.edges().iter().find(|edge| edge.id == *id))
            .collect::<Vec<_>>();
        assert!(
            found_edges
                .iter()
                .any(|edge| edge.kind == DataFlowEdgeKind::SummaryTito),
            "call path should cross the target TITO summary edge: {found_edges:#?}"
        );
        assert!(
            found_edges
                .iter()
                .filter(|edge| edge.kind == DataFlowEdgeKind::SummaryProjected)
                .count()
                >= 2,
            "call boundary must connect to both summary input and output"
        );
    }

    #[test]
    fn target_data_flow_summary_only_bridges_matching_argument_root() {
        let mut db = LocalAnalysisDb::default();
        db.replace_call_facts(CallOutput {
            sites: vec![call_site(None)],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("valid call facts");
        db.replace_refined_call_facts(RefinedCallOutput {
            edges: vec![refined_edge(CallTargetStatus::Resolved)],
        })
        .expect("valid refined calls");
        db.replace_summary_facts(SummaryOutput {
            summaries: vec![summary_fact()],
            events: Vec::new(),
        });
        let mut output = DataFlowOutput {
            nodes: vec![
                place_node(10, PlaceId(10)),
                place_node(20, PlaceId(20)),
                place_node(30, PlaceId(30)),
            ],
            edges: Vec::new(),
            models: Vec::new(),
            budgets: Vec::new(),
        };

        derive_direct_call_edges(&db, &mut output);
        super::super::summary_edges::derive_summary_projected_edges(&db, &mut output);
        let store = DataFlowStore::from_output(output, &polint_core::test_stable_key_interner())
            .expect("valid store");
        let unrelated_argument = store
            .nodes()
            .iter()
            .find(|node| node.place == Some(PlaceId(20)))
            .expect("second argument node")
            .id;
        let sink = store
            .nodes()
            .iter()
            .find(|node| node.place == Some(PlaceId(30)))
            .expect("return node")
            .id;
        let paths = find_taint_paths(
            &db,
            &store,
            unrelated_argument,
            sink,
            &std::collections::BTreeSet::new(),
            DataFlowSearchBudget {
                max_depth: 8,
                max_paths: 4,
            },
        );

        assert_eq!(paths[0].status, DataFlowPathStatus::NotFound);
    }

    #[test]
    fn go_method_param_zero_summary_bridges_receiver_not_first_argument() {
        let mut db = LocalAnalysisDb::default();
        db.replace_call_facts(CallOutput {
            sites: vec![go_method_call_site()],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("valid call facts");
        db.replace_refined_call_facts(RefinedCallOutput {
            edges: vec![RefinedCallEdgeFact {
                language: Language::Go,
                ..refined_edge(CallTargetStatus::Resolved)
            }],
        })
        .expect("valid refined calls");
        db.replace_summary_facts(SummaryOutput {
            summaries: vec![summary_fact()],
            events: Vec::new(),
        });
        let mut output = DataFlowOutput {
            nodes: vec![
                place_node(10, PlaceId(10)),
                place_node(20, PlaceId(20)),
                place_node(30, PlaceId(30)),
            ],
            edges: Vec::new(),
            models: Vec::new(),
            budgets: Vec::new(),
        };

        derive_direct_call_edges(&db, &mut output);
        super::super::summary_edges::derive_summary_projected_edges(&db, &mut output);
        let store = DataFlowStore::from_output(output, &polint_core::test_stable_key_interner())
            .expect("valid store");
        let receiver = store
            .nodes()
            .iter()
            .find(|node| node.place == Some(PlaceId(20)))
            .expect("receiver node")
            .id;
        let first_argument = store
            .nodes()
            .iter()
            .find(|node| node.place == Some(PlaceId(10)))
            .expect("argument node")
            .id;
        let sink = store
            .nodes()
            .iter()
            .find(|node| node.place == Some(PlaceId(30)))
            .expect("return node")
            .id;

        let receiver_paths = find_taint_paths(
            &db,
            &store,
            receiver,
            sink,
            &std::collections::BTreeSet::new(),
            DataFlowSearchBudget {
                max_depth: 8,
                max_paths: 4,
            },
        );
        let argument_paths = find_taint_paths(
            &db,
            &store,
            first_argument,
            sink,
            &std::collections::BTreeSet::new(),
            DataFlowSearchBudget {
                max_depth: 8,
                max_paths: 4,
            },
        );

        assert_eq!(receiver_paths[0].status, DataFlowPathStatus::Found);
        assert_eq!(argument_paths[0].status, DataFlowPathStatus::NotFound);
    }

    #[test]
    fn barrier_summary_flow_does_not_bridge_direct_call_path() {
        let mut db = LocalAnalysisDb::default();
        db.replace_call_facts(CallOutput {
            sites: vec![call_site(None)],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("valid call facts");
        db.replace_refined_call_facts(RefinedCallOutput {
            edges: vec![refined_edge(CallTargetStatus::Resolved)],
        })
        .expect("valid refined calls");
        let mut summary = summary_fact();
        summary.tito_flows[0].kind = FlowKind::Barrier;
        db.replace_summary_facts(SummaryOutput {
            summaries: vec![summary],
            events: Vec::new(),
        });
        let mut output = DataFlowOutput {
            nodes: vec![
                place_node(10, PlaceId(10)),
                place_node(20, PlaceId(20)),
                place_node(30, PlaceId(30)),
            ],
            edges: Vec::new(),
            models: Vec::new(),
            budgets: Vec::new(),
        };

        derive_direct_call_edges(&db, &mut output);
        super::super::summary_edges::derive_summary_projected_edges(&db, &mut output);
        let store = DataFlowStore::from_output(output, &polint_core::test_stable_key_interner())
            .expect("valid store");
        let source = store
            .nodes()
            .iter()
            .find(|node| node.place == Some(PlaceId(10)))
            .expect("argument node")
            .id;
        let sink = store
            .nodes()
            .iter()
            .find(|node| node.place == Some(PlaceId(30)))
            .expect("return node")
            .id;
        let paths = find_taint_paths(
            &db,
            &store,
            source,
            sink,
            &std::collections::BTreeSet::new(),
            DataFlowSearchBudget {
                max_depth: 8,
                max_paths: 4,
            },
        );

        assert_eq!(paths[0].status, DataFlowPathStatus::NotFound);
    }

    #[test]
    fn repeated_target_summary_paths_do_not_cross_between_call_sites() {
        let mut db = LocalAnalysisDb::default();
        db.replace_call_facts(CallOutput {
            sites: vec![
                call_site_with(CallSiteId(2), vec![PlaceId(10)], Some(PlaceId(30))),
                call_site_with(CallSiteId(3), vec![PlaceId(40)], Some(PlaceId(50))),
            ],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("valid call facts");
        db.replace_refined_call_facts(RefinedCallOutput {
            edges: vec![
                refined_edge_for(RefinedCallEdgeId(1), CallSiteId(2)),
                refined_edge_for(RefinedCallEdgeId(2), CallSiteId(3)),
            ],
        })
        .expect("valid refined calls");
        db.replace_summary_facts(SummaryOutput {
            summaries: vec![summary_fact()],
            events: Vec::new(),
        });
        let mut output = DataFlowOutput {
            nodes: vec![
                place_node(0, PlaceId(10)),
                place_node(1, PlaceId(30)),
                place_node(2, PlaceId(40)),
                place_node(3, PlaceId(50)),
            ],
            edges: Vec::new(),
            models: Vec::new(),
            budgets: Vec::new(),
        };

        derive_direct_call_edges(&db, &mut output);
        super::super::summary_edges::derive_summary_projected_edges(&db, &mut output);
        let store = DataFlowStore::from_output(output, &polint_core::test_stable_key_interner())
            .expect("valid store");
        let first_argument = store
            .nodes()
            .iter()
            .find(|node| node.place == Some(PlaceId(10)))
            .expect("first argument node")
            .id;
        let first_result = store
            .nodes()
            .iter()
            .find(|node| node.place == Some(PlaceId(30)))
            .expect("first result node")
            .id;
        let second_result = store
            .nodes()
            .iter()
            .find(|node| node.place == Some(PlaceId(50)))
            .expect("second result node")
            .id;

        let same_call_paths = find_taint_paths(
            &db,
            &store,
            first_argument,
            first_result,
            &std::collections::BTreeSet::new(),
            DataFlowSearchBudget {
                max_depth: 8,
                max_paths: 4,
            },
        );
        let cross_call_paths = find_taint_paths(
            &db,
            &store,
            first_argument,
            second_result,
            &std::collections::BTreeSet::new(),
            DataFlowSearchBudget {
                max_depth: 8,
                max_paths: 4,
            },
        );

        assert_eq!(same_call_paths[0].status, DataFlowPathStatus::Found);
        assert_eq!(cross_call_paths[0].status, DataFlowPathStatus::NotFound);
    }

    #[test]
    fn unresolved_refined_call_preserves_non_unknown_statuses() {
        for (call_status, data_flow_status) in [
            (CallTargetStatus::Unsupported, DataFlowStatus::Unsupported),
            (CallTargetStatus::SetupMissing, DataFlowStatus::SetupMissing),
            (
                CallTargetStatus::BudgetExceeded,
                DataFlowStatus::BudgetExceeded,
            ),
            (CallTargetStatus::Rejected, DataFlowStatus::Rejected),
        ] {
            let mut output = DataFlowOutput::empty();
            derive_unresolved_call_edge(
                &crate::LocalAnalysisDb::new().stable_key_interner(),
                &mut output,
                &refined_edge(call_status),
            );

            assert_eq!(output.edges[0].status, data_flow_status);
            if data_flow_status == DataFlowStatus::BudgetExceeded {
                assert!(
                    output
                        .budgets
                        .iter()
                        .any(|budget| budget.observed > budget.limit),
                    "budget exceeded rows must record observed > limit"
                );
            }
        }
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
            stable_key: polint_core::stable_key_for_test("refined:edge"),
        }
    }

    fn refined_edge_for(id: RefinedCallEdgeId, site: CallSiteId) -> RefinedCallEdgeFact {
        RefinedCallEdgeFact {
            id,
            site,
            stable_key: polint_core::stable_key_for_test(&format!("refined:edge:{}", id.0)),
            ..refined_edge(CallTargetStatus::Resolved)
        }
    }

    fn call_site(receiver: Option<PlaceId>) -> CallSiteFact {
        CallSiteFact {
            in_throw: false,
            id: CallSiteId(2),
            language: Language::TypeScript,
            file: FileId(1),
            caller: FunctionId(4),
            owner_symbol: None,
            body: MirBodyId(5),
            operation: MirOpId(6),
            span: Span::point(FileId(1), 1, 2),
            kind: if receiver.is_some() {
                CallSyntaxKind::Method
            } else {
                CallSyntaxKind::Function
            },
            callee: CallCallee::Identifier {
                reference: None,
                name: "target".to_string(),
            },
            receiver,
            arguments: vec![PlaceId(10), PlaceId(20)],
            result: Some(PlaceId(30)),
            status: CallTargetStatus::Resolved,
            precision: CallPrecision::Exact,
            stable_key: polint_core::StableKeyId(2),
        }
    }

    fn call_site_with(
        id: CallSiteId,
        arguments: Vec<PlaceId>,
        result: Option<PlaceId>,
    ) -> CallSiteFact {
        CallSiteFact {
            id,
            operation: MirOpId(id.0),
            arguments,
            result,
            stable_key: polint_core::StableKeyId(id.0 as u32),
            ..call_site(None)
        }
    }

    fn go_method_call_site() -> CallSiteFact {
        CallSiteFact {
            language: Language::Go,
            kind: CallSyntaxKind::Method,
            receiver: Some(PlaceId(20)),
            arguments: vec![PlaceId(10)],
            ..call_site(Some(PlaceId(20)))
        }
    }

    fn place_node(id: u64, place: PlaceId) -> DataFlowNodeFact {
        DataFlowNodeFact {
            id: DataFlowNodeId(id),
            kind: DataFlowNodeKind::Place,
            language: Language::TypeScript,
            file: Some(FileId(1)),
            function: Some(FunctionId(4)),
            body: None,
            operation: None,
            cfg_node: None,
            place: Some(place),
            symbol: None,
            reference: None,
            call_site: None,
            model: None,
            span: None,
            stable_key: polint_core::stable_key_for_test(&format!("node:place:{}", place.0)),
        }
    }

    fn summary_fact() -> SummaryFact {
        SummaryFact {
            id: SummaryId(1),
            callable_stable_key: stable_key_for_test("callable:target"),
            function: FunctionId(5),
            domain: SummaryDomainKind::DataFlowTito,
            status: SummaryStatus::Present,
            precision: SummaryPrecision::Local,
            provenance: SummaryProvenance::NativeLocal,
            payload_digest: "1234567890abcdef".to_string(),
            tito_flows: vec![SummaryFlowEdge {
                from: FlowRoot::Param(0),
                to: FlowRoot::Return,
                kind: FlowKind::Value,
            }],
            stable_key: stable_key_for_test("summary:target:tito"),
        }
    }
}

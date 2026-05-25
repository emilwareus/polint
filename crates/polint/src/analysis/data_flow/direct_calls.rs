use super::facts::{
    DataFlowAlgorithm, DataFlowConfidence, DataFlowEdgeFact, DataFlowEdgeKind, DataFlowNodeFact,
    DataFlowNodeKind, DataFlowPrecision, DataFlowProvenance, DataFlowStatus, DataFlowValidation,
};
use super::store::{DataFlowOutput, next_data_flow_edge_id, next_data_flow_node_id};
use crate::analysis::calls::facts::{CallSiteFact, CallTargetStatus};
use crate::analysis::ids::{CallSiteId, DataFlowBudgetId, DataFlowNodeId, PlaceId};
use crate::analysis::refined_calls::facts::{RefinedCallConfidence, RefinedCallEdgeFact};
use crate::analysis::summaries::facts::{
    SummaryDomainKind, SummaryFact, SummaryPrecision, SummaryStatus,
};
use crate::analysis_kernel::{FactFamily, stable_key_from_parts};
use crate::core::{AnalysisDb, Language};

pub(crate) fn derive_direct_call_edges(db: &AnalysisDb, output: &mut DataFlowOutput) {
    for edge in db.refined_call_edges() {
        if edge.status == CallTargetStatus::Resolved {
            derive_resolved_call_edge(db, output, edge);
        } else {
            derive_unresolved_call_edge(output, edge);
        }
    }
}

fn derive_resolved_call_edge(
    db: &AnalysisDb,
    output: &mut DataFlowOutput,
    edge: &RefinedCallEdgeFact,
) {
    let site = db.call_sites().iter().find(|site| site.id == edge.site);
    let site_id = site.map(|site| site.id);
    let callee_input = call_node(
        output,
        DataFlowNodeKind::SummaryInput,
        edge,
        "callee-input".to_string(),
        site_id,
    );
    let callee_output = call_node(
        output,
        DataFlowNodeKind::SummaryOutput,
        edge,
        "callee-output".to_string(),
        site_id,
    );

    let argument_nodes = argument_nodes(output, edge, site);
    for (index, argument) in argument_nodes.into_iter().enumerate() {
        push_edge(
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
    if let Some(receiver) = receiver_node(output, edge, site) {
        push_edge(
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
    bridge_target_summaries(db, output, edge, callee_input, callee_output, site_id);
    if let Some(returned) = return_node(output, site) {
        push_edge(
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
    db: &AnalysisDb,
    output: &mut DataFlowOutput,
    edge: &RefinedCallEdgeFact,
    callee_input: DataFlowNodeId,
    callee_output: DataFlowNodeId,
    call_site: Option<CallSiteId>,
) {
    let Some(target_function) = edge.target_function else {
        return;
    };

    for summary in db.summary_facts().iter().filter(|summary| {
        summary.function == target_function
            && summary.domain == SummaryDomainKind::DataFlowTito
            && summary.status == SummaryStatus::Present
    }) {
        let summary_input = summary_node(output, summary, DataFlowNodeKind::SummaryInput, "input");
        let summary_output =
            summary_node(output, summary, DataFlowNodeKind::SummaryOutput, "output");
        let summary_inputs = vec![
            summary.stable_key.clone(),
            summary.callable_stable_key.clone(),
        ];
        push_edge(
            output,
            CallEdgeDraft {
                from: callee_input,
                to: summary_input,
                kind: DataFlowEdgeKind::SummaryProjected,
                status: DataFlowStatus::Present,
                precision: summary_precision(summary.precision),
                validation: DataFlowValidation::ReferentiallyValidated,
                budget: None,
                evidence: vec![
                    "direct_call_summary_input_bridge".to_string(),
                    format!("summary={}", summary.stable_key),
                ],
                extra_input_stable_keys: summary_inputs.clone(),
                call_site,
                edge,
            },
        );
        push_edge(
            output,
            CallEdgeDraft {
                from: summary_output,
                to: callee_output,
                kind: DataFlowEdgeKind::SummaryProjected,
                status: DataFlowStatus::Present,
                precision: summary_precision(summary.precision),
                validation: DataFlowValidation::ReferentiallyValidated,
                budget: None,
                evidence: vec![
                    "direct_call_summary_output_bridge".to_string(),
                    format!("summary={}", summary.stable_key),
                ],
                extra_input_stable_keys: summary_inputs,
                call_site,
                edge,
            },
        );
    }
}

fn argument_nodes(
    output: &mut DataFlowOutput,
    _edge: &RefinedCallEdgeFact,
    site: Option<&CallSiteFact>,
) -> Vec<DataFlowNodeId> {
    site.into_iter()
        .flat_map(|site| site.arguments.iter().copied())
        .filter_map(|place| place_node(output, place))
        .collect::<Vec<_>>()
}

fn receiver_node(
    output: &mut DataFlowOutput,
    _edge: &RefinedCallEdgeFact,
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

fn derive_unresolved_call_edge(output: &mut DataFlowOutput, edge: &RefinedCallEdgeFact) {
    let source = call_node(
        output,
        DataFlowNodeKind::CallArgument,
        edge,
        "unresolved-argument".to_string(),
        None,
    );
    let sink = call_node(
        output,
        DataFlowNodeKind::Synthetic,
        edge,
        "unresolved-call".to_string(),
        None,
    );
    let status = unresolved_status(edge.status);
    let budget = (status == DataFlowStatus::BudgetExceeded).then(|| {
        super::local::budget_fact(
            super::facts::DataFlowBudgetReason::PathCount,
            1,
            2,
            &edge.stable_key,
            output,
        )
    });
    push_edge(
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
            keys.push(draft.edge.stable_key.clone());
            keys.extend(draft.extra_input_stable_keys);
            keys
        },
        stable_key,
    });
}

fn summary_node(
    output: &mut DataFlowOutput,
    fact: &SummaryFact,
    kind: DataFlowNodeKind,
    role: &str,
) -> DataFlowNodeId {
    let stable_key = stable_key_from_parts(
        FactFamily::DataFlowNode,
        &[
            ("kind", format!("{kind:?}")),
            ("summary", fact.stable_key.clone()),
            ("role", role.to_string()),
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
        call_site: None,
        model: None,
        span: None,
        stable_key,
    });
    id
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
    call_site: Option<CallSiteId>,
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
        call_site,
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
    use super::*;
    use crate::analysis::calls::facts::{
        CallAlgorithm, CallCallee, CallEdgeKind, CallPrecision, CallProvenance, CallSiteFact,
        CallSyntaxKind, UnresolvedCallReason,
    };
    use crate::analysis::calls::store::CallOutput;
    use crate::analysis::data_flow::query::{DataFlowPathStatus, DataFlowSearchBudget, find_paths};
    use crate::analysis::data_flow::store::DataFlowStore;
    use crate::analysis::ids::{
        CallSiteId, CallTargetId, DataFlowNodeId, MirBodyId, MirOpId, PlaceId, RefinedCallEdgeId,
        SummaryId,
    };
    use crate::analysis::refined_calls::facts::{
        RefinedCallEdgeFact, RefinedCallTier, RefinedCallValidation,
    };
    use crate::analysis::refined_calls::store::RefinedCallOutput;
    use crate::analysis::summaries::facts::{
        SummaryDomainKind, SummaryFact, SummaryPrecision, SummaryProvenance, SummaryStatus,
    };
    use crate::analysis::summaries::store::SummaryOutput;
    use crate::core::{FileId, FunctionId, Language, Span};

    #[test]
    fn resolved_refined_call_creates_role_specific_edges() {
        let mut db = AnalysisDb::default();
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
        derive_unresolved_call_edge(&mut output, &refined_edge(CallTargetStatus::Unresolved));

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
        let mut db = AnalysisDb::default();
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
        let mut db = AnalysisDb::default();
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
        let mut db = AnalysisDb::default();
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
        let store = DataFlowStore::from_output(output).expect("valid store");
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
        let paths = find_paths(
            &store,
            source,
            sink,
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
            derive_unresolved_call_edge(&mut output, &refined_edge(call_status));

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
            stable_key: "refined:edge".to_string(),
        }
    }

    fn call_site(receiver: Option<PlaceId>) -> CallSiteFact {
        CallSiteFact {
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
            stable_key: "call-site:2".to_string(),
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
            stable_key: format!("node:place:{}", place.0),
        }
    }

    fn summary_fact() -> SummaryFact {
        SummaryFact {
            id: SummaryId(1),
            callable_stable_key: "callable:target".to_string(),
            function: FunctionId(5),
            domain: SummaryDomainKind::DataFlowTito,
            status: SummaryStatus::Present,
            precision: SummaryPrecision::Local,
            provenance: SummaryProvenance::NativeLocal,
            payload_digest: "1234567890abcdef".to_string(),
            stable_key: "summary:target:tito".to_string(),
        }
    }
}

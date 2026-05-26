use std::collections::{BTreeMap, BTreeSet};

use super::facts::{
    DataFlowAlgorithm, DataFlowBudgetFact, DataFlowBudgetReason, DataFlowConfidence,
    DataFlowEdgeFact, DataFlowEdgeKind, DataFlowNodeFact, DataFlowNodeKind, DataFlowPrecision,
    DataFlowProvenance, DataFlowStatus, DataFlowValidation,
};
use super::store::{
    DataFlowOutput, next_data_flow_budget_id, next_data_flow_edge_id, next_data_flow_node_id,
};
use crate::analysis::ids::{DataFlowBudgetId, DataFlowNodeId, MirBodyId, PlaceId};
use crate::analysis::mir::op::{
    AssignMode, ConservativeAction, MirOperation, MirOperationKind, MirValue,
};
use crate::analysis::places::{PlaceFact, PlaceProjection};
use crate::analysis_kernel::{FactFamily, stable_key_from_parts};
use crate::core::{AnalysisDb, FunctionId, Language};

pub(crate) fn derive_local_value_flow(db: &AnalysisDb, output: &mut DataFlowOutput) {
    let mut builder = LocalFlowBuilder::new(db, output);
    builder.derive();
}

struct LocalFlowBuilder<'a, 'b> {
    db: &'a AnalysisDb,
    output: &'b mut DataFlowOutput,
    place_nodes: BTreeMap<PlaceId, DataFlowNodeId>,
    emitted_edges: BTreeSet<String>,
    branchy_bodies: BTreeSet<MirBodyId>,
}

impl<'a, 'b> LocalFlowBuilder<'a, 'b> {
    fn new(db: &'a AnalysisDb, output: &'b mut DataFlowOutput) -> Self {
        let place_nodes = output
            .nodes
            .iter()
            .filter_map(|node| node.place.map(|place| (place, node.id)))
            .collect();
        let branchy_bodies = db
            .mir_operations()
            .iter()
            .filter_map(|operation| {
                matches!(operation.kind, MirOperationKind::Branch { .. }).then_some(operation.body)
            })
            .collect();
        Self {
            db,
            output,
            place_nodes,
            emitted_edges: BTreeSet::new(),
            branchy_bodies,
        }
    }

    fn derive(&mut self) {
        for operation in self.db.mir_operations() {
            match &operation.kind {
                MirOperationKind::Bind { place, value } => {
                    self.edge_from_value(
                        operation,
                        value,
                        *place,
                        DataFlowEdgeKind::LocalBinding,
                        "bind",
                    );
                }
                MirOperationKind::Assign { place, value, mode } => {
                    let kind = match mode {
                        AssignMode::ProjectionMutation => self.projection_edge_kind(*place),
                        AssignMode::UnknownWrite => DataFlowEdgeKind::HavocFlow,
                        AssignMode::DeclarationBinding => DataFlowEdgeKind::LocalBinding,
                        AssignMode::PartialWrite => DataFlowEdgeKind::LocalWrite,
                        AssignMode::Overwrite | AssignMode::Simultaneous => {
                            DataFlowEdgeKind::LocalAssignment
                        }
                    };
                    self.edge_from_value(
                        operation,
                        value,
                        *place,
                        kind,
                        &format!("assign_mode={mode:?}"),
                    );
                }
                MirOperationKind::Write { place, value } => {
                    self.edge_from_value(
                        operation,
                        value,
                        *place,
                        DataFlowEdgeKind::LocalWrite,
                        "write",
                    );
                }
                MirOperationKind::Read { place } => {
                    if let Some(from) = self.node_for_place(*place) {
                        let to = self.operation_node(
                            operation,
                            DataFlowNodeKind::Value,
                            format!("read:{}", operation.stable_key),
                        );
                        self.push_edge(EdgeDraft {
                            operation,
                            from,
                            to,
                            kind: DataFlowEdgeKind::LocalRead,
                            status: DataFlowStatus::Present,
                            precision: DataFlowPrecision::Syntax,
                            validation: DataFlowValidation::Native,
                            provenance: DataFlowProvenance::Native,
                            budget: None,
                            evidence: vec!["read".to_string()],
                            input_stable_keys: vec![
                                self.place_key(*place),
                                operation.stable_key.clone(),
                            ],
                        });
                    }
                }
                MirOperationKind::Return { value: Some(value) } => {
                    if let Some(from) = self.node_for_value(operation, value) {
                        let to = self.operation_node(
                            operation,
                            DataFlowNodeKind::SummaryOutput,
                            format!("return:{}", operation.stable_key),
                        );
                        self.push_edge(EdgeDraft {
                            operation,
                            from,
                            to,
                            kind: DataFlowEdgeKind::ReturnValue,
                            status: DataFlowStatus::Present,
                            precision: DataFlowPrecision::Syntax,
                            validation: DataFlowValidation::Native,
                            provenance: DataFlowProvenance::Native,
                            budget: None,
                            evidence: vec!["return_value".to_string()],
                            input_stable_keys: vec![operation.stable_key.clone()],
                        });
                    }
                }
                MirOperationKind::Call {
                    site,
                    arguments,
                    return_place,
                    ..
                } => {
                    let return_node = self.node_for_place(*return_place);
                    for (index, argument) in arguments.iter().enumerate() {
                        if let (Some(from), Some(to)) =
                            (self.node_for_place(*argument), return_node)
                        {
                            self.push_edge(EdgeDraft {
                                operation,
                                from,
                                to,
                                kind: DataFlowEdgeKind::CallArgumentToReturn,
                                status: DataFlowStatus::Present,
                                precision: DataFlowPrecision::Conservative,
                                validation: DataFlowValidation::ReferentiallyValidated,
                                provenance: DataFlowProvenance::Native,
                                budget: None,
                                evidence: vec![
                                    format!("local_call_argument_index={index}"),
                                    format!("call_site={}", site.0),
                                ],
                                input_stable_keys: vec![
                                    self.place_key(*argument),
                                    self.place_key(*return_place),
                                    operation.stable_key.clone(),
                                ],
                            });
                        }
                    }
                }
                MirOperationKind::Unsupported { unsupported } => {
                    self.emit_unsupported_operation(operation, *unsupported);
                }
                MirOperationKind::StorageLive { .. }
                | MirOperationKind::Branch { .. }
                | MirOperationKind::Return { value: None } => {}
            }
        }
    }

    fn edge_from_value(
        &mut self,
        operation: &MirOperation,
        value: &MirValue,
        target: PlaceId,
        kind: DataFlowEdgeKind,
        evidence: &str,
    ) {
        let Some(to) = self.node_for_place(target) else {
            return;
        };
        if matches!(value, MirValue::Literal { .. }) {
            if !self.branchy_bodies.contains(&operation.body) {
                self.kill_incoming_overwrite_flows(to, kind);
            }
            return;
        }
        let status =
            if matches!(value, MirValue::Unknown { .. }) || kind == DataFlowEdgeKind::HavocFlow {
                DataFlowStatus::Unknown
            } else {
                DataFlowStatus::Present
            };
        let from = self.node_for_value(operation, value).unwrap_or_else(|| {
            self.operation_node(
                operation,
                DataFlowNodeKind::Synthetic,
                format!("unknown-source:{}", operation.stable_key),
            )
        });
        self.push_edge(EdgeDraft {
            operation,
            from,
            to,
            kind,
            status,
            precision: if status == DataFlowStatus::Present {
                DataFlowPrecision::Syntax
            } else {
                DataFlowPrecision::Unknown
            },
            validation: DataFlowValidation::Native,
            provenance: DataFlowProvenance::Native,
            budget: None,
            evidence: vec![evidence.to_string(), value_evidence(value)],
            input_stable_keys: vec![self.place_key(target), operation.stable_key.clone()],
        });
    }

    fn emit_unsupported_operation(
        &mut self,
        operation: &MirOperation,
        unsupported: crate::analysis::ids::UnsupportedId,
    ) {
        let unsupported_fact = self
            .db
            .unsupported_semantics()
            .iter()
            .find(|fact| fact.id == unsupported);
        let affected_places = unsupported_fact
            .map(|fact| fact.affected_places.as_slice())
            .unwrap_or(&[]);
        let action = unsupported_fact
            .map(|fact| fact.conservative_action)
            .unwrap_or(ConservativeAction::PreserveWithUnknownValue);

        for place in affected_places {
            let Some(node) = self.node_for_place(*place) else {
                continue;
            };
            self.push_edge(EdgeDraft {
                operation,
                from: node,
                to: node,
                kind: match action {
                    ConservativeAction::HavocAffectedPlaces | ConservativeAction::StopLowering => {
                        DataFlowEdgeKind::HavocFlow
                    }
                    ConservativeAction::SkipOperation
                    | ConservativeAction::PreserveWithUnknownValue => DataFlowEdgeKind::UnknownFlow,
                },
                status: DataFlowStatus::Unsupported,
                precision: DataFlowPrecision::Unknown,
                validation: DataFlowValidation::Native,
                provenance: DataFlowProvenance::Native,
                budget: None,
                evidence: vec![
                    "unsupported_semantic".to_string(),
                    unsupported_fact
                        .map(|fact| fact.construct.clone())
                        .unwrap_or_else(|| "missing_unsupported_fact".to_string()),
                ],
                input_stable_keys: vec![self.place_key(*place), operation.stable_key.clone()],
            });
        }
    }

    fn kill_incoming_overwrite_flows(
        &mut self,
        target_node: DataFlowNodeId,
        kind: DataFlowEdgeKind,
    ) {
        if !matches!(
            kind,
            DataFlowEdgeKind::LocalAssignment | DataFlowEdgeKind::LocalBinding
        ) {
            return;
        }

        let removed = self
            .output
            .edges
            .iter()
            .filter(|edge| {
                edge.algorithm == DataFlowAlgorithm::LocalMir
                    && edge.to == target_node
                    && edge.status == DataFlowStatus::Present
                    && matches!(
                        edge.kind,
                        DataFlowEdgeKind::LocalAssignment | DataFlowEdgeKind::LocalBinding
                    )
            })
            .map(|edge| edge.stable_key.clone())
            .collect::<BTreeSet<_>>();
        if removed.is_empty() {
            return;
        }

        self.output
            .edges
            .retain(|edge| !removed.contains(&edge.stable_key));
        for stable_key in &removed {
            self.emitted_edges.remove(stable_key);
        }
    }

    fn projection_edge_kind(&self, place: PlaceId) -> DataFlowEdgeKind {
        let Some(place) = self.db.mir_places().iter().find(|fact| fact.id == place) else {
            return DataFlowEdgeKind::FieldProjection;
        };
        if place.projections.iter().any(|projection| {
            matches!(
                projection,
                PlaceProjection::IndexKnown(_) | PlaceProjection::IndexUnknown { .. }
            )
        }) {
            DataFlowEdgeKind::IndexProjection
        } else {
            DataFlowEdgeKind::FieldProjection
        }
    }

    fn node_for_value(
        &mut self,
        operation: &MirOperation,
        value: &MirValue,
    ) -> Option<DataFlowNodeId> {
        match value {
            MirValue::Place(place) => self.node_for_place(*place),
            MirValue::CallReturn(site) => Some(self.operation_node(
                operation,
                DataFlowNodeKind::CallReturn,
                format!("call-return:{}", site.0),
            )),
            MirValue::Temporary(value_id) => Some(self.operation_node(
                operation,
                DataFlowNodeKind::Value,
                format!("temporary:{}", value_id.0),
            )),
            MirValue::Unknown { evidence } => Some(self.operation_node(
                operation,
                DataFlowNodeKind::Synthetic,
                format!("unknown:{evidence}:{}", operation.stable_key),
            )),
            MirValue::Literal { .. } => None,
        }
    }

    fn node_for_place(&self, place: PlaceId) -> Option<DataFlowNodeId> {
        self.place_nodes.get(&place).copied()
    }

    fn operation_node(
        &mut self,
        operation: &MirOperation,
        kind: DataFlowNodeKind,
        suffix: String,
    ) -> DataFlowNodeId {
        let stable_key = stable_key_from_parts(
            FactFamily::DataFlowNode,
            &[
                ("operation", operation.stable_key.clone()),
                ("node", suffix),
            ],
        );
        if let Some(node) = self
            .output
            .nodes
            .iter()
            .find(|node| node.stable_key == stable_key)
            .map(|node| node.id)
        {
            return node;
        }

        let (language, file, function) = self.body_context(operation.body);
        let id = next_data_flow_node_id(&self.output.nodes);
        self.output.nodes.push(DataFlowNodeFact {
            id,
            kind,
            language,
            file,
            function,
            body: Some(operation.body),
            operation: Some(operation.id),
            cfg_node: None,
            place: None,
            symbol: None,
            reference: None,
            call_site: None,
            model: None,
            span: Some(operation.span.clone()),
            stable_key,
        });
        id
    }

    fn push_edge(&mut self, draft: EdgeDraft<'_>) {
        let stable_key = stable_key_from_parts(
            FactFamily::DataFlowEdge,
            &[
                ("kind", format!("{:?}", draft.kind)),
                ("operation", draft.operation.stable_key.clone()),
                ("from", self.node_key(draft.from)),
                ("to", self.node_key(draft.to)),
                ("status", format!("{:?}", draft.status)),
            ],
        );
        if !self.emitted_edges.insert(stable_key.clone())
            || self
                .output
                .edges
                .iter()
                .any(|edge| edge.stable_key == stable_key)
        {
            return;
        }
        self.output.edges.push(DataFlowEdgeFact {
            id: next_data_flow_edge_id(&self.output.edges),
            from: draft.from,
            to: draft.to,
            kind: draft.kind,
            algorithm: DataFlowAlgorithm::LocalMir,
            status: draft.status,
            precision: draft.precision,
            validation: draft.validation,
            confidence: if draft.status == DataFlowStatus::Present {
                DataFlowConfidence::High
            } else {
                DataFlowConfidence::Low
            },
            provenance: draft.provenance,
            call_site: None,
            call_target: None,
            refined_call: None,
            model: None,
            budget: draft.budget,
            evidence: draft.evidence,
            input_stable_keys: draft.input_stable_keys,
            stable_key,
        });
    }

    fn body_context(
        &self,
        body: MirBodyId,
    ) -> (Language, Option<crate::core::FileId>, Option<FunctionId>) {
        self.db
            .mir_bodies()
            .iter()
            .find(|fact| fact.id == body)
            .map(|fact| (fact.language, Some(fact.file), Some(fact.function)))
            .unwrap_or((Language::Unknown, None, None))
    }

    fn place_key(&self, place: PlaceId) -> String {
        self.db
            .mir_places()
            .iter()
            .find(|fact| fact.id == place)
            .map(|place| place.stable_key.clone())
            .unwrap_or_else(|| format!("place:{}", place.0))
    }

    fn node_key(&self, node: DataFlowNodeId) -> String {
        self.output
            .nodes
            .iter()
            .find(|fact| fact.id == node)
            .map(|fact| fact.stable_key.clone())
            .unwrap_or_else(|| format!("node:{}", node.0))
    }
}

struct EdgeDraft<'a> {
    operation: &'a MirOperation,
    from: DataFlowNodeId,
    to: DataFlowNodeId,
    kind: DataFlowEdgeKind,
    status: DataFlowStatus,
    precision: DataFlowPrecision,
    validation: DataFlowValidation,
    provenance: DataFlowProvenance,
    budget: Option<DataFlowBudgetId>,
    evidence: Vec<String>,
    input_stable_keys: Vec<String>,
}

pub(crate) fn budget_fact(
    reason: DataFlowBudgetReason,
    limit: u64,
    observed: u64,
    context: &str,
    output: &mut DataFlowOutput,
) -> DataFlowBudgetId {
    let stable_key = stable_key_from_parts(
        FactFamily::DataFlowBudget,
        &[
            ("reason", format!("{reason:?}")),
            ("limit", limit.to_string()),
            ("observed", observed.to_string()),
            ("context", context.to_string()),
        ],
    );
    if let Some(existing) = output
        .budgets
        .iter()
        .find(|budget| budget.stable_key == stable_key)
        .map(|budget| budget.id)
    {
        return existing;
    }
    let id = next_data_flow_budget_id(&output.budgets);
    output.budgets.push(DataFlowBudgetFact {
        id,
        reason,
        limit,
        observed,
        status: DataFlowStatus::BudgetExceeded,
        stable_key,
    });
    id
}

fn value_evidence(value: &MirValue) -> String {
    match value {
        MirValue::Literal { .. } => "literal".to_string(),
        MirValue::Place(place) => format!("source_place={}", place.0),
        MirValue::Temporary(value) => format!("temporary={}", value.0),
        MirValue::CallReturn(site) => format!("call_return={}", site.0),
        MirValue::Unknown { evidence } => format!("unknown={evidence}"),
    }
}

pub(crate) fn node_from_place(
    output: &DataFlowOutput,
    place: &PlaceFact,
    db: &AnalysisDb,
) -> DataFlowNodeFact {
    DataFlowNodeFact {
        id: next_data_flow_node_id(&output.nodes),
        kind: DataFlowNodeKind::Place,
        language: place.language,
        file: place.file,
        function: place.function,
        body: None,
        operation: None,
        cfg_node: None,
        place: Some(place.id),
        symbol: None,
        reference: None,
        call_site: None,
        model: None,
        span: None,
        stable_key: db
            .metadata_for(crate::analysis_kernel::FactRef::new(
                FactFamily::Place,
                place.id.0,
            ))
            .map(|metadata| {
                stable_key_from_parts(
                    FactFamily::DataFlowNode,
                    &[("place", metadata.stable_key.clone())],
                )
            })
            .unwrap_or_else(|| {
                stable_key_from_parts(
                    FactFamily::DataFlowNode,
                    &[("place_id", place.id.0.to_string())],
                )
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ids::{MirBodyId, MirOpId, MirPredicateId, PlaceId};
    use crate::analysis::mir::body::{MirBody, MirOutput, MirStatus};
    use crate::analysis::mir::op::{AssignMode, MirOperation, MirOperationKind, MirValue};
    use crate::analysis::places::{PlaceFact, PlaceProjection, PlaceRoot, PlaceStatus};
    use crate::core::{AnalysisDb, FileId, FunctionId, Language, Span};

    #[test]
    fn local_builder_derives_parameter_local_return_flow() {
        let mut db = AnalysisDb::default();
        db.replace_semantic_mir(MirOutput {
            bodies: vec![body()],
            places: vec![
                place(
                    0,
                    PlaceRoot::Parameter {
                        function: FunctionId(1),
                        index: 0,
                        name: Some("input".to_string()),
                    },
                    Vec::new(),
                ),
                place(
                    1,
                    PlaceRoot::Local {
                        function: FunctionId(1),
                        name: "local".to_string(),
                    },
                    Vec::new(),
                ),
            ],
            operations: vec![
                op(
                    0,
                    MirOperationKind::Bind {
                        place: PlaceId(1),
                        value: MirValue::Place(PlaceId(0)),
                    },
                ),
                op(
                    1,
                    MirOperationKind::Return {
                        value: Some(MirValue::Place(PlaceId(1))),
                    },
                ),
            ],
            unsupported: Vec::new(),
        })
        .expect("valid MIR");
        let mut output = DataFlowOutput::empty();
        push_place_nodes(&db, &mut output);

        derive_local_value_flow(&db, &mut output);

        assert!(
            output
                .edges
                .iter()
                .any(|edge| edge.kind == DataFlowEdgeKind::LocalBinding)
        );
        assert!(
            output
                .edges
                .iter()
                .any(|edge| edge.kind == DataFlowEdgeKind::ReturnValue)
        );
    }

    #[test]
    fn projection_mutation_emits_projection_edge_with_place_evidence() {
        let mut db = AnalysisDb::default();
        db.replace_semantic_mir(MirOutput {
            bodies: vec![body()],
            places: vec![
                place(
                    0,
                    PlaceRoot::Local {
                        function: FunctionId(1),
                        name: "source".to_string(),
                    },
                    Vec::new(),
                ),
                place(
                    1,
                    PlaceRoot::Local {
                        function: FunctionId(1),
                        name: "target".to_string(),
                    },
                    vec![PlaceProjection::Property("field".to_string())],
                ),
            ],
            operations: vec![op(
                0,
                MirOperationKind::Assign {
                    place: PlaceId(1),
                    value: MirValue::Place(PlaceId(0)),
                    mode: AssignMode::ProjectionMutation,
                },
            )],
            unsupported: Vec::new(),
        })
        .expect("valid MIR");
        let mut output = DataFlowOutput::empty();
        push_place_nodes(&db, &mut output);

        derive_local_value_flow(&db, &mut output);

        let edge = output
            .edges
            .iter()
            .find(|edge| edge.kind == DataFlowEdgeKind::FieldProjection)
            .expect("field projection edge");
        assert!(
            edge.input_stable_keys
                .iter()
                .any(|key| key.contains("target"))
        );
    }

    #[test]
    fn unknown_write_emits_havoc_or_unknown_edge() {
        let mut db = AnalysisDb::default();
        db.replace_semantic_mir(MirOutput {
            bodies: vec![body()],
            places: vec![place(
                0,
                PlaceRoot::Local {
                    function: FunctionId(1),
                    name: "target".to_string(),
                },
                Vec::new(),
            )],
            operations: vec![op(
                0,
                MirOperationKind::Assign {
                    place: PlaceId(0),
                    value: MirValue::Unknown {
                        evidence: "dynamic".to_string(),
                    },
                    mode: AssignMode::UnknownWrite,
                },
            )],
            unsupported: Vec::new(),
        })
        .expect("valid MIR");
        let mut output = DataFlowOutput::empty();
        push_place_nodes(&db, &mut output);

        derive_local_value_flow(&db, &mut output);

        let edge = output
            .edges
            .iter()
            .find(|edge| edge.kind == DataFlowEdgeKind::HavocFlow)
            .expect("havoc edge");
        assert_eq!(edge.status, DataFlowStatus::Unknown);
        assert!(!edge.evidence.is_empty());
    }

    #[test]
    fn literal_assignment_does_not_emit_unknown_source_present_flow() {
        let mut db = AnalysisDb::default();
        db.replace_semantic_mir(MirOutput {
            bodies: vec![body()],
            places: vec![place(
                0,
                PlaceRoot::Local {
                    function: FunctionId(1),
                    name: "target".to_string(),
                },
                Vec::new(),
            )],
            operations: vec![op(
                0,
                MirOperationKind::Assign {
                    place: PlaceId(0),
                    value: MirValue::Literal {
                        value: "\"safe\"".to_string(),
                    },
                    mode: AssignMode::Overwrite,
                },
            )],
            unsupported: Vec::new(),
        })
        .expect("valid MIR");
        let mut output = DataFlowOutput::empty();
        push_place_nodes(&db, &mut output);

        derive_local_value_flow(&db, &mut output);

        assert!(
            output.edges.is_empty(),
            "literal assignments should not create synthetic source-to-place flows"
        );
    }

    #[test]
    fn literal_overwrite_kills_stale_incoming_local_flow() {
        let mut db = AnalysisDb::default();
        db.replace_semantic_mir(MirOutput {
            bodies: vec![body()],
            places: vec![
                place(
                    0,
                    PlaceRoot::Parameter {
                        function: FunctionId(1),
                        index: 0,
                        name: Some("input".to_string()),
                    },
                    Vec::new(),
                ),
                place(
                    1,
                    PlaceRoot::Local {
                        function: FunctionId(1),
                        name: "local".to_string(),
                    },
                    Vec::new(),
                ),
            ],
            operations: vec![
                op(
                    0,
                    MirOperationKind::Bind {
                        place: PlaceId(1),
                        value: MirValue::Place(PlaceId(0)),
                    },
                ),
                op(
                    1,
                    MirOperationKind::Assign {
                        place: PlaceId(1),
                        value: MirValue::Literal {
                            value: "\"safe\"".to_string(),
                        },
                        mode: AssignMode::Overwrite,
                    },
                ),
                op(
                    2,
                    MirOperationKind::Return {
                        value: Some(MirValue::Place(PlaceId(1))),
                    },
                ),
            ],
            unsupported: Vec::new(),
        })
        .expect("valid MIR");
        let mut output = DataFlowOutput::empty();
        push_place_nodes(&db, &mut output);

        derive_local_value_flow(&db, &mut output);

        assert!(
            !output.edges.iter().any(|edge| {
                edge.kind == DataFlowEdgeKind::LocalBinding
                    && edge.from == DataFlowNodeId(0)
                    && edge.to == DataFlowNodeId(1)
            }),
            "literal overwrite should clear stale parameter-to-local flow"
        );
        assert!(
            output
                .edges
                .iter()
                .any(|edge| edge.kind == DataFlowEdgeKind::ReturnValue),
            "the later return edge should still be represented"
        );
    }

    #[test]
    fn literal_overwrite_in_branchy_body_preserves_possible_incoming_local_flow() {
        let mut db = AnalysisDb::default();
        db.replace_semantic_mir(MirOutput {
            bodies: vec![body()],
            places: vec![
                place(
                    0,
                    PlaceRoot::Parameter {
                        function: FunctionId(1),
                        index: 0,
                        name: Some("input".to_string()),
                    },
                    Vec::new(),
                ),
                place(
                    1,
                    PlaceRoot::Local {
                        function: FunctionId(1),
                        name: "local".to_string(),
                    },
                    Vec::new(),
                ),
            ],
            operations: vec![
                op(
                    0,
                    MirOperationKind::Bind {
                        place: PlaceId(1),
                        value: MirValue::Place(PlaceId(0)),
                    },
                ),
                op(
                    1,
                    MirOperationKind::Branch {
                        predicate: MirPredicateId(1),
                        predicate_place: Some(PlaceId(0)),
                    },
                ),
                op(
                    2,
                    MirOperationKind::Assign {
                        place: PlaceId(1),
                        value: MirValue::Literal {
                            value: "\"safe\"".to_string(),
                        },
                        mode: AssignMode::Overwrite,
                    },
                ),
                op(
                    3,
                    MirOperationKind::Return {
                        value: Some(MirValue::Place(PlaceId(1))),
                    },
                ),
            ],
            unsupported: Vec::new(),
        })
        .expect("valid MIR");
        let mut output = DataFlowOutput::empty();
        push_place_nodes(&db, &mut output);

        derive_local_value_flow(&db, &mut output);

        assert!(
            output.edges.iter().any(|edge| {
                edge.kind == DataFlowEdgeKind::LocalBinding
                    && edge.from == DataFlowNodeId(0)
                    && edge.to == DataFlowNodeId(1)
            }),
            "branchy bodies are not path-sensitive, so a literal overwrite must not erase a possible parameter flow"
        );
    }

    fn body() -> MirBody {
        MirBody {
            id: MirBodyId(1),
            language: Language::TypeScript,
            file: FileId(1),
            function: FunctionId(1),
            package: None,
            module: None,
            owner_stable_key: "function:one".to_string(),
            span: span(),
            stable_key: "body:one".to_string(),
            status: MirStatus::Resolved,
        }
    }

    fn push_place_nodes(db: &AnalysisDb, output: &mut DataFlowOutput) {
        for place in db.mir_places() {
            let node = node_from_place(output, place, db);
            output.nodes.push(node);
        }
    }

    fn op(ordinal: u32, kind: MirOperationKind) -> MirOperation {
        MirOperation {
            id: MirOpId(u64::from(ordinal)),
            body: MirBodyId(1),
            ordinal,
            span: span(),
            kind,
            stable_key: format!("op:{ordinal}"),
            status: MirStatus::Resolved,
        }
    }

    fn place(id: u64, root: PlaceRoot, projections: Vec<PlaceProjection>) -> PlaceFact {
        PlaceFact {
            id: PlaceId(id),
            language: Language::TypeScript,
            file: Some(FileId(1)),
            function: Some(FunctionId(1)),
            root,
            projections,
            stable_key: format!("place:{id}:target"),
            status: PlaceStatus::Resolved,
        }
    }

    fn span() -> Span {
        Span::point(FileId(1), 1, 1)
    }
}

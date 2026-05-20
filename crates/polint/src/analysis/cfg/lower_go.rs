use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::cfg::builder::CfgBuilder;
use crate::analysis::cfg::facts::{
    BasicBlockKind, CfgEdgeKind, CfgNodeKind, CfgPrecision, CfgStatus, ControlFlowAction,
    UnsupportedControlFlowFact,
};
use crate::analysis::cfg::ids::{CfgFunctionId, UnsupportedControlFlowId};
use crate::analysis::cfg::store::CfgOutput;
use crate::analysis::ids::{MirBodyId, MirOpId, UnsupportedId};
use crate::analysis::mir::body::MirStatus;
use crate::analysis::mir::op::{
    ConservativeAction, MirOperation, MirOperationKind, UnsupportedDomain, UnsupportedPrecision,
    UnsupportedSemanticFact,
};
use crate::analysis::stable_key::semantic_stable_key;
use crate::analysis_kernel::FactFamily;
use crate::core::{AnalysisDb, Language, SourceFile, Span};

pub(crate) fn lower_go_cfg(db: &AnalysisDb) -> CfgOutput {
    let mut lowering = GoCfgLowering::new(db);
    lowering.lower();
    lowering.finish()
}

struct GoCfgLowering<'db> {
    db: &'db AnalysisDb,
    builder: CfgBuilder,
    body_to_function: BTreeMap<MirBodyId, CfgFunctionId>,
}

impl<'db> GoCfgLowering<'db> {
    fn new(db: &'db AnalysisDb) -> Self {
        Self {
            db,
            builder: CfgBuilder::new(),
            body_to_function: BTreeMap::new(),
        }
    }

    fn lower(&mut self) {
        let mut bodies = self
            .db
            .mir_bodies()
            .iter()
            .filter(|body| body.language == Language::Go)
            .collect::<Vec<_>>();
        bodies.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));

        for body in bodies {
            let has_exceptional_control = self
                .unsupported_for_body(body.id)
                .any(|row| matches!(row.construct.as_str(), "panic" | "recover" | "ERROR"));
            let function = self.builder.start_function(body, has_exceptional_control);
            self.body_to_function.insert(body.id, function);
            self.lower_body(body.id);
            self.builder.finish_function();
        }
    }

    fn lower_body(&mut self, body: MirBodyId) {
        let mut operations = self
            .db
            .mir_operations()
            .iter()
            .filter(|operation| operation.body == body)
            .collect::<Vec<_>>();
        operations.sort_by(|left, right| {
            (left.ordinal, left.stable_key.as_str())
                .cmp(&(right.ordinal, right.stable_key.as_str()))
        });

        let mut terminated = false;
        for operation in operations {
            if terminated {
                let block = self.builder.start_block(BasicBlockKind::Unreachable);
                self.builder.mark_unreachable(block);
            }

            match &operation.kind {
                MirOperationKind::Branch { .. } => {
                    self.lower_branch(operation);
                    terminated = false;
                }
                MirOperationKind::Return { .. } => {
                    self.builder.append_operation_node(
                        Some(operation),
                        CfgNodeKind::Return,
                        Some(operation.span.clone()),
                    );
                    let current = self.builder.current_block();
                    let exit = self.builder.normal_exit_block();
                    self.builder.add_edge(current, exit, CfgEdgeKind::Return);
                    terminated = true;
                }
                MirOperationKind::Call { .. } => {
                    let shape = self.call_shape(operation);
                    self.builder.append_operation_node(
                        Some(operation),
                        shape.node_kind,
                        Some(operation.span.clone()),
                    );
                    if let Some(edge_kind) = shape.exit_edge {
                        let current = self.builder.current_block();
                        let exit = if edge_kind == CfgEdgeKind::Panic {
                            self.builder
                                .exceptional_exit_block()
                                .unwrap_or_else(|| self.builder.normal_exit_block())
                        } else {
                            self.builder.normal_exit_block()
                        };
                        self.builder.add_edge(current, exit, edge_kind);
                        terminated = matches!(edge_kind, CfgEdgeKind::Panic);
                    }
                }
                MirOperationKind::Unsupported { unsupported } => {
                    let shape = self.unsupported_shape(*unsupported);
                    self.builder.append_operation_node(
                        Some(operation),
                        shape.node_kind,
                        Some(operation.span.clone()),
                    );
                    if let Some(edge_kind) = shape.boundary_edge {
                        let current = self.builder.current_block();
                        let next = self.builder.start_block(BasicBlockKind::Synthetic);
                        self.builder.append_operation_node(
                            None,
                            CfgNodeKind::Synthetic,
                            Some(operation.span.clone()),
                        );
                        self.builder.add_edge(current, next, edge_kind);
                    }
                    terminated = shape.stop_lowering;
                }
                MirOperationKind::StorageLive { .. }
                | MirOperationKind::Bind { .. }
                | MirOperationKind::Assign { .. }
                | MirOperationKind::Read { .. }
                | MirOperationKind::Write { .. } => {
                    self.builder.append_operation_node(
                        Some(operation),
                        CfgNodeKind::Operation,
                        Some(operation.span.clone()),
                    );
                    terminated = false;
                }
            }
        }
    }

    fn lower_branch(&mut self, operation: &MirOperation) {
        let shape = self.branch_shape(operation);
        self.builder.append_operation_node(
            Some(operation),
            CfgNodeKind::Condition,
            Some(operation.span.clone()),
        );
        let condition = self.builder.current_block();

        match shape {
            BranchShape::Loop => {
                let body = self.builder.start_block(BasicBlockKind::LoopBody);
                self.builder.append_operation_node(
                    None,
                    CfgNodeKind::Synthetic,
                    Some(operation.span.clone()),
                );
                self.builder
                    .add_edge(condition, body, CfgEdgeKind::LoopEnter);

                let join = self.builder.start_block(BasicBlockKind::Join);
                self.builder.append_operation_node(
                    None,
                    CfgNodeKind::Synthetic,
                    Some(operation.span.clone()),
                );
                self.builder
                    .add_edge(condition, join, CfgEdgeKind::LoopExit);
                self.builder
                    .add_edge(body, condition, CfgEdgeKind::LoopBack);
            }
            BranchShape::ShortCircuit => {
                let lhs_true = self.builder.start_block(BasicBlockKind::Branch);
                self.builder.append_operation_node(
                    None,
                    CfgNodeKind::Synthetic,
                    Some(operation.span.clone()),
                );
                self.builder
                    .add_edge(condition, lhs_true, CfgEdgeKind::True);

                let short_circuit = self.builder.start_block(BasicBlockKind::Join);
                self.builder.append_operation_node(
                    None,
                    CfgNodeKind::Synthetic,
                    Some(operation.span.clone()),
                );
                self.builder
                    .add_edge(condition, short_circuit, CfgEdgeKind::ShortCircuit);
                self.builder
                    .add_edge(lhs_true, short_circuit, CfgEdgeKind::Normal);
            }
            BranchShape::Conditional => {
                let then_block = self.builder.start_block(BasicBlockKind::Branch);
                self.builder.append_operation_node(
                    None,
                    CfgNodeKind::Synthetic,
                    Some(operation.span.clone()),
                );
                self.builder
                    .add_edge(condition, then_block, CfgEdgeKind::True);

                let else_block = self.builder.start_block(BasicBlockKind::Branch);
                self.builder.append_operation_node(
                    None,
                    CfgNodeKind::Synthetic,
                    Some(operation.span.clone()),
                );
                self.builder
                    .add_edge(condition, else_block, CfgEdgeKind::False);

                let join = self.builder.start_block(BasicBlockKind::Join);
                self.builder.append_operation_node(
                    None,
                    CfgNodeKind::Synthetic,
                    Some(operation.span.clone()),
                );
                self.builder.add_edge(then_block, join, CfgEdgeKind::Normal);
                self.builder.add_edge(else_block, join, CfgEdgeKind::Normal);
            }
        }
    }

    fn branch_shape(&self, operation: &MirOperation) -> BranchShape {
        let evidence = self.operation_evidence(operation);
        if contains_token(&evidence, &["for ", "range ", "for_statement"]) {
            BranchShape::Loop
        } else if contains_token(&evidence, &["&&", "||", "ShortCircuit"]) {
            BranchShape::ShortCircuit
        } else {
            BranchShape::Conditional
        }
    }

    fn call_shape(&self, operation: &MirOperation) -> OperationShape {
        match self
            .unsupported_at_operation_span(operation)
            .map(|row| row.construct.as_str())
        {
            Some("panic") => OperationShape {
                node_kind: CfgNodeKind::Panic,
                exit_edge: Some(CfgEdgeKind::Panic),
            },
            Some("recover") => OperationShape {
                node_kind: CfgNodeKind::CallSite,
                exit_edge: Some(CfgEdgeKind::Recover),
            },
            _ => OperationShape {
                node_kind: CfgNodeKind::CallSite,
                exit_edge: None,
            },
        }
    }

    fn unsupported_shape(&self, unsupported: UnsupportedId) -> UnsupportedShape {
        match self
            .unsupported_by_id(unsupported)
            .map(|row| row.construct.as_str())
        {
            Some("go_statement") => UnsupportedShape {
                node_kind: CfgNodeKind::CallSite,
                boundary_edge: Some(CfgEdgeKind::Spawn),
                stop_lowering: false,
            },
            Some("defer_statement") => UnsupportedShape {
                node_kind: CfgNodeKind::Defer,
                boundary_edge: Some(CfgEdgeKind::Defer),
                stop_lowering: false,
            },
            Some("select_statement") => UnsupportedShape {
                node_kind: CfgNodeKind::Unsupported,
                boundary_edge: Some(CfgEdgeKind::Unknown),
                stop_lowering: false,
            },
            Some("ERROR") => UnsupportedShape {
                node_kind: CfgNodeKind::Unsupported,
                boundary_edge: Some(CfgEdgeKind::Unknown),
                stop_lowering: true,
            },
            Some("fallthrough") => UnsupportedShape {
                node_kind: CfgNodeKind::Goto,
                boundary_edge: Some(CfgEdgeKind::Unknown),
                stop_lowering: false,
            },
            Some("goto") => UnsupportedShape {
                node_kind: CfgNodeKind::Goto,
                boundary_edge: Some(CfgEdgeKind::Goto),
                stop_lowering: false,
            },
            _ => UnsupportedShape {
                node_kind: CfgNodeKind::Unsupported,
                boundary_edge: Some(CfgEdgeKind::Unknown),
                stop_lowering: false,
            },
        }
    }

    fn operation_evidence(&self, operation: &MirOperation) -> String {
        self.source_text(operation.span.clone())
            .map_or_else(|| operation.stable_key.clone(), ToString::to_string)
    }

    fn source_text(&self, span: Span) -> Option<&str> {
        let file = self.db.files().iter().find(|file| file.id == span.file)?;
        source_slice(file, &span)
    }

    fn unsupported_by_id(&self, id: UnsupportedId) -> Option<&UnsupportedSemanticFact> {
        self.db
            .unsupported_semantics()
            .iter()
            .find(|row| row.id == id && row.language == Language::Go)
    }

    fn unsupported_for_body(
        &self,
        body: MirBodyId,
    ) -> impl Iterator<Item = &'db UnsupportedSemanticFact> + '_ {
        self.db.unsupported_semantics().iter().filter(move |row| {
            row.language == Language::Go
                && row.body == Some(body)
                && row.affected_domains.contains(&UnsupportedDomain::Cfg)
        })
    }

    fn unsupported_at_operation_span(
        &self,
        operation: &MirOperation,
    ) -> Option<&UnsupportedSemanticFact> {
        self.db.unsupported_semantics().iter().find(|row| {
            row.language == Language::Go
                && row.body == Some(operation.body)
                && row.operation.is_none()
                && row.span == operation.span
                && row.affected_domains.contains(&UnsupportedDomain::Cfg)
        })
    }

    fn finish(self) -> CfgOutput {
        let body_to_function = self.body_to_function;
        let db = self.db;
        let mut output = self.builder.finish();
        let mut unsupported = db
            .unsupported_semantics()
            .iter()
            .filter(|row| {
                row.language == Language::Go
                    && row.affected_domains.contains(&UnsupportedDomain::Cfg)
            })
            .enumerate()
            .map(|(index, row)| unsupported_control_flow_fact(index, row, &body_to_function))
            .collect::<Vec<_>>();
        output.unsupported.append(&mut unsupported);
        output.normalized()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BranchShape {
    Conditional,
    Loop,
    ShortCircuit,
}

struct OperationShape {
    node_kind: CfgNodeKind,
    exit_edge: Option<CfgEdgeKind>,
}

#[derive(Clone, Copy)]
struct UnsupportedShape {
    node_kind: CfgNodeKind,
    boundary_edge: Option<CfgEdgeKind>,
    stop_lowering: bool,
}

fn unsupported_control_flow_fact(
    index: usize,
    row: &UnsupportedSemanticFact,
    body_to_function: &BTreeMap<MirBodyId, CfgFunctionId>,
) -> UnsupportedControlFlowFact {
    let cfg_function = row
        .body
        .and_then(|body| body_to_function.get(&body).copied());
    UnsupportedControlFlowFact {
        id: UnsupportedControlFlowId(index as u64 + 1),
        cfg_function,
        body: row.body,
        operation: row.operation,
        language: row.language,
        file: row.file,
        span: row.span.clone(),
        construct: row.construct.clone(),
        source_evidence: row.source_evidence.clone(),
        conservative_action: control_flow_action(row.conservative_action),
        stable_key: semantic_stable_key(
            FactFamily::UnsupportedControlFlow,
            &[
                ("language", "go".to_string()),
                (
                    "body",
                    row.body
                        .map_or_else(|| "none".to_string(), |body| body.0.to_string()),
                ),
                (
                    "operation",
                    row.operation
                        .map_or_else(|| "none".to_string(), |operation| operation.0.to_string()),
                ),
                ("construct", row.construct.clone()),
                (
                    "span",
                    format!("{}..{}", row.span.start_byte, row.span.end_byte),
                ),
                ("source", row.stable_key.clone()),
            ],
        )
        .into_string(),
        status: cfg_status(row.status),
        precision: cfg_precision(row.precision),
    }
}

fn control_flow_action(action: ConservativeAction) -> ControlFlowAction {
    match action {
        ConservativeAction::SkipOperation => ControlFlowAction::SkipUnreachableTail,
        ConservativeAction::HavocAffectedPlaces | ConservativeAction::PreserveWithUnknownValue => {
            ControlFlowAction::PreserveUnknownEdge
        }
        ConservativeAction::StopLowering => ControlFlowAction::StopAtBoundary,
    }
}

fn cfg_status(status: MirStatus) -> CfgStatus {
    match status {
        MirStatus::Resolved => CfgStatus::Resolved,
        MirStatus::Partial => CfgStatus::Partial,
        MirStatus::Unknown => CfgStatus::Unknown,
        MirStatus::Unsupported => CfgStatus::Unsupported,
    }
}

fn cfg_precision(precision: UnsupportedPrecision) -> CfgPrecision {
    match precision {
        UnsupportedPrecision::Partial => CfgPrecision::Conservative,
        UnsupportedPrecision::Unknown => CfgPrecision::Unknown,
        UnsupportedPrecision::Unsupported => CfgPrecision::Unsupported,
    }
}

fn source_slice<'source>(file: &'source SourceFile, span: &Span) -> Option<&'source str> {
    let source = file.source.as_ref();
    let start = span.start_byte as usize;
    let end = span.end_byte as usize;
    if start <= end && end <= source.len() {
        source.get(start..end)
    } else {
        None
    }
}

fn contains_token(evidence: &str, tokens: &[&str]) -> bool {
    tokens.iter().any(|token| evidence.contains(token))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::cfg::facts::CfgEdgeKind;
    use crate::analysis::ids::{CallSiteId, MirPredicateId, PlaceId};
    use crate::analysis::mir::body::{MirBody, MirOutput};
    use crate::analysis::mir::op::{
        AssignMode, MirOperation, MirOperationKind, MirValue, UnsupportedDomain,
        UnsupportedSemanticFact,
    };
    use crate::analysis::places::{PlaceFact, PlaceRoot, PlaceStatus};
    use crate::core::{AnalysisDb, FileId, FunctionId, Language, Span};

    fn span(start: usize, end: usize) -> Span {
        Span {
            file: FileId(1),
            start_byte: start as u32,
            end_byte: end as u32,
            start_line: 1,
            start_col: start as u32 + 1,
            end_line: 1,
            end_col: end as u32 + 1,
        }
    }

    fn body() -> MirBody {
        MirBody {
            id: MirBodyId(0),
            language: Language::Go,
            file: FileId(1),
            function: FunctionId(1),
            package: None,
            module: None,
            owner_stable_key: "go:function:f".to_string(),
            span: span(0, 10),
            stable_key: "go:body:f".to_string(),
            status: MirStatus::Partial,
        }
    }

    fn assign(id: u64, ordinal: u32) -> MirOperation {
        MirOperation {
            id: MirOpId(id),
            body: MirBodyId(0),
            ordinal,
            span: span(ordinal as usize, ordinal as usize + 1),
            kind: MirOperationKind::Assign {
                place: PlaceId(1),
                value: MirValue::Place(PlaceId(1)),
                mode: AssignMode::Overwrite,
            },
            stable_key: format!("go:assign:{ordinal}"),
            status: MirStatus::Partial,
        }
    }

    fn branch(id: u64, ordinal: u32, evidence: &str) -> MirOperation {
        MirOperation {
            id: MirOpId(id),
            body: MirBodyId(0),
            ordinal,
            span: span(ordinal as usize, ordinal as usize + 1),
            kind: MirOperationKind::Branch {
                predicate: MirPredicateId(u64::from(ordinal)),
            },
            stable_key: evidence.to_string(),
            status: MirStatus::Partial,
        }
    }

    fn call(id: u64, ordinal: u32) -> MirOperation {
        MirOperation {
            id: MirOpId(id),
            body: MirBodyId(0),
            ordinal,
            span: span(ordinal as usize, ordinal as usize + 1),
            kind: MirOperationKind::Call {
                site: CallSiteId(1),
                callee: MirValue::Unknown {
                    evidence: "callee".to_string(),
                },
                arguments: Vec::new(),
                return_place: PlaceId(2),
            },
            stable_key: format!("go:call:{ordinal}"),
            status: MirStatus::Partial,
        }
    }

    fn return_op(id: u64, ordinal: u32) -> MirOperation {
        MirOperation {
            id: MirOpId(id),
            body: MirBodyId(0),
            ordinal,
            span: span(ordinal as usize, ordinal as usize + 1),
            kind: MirOperationKind::Return { value: None },
            stable_key: format!("go:return:{ordinal}"),
            status: MirStatus::Partial,
        }
    }

    fn unsupported_operation(id: u64, ordinal: u32, unsupported: u64) -> MirOperation {
        MirOperation {
            id: MirOpId(id),
            body: MirBodyId(0),
            ordinal,
            span: span(ordinal as usize, ordinal as usize + 1),
            kind: MirOperationKind::Unsupported {
                unsupported: UnsupportedId(unsupported),
            },
            stable_key: format!("go:unsupported:{ordinal}"),
            status: MirStatus::Unsupported,
        }
    }

    fn unsupported(
        id: u64,
        operation: Option<MirOpId>,
        construct: &str,
    ) -> UnsupportedSemanticFact {
        let ordinal = operation.map_or_else(
            || {
                if construct == "panic" { 4 } else { 9 }
            },
            |operation| operation.0 as u32,
        );
        UnsupportedSemanticFact {
            id: UnsupportedId(id),
            body: Some(MirBodyId(0)),
            operation,
            language: Language::Go,
            file: FileId(1),
            span: span(ordinal as usize, ordinal as usize + 1),
            construct: construct.to_string(),
            source_evidence: construct.to_string(),
            affected_places: Vec::new(),
            affected_domains: vec![UnsupportedDomain::Mir, UnsupportedDomain::Cfg],
            conservative_action: ConservativeAction::HavocAffectedPlaces,
            precision: UnsupportedPrecision::Unsupported,
            status: MirStatus::Unsupported,
            stable_key: format!("go:unsupported:{construct}:{id}"),
        }
    }

    fn place(id: u64, name: &str) -> PlaceFact {
        PlaceFact {
            id: PlaceId(id),
            language: Language::Go,
            file: Some(FileId(1)),
            function: Some(FunctionId(1)),
            root: PlaceRoot::Local {
                function: FunctionId(1),
                name: name.to_string(),
            },
            projections: Vec::new(),
            stable_key: format!("go:place:{name}"),
            status: PlaceStatus::Resolved,
        }
    }

    fn db_with(
        operations: Vec<MirOperation>,
        unsupported: Vec<UnsupportedSemanticFact>,
    ) -> AnalysisDb {
        let mut db = AnalysisDb::new();
        db.replace_semantic_mir(MirOutput {
            bodies: vec![body()],
            places: vec![place(1, "x"), place(2, "ret")],
            operations,
            unsupported,
        })
        .expect("MIR output should store");
        db
    }

    #[test]
    fn go_cfg_lowers_straight_line_return_without_derived_rows() {
        let db = db_with(vec![assign(1, 1), return_op(2, 2)], Vec::new());
        let output = lower_go_cfg(&db);

        assert!(
            output
                .functions
                .iter()
                .any(|function| function.language == Language::Go)
        );
        assert!(
            output
                .edges
                .iter()
                .any(|edge| edge.kind == CfgEdgeKind::Return)
        );
        assert!(output.reachability.is_empty());
        assert!(output.dominators.is_empty());
    }

    #[test]
    fn go_cfg_lowers_if_loop_and_short_circuit_edges() {
        let db = db_with(
            vec![
                branch(1, 1, "if_statement"),
                branch(2, 2, "for_statement"),
                branch(3, 3, "a && b"),
            ],
            Vec::new(),
        );
        let output = lower_go_cfg(&db);
        let edge_kinds = output
            .edges
            .iter()
            .map(|edge| edge.kind)
            .collect::<BTreeSet<_>>();

        assert!(edge_kinds.contains(&CfgEdgeKind::True));
        assert!(edge_kinds.contains(&CfgEdgeKind::False));
        assert!(edge_kinds.contains(&CfgEdgeKind::LoopEnter));
        assert!(edge_kinds.contains(&CfgEdgeKind::LoopBack));
        assert!(edge_kinds.contains(&CfgEdgeKind::LoopExit));
        assert!(edge_kinds.contains(&CfgEdgeKind::ShortCircuit));
    }

    #[test]
    fn go_cfg_return_prevents_following_tail_from_becoming_reachable() {
        let db = db_with(vec![return_op(1, 1), assign(2, 2)], Vec::new());
        let output = lower_go_cfg(&db);
        let unreachable_blocks = output
            .blocks
            .iter()
            .filter(|block| block.kind == BasicBlockKind::Unreachable)
            .map(|block| block.id)
            .collect::<BTreeSet<_>>();

        assert!(output.edges.iter().all(|edge| {
            !unreachable_blocks.contains(&edge.to_block)
                || unreachable_blocks.contains(&edge.from_block)
        }));
    }

    #[test]
    fn go_cfg_abrupt_and_unsupported_rows_are_truthful() {
        let db = db_with(
            vec![
                unsupported_operation(1, 1, 1),
                unsupported_operation(2, 2, 2),
                unsupported_operation(3, 3, 3),
                call(4, 4),
            ],
            vec![
                unsupported(1, Some(MirOpId(1)), "go_statement"),
                unsupported(2, Some(MirOpId(2)), "defer_statement"),
                unsupported(3, Some(MirOpId(3)), "select_statement"),
                unsupported(4, None, "panic"),
                unsupported(5, Some(MirOpId(3)), "goto"),
                unsupported(6, Some(MirOpId(3)), "fallthrough"),
            ],
        );
        let output = lower_go_cfg(&db);
        let edge_kinds = output
            .edges
            .iter()
            .map(|edge| edge.kind)
            .collect::<BTreeSet<_>>();
        let unsupported = output
            .unsupported
            .iter()
            .map(|row| row.construct.as_str())
            .collect::<BTreeSet<_>>();

        assert!(edge_kinds.contains(&CfgEdgeKind::Spawn));
        assert!(edge_kinds.contains(&CfgEdgeKind::Defer));
        assert!(edge_kinds.contains(&CfgEdgeKind::Unknown));
        assert!(edge_kinds.contains(&CfgEdgeKind::Panic));
        assert!(unsupported.contains("select_statement"));
        assert!(unsupported.contains("panic"));
        assert!(unsupported.contains("goto"));
        assert!(unsupported.contains("fallthrough"));
    }
}

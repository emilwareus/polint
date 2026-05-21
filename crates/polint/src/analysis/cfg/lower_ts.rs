use std::collections::BTreeMap;
#[cfg(test)]
use std::collections::BTreeSet;

use crate::analysis::cfg::builder::CfgBuilder;
use crate::analysis::cfg::facts::{
    BasicBlockKind, CfgEdgeKind, CfgNodeKind, CfgPrecision, CfgStatus, ControlFlowAction,
    UnsupportedControlFlowFact,
};
use crate::analysis::cfg::ids::{CfgFunctionId, UnsupportedControlFlowId};
use crate::analysis::cfg::store::CfgOutput;
#[cfg(test)]
use crate::analysis::ids::MirOpId;
use crate::analysis::ids::{MirBodyId, UnsupportedId};
use crate::analysis::mir::body::MirStatus;
use crate::analysis::mir::op::{
    ConservativeAction, MirOperation, MirOperationKind, UnsupportedDomain, UnsupportedPrecision,
    UnsupportedSemanticFact,
};
use crate::analysis::stable_key::semantic_stable_key;
use crate::analysis_kernel::FactFamily;
use crate::core::{AnalysisDb, Language, SourceFile, Span};

pub(crate) fn lower_ts_cfg(db: &AnalysisDb) -> CfgOutput {
    let mut lowering = TsCfgLowering::new(db);
    lowering.lower();
    lowering.finish()
}

struct TsCfgLowering<'db> {
    db: &'db AnalysisDb,
    builder: CfgBuilder,
    body_to_function: BTreeMap<MirBodyId, CfgFunctionId>,
}

impl<'db> TsCfgLowering<'db> {
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
            .filter(|body| body.language.is_ts_family())
            .collect::<Vec<_>>();
        bodies.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));

        for body in bodies {
            let has_exceptional_control = self.unsupported_for_body(body.id).any(|row| {
                matches!(
                    row.construct.as_str(),
                    "throw" | "try" | "async rejection path" | "parser recovery"
                )
            });
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
                    self.builder.append_operation_node(
                        Some(operation),
                        CfgNodeKind::CallSite,
                        Some(operation.span.clone()),
                    );
                    terminated = false;
                }
                MirOperationKind::Unsupported { unsupported } => {
                    let shape = self.unsupported_shape(*unsupported);
                    self.builder.append_operation_node(
                        Some(operation),
                        shape.node_kind,
                        Some(operation.span.clone()),
                    );
                    if !shape.boundary_edges.is_empty() {
                        let current = self.builder.current_block();
                        let next = if shape.to_exceptional_exit {
                            self.builder
                                .exceptional_exit_block()
                                .unwrap_or_else(|| self.builder.normal_exit_block())
                        } else {
                            let next = self.builder.start_block(BasicBlockKind::Synthetic);
                            self.builder.append_operation_node(
                                None,
                                CfgNodeKind::Synthetic,
                                Some(operation.span.clone()),
                            );
                            next
                        };
                        for edge_kind in shape.boundary_edges {
                            self.builder.add_edge(current, next, edge_kind);
                        }
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
        if !terminated {
            let current = self.builder.current_block();
            let exit = self.builder.normal_exit_block();
            if current != exit {
                self.builder.add_edge(current, exit, CfgEdgeKind::Normal);
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
            BranchShape::ShortCircuit(edge_kind) => {
                let evaluate_right = self.builder.start_block(BasicBlockKind::Branch);
                self.builder.append_operation_node(
                    None,
                    CfgNodeKind::Synthetic,
                    Some(operation.span.clone()),
                );
                self.builder
                    .add_edge(condition, evaluate_right, CfgEdgeKind::True);

                let join = self.builder.start_block(BasicBlockKind::Join);
                self.builder.append_operation_node(
                    None,
                    CfgNodeKind::Synthetic,
                    Some(operation.span.clone()),
                );
                self.builder.add_edge(condition, join, edge_kind);
                self.builder
                    .add_edge(evaluate_right, join, CfgEdgeKind::Normal);
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
        if contains_token(
            &evidence,
            &[
                "for ",
                "while",
                "do while",
                "for-of",
                "for-in",
                "for await",
                "for_statement",
            ],
        ) {
            BranchShape::Loop
        } else if contains_token(&evidence, &["??", "Nullish"]) {
            BranchShape::ShortCircuit(CfgEdgeKind::Nullish)
        } else if contains_token(&evidence, &["&&", "||", "ShortCircuit", "logical"]) {
            BranchShape::ShortCircuit(CfgEdgeKind::ShortCircuit)
        } else {
            BranchShape::Conditional
        }
    }

    fn unsupported_shape(&self, unsupported: UnsupportedId) -> UnsupportedShape {
        match self
            .unsupported_by_id(unsupported)
            .map(|row| row.construct.as_str())
        {
            Some("throw") => UnsupportedShape {
                node_kind: CfgNodeKind::Throw,
                boundary_edges: vec![CfgEdgeKind::Throw],
                to_exceptional_exit: true,
                stop_lowering: true,
            },
            Some("break") => UnsupportedShape {
                node_kind: CfgNodeKind::Break,
                boundary_edges: vec![CfgEdgeKind::Break],
                to_exceptional_exit: false,
                stop_lowering: false,
            },
            Some("continue") => UnsupportedShape {
                node_kind: CfgNodeKind::Continue,
                boundary_edges: vec![CfgEdgeKind::Continue],
                to_exceptional_exit: false,
                stop_lowering: false,
            },
            Some("optional chaining") => UnsupportedShape {
                node_kind: CfgNodeKind::Condition,
                boundary_edges: vec![CfgEdgeKind::OptionalChain],
                to_exceptional_exit: false,
                stop_lowering: false,
            },
            Some("await") | Some("for await") | Some("async rejection path") => UnsupportedShape {
                node_kind: CfgNodeKind::Await,
                boundary_edges: vec![CfgEdgeKind::AwaitSuspend, CfgEdgeKind::AwaitResume],
                to_exceptional_exit: false,
                stop_lowering: false,
            },
            Some("yield") => UnsupportedShape {
                node_kind: CfgNodeKind::Yield,
                boundary_edges: vec![CfgEdgeKind::YieldSuspend, CfgEdgeKind::YieldResume],
                to_exceptional_exit: false,
                stop_lowering: false,
            },
            Some("try") => UnsupportedShape {
                node_kind: CfgNodeKind::FinallyEnter,
                boundary_edges: vec![CfgEdgeKind::Finally, CfgEdgeKind::Cleanup],
                to_exceptional_exit: false,
                stop_lowering: false,
            },
            Some("switch") => UnsupportedShape {
                node_kind: CfgNodeKind::Condition,
                boundary_edges: vec![CfgEdgeKind::SwitchCase, CfgEdgeKind::DefaultCase],
                to_exceptional_exit: false,
                stop_lowering: false,
            },
            Some("parser recovery") => UnsupportedShape {
                node_kind: CfgNodeKind::Unsupported,
                boundary_edges: vec![CfgEdgeKind::Unknown],
                to_exceptional_exit: false,
                stop_lowering: true,
            },
            Some("dynamic import")
            | Some("eval")
            | Some("Proxy")
            | Some("getter")
            | Some("setter") => UnsupportedShape {
                node_kind: CfgNodeKind::Unsupported,
                boundary_edges: vec![CfgEdgeKind::Unknown],
                to_exceptional_exit: false,
                stop_lowering: false,
            },
            _ => UnsupportedShape {
                node_kind: CfgNodeKind::Unsupported,
                boundary_edges: vec![CfgEdgeKind::Unknown],
                to_exceptional_exit: false,
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
            .find(|row| row.id == id && row.language.is_ts_family())
    }

    fn unsupported_for_body(
        &self,
        body: MirBodyId,
    ) -> impl Iterator<Item = &'db UnsupportedSemanticFact> + '_ {
        self.db.unsupported_semantics().iter().filter(move |row| {
            row.language.is_ts_family()
                && row.body == Some(body)
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
                row.language.is_ts_family()
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
    ShortCircuit(CfgEdgeKind),
}

struct UnsupportedShape {
    node_kind: CfgNodeKind,
    boundary_edges: Vec<CfgEdgeKind>,
    to_exceptional_exit: bool,
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
                ("language", language_label(row.language).to_string()),
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

fn language_label(language: Language) -> &'static str {
    match language {
        Language::TypeScript => "ts",
        Language::Tsx => "tsx",
        Language::JavaScript => "js",
        Language::Jsx => "jsx",
        Language::Go => "go",
        Language::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn body(language: Language) -> MirBody {
        MirBody {
            id: MirBodyId(0),
            language,
            file: FileId(1),
            function: FunctionId(1),
            package: None,
            module: None,
            owner_stable_key: "ts:function:f".to_string(),
            span: span(0, 10),
            stable_key: format!("{}:body:f", language_label(language)),
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
            stable_key: format!("ts:assign:{ordinal}"),
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
            stable_key: format!("ts:call:{ordinal}"),
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
            stable_key: format!("ts:return:{ordinal}"),
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
            stable_key: format!("ts:unsupported:{ordinal}"),
            status: MirStatus::Unsupported,
        }
    }

    fn unsupported(
        id: u64,
        operation: Option<MirOpId>,
        construct: &str,
    ) -> UnsupportedSemanticFact {
        let ordinal = operation.map_or(9, |operation| operation.0 as u32);
        UnsupportedSemanticFact {
            id: UnsupportedId(id),
            body: Some(MirBodyId(0)),
            operation,
            language: Language::TypeScript,
            file: FileId(1),
            span: span(ordinal as usize, ordinal as usize + 1),
            construct: construct.to_string(),
            source_evidence: construct.to_string(),
            affected_places: Vec::new(),
            affected_domains: vec![UnsupportedDomain::Mir, UnsupportedDomain::Cfg],
            conservative_action: ConservativeAction::HavocAffectedPlaces,
            precision: UnsupportedPrecision::Unsupported,
            status: MirStatus::Unsupported,
            stable_key: format!("ts:unsupported:{construct}:{id}"),
        }
    }

    fn place(id: u64, name: &str) -> PlaceFact {
        PlaceFact {
            id: PlaceId(id),
            language: Language::TypeScript,
            file: Some(FileId(1)),
            function: Some(FunctionId(1)),
            root: PlaceRoot::Local {
                function: FunctionId(1),
                name: name.to_string(),
            },
            projections: Vec::new(),
            stable_key: format!("ts:place:{name}"),
            status: PlaceStatus::Resolved,
        }
    }

    fn db_with(
        operations: Vec<MirOperation>,
        unsupported: Vec<UnsupportedSemanticFact>,
    ) -> AnalysisDb {
        let mut db = AnalysisDb::new();
        db.replace_semantic_mir(MirOutput {
            bodies: vec![body(Language::TypeScript)],
            places: vec![place(1, "x"), place(2, "ret")],
            operations,
            unsupported,
        })
        .expect("MIR output should store");
        db
    }

    #[test]
    fn ts_cfg_lowers_straight_line_return_without_derived_rows() {
        let db = db_with(vec![assign(1, 1), return_op(2, 2)], Vec::new());
        let output = lower_ts_cfg(&db);

        assert!(
            output
                .functions
                .iter()
                .any(|function| function.language == Language::TypeScript)
        );
        assert!(
            output
                .edges
                .iter()
                .any(|edge| edge.kind == CfgEdgeKind::Return)
        );
        assert!(output.reachability.is_empty());
    }

    #[test]
    fn ts_cfg_connects_implicit_fallthrough_to_normal_exit() {
        let db = db_with(vec![assign(1, 1)], Vec::new());
        let output = lower_ts_cfg(&db);
        let exit = output
            .blocks
            .iter()
            .find(|block| block.kind == BasicBlockKind::ExitNormal)
            .expect("normal exit block")
            .id;

        assert!(
            output
                .edges
                .iter()
                .any(|edge| { edge.to_block == exit && edge.kind == CfgEdgeKind::Normal })
        );
    }

    #[test]
    fn ts_cfg_lowers_branch_loop_short_circuit_and_nullish_edges() {
        let db = db_with(
            vec![
                branch(1, 1, "if_statement"),
                branch(2, 2, "while_statement"),
                branch(3, 3, "a && b"),
                branch(4, 4, "a ?? b"),
            ],
            Vec::new(),
        );
        let output = lower_ts_cfg(&db);
        let edge_kinds = output
            .edges
            .iter()
            .map(|edge| edge.kind)
            .collect::<BTreeSet<_>>();

        assert!(edge_kinds.contains(&CfgEdgeKind::True));
        assert!(edge_kinds.contains(&CfgEdgeKind::False));
        assert!(edge_kinds.contains(&CfgEdgeKind::LoopEnter));
        assert!(edge_kinds.contains(&CfgEdgeKind::LoopBack));
        assert!(edge_kinds.contains(&CfgEdgeKind::ShortCircuit));
        assert!(edge_kinds.contains(&CfgEdgeKind::Nullish));
    }

    #[test]
    fn ts_cfg_throw_prevents_impossible_fallthrough() {
        let db = db_with(
            vec![unsupported_operation(1, 1, 1), assign(2, 2)],
            vec![unsupported(1, Some(MirOpId(1)), "throw")],
        );
        let output = lower_ts_cfg(&db);
        let unreachable_blocks = output
            .blocks
            .iter()
            .filter(|block| block.kind == BasicBlockKind::Unreachable)
            .map(|block| block.id)
            .collect::<BTreeSet<_>>();

        assert!(
            output
                .edges
                .iter()
                .any(|edge| edge.kind == CfgEdgeKind::Throw)
        );
        assert!(
            output
                .blocks
                .iter()
                .any(|block| block.kind == BasicBlockKind::ExitExceptional && block.reachable)
        );
        assert!(
            output
                .blocks
                .iter()
                .any(|block| block.kind == BasicBlockKind::ExitNormal && !block.reachable)
        );
        assert!(output.edges.iter().all(|edge| {
            !unreachable_blocks.contains(&edge.to_block)
                || unreachable_blocks.contains(&edge.from_block)
        }));
    }

    #[test]
    fn ts_cfg_async_cleanup_and_unsupported_rows_are_truthful() {
        let db = db_with(
            vec![
                unsupported_operation(1, 1, 1),
                unsupported_operation(2, 2, 2),
                unsupported_operation(3, 3, 3),
                unsupported_operation(4, 4, 4),
                unsupported_operation(5, 5, 5),
                call(6, 6),
            ],
            vec![
                unsupported(1, Some(MirOpId(1)), "await"),
                unsupported(2, Some(MirOpId(2)), "yield"),
                unsupported(3, Some(MirOpId(3)), "try"),
                unsupported(4, Some(MirOpId(4)), "optional chaining"),
                unsupported(5, Some(MirOpId(5)), "dynamic import"),
            ],
        );
        let output = lower_ts_cfg(&db);
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

        assert!(edge_kinds.contains(&CfgEdgeKind::AwaitSuspend));
        assert!(edge_kinds.contains(&CfgEdgeKind::AwaitResume));
        assert!(edge_kinds.contains(&CfgEdgeKind::YieldSuspend));
        assert!(edge_kinds.contains(&CfgEdgeKind::YieldResume));
        assert!(edge_kinds.contains(&CfgEdgeKind::Finally));
        assert!(edge_kinds.contains(&CfgEdgeKind::Cleanup));
        assert!(edge_kinds.contains(&CfgEdgeKind::OptionalChain));
        assert!(edge_kinds.contains(&CfgEdgeKind::Unknown));
        assert!(unsupported.contains("dynamic import"));
    }

    #[test]
    fn ts_unsupported_control_flow_key_uses_source_stable_identity() {
        let mut first = unsupported(1, Some(MirOpId(1)), "throw");
        let mut second = unsupported(2, Some(MirOpId(99)), "throw");
        first.body = Some(MirBodyId(7));
        second.body = Some(MirBodyId(42));
        second.span = first.span.clone();
        first.stable_key = "ts:unsupported:stable-source".to_string();
        second.stable_key = first.stable_key.clone();

        let first_fact = unsupported_control_flow_fact(0, &first, &BTreeMap::new());
        let second_fact = unsupported_control_flow_fact(0, &second, &BTreeMap::new());

        assert_eq!(first_fact.stable_key, second_fact.stable_key);
    }
}

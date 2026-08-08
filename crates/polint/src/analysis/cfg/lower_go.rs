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
use crate::analysis::mir::body::{MirBlockId, MirStatus, MirTerminatorKind};
use crate::analysis::mir::op::{
    ConservativeAction, MirOperation, MirOperationKind, UnsupportedDomain, UnsupportedPrecision,
    UnsupportedSemanticFact,
};
use crate::analysis::stable_key::semantic_stable_key;
use crate::analysis_kernel::FactFamily;
use crate::core::{AnalysisDb, Language};

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
        let mut blocks = self
            .db
            .mir_blocks()
            .iter()
            .filter(|block| block.body == body)
            .collect::<Vec<_>>();
        blocks.sort_by(|left, right| {
            (left.ordinal, left.stable_key.as_str())
                .cmp(&(right.ordinal, right.stable_key.as_str()))
        });
        if blocks.is_empty() {
            let current = self.builder.current_block();
            let exit = self.builder.normal_exit_block();
            self.builder.add_edge(current, exit, CfgEdgeKind::Normal);
            return;
        }

        let entry = self.builder.current_block();
        let mut cfg_blocks = BTreeMap::new();
        for block in &blocks {
            let terminator = self
                .db
                .mir_terminators()
                .iter()
                .find(|terminator| terminator.id == block.terminator)
                .expect("MIR block terminator must exist");
            let kind = match terminator.kind {
                MirTerminatorKind::Branch { .. } | MirTerminatorKind::Switch { .. } => {
                    BasicBlockKind::Branch
                }
                MirTerminatorKind::Unreachable => BasicBlockKind::Unreachable,
                _ => BasicBlockKind::StraightLine,
            };
            let cfg_block = self.builder.start_block(kind);
            if matches!(terminator.kind, MirTerminatorKind::Unreachable) {
                self.builder.mark_unreachable(cfg_block);
            }
            for (index, statement_id) in block.statements.iter().enumerate() {
                let statement = self
                    .db
                    .mir_statements()
                    .iter()
                    .find(|statement| statement.id == *statement_id)
                    .expect("MIR block statement must exist");
                let operation = self
                    .db
                    .mir_operations()
                    .iter()
                    .find(|operation| operation.id == statement.operation)
                    .expect("MIR statement operation must exist");
                let is_condition = index + 1 == block.statements.len()
                    && matches!(
                        terminator.kind,
                        MirTerminatorKind::Branch { .. } | MirTerminatorKind::Switch { .. }
                    );
                self.append_operation(cfg_block, operation, is_condition);
            }
            if block.statements.is_empty() {
                self.builder
                    .append_operation_node(None, CfgNodeKind::Synthetic, None);
            }
            cfg_blocks.insert(block.id, cfg_block);
        }
        self.builder.add_edge(
            entry,
            *cfg_blocks
                .get(&blocks[0].id)
                .expect("first MIR block must have a CFG block"),
            CfgEdgeKind::Normal,
        );

        for block in blocks {
            let from = *cfg_blocks
                .get(&block.id)
                .expect("MIR block must have a CFG block");
            let mut stop_lowering = false;
            for statement_id in &block.statements {
                let statement = self
                    .db
                    .mir_statements()
                    .iter()
                    .find(|statement| statement.id == *statement_id)
                    .expect("MIR block statement must exist");
                let operation = self
                    .db
                    .mir_operations()
                    .iter()
                    .find(|operation| operation.id == statement.operation)
                    .expect("MIR statement operation must exist");
                if matches!(operation.kind, MirOperationKind::Call { .. })
                    && self.call_shape(operation).exit_edge == Some(CfgEdgeKind::Panic)
                {
                    stop_lowering = true;
                }
                let MirOperationKind::Unsupported { unsupported } = &operation.kind else {
                    continue;
                };
                let shape = self.unsupported_shape(*unsupported);
                if let Some(edge_kind) = shape.boundary_edge {
                    let target = self.builder.start_block(BasicBlockKind::Synthetic);
                    self.builder.append_operation_node(
                        None,
                        CfgNodeKind::Synthetic,
                        Some(operation.span.clone()),
                    );
                    self.builder.add_edge(from, target, edge_kind);
                }
                stop_lowering |= shape.stop_lowering;
            }
            if stop_lowering {
                continue;
            }
            let terminator = self
                .db
                .mir_terminators()
                .iter()
                .find(|terminator| terminator.id == block.terminator)
                .expect("MIR block terminator must exist");
            match &terminator.kind {
                MirTerminatorKind::Goto { target } => {
                    self.add_mir_edge(from, *target, &cfg_blocks, CfgEdgeKind::Normal);
                }
                MirTerminatorKind::Branch {
                    then_target,
                    else_target,
                    ..
                } => {
                    self.add_mir_edge(from, *then_target, &cfg_blocks, CfgEdgeKind::True);
                    self.add_mir_edge(from, *else_target, &cfg_blocks, CfgEdgeKind::False);
                }
                MirTerminatorKind::Switch {
                    cases, otherwise, ..
                } => {
                    for (_, target) in cases {
                        self.add_mir_edge(from, *target, &cfg_blocks, CfgEdgeKind::SwitchCase);
                    }
                    self.add_mir_edge(from, *otherwise, &cfg_blocks, CfgEdgeKind::DefaultCase);
                }
                MirTerminatorKind::Return { .. } => {
                    self.builder.add_edge(
                        from,
                        self.builder.normal_exit_block(),
                        CfgEdgeKind::Return,
                    );
                }
                MirTerminatorKind::Unreachable | MirTerminatorKind::Unsupported { .. } => {}
            }
        }
    }

    fn add_mir_edge(
        &mut self,
        from: crate::analysis::cfg::ids::BasicBlockId,
        target: MirBlockId,
        blocks: &BTreeMap<MirBlockId, crate::analysis::cfg::ids::BasicBlockId>,
        kind: CfgEdgeKind,
    ) {
        let to = *blocks
            .get(&target)
            .expect("MIR terminator target must exist in its body");
        self.builder.add_edge(from, to, kind);
    }

    fn append_operation(
        &mut self,
        cfg_block: crate::analysis::cfg::ids::BasicBlockId,
        operation: &MirOperation,
        is_condition: bool,
    ) {
        let node_kind = if is_condition {
            CfgNodeKind::Condition
        } else {
            match &operation.kind {
                MirOperationKind::Return { .. } => CfgNodeKind::Return,
                MirOperationKind::Call { .. } => self.call_shape(operation).node_kind,
                MirOperationKind::Unsupported { unsupported } => {
                    self.unsupported_shape(*unsupported).node_kind
                }
                _ => CfgNodeKind::Operation,
            }
        };
        self.builder.append_operation_node(
            Some(operation),
            node_kind,
            Some(operation.span.clone()),
        );
        if let MirOperationKind::Call { .. } = &operation.kind
            && let Some(edge_kind) = self.call_shape(operation).exit_edge
        {
            let exit = self
                .builder
                .exceptional_exit_block()
                .unwrap_or_else(|| self.builder.normal_exit_block());
            self.builder.add_edge(cfg_block, exit, edge_kind);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::cfg::facts::CfgEdgeKind;
    use crate::analysis::ids::{CallSiteId, MirStatementId, MirTerminatorId, PlaceId};
    use crate::analysis::mir::body::{
        MirBlock, MirBlockId, MirBody, MirOutput, MirStatement, MirTerminator, MirTerminatorKind,
    };
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
        let statements = operations
            .iter()
            .enumerate()
            .map(|(index, operation)| MirStatement {
                id: MirStatementId(index as u64),
                body: MirBodyId(0),
                ordinal: operation.ordinal,
                operation: operation.id,
                stable_key: format!("statement:{index}"),
                status: operation.status,
            })
            .collect::<Vec<_>>();
        let value = operations.iter().rev().find_map(|operation| {
            if let MirOperationKind::Return { value } = &operation.kind {
                Some(value.clone())
            } else {
                None
            }
        });
        let mut db = AnalysisDb::new();
        db.replace_semantic_mir(MirOutput {
            bodies: vec![body()],
            blocks: vec![MirBlock {
                id: MirBlockId(0),
                body: MirBodyId(0),
                ordinal: 0,
                statements: statements.iter().map(|statement| statement.id).collect(),
                terminator: MirTerminatorId(0),
                stable_key: "block:0".to_string(),
            }],
            statements,
            terminators: vec![MirTerminator {
                id: MirTerminatorId(0),
                body: MirBodyId(0),
                ordinal: 0,
                kind: MirTerminatorKind::Return {
                    value: value.flatten(),
                },
                stable_key: "terminator:0".to_string(),
                status: MirStatus::Partial,
            }],
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
    fn go_cfg_connects_explicit_mir_return_to_normal_exit() {
        let db = db_with(vec![assign(1, 1)], Vec::new());
        let output = lower_go_cfg(&db);
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
                .any(|edge| { edge.to_block == exit && edge.kind == CfgEdgeKind::Return })
        );
    }

    #[test]
    fn go_cfg_lowers_edges_from_production_mir_terminators() {
        let mut db = AnalysisDb::new();
        db.add_file(
            std::path::PathBuf::from("flow.go"),
            "flow.go".to_string(),
            "package p\nfunc flow(x bool) { if x {} ; for x {} ; switch x { case true: } }"
                .to_string(),
        );
        assert!(crate::go::analyze(&mut db).is_empty());
        let mir = crate::analysis::mir::lower_go::lower_go_mir(&db);
        db.replace_semantic_mir(mir)
            .expect("MIR output should store");

        let edge_kinds = lower_go_cfg(&db)
            .edges
            .into_iter()
            .map(|edge| edge.kind)
            .collect::<BTreeSet<_>>();
        assert!(edge_kinds.contains(&CfgEdgeKind::True));
        assert!(edge_kinds.contains(&CfgEdgeKind::False));
        assert!(edge_kinds.contains(&CfgEdgeKind::SwitchCase));
        assert!(edge_kinds.contains(&CfgEdgeKind::DefaultCase));
        assert!(edge_kinds.contains(&CfgEdgeKind::Normal));
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
    }

    #[test]
    fn go_unsupported_control_flow_key_uses_source_stable_identity() {
        let mut first = unsupported(1, Some(MirOpId(1)), "goto");
        let mut second = unsupported(2, Some(MirOpId(99)), "goto");
        first.body = Some(MirBodyId(7));
        second.body = Some(MirBodyId(42));
        second.span = first.span.clone();
        first.stable_key = "go:unsupported:stable-source".to_string();
        second.stable_key = first.stable_key.clone();

        let first_fact = unsupported_control_flow_fact(0, &first, &BTreeMap::new());
        let second_fact = unsupported_control_flow_fact(0, &second, &BTreeMap::new());

        assert_eq!(first_fact.stable_key, second_fact.stable_key);
    }
}

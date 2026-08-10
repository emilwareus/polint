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
    ConservativeAction, MirOperationKind, UnsupportedDomain, UnsupportedPrecision,
    UnsupportedSemanticFact,
};
use crate::analysis::stable_key::semantic_stable_key;
use crate::analysis_kernel::FactFamily;
use crate::core::{AnalysisDb, Language};

pub(crate) fn lower_cfg(db: &AnalysisDb) -> CfgOutput {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    let mut lowering = CfgLowering::new(db);
    lowering.lower(interner);
    lowering.finish(interner)
}

struct CfgLowering<'db> {
    db: &'db AnalysisDb,
    builder: CfgBuilder,
    body_to_function: BTreeMap<MirBodyId, CfgFunctionId>,
}

impl<'db> CfgLowering<'db> {
    fn new(db: &'db AnalysisDb) -> Self {
        Self {
            db,
            builder: CfgBuilder::new(),
            body_to_function: BTreeMap::new(),
        }
    }

    fn lower(&mut self, interner: &crate::core::StableKeyInterner) {
        let mut bodies = self.db.mir_bodies().iter().collect::<Vec<_>>();
        bodies.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));

        for body in bodies {
            let has_exceptional_control = self.db.mir_terminators().iter().any(|terminator| {
                terminator.body == body.id
                    && matches!(
                        terminator.kind,
                        MirTerminatorKind::Throw { .. }
                            | MirTerminatorKind::Call {
                                unwind: Some(_),
                                ..
                            }
                    )
            }) || self
                .unsupported_for_body(body.id)
                .any(|row| matches!(row.construct.as_str(), "try" | "parser recovery" | "ERROR"));
            let function = self
                .builder
                .start_function(interner, body, has_exceptional_control);
            self.body_to_function.insert(body.id, function);
            self.lower_body(interner, body.id);
            self.builder.finish_function();
        }
    }

    fn lower_body(&mut self, interner: &crate::core::StableKeyInterner, body: MirBodyId) {
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
        let unwind_targets = self
            .db
            .mir_terminators()
            .iter()
            .filter(|terminator| terminator.body == body)
            .filter_map(|terminator| match terminator.kind {
                MirTerminatorKind::Throw { unwind, .. } => Some((unwind, CfgEdgeKind::Throw)),
                MirTerminatorKind::Call {
                    unwind: Some(unwind),
                    ..
                } => Some((unwind, CfgEdgeKind::ImplicitThrow)),
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();
        if blocks.is_empty() {
            let current = self.builder.current_block();
            let exit = self.builder.normal_exit_block();
            self.builder
                .add_edge(interner, current, exit, CfgEdgeKind::Normal);
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
                MirTerminatorKind::Unreachable if !unwind_targets.contains_key(&block.id) => {
                    BasicBlockKind::Unreachable
                }
                _ => BasicBlockKind::StraightLine,
            };
            let cfg_block = self.builder.start_block(interner, kind);
            if matches!(terminator.kind, MirTerminatorKind::Unreachable)
                && !unwind_targets.contains_key(&block.id)
            {
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
                let node_kind = if index + 1 == block.statements.len() {
                    match terminator.kind {
                        MirTerminatorKind::Branch { .. } | MirTerminatorKind::Switch { .. } => {
                            CfgNodeKind::Condition
                        }
                        MirTerminatorKind::Throw { .. } => CfgNodeKind::Throw,
                        MirTerminatorKind::Suspend {
                            kind: crate::analysis::mir::body::SuspendKind::Await,
                            ..
                        } => CfgNodeKind::Await,
                        MirTerminatorKind::Suspend {
                            kind: crate::analysis::mir::body::SuspendKind::Yield,
                            ..
                        } => CfgNodeKind::Yield,
                        _ => self.operation_node_kind(operation),
                    }
                } else {
                    self.operation_node_kind(operation)
                };
                self.builder.append_operation_node(
                    interner,
                    Some(operation),
                    node_kind,
                    Some(operation.span.clone()),
                );
            }
            if block.statements.is_empty() {
                let node_kind = match terminator.kind {
                    MirTerminatorKind::Throw { .. } => CfgNodeKind::Throw,
                    MirTerminatorKind::Suspend {
                        kind: crate::analysis::mir::body::SuspendKind::Await,
                        ..
                    } => CfgNodeKind::Await,
                    MirTerminatorKind::Suspend {
                        kind: crate::analysis::mir::body::SuspendKind::Yield,
                        ..
                    } => CfgNodeKind::Yield,
                    _ => CfgNodeKind::Synthetic,
                };
                self.builder
                    .append_operation_node(interner, None, node_kind, None);
            }
            cfg_blocks.insert(block.id, cfg_block);
        }
        self.builder.add_edge(
            interner,
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
                let MirOperationKind::Unsupported { unsupported } = &operation.kind else {
                    continue;
                };
                let shape = self.unsupported_shape(*unsupported);
                if !shape.boundary_edges.is_empty() {
                    let target = if shape.to_exceptional_exit {
                        self.builder
                            .exceptional_exit_block()
                            .unwrap_or_else(|| self.builder.normal_exit_block())
                    } else {
                        let target = self
                            .builder
                            .start_block(interner, BasicBlockKind::Synthetic);
                        self.builder.append_operation_node(
                            interner,
                            None,
                            CfgNodeKind::Synthetic,
                            Some(operation.span.clone()),
                        );
                        target
                    };
                    for edge_kind in &shape.boundary_edges {
                        self.builder.add_edge(interner, from, target, *edge_kind);
                    }
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
                    self.add_mir_edge(interner, from, *target, &cfg_blocks, CfgEdgeKind::Normal);
                }
                MirTerminatorKind::Branch {
                    then_target,
                    else_target,
                    ..
                } => {
                    self.add_mir_edge(interner, from, *then_target, &cfg_blocks, CfgEdgeKind::True);
                    self.add_mir_edge(
                        interner,
                        from,
                        *else_target,
                        &cfg_blocks,
                        CfgEdgeKind::False,
                    );
                }
                MirTerminatorKind::Switch {
                    cases, otherwise, ..
                } => {
                    for (_, target) in cases {
                        self.add_mir_edge(
                            interner,
                            from,
                            *target,
                            &cfg_blocks,
                            CfgEdgeKind::SwitchCase,
                        );
                    }
                    self.add_mir_edge(
                        interner,
                        from,
                        *otherwise,
                        &cfg_blocks,
                        CfgEdgeKind::DefaultCase,
                    );
                }
                MirTerminatorKind::Return { .. } => {
                    self.builder.add_edge(
                        interner,
                        from,
                        self.builder.normal_exit_block(),
                        CfgEdgeKind::Return,
                    );
                }
                MirTerminatorKind::Throw { unwind, .. } => {
                    self.add_mir_edge(interner, from, *unwind, &cfg_blocks, CfgEdgeKind::Throw);
                }
                MirTerminatorKind::Call { normal, unwind, .. } => {
                    self.add_mir_edge(interner, from, *normal, &cfg_blocks, CfgEdgeKind::Normal);
                    if let Some(unwind) = unwind {
                        self.add_mir_edge(
                            interner,
                            from,
                            *unwind,
                            &cfg_blocks,
                            CfgEdgeKind::ImplicitThrow,
                        );
                    }
                }
                MirTerminatorKind::Suspend { kind, resume, .. } => {
                    let (suspend, resumed) = match kind {
                        crate::analysis::mir::body::SuspendKind::Await => {
                            (CfgEdgeKind::AwaitSuspend, CfgEdgeKind::AwaitResume)
                        }
                        crate::analysis::mir::body::SuspendKind::Yield => {
                            (CfgEdgeKind::YieldSuspend, CfgEdgeKind::YieldResume)
                        }
                        crate::analysis::mir::body::SuspendKind::ChannelRecv
                        | crate::analysis::mir::body::SuspendKind::ChannelSend => {
                            (CfgEdgeKind::Unknown, CfgEdgeKind::Normal)
                        }
                    };
                    self.add_mir_edge(interner, from, *resume, &cfg_blocks, suspend);
                    self.add_mir_edge(interner, from, *resume, &cfg_blocks, resumed);
                }
                MirTerminatorKind::Unreachable => {
                    if unwind_targets.contains_key(&block.id) {
                        let exit = self
                            .builder
                            .exceptional_exit_block()
                            .unwrap_or_else(|| self.builder.normal_exit_block());
                        self.builder
                            .add_edge(interner, from, exit, CfgEdgeKind::Normal);
                    }
                }
                MirTerminatorKind::Unsupported { .. } => {}
            }
        }
    }

    fn add_mir_edge(
        &mut self,
        interner: &crate::core::StableKeyInterner,
        from: crate::analysis::cfg::ids::BasicBlockId,
        target: MirBlockId,
        blocks: &BTreeMap<MirBlockId, crate::analysis::cfg::ids::BasicBlockId>,
        kind: CfgEdgeKind,
    ) {
        let to = *blocks
            .get(&target)
            .expect("MIR terminator target must exist in its body");
        self.builder.add_edge(interner, from, to, kind);
    }

    fn operation_node_kind(
        &self,
        operation: &crate::analysis::mir::op::MirOperation,
    ) -> CfgNodeKind {
        match &operation.kind {
            MirOperationKind::Return { .. } => CfgNodeKind::Return,
            MirOperationKind::Call { .. } => CfgNodeKind::CallSite,
            MirOperationKind::Unsupported { unsupported } => {
                self.unsupported_shape(*unsupported).node_kind
            }
            _ => CfgNodeKind::Operation,
        }
    }

    fn unsupported_shape(&self, unsupported: UnsupportedId) -> UnsupportedShape {
        match self
            .unsupported_by_id(unsupported)
            .map(|row| row.construct.as_str())
        {
            Some("go_statement") => UnsupportedShape {
                node_kind: CfgNodeKind::CallSite,
                boundary_edges: vec![CfgEdgeKind::Spawn],
                to_exceptional_exit: false,
                stop_lowering: false,
            },
            Some("defer_statement") => UnsupportedShape {
                node_kind: CfgNodeKind::Defer,
                boundary_edges: vec![CfgEdgeKind::Defer],
                to_exceptional_exit: false,
                stop_lowering: false,
            },
            Some("select_statement") => UnsupportedShape {
                node_kind: CfgNodeKind::Unsupported,
                boundary_edges: vec![CfgEdgeKind::Unknown],
                to_exceptional_exit: false,
                stop_lowering: false,
            },
            Some("ERROR") => UnsupportedShape {
                node_kind: CfgNodeKind::Unsupported,
                boundary_edges: vec![CfgEdgeKind::Unknown],
                to_exceptional_exit: false,
                stop_lowering: true,
            },
            Some("fallthrough") => UnsupportedShape {
                node_kind: CfgNodeKind::Goto,
                boundary_edges: vec![CfgEdgeKind::Unknown],
                to_exceptional_exit: false,
                stop_lowering: false,
            },
            Some("goto") => UnsupportedShape {
                node_kind: CfgNodeKind::Goto,
                boundary_edges: vec![CfgEdgeKind::Goto],
                to_exceptional_exit: false,
                stop_lowering: false,
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

    fn unsupported_by_id(&self, id: UnsupportedId) -> Option<&UnsupportedSemanticFact> {
        self.db
            .unsupported_semantics()
            .iter()
            .find(|row| row.id == id)
    }

    fn unsupported_for_body(
        &self,
        body: MirBodyId,
    ) -> impl Iterator<Item = &'db UnsupportedSemanticFact> + '_ {
        self.db.unsupported_semantics().iter().filter(move |row| {
            row.body == Some(body) && row.affected_domains.contains(&UnsupportedDomain::Cfg)
        })
    }

    fn finish(self, interner: &crate::core::StableKeyInterner) -> CfgOutput {
        let body_to_function = self.body_to_function;
        let db = self.db;
        let mut output = self.builder.finish();
        let mut unsupported = db
            .unsupported_semantics()
            .iter()
            .filter(|row| row.affected_domains.contains(&UnsupportedDomain::Cfg))
            .enumerate()
            .map(|(index, row)| {
                unsupported_control_flow_fact(interner, index, row, &body_to_function)
            })
            .collect::<Vec<_>>();
        output.unsupported.append(&mut unsupported);
        output.normalized()
    }
}

struct UnsupportedShape {
    node_kind: CfgNodeKind,
    boundary_edges: Vec<CfgEdgeKind>,
    to_exceptional_exit: bool,
    stop_lowering: bool,
}

fn unsupported_control_flow_fact(
    interner: &crate::core::StableKeyInterner,
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
            interner,
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
    use crate::analysis::ids::{MirStatementId, MirTerminatorId, PlaceId};
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

    fn place(id: u64, name: &str, language: Language) -> PlaceFact {
        PlaceFact {
            id: PlaceId(id),
            language,
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
        language: Language,
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
            bodies: vec![body(language)],
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
            places: vec![place(1, "x", language), place(2, "ret", language)],
            place_types: Vec::new(),
            operations,
            unsupported,
        })
        .expect("MIR output should store");
        db
    }

    #[test]
    fn cfg_lowers_straight_line_return_without_derived_rows() {
        let db = db_with(
            Language::TypeScript,
            vec![assign(1, 1), return_op(2, 2)],
            Vec::new(),
        );
        let output = lower_cfg(&db);

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
    fn cfg_connects_explicit_mir_return_to_normal_exit() {
        let db = db_with(Language::TypeScript, vec![assign(1, 1)], Vec::new());
        let output = lower_cfg(&db);
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
    fn cfg_lowers_go_body_without_language_dispatch() {
        let db = db_with(Language::Go, vec![assign(1, 1)], Vec::new());
        let output = lower_cfg(&db);

        assert_eq!(output.functions.len(), 1);
        assert_eq!(output.functions[0].language, Language::Go);
        assert!(
            output
                .edges
                .iter()
                .any(|edge| edge.kind == CfgEdgeKind::Return)
        );
    }

    #[test]
    fn ts_cfg_lowers_edges_from_production_mir_terminators() {
        let mut db = AnalysisDb::new();
        db.add_file(
            std::path::PathBuf::from("flow.ts"),
            "flow.ts".to_string(),
            "export function flow(x) { if (x) {} while (x) {} switch (x) { case true: break; } }"
                .to_string(),
        );
        assert!(crate::ts::analyze(&mut db).is_empty());
        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir)
            .expect("MIR output should store");

        let edge_kinds = lower_cfg(&db)
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
    fn ts_cfg_throw_prevents_impossible_fallthrough() {
        let mut db = AnalysisDb::new();
        db.add_file(
            std::path::PathBuf::from("throw.ts"),
            "throw.ts".to_string(),
            "export function fail(value) { throw new Error(value); value = 1; }".to_string(),
        );
        assert!(crate::ts::analyze(&mut db).is_empty());
        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir)
            .expect("MIR output should store");
        let output = lower_cfg(&db);
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
        let mut db = AnalysisDb::new();
        db.add_file(
            std::path::PathBuf::from("effects.ts"),
            "effects.ts".to_string(),
            r#"
export async function load(promise, value) {
  await promise;
  try { value?.run(); } finally { cleanup(); }
  return import("./module.js");
}
export function* values(value) { yield value; }
"#
            .to_string(),
        );
        assert!(crate::ts::analyze(&mut db).is_empty());
        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir)
            .expect("MIR output should store");
        let output = lower_cfg(&db);
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
    fn unsupported_control_flow_key_uses_source_stable_identity() {
        let mut first = unsupported(1, Some(MirOpId(1)), "throw");
        let mut second = unsupported(2, Some(MirOpId(99)), "throw");
        first.body = Some(MirBodyId(7));
        second.body = Some(MirBodyId(42));
        second.span = first.span.clone();
        first.stable_key = "ts:unsupported:stable-source".to_string();
        second.stable_key = first.stable_key.clone();

        let first_fact = unsupported_control_flow_fact(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            0,
            &first,
            &BTreeMap::new(),
        );
        let second_fact = unsupported_control_flow_fact(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            0,
            &second,
            &BTreeMap::new(),
        );

        assert_eq!(first_fact.stable_key, second_fact.stable_key);
    }
}

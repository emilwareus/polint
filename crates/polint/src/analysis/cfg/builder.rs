use crate::analysis::cfg::facts::{
    BasicBlockFact, BasicBlockKind, CfgEdgeFact, CfgEdgeKind, CfgFunctionFact, CfgNodeFact,
    CfgNodeKind, CfgPrecision, CfgStatus, CfgView,
};
use crate::analysis::cfg::ids::{BasicBlockId, CfgEdgeId, CfgFunctionId, CfgNodeId};
use crate::analysis::cfg::store::CfgOutput;
use crate::analysis::ids::MirOpId;
use crate::analysis::mir::body::MirBody;
use crate::analysis::mir::op::MirOperation;
use crate::analysis::stable_key::semantic_stable_key;
use crate::analysis_kernel::FactFamily;
use crate::core::Span;

#[derive(Debug, Clone)]
pub(crate) struct CfgBuilder {
    output: CfgOutput,
    next_function: u64,
    next_node: u64,
    next_block: u64,
    next_edge: u64,
    current_function: Option<CfgFunctionId>,
    current_body: Option<crate::analysis::ids::MirBodyId>,
    current_body_stable_key: Option<String>,
    current_block: Option<BasicBlockId>,
    next_body_block_ordinal: u32,
    next_synthetic_node_ordinal: u32,
}

impl CfgBuilder {
    pub(crate) fn new() -> Self {
        Self {
            output: CfgOutput::empty(),
            next_function: 1,
            next_node: 1,
            next_block: 1,
            next_edge: 1,
            current_function: None,
            current_body: None,
            current_body_stable_key: None,
            current_block: None,
            next_body_block_ordinal: 0,
            next_synthetic_node_ordinal: 0,
        }
    }

    pub(crate) fn start_function(
        &mut self,
        body: &MirBody,
        include_exceptional_exit: bool,
    ) -> CfgFunctionId {
        let function_id = self.alloc_function_id();
        self.current_function = Some(function_id);
        self.current_body = Some(body.id);
        self.current_body_stable_key = Some(body.stable_key.clone());
        self.next_body_block_ordinal = 0;
        self.next_synthetic_node_ordinal = 0;

        let entry_node = self.new_node(
            function_id,
            body.id,
            None,
            CfgNodeKind::Entry,
            Some(body.span.clone()),
            true,
            0,
            stable_key(
                FactFamily::CfgNode,
                &[
                    ("body", body.stable_key.clone()),
                    ("kind", "entry".to_string()),
                ],
            ),
        );
        let entry_block = self.new_block(
            function_id,
            BasicBlockKind::Entry,
            Some(entry_node),
            Some(entry_node),
            0,
            stable_key(
                FactFamily::BasicBlock,
                &[
                    ("body", body.stable_key.clone()),
                    ("kind", "entry".to_string()),
                ],
            ),
        );

        let normal_exit_node = self.new_node(
            function_id,
            body.id,
            None,
            CfgNodeKind::ExitNormal,
            Some(body.span.clone()),
            true,
            u32::MAX - 1,
            stable_key(
                FactFamily::CfgNode,
                &[
                    ("body", body.stable_key.clone()),
                    ("kind", "exit-normal".to_string()),
                ],
            ),
        );
        self.new_block(
            function_id,
            BasicBlockKind::ExitNormal,
            Some(normal_exit_node),
            Some(normal_exit_node),
            u32::MAX - 1,
            stable_key(
                FactFamily::BasicBlock,
                &[
                    ("body", body.stable_key.clone()),
                    ("kind", "exit-normal".to_string()),
                ],
            ),
        );

        let exceptional_exit_node = include_exceptional_exit.then(|| {
            self.new_node(
                function_id,
                body.id,
                None,
                CfgNodeKind::ExitExceptional,
                Some(body.span.clone()),
                true,
                u32::MAX,
                stable_key(
                    FactFamily::CfgNode,
                    &[
                        ("body", body.stable_key.clone()),
                        ("kind", "exit-exceptional".to_string()),
                    ],
                ),
            )
        });
        if let Some(node) = exceptional_exit_node {
            self.new_block(
                function_id,
                BasicBlockKind::ExitExceptional,
                Some(node),
                Some(node),
                u32::MAX,
                stable_key(
                    FactFamily::BasicBlock,
                    &[
                        ("body", body.stable_key.clone()),
                        ("kind", "exit-exceptional".to_string()),
                    ],
                ),
            );
        }

        self.output.functions.push(CfgFunctionFact {
            id: function_id,
            body: body.id,
            function: body.function,
            language: body.language,
            file: body.file,
            span: body.span.clone(),
            entry_node,
            normal_exit_node,
            exceptional_exit_node,
            stable_key: stable_key(
                FactFamily::CfgFunction,
                &[
                    ("body", body.stable_key.clone()),
                    ("owner", body.owner_stable_key.clone()),
                ],
            ),
            status: CfgStatus::Resolved,
            precision: CfgPrecision::ExactLowered,
        });
        self.current_block = Some(entry_block);
        function_id
    }

    pub(crate) fn current_block(&self) -> BasicBlockId {
        self.current_block
            .expect("CfgBuilder::start_function must be called before current_block")
    }

    pub(crate) fn normal_exit_block(&self) -> BasicBlockId {
        let function = self.expect_function();
        self.output
            .blocks
            .iter()
            .find(|block| {
                block.cfg_function == function && block.kind == BasicBlockKind::ExitNormal
            })
            .map(|block| block.id)
            .expect("CfgBuilder::start_function must create a normal exit block")
    }

    pub(crate) fn exceptional_exit_block(&self) -> Option<BasicBlockId> {
        let function = self.expect_function();
        self.output
            .blocks
            .iter()
            .find(|block| {
                block.cfg_function == function && block.kind == BasicBlockKind::ExitExceptional
            })
            .map(|block| block.id)
    }

    pub(crate) fn start_block(&mut self, kind: BasicBlockKind) -> BasicBlockId {
        let function = self.expect_function();
        let ordinal = self.next_body_block_ordinal;
        self.next_body_block_ordinal += 1;
        let body_key = self.expect_body_stable_key().to_string();
        let block_id = self.new_block(
            function,
            kind,
            None,
            None,
            ordinal,
            stable_key(
                FactFamily::BasicBlock,
                &[
                    ("body", body_key),
                    ("ordinal", ordinal.to_string()),
                    ("kind", format!("{kind:?}")),
                ],
            ),
        );
        self.current_block = Some(block_id);
        block_id
    }

    pub(crate) fn append_operation_node(
        &mut self,
        operation: Option<&MirOperation>,
        kind: CfgNodeKind,
        span: Option<Span>,
    ) -> CfgNodeId {
        let function = self.expect_function();
        let body = self
            .current_body
            .expect("CfgBuilder::start_function must be called before append_operation_node");
        let block = self.current_block();
        let block_key = self.block(block).stable_key.clone();
        let previous_last = self.block_mut(block).last_node;
        let operation_ordinal = operation.map_or_else(
            || self.alloc_synthetic_node_ordinal(),
            |operation| operation.ordinal,
        );
        let operation_id = operation.map(|operation| operation.id);
        let operation_key = operation.map_or_else(
            || format!("synthetic:{operation_ordinal}"),
            |operation| operation.stable_key.clone(),
        );
        let body_key = self.expect_body_stable_key().to_string();
        let node_id = self.new_node(
            function,
            body,
            operation_id,
            kind,
            span,
            false,
            operation_ordinal,
            stable_key(
                FactFamily::CfgNode,
                &[
                    ("body", body_key),
                    ("block", block_key),
                    ("operation", operation_key),
                    ("ordinal", operation_ordinal.to_string()),
                    ("kind", format!("{kind:?}")),
                ],
            ),
        );
        if let Some(node) = self.output.nodes.iter_mut().find(|node| node.id == node_id) {
            node.block = block;
        }

        {
            let block_fact = self.block_mut(block);
            if block_fact.first_node.is_none() {
                block_fact.first_node = Some(node_id);
            }
            block_fact.last_node = Some(node_id);
        }

        if let Some(previous) = previous_last {
            self.add_node_edge(previous, node_id, block, block, CfgEdgeKind::Normal);
        }
        node_id
    }

    pub(crate) fn add_edge(
        &mut self,
        from_block: BasicBlockId,
        to_block: BasicBlockId,
        kind: CfgEdgeKind,
    ) -> CfgEdgeId {
        let from_node = self
            .block(from_block)
            .last_node
            .expect("source block must have at least one node before adding an edge");
        let to_node = self
            .block(to_block)
            .first_node
            .expect("target block must have at least one node before adding an edge");
        self.add_node_edge(from_node, to_node, from_block, to_block, kind)
    }

    pub(crate) fn mark_unreachable(&mut self, block: BasicBlockId) {
        self.block_mut(block).reachable = false;
    }

    pub(crate) fn finish_function(&mut self) {
        self.current_function = None;
        self.current_body = None;
        self.current_body_stable_key = None;
        self.current_block = None;
        self.next_body_block_ordinal = 0;
        self.next_synthetic_node_ordinal = 0;
    }

    pub(crate) fn finish(self) -> CfgOutput {
        self.output.normalized()
    }

    fn add_node_edge(
        &mut self,
        from: CfgNodeId,
        to: CfgNodeId,
        from_block: BasicBlockId,
        to_block: BasicBlockId,
        kind: CfgEdgeKind,
    ) -> CfgEdgeId {
        let function = self.expect_function();
        let body_key = self.expect_body_stable_key().to_string();
        let from_block_key = self.block(from_block).stable_key.clone();
        let to_block_key = self.block(to_block).stable_key.clone();
        let from_node_key = self.node(from).stable_key.clone();
        let to_node_key = self.node(to).stable_key.clone();
        let edge_id = self.alloc_edge_id();
        self.output.edges.push(CfgEdgeFact {
            id: edge_id,
            cfg_function: function,
            view: CfgView::NormalControl,
            from,
            to,
            from_block,
            to_block,
            kind,
            label: None,
            stable_key: stable_key(
                FactFamily::CfgEdge,
                &[
                    ("body", body_key),
                    ("from_block", from_block_key),
                    ("to_block", to_block_key),
                    ("from_node", from_node_key),
                    ("to_node", to_node_key),
                    ("kind", format!("{kind:?}")),
                ],
            ),
            status: CfgStatus::Resolved,
            precision: CfgPrecision::ExactLowered,
        });
        edge_id
    }

    fn new_node(
        &mut self,
        function: CfgFunctionId,
        body: crate::analysis::ids::MirBodyId,
        operation: Option<MirOpId>,
        kind: CfgNodeKind,
        span: Option<Span>,
        generated: bool,
        operation_ordinal: u32,
        stable_key: String,
    ) -> CfgNodeId {
        let id = self.alloc_node_id();
        self.output.nodes.push(CfgNodeFact {
            id,
            cfg_function: function,
            body,
            operation,
            block: BasicBlockId(0),
            kind,
            span,
            generated,
            operation_ordinal,
            stable_key,
            status: CfgStatus::Resolved,
            precision: CfgPrecision::ExactLowered,
        });
        id
    }

    fn new_block(
        &mut self,
        function: CfgFunctionId,
        kind: BasicBlockKind,
        first_node: Option<CfgNodeId>,
        last_node: Option<CfgNodeId>,
        reverse_postorder: u32,
        stable_key: String,
    ) -> BasicBlockId {
        let id = self.alloc_block_id();
        for node_id in [first_node, last_node].into_iter().flatten() {
            if let Some(node) = self.output.nodes.iter_mut().find(|node| node.id == node_id) {
                node.block = id;
            }
        }
        self.output.blocks.push(BasicBlockFact {
            id,
            cfg_function: function,
            kind,
            first_node,
            last_node,
            reachable: true,
            reverse_postorder,
            stable_key,
            status: CfgStatus::Resolved,
            precision: CfgPrecision::ExactLowered,
        });
        id
    }

    fn block(&self, id: BasicBlockId) -> &BasicBlockFact {
        self.output
            .blocks
            .iter()
            .find(|block| block.id == id)
            .expect("block id must refer to a builder-owned block")
    }

    fn node(&self, id: CfgNodeId) -> &CfgNodeFact {
        self.output
            .nodes
            .iter()
            .find(|node| node.id == id)
            .expect("node id must refer to a builder-owned node")
    }

    fn block_mut(&mut self, id: BasicBlockId) -> &mut BasicBlockFact {
        self.output
            .blocks
            .iter_mut()
            .find(|block| block.id == id)
            .expect("block id must refer to a builder-owned block")
    }

    fn expect_function(&self) -> CfgFunctionId {
        self.current_function
            .expect("CfgBuilder::start_function must be called before adding CFG rows")
    }

    fn expect_body_stable_key(&self) -> &str {
        self.current_body_stable_key
            .as_deref()
            .expect("CfgBuilder::start_function must be called before adding CFG rows")
    }

    fn alloc_synthetic_node_ordinal(&mut self) -> u32 {
        let ordinal = self.next_synthetic_node_ordinal;
        self.next_synthetic_node_ordinal += 1;
        ordinal
    }

    fn alloc_function_id(&mut self) -> CfgFunctionId {
        let id = CfgFunctionId(self.next_function);
        self.next_function += 1;
        id
    }

    fn alloc_node_id(&mut self) -> CfgNodeId {
        let id = CfgNodeId(self.next_node);
        self.next_node += 1;
        id
    }

    fn alloc_block_id(&mut self) -> BasicBlockId {
        let id = BasicBlockId(self.next_block);
        self.next_block += 1;
        id
    }

    fn alloc_edge_id(&mut self) -> CfgEdgeId {
        let id = CfgEdgeId(self.next_edge);
        self.next_edge += 1;
        id
    }
}

fn stable_key(family: FactFamily, parts: &[(&str, String)]) -> String {
    semantic_stable_key(family, parts).into_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ids::{MirBodyId, MirOpId, PlaceId};
    use crate::analysis::mir::body::{MirBody, MirStatus};
    use crate::analysis::mir::op::{AssignMode, MirOperation, MirOperationKind, MirValue};
    use crate::core::{FileId, FunctionId, Language};

    fn span() -> Span {
        Span {
            file: FileId(1),
            start_byte: 1,
            end_byte: 2,
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 2,
        }
    }

    fn body() -> MirBody {
        MirBody {
            id: MirBodyId(1),
            language: Language::Go,
            file: FileId(1),
            function: FunctionId(1),
            package: None,
            module: None,
            owner_stable_key: "owner".to_string(),
            span: span(),
            stable_key: "body:one".to_string(),
            status: MirStatus::Resolved,
        }
    }

    fn op(id: u64, ordinal: u32) -> MirOperation {
        MirOperation {
            id: MirOpId(id),
            body: MirBodyId(1),
            ordinal,
            span: span(),
            kind: MirOperationKind::Assign {
                place: PlaceId(1),
                value: MirValue::Place(PlaceId(2)),
                mode: AssignMode::Overwrite,
            },
            stable_key: format!("op:{ordinal}"),
            status: MirStatus::Resolved,
        }
    }

    #[test]
    fn builder_creates_virtual_entry_and_exit_per_function() {
        let mut builder = CfgBuilder::new();
        let function = builder.start_function(&body(), true);
        builder.finish_function();
        let output = builder.finish();

        let function_fact = output
            .functions
            .iter()
            .find(|fact| fact.id == function)
            .expect("function row");
        assert!(
            output
                .nodes
                .iter()
                .any(|node| node.id == function_fact.entry_node)
        );
        assert!(
            output
                .nodes
                .iter()
                .any(|node| node.id == function_fact.normal_exit_node)
        );
        assert!(function_fact.exceptional_exit_node.is_some());
    }

    #[test]
    fn builder_assigns_deterministic_ids_for_straight_line_graphs() {
        fn build_keys() -> Vec<String> {
            let mut builder = CfgBuilder::new();
            builder.start_function(&body(), false);
            builder.append_operation_node(Some(&op(1, 1)), CfgNodeKind::Operation, Some(span()));
            builder.append_operation_node(Some(&op(2, 2)), CfgNodeKind::Operation, Some(span()));
            builder.finish_function();
            builder
                .finish()
                .nodes
                .into_iter()
                .map(|node| format!("{}:{}", node.id.0, node.stable_key))
                .collect()
        }

        assert_eq!(build_keys(), build_keys());
    }

    #[test]
    fn builder_models_if_else_join_loop_return_unreachable_and_short_circuit_edges() {
        let mut builder = CfgBuilder::new();
        builder.start_function(&body(), false);
        let entry = builder.current_block();
        let condition = builder.start_block(BasicBlockKind::Branch);
        builder.append_operation_node(Some(&op(1, 1)), CfgNodeKind::Condition, Some(span()));
        let then_block = builder.start_block(BasicBlockKind::StraightLine);
        builder.append_operation_node(Some(&op(2, 2)), CfgNodeKind::Operation, Some(span()));
        let else_block = builder.start_block(BasicBlockKind::StraightLine);
        builder.append_operation_node(Some(&op(3, 3)), CfgNodeKind::Operation, Some(span()));
        let loop_header = builder.start_block(BasicBlockKind::LoopHeader);
        builder.append_operation_node(Some(&op(4, 4)), CfgNodeKind::Condition, Some(span()));
        let return_block = builder.start_block(BasicBlockKind::StraightLine);
        builder.append_operation_node(Some(&op(5, 5)), CfgNodeKind::Return, Some(span()));
        let unreachable = builder.start_block(BasicBlockKind::Unreachable);
        builder.append_operation_node(Some(&op(6, 6)), CfgNodeKind::Operation, Some(span()));
        builder.mark_unreachable(unreachable);

        builder.add_edge(entry, condition, CfgEdgeKind::Normal);
        builder.add_edge(condition, then_block, CfgEdgeKind::True);
        builder.add_edge(condition, else_block, CfgEdgeKind::False);
        builder.add_edge(then_block, loop_header, CfgEdgeKind::ShortCircuit);
        builder.add_edge(else_block, loop_header, CfgEdgeKind::Normal);
        builder.add_edge(loop_header, loop_header, CfgEdgeKind::LoopBack);
        builder.add_edge(loop_header, return_block, CfgEdgeKind::LoopExit);
        builder.finish_function();
        let output = builder.finish();

        assert!(
            output
                .edges
                .iter()
                .any(|edge| edge.kind == CfgEdgeKind::True)
        );
        assert!(
            output
                .edges
                .iter()
                .any(|edge| edge.kind == CfgEdgeKind::False)
        );
        assert!(
            output
                .edges
                .iter()
                .any(|edge| edge.kind == CfgEdgeKind::LoopBack)
        );
        assert!(
            output
                .edges
                .iter()
                .any(|edge| edge.kind == CfgEdgeKind::ShortCircuit)
        );
        assert!(
            output
                .blocks
                .iter()
                .any(|block| block.kind == BasicBlockKind::Unreachable && !block.reachable)
        );
    }
}

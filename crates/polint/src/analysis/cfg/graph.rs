use crate::analysis::cfg::facts::{
    BasicBlockFact, BasicBlockKind, CfgEdgeFact, CfgFunctionFact, CfgView,
};
use crate::analysis::cfg::ids::{BasicBlockId, CfgFunctionId};
use crate::analysis::cfg::store::CfgOutput;

#[derive(Debug, Clone, Copy)]
pub(crate) struct CfgGraph<'a> {
    output: &'a CfgOutput,
    function: CfgFunctionId,
    view: CfgView,
}

impl<'a> CfgGraph<'a> {
    pub(crate) fn new(output: &'a CfgOutput, function: CfgFunctionId, view: CfgView) -> Self {
        Self {
            output,
            function,
            view,
        }
    }

    pub(crate) fn function(&self) -> Option<&'a CfgFunctionFact> {
        self.output
            .functions
            .iter()
            .find(|function| function.id == self.function)
    }

    pub(crate) fn nodes(&self) -> Vec<&'a crate::analysis::cfg::facts::CfgNodeFact> {
        let mut nodes = self
            .output
            .nodes
            .iter()
            .filter(|node| node.cfg_function == self.function)
            .collect::<Vec<_>>();
        nodes.sort_by(|left, right| {
            (left.block, left.operation_ordinal, left.stable_key.as_str()).cmp(&(
                right.block,
                right.operation_ordinal,
                right.stable_key.as_str(),
            ))
        });
        nodes
    }

    pub(crate) fn blocks(&self) -> Vec<&'a BasicBlockFact> {
        let mut blocks = self
            .output
            .blocks
            .iter()
            .filter(|block| block.cfg_function == self.function)
            .collect::<Vec<_>>();
        blocks.sort_by(|left, right| {
            (left.reverse_postorder, left.stable_key.as_str())
                .cmp(&(right.reverse_postorder, right.stable_key.as_str()))
        });
        blocks
    }

    pub(crate) fn edges(&self) -> Vec<&'a CfgEdgeFact> {
        let mut edges = self
            .output
            .edges
            .iter()
            .filter(|edge| edge.cfg_function == self.function && edge.view == self.view)
            .collect::<Vec<_>>();
        edges.sort_by(|left, right| {
            (
                left.from_block,
                left.to_block,
                left.kind,
                left.stable_key.as_str(),
            )
                .cmp(&(
                    right.from_block,
                    right.to_block,
                    right.kind,
                    right.stable_key.as_str(),
                ))
        });
        edges
    }

    pub(crate) fn successors(&self, block: BasicBlockId) -> Vec<BasicBlockId> {
        let mut successors = self
            .edges()
            .into_iter()
            .filter(|edge| edge.from_block == block && edge.to_block != block)
            .map(|edge| edge.to_block)
            .collect::<Vec<_>>();
        successors.sort();
        successors.dedup();
        successors
    }

    pub(crate) fn predecessors(&self, block: BasicBlockId) -> Vec<BasicBlockId> {
        let mut predecessors = self
            .edges()
            .into_iter()
            .filter(|edge| edge.to_block == block && edge.from_block != block)
            .map(|edge| edge.from_block)
            .collect::<Vec<_>>();
        predecessors.sort();
        predecessors.dedup();
        predecessors
    }

    pub(crate) fn entry_block(&self) -> Option<BasicBlockId> {
        self.blocks()
            .into_iter()
            .find(|block| block.kind == BasicBlockKind::Entry)
            .map(|block| block.id)
    }

    pub(crate) fn synthetic_exit_block(&self, _view: CfgView) -> Option<BasicBlockId> {
        self.blocks()
            .into_iter()
            .find(|block| block.kind == BasicBlockKind::ExitNormal)
            .map(|block| block.id)
    }

    pub(crate) fn block(&self, id: BasicBlockId) -> Option<&'a BasicBlockFact> {
        self.output
            .blocks
            .iter()
            .find(|block| block.cfg_function == self.function && block.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::cfg::builder::CfgBuilder;
    use crate::analysis::cfg::facts::{BasicBlockKind, CfgEdgeKind, CfgNodeKind, CfgView};
    use crate::analysis::ids::PlaceId;
    use crate::analysis::ids::{MirBodyId, MirOpId};
    use crate::analysis::mir::body::{MirBody, MirStatus};
    use crate::analysis::mir::op::{AssignMode, MirOperation, MirOperationKind, MirValue};
    use crate::core::{FileId, FunctionId, Language, Span};

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
    fn graph_view_exposes_sorted_block_successors_and_predecessors() {
        let mut builder = CfgBuilder::new();
        let function = builder.start_function(&body(), false);
        let entry = builder.current_block();
        let branch = builder.start_block(BasicBlockKind::Branch);
        builder.append_operation_node(Some(&op(1, 1)), CfgNodeKind::Condition, Some(span()));
        let then_block = builder.start_block(BasicBlockKind::StraightLine);
        builder.append_operation_node(Some(&op(2, 2)), CfgNodeKind::Operation, Some(span()));
        let else_block = builder.start_block(BasicBlockKind::StraightLine);
        builder.append_operation_node(Some(&op(3, 3)), CfgNodeKind::Operation, Some(span()));

        builder.add_edge(entry, branch, CfgEdgeKind::Normal);
        builder.add_edge(branch, else_block, CfgEdgeKind::False);
        builder.add_edge(branch, then_block, CfgEdgeKind::True);
        builder.finish_function();
        let output = builder.finish();

        let graph = CfgGraph::new(&output, function, CfgView::NormalControl);
        assert!(graph.function().is_some());
        assert!(!graph.nodes().is_empty());
        assert_eq!(graph.entry_block(), Some(entry));
        assert_eq!(graph.successors(branch), vec![then_block, else_block]);
        assert_eq!(graph.predecessors(then_block), vec![branch]);
        assert!(graph.block(branch).is_some());
        assert!(
            graph
                .blocks()
                .iter()
                .any(|block| block.kind == BasicBlockKind::ExitNormal)
        );
    }
}

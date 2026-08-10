use std::collections::BTreeMap;

use crate::analysis::cfg::facts::{
    BasicBlockFact, BasicBlockKind, CfgEdgeFact, CfgFunctionFact, CfgNodeFact, CfgView,
};
use crate::analysis::cfg::ids::{BasicBlockId, CfgFunctionId};
use crate::analysis::cfg::store::CfgOutput;

#[derive(Debug)]
pub(crate) struct CfgGraphIndex<'a> {
    output: &'a CfgOutput,
    functions: Vec<&'a CfgFunctionFact>,
    function_by_id: BTreeMap<CfgFunctionId, &'a CfgFunctionFact>,
    nodes_by_function: BTreeMap<CfgFunctionId, Vec<&'a CfgNodeFact>>,
    blocks_by_function: BTreeMap<CfgFunctionId, Vec<&'a BasicBlockFact>>,
    edges_by_function_view: BTreeMap<(CfgFunctionId, CfgView), Vec<&'a CfgEdgeFact>>,
}

impl<'a> CfgGraphIndex<'a> {
    pub(crate) fn new(output: &'a CfgOutput) -> Self {
        let mut functions = output.functions.iter().collect::<Vec<_>>();
        functions.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        let function_by_id = output
            .functions
            .iter()
            .map(|function| (function.id, function))
            .collect::<BTreeMap<_, _>>();

        let mut nodes_by_function = BTreeMap::<CfgFunctionId, Vec<&CfgNodeFact>>::new();
        for node in &output.nodes {
            nodes_by_function
                .entry(node.cfg_function)
                .or_default()
                .push(node);
        }
        for nodes in nodes_by_function.values_mut() {
            nodes.sort_by(|left, right| {
                (left.block, left.operation_ordinal, left.stable_key.as_str()).cmp(&(
                    right.block,
                    right.operation_ordinal,
                    right.stable_key.as_str(),
                ))
            });
        }

        let mut blocks_by_function = BTreeMap::<CfgFunctionId, Vec<&BasicBlockFact>>::new();
        for block in &output.blocks {
            blocks_by_function
                .entry(block.cfg_function)
                .or_default()
                .push(block);
        }
        for blocks in blocks_by_function.values_mut() {
            blocks.sort_by(|left, right| {
                (left.reverse_postorder, left.stable_key.as_str())
                    .cmp(&(right.reverse_postorder, right.stable_key.as_str()))
            });
        }

        let mut edges_by_function_view =
            BTreeMap::<(CfgFunctionId, CfgView), Vec<&CfgEdgeFact>>::new();
        for edge in &output.edges {
            edges_by_function_view
                .entry((edge.cfg_function, edge.view))
                .or_default()
                .push(edge);
        }
        for edges in edges_by_function_view.values_mut() {
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
        }

        Self {
            output,
            functions,
            function_by_id,
            nodes_by_function,
            blocks_by_function,
            edges_by_function_view,
        }
    }

    pub(crate) fn graphs(&self, view: CfgView) -> Vec<CfgGraph<'a>> {
        self.functions
            .iter()
            .map(|function| self.graph(function.id, view))
            .collect()
    }

    fn graph(&self, function: CfgFunctionId, view: CfgView) -> CfgGraph<'a> {
        CfgGraph::from_grouped(
            self.output,
            function,
            self.function_by_id.get(&function).copied(),
            self.nodes_by_function
                .get(&function)
                .cloned()
                .unwrap_or_default(),
            self.blocks_by_function
                .get(&function)
                .cloned()
                .unwrap_or_default(),
            self.edges_by_function_view
                .get(&(function, view))
                .cloned()
                .unwrap_or_default(),
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CfgGraph<'a> {
    output: &'a CfgOutput,
    function: CfgFunctionId,
    function_fact: Option<&'a CfgFunctionFact>,
    nodes: Vec<&'a CfgNodeFact>,
    blocks: Vec<&'a BasicBlockFact>,
    edges: Vec<&'a CfgEdgeFact>,
    successors: BTreeMap<BasicBlockId, Vec<BasicBlockId>>,
    predecessors: BTreeMap<BasicBlockId, Vec<BasicBlockId>>,
}

impl<'a> CfgGraph<'a> {
    pub(crate) fn new(output: &'a CfgOutput, function: CfgFunctionId, view: CfgView) -> Self {
        let function_fact = output
            .functions
            .iter()
            .find(|function_fact| function_fact.id == function);

        let mut nodes = output
            .nodes
            .iter()
            .filter(|node| node.cfg_function == function)
            .collect::<Vec<_>>();
        nodes.sort_by(|left, right| {
            (left.block, left.operation_ordinal, left.stable_key.as_str()).cmp(&(
                right.block,
                right.operation_ordinal,
                right.stable_key.as_str(),
            ))
        });

        let mut blocks = output
            .blocks
            .iter()
            .filter(|block| block.cfg_function == function)
            .collect::<Vec<_>>();
        blocks.sort_by(|left, right| {
            (left.reverse_postorder, left.stable_key.as_str())
                .cmp(&(right.reverse_postorder, right.stable_key.as_str()))
        });

        let mut edges = output
            .edges
            .iter()
            .filter(|edge| edge.cfg_function == function && edge.view == view)
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

        let mut successors = BTreeMap::<BasicBlockId, Vec<BasicBlockId>>::new();
        let mut predecessors = BTreeMap::<BasicBlockId, Vec<BasicBlockId>>::new();
        for edge in &edges {
            if edge.from_block == edge.to_block {
                continue;
            }
            successors
                .entry(edge.from_block)
                .or_default()
                .push(edge.to_block);
            predecessors
                .entry(edge.to_block)
                .or_default()
                .push(edge.from_block);
        }
        for targets in successors.values_mut() {
            targets.sort();
            targets.dedup();
        }
        for sources in predecessors.values_mut() {
            sources.sort();
            sources.dedup();
        }

        Self {
            output,
            function,
            function_fact,
            nodes,
            blocks,
            edges,
            successors,
            predecessors,
        }
    }

    fn from_grouped(
        output: &'a CfgOutput,
        function: CfgFunctionId,
        function_fact: Option<&'a CfgFunctionFact>,
        nodes: Vec<&'a CfgNodeFact>,
        blocks: Vec<&'a BasicBlockFact>,
        edges: Vec<&'a CfgEdgeFact>,
    ) -> Self {
        let mut successors = BTreeMap::<BasicBlockId, Vec<BasicBlockId>>::new();
        let mut predecessors = BTreeMap::<BasicBlockId, Vec<BasicBlockId>>::new();
        for edge in &edges {
            if edge.from_block == edge.to_block {
                continue;
            }
            successors
                .entry(edge.from_block)
                .or_default()
                .push(edge.to_block);
            predecessors
                .entry(edge.to_block)
                .or_default()
                .push(edge.from_block);
        }
        for targets in successors.values_mut() {
            targets.sort();
            targets.dedup();
        }
        for sources in predecessors.values_mut() {
            sources.sort();
            sources.dedup();
        }

        Self {
            output,
            function,
            function_fact,
            nodes,
            blocks,
            edges,
            successors,
            predecessors,
        }
    }

    pub(crate) fn function_id(&self) -> CfgFunctionId {
        self.function
    }

    pub(crate) fn function_stable_key(&self) -> String {
        self.function_fact
            .map(|function| function.stable_key.clone())
            .unwrap_or_else(|| format!("<missing-function:{}>", self.function.0))
    }

    pub(crate) fn function(&self) -> Option<&'a CfgFunctionFact> {
        self.function_fact
    }

    pub(crate) fn nodes(&self) -> Vec<&'a crate::analysis::cfg::facts::CfgNodeFact> {
        self.nodes.clone()
    }

    pub(crate) fn blocks(&self) -> Vec<&'a BasicBlockFact> {
        self.blocks.clone()
    }

    pub(crate) fn block_refs(&self) -> &[&'a BasicBlockFact] {
        &self.blocks
    }

    pub(crate) fn edge_refs(&self) -> &[&'a CfgEdgeFact] {
        &self.edges
    }

    pub(crate) fn successors(&self, block: BasicBlockId) -> Vec<BasicBlockId> {
        self.successor_blocks(block).to_vec()
    }

    pub(crate) fn successor_blocks(&self, block: BasicBlockId) -> &[BasicBlockId] {
        self.successors
            .get(&block)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub(crate) fn predecessors(&self, block: BasicBlockId) -> Vec<BasicBlockId> {
        self.predecessor_blocks(block).to_vec()
    }

    pub(crate) fn predecessor_blocks(&self, block: BasicBlockId) -> &[BasicBlockId] {
        self.predecessors
            .get(&block)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub(crate) fn entry_block(&self) -> Option<BasicBlockId> {
        self.blocks
            .iter()
            .find(|block| block.kind == BasicBlockKind::Entry)
            .map(|block| block.id)
    }

    pub(crate) fn synthetic_exit_block(&self, _view: CfgView) -> Option<BasicBlockId> {
        self.blocks
            .iter()
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

    fn body(interner: &crate::core::StableKeyInterner) -> MirBody {
        MirBody {
            id: MirBodyId(1),
            language: Language::Go,
            file: FileId(1),
            function: FunctionId(1),
            package: None,
            module: None,
            owner_stable_key: interner.intern("owner"),
            span: span(),
            stable_key: interner.intern("body:one"),
            status: MirStatus::Resolved,
        }
    }

    fn op(interner: &crate::core::StableKeyInterner, id: u64, ordinal: u32) -> MirOperation {
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
            stable_key: interner.intern(format!("op:{ordinal}")),
            status: MirStatus::Resolved,
        }
    }

    #[test]
    fn graph_view_exposes_sorted_block_successors_and_predecessors() {
        let interner = crate::core::StableKeyInterner::default();
        let mut builder = CfgBuilder::new();
        let function = builder.start_function(&interner, &body(&interner), false);
        let entry = builder.current_block();
        let branch = builder.start_block(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            BasicBlockKind::Branch,
        );
        builder.append_operation_node(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            Some(&op(&interner, 1, 1)),
            CfgNodeKind::Condition,
            Some(span()),
        );
        let then_block = builder.start_block(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            BasicBlockKind::StraightLine,
        );
        builder.append_operation_node(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            Some(&op(&interner, 2, 2)),
            CfgNodeKind::Operation,
            Some(span()),
        );
        let else_block = builder.start_block(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            BasicBlockKind::StraightLine,
        );
        builder.append_operation_node(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            Some(&op(&interner, 3, 3)),
            CfgNodeKind::Operation,
            Some(span()),
        );

        builder.add_edge(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            entry,
            branch,
            CfgEdgeKind::Normal,
        );
        builder.add_edge(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            branch,
            else_block,
            CfgEdgeKind::False,
        );
        builder.add_edge(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            branch,
            then_block,
            CfgEdgeKind::True,
        );
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

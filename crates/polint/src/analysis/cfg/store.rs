use crate::analysis::cfg::facts::{
    BasicBlockFact, CfgEdgeFact, CfgFunctionFact, CfgNodeFact, ControlDependenceFact,
    DominatorFact, PostDominatorFact, ReachabilityFact, UnsupportedControlFlowFact,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CfgOutput {
    pub(crate) functions: Vec<CfgFunctionFact>,
    pub(crate) nodes: Vec<CfgNodeFact>,
    pub(crate) blocks: Vec<BasicBlockFact>,
    pub(crate) edges: Vec<CfgEdgeFact>,
    pub(crate) reachability: Vec<ReachabilityFact>,
    pub(crate) dominators: Vec<DominatorFact>,
    pub(crate) postdominators: Vec<PostDominatorFact>,
    pub(crate) control_dependence: Vec<ControlDependenceFact>,
    pub(crate) unsupported: Vec<UnsupportedControlFlowFact>,
}

impl CfgOutput {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    pub(crate) fn normalized(mut self) -> Self {
        self.functions
            .sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        self.nodes.sort_by(|left, right| {
            (
                left.cfg_function,
                left.operation_ordinal,
                left.stable_key.as_str(),
            )
                .cmp(&(
                    right.cfg_function,
                    right.operation_ordinal,
                    right.stable_key.as_str(),
                ))
        });
        self.blocks
            .sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        self.edges.sort_by(|left, right| {
            (
                left.cfg_function,
                left.view,
                left.from_block,
                left.to_block,
                left.kind,
                left.stable_key.as_str(),
            )
                .cmp(&(
                    right.cfg_function,
                    right.view,
                    right.from_block,
                    right.to_block,
                    right.kind,
                    right.stable_key.as_str(),
                ))
        });
        self.reachability
            .sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        self.dominators
            .sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        self.postdominators
            .sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        self.control_dependence
            .sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        self.unsupported
            .sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::cfg::facts::{BasicBlockKind, CfgNodeKind, CfgPrecision, CfgStatus};
    use crate::analysis::cfg::ids::{BasicBlockId, CfgFunctionId, CfgNodeId};
    use crate::analysis::ids::MirBodyId;
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

    fn function(id: u64, stable_key: &str) -> CfgFunctionFact {
        CfgFunctionFact {
            id: CfgFunctionId(id),
            body: MirBodyId(id),
            function: FunctionId(id),
            language: Language::Go,
            file: FileId(1),
            span: span(),
            entry_node: CfgNodeId(1),
            normal_exit_node: CfgNodeId(2),
            exceptional_exit_node: None,
            stable_key: stable_key.to_string(),
            status: CfgStatus::Resolved,
            precision: CfgPrecision::ExactSyntax,
        }
    }

    fn node(function: u64, ordinal: u32, stable_key: &str) -> CfgNodeFact {
        CfgNodeFact {
            id: CfgNodeId(u64::from(ordinal)),
            cfg_function: CfgFunctionId(function),
            body: MirBodyId(function),
            operation: None,
            block: BasicBlockId(function),
            kind: CfgNodeKind::Operation,
            span: Some(span()),
            generated: false,
            operation_ordinal: ordinal,
            stable_key: stable_key.to_string(),
            status: CfgStatus::Resolved,
            precision: CfgPrecision::ExactSyntax,
        }
    }

    fn block(stable_key: &str) -> BasicBlockFact {
        BasicBlockFact {
            id: BasicBlockId(1),
            cfg_function: CfgFunctionId(1),
            kind: BasicBlockKind::StraightLine,
            first_node: Some(CfgNodeId(1)),
            last_node: Some(CfgNodeId(1)),
            reachable: true,
            reverse_postorder: 0,
            stable_key: stable_key.to_string(),
            status: CfgStatus::Resolved,
            precision: CfgPrecision::ExactSyntax,
        }
    }

    #[test]
    fn normalized_sorts_families_without_deduplicating() {
        let output = CfgOutput {
            functions: vec![function(2, "b"), function(1, "a")],
            nodes: vec![node(1, 2, "b"), node(1, 1, "a")],
            blocks: vec![block("b"), block("a"), block("a")],
            ..CfgOutput::empty()
        }
        .normalized();

        assert_eq!(
            output
                .functions
                .iter()
                .map(|fact| fact.stable_key.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b"]
        );
        assert_eq!(
            output
                .nodes
                .iter()
                .map(|fact| fact.operation_ordinal)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(
            output
                .blocks
                .iter()
                .map(|fact| fact.stable_key.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "a", "b"]
        );
    }

    #[test]
    fn empty_output_contains_no_rows() {
        let output = CfgOutput::empty().normalized();
        assert!(output.functions.is_empty());
        assert!(output.control_dependence.is_empty());
    }
}

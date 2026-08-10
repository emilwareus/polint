use serde::{Deserialize, Serialize};

use crate::analysis::cfg::ids::{
    BasicBlockId, CfgEdgeId, CfgFunctionId, CfgNodeId, ControlDependenceId, DominatorId,
    PostDominatorId, ReachabilityId, UnsupportedControlFlowId,
};
use crate::analysis::ids::{MirBodyId, MirOpId};
use crate::core::{FileId, FunctionId, Language, Span, StableKeyId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CfgFunctionFact {
    pub(crate) id: CfgFunctionId,
    pub(crate) body: MirBodyId,
    pub(crate) function: FunctionId,
    pub(crate) language: Language,
    pub(crate) file: FileId,
    pub(crate) span: Span,
    pub(crate) entry_node: CfgNodeId,
    pub(crate) normal_exit_node: CfgNodeId,
    pub(crate) exceptional_exit_node: Option<CfgNodeId>,
    pub(crate) stable_key: StableKeyId,
    pub(crate) status: CfgStatus,
    pub(crate) precision: CfgPrecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CfgNodeFact {
    pub(crate) id: CfgNodeId,
    pub(crate) cfg_function: CfgFunctionId,
    pub(crate) body: MirBodyId,
    pub(crate) operation: Option<MirOpId>,
    pub(crate) block: BasicBlockId,
    pub(crate) kind: CfgNodeKind,
    pub(crate) span: Option<Span>,
    pub(crate) generated: bool,
    pub(crate) operation_ordinal: u32,
    pub(crate) stable_key: StableKeyId,
    pub(crate) status: CfgStatus,
    pub(crate) precision: CfgPrecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BasicBlockFact {
    pub(crate) id: BasicBlockId,
    pub(crate) cfg_function: CfgFunctionId,
    pub(crate) kind: BasicBlockKind,
    pub(crate) first_node: Option<CfgNodeId>,
    pub(crate) last_node: Option<CfgNodeId>,
    pub(crate) reachable: bool,
    pub(crate) reverse_postorder: u32,
    pub(crate) stable_key: StableKeyId,
    pub(crate) status: CfgStatus,
    pub(crate) precision: CfgPrecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CfgEdgeFact {
    pub(crate) id: CfgEdgeId,
    pub(crate) cfg_function: CfgFunctionId,
    pub(crate) view: CfgView,
    pub(crate) from: CfgNodeId,
    pub(crate) to: CfgNodeId,
    pub(crate) from_block: BasicBlockId,
    pub(crate) to_block: BasicBlockId,
    pub(crate) kind: CfgEdgeKind,
    pub(crate) label: Option<String>,
    pub(crate) stable_key: StableKeyId,
    pub(crate) status: CfgStatus,
    pub(crate) precision: CfgPrecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReachabilityFact {
    pub(crate) id: ReachabilityId,
    pub(crate) cfg_function: CfgFunctionId,
    pub(crate) view: CfgView,
    pub(crate) block: BasicBlockId,
    pub(crate) reachable: bool,
    pub(crate) stable_key: StableKeyId,
    pub(crate) status: CfgStatus,
    pub(crate) precision: CfgPrecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DominatorFact {
    pub(crate) id: DominatorId,
    pub(crate) cfg_function: CfgFunctionId,
    pub(crate) view: CfgView,
    pub(crate) dominator: BasicBlockId,
    pub(crate) dominated: BasicBlockId,
    pub(crate) immediate: bool,
    pub(crate) stable_key: StableKeyId,
    pub(crate) status: CfgStatus,
    pub(crate) precision: CfgPrecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PostDominatorFact {
    pub(crate) id: PostDominatorId,
    pub(crate) cfg_function: CfgFunctionId,
    pub(crate) view: CfgView,
    pub(crate) postdominator: BasicBlockId,
    pub(crate) postdominated: BasicBlockId,
    pub(crate) immediate: bool,
    pub(crate) stable_key: StableKeyId,
    pub(crate) status: CfgStatus,
    pub(crate) precision: CfgPrecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ControlDependenceFact {
    pub(crate) id: ControlDependenceId,
    pub(crate) cfg_function: CfgFunctionId,
    pub(crate) view: CfgView,
    pub(crate) controlling_edge: CfgEdgeId,
    pub(crate) controlling_edge_kind: CfgEdgeKind,
    pub(crate) controlled_block: BasicBlockId,
    pub(crate) stable_key: StableKeyId,
    pub(crate) status: CfgStatus,
    pub(crate) precision: CfgPrecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnsupportedControlFlowFact {
    pub(crate) id: UnsupportedControlFlowId,
    pub(crate) cfg_function: Option<CfgFunctionId>,
    pub(crate) body: Option<MirBodyId>,
    pub(crate) operation: Option<MirOpId>,
    pub(crate) language: Language,
    pub(crate) file: FileId,
    pub(crate) span: Span,
    pub(crate) construct: String,
    pub(crate) source_evidence: String,
    pub(crate) conservative_action: ControlFlowAction,
    pub(crate) stable_key: StableKeyId,
    pub(crate) status: CfgStatus,
    pub(crate) precision: CfgPrecision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum CfgNodeKind {
    Entry,
    ExitNormal,
    ExitExceptional,
    Operation,
    Condition,
    CallSite,
    Return,
    Throw,
    Panic,
    Break,
    Continue,
    Goto,
    Yield,
    Await,
    Defer,
    RunDefers,
    FinallyEnter,
    FinallyExit,
    Synthetic,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum BasicBlockKind {
    Entry,
    ExitNormal,
    ExitExceptional,
    StraightLine,
    Branch,
    LoopHeader,
    LoopBody,
    Join,
    Cleanup,
    Unreachable,
    Synthetic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum CfgEdgeKind {
    Normal,
    True,
    False,
    SwitchCase,
    DefaultCase,
    LoopEnter,
    LoopBack,
    LoopExit,
    Break,
    Continue,
    Goto,
    Return,
    Throw,
    ImplicitThrow,
    Panic,
    Recover,
    Finally,
    Cleanup,
    Defer,
    ShortCircuit,
    OptionalChain,
    Nullish,
    YieldSuspend,
    YieldResume,
    AwaitSuspend,
    AwaitResume,
    Spawn,
    Unreachable,
    Unknown,
    Synthetic,
    Extension,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum CfgView {
    NormalControl,
    AbruptAware,
    ExceptionConservative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum CfgStatus {
    Resolved,
    Partial,
    Unknown,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum CfgPrecision {
    ExactSyntax,
    ExactLowered,
    SetupAware,
    Conservative,
    Heuristic,
    Unknown,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum ControlFlowAction {
    PreserveUnknownEdge,
    StopAtBoundary,
    AddConservativeExit,
    SkipUnreachableTail,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::cfg::ids::{BasicBlockId, CfgEdgeId, CfgFunctionId, CfgNodeId};
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

    #[test]
    fn cfg_facts_keep_stable_keys_separate_from_dense_ids() {
        let interner = crate::core::StableKeyInterner::default();
        let function = CfgFunctionFact {
            id: CfgFunctionId(7),
            body: MirBodyId(9),
            function: FunctionId(1),
            language: Language::Go,
            file: FileId(1),
            span: span(),
            entry_node: CfgNodeId(1),
            normal_exit_node: CfgNodeId(2),
            exceptional_exit_node: None,
            stable_key: interner.intern("cfg:function:src/app.go:f"),
            status: CfgStatus::Resolved,
            precision: CfgPrecision::ExactSyntax,
        };

        assert!(
            !interner
                .resolve(function.stable_key)
                .contains(&function.id.0.to_string())
        );
    }

    #[test]
    fn cfg_edge_kind_covers_required_control_transfers() {
        let kinds = [
            CfgEdgeKind::Normal,
            CfgEdgeKind::True,
            CfgEdgeKind::False,
            CfgEdgeKind::LoopBack,
            CfgEdgeKind::Return,
            CfgEdgeKind::ImplicitThrow,
            CfgEdgeKind::Cleanup,
            CfgEdgeKind::ShortCircuit,
            CfgEdgeKind::AwaitSuspend,
            CfgEdgeKind::Spawn,
            CfgEdgeKind::Synthetic,
        ];

        assert_eq!(kinds.len(), 11);
    }

    #[test]
    fn edge_facts_carry_view_and_block_endpoints() {
        let interner = crate::core::StableKeyInterner::default();
        let edge = CfgEdgeFact {
            id: CfgEdgeId(1),
            cfg_function: CfgFunctionId(1),
            view: CfgView::AbruptAware,
            from: CfgNodeId(1),
            to: CfgNodeId(2),
            from_block: BasicBlockId(1),
            to_block: BasicBlockId(2),
            kind: CfgEdgeKind::Return,
            label: Some("return".to_string()),
            stable_key: interner.intern("edge:return"),
            status: CfgStatus::Resolved,
            precision: CfgPrecision::ExactLowered,
        };

        assert_eq!(edge.view, CfgView::AbruptAware);
        assert_eq!(edge.kind, CfgEdgeKind::Return);
    }
}

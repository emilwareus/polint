use serde::{Deserialize, Serialize};

use crate::cfg::ids::{
    BasicBlockId, CfgEdgeId, CfgFunctionId, CfgNodeId, ControlDependenceId, DominatorId,
    PostDominatorId, ReachabilityId, UnsupportedControlFlowId,
};
use crate::ids::{MirBodyId, MirOpId};
use polint_core::{FileId, FunctionId, Language, Span, StableKeyId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfgFunctionFact {
    pub id: CfgFunctionId,
    pub body: MirBodyId,
    pub function: FunctionId,
    pub language: Language,
    pub file: FileId,
    pub span: Span,
    pub entry_node: CfgNodeId,
    pub normal_exit_node: CfgNodeId,
    pub exceptional_exit_node: Option<CfgNodeId>,
    pub stable_key: StableKeyId,
    pub status: CfgStatus,
    pub precision: CfgPrecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfgNodeFact {
    pub id: CfgNodeId,
    pub cfg_function: CfgFunctionId,
    pub body: MirBodyId,
    pub operation: Option<MirOpId>,
    pub block: BasicBlockId,
    pub kind: CfgNodeKind,
    pub span: Option<Span>,
    pub generated: bool,
    pub operation_ordinal: u32,
    pub stable_key: StableKeyId,
    pub status: CfgStatus,
    pub precision: CfgPrecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicBlockFact {
    pub id: BasicBlockId,
    pub cfg_function: CfgFunctionId,
    pub kind: BasicBlockKind,
    pub first_node: Option<CfgNodeId>,
    pub last_node: Option<CfgNodeId>,
    pub reachable: bool,
    pub reverse_postorder: u32,
    pub stable_key: StableKeyId,
    pub status: CfgStatus,
    pub precision: CfgPrecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfgEdgeFact {
    pub id: CfgEdgeId,
    pub cfg_function: CfgFunctionId,
    pub view: CfgView,
    pub from: CfgNodeId,
    pub to: CfgNodeId,
    pub from_block: BasicBlockId,
    pub to_block: BasicBlockId,
    pub kind: CfgEdgeKind,
    pub label: Option<String>,
    pub stable_key: StableKeyId,
    pub status: CfgStatus,
    pub precision: CfgPrecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReachabilityFact {
    pub id: ReachabilityId,
    pub cfg_function: CfgFunctionId,
    pub view: CfgView,
    pub block: BasicBlockId,
    pub reachable: bool,
    pub stable_key: StableKeyId,
    pub status: CfgStatus,
    pub precision: CfgPrecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DominatorFact {
    pub id: DominatorId,
    pub cfg_function: CfgFunctionId,
    pub view: CfgView,
    pub dominator: BasicBlockId,
    pub dominated: BasicBlockId,
    pub immediate: bool,
    pub stable_key: StableKeyId,
    pub status: CfgStatus,
    pub precision: CfgPrecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostDominatorFact {
    pub id: PostDominatorId,
    pub cfg_function: CfgFunctionId,
    pub view: CfgView,
    pub postdominator: BasicBlockId,
    pub postdominated: BasicBlockId,
    pub immediate: bool,
    pub stable_key: StableKeyId,
    pub status: CfgStatus,
    pub precision: CfgPrecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlDependenceFact {
    pub id: ControlDependenceId,
    pub cfg_function: CfgFunctionId,
    pub view: CfgView,
    pub controlling_edge: CfgEdgeId,
    pub controlling_edge_kind: CfgEdgeKind,
    pub controlled_block: BasicBlockId,
    pub stable_key: StableKeyId,
    pub status: CfgStatus,
    pub precision: CfgPrecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedControlFlowFact {
    pub id: UnsupportedControlFlowId,
    pub cfg_function: Option<CfgFunctionId>,
    pub body: Option<MirBodyId>,
    pub operation: Option<MirOpId>,
    pub language: Language,
    pub file: FileId,
    pub span: Span,
    pub construct: String,
    pub source_evidence: String,
    pub conservative_action: ControlFlowAction,
    pub stable_key: StableKeyId,
    pub status: CfgStatus,
    pub precision: CfgPrecision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CfgNodeKind {
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
pub enum BasicBlockKind {
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
pub enum CfgEdgeKind {
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
pub enum CfgView {
    NormalControl,
    AbruptAware,
    ExceptionConservative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CfgStatus {
    Resolved,
    Partial,
    Unknown,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CfgPrecision {
    ExactSyntax,
    ExactLowered,
    SetupAware,
    Conservative,
    Heuristic,
    Unknown,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ControlFlowAction {
    PreserveUnknownEdge,
    StopAtBoundary,
    AddConservativeExit,
    SkipUnreachableTail,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::ids::{BasicBlockId, CfgEdgeId, CfgFunctionId, CfgNodeId};
    use crate::ids::MirBodyId;
    use polint_core::{FileId, FunctionId, Language, Span};

    fn span() -> Span {
        Span::new(FileId::from_raw(1), 1, 2, 1, 1, 1, 2)
    }

    #[test]
    fn cfg_facts_keep_stable_keys_separate_from_dense_ids() {
        let interner = polint_core::StableKeyInterner::default();
        let function = CfgFunctionFact {
            id: CfgFunctionId(7),
            body: MirBodyId(9),
            function: FunctionId::from_raw(1),
            language: Language::Go,
            file: FileId::from_raw(1),
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
        let interner = polint_core::StableKeyInterner::default();
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

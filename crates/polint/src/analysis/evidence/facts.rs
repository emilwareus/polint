use serde::{Deserialize, Serialize};

use crate::analysis::cfg::ids::CfgNodeId;
use crate::analysis::ids::{
    CallSiteId, EvidenceBundleId, EvidenceEdgeId, EvidenceNodeId, EvidenceOmittedRegionId,
    EvidencePathId, EvidenceSliceId, MirBodyId, MirOpId, PlaceId,
};
use crate::core::{FileId, FunctionId, Language, ReferenceId, Span, SymbolId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct EvidenceNodeFact {
    pub(crate) id: EvidenceNodeId,
    pub(crate) kind: EvidenceNodeKind,
    pub(crate) language: Language,
    pub(crate) file: Option<FileId>,
    pub(crate) function: Option<FunctionId>,
    pub(crate) body: Option<MirBodyId>,
    pub(crate) operation: Option<MirOpId>,
    pub(crate) cfg_node: Option<CfgNodeId>,
    pub(crate) place: Option<PlaceId>,
    pub(crate) symbol: Option<SymbolId>,
    pub(crate) reference: Option<ReferenceId>,
    pub(crate) call_site: Option<CallSiteId>,
    pub(crate) span: Option<Span>,
    pub(crate) status: EvidenceStatus,
    pub(crate) precision: EvidencePrecision,
    pub(crate) provenance: EvidenceProvenance,
    pub(crate) validation: EvidenceValidation,
    pub(crate) confidence: EvidenceConfidence,
    pub(crate) compact_label: Option<String>,
    pub(crate) source_fact_stable_keys: Vec<String>,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct EvidenceEdgeFact {
    pub(crate) id: EvidenceEdgeId,
    pub(crate) from: EvidenceNodeId,
    pub(crate) to: EvidenceNodeId,
    pub(crate) kind: EvidenceEdgeKind,
    pub(crate) query_mode: EvidenceQueryMode,
    pub(crate) status: EvidenceStatus,
    pub(crate) precision: EvidencePrecision,
    pub(crate) provenance: EvidenceProvenance,
    pub(crate) validation: EvidenceValidation,
    pub(crate) confidence: EvidenceConfidence,
    pub(crate) call_site: Option<CallSiteId>,
    pub(crate) summary_stable_key: Option<String>,
    pub(crate) expansion: EvidenceExpansion,
    pub(crate) compact_label: Option<String>,
    pub(crate) source_fact_stable_keys: Vec<String>,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct EvidenceBundleFact {
    pub(crate) id: EvidenceBundleId,
    pub(crate) diagnostic_stable_key: String,
    pub(crate) query_mode: EvidenceQueryMode,
    pub(crate) status: EvidenceStatus,
    pub(crate) precision: EvidencePrecision,
    pub(crate) provenance: EvidenceProvenance,
    pub(crate) validation: EvidenceValidation,
    pub(crate) confidence: EvidenceConfidence,
    pub(crate) entry_node: Option<EvidenceNodeId>,
    pub(crate) selected_paths: Vec<EvidencePathId>,
    pub(crate) selected_slices: Vec<EvidenceSliceId>,
    pub(crate) replay_key: Option<String>,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct EvidencePathFact {
    pub(crate) id: EvidencePathId,
    pub(crate) bundle: Option<EvidenceBundleId>,
    pub(crate) query_mode: EvidenceQueryMode,
    pub(crate) nodes: Vec<EvidenceNodeId>,
    pub(crate) edges: Vec<EvidenceEdgeId>,
    pub(crate) rank: u32,
    pub(crate) score: EvidenceRankScore,
    pub(crate) status: EvidenceStatus,
    pub(crate) hidden_node_count: u32,
    pub(crate) omitted_regions: Vec<EvidenceOmittedRegionId>,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct EvidenceSliceFact {
    pub(crate) id: EvidenceSliceId,
    pub(crate) bundle: Option<EvidenceBundleId>,
    pub(crate) query_mode: EvidenceQueryMode,
    pub(crate) root_nodes: Vec<EvidenceNodeId>,
    pub(crate) nodes: Vec<EvidenceNodeId>,
    pub(crate) edges: Vec<EvidenceEdgeId>,
    pub(crate) status: EvidenceStatus,
    pub(crate) omitted_regions: Vec<EvidenceOmittedRegionId>,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct EvidenceUnknownFact {
    pub(crate) bundle: Option<EvidenceBundleId>,
    pub(crate) path: Option<EvidencePathId>,
    pub(crate) slice: Option<EvidenceSliceId>,
    pub(crate) edge: Option<EvidenceEdgeId>,
    pub(crate) reason: EvidenceUnknownReason,
    pub(crate) message: String,
    pub(crate) source_fact_stable_keys: Vec<String>,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct EvidenceOmittedRegionFact {
    pub(crate) id: EvidenceOmittedRegionId,
    pub(crate) bundle: Option<EvidenceBundleId>,
    pub(crate) path: Option<EvidencePathId>,
    pub(crate) slice: Option<EvidenceSliceId>,
    pub(crate) reason: EvidenceOmittedReason,
    pub(crate) hidden_node_count: u32,
    pub(crate) hidden_edge_count: u32,
    pub(crate) budget_label: Option<String>,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct EvidenceReplayKeyFact {
    pub(crate) bundle: EvidenceBundleId,
    pub(crate) query_mode: EvidenceQueryMode,
    pub(crate) graph_schema: String,
    pub(crate) query_budget: EvidenceQueryBudget,
    pub(crate) ranking: EvidenceRankingMode,
    pub(crate) renderer: EvidenceRendererMode,
    pub(crate) upstream_digest_keys: Vec<String>,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum EvidenceNodeKind {
    Operation,
    Statement,
    Symbol,
    Place,
    CallSite,
    FunctionEntry,
    FunctionExit,
    Summary,
    Model,
    Diagnostic,
    Synthetic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum EvidenceEdgeKind {
    DataValue,
    DataTaint,
    DataAddress,
    Control,
    Call,
    Return,
    ParameterIn,
    ParameterOut,
    Summary,
    Model,
    Alias,
    Unknown,
    ExplanationOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum EvidenceQueryMode {
    ThinBackward,
    FullBackward,
    ForwardImpact,
    Chop,
    Path,
    Expansion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum EvidenceStatus {
    Present,
    Partial,
    Unknown,
    Unsupported,
    SetupMissing,
    BudgetExceeded,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum EvidencePrecision {
    Exact,
    SetupAware,
    Syntax,
    Conservative,
    Heuristic,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum EvidenceProvenance {
    Native,
    Summary,
    Extension,
    Model,
    Query,
    Synthetic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum EvidenceValidation {
    Native,
    ReferentiallyValidated,
    ExtensionValidated,
    BudgetValidated,
    RendererValidated,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum EvidenceConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum EvidenceExpansion {
    None,
    Expandable { key: String },
    Opaque { reason: String },
    ExternalModel { model: String },
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub(crate) struct EvidenceRankScore {
    pub(crate) native_exact_edges: u32,
    pub(crate) unknown_edges: u32,
    pub(crate) unvalidated_edges: u32,
    pub(crate) model_or_heuristic_edges: u32,
    pub(crate) edge_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum EvidenceUnknownReason {
    DynamicCall,
    UnsupportedEdge,
    SetupMissing,
    BudgetExceeded,
    OpaqueSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum EvidenceOmittedReason {
    NodeLimit,
    EdgeLimit,
    PathDepth,
    PathCount,
    CompactRendering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum EvidenceRankingMode {
    DeterministicDisplay,
    StableKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum EvidenceRendererMode {
    Json,
    Sarif,
    Human,
    Debug,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct EvidenceQueryBudget {
    pub(crate) max_paths: u32,
    pub(crate) max_nodes: u32,
    pub(crate) max_edges: u32,
    pub(crate) max_depth: u32,
}

impl Default for EvidenceQueryBudget {
    fn default() -> Self {
        Self {
            max_paths: 5,
            max_nodes: 64,
            max_edges: 96,
            max_depth: 32,
        }
    }
}

impl EvidenceNodeFact {
    pub(crate) fn normalized(mut self) -> Self {
        self.source_fact_stable_keys.sort();
        self.source_fact_stable_keys.dedup();
        self
    }
}

impl EvidenceEdgeFact {
    pub(crate) fn normalized(mut self) -> Self {
        self.source_fact_stable_keys.sort();
        self.source_fact_stable_keys.dedup();
        self
    }
}

impl EvidenceBundleFact {
    pub(crate) fn normalized(mut self) -> Self {
        self.selected_paths.sort();
        self.selected_paths.dedup();
        self.selected_slices.sort();
        self.selected_slices.dedup();
        self
    }
}

impl EvidencePathFact {
    pub(crate) fn normalized(mut self) -> Self {
        self.omitted_regions.sort();
        self.omitted_regions.dedup();
        self
    }
}

impl EvidenceSliceFact {
    pub(crate) fn normalized(mut self) -> Self {
        self.root_nodes.sort();
        self.root_nodes.dedup();
        self.nodes.sort();
        self.nodes.dedup();
        self.edges.sort();
        self.edges.dedup();
        self.omitted_regions.sort();
        self.omitted_regions.dedup();
        self
    }
}

impl EvidenceUnknownFact {
    pub(crate) fn normalized(mut self) -> Self {
        self.source_fact_stable_keys.sort();
        self.source_fact_stable_keys.dedup();
        self
    }
}

impl EvidenceReplayKeyFact {
    pub(crate) fn normalized(mut self) -> Self {
        self.upstream_digest_keys.sort();
        self.upstream_digest_keys.dedup();
        self
    }
}

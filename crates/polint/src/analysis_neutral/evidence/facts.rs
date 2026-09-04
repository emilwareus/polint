use std::sync::Arc;
use serde::{Deserialize, Serialize};

use crate::analysis_neutral::cfg::ids::CfgNodeId;
use crate::analysis_neutral::ids::{
    CallSiteId, EvidenceBundleId, EvidenceEdgeId, EvidenceNodeId, EvidenceOmittedRegionId,
    EvidencePathId, EvidenceSliceId, MirBodyId, MirOpId, PlaceId,
};
use crate::internal_core::{
    FileId, FunctionId, Language, ReferenceId, Span, StableKeyId, SymbolId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceNodeFact {
    pub id: EvidenceNodeId,
    pub kind: EvidenceNodeKind,
    pub language: Language,
    pub file: Option<FileId>,
    pub function: Option<FunctionId>,
    pub body: Option<MirBodyId>,
    pub operation: Option<MirOpId>,
    pub cfg_node: Option<CfgNodeId>,
    pub place: Option<PlaceId>,
    pub symbol: Option<SymbolId>,
    pub reference: Option<ReferenceId>,
    pub call_site: Option<CallSiteId>,
    pub span: Option<Span>,
    pub status: EvidenceStatus,
    pub precision: EvidencePrecision,
    pub provenance: EvidenceProvenance,
    pub validation: EvidenceValidation,
    pub confidence: EvidenceConfidence,
    pub compact_label: Option<String>,
    pub source_fact_stable_keys: Vec<Arc<str>>,
    pub stable_key: StableKeyId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceEdgeFact {
    pub id: EvidenceEdgeId,
    pub from: EvidenceNodeId,
    pub to: EvidenceNodeId,
    pub kind: EvidenceEdgeKind,
    pub query_mode: EvidenceQueryMode,
    pub status: EvidenceStatus,
    pub precision: EvidencePrecision,
    pub provenance: EvidenceProvenance,
    pub validation: EvidenceValidation,
    pub confidence: EvidenceConfidence,
    pub call_site: Option<CallSiteId>,
    pub summary_stable_key: Option<Arc<str>>,
    pub expansion: EvidenceExpansion,
    pub compact_label: Option<String>,
    pub source_fact_stable_keys: Vec<Arc<str>>,
    pub stable_key: StableKeyId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceBundleFact {
    pub id: EvidenceBundleId,
    pub diagnostic_stable_key: StableKeyId,
    pub query_mode: EvidenceQueryMode,
    pub status: EvidenceStatus,
    pub precision: EvidencePrecision,
    pub provenance: EvidenceProvenance,
    pub validation: EvidenceValidation,
    pub confidence: EvidenceConfidence,
    pub entry_node: Option<EvidenceNodeId>,
    pub selected_paths: Vec<EvidencePathId>,
    pub selected_slices: Vec<EvidenceSliceId>,
    pub replay_key: Option<String>,
    pub stable_key: StableKeyId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidencePathFact {
    pub id: EvidencePathId,
    pub bundle: Option<EvidenceBundleId>,
    pub query_mode: EvidenceQueryMode,
    pub nodes: Vec<EvidenceNodeId>,
    pub edges: Vec<EvidenceEdgeId>,
    pub rank: u32,
    pub score: EvidenceRankScore,
    pub status: EvidenceStatus,
    pub hidden_node_count: u32,
    pub omitted_regions: Vec<EvidenceOmittedRegionId>,
    pub stable_key: StableKeyId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceSliceFact {
    pub id: EvidenceSliceId,
    pub bundle: Option<EvidenceBundleId>,
    pub query_mode: EvidenceQueryMode,
    pub root_nodes: Vec<EvidenceNodeId>,
    pub nodes: Vec<EvidenceNodeId>,
    pub edges: Vec<EvidenceEdgeId>,
    pub status: EvidenceStatus,
    pub omitted_regions: Vec<EvidenceOmittedRegionId>,
    pub stable_key: StableKeyId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceUnknownFact {
    pub bundle: Option<EvidenceBundleId>,
    pub path: Option<EvidencePathId>,
    pub slice: Option<EvidenceSliceId>,
    pub edge: Option<EvidenceEdgeId>,
    pub reason: EvidenceUnknownReason,
    pub message: String,
    pub source_fact_stable_keys: Vec<Arc<str>>,
    pub stable_key: StableKeyId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceOmittedRegionFact {
    pub id: EvidenceOmittedRegionId,
    pub bundle: Option<EvidenceBundleId>,
    pub path: Option<EvidencePathId>,
    pub slice: Option<EvidenceSliceId>,
    pub reason: EvidenceOmittedReason,
    pub hidden_node_count: u32,
    pub hidden_edge_count: u32,
    pub budget_label: Option<String>,
    pub stable_key: StableKeyId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceReplayKeyFact {
    pub bundle: EvidenceBundleId,
    pub query_mode: EvidenceQueryMode,
    pub graph_schema: String,
    pub query_budget: EvidenceQueryBudget,
    pub ranking: EvidenceRankingMode,
    pub renderer: EvidenceRendererMode,
    pub upstream_digest_keys: Vec<String>,
    pub stable_key: StableKeyId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionEvidenceCandidateFact {
    pub extension_id: String,
    pub provider_id: String,
    pub stable_key: StableKeyId,
    pub from: EvidenceNodeId,
    pub to: EvidenceNodeId,
    pub kind: EvidenceEdgeKind,
    pub claimed_status: EvidenceStatus,
    pub claimed_precision: EvidencePrecision,
    pub confidence: EvidenceConfidence,
    pub source_path: Option<String>,
    pub source_span: Option<Span>,
    pub summary_stable_key: Option<Arc<str>>,
    pub expansion: EvidenceExpansion,
    pub replay_key: Option<String>,
    pub native_anchor_stable_keys: Vec<String>,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionEvidenceMergeFact {
    pub extension_id: String,
    pub provider_id: String,
    pub stable_key: StableKeyId,
    pub verdict: ExtensionEvidenceMergeVerdict,
    pub reason: Option<ExtensionEvidenceMergeReason>,
    pub effective_status: EvidenceStatus,
    pub effective_precision: EvidencePrecision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ExtensionEvidenceMergeVerdict {
    Accepted,
    AcceptedWithPrecisionDowngrade,
    CandidateOnly,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ExtensionEvidenceMergeReason {
    InvalidEndpoint,
    InvalidSpan,
    ExactClaimRequiresNativeAnchor,
    UnboundedExpansion,
    CandidateCannotStrengthenDiagnostic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EvidenceNodeKind {
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
pub enum EvidenceEdgeKind {
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
pub enum EvidenceQueryMode {
    ThinBackward,
    FullBackward,
    ForwardImpact,
    Chop,
    Path,
    Expansion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EvidenceStatus {
    Present,
    Partial,
    Unknown,
    Unsupported,
    SetupMissing,
    BudgetExceeded,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EvidencePrecision {
    Exact,
    SetupAware,
    Syntax,
    Conservative,
    Heuristic,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EvidenceProvenance {
    Native,
    Summary,
    Extension,
    Model,
    Query,
    Synthetic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EvidenceValidation {
    Native,
    ReferentiallyValidated,
    ExtensionValidated,
    BudgetValidated,
    RendererValidated,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EvidenceConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EvidenceExpansion {
    None,
    Expandable { key: String },
    Opaque { reason: String },
    ExternalModel { model: String },
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct EvidenceRankScore {
    pub native_exact_edges: u32,
    pub unknown_edges: u32,
    pub unvalidated_edges: u32,
    pub model_or_heuristic_edges: u32,
    pub edge_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EvidenceUnknownReason {
    DynamicCall,
    UnsupportedEdge,
    SetupMissing,
    BudgetExceeded,
    OpaqueSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EvidenceOmittedReason {
    NodeLimit,
    EdgeLimit,
    PathDepth,
    PathCount,
    CompactRendering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EvidenceRankingMode {
    DeterministicDisplay,
    StableKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum EvidenceRendererMode {
    Json,
    Sarif,
    Human,
    Debug,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceQueryBudget {
    pub max_paths: u32,
    pub max_nodes: u32,
    pub max_edges: u32,
    pub max_depth: u32,
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
    pub fn normalized(mut self) -> Self {
        self.source_fact_stable_keys.sort();
        self.source_fact_stable_keys.dedup();
        self
    }
}

impl EvidenceEdgeFact {
    pub fn normalized(mut self) -> Self {
        self.source_fact_stable_keys.sort();
        self.source_fact_stable_keys.dedup();
        self
    }
}

impl EvidenceBundleFact {
    pub fn normalized(mut self) -> Self {
        self.selected_paths.sort();
        self.selected_paths.dedup();
        self.selected_slices.sort();
        self.selected_slices.dedup();
        self
    }
}

impl EvidencePathFact {
    pub fn normalized(mut self) -> Self {
        self.omitted_regions.sort();
        self.omitted_regions.dedup();
        self
    }
}

impl EvidenceSliceFact {
    pub fn normalized(mut self) -> Self {
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
    pub fn normalized(mut self) -> Self {
        self.source_fact_stable_keys.sort();
        self.source_fact_stable_keys.dedup();
        self
    }
}

impl EvidenceReplayKeyFact {
    pub fn normalized(mut self) -> Self {
        self.upstream_digest_keys.sort();
        self.upstream_digest_keys.dedup();
        self
    }
}

impl ExtensionEvidenceCandidateFact {
    pub fn normalized(mut self) -> Self {
        self.native_anchor_stable_keys.sort();
        self.native_anchor_stable_keys.dedup();
        self.evidence.sort();
        self.evidence.dedup();
        self
    }
}

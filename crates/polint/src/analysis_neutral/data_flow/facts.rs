use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::analysis_neutral::cfg::ids::CfgNodeId;
use crate::analysis_neutral::ids::{
    CallSiteId, CallTargetId, DataFlowBudgetId, DataFlowEdgeId, DataFlowModelId, DataFlowNodeId,
    MirBodyId, MirOpId, PlaceId, RefinedCallEdgeId,
};
use crate::internal_core::{
    FileId, FunctionId, Language, ReferenceId, Span, StableKeyId, SymbolId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataFlowNodeFact {
    pub id: DataFlowNodeId,
    pub kind: DataFlowNodeKind,
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
    pub model: Option<DataFlowModelId>,
    pub span: Option<Span>,
    pub stable_key: StableKeyId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataFlowEdgeFact {
    pub id: DataFlowEdgeId,
    pub from: DataFlowNodeId,
    pub to: DataFlowNodeId,
    pub kind: DataFlowEdgeKind,
    pub algorithm: DataFlowAlgorithm,
    pub status: DataFlowStatus,
    pub precision: DataFlowPrecision,
    pub validation: DataFlowValidation,
    pub confidence: DataFlowConfidence,
    pub provenance: DataFlowProvenance,
    pub call_site: Option<CallSiteId>,
    pub call_target: Option<CallTargetId>,
    pub refined_call: Option<RefinedCallEdgeId>,
    pub model: Option<DataFlowModelId>,
    pub budget: Option<DataFlowBudgetId>,
    pub evidence: Vec<String>,
    pub input_stable_keys: Vec<Arc<str>>,
    pub stable_key: StableKeyId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataFlowModelFact {
    pub id: DataFlowModelId,
    pub kind: DataFlowModelKind,
    pub language: Language,
    pub provider_id: String,
    pub model_id: Option<String>,
    pub source_stable_key: Option<String>,
    pub status: DataFlowStatus,
    pub precision: DataFlowPrecision,
    pub validation: DataFlowValidation,
    pub confidence: DataFlowConfidence,
    pub provenance: DataFlowProvenance,
    pub evidence: Vec<String>,
    pub payload_labels: Vec<String>,
    pub stable_key: StableKeyId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataFlowBudgetFact {
    pub id: DataFlowBudgetId,
    pub reason: DataFlowBudgetReason,
    pub limit: u64,
    pub observed: u64,
    pub status: DataFlowStatus,
    pub stable_key: StableKeyId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DataFlowNodeKind {
    Place,
    Value,
    CallArgument,
    CallReceiver,
    CallReturn,
    SummaryInput,
    SummaryOutput,
    Source,
    Sink,
    Sanitizer,
    Barrier,
    Synthetic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DataFlowEdgeKind {
    LocalBinding,
    LocalAssignment,
    LocalUse,
    LocalRead,
    LocalWrite,
    ReturnValue,
    FieldProjection,
    IndexProjection,
    Dereference,
    AddressOf,
    CallArgumentToParameter,
    CallArgumentToReturn,
    CallReturnToUse,
    ReceiverToMethod,
    SummaryTito,
    SummaryProjected,
    UnknownFlow,
    HavocFlow,
    BudgetTruncated,
    SourceIntroduction,
    Sanitizer,
    Barrier,
    Model,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DataFlowAlgorithm {
    LocalMir,
    DirectCall,
    SummaryProjection,
    ExtensionModel,
    QuerySearch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DataFlowStatus {
    Present,
    Unknown,
    Unsupported,
    SetupMissing,
    BudgetExceeded,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DataFlowPrecision {
    Exact,
    SetupAware,
    Syntax,
    Conservative,
    Heuristic,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DataFlowValidation {
    Native,
    ReferentiallyValidated,
    ExtensionValidated,
    BudgetValidated,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DataFlowConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DataFlowProvenance {
    Native,
    Summary,
    Extension,
    Model,
    Query,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DataFlowModelKind {
    Source,
    Sink,
    Sanitizer,
    Barrier,
    Tito,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DataFlowBudgetReason {
    NodeLimit,
    EdgeLimit,
    PathDepth,
    PathCount,
}

impl DataFlowEdgeFact {
    pub fn normalized(mut self) -> Self {
        self.evidence.sort();
        self.evidence.dedup();
        self.input_stable_keys.sort();
        self.input_stable_keys.dedup();
        self
    }
}

impl DataFlowModelFact {
    pub fn normalized(mut self) -> Self {
        self.evidence.sort();
        self.evidence.dedup();
        self.payload_labels.sort();
        self.payload_labels.dedup();
        self
    }
}

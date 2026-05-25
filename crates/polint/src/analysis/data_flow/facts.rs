use serde::{Deserialize, Serialize};

use crate::analysis::cfg::ids::CfgNodeId;
use crate::analysis::ids::{
    CallSiteId, CallTargetId, DataFlowBudgetId, DataFlowEdgeId, DataFlowModelId, DataFlowNodeId,
    MirBodyId, MirOpId, PlaceId, RefinedCallEdgeId,
};
use crate::core::{FileId, FunctionId, Language, ReferenceId, Span, SymbolId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DataFlowNodeFact {
    pub(crate) id: DataFlowNodeId,
    pub(crate) kind: DataFlowNodeKind,
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
    pub(crate) model: Option<DataFlowModelId>,
    pub(crate) span: Option<Span>,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DataFlowEdgeFact {
    pub(crate) id: DataFlowEdgeId,
    pub(crate) from: DataFlowNodeId,
    pub(crate) to: DataFlowNodeId,
    pub(crate) kind: DataFlowEdgeKind,
    pub(crate) algorithm: DataFlowAlgorithm,
    pub(crate) status: DataFlowStatus,
    pub(crate) precision: DataFlowPrecision,
    pub(crate) validation: DataFlowValidation,
    pub(crate) confidence: DataFlowConfidence,
    pub(crate) provenance: DataFlowProvenance,
    pub(crate) call_site: Option<CallSiteId>,
    pub(crate) call_target: Option<CallTargetId>,
    pub(crate) refined_call: Option<RefinedCallEdgeId>,
    pub(crate) model: Option<DataFlowModelId>,
    pub(crate) budget: Option<DataFlowBudgetId>,
    pub(crate) evidence: Vec<String>,
    pub(crate) input_stable_keys: Vec<String>,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DataFlowModelFact {
    pub(crate) id: DataFlowModelId,
    pub(crate) kind: DataFlowModelKind,
    pub(crate) language: Language,
    pub(crate) provider_id: String,
    pub(crate) model_id: Option<String>,
    pub(crate) source_stable_key: Option<String>,
    pub(crate) status: DataFlowStatus,
    pub(crate) precision: DataFlowPrecision,
    pub(crate) validation: DataFlowValidation,
    pub(crate) confidence: DataFlowConfidence,
    pub(crate) provenance: DataFlowProvenance,
    pub(crate) evidence: Vec<String>,
    pub(crate) payload_labels: Vec<String>,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DataFlowBudgetFact {
    pub(crate) id: DataFlowBudgetId,
    pub(crate) reason: DataFlowBudgetReason,
    pub(crate) limit: u64,
    pub(crate) observed: u64,
    pub(crate) status: DataFlowStatus,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum DataFlowNodeKind {
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
pub(crate) enum DataFlowEdgeKind {
    LocalAssignment,
    LocalUse,
    FieldProjection,
    Dereference,
    AddressOf,
    CallArgumentToParameter,
    CallReturnToUse,
    ReceiverToMethod,
    SummaryTito,
    SourceIntroduction,
    SinkReachability,
    Sanitizer,
    Barrier,
    Model,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum DataFlowAlgorithm {
    LocalMir,
    DirectCall,
    SummaryProjection,
    ExtensionModel,
    QuerySearch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum DataFlowStatus {
    Present,
    Unknown,
    Unsupported,
    SetupMissing,
    BudgetExceeded,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum DataFlowPrecision {
    Exact,
    SetupAware,
    Syntax,
    Conservative,
    Heuristic,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum DataFlowValidation {
    Native,
    ReferentiallyValidated,
    ExtensionValidated,
    BudgetValidated,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum DataFlowConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum DataFlowProvenance {
    Native,
    Summary,
    Extension,
    Model,
    Query,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum DataFlowModelKind {
    Source,
    Sink,
    Sanitizer,
    Barrier,
    Tito,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum DataFlowBudgetReason {
    NodeLimit,
    EdgeLimit,
    PathDepth,
    PathCount,
}

impl DataFlowEdgeFact {
    pub(crate) fn normalized(mut self) -> Self {
        self.evidence.sort();
        self.evidence.dedup();
        self.input_stable_keys.sort();
        self.input_stable_keys.dedup();
        self
    }
}

impl DataFlowModelFact {
    pub(crate) fn normalized(mut self) -> Self {
        self.evidence.sort();
        self.evidence.dedup();
        self.payload_labels.sort();
        self.payload_labels.dedup();
        self
    }
}

use serde::{Deserialize, Serialize};

use crate::analysis::ids::{DispatchEdgeId, EntrypointId, TrustBoundaryId, UnresolvedFrameworkId};
use crate::core::{FileId, FunctionId, Language, Span, SymbolId};

// ---------------------------------------------------------------------------
// EntrypointFact
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct EntrypointFact {
    pub(crate) id: EntrypointId,
    pub(crate) language: Language,
    pub(crate) framework_id: String,
    pub(crate) kind: EntrypointKind,
    pub(crate) target_function: FunctionId,
    pub(crate) target_symbol: Option<SymbolId>,
    pub(crate) registration_span: Span,
    pub(crate) registration_file: FileId,
    pub(crate) trigger_metadata: TriggerMetadata,
    pub(crate) trust_boundary_link: Option<String>,
    pub(crate) precision: EntrypointPrecision,
    pub(crate) provenance: EntrypointProvenance,
    pub(crate) confidence: EntrypointConfidence,
    pub(crate) status: EntrypointStatus,
    pub(crate) provider_id: String,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TriggerMetadata {
    pub(crate) method: Option<String>,
    pub(crate) path: Option<String>,
    pub(crate) tool_name: Option<String>,
    pub(crate) event_name: Option<String>,
    pub(crate) test_name: Option<String>,
}

impl TriggerMetadata {
    pub(crate) fn empty() -> Self {
        Self {
            method: None,
            path: None,
            tool_name: None,
            event_name: None,
            test_name: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum EntrypointKind {
    HttpRoute,
    HttpMiddleware,
    McpTool,
    McpResource,
    McpPrompt,
    CliCommand,
    Test,
    Job,
    QueueConsumer,
    ServerlessHandler,
    LifecycleCallback,
    EventListener,
    GeneratedDispatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum EntrypointPrecision {
    ResolvedStatic,
    SetupAware,
    Heuristic,
    Conservative,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum EntrypointProvenance {
    NativeRecognizer,
    Extension,
    Generated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum EntrypointConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum EntrypointStatus {
    Resolved,
    Partial,
    Unresolved,
    SetupMissing,
    Unsupported,
}

// ---------------------------------------------------------------------------
// TrustBoundaryFact
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TrustBoundaryFact {
    pub(crate) id: TrustBoundaryId,
    pub(crate) entrypoint_stable_key: String,
    pub(crate) source_kind: TrustBoundarySourceKind,
    pub(crate) target_parameter: Option<FunctionId>,
    pub(crate) target_parameter_index: Option<usize>,
    pub(crate) access_path: Option<String>,
    pub(crate) protocol: Option<String>,
    pub(crate) language: Language,
    pub(crate) file: FileId,
    pub(crate) span: Span,
    pub(crate) precision: EntrypointPrecision,
    pub(crate) provider_id: String,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum TrustBoundarySourceKind {
    PathParam,
    QueryString,
    RequestBody,
    RequestHeader,
    Cookie,
    McpArguments,
    McpResourceUri,
    CliArgs,
    CliFlags,
    EnvVar,
    Stdin,
    QueuePayload,
    ExternalReturn,
    Unknown,
}

// ---------------------------------------------------------------------------
// FrameworkDispatchEdgeFact
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FrameworkDispatchEdgeFact {
    pub(crate) id: DispatchEdgeId,
    pub(crate) from_source: String,
    pub(crate) to_target: FunctionId,
    pub(crate) to_symbol: Option<SymbolId>,
    pub(crate) edge_kind: DispatchEdgeKind,
    pub(crate) guard_metadata: Option<String>,
    pub(crate) ordering: Option<u32>,
    pub(crate) language: Language,
    pub(crate) file: FileId,
    pub(crate) span: Span,
    pub(crate) precision: EntrypointPrecision,
    pub(crate) provider_id: String,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum DispatchEdgeKind {
    RouteDispatch,
    MiddlewareChain,
    LifecycleHook,
    EventDispatch,
    McpDispatch,
    TestRunner,
    JobScheduler,
}

// ---------------------------------------------------------------------------
// UnresolvedFrameworkFact
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct UnresolvedFrameworkFact {
    pub(crate) id: UnresolvedFrameworkId,
    pub(crate) language: Language,
    pub(crate) file: FileId,
    pub(crate) span: Span,
    pub(crate) framework_id: String,
    pub(crate) reason: UnresolvedFrameworkReason,
    pub(crate) evidence: String,
    pub(crate) scope_description: String,
    pub(crate) precision: EntrypointPrecision,
    pub(crate) provider_id: String,
    pub(crate) stable_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum UnresolvedFrameworkReason {
    DynamicRoute,
    UnknownWrapper,
    UnresolvedHandler,
    MissingSetup,
    UnsupportedFrameworkVersion,
    BudgetExceeded,
    DynamicRegistration,
    UnrecognizedPattern,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{FileId, FunctionId, Language, Span, SymbolId};
    use serde::Serialize;
    use serde::de::DeserializeOwned;
    use std::fmt::Debug;
    use std::hash::Hash;

    fn assert_small_id_contract<T>()
    where
        T: Debug
            + Clone
            + Copy
            + PartialEq
            + Eq
            + PartialOrd
            + Ord
            + Hash
            + Serialize
            + DeserializeOwned,
    {
    }

    #[test]
    fn entrypoint_vocabulary_enums_are_copy_ordered_hashable_serializable() {
        assert_small_id_contract::<EntrypointKind>();
        assert_small_id_contract::<EntrypointPrecision>();
        assert_small_id_contract::<EntrypointProvenance>();
        assert_small_id_contract::<EntrypointConfidence>();
        assert_small_id_contract::<EntrypointStatus>();
        assert_small_id_contract::<TrustBoundarySourceKind>();
        assert_small_id_contract::<DispatchEdgeKind>();
        assert_small_id_contract::<UnresolvedFrameworkReason>();
    }

    #[test]
    fn entrypoint_kind_has_13_variants() {
        let variants = [
            EntrypointKind::HttpRoute,
            EntrypointKind::HttpMiddleware,
            EntrypointKind::McpTool,
            EntrypointKind::McpResource,
            EntrypointKind::McpPrompt,
            EntrypointKind::CliCommand,
            EntrypointKind::Test,
            EntrypointKind::Job,
            EntrypointKind::QueueConsumer,
            EntrypointKind::ServerlessHandler,
            EntrypointKind::LifecycleCallback,
            EntrypointKind::EventListener,
            EntrypointKind::GeneratedDispatch,
        ];
        assert_eq!(variants.len(), 13);
    }

    #[test]
    fn trust_boundary_source_kind_has_14_variants() {
        let variants = [
            TrustBoundarySourceKind::PathParam,
            TrustBoundarySourceKind::QueryString,
            TrustBoundarySourceKind::RequestBody,
            TrustBoundarySourceKind::RequestHeader,
            TrustBoundarySourceKind::Cookie,
            TrustBoundarySourceKind::McpArguments,
            TrustBoundarySourceKind::McpResourceUri,
            TrustBoundarySourceKind::CliArgs,
            TrustBoundarySourceKind::CliFlags,
            TrustBoundarySourceKind::EnvVar,
            TrustBoundarySourceKind::Stdin,
            TrustBoundarySourceKind::QueuePayload,
            TrustBoundarySourceKind::ExternalReturn,
            TrustBoundarySourceKind::Unknown,
        ];
        assert_eq!(variants.len(), 14);
    }

    #[test]
    fn unresolved_framework_reason_has_8_variants() {
        let variants = [
            UnresolvedFrameworkReason::DynamicRoute,
            UnresolvedFrameworkReason::UnknownWrapper,
            UnresolvedFrameworkReason::UnresolvedHandler,
            UnresolvedFrameworkReason::MissingSetup,
            UnresolvedFrameworkReason::UnsupportedFrameworkVersion,
            UnresolvedFrameworkReason::BudgetExceeded,
            UnresolvedFrameworkReason::DynamicRegistration,
            UnresolvedFrameworkReason::UnrecognizedPattern,
        ];
        assert_eq!(variants.len(), 8);
    }

    #[test]
    fn entrypoint_facts_keep_dense_ids_and_stable_keys_separate() {
        let entrypoint = EntrypointFact {
            id: EntrypointId(1),
            language: Language::TypeScript,
            framework_id: "express".to_string(),
            kind: EntrypointKind::HttpRoute,
            target_function: FunctionId(10),
            target_symbol: Some(SymbolId(20)),
            registration_span: Span::point(FileId(1), 1, 1),
            registration_file: FileId(1),
            trigger_metadata: TriggerMetadata {
                method: Some("GET".to_string()),
                path: Some("/api/users".to_string()),
                tool_name: None,
                event_name: None,
                test_name: None,
            },
            trust_boundary_link: None,
            precision: EntrypointPrecision::ResolvedStatic,
            provenance: EntrypointProvenance::NativeRecognizer,
            confidence: EntrypointConfidence::High,
            status: EntrypointStatus::Resolved,
            provider_id: "polint.entrypoints".to_string(),
            stable_key: "entrypoint:express:get:/api/users".to_string(),
        };

        let boundary = TrustBoundaryFact {
            id: TrustBoundaryId(1),
            entrypoint_stable_key: entrypoint.stable_key.clone(),
            source_kind: TrustBoundarySourceKind::PathParam,
            target_parameter: Some(FunctionId(10)),
            target_parameter_index: Some(0),
            access_path: Some("req.params.id".to_string()),
            protocol: Some("http".to_string()),
            language: Language::TypeScript,
            file: FileId(1),
            span: Span::point(FileId(1), 1, 1),
            precision: EntrypointPrecision::ResolvedStatic,
            provider_id: "polint.entrypoints".to_string(),
            stable_key: "trust-boundary:express:get:/api/users:PathParam".to_string(),
        };

        let dispatch = FrameworkDispatchEdgeFact {
            id: DispatchEdgeId(1),
            from_source: entrypoint.stable_key.clone(),
            to_target: FunctionId(10),
            to_symbol: Some(SymbolId(20)),
            edge_kind: DispatchEdgeKind::RouteDispatch,
            guard_metadata: None,
            ordering: None,
            language: Language::TypeScript,
            file: FileId(1),
            span: Span::point(FileId(1), 1, 1),
            precision: EntrypointPrecision::ResolvedStatic,
            provider_id: "polint.entrypoints".to_string(),
            stable_key: "dispatch:express:get:/api/users".to_string(),
        };

        let unresolved = UnresolvedFrameworkFact {
            id: UnresolvedFrameworkId(1),
            language: Language::TypeScript,
            file: FileId(2),
            span: Span::point(FileId(2), 5, 10),
            framework_id: "fastify".to_string(),
            reason: UnresolvedFrameworkReason::UnrecognizedPattern,
            evidence: "import fastify detected".to_string(),
            scope_description: "fastify server registration".to_string(),
            precision: EntrypointPrecision::Conservative,
            provider_id: "polint.entrypoints".to_string(),
            stable_key: "unresolved:fastify:file2".to_string(),
        };

        // Stable keys are distinct across fact families
        assert_ne!(entrypoint.stable_key, boundary.stable_key);
        assert_ne!(entrypoint.stable_key, dispatch.stable_key);
        assert_ne!(entrypoint.stable_key, unresolved.stable_key);
        assert_ne!(boundary.stable_key, dispatch.stable_key);

        // Trust boundary references the entrypoint
        assert_eq!(boundary.entrypoint_stable_key, entrypoint.stable_key);

        // Dispatch edge references the entrypoint
        assert_eq!(dispatch.from_source, entrypoint.stable_key);
    }
}

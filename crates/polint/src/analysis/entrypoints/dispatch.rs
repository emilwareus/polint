use crate::analysis::entrypoints::facts::{
    DispatchEdgeKind, EntrypointFact, EntrypointKind, FrameworkDispatchEdgeFact,
};
use crate::analysis::entrypoints::provider::ENTRYPOINTS_PROVIDER_ID;
use crate::analysis::ids::DispatchEdgeId;
use crate::analysis::stable_key::semantic_stable_key;
use crate::analysis_kernel::FactFamily;
use crate::core::AnalysisDb;

/// Derive framework dispatch edge facts from recognized entrypoints.
///
/// Per D-04: dispatch edges represent synthetic call edges from framework/protocol
/// dispatch roots to handler functions. For each entrypoint with a resolved
/// target_function, emit a FrameworkDispatchEdgeFact connecting a synthetic
/// framework dispatch root to the handler.
pub(crate) fn derive_dispatch_edges(
    _db: &AnalysisDb,
    entrypoints: &[EntrypointFact],
) -> Vec<FrameworkDispatchEdgeFact> {
    let mut edges = Vec::new();

    for entrypoint in entrypoints {
        let edge_kind = edge_kind_for_entrypoint(entrypoint.kind);

        let stable_key = dispatch_edge_stable_key(
            &entrypoint.stable_key,
            &format!("{}", entrypoint.target_function.0),
            edge_kind,
            entrypoint.language,
        );

        edges.push(FrameworkDispatchEdgeFact {
            id: DispatchEdgeId(0), // Reassigned during normalization
            from_source: entrypoint.stable_key.clone(),
            to_target: entrypoint.target_function,
            to_symbol: entrypoint.target_symbol,
            edge_kind,
            guard_metadata: None,
            ordering: None,
            language: entrypoint.language,
            file: entrypoint.registration_file,
            span: entrypoint.registration_span.clone(),
            precision: entrypoint.precision,
            provider_id: ENTRYPOINTS_PROVIDER_ID.to_string(),
            stable_key,
        });
    }

    edges.sort_by(|a, b| a.stable_key.cmp(&b.stable_key));
    edges
}

/// Map EntrypointKind to DispatchEdgeKind.
///
/// Per D-04:
/// - HttpRoute -> RouteDispatch
/// - HttpMiddleware -> MiddlewareChain
/// - McpTool/McpResource/McpPrompt -> McpDispatch
/// - Test -> TestRunner
/// - CliCommand -> RouteDispatch
/// - Job/QueueConsumer -> JobScheduler
/// - LifecycleCallback -> LifecycleHook
/// - EventListener -> EventDispatch
/// - ServerlessHandler/GeneratedDispatch -> RouteDispatch
fn edge_kind_for_entrypoint(kind: EntrypointKind) -> DispatchEdgeKind {
    match kind {
        EntrypointKind::HttpRoute => DispatchEdgeKind::RouteDispatch,
        EntrypointKind::HttpMiddleware => DispatchEdgeKind::MiddlewareChain,
        EntrypointKind::McpTool | EntrypointKind::McpResource | EntrypointKind::McpPrompt => {
            DispatchEdgeKind::McpDispatch
        }
        EntrypointKind::Test => DispatchEdgeKind::TestRunner,
        EntrypointKind::CliCommand => DispatchEdgeKind::RouteDispatch,
        EntrypointKind::Job | EntrypointKind::QueueConsumer => DispatchEdgeKind::JobScheduler,
        EntrypointKind::LifecycleCallback => DispatchEdgeKind::LifecycleHook,
        EntrypointKind::EventListener => DispatchEdgeKind::EventDispatch,
        EntrypointKind::ServerlessHandler | EntrypointKind::GeneratedDispatch => {
            DispatchEdgeKind::RouteDispatch
        }
    }
}

/// Generate a stable key for a dispatch edge fact.
fn dispatch_edge_stable_key(
    from_source: &str,
    to_function_key: &str,
    edge_kind: DispatchEdgeKind,
    language: crate::core::Language,
) -> String {
    semantic_stable_key(
        FactFamily::DispatchEdge,
        &[
            ("from", from_source.to_string()),
            ("to_function_key", to_function_key.to_string()),
            ("edge_kind", format!("{edge_kind:?}")),
            ("language", format!("{language:?}")),
        ],
    )
    .into_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::entrypoints::facts::{
        EntrypointConfidence, EntrypointPrecision, EntrypointProvenance, EntrypointStatus,
        TriggerMetadata,
    };
    use crate::analysis::ids::EntrypointId;
    use crate::core::{AnalysisDb, FileId, FunctionId, Language, Span};

    fn make_entrypoint(kind: EntrypointKind, framework_id: &str) -> EntrypointFact {
        EntrypointFact {
            id: EntrypointId(0),
            language: Language::Go,
            framework_id: framework_id.to_string(),
            kind,
            target_function: FunctionId(42),
            target_symbol: None,
            registration_span: Span::point(FileId(1), 1, 1),
            registration_file: FileId(1),
            trigger_metadata: TriggerMetadata::empty(),
            trust_boundary_link: None,
            precision: EntrypointPrecision::Heuristic,
            provenance: EntrypointProvenance::NativeRecognizer,
            confidence: EntrypointConfidence::High,
            status: EntrypointStatus::Resolved,
            provider_id: ENTRYPOINTS_PROVIDER_ID.to_string(),
            stable_key: format!("ep-{kind:?}"),
        }
    }

    #[test]
    fn http_route_produces_route_dispatch_edge() {
        let db = AnalysisDb::new();
        let ep = make_entrypoint(EntrypointKind::HttpRoute, "go.net_http");

        let edges = derive_dispatch_edges(&db, &[ep]);

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].edge_kind, DispatchEdgeKind::RouteDispatch);
        assert_eq!(edges[0].to_target, FunctionId(42));
        assert_eq!(edges[0].from_source, "ep-HttpRoute");
    }

    #[test]
    fn http_middleware_produces_middleware_chain_edge() {
        let db = AnalysisDb::new();
        let ep = make_entrypoint(EntrypointKind::HttpMiddleware, "go.chi");

        let edges = derive_dispatch_edges(&db, &[ep]);

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].edge_kind, DispatchEdgeKind::MiddlewareChain);
    }

    #[test]
    fn mcp_tool_produces_mcp_dispatch_edge() {
        let db = AnalysisDb::new();
        let ep = make_entrypoint(EntrypointKind::McpTool, "ts.mcp_sdk");

        let edges = derive_dispatch_edges(&db, &[ep]);

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].edge_kind, DispatchEdgeKind::McpDispatch);
    }

    #[test]
    fn test_entrypoint_produces_test_runner_edge() {
        let db = AnalysisDb::new();
        let ep = make_entrypoint(EntrypointKind::Test, "go.testing");

        let edges = derive_dispatch_edges(&db, &[ep]);

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].edge_kind, DispatchEdgeKind::TestRunner);
    }

    #[test]
    fn cli_command_produces_route_dispatch_edge() {
        let db = AnalysisDb::new();
        let ep = make_entrypoint(EntrypointKind::CliCommand, "go.cobra");

        let edges = derive_dispatch_edges(&db, &[ep]);

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].edge_kind, DispatchEdgeKind::RouteDispatch);
    }

    #[test]
    fn job_produces_job_scheduler_edge() {
        let db = AnalysisDb::new();
        let ep = make_entrypoint(EntrypointKind::Job, "test.jobs");

        let edges = derive_dispatch_edges(&db, &[ep]);

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].edge_kind, DispatchEdgeKind::JobScheduler);
    }

    #[test]
    fn dispatch_edge_precision_follows_entrypoint_precision() {
        let db = AnalysisDb::new();
        let mut ep = make_entrypoint(EntrypointKind::HttpRoute, "go.net_http");
        ep.precision = EntrypointPrecision::Conservative;

        let edges = derive_dispatch_edges(&db, &[ep]);

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].precision, EntrypointPrecision::Conservative);
    }

    #[test]
    fn dispatch_edge_stable_keys_include_fact_family() {
        let db = AnalysisDb::new();
        let ep = make_entrypoint(EntrypointKind::HttpRoute, "go.net_http");

        let edges = derive_dispatch_edges(&db, &[ep]);

        assert!(!edges.is_empty());
        let key = &edges[0].stable_key;
        assert!(
            key.contains("12:DispatchEdge"),
            "stable key should contain FactFamily::DispatchEdge; got: {key}"
        );
    }

    #[test]
    fn multiple_entrypoints_produce_multiple_edges() {
        let db = AnalysisDb::new();
        let ep1 = make_entrypoint(EntrypointKind::HttpRoute, "go.net_http");
        let ep2 = make_entrypoint(EntrypointKind::McpTool, "ts.mcp_sdk");

        let edges = derive_dispatch_edges(&db, &[ep1, ep2]);

        assert_eq!(edges.len(), 2);
    }

    #[test]
    fn output_is_sorted_by_stable_key() {
        let db = AnalysisDb::new();
        let mut ep1 = make_entrypoint(EntrypointKind::HttpRoute, "z.framework");
        ep1.target_function = FunctionId(1);
        let mut ep2 = make_entrypoint(EntrypointKind::HttpRoute, "a.framework");
        ep2.target_function = FunctionId(2);

        let edges = derive_dispatch_edges(&db, &[ep1, ep2]);

        let keys: Vec<&str> = edges.iter().map(|e| e.stable_key.as_str()).collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(
            keys, sorted,
            "dispatch edges should be sorted by stable key"
        );
    }

    #[test]
    fn dispatch_edge_from_source_references_entrypoint_stable_key() {
        let db = AnalysisDb::new();
        let ep = make_entrypoint(EntrypointKind::Test, "go.testing");
        let entrypoint_key = ep.stable_key.clone();

        let edges = derive_dispatch_edges(&db, &[ep]);

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].from_source, entrypoint_key);
    }
}

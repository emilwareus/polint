use crate::analysis::entrypoints::facts::{
    EntrypointFact, EntrypointKind, TrustBoundaryFact, TrustBoundarySourceKind,
};
use crate::analysis::entrypoints::provider::ENTRYPOINTS_PROVIDER_ID;
use crate::analysis::ids::TrustBoundaryId;
use crate::analysis::stable_key::semantic_stable_key;
use crate::analysis_kernel::FactFamily;
use crate::core::AnalysisDb;

/// Derive trust boundary facts from recognized entrypoints.
///
/// Per D-03, D-19, D-20, D-21, D-22: trust boundaries are per-entrypoint,
/// per-source-kind facts. Each identifies a concrete source of untrusted data
/// entering through an entrypoint. Trust boundary precision follows entrypoint
/// precision per D-21.
pub(crate) fn derive_trust_boundaries(
    _db: &AnalysisDb,
    entrypoints: &[EntrypointFact],
) -> Vec<TrustBoundaryFact> {
    let mut boundaries = Vec::new();

    for entrypoint in entrypoints {
        let source_kinds = source_kinds_for_entrypoint(entrypoint);

        for source_kind in source_kinds {
            let stable_key =
                trust_boundary_stable_key(&entrypoint.stable_key, source_kind, entrypoint.language);

            boundaries.push(TrustBoundaryFact {
                id: TrustBoundaryId(0), // Reassigned during normalization
                entrypoint_stable_key: entrypoint.stable_key.clone(),
                source_kind,
                target_parameter: Some(entrypoint.target_function),
                target_parameter_index: None,
                access_path: None,
                protocol: protocol_for_kind(entrypoint.kind),
                language: entrypoint.language,
                file: entrypoint.registration_file,
                span: entrypoint.registration_span.clone(),
                precision: entrypoint.precision, // Per D-21
                provider_id: ENTRYPOINTS_PROVIDER_ID.to_string(),
                stable_key,
            });
        }
    }

    boundaries.sort_by(|a, b| a.stable_key.cmp(&b.stable_key));
    boundaries
}

/// Determine which trust boundary source kinds apply to a given entrypoint kind.
///
/// Per D-19 and D-20:
/// - HTTP routes: PathParam (if path has params), QueryString, RequestBody (for
///   mutation methods), RequestHeader
/// - HTTP middleware: RequestBody, RequestHeader, QueryString
/// - MCP tools/prompts: McpArguments
/// - MCP resources: McpResourceUri
/// - CLI commands: CliArgs, CliFlags
/// - Tests: no trust boundaries (not external-facing)
/// - Job/QueueConsumer: QueuePayload
/// - Others: Unknown (per D-20, mandatory for unknown source kinds)
fn source_kinds_for_entrypoint(entrypoint: &EntrypointFact) -> Vec<TrustBoundarySourceKind> {
    match entrypoint.kind {
        EntrypointKind::HttpRoute => {
            let mut kinds = Vec::new();

            // PathParam if trigger_metadata.path contains parameter placeholders
            if let Some(path) = &entrypoint.trigger_metadata.path
                && path_has_parameters(path)
            {
                kinds.push(TrustBoundarySourceKind::PathParam);
            }

            // HTTP routes always accept query strings
            kinds.push(TrustBoundarySourceKind::QueryString);

            // RequestBody for POST, PUT, PATCH, DELETE methods
            if method_accepts_body(entrypoint.trigger_metadata.method.as_deref()) {
                kinds.push(TrustBoundarySourceKind::RequestBody);
            }

            // All HTTP routes receive headers
            kinds.push(TrustBoundarySourceKind::RequestHeader);

            kinds
        }

        EntrypointKind::HttpMiddleware => {
            vec![
                TrustBoundarySourceKind::RequestBody,
                TrustBoundarySourceKind::RequestHeader,
                TrustBoundarySourceKind::QueryString,
            ]
        }

        EntrypointKind::McpTool => {
            vec![TrustBoundarySourceKind::McpArguments]
        }

        EntrypointKind::McpResource => {
            vec![TrustBoundarySourceKind::McpResourceUri]
        }

        EntrypointKind::McpPrompt => {
            vec![TrustBoundarySourceKind::McpArguments]
        }

        EntrypointKind::CliCommand => {
            vec![
                TrustBoundarySourceKind::CliArgs,
                TrustBoundarySourceKind::CliFlags,
            ]
        }

        EntrypointKind::Test => {
            // Tests are not external-facing; no trust boundary facts
            Vec::new()
        }

        EntrypointKind::Job | EntrypointKind::QueueConsumer => {
            vec![TrustBoundarySourceKind::QueuePayload]
        }

        EntrypointKind::ServerlessHandler
        | EntrypointKind::LifecycleCallback
        | EntrypointKind::EventListener
        | EntrypointKind::GeneratedDispatch => {
            // Per D-20: Unknown is mandatory for entrypoints where source kind
            // cannot be determined
            vec![TrustBoundarySourceKind::Unknown]
        }
    }
}

/// Check if an HTTP path contains parameter placeholders.
/// Recognizes both /:id (Express/chi) and /{id} (OpenAPI) styles.
fn path_has_parameters(path: &str) -> bool {
    // Express/chi style: /:param
    if path.contains("/:") {
        return true;
    }
    // OpenAPI/Go style: /{param}
    if path.contains("/{") && path.contains('}') {
        return true;
    }
    false
}

/// Check if an HTTP method accepts a request body.
/// POST, PUT, PATCH, DELETE accept bodies. GET, HEAD, OPTIONS, CONNECT, TRACE
/// typically do not. When method is None (all methods), we assume body is possible.
fn method_accepts_body(method: Option<&str>) -> bool {
    match method {
        None => true, // All methods registered; body is possible
        Some(m) => matches!(
            m.to_uppercase().as_str(),
            "POST" | "PUT" | "PATCH" | "DELETE"
        ),
    }
}

/// Determine protocol for a given entrypoint kind.
fn protocol_for_kind(kind: EntrypointKind) -> Option<String> {
    match kind {
        EntrypointKind::HttpRoute | EntrypointKind::HttpMiddleware => Some("http".to_string()),
        EntrypointKind::McpTool | EntrypointKind::McpResource | EntrypointKind::McpPrompt => {
            Some("mcp".to_string())
        }
        _ => None,
    }
}

/// Generate a stable key for a trust boundary fact.
/// Per D-21: uses the entrypoint's provider_id. Generate stable keys using
/// semantic_stable_key(FactFamily::TrustBoundary, ...).
fn trust_boundary_stable_key(
    entrypoint_key: &str,
    source_kind: TrustBoundarySourceKind,
    language: crate::core::Language,
) -> String {
    semantic_stable_key(
        FactFamily::TrustBoundary,
        &[
            ("entrypoint_key", entrypoint_key.to_string()),
            ("source_kind", format!("{source_kind:?}")),
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

    fn make_entrypoint(
        kind: EntrypointKind,
        precision: EntrypointPrecision,
        method: Option<&str>,
        path: Option<&str>,
    ) -> EntrypointFact {
        EntrypointFact {
            id: EntrypointId(0),
            language: Language::TypeScript,
            framework_id: "test.framework".to_string(),
            kind,
            target_function: FunctionId(1),
            target_symbol: None,
            registration_span: Span::point(FileId(1), 1, 1),
            registration_file: FileId(1),
            trigger_metadata: TriggerMetadata {
                method: method.map(String::from),
                path: path.map(String::from),
                tool_name: None,
                event_name: None,
                test_name: None,
            },
            trust_boundary_link: None,
            precision,
            provenance: EntrypointProvenance::NativeRecognizer,
            confidence: EntrypointConfidence::High,
            status: EntrypointStatus::Resolved,
            provider_id: ENTRYPOINTS_PROVIDER_ID.to_string(),
            stable_key: format!("ep-{kind:?}-{method:?}"),
        }
    }

    #[test]
    fn http_route_get_produces_query_and_header_boundaries() {
        let db = AnalysisDb::new();
        let ep = make_entrypoint(
            EntrypointKind::HttpRoute,
            EntrypointPrecision::Heuristic,
            Some("GET"),
            Some("/api/users"),
        );

        let boundaries = derive_trust_boundaries(&db, &[ep]);

        let source_kinds: Vec<_> = boundaries.iter().map(|tb| tb.source_kind).collect();
        // GET on /api/users (no path params) -> QueryString, RequestHeader
        assert!(source_kinds.contains(&TrustBoundarySourceKind::QueryString));
        assert!(source_kinds.contains(&TrustBoundarySourceKind::RequestHeader));
        assert!(!source_kinds.contains(&TrustBoundarySourceKind::PathParam));
        assert!(!source_kinds.contains(&TrustBoundarySourceKind::RequestBody));
    }

    #[test]
    fn http_route_post_with_params_produces_all_four_boundary_kinds() {
        let db = AnalysisDb::new();
        let ep = make_entrypoint(
            EntrypointKind::HttpRoute,
            EntrypointPrecision::Heuristic,
            Some("POST"),
            Some("/api/users/:id"),
        );

        let boundaries = derive_trust_boundaries(&db, &[ep]);

        let source_kinds: Vec<_> = boundaries.iter().map(|tb| tb.source_kind).collect();
        assert!(
            source_kinds.contains(&TrustBoundarySourceKind::PathParam),
            "POST with /:id should have PathParam"
        );
        assert!(source_kinds.contains(&TrustBoundarySourceKind::QueryString));
        assert!(source_kinds.contains(&TrustBoundarySourceKind::RequestBody));
        assert!(source_kinds.contains(&TrustBoundarySourceKind::RequestHeader));
        assert_eq!(source_kinds.len(), 4);
    }

    #[test]
    fn http_route_no_method_includes_request_body() {
        let db = AnalysisDb::new();
        let ep = make_entrypoint(
            EntrypointKind::HttpRoute,
            EntrypointPrecision::Heuristic,
            None, // All methods
            Some("/api/users"),
        );

        let boundaries = derive_trust_boundaries(&db, &[ep]);

        let source_kinds: Vec<_> = boundaries.iter().map(|tb| tb.source_kind).collect();
        // No method means all methods -> body is possible
        assert!(source_kinds.contains(&TrustBoundarySourceKind::RequestBody));
    }

    #[test]
    fn http_middleware_produces_body_header_query_boundaries() {
        let db = AnalysisDb::new();
        let ep = make_entrypoint(
            EntrypointKind::HttpMiddleware,
            EntrypointPrecision::Heuristic,
            None,
            None,
        );

        let boundaries = derive_trust_boundaries(&db, &[ep]);

        let source_kinds: Vec<_> = boundaries.iter().map(|tb| tb.source_kind).collect();
        assert_eq!(source_kinds.len(), 3);
        assert!(source_kinds.contains(&TrustBoundarySourceKind::RequestBody));
        assert!(source_kinds.contains(&TrustBoundarySourceKind::RequestHeader));
        assert!(source_kinds.contains(&TrustBoundarySourceKind::QueryString));
    }

    #[test]
    fn mcp_tool_produces_mcp_arguments_boundary() {
        let db = AnalysisDb::new();
        let ep = make_entrypoint(
            EntrypointKind::McpTool,
            EntrypointPrecision::Heuristic,
            None,
            None,
        );

        let boundaries = derive_trust_boundaries(&db, &[ep]);

        assert_eq!(boundaries.len(), 1);
        assert_eq!(
            boundaries[0].source_kind,
            TrustBoundarySourceKind::McpArguments
        );
    }

    #[test]
    fn mcp_resource_produces_mcp_resource_uri_boundary() {
        let db = AnalysisDb::new();
        let ep = make_entrypoint(
            EntrypointKind::McpResource,
            EntrypointPrecision::Heuristic,
            None,
            None,
        );

        let boundaries = derive_trust_boundaries(&db, &[ep]);

        assert_eq!(boundaries.len(), 1);
        assert_eq!(
            boundaries[0].source_kind,
            TrustBoundarySourceKind::McpResourceUri
        );
    }

    #[test]
    fn mcp_prompt_produces_mcp_arguments_boundary() {
        let db = AnalysisDb::new();
        let ep = make_entrypoint(
            EntrypointKind::McpPrompt,
            EntrypointPrecision::Heuristic,
            None,
            None,
        );

        let boundaries = derive_trust_boundaries(&db, &[ep]);

        assert_eq!(boundaries.len(), 1);
        assert_eq!(
            boundaries[0].source_kind,
            TrustBoundarySourceKind::McpArguments
        );
    }

    #[test]
    fn cli_command_produces_cli_args_and_flags_boundaries() {
        let db = AnalysisDb::new();
        let ep = make_entrypoint(
            EntrypointKind::CliCommand,
            EntrypointPrecision::Conservative,
            None,
            None,
        );

        let boundaries = derive_trust_boundaries(&db, &[ep]);

        let source_kinds: Vec<_> = boundaries.iter().map(|tb| tb.source_kind).collect();
        assert_eq!(source_kinds.len(), 2);
        assert!(source_kinds.contains(&TrustBoundarySourceKind::CliArgs));
        assert!(source_kinds.contains(&TrustBoundarySourceKind::CliFlags));
    }

    #[test]
    fn test_entrypoint_produces_no_boundaries() {
        let db = AnalysisDb::new();
        let ep = make_entrypoint(
            EntrypointKind::Test,
            EntrypointPrecision::ResolvedStatic,
            None,
            None,
        );

        let boundaries = derive_trust_boundaries(&db, &[ep]);

        assert!(
            boundaries.is_empty(),
            "test entrypoints should produce no trust boundaries"
        );
    }

    #[test]
    fn job_entrypoint_produces_queue_payload_boundary() {
        let db = AnalysisDb::new();
        let ep = make_entrypoint(
            EntrypointKind::Job,
            EntrypointPrecision::Heuristic,
            None,
            None,
        );

        let boundaries = derive_trust_boundaries(&db, &[ep]);

        assert_eq!(boundaries.len(), 1);
        assert_eq!(
            boundaries[0].source_kind,
            TrustBoundarySourceKind::QueuePayload
        );
    }

    #[test]
    fn serverless_handler_produces_unknown_boundary() {
        let db = AnalysisDb::new();
        let ep = make_entrypoint(
            EntrypointKind::ServerlessHandler,
            EntrypointPrecision::Conservative,
            None,
            None,
        );

        let boundaries = derive_trust_boundaries(&db, &[ep]);

        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].source_kind, TrustBoundarySourceKind::Unknown);
    }

    #[test]
    fn trust_boundary_precision_follows_entrypoint_precision() {
        let db = AnalysisDb::new();
        let ep = make_entrypoint(
            EntrypointKind::McpTool,
            EntrypointPrecision::Conservative,
            None,
            None,
        );

        let boundaries = derive_trust_boundaries(&db, &[ep]);

        assert_eq!(boundaries.len(), 1);
        assert_eq!(
            boundaries[0].precision,
            EntrypointPrecision::Conservative,
            "trust boundary precision should match entrypoint precision per D-21"
        );
    }

    #[test]
    fn trust_boundary_stable_keys_include_fact_family() {
        let db = AnalysisDb::new();
        let ep = make_entrypoint(
            EntrypointKind::McpTool,
            EntrypointPrecision::Heuristic,
            None,
            None,
        );

        let boundaries = derive_trust_boundaries(&db, &[ep]);

        assert!(!boundaries.is_empty());
        let key = &boundaries[0].stable_key;
        assert!(
            key.contains("13:TrustBoundary"),
            "stable key should contain FactFamily::TrustBoundary; got: {key}"
        );
    }

    #[test]
    fn path_param_detection_with_express_style() {
        assert!(path_has_parameters("/api/users/:id"));
        assert!(path_has_parameters("/api/:org/repos/:id"));
    }

    #[test]
    fn path_param_detection_with_openapi_style() {
        assert!(path_has_parameters("/api/users/{id}"));
        assert!(path_has_parameters("/api/{org}/repos/{id}"));
    }

    #[test]
    fn path_param_detection_plain_path() {
        assert!(!path_has_parameters("/api/users"));
        assert!(!path_has_parameters("/"));
        assert!(!path_has_parameters("/api/users/list"));
    }

    #[test]
    fn http_protocol_set_for_http_boundaries() {
        let db = AnalysisDb::new();
        let ep = make_entrypoint(
            EntrypointKind::HttpRoute,
            EntrypointPrecision::Heuristic,
            Some("GET"),
            Some("/api/users"),
        );

        let boundaries = derive_trust_boundaries(&db, &[ep]);

        for tb in &boundaries {
            assert_eq!(tb.protocol.as_deref(), Some("http"));
        }
    }

    #[test]
    fn mcp_protocol_set_for_mcp_boundaries() {
        let db = AnalysisDb::new();
        let ep = make_entrypoint(
            EntrypointKind::McpTool,
            EntrypointPrecision::Heuristic,
            None,
            None,
        );

        let boundaries = derive_trust_boundaries(&db, &[ep]);

        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].protocol.as_deref(), Some("mcp"));
    }
}

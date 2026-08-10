#![cfg(test)]

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::analysis::entrypoints::facts::{
    DispatchEdgeKind, EntrypointKind, EntrypointPrecision, EntrypointProvenance, EntrypointStatus,
    TrustBoundarySourceKind, UnresolvedFrameworkReason,
};
use crate::core::{AnalysisDb, FileId, Language};

/// Produces a debug JSON snapshot of all framework entrypoint facts in the db.
///
/// The output includes:
/// - Summary counts (total_entrypoints, total_trust_boundaries, total_dispatch_edges, total_unresolved)
/// - Breakdown counts by language, framework_id, kind, status, precision
/// - Trust boundary breakdown by source_kind
/// - Dispatch edge breakdown by edge_kind
/// - Unresolved framework breakdown by reason
/// - Per-entrypoint detail rows (sorted by stable_key)
/// - Per-trust-boundary detail rows (sorted by stable_key)
///
/// Avoids: raw source bodies, absolute paths, parser IDs, timestamps, run-local dense IDs as identity.
pub(crate) fn metadata_debug_json_for_test(db: &AnalysisDb) -> Value {
    let report = EntrypointDebugReport {
        counts: entrypoint_counts(db),
        entrypoints: entrypoint_detail_rows(db),
        trust_boundaries: trust_boundary_detail_rows(db),
        dispatch_edges: dispatch_edge_detail_rows(db),
        unresolved: unresolved_detail_rows(db),
    };
    serde_json::to_value(report).expect("entrypoint debug report should serialize")
}

// ---------------------------------------------------------------------------
// Report structures
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct EntrypointDebugReport {
    counts: EntrypointDebugCounts,
    entrypoints: Vec<EntrypointDetailRow>,
    trust_boundaries: Vec<TrustBoundaryDetailRow>,
    dispatch_edges: Vec<DispatchEdgeDetailRow>,
    unresolved: Vec<UnresolvedDetailRow>,
}

#[derive(Default, Serialize)]
struct EntrypointDebugCounts {
    total_entrypoints: usize,
    total_trust_boundaries: usize,
    total_dispatch_edges: usize,
    total_unresolved: usize,
    by_language: BTreeMap<String, usize>,
    by_framework_id: BTreeMap<String, usize>,
    by_kind: BTreeMap<String, usize>,
    by_status: BTreeMap<String, usize>,
    by_precision: BTreeMap<String, usize>,
    trust_boundary_by_source_kind: BTreeMap<String, usize>,
    dispatch_edge_by_edge_kind: BTreeMap<String, usize>,
    unresolved_by_reason: BTreeMap<String, usize>,
}

#[derive(Serialize)]
struct EntrypointDetailRow {
    relative_path: String,
    function_name: String,
    framework_id: String,
    kind: String,
    trigger_summary: String,
    precision: String,
    status: String,
    #[serde(rename = "stable_key")]
    stable_key_text: String,
}

#[derive(Serialize)]
struct TrustBoundaryDetailRow {
    #[serde(rename = "entrypoint_stable_key")]
    entrypoint_stable_key_text: String,
    source_kind: String,
    precision: String,
    #[serde(rename = "stable_key")]
    stable_key_text: String,
}

#[derive(Serialize)]
struct DispatchEdgeDetailRow {
    from_source: String,
    edge_kind: String,
    precision: String,
    #[serde(rename = "stable_key")]
    stable_key_text: String,
}

#[derive(Serialize)]
struct UnresolvedDetailRow {
    framework_id: String,
    reason: String,
    evidence: String,
    relative_path: String,
    #[serde(rename = "stable_key")]
    stable_key_text: String,
}

// ---------------------------------------------------------------------------
// Counting
// ---------------------------------------------------------------------------

fn entrypoint_counts(db: &AnalysisDb) -> EntrypointDebugCounts {
    let mut counts = EntrypointDebugCounts {
        total_entrypoints: db.entrypoint_facts().len(),
        total_trust_boundaries: db.trust_boundary_facts().len(),
        total_dispatch_edges: db.dispatch_edge_facts().len(),
        total_unresolved: db.unresolved_framework_facts().len(),
        ..Default::default()
    };

    for ep in db.entrypoint_facts() {
        increment(&mut counts.by_language, language_label(ep.language));
        increment(&mut counts.by_framework_id, &ep.framework_id);
        increment(&mut counts.by_kind, kind_label(ep.kind));
        increment(&mut counts.by_status, status_label(ep.status));
        increment(&mut counts.by_precision, precision_label(ep.precision));
    }

    for tb in db.trust_boundary_facts() {
        increment(
            &mut counts.trust_boundary_by_source_kind,
            source_kind_label(tb.source_kind),
        );
    }

    for de in db.dispatch_edge_facts() {
        increment(
            &mut counts.dispatch_edge_by_edge_kind,
            edge_kind_label(de.edge_kind),
        );
    }

    for uf in db.unresolved_framework_facts() {
        increment(
            &mut counts.unresolved_by_reason,
            unresolved_reason_label(uf.reason),
        );
    }

    counts
}

// ---------------------------------------------------------------------------
// Detail rows
// ---------------------------------------------------------------------------

fn entrypoint_detail_rows(db: &AnalysisDb) -> Vec<EntrypointDetailRow> {
    let mut rows: Vec<EntrypointDetailRow> = db
        .entrypoint_facts()
        .iter()
        .map(|ep| {
            let relative_path = relative_path_for(db, ep.registration_file);
            let function_name = db
                .functions()
                .iter()
                .find(|f| f.id == ep.target_function)
                .map(|f| f.name.clone())
                .unwrap_or_else(|| format!("<function:{}>", ep.target_function.0));

            let trigger_summary = trigger_summary(&ep.trigger_metadata);

            EntrypointDetailRow {
                relative_path,
                function_name,
                framework_id: ep.framework_id.clone(),
                kind: kind_label(ep.kind).to_string(),
                trigger_summary,
                precision: precision_label(ep.precision).to_string(),
                status: status_label(ep.status).to_string(),
                stable_key_text: db.resolve_stable_key(ep.stable_key).to_string(),
            }
        })
        .collect();
    rows.sort_by(|a, b| a.stable_key_text.cmp(&b.stable_key_text));
    rows
}

fn trust_boundary_detail_rows(db: &AnalysisDb) -> Vec<TrustBoundaryDetailRow> {
    let mut rows: Vec<TrustBoundaryDetailRow> = db
        .trust_boundary_facts()
        .iter()
        .map(|tb| {
            let entrypoint_stable_key = db.resolve_stable_key(tb.entrypoint_stable_key);
            let truncated_key = if entrypoint_stable_key.len() > 40 {
                format!("{}...", &entrypoint_stable_key[..40])
            } else {
                entrypoint_stable_key.to_string()
            };

            TrustBoundaryDetailRow {
                entrypoint_stable_key_text: truncated_key,
                source_kind: source_kind_label(tb.source_kind).to_string(),
                precision: precision_label(tb.precision).to_string(),
                stable_key_text: db.resolve_stable_key(tb.stable_key).to_string(),
            }
        })
        .collect();
    rows.sort_by(|a, b| a.stable_key_text.cmp(&b.stable_key_text));
    rows
}

fn dispatch_edge_detail_rows(db: &AnalysisDb) -> Vec<DispatchEdgeDetailRow> {
    let mut rows: Vec<DispatchEdgeDetailRow> = db
        .dispatch_edge_facts()
        .iter()
        .map(|de| DispatchEdgeDetailRow {
            from_source: de.from_source.clone(),
            edge_kind: edge_kind_label(de.edge_kind).to_string(),
            precision: precision_label(de.precision).to_string(),
            stable_key_text: db.resolve_stable_key(de.stable_key).to_string(),
        })
        .collect();
    rows.sort_by(|a, b| a.stable_key_text.cmp(&b.stable_key_text));
    rows
}

fn unresolved_detail_rows(db: &AnalysisDb) -> Vec<UnresolvedDetailRow> {
    let mut rows: Vec<UnresolvedDetailRow> = db
        .unresolved_framework_facts()
        .iter()
        .map(|uf| {
            let relative_path = relative_path_for(db, uf.file);
            UnresolvedDetailRow {
                framework_id: uf.framework_id.clone(),
                reason: unresolved_reason_label(uf.reason).to_string(),
                evidence: uf.evidence.clone(),
                relative_path,
                stable_key_text: db.resolve_stable_key(uf.stable_key).to_string(),
            }
        })
        .collect();
    rows.sort_by(|a, b| a.stable_key_text.cmp(&b.stable_key_text));
    rows
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn relative_path_for(db: &AnalysisDb, file: FileId) -> String {
    db.file(file)
        .map(|f| f.relative_path.clone())
        .unwrap_or_else(|| "<unknown-file>".to_string())
}

fn trigger_summary(metadata: &crate::analysis::entrypoints::facts::TriggerMetadata) -> String {
    let mut parts = Vec::new();
    if let Some(method) = &metadata.method {
        parts.push(format!("method={method}"));
    }
    if let Some(path) = &metadata.path {
        parts.push(format!("path={path}"));
    }
    if let Some(tool_name) = &metadata.tool_name {
        parts.push(format!("tool_name={tool_name}"));
    }
    if let Some(event_name) = &metadata.event_name {
        parts.push(format!("event_name={event_name}"));
    }
    if let Some(test_name) = &metadata.test_name {
        parts.push(format!("test_name={test_name}"));
    }
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(";")
    }
}

fn increment(counts: &mut BTreeMap<String, usize>, key: &str) {
    *counts.entry(key.to_string()).or_default() += 1;
}

fn language_label(language: Language) -> &'static str {
    match language {
        Language::Go => "Go",
        Language::TypeScript => "TypeScript",
        Language::Tsx => "Tsx",
        Language::JavaScript => "JavaScript",
        Language::Jsx => "Jsx",
        Language::Unknown => "Unknown",
    }
}

fn kind_label(kind: EntrypointKind) -> &'static str {
    match kind {
        EntrypointKind::HttpRoute => "HttpRoute",
        EntrypointKind::HttpMiddleware => "HttpMiddleware",
        EntrypointKind::McpTool => "McpTool",
        EntrypointKind::McpResource => "McpResource",
        EntrypointKind::McpPrompt => "McpPrompt",
        EntrypointKind::CliCommand => "CliCommand",
        EntrypointKind::Test => "Test",
        EntrypointKind::Job => "Job",
        EntrypointKind::QueueConsumer => "QueueConsumer",
        EntrypointKind::ServerlessHandler => "ServerlessHandler",
        EntrypointKind::LifecycleCallback => "LifecycleCallback",
        EntrypointKind::EventListener => "EventListener",
        EntrypointKind::GeneratedDispatch => "GeneratedDispatch",
    }
}

fn status_label(status: EntrypointStatus) -> &'static str {
    match status {
        EntrypointStatus::Resolved => "Resolved",
        EntrypointStatus::Partial => "Partial",
        EntrypointStatus::Unresolved => "Unresolved",
        EntrypointStatus::SetupMissing => "SetupMissing",
        EntrypointStatus::Unsupported => "Unsupported",
    }
}

fn precision_label(precision: EntrypointPrecision) -> &'static str {
    match precision {
        EntrypointPrecision::ResolvedStatic => "ResolvedStatic",
        EntrypointPrecision::SetupAware => "SetupAware",
        EntrypointPrecision::Heuristic => "Heuristic",
        EntrypointPrecision::Conservative => "Conservative",
        EntrypointPrecision::Unknown => "Unknown",
    }
}

fn source_kind_label(kind: TrustBoundarySourceKind) -> &'static str {
    match kind {
        TrustBoundarySourceKind::PathParam => "PathParam",
        TrustBoundarySourceKind::QueryString => "QueryString",
        TrustBoundarySourceKind::RequestBody => "RequestBody",
        TrustBoundarySourceKind::RequestHeader => "RequestHeader",
        TrustBoundarySourceKind::Cookie => "Cookie",
        TrustBoundarySourceKind::McpArguments => "McpArguments",
        TrustBoundarySourceKind::McpResourceUri => "McpResourceUri",
        TrustBoundarySourceKind::CliArgs => "CliArgs",
        TrustBoundarySourceKind::CliFlags => "CliFlags",
        TrustBoundarySourceKind::EnvVar => "EnvVar",
        TrustBoundarySourceKind::Stdin => "Stdin",
        TrustBoundarySourceKind::QueuePayload => "QueuePayload",
        TrustBoundarySourceKind::ExternalReturn => "ExternalReturn",
        TrustBoundarySourceKind::Unknown => "Unknown",
    }
}

fn edge_kind_label(kind: DispatchEdgeKind) -> &'static str {
    match kind {
        DispatchEdgeKind::RouteDispatch => "RouteDispatch",
        DispatchEdgeKind::MiddlewareChain => "MiddlewareChain",
        DispatchEdgeKind::LifecycleHook => "LifecycleHook",
        DispatchEdgeKind::EventDispatch => "EventDispatch",
        DispatchEdgeKind::McpDispatch => "McpDispatch",
        DispatchEdgeKind::TestRunner => "TestRunner",
        DispatchEdgeKind::JobScheduler => "JobScheduler",
    }
}

fn unresolved_reason_label(reason: UnresolvedFrameworkReason) -> &'static str {
    match reason {
        UnresolvedFrameworkReason::DynamicRoute => "DynamicRoute",
        UnresolvedFrameworkReason::UnknownWrapper => "UnknownWrapper",
        UnresolvedFrameworkReason::UnresolvedHandler => "UnresolvedHandler",
        UnresolvedFrameworkReason::MissingSetup => "MissingSetup",
        UnresolvedFrameworkReason::UnsupportedFrameworkVersion => "UnsupportedFrameworkVersion",
        UnresolvedFrameworkReason::BudgetExceeded => "BudgetExceeded",
        UnresolvedFrameworkReason::DynamicRegistration => "DynamicRegistration",
        UnresolvedFrameworkReason::UnrecognizedPattern => "UnrecognizedPattern",
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::entrypoints::facts::{
        EntrypointConfidence, EntrypointFact, FrameworkDispatchEdgeFact, TriggerMetadata,
        TrustBoundaryFact, UnresolvedFrameworkFact,
    };
    use crate::analysis::entrypoints::store::EntrypointOutput;
    use crate::analysis::ids::{
        DispatchEdgeId, EntrypointId, TrustBoundaryId, UnresolvedFrameworkId,
    };
    use crate::core::{FunctionFact, FunctionId, Language, Span, SymbolId};
    use std::path::PathBuf;

    fn test_db_empty() -> AnalysisDb {
        AnalysisDb::new()
    }

    fn test_db_populated() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/server.ts"),
            "src/server.ts".to_string(),
            "export function handler() {}\n".to_string(),
        );
        let function = db.push_function(FunctionFact {
            id: FunctionId(999),
            file,
            name: "handler".to_string(),
            span: Span::point(file, 1, 1),
            language: Language::TypeScript,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });

        let entrypoint = EntrypointFact {
            id: EntrypointId(0),
            language: Language::TypeScript,
            framework_id: "express".to_string(),
            kind: EntrypointKind::HttpRoute,
            target_function: function,
            target_symbol: Some(SymbolId(0)),
            registration_span: Span::point(file, 1, 1),
            registration_file: file,
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
            stable_key: crate::core::stable_key_for_test("entrypoint:express:get:/api/users"),
        };

        let trust_boundary = TrustBoundaryFact {
            id: TrustBoundaryId(0),
            entrypoint_stable_key: entrypoint.stable_key,
            source_kind: TrustBoundarySourceKind::QueryString,
            target_parameter: Some(function),
            target_parameter_index: Some(0),
            access_path: Some("req.query".to_string()),
            protocol: Some("http".to_string()),
            language: Language::TypeScript,
            file,
            span: Span::point(file, 1, 1),
            precision: EntrypointPrecision::ResolvedStatic,
            provider_id: "polint.entrypoints".to_string(),
            stable_key: crate::core::stable_key_for_test(
                "trust-boundary:express:get:/api/users:QueryString",
            ),
        };

        let dispatch_edge = FrameworkDispatchEdgeFact {
            id: DispatchEdgeId(0),
            from_source: db.resolve_stable_key(entrypoint.stable_key).to_string(),
            to_target: function,
            to_symbol: Some(SymbolId(0)),
            edge_kind: DispatchEdgeKind::RouteDispatch,
            guard_metadata: None,
            ordering: None,
            language: Language::TypeScript,
            file,
            span: Span::point(file, 1, 1),
            precision: EntrypointPrecision::ResolvedStatic,
            provider_id: "polint.entrypoints".to_string(),
            stable_key: crate::core::stable_key_for_test("dispatch:express:get:/api/users"),
        };

        let unresolved = UnresolvedFrameworkFact {
            id: UnresolvedFrameworkId(0),
            language: Language::TypeScript,
            file,
            span: Span::point(file, 5, 10),
            framework_id: "fastify".to_string(),
            reason: UnresolvedFrameworkReason::UnrecognizedPattern,
            evidence: "import fastify detected".to_string(),
            scope_description: "fastify server".to_string(),
            precision: EntrypointPrecision::Conservative,
            provider_id: "polint.entrypoints".to_string(),
            stable_key: crate::core::stable_key_for_test("unresolved:fastify:file"),
        };

        let output = EntrypointOutput {
            entrypoints: vec![entrypoint],
            trust_boundaries: vec![trust_boundary],
            dispatch_edges: vec![dispatch_edge],
            unresolved: vec![unresolved],
        };

        db.replace_entrypoint_facts(output)
            .expect("should store entrypoint facts");

        db
    }

    #[test]
    fn empty_db_produces_zero_counts() {
        let db = test_db_empty();
        let report = metadata_debug_json_for_test(&db);

        assert_eq!(report["counts"]["total_entrypoints"], 0);
        assert_eq!(report["counts"]["total_trust_boundaries"], 0);
        assert_eq!(report["counts"]["total_dispatch_edges"], 0);
        assert_eq!(report["counts"]["total_unresolved"], 0);
        assert!(report["entrypoints"].as_array().unwrap().is_empty());
        assert!(report["trust_boundaries"].as_array().unwrap().is_empty());
        assert!(report["dispatch_edges"].as_array().unwrap().is_empty());
        assert!(report["unresolved"].as_array().unwrap().is_empty());
    }

    #[test]
    fn populated_db_produces_correct_count_breakdowns() {
        let db = test_db_populated();
        let report = metadata_debug_json_for_test(&db);

        // Total counts
        assert_eq!(report["counts"]["total_entrypoints"], 1);
        assert_eq!(report["counts"]["total_trust_boundaries"], 1);
        assert_eq!(report["counts"]["total_dispatch_edges"], 1);
        assert_eq!(report["counts"]["total_unresolved"], 1);

        // Breakdown counts
        assert_eq!(report["counts"]["by_language"]["TypeScript"], 1);
        assert_eq!(report["counts"]["by_framework_id"]["express"], 1);
        assert_eq!(report["counts"]["by_kind"]["HttpRoute"], 1);
        assert_eq!(report["counts"]["by_status"]["Resolved"], 1);
        assert_eq!(report["counts"]["by_precision"]["ResolvedStatic"], 1);
        assert_eq!(
            report["counts"]["trust_boundary_by_source_kind"]["QueryString"],
            1
        );
        assert_eq!(
            report["counts"]["dispatch_edge_by_edge_kind"]["RouteDispatch"],
            1
        );
        assert_eq!(
            report["counts"]["unresolved_by_reason"]["UnrecognizedPattern"],
            1
        );
    }

    #[test]
    fn entrypoint_detail_rows_contain_relative_path_and_function_name() {
        let db = test_db_populated();
        let report = metadata_debug_json_for_test(&db);

        let entrypoints = report["entrypoints"].as_array().unwrap();
        assert_eq!(entrypoints.len(), 1);

        let ep = &entrypoints[0];
        assert_eq!(ep["relative_path"], "src/server.ts");
        assert_eq!(ep["function_name"], "handler");
        assert_eq!(ep["framework_id"], "express");
        assert_eq!(ep["kind"], "HttpRoute");
        assert_eq!(ep["precision"], "ResolvedStatic");
        assert_eq!(ep["status"], "Resolved");
        assert_eq!(ep["stable_key"], "entrypoint:express:get:/api/users");
        assert!(
            ep["trigger_summary"]
                .as_str()
                .unwrap()
                .contains("method=GET")
        );
        assert!(
            ep["trigger_summary"]
                .as_str()
                .unwrap()
                .contains("path=/api/users")
        );
    }

    #[test]
    fn trust_boundary_rows_contain_truncated_entrypoint_key() {
        let db = test_db_populated();
        let report = metadata_debug_json_for_test(&db);

        let boundaries = report["trust_boundaries"].as_array().unwrap();
        assert_eq!(boundaries.len(), 1);

        let tb = &boundaries[0];
        assert_eq!(tb["source_kind"], "QueryString");
        assert_eq!(tb["precision"], "ResolvedStatic");
        // Key is short enough to not be truncated
        assert_eq!(
            tb["entrypoint_stable_key"],
            "entrypoint:express:get:/api/users"
        );
    }

    #[test]
    fn dispatch_edge_rows_contain_from_source_and_edge_kind() {
        let db = test_db_populated();
        let report = metadata_debug_json_for_test(&db);

        let edges = report["dispatch_edges"].as_array().unwrap();
        assert_eq!(edges.len(), 1);

        let de = &edges[0];
        assert_eq!(de["from_source"], "entrypoint:express:get:/api/users");
        assert_eq!(de["edge_kind"], "RouteDispatch");
        assert_eq!(de["precision"], "ResolvedStatic");
    }

    #[test]
    fn unresolved_rows_contain_reason_and_evidence() {
        let db = test_db_populated();
        let report = metadata_debug_json_for_test(&db);

        let unresolved = report["unresolved"].as_array().unwrap();
        assert_eq!(unresolved.len(), 1);

        let uf = &unresolved[0];
        assert_eq!(uf["framework_id"], "fastify");
        assert_eq!(uf["reason"], "UnrecognizedPattern");
        assert_eq!(uf["evidence"], "import fastify detected");
        assert_eq!(uf["relative_path"], "src/server.ts");
    }

    #[test]
    fn debug_output_avoids_absolute_paths_and_dense_ids() {
        let db = test_db_populated();
        let report = metadata_debug_json_for_test(&db);
        let serialized = serde_json::to_string_pretty(&report).unwrap();

        // No absolute paths
        assert!(
            !serialized.contains("/Users/"),
            "debug output should not contain absolute paths"
        );
        assert!(
            !serialized.contains("\\Users\\"),
            "debug output should not contain absolute Windows paths"
        );

        // No dense IDs as identity (EntrypointId, TrustBoundaryId, etc.)
        // The detail rows use stable_key, relative_path, function_name - not id fields
        assert!(
            !serialized.contains("\"id\":"),
            "debug output should not expose run-local dense IDs"
        );
    }

    #[test]
    fn debug_output_is_deterministic() {
        let db = test_db_populated();
        let first = metadata_debug_json_for_test(&db);
        let second = metadata_debug_json_for_test(&db);
        assert_eq!(first, second);
    }
}

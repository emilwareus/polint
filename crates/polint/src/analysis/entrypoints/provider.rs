use crate::analysis::entrypoints::cache_key::entrypoints_provider_parameter_digest;
use crate::analysis::entrypoints::extract::extract_entrypoints;
use crate::analysis::entrypoints::store::EntrypointOutput;
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{
    CacheStats, Digest, DigestKind, InputComponent, InputSnapshot,
};
use crate::core::AnalysisDb;
use crate::diagnostics::{Diagnostic, TextRange};
use serde::Serialize;
use std::fmt::Debug;

pub(crate) const ENTRYPOINTS_PROVIDER_ID: &str = "polint.entrypoints";

#[derive(Debug, Clone, Default)]
pub(crate) struct EntrypointsProviderOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_entrypoints_with_cache_stats(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    semantic_mir_output_digest: Digest,
    cfg_output_digest: Digest,
    calls_output_digest: Digest,
    symbol_output_digest: Digest,
    module_topology_output_digest: Digest,
    upstream_syntax_output_digests: Vec<Digest>,
) -> EntrypointsProviderOutput {
    debug_assert_eq!(manifest.id, ENTRYPOINTS_PROVIDER_ID);
    // Run Go and TS/JS recognizers, derive trust boundaries, dispatch edges, merge unresolved
    let output = extract_entrypoints(db).normalized();
    let output_digest = entrypoints_output_digest(
        db,
        manifest,
        input_snapshot,
        &semantic_mir_output_digest,
        &cfg_output_digest,
        &calls_output_digest,
        &symbol_output_digest,
        &module_topology_output_digest,
        &upstream_syntax_output_digests,
        &output,
    );
    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();

    match db.replace_entrypoint_facts(output) {
        Ok(()) => EntrypointsProviderOutput {
            diagnostics: Vec::new(),
            cache_stats,
            output_digest: Some(output_digest),
        },
        Err(error) => EntrypointsProviderOutput {
            diagnostics: vec![provider_error_diagnostic(error.to_string())],
            cache_stats,
            output_digest: Some(output_digest),
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn entrypoints_output_digest(
    _db: &AnalysisDb,
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    semantic_mir_output_digest: &Digest,
    cfg_output_digest: &Digest,
    calls_output_digest: &Digest,
    symbol_output_digest: &Digest,
    module_topology_output_digest: &Digest,
    upstream_syntax_output_digests: &[Digest],
    output: &EntrypointOutput,
) -> Digest {
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!("parameters={}", entrypoints_provider_parameter_digest()),
        format!("config={}", input_snapshot.config.digest),
        format!("semantic_mir={semantic_mir_output_digest}"),
        format!("cfg={cfg_output_digest}"),
        format!("calls={calls_output_digest}"),
        format!("symbol_graph={symbol_output_digest}"),
        format!("module_topology={module_topology_output_digest}"),
    ];
    extend_component_parts(
        &mut parts,
        "go_lifecycle",
        &input_snapshot.go_lifecycle.components,
    );
    extend_component_parts(
        &mut parts,
        "ts_js_lifecycle",
        &input_snapshot.ts_js_lifecycle.components,
    );
    extend_component_parts(&mut parts, "model", &input_snapshot.models);
    extend_component_parts(&mut parts, "extension", &input_snapshot.extensions);
    extend_component_parts(&mut parts, "tool", &input_snapshot.tool_invocations);

    parts.extend(
        upstream_syntax_output_digests
            .iter()
            .map(|digest| format!("upstream_syntax={digest}")),
    );

    parts.extend(
        output
            .entrypoints
            .iter()
            .map(|ep| format!("entrypoint={}", stable_fact_payload(ep))),
    );
    parts.extend(
        output
            .trust_boundaries
            .iter()
            .map(|tb| format!("trust_boundary={}", stable_fact_payload(tb))),
    );
    parts.extend(
        output
            .dispatch_edges
            .iter()
            .map(|de| format!("dispatch_edge={}", stable_fact_payload(de))),
    );
    parts.extend(
        output
            .unresolved
            .iter()
            .map(|ur| format!("unresolved_framework={}", stable_fact_payload(ur))),
    );
    if output.entrypoints.is_empty()
        && output.trust_boundaries.is_empty()
        && output.dispatch_edges.is_empty()
        && output.unresolved.is_empty()
    {
        parts.push("entrypoint_output=empty".to_string());
    }

    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "entrypoints_output", &refs)
}

fn extend_component_parts(parts: &mut Vec<String>, prefix: &str, components: &[InputComponent]) {
    parts.extend(components.iter().map(|component| {
        format!(
            "{prefix}:{}:{:?}:{}",
            component.name, component.status, component.digest
        )
    }));
}

fn provider_error_diagnostic(message: String) -> Diagnostic {
    let _message = message;
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        TextRange::point(1, 1),
        "Framework entrypoint analysis failed; run internal debug output for details.",
    )
}

fn stable_fact_payload<T>(fact: &T) -> String
where
    T: Serialize + Debug,
{
    serde_json::to_string(fact).unwrap_or_else(|_| format!("{fact:?}"))
}

#[cfg(test)]
fn entrypoints_output_digest_for_test(parts: &[&str]) -> Digest {
    Digest::from_parts(DigestKind::ProviderOutput, "entrypoints_output", parts)
}

#[cfg(test)]
mod entrypoints_provider {
    use crate::analysis_kernel::AnalysisKernel;
    use crate::analysis_kernel::incremental::Digest;

    #[test]
    fn entrypoints_provider_accepts_empty_output_with_deterministic_digest() {
        let first = super::entrypoints_output_digest_for_test(&[]);
        let second = super::entrypoints_output_digest_for_test(&[]);

        assert_eq!(first, second);
        assert!(!first.value.is_empty());
    }

    #[test]
    fn entrypoints_provider_manifest_declares_private_outputs() {
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.entrypoints")
            .expect("entrypoints manifest should exist");

        assert_eq!(manifest.primary_schema_label(), "entrypoints-facts-1:1");
        assert!(manifest.outputs.contains(&"entrypoints"));
        assert!(manifest.outputs.contains(&"trust_boundaries"));
        assert!(manifest.outputs.contains(&"dispatch_edges"));
        assert!(manifest.outputs.contains(&"unresolved_framework"));
    }

    #[test]
    fn entrypoints_provider_populates_empty_output_with_deterministic_digest() {
        let mut first_db = crate::core::AnalysisDb::new();
        let mut second_db = crate::core::AnalysisDb::new();
        let first = derive_for_test(&mut first_db);
        let second = derive_for_test(&mut second_db);

        assert_eq!(first.output_digest, second.output_digest);
        assert!(first_db.entrypoint_facts().is_empty());
        assert!(first_db.trust_boundary_facts().is_empty());
        assert!(first_db.dispatch_edge_facts().is_empty());
        assert!(first_db.unresolved_framework_facts().is_empty());
    }

    #[test]
    fn populated_output_produces_non_absent_digest() {
        use crate::analysis::entrypoints::facts::{
            EntrypointConfidence, EntrypointFact, EntrypointKind, EntrypointPrecision,
            EntrypointProvenance, EntrypointStatus, TriggerMetadata,
        };
        use crate::analysis::entrypoints::store::EntrypointOutput;
        use crate::analysis::ids::EntrypointId;
        use crate::analysis_kernel::incremental::{DigestKind, InputSnapshot};
        use crate::analysis_plan::AnalysisPlan;
        use crate::config::load_config;
        use crate::core::{FileId, FunctionId, Language, Span};

        let db = crate::core::AnalysisDb::new();
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        let snapshot = InputSnapshot::from_run_inputs(
            &loaded,
            &db,
            "config-a",
            "rules-a",
            AnalysisPlan::empty().digest(),
            AnalysisKernel::provider_manifests(),
        );
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|m| m.id == "polint.entrypoints")
            .expect("manifest");

        // Create a populated output directly and compute its digest
        let output = EntrypointOutput {
            entrypoints: vec![EntrypointFact {
                id: EntrypointId(0),
                language: Language::Go,
                framework_id: "go.net_http".to_string(),
                kind: EntrypointKind::HttpRoute,
                target_function: FunctionId(1),
                target_symbol: None,
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
                precision: EntrypointPrecision::Heuristic,
                provenance: EntrypointProvenance::NativeRecognizer,
                confidence: EntrypointConfidence::High,
                status: EntrypointStatus::Resolved,
                provider_id: "polint.entrypoints".to_string(),
                stable_key: "ep-test".to_string(),
            }],
            trust_boundaries: Vec::new(),
            dispatch_edges: Vec::new(),
            unresolved: Vec::new(),
        }
        .normalized();

        let digest = super::entrypoints_output_digest(
            &db,
            manifest,
            &snapshot,
            &Digest::from_parts(DigestKind::ProviderOutput, "semantic_mir", &["a"]),
            &Digest::from_parts(DigestKind::ProviderOutput, "cfg", &["a"]),
            &Digest::from_parts(DigestKind::ProviderOutput, "calls", &["a"]),
            &Digest::from_parts(DigestKind::ProviderOutput, "symbol_graph", &["a"]),
            &Digest::from_parts(DigestKind::ProviderOutput, "module_topology", &["a"]),
            &[],
            &output,
        );

        assert!(
            !digest.value.is_empty(),
            "populated output should produce non-empty digest"
        );
    }

    #[test]
    fn output_digest_changes_when_entrypoints_added() {
        use crate::analysis::entrypoints::facts::{
            EntrypointConfidence, EntrypointFact, EntrypointKind, EntrypointPrecision,
            EntrypointProvenance, EntrypointStatus, TriggerMetadata,
        };
        use crate::analysis::entrypoints::store::EntrypointOutput;
        use crate::analysis::ids::EntrypointId;
        use crate::analysis_kernel::incremental::{DigestKind, InputSnapshot};
        use crate::analysis_plan::AnalysisPlan;
        use crate::config::load_config;
        use crate::core::{FileId, FunctionId, Language, Span};

        let db = crate::core::AnalysisDb::new();
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        let snapshot = InputSnapshot::from_run_inputs(
            &loaded,
            &db,
            "config-a",
            "rules-a",
            AnalysisPlan::empty().digest(),
            AnalysisKernel::provider_manifests(),
        );
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|m| m.id == "polint.entrypoints")
            .expect("manifest");

        let upstream_mir = Digest::from_parts(DigestKind::ProviderOutput, "semantic_mir", &["a"]);
        let upstream_cfg = Digest::from_parts(DigestKind::ProviderOutput, "cfg", &["a"]);
        let upstream_calls = Digest::from_parts(DigestKind::ProviderOutput, "calls", &["a"]);
        let upstream_symbol =
            Digest::from_parts(DigestKind::ProviderOutput, "symbol_graph", &["a"]);
        let upstream_topology =
            Digest::from_parts(DigestKind::ProviderOutput, "module_topology", &["a"]);

        let empty_output = EntrypointOutput::empty().normalized();
        let empty_digest = super::entrypoints_output_digest(
            &db,
            manifest,
            &snapshot,
            &upstream_mir,
            &upstream_cfg,
            &upstream_calls,
            &upstream_symbol,
            &upstream_topology,
            &[],
            &empty_output,
        );

        let populated_output = EntrypointOutput {
            entrypoints: vec![EntrypointFact {
                id: EntrypointId(0),
                language: Language::Go,
                framework_id: "go.net_http".to_string(),
                kind: EntrypointKind::HttpRoute,
                target_function: FunctionId(1),
                target_symbol: None,
                registration_span: Span::point(FileId(1), 1, 1),
                registration_file: FileId(1),
                trigger_metadata: TriggerMetadata::empty(),
                trust_boundary_link: None,
                precision: EntrypointPrecision::Heuristic,
                provenance: EntrypointProvenance::NativeRecognizer,
                confidence: EntrypointConfidence::High,
                status: EntrypointStatus::Resolved,
                provider_id: "polint.entrypoints".to_string(),
                stable_key: "ep-test".to_string(),
            }],
            trust_boundaries: Vec::new(),
            dispatch_edges: Vec::new(),
            unresolved: Vec::new(),
        }
        .normalized();
        let populated_digest = super::entrypoints_output_digest(
            &db,
            manifest,
            &snapshot,
            &upstream_mir,
            &upstream_cfg,
            &upstream_calls,
            &upstream_symbol,
            &upstream_topology,
            &[],
            &populated_output,
        );

        assert_ne!(
            empty_digest.value, populated_digest.value,
            "output digest should change when entrypoints are added"
        );
    }

    #[test]
    fn output_digest_is_deterministic_for_same_input() {
        use crate::analysis::entrypoints::facts::{
            EntrypointConfidence, EntrypointFact, EntrypointKind, EntrypointPrecision,
            EntrypointProvenance, EntrypointStatus, TriggerMetadata,
        };
        use crate::analysis::entrypoints::store::EntrypointOutput;
        use crate::analysis::ids::EntrypointId;
        use crate::analysis_kernel::incremental::{DigestKind, InputSnapshot};
        use crate::analysis_plan::AnalysisPlan;
        use crate::config::load_config;
        use crate::core::{FileId, FunctionId, Language, Span};

        let db = crate::core::AnalysisDb::new();
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        let snapshot = InputSnapshot::from_run_inputs(
            &loaded,
            &db,
            "config-a",
            "rules-a",
            AnalysisPlan::empty().digest(),
            AnalysisKernel::provider_manifests(),
        );
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|m| m.id == "polint.entrypoints")
            .expect("manifest");

        let make_output = || {
            EntrypointOutput {
                entrypoints: vec![EntrypointFact {
                    id: EntrypointId(0),
                    language: Language::Go,
                    framework_id: "go.net_http".to_string(),
                    kind: EntrypointKind::HttpRoute,
                    target_function: FunctionId(1),
                    target_symbol: None,
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
                    precision: EntrypointPrecision::Heuristic,
                    provenance: EntrypointProvenance::NativeRecognizer,
                    confidence: EntrypointConfidence::High,
                    status: EntrypointStatus::Resolved,
                    provider_id: "polint.entrypoints".to_string(),
                    stable_key: "ep-test".to_string(),
                }],
                trust_boundaries: Vec::new(),
                dispatch_edges: Vec::new(),
                unresolved: Vec::new(),
            }
            .normalized()
        };

        let upstream_mir = Digest::from_parts(DigestKind::ProviderOutput, "semantic_mir", &["a"]);
        let upstream_cfg = Digest::from_parts(DigestKind::ProviderOutput, "cfg", &["a"]);
        let upstream_calls = Digest::from_parts(DigestKind::ProviderOutput, "calls", &["a"]);
        let upstream_symbol =
            Digest::from_parts(DigestKind::ProviderOutput, "symbol_graph", &["a"]);
        let upstream_topology =
            Digest::from_parts(DigestKind::ProviderOutput, "module_topology", &["a"]);

        let first = super::entrypoints_output_digest(
            &db,
            manifest,
            &snapshot,
            &upstream_mir,
            &upstream_cfg,
            &upstream_calls,
            &upstream_symbol,
            &upstream_topology,
            &[],
            &make_output(),
        );
        let second = super::entrypoints_output_digest(
            &db,
            manifest,
            &snapshot,
            &upstream_mir,
            &upstream_cfg,
            &upstream_calls,
            &upstream_symbol,
            &upstream_topology,
            &[],
            &make_output(),
        );

        assert_eq!(
            first.value, second.value,
            "output digest should be deterministic for the same input"
        );
    }

    #[test]
    fn output_digest_changes_when_trigger_metadata_changes() {
        use crate::analysis::entrypoints::facts::{
            EntrypointConfidence, EntrypointFact, EntrypointKind, EntrypointPrecision,
            EntrypointProvenance, EntrypointStatus, TriggerMetadata,
        };
        use crate::analysis::entrypoints::store::EntrypointOutput;
        use crate::analysis::ids::EntrypointId;
        use crate::analysis_kernel::incremental::{DigestKind, InputSnapshot};
        use crate::analysis_plan::AnalysisPlan;
        use crate::config::load_config;
        use crate::core::{FileId, FunctionId, Language, Span};

        let db = crate::core::AnalysisDb::new();
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        let snapshot = InputSnapshot::from_run_inputs(
            &loaded,
            &db,
            "config-a",
            "rules-a",
            AnalysisPlan::empty().digest(),
            AnalysisKernel::provider_manifests(),
        );
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|m| m.id == "polint.entrypoints")
            .expect("manifest");

        let make_output = |path: &str| {
            EntrypointOutput {
                entrypoints: vec![EntrypointFact {
                    id: EntrypointId(0),
                    language: Language::TypeScript,
                    framework_id: "ts.express".to_string(),
                    kind: EntrypointKind::HttpRoute,
                    target_function: FunctionId(1),
                    target_symbol: None,
                    registration_span: Span::point(FileId(1), 1, 1),
                    registration_file: FileId(1),
                    trigger_metadata: TriggerMetadata {
                        method: Some("GET".to_string()),
                        path: Some(path.to_string()),
                        tool_name: None,
                        event_name: None,
                        test_name: None,
                    },
                    trust_boundary_link: None,
                    precision: EntrypointPrecision::Heuristic,
                    provenance: EntrypointProvenance::NativeRecognizer,
                    confidence: EntrypointConfidence::High,
                    status: EntrypointStatus::Resolved,
                    provider_id: "polint.entrypoints".to_string(),
                    stable_key: "ep-route".to_string(),
                }],
                trust_boundaries: Vec::new(),
                dispatch_edges: Vec::new(),
                unresolved: Vec::new(),
            }
            .normalized()
        };

        let upstream_mir = Digest::from_parts(DigestKind::ProviderOutput, "semantic_mir", &["a"]);
        let upstream_cfg = Digest::from_parts(DigestKind::ProviderOutput, "cfg", &["a"]);
        let upstream_calls = Digest::from_parts(DigestKind::ProviderOutput, "calls", &["a"]);
        let upstream_symbol =
            Digest::from_parts(DigestKind::ProviderOutput, "symbol_graph", &["a"]);
        let upstream_topology =
            Digest::from_parts(DigestKind::ProviderOutput, "module_topology", &["a"]);

        let first = super::entrypoints_output_digest(
            &db,
            manifest,
            &snapshot,
            &upstream_mir,
            &upstream_cfg,
            &upstream_calls,
            &upstream_symbol,
            &upstream_topology,
            &[],
            &make_output("/api/one"),
        );
        let second = super::entrypoints_output_digest(
            &db,
            manifest,
            &snapshot,
            &upstream_mir,
            &upstream_cfg,
            &upstream_calls,
            &upstream_symbol,
            &upstream_topology,
            &[],
            &make_output("/api/two"),
        );

        assert_ne!(
            first, second,
            "output digest must change when trigger metadata changes"
        );
    }

    #[test]
    fn provider_error_diagnostic_does_not_leak_framework_internal_markers() {
        let diagnostic = super::provider_error_diagnostic(
            "invalid semantic fact from polint.entrypoints: dangling EntrypointFact".to_string(),
        );
        let rendered = crate::diagnostics::render(
            crate::diagnostics::OutputFormat::Json,
            &[diagnostic],
            crate::diagnostics::RenderOpts {
                json: crate::diagnostics::JsonReportMeta {
                    tool_name: "polint",
                    tool_version: "test",
                },
                color: crate::diagnostics::ColorChoice::Never,
                sources: None,
            },
        );

        for marker in ["polint.entrypoints", "EntrypointFact"] {
            assert!(
                !rendered.contains(marker),
                "provider error leaked internal marker `{marker}`: {rendered}"
            );
        }
    }

    fn derive_for_test(db: &mut crate::core::AnalysisDb) -> super::EntrypointsProviderOutput {
        use crate::analysis_kernel::incremental::{DigestKind, InputSnapshot};
        use crate::analysis_plan::AnalysisPlan;
        use crate::config::load_config;

        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        let snapshot = InputSnapshot::from_run_inputs(
            &loaded,
            db,
            "config-a",
            "rules-a",
            AnalysisPlan::empty().digest(),
            AnalysisKernel::provider_manifests(),
        );
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.entrypoints")
            .expect("entrypoints manifest");

        super::derive_entrypoints_with_cache_stats(
            db,
            &snapshot,
            manifest,
            Digest::from_parts(DigestKind::ProviderOutput, "semantic_mir", &["a"]),
            Digest::from_parts(DigestKind::ProviderOutput, "cfg", &["a"]),
            Digest::from_parts(DigestKind::ProviderOutput, "calls", &["a"]),
            Digest::from_parts(DigestKind::ProviderOutput, "symbol_graph", &["a"]),
            Digest::from_parts(DigestKind::ProviderOutput, "module_topology", &["a"]),
            Vec::new(),
        )
    }
}

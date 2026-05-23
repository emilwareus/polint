use crate::analysis::entrypoints::cache_key::entrypoints_provider_parameter_digest;
use crate::analysis::entrypoints::store::EntrypointOutput;
use crate::analysis_kernel::incremental::{
    CacheStats, Digest, DigestKind, InputComponent, InputSnapshot,
};
use crate::analysis_kernel::ProviderManifest;
use crate::core::AnalysisDb;
use crate::diagnostics::{Diagnostic, TextRange};

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
    // For now the provider produces empty output; recognizers are added in Plans 03-04.
    let output = EntrypointOutput::empty().normalized();
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
        format!(
            "parameters={}",
            entrypoints_provider_parameter_digest()
        ),
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

    // Per-fact stable payload lines for entrypoints
    parts.extend(output.entrypoints.iter().map(|ep| {
        format!(
            "entrypoint={} language={:?} kind={:?} framework={} status={:?} precision={:?} provenance={:?}",
            ep.stable_key,
            ep.language,
            ep.kind,
            ep.framework_id,
            ep.status,
            ep.precision,
            ep.provenance,
        )
    }));
    parts.extend(output.trust_boundaries.iter().map(|tb| {
        format!(
            "trust_boundary={} entrypoint={} source_kind={:?} precision={:?}",
            tb.stable_key,
            tb.entrypoint_stable_key,
            tb.source_kind,
            tb.precision,
        )
    }));
    parts.extend(output.dispatch_edges.iter().map(|de| {
        format!(
            "dispatch_edge={} from={} edge_kind={:?} precision={:?}",
            de.stable_key, de.from_source, de.edge_kind, de.precision,
        )
    }));
    parts.extend(output.unresolved.iter().map(|ur| {
        format!(
            "unresolved_framework={} framework={} reason={:?} precision={:?}",
            ur.stable_key, ur.framework_id, ur.reason, ur.precision,
        )
    }));
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
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        TextRange::point(1, 1),
        format!("Entrypoints provider failed: {message}"),
    )
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

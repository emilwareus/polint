use serde::Serialize;
use std::fmt::Debug;

use super::cache_key::{
    evidence_provider_parameter_digest, evidence_provider_parameter_digest_for_snapshot,
};
use super::store::EvidenceOutput;
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{
    CacheStats, Digest, DigestKind, InputComponent, InputSnapshot,
};
use crate::core::AnalysisDb;
use crate::diagnostics::Diagnostic;

pub(crate) const EVIDENCE_PROVIDER_ID: &str = "polint.evidence";

#[derive(Debug, Clone, Default)]
pub(crate) struct EvidenceProviderOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_evidence_with_cache_stats(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    semantic_mir_output_digest: Digest,
    cfg_output_digest: Digest,
    calls_output_digest: Digest,
    refined_calls_output_digest: Digest,
    direct_summaries_output_digest: Digest,
    type_value_alias_output_digest: Digest,
    entrypoints_output_digest: Digest,
    extensions_output_digest: Digest,
    data_flow_output_digest: Digest,
) -> EvidenceProviderOutput {
    debug_assert_eq!(manifest.id, EVIDENCE_PROVIDER_ID);
    let output = EvidenceOutput::empty().normalized();
    let output_digest = evidence_output_digest(
        manifest,
        input_snapshot,
        &semantic_mir_output_digest,
        &cfg_output_digest,
        &calls_output_digest,
        &refined_calls_output_digest,
        &direct_summaries_output_digest,
        &type_value_alias_output_digest,
        &entrypoints_output_digest,
        &extensions_output_digest,
        &data_flow_output_digest,
        &output,
    );
    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();

    match db.replace_evidence_facts(output) {
        Ok(()) => EvidenceProviderOutput {
            diagnostics: Vec::new(),
            cache_stats,
            output_digest: Some(output_digest),
        },
        Err(error) => EvidenceProviderOutput {
            diagnostics: vec![provider_error_diagnostic(error.to_string())],
            cache_stats,
            output_digest: Some(output_digest),
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn evidence_output_digest(
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    semantic_mir_output_digest: &Digest,
    cfg_output_digest: &Digest,
    calls_output_digest: &Digest,
    refined_calls_output_digest: &Digest,
    direct_summaries_output_digest: &Digest,
    type_value_alias_output_digest: &Digest,
    entrypoints_output_digest: &Digest,
    extensions_output_digest: &Digest,
    data_flow_output_digest: &Digest,
    output: &EvidenceOutput,
) -> Digest {
    let upstream = vec![
        semantic_mir_output_digest.clone(),
        cfg_output_digest.clone(),
        calls_output_digest.clone(),
        refined_calls_output_digest.clone(),
        direct_summaries_output_digest.clone(),
        type_value_alias_output_digest.clone(),
        entrypoints_output_digest.clone(),
        extensions_output_digest.clone(),
        data_flow_output_digest.clone(),
    ];
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!("parameters={}", evidence_provider_parameter_digest()),
        format!(
            "input_parameters={}",
            evidence_provider_parameter_digest_for_snapshot(input_snapshot, &upstream)
        ),
        format!("semantic_mir={semantic_mir_output_digest}"),
        format!("cfg={cfg_output_digest}"),
        format!("calls={calls_output_digest}"),
        format!("refined_calls={refined_calls_output_digest}"),
        format!("direct_summaries={direct_summaries_output_digest}"),
        format!("type_value_alias={type_value_alias_output_digest}"),
        format!("entrypoints={entrypoints_output_digest}"),
        format!("extensions={extensions_output_digest}"),
        format!("data_flow={data_flow_output_digest}"),
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
        output
            .nodes
            .iter()
            .map(|node| format!("evidence_node={}", stable_fact_payload(node))),
    );
    parts.extend(
        output
            .edges
            .iter()
            .map(|edge| format!("evidence_edge={}", stable_fact_payload(edge))),
    );
    parts.extend(
        output
            .bundles
            .iter()
            .map(|bundle| format!("evidence_bundle={}", stable_fact_payload(bundle))),
    );
    if output.nodes.is_empty()
        && output.edges.is_empty()
        && output.bundles.is_empty()
        && output.paths.is_empty()
        && output.slices.is_empty()
    {
        parts.push("evidence_output=empty".to_string());
    }
    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "evidence_output", &refs)
}

fn extend_component_parts(parts: &mut Vec<String>, prefix: &str, components: &[InputComponent]) {
    if components.is_empty() {
        parts.push(format!("{prefix}=absent"));
        return;
    }
    parts.extend(components.iter().map(|component| {
        format!(
            "{prefix}:{}:{:?}:{}",
            component.name, component.status, component.digest
        )
    }));
}

fn stable_fact_payload<T>(fact: &T) -> String
where
    T: Serialize + Debug,
{
    serde_json::to_string(fact).unwrap_or_else(|_| format!("{fact:?}"))
}

fn provider_error_diagnostic(message: String) -> Diagnostic {
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        crate::diagnostics::TextRange::point(1, 1),
        format!("Evidence provider failed: {message}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::AnalysisKernel;

    #[test]
    fn evidence_runs_after_data_flow_and_before_metrics() {
        let order = AnalysisKernel::provider_manifests()
            .iter()
            .map(|manifest| manifest.id)
            .collect::<Vec<_>>();
        let data_flow = order
            .iter()
            .position(|provider| *provider == "polint.data_flow")
            .expect("data-flow provider");
        let evidence = order
            .iter()
            .position(|provider| *provider == EVIDENCE_PROVIDER_ID)
            .expect("evidence provider");
        let metrics = order
            .iter()
            .position(|provider| *provider == "polint.metrics")
            .expect("metrics provider");

        assert!(data_flow < evidence);
        assert!(evidence < metrics);
    }
}

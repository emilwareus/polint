//! Neutral direct-summary digest and host projection helpers.
//!
//! Cache/provider orchestration remains in the facade, while these helpers own
//! deterministic output identity over the summary facts themselves.

use polint_analysis_api::{
    Digest, DigestKind, InputComponent, InputSnapshot, MirBodyId, ProviderManifest,
};
use polint_core::{StableKeyId, StableKeyInterner};

use super::cache_key::direct_summaries_provider_parameter_digest;
use super::store::SummaryOutput;
use crate::AnalysisHost;

#[allow(clippy::too_many_arguments)]
pub fn direct_summaries_output_digest(
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    semantic_mir_output_digest: &Digest,
    cfg_output_digest: &Digest,
    calls_output_digest: &Digest,
    abstract_domains_output_digest: &Digest,
    symbol_graph_output_digest: &Digest,
    module_topology_output_digest: &Digest,
    upstream_syntax_output_digests: &[Digest],
    interner: &StableKeyInterner,
    callable_keys: &std::collections::BTreeMap<MirBodyId, StableKeyId>,
    output: &SummaryOutput,
) -> Digest {
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!(
            "parameters={}",
            direct_summaries_provider_parameter_digest()
        ),
        format!("config={}", input_snapshot.config.digest),
        format!("semantic_mir={semantic_mir_output_digest}"),
        format!("cfg={cfg_output_digest}"),
        format!("calls={calls_output_digest}"),
        format!("abstract_domains={abstract_domains_output_digest}"),
        format!("symbol_graph={symbol_graph_output_digest}"),
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
    parts.extend(output.summaries.iter().map(|row| {
        let callable = callable_keys
            .get(&MirBodyId(row.function.0))
            .copied()
            .unwrap_or(row.callable_stable_key);
        format!(
            "summary={} callable={} domain={:?} status={:?} precision={:?} provenance={:?} payload={} tito_flows={:?}",
            interner.resolve(row.stable_key), interner.resolve(callable), row.domain,
            row.status, row.precision, row.provenance, row.payload_digest, row.tito_flows,
        )
    }));
    parts.extend(output.events.iter().map(|row| {
        let callable = callable_keys
            .get(&MirBodyId(row.function.0))
            .copied()
            .unwrap_or(row.callable_stable_key);
        format!(
            "event={} callable={} domain={:?} kind={} status={:?} precision={:?} reason={}",
            interner.resolve(row.stable_key),
            interner.resolve(callable),
            row.domain,
            row.event_kind,
            row.status,
            row.precision,
            row.reason,
        )
    }));
    if output.summaries.is_empty() && output.events.is_empty() {
        parts.push("summaries_output=empty".to_string());
    }
    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "direct_summaries_output", &refs)
}

fn extend_component_parts(parts: &mut Vec<String>, prefix: &str, components: &[InputComponent]) {
    parts.extend(components.iter().map(|component| {
        format!(
            "{prefix}:{}:{:?}:{}",
            component.name, component.status, component.digest
        )
    }));
}

pub fn callable_stable_key_map(
    db: &impl AnalysisHost,
) -> std::collections::BTreeMap<MirBodyId, StableKeyId> {
    db.mir_bodies()
        .iter()
        .map(|body| (body.id, body.stable_key))
        .collect()
}

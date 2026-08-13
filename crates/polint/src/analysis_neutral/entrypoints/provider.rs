use crate::analysis_api::ProviderManifest;
use crate::analysis_api::{
    CacheStats, Digest, DigestKind, InputComponent, InputSnapshot, ProviderExecution,
    ProviderFailureReason, ProviderFailureStage,
};
use crate::analysis_neutral::AnalysisHost;
use crate::analysis_neutral::entrypoints::cache_key::entrypoints_provider_parameter_digest;
use crate::analysis_neutral::entrypoints::extract::extract_entrypoints;
use crate::analysis_neutral::entrypoints::store::EntrypointOutput;
use crate::internal_core::{Diagnostic, DiagnosticRange};
use std::fmt::Debug;

pub const ENTRYPOINTS_PROVIDER_ID: &str = "polint.entrypoints";

#[derive(Debug, Clone, Default)]
pub struct EntrypointsProviderOutput {
    pub diagnostics: Vec<Diagnostic>,
    pub cache_stats: CacheStats,
    pub output_digest: Option<Digest>,
    pub execution: ProviderExecution,
}

#[allow(clippy::too_many_arguments)]
pub fn derive_entrypoints_with_cache_stats(
    db: &mut impl AnalysisHost,
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
    let interner = db.stable_key_interner();
    let output = extract_entrypoints(db).normalized(&interner);
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
            execution: Default::default(),
        },
        Err(error) => EntrypointsProviderOutput {
            diagnostics: vec![provider_error_diagnostic(error.to_string())],
            cache_stats,
            output_digest: None,
            execution: ProviderExecution::Failed {
                stage: ProviderFailureStage::Validation,
                reason: ProviderFailureReason::ValidationRejected,
            },
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn entrypoints_output_digest(
    _db: &impl AnalysisHost,
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

    parts.extend(output.entrypoints.iter().map(|ep| {
        format!(
            "entrypoint={}",
            stable_fact_payload(&_db.stable_key_interner(), ep)
        )
    }));
    parts.extend(output.trust_boundaries.iter().map(|tb| {
        format!(
            "trust_boundary={}",
            stable_fact_payload(&_db.stable_key_interner(), tb)
        )
    }));
    parts.extend(output.dispatch_edges.iter().map(|de| {
        format!(
            "dispatch_edge={}",
            stable_fact_payload(&_db.stable_key_interner(), de)
        )
    }));
    parts.extend(output.unresolved.iter().map(|ur| {
        format!(
            "unresolved_framework={}",
            stable_fact_payload(&_db.stable_key_interner(), ur)
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
    let _message = message;
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        DiagnosticRange::point(1, 1),
        "Framework entrypoint analysis failed; run internal debug output for details.",
    )
}

fn stable_fact_payload<T>(interner: &crate::internal_core::StableKeyInterner, fact: &T) -> String
where
    T: Debug,
{
    resolve_stable_key_ids(interner, &format!("{fact:?}"))
}

fn resolve_stable_key_ids(
    interner: &crate::internal_core::StableKeyInterner,
    payload: &str,
) -> String {
    let mut resolved = String::with_capacity(payload.len());
    let mut remaining = payload;
    while let Some(start) = remaining.find("StableKeyId(") {
        resolved.push_str(&remaining[..start]);
        let id_start = start + "StableKeyId(".len();
        let Some(relative_end) = remaining[id_start..].find(')') else {
            resolved.push_str(&remaining[start..]);
            return resolved;
        };
        let id_end = id_start + relative_end;
        let Ok(id) = remaining[id_start..id_end].parse::<u32>() else {
            resolved.push_str(&remaining[start..=id_end]);
            remaining = &remaining[id_end + 1..];
            continue;
        };
        resolved.push_str(&format!(
            "{:?}",
            interner.resolve(crate::internal_core::StableKeyId(id))
        ));
        remaining = &remaining[id_end + 1..];
    }
    resolved.push_str(remaining);
    resolved
}

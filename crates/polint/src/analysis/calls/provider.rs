use std::collections::BTreeMap;

use crate::analysis::calls::store::CallOutput;
use crate::analysis::ids::CallSiteId;
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{
    CacheStats, Digest, DigestKind, InputComponent, InputSnapshot,
};
use crate::core::AnalysisDb;
use crate::diagnostics::{Diagnostic, TextRange};

#[derive(Debug, Clone, Default)]
pub(crate) struct CallsProviderOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}

pub(crate) fn derive_calls_with_cache_stats(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    semantic_mir_output_digest: Digest,
    cfg_output_digest: Digest,
    symbol_graph_output_digest: Digest,
    module_topology_output_digest: Digest,
    upstream_syntax_output_digests: Vec<Digest>,
) -> CallsProviderOutput {
    let output = CallOutput::empty().normalized();
    let output_digest = calls_output_digest(
        manifest,
        input_snapshot,
        &semantic_mir_output_digest,
        &cfg_output_digest,
        &symbol_graph_output_digest,
        &module_topology_output_digest,
        &upstream_syntax_output_digests,
        &output,
    );
    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();

    match db.replace_call_facts(output) {
        Ok(()) => CallsProviderOutput {
            diagnostics: Vec::new(),
            cache_stats,
            output_digest: Some(output_digest),
        },
        Err(error) => CallsProviderOutput {
            diagnostics: vec![provider_error_diagnostic(error.to_string())],
            cache_stats,
            output_digest: Some(output_digest),
        },
    }
}

fn calls_output_digest(
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    semantic_mir_output_digest: &Digest,
    cfg_output_digest: &Digest,
    symbol_graph_output_digest: &Digest,
    module_topology_output_digest: &Digest,
    upstream_syntax_output_digests: &[Digest],
    output: &CallOutput,
) -> Digest {
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!("config={}", input_snapshot.config.digest),
        format!("semantic_mir={semantic_mir_output_digest}"),
        format!("cfg={cfg_output_digest}"),
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

    let site_keys = call_site_key_map(output);
    parts.extend(output.sites.iter().map(|site| {
        format!(
            "call_site={} language={:?} span={} kind={:?} status={:?} precision={:?} callee={}",
            site.stable_key,
            site.language,
            span_part(&site.span),
            site.kind,
            site.status,
            site.precision,
            callee_part(&site.callee),
        )
    }));
    parts.extend(output.targets.iter().map(|target| {
        format!(
            "call_target={} site={} edge={:?} algorithm={:?} status={:?} reason={} provenance={:?} precision={:?} target_function={} target_symbol={}",
            target.stable_key,
            stable_site_key(&site_keys, target.site),
            target.edge_kind,
            target.algorithm,
            target.status,
            target.reason.map(|reason| format!("{reason:?}")).unwrap_or_else(|| "none".to_string()),
            target.provenance,
            target.precision,
            presence_label(target.target_function.is_some()),
            presence_label(target.target_symbol.is_some()),
        )
    }));
    parts.extend(output.unresolved.iter().map(|unresolved| {
        format!(
            "unresolved_call={} site={} algorithm={:?} status={:?} reason={:?} provenance={:?} precision={:?}",
            unresolved.stable_key,
            stable_site_key(&site_keys, unresolved.site),
            unresolved.algorithm,
            unresolved.status,
            unresolved.reason,
            unresolved.provenance,
            unresolved.precision,
        )
    }));
    if output.sites.is_empty() && output.targets.is_empty() && output.unresolved.is_empty() {
        parts.push("call_output=empty".to_string());
    }

    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "calls_output", &refs)
}

fn extend_component_parts(parts: &mut Vec<String>, prefix: &str, components: &[InputComponent]) {
    parts.extend(components.iter().map(|component| {
        format!(
            "{prefix}:{}:{:?}:{}",
            component.name, component.status, component.digest
        )
    }));
}

fn call_site_key_map(output: &CallOutput) -> BTreeMap<CallSiteId, String> {
    output
        .sites
        .iter()
        .map(|site| (site.id, site.stable_key.clone()))
        .collect()
}

fn stable_site_key(keys: &BTreeMap<CallSiteId, String>, site: CallSiteId) -> String {
    keys.get(&site)
        .cloned()
        .unwrap_or_else(|| "<missing-site>".to_string())
}

fn span_part(span: &crate::core::Span) -> String {
    format!(
        "{}:{}..{}:{}@{}..{}",
        span.start_line,
        span.start_col,
        span.end_line,
        span.end_col,
        span.start_byte,
        span.end_byte
    )
}

fn callee_part(callee: &crate::analysis::calls::facts::CallCallee) -> String {
    match callee {
        crate::analysis::calls::facts::CallCallee::Identifier { name, .. } => {
            format!("identifier:{name}")
        }
        crate::analysis::calls::facts::CallCallee::Member { property, .. } => {
            format!("member:{property}")
        }
        crate::analysis::calls::facts::CallCallee::Index { .. } => "index".to_string(),
        crate::analysis::calls::facts::CallCallee::Super => "super".to_string(),
        crate::analysis::calls::facts::CallCallee::Import => "import".to_string(),
        crate::analysis::calls::facts::CallCallee::FunctionValue { .. } => {
            "function_value".to_string()
        }
        crate::analysis::calls::facts::CallCallee::Constructor { name, .. } => {
            format!("constructor:{}", name.as_deref().unwrap_or("unknown"))
        }
        crate::analysis::calls::facts::CallCallee::Unknown { reason } => {
            format!("unknown:{reason:?}")
        }
    }
}

fn presence_label(present: bool) -> &'static str {
    if present { "present" } else { "absent" }
}

fn provider_error_diagnostic(message: String) -> Diagnostic {
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        TextRange::point(1, 1),
        format!("Calls provider failed: {message}"),
    )
}

#[cfg(test)]
fn calls_output_digest_for_test(parts: &[&str]) -> Digest {
    Digest::from_parts(DigestKind::ProviderOutput, "calls_output", parts)
}

#[cfg(test)]
mod calls_provider {
    use crate::analysis_kernel::AnalysisKernel;

    #[test]
    fn calls_provider_accepts_empty_output_with_deterministic_digest() {
        let first = super::calls_output_digest_for_test(&[]);
        let second = super::calls_output_digest_for_test(&[]);

        assert_eq!(first, second);
        assert!(!first.value.is_empty());
    }

    #[test]
    fn calls_provider_manifest_declares_private_outputs() {
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.calls")
            .expect("calls manifest should exist");

        assert_eq!(manifest.primary_schema_label(), "calls-facts-1:1");
        assert!(manifest.outputs.contains(&"call_sites"));
        assert!(manifest.outputs.contains(&"call_targets"));
        assert!(manifest.outputs.contains(&"unresolved_calls"));
    }
}

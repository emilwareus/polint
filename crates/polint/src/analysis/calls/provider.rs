use std::collections::BTreeMap;

use crate::analysis::calls::cache_key::calls_provider_parameter_digest;
use crate::analysis::calls::extract::extract_call_sites;
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
    let output = CallOutput {
        sites: extract_call_sites(db),
        targets: Vec::new(),
        unresolved: Vec::new(),
    }
    .normalized();
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
        format!("parameters={}", calls_provider_parameter_digest()),
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
    use crate::analysis_kernel::incremental::{Digest, DigestKind, InputSnapshot};
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::load_config;
    use crate::core::{FileId, FunctionId, Language, ReferenceId, Span, SymbolId};
    use std::fs;
    use tempfile::tempdir;

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

    #[test]
    fn calls_output_digest_uses_stable_payloads_not_dense_ids() {
        let output = call_output(1, "resolved");
        let shifted_dense_ids = call_output(100, "resolved");
        let changed_status = call_output(1, "unresolved");

        assert_eq!(
            digest_for_output(&output),
            digest_for_output(&shifted_dense_ids)
        );
        assert_ne!(
            digest_for_output(&output),
            digest_for_output(&changed_status)
        );
    }

    fn digest_for_output(output: &crate::analysis::calls::store::CallOutput) -> Digest {
        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        let db = crate::core::AnalysisDb::new();
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
            .find(|manifest| manifest.id == "polint.calls")
            .expect("calls manifest");

        super::calls_output_digest(
            manifest,
            &snapshot,
            &Digest::from_parts(DigestKind::ProviderOutput, "semantic_mir", &["a"]),
            &Digest::from_parts(DigestKind::ProviderOutput, "cfg", &["a"]),
            &Digest::from_parts(DigestKind::ProviderOutput, "symbol_graph", &["a"]),
            &Digest::from_parts(DigestKind::ProviderOutput, "module_topology", &["a"]),
            &[],
            output,
        )
    }

    fn call_output(id_offset: u64, status: &str) -> crate::analysis::calls::store::CallOutput {
        use crate::analysis::calls::facts::{
            CallAlgorithm, CallCallee, CallEdgeKind, CallPrecision, CallProvenance, CallSiteFact,
            CallSyntaxKind, CallTargetFact, CallTargetStatus, UnresolvedCallFact,
            UnresolvedCallReason,
        };
        use crate::analysis::ids::{CallSiteId, CallTargetId, MirBodyId, MirOpId, PlaceId};

        let site = CallSiteFact {
            id: CallSiteId(id_offset),
            language: Language::TypeScript,
            file: FileId(1),
            caller: FunctionId(id_offset),
            owner_symbol: Some(SymbolId(id_offset)),
            body: MirBodyId(id_offset),
            operation: MirOpId(id_offset),
            span: Span::point(FileId(1), 10, 20),
            kind: CallSyntaxKind::Function,
            callee: CallCallee::Identifier {
                reference: Some(ReferenceId(id_offset)),
                name: "run".to_string(),
            },
            receiver: None,
            arguments: vec![PlaceId(id_offset)],
            result: Some(PlaceId(id_offset + 1)),
            status: CallTargetStatus::Resolved,
            precision: CallPrecision::Exact,
            stable_key: "call-site:stable".to_string(),
        };
        let target_status = if status == "resolved" {
            CallTargetStatus::Resolved
        } else {
            CallTargetStatus::Unresolved
        };
        let target = CallTargetFact {
            id: CallTargetId(id_offset),
            site: site.id,
            caller: site.caller,
            target_function: Some(FunctionId(id_offset + 10)),
            target_symbol: Some(SymbolId(id_offset + 20)),
            edge_kind: CallEdgeKind::Direct,
            algorithm: CallAlgorithm::DirectReference,
            status: target_status,
            reason: None,
            provenance: CallProvenance::Native,
            precision: CallPrecision::Exact,
            stable_key: "call-target:stable".to_string(),
        };
        let unresolved = UnresolvedCallFact {
            site: site.id,
            caller: site.caller,
            status: CallTargetStatus::Unresolved,
            reason: UnresolvedCallReason::FunctionValue,
            algorithm: CallAlgorithm::SyntaxOnly,
            provenance: CallProvenance::MirShape,
            precision: CallPrecision::Unknown,
            stable_key: "unresolved-call:stable".to_string(),
        };

        crate::analysis::calls::store::CallOutput {
            sites: vec![site],
            targets: vec![target],
            unresolved: vec![unresolved],
        }
        .normalized()
    }
}

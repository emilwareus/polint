use super::cache_key::abstract_domains_provider_parameter_digest;
use super::solver::{IdeDomainSolver, SolverInput, SolverPolicy};
use super::store::{DomainMaterialization, DomainOutput};
use crate::analysis::cfg::ids::BasicBlockId;
use crate::analysis::ids::{MirBodyId, MirOpId, PlaceId};
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{
    CacheStats, Digest, DigestKind, InputComponent, InputSnapshot,
};
use crate::core::AnalysisDb;
use crate::diagnostics::Diagnostic;

#[derive(Debug, Clone, Default)]
pub(crate) struct AbstractDomainsProviderOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_abstract_domains_with_cache_stats(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    semantic_mir_output_digest: Digest,
    cfg_output_digest: Digest,
    calls_output_digest: Digest,
    symbol_graph_output_digest: Digest,
    module_topology_output_digest: Digest,
    upstream_syntax_output_digests: Vec<Digest>,
) -> AbstractDomainsProviderOutput {
    derive_abstract_domains_with_materialization(
        db,
        input_snapshot,
        manifest,
        semantic_mir_output_digest,
        cfg_output_digest,
        calls_output_digest,
        symbol_graph_output_digest,
        module_topology_output_digest,
        upstream_syntax_output_digests,
        DomainMaterialization::Full,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_summary_input_abstract_domains_with_cache_stats(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    semantic_mir_output_digest: Digest,
    cfg_output_digest: Digest,
    calls_output_digest: Digest,
    symbol_graph_output_digest: Digest,
    module_topology_output_digest: Digest,
    upstream_syntax_output_digests: Vec<Digest>,
) -> AbstractDomainsProviderOutput {
    derive_abstract_domains_with_materialization(
        db,
        input_snapshot,
        manifest,
        semantic_mir_output_digest,
        cfg_output_digest,
        calls_output_digest,
        symbol_graph_output_digest,
        module_topology_output_digest,
        upstream_syntax_output_digests,
        DomainMaterialization::SummaryInputs,
    )
}

#[allow(clippy::too_many_arguments)]
fn derive_abstract_domains_with_materialization(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    semantic_mir_output_digest: Digest,
    cfg_output_digest: Digest,
    calls_output_digest: Digest,
    symbol_graph_output_digest: Digest,
    module_topology_output_digest: Digest,
    upstream_syntax_output_digests: Vec<Digest>,
    materialization: DomainMaterialization,
) -> AbstractDomainsProviderOutput {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    let solver = IdeDomainSolver::new(SolverPolicy::deterministic());
    let result = match materialization {
        DomainMaterialization::Full => solver.solve(SolverInput::from(&*db)),
        DomainMaterialization::SummaryInputs => {
            solver.solve_summary_inputs(SolverInput::from(&*db))
        }
    };
    let body_keys = body_stable_key_map(db);
    let block_keys = block_stable_key_map(db);
    let operation_keys = operation_stable_key_map(db);
    let place_keys = place_stable_key_map(db);
    let output = DomainOutput::from_results_with_materialization(
        interner,
        result.results(),
        Some(&place_keys),
        materialization,
    );
    let output_digest = abstract_domains_output_digest(
        manifest,
        input_snapshot,
        &semantic_mir_output_digest,
        &cfg_output_digest,
        &calls_output_digest,
        &symbol_graph_output_digest,
        &module_topology_output_digest,
        &upstream_syntax_output_digests,
        &body_keys,
        &block_keys,
        &operation_keys,
        &place_keys,
        &output,
        materialization,
    );
    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();
    db.replace_normalized_abstract_domain_facts(output);

    AbstractDomainsProviderOutput {
        diagnostics: Vec::new(),
        cache_stats,
        output_digest: Some(output_digest),
    }
}

#[allow(clippy::too_many_arguments)]
fn abstract_domains_output_digest(
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    semantic_mir_output_digest: &Digest,
    cfg_output_digest: &Digest,
    calls_output_digest: &Digest,
    symbol_graph_output_digest: &Digest,
    module_topology_output_digest: &Digest,
    upstream_syntax_output_digests: &[Digest],
    body_keys: &std::collections::BTreeMap<MirBodyId, String>,
    block_keys: &std::collections::BTreeMap<BasicBlockId, String>,
    operation_keys: &std::collections::BTreeMap<MirOpId, String>,
    place_keys: &std::collections::BTreeMap<PlaceId, String>,
    output: &DomainOutput,
    materialization: DomainMaterialization,
) -> Digest {
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!("materialization={}", materialization_label(materialization)),
        format!(
            "parameters={}",
            abstract_domains_provider_parameter_digest()
        ),
        format!("config={}", input_snapshot.config.digest),
        format!("semantic_mir={semantic_mir_output_digest}"),
        format!("cfg={cfg_output_digest}"),
        format!("calls={calls_output_digest}"),
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
    parts.extend(output.observations.iter().map(|row| {
        format!(
            "observation={} body={} block={} operation={} place={} slot={:?} location={:?} status={:?} precision={:?} value={:?}",
            row.stable_key,
            stable_body_key(body_keys, row.body),
            row.block
                .and_then(|block| block_keys.get(&block).cloned())
                .unwrap_or_else(|| "none".to_string()),
            row.operation
                .and_then(|operation| operation_keys.get(&operation).cloned())
                .unwrap_or_else(|| "none".to_string()),
            row.place
                .and_then(|place| place_keys.get(&place).cloned())
                .unwrap_or_else(|| "none".to_string()),
            row.slot,
            row.location,
            row.status,
            row.precision,
            row.value,
        )
    }));
    parts.extend(output.events.iter().map(|row| {
        format!(
            "event={} body={} block={} operation={} slot={:?} status={:?} precision={:?} reason={}",
            row.stable_key,
            stable_body_key(body_keys, row.body),
            row.block
                .and_then(|block| block_keys.get(&block).cloned())
                .unwrap_or_else(|| "none".to_string()),
            row.operation
                .and_then(|operation| operation_keys.get(&operation).cloned())
                .unwrap_or_else(|| "none".to_string()),
            row.slot,
            row.status,
            row.precision,
            row.reason,
        )
    }));
    if output.observations.is_empty() && output.events.is_empty() {
        parts.push("domain_output=empty".to_string());
    }

    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "abstract_domains_output", &refs)
}

fn materialization_label(materialization: DomainMaterialization) -> &'static str {
    match materialization {
        DomainMaterialization::Full => "full",
        DomainMaterialization::SummaryInputs => "summary_inputs",
    }
}

fn extend_component_parts(parts: &mut Vec<String>, prefix: &str, components: &[InputComponent]) {
    parts.extend(components.iter().map(|component| {
        format!(
            "{prefix}:{}:{:?}:{}",
            component.name, component.status, component.digest
        )
    }));
}

fn body_stable_key_map(db: &AnalysisDb) -> std::collections::BTreeMap<MirBodyId, String> {
    db.mir_bodies()
        .iter()
        .map(|body| (body.id, db.resolve_stable_key(body.stable_key).to_string()))
        .collect()
}

fn block_stable_key_map(db: &AnalysisDb) -> std::collections::BTreeMap<BasicBlockId, String> {
    db.cfg_blocks()
        .iter()
        .map(|block| (block.id, block.stable_key.clone()))
        .collect()
}

fn operation_stable_key_map(db: &AnalysisDb) -> std::collections::BTreeMap<MirOpId, String> {
    db.mir_operations()
        .iter()
        .map(|operation| {
            (
                operation.id,
                db.resolve_stable_key(operation.stable_key).to_string(),
            )
        })
        .collect()
}

fn place_stable_key_map(db: &AnalysisDb) -> std::collections::BTreeMap<PlaceId, String> {
    db.mir_places()
        .iter()
        .map(|place| {
            (
                place.id,
                db.resolve_stable_key(place.stable_key).to_string(),
            )
        })
        .collect()
}

fn stable_body_key(
    body_keys: &std::collections::BTreeMap<MirBodyId, String>,
    body: MirBodyId,
) -> String {
    body_keys
        .get(&body)
        .cloned()
        .unwrap_or_else(|| "missing-body".to_string())
}

#[cfg(test)]
mod abstract_domains_provider {
    use super::*;
    use crate::analysis_kernel::AnalysisKernel;
    use crate::analysis_kernel::incremental::{Digest, DigestKind, InputSnapshot};
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::load_config;
    use crate::core::AnalysisDb;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn abstract_domains_provider_accepts_empty_output_with_deterministic_digest() {
        let mut db = AnalysisDb::new();
        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        let input_snapshot = InputSnapshot::from_run_inputs(
            &loaded,
            &db,
            "config",
            "rules",
            AnalysisPlan::empty().digest(),
            AnalysisKernel::provider_manifests(),
        );
        let output = derive_abstract_domains_with_cache_stats(
            &mut db,
            &input_snapshot,
            AnalysisKernel::provider_manifests()
                .iter()
                .find(|manifest| manifest.id == "polint.abstract_domains")
                .expect("abstract domains manifest should exist"),
            Digest::absent(DigestKind::ProviderOutput, "semantic_mir"),
            Digest::absent(DigestKind::ProviderOutput, "cfg"),
            Digest::absent(DigestKind::ProviderOutput, "calls"),
            Digest::absent(DigestKind::ProviderOutput, "symbol_graph"),
            Digest::absent(DigestKind::ProviderOutput, "module_topology"),
            Vec::new(),
        );

        assert!(output.diagnostics.is_empty());
        assert!(output.output_digest.is_some());
        assert_eq!(output.cache_stats.recomputes, 1);
    }

    #[test]
    fn abstract_domains_provider_manifest_declares_private_outputs() {
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.abstract_domains")
            .expect("abstract domains manifest should exist");

        assert_eq!(manifest.primary_schema_label(), "abstract-domain-facts-1:1");
        assert!(manifest.outputs.contains(&"domain_observations"));
        assert!(manifest.outputs.contains(&"domain_events"));
    }
}

#[cfg(test)]
mod kernel_run_report_abstract_domains_row_carries_output_digest {
    use crate::analysis_kernel::AnalysisKernel;

    #[test]
    fn abstract_domains_runs_after_calls_and_before_metrics() {
        let order = AnalysisKernel::provider_manifests()
            .iter()
            .map(|manifest| manifest.id)
            .collect::<Vec<_>>();
        let calls = order
            .iter()
            .position(|provider| *provider == "polint.calls")
            .expect("calls provider");
        let domains = order
            .iter()
            .position(|provider| *provider == "polint.abstract_domains")
            .expect("abstract domains provider");
        let metrics = order
            .iter()
            .position(|provider| *provider == "polint.metrics")
            .expect("metrics provider");

        assert!(calls < domains);
        assert!(domains < metrics);
    }
}

#[cfg(test)]
mod abstract_domains_layer_key {
    use crate::analysis::domains::cache_key::abstract_domains_provider_parameter_digest;
    use crate::analysis_kernel::AnalysisKernel;
    use crate::analysis_kernel::incremental::{Digest, DigestKind, LayerKey, LayerKind};

    #[test]
    fn abstract_domains_layer_key_records_upstream_and_absent_future_inputs() {
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.abstract_domains")
            .expect("abstract domains manifest should exist");
        let key = LayerKey::abstract_domains_layer_key(
            manifest,
            Vec::new(),
            Digest::from_parts(DigestKind::Config, "config", &["a"]),
            Digest::absent(DigestKind::ProviderParameters, "go_lifecycle"),
            Digest::absent(DigestKind::ProviderParameters, "ts_lifecycle"),
            Vec::new(),
            Digest::absent(DigestKind::ProviderOutput, "semantic_mir"),
            Digest::absent(DigestKind::ProviderOutput, "cfg"),
            Digest::absent(DigestKind::ProviderOutput, "calls"),
            Digest::absent(DigestKind::ProviderOutput, "symbol_graph"),
            Digest::absent(DigestKind::ProviderOutput, "module_topology"),
            abstract_domains_provider_parameter_digest(),
        );

        assert_eq!(key.layer_kind, LayerKind::AbstractDomains);
        assert!(key.extension_digests.len() >= 2);
    }
}

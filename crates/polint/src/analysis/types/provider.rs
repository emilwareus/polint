use super::cache_key::{
    type_value_alias_provider_parameter_digest,
    type_value_alias_provider_parameter_digest_for_snapshot,
};
use super::store::TypeValueAliasOutput;
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{
    CacheStats, Digest, DigestKind, InputComponent, InputSnapshot,
};
use crate::core::AnalysisDb;
use crate::diagnostics::Diagnostic;
use serde::Serialize;
use std::fmt::Debug;

pub(crate) const TYPE_VALUE_ALIAS_PROVIDER_ID: &str = "polint.type_value_alias";

#[derive(Debug, Clone, Default)]
pub(crate) struct TypeValueAliasProviderOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_type_value_alias_with_cache_stats(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    semantic_mir_output_digest: Digest,
    cfg_output_digest: Digest,
    calls_output_digest: Digest,
    abstract_domains_output_digest: Digest,
    direct_summaries_output_digest: Digest,
    entrypoints_output_digest: Digest,
    symbol_graph_output_digest: Digest,
    module_topology_output_digest: Digest,
    upstream_syntax_output_digests: Vec<Digest>,
) -> TypeValueAliasProviderOutput {
    debug_assert_eq!(manifest.id, TYPE_VALUE_ALIAS_PROVIDER_ID);
    let mut output = super::go::derive_go_type_value_alias(db).normalized();
    let ts_js_output = super::ts_js::derive_ts_js_type_value_alias(db).normalized();
    output.types.types.extend(ts_js_output.types.types);
    output.types.narrowed.extend(ts_js_output.types.narrowed);
    output.values.values.extend(ts_js_output.values.values);
    output
        .values
        .allocations
        .extend(ts_js_output.values.allocations);
    output
        .access_paths
        .access_paths
        .extend(ts_js_output.access_paths.access_paths);
    output = output.normalized();
    let output_digest = type_value_alias_output_digest(
        manifest,
        input_snapshot,
        &semantic_mir_output_digest,
        &cfg_output_digest,
        &calls_output_digest,
        &abstract_domains_output_digest,
        &direct_summaries_output_digest,
        &entrypoints_output_digest,
        &symbol_graph_output_digest,
        &module_topology_output_digest,
        &upstream_syntax_output_digests,
        &output,
    );
    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();

    db.replace_type_value_alias_facts(output);
    TypeValueAliasProviderOutput {
        diagnostics: Vec::new(),
        cache_stats,
        output_digest: Some(output_digest),
    }
}

#[allow(clippy::too_many_arguments)]
fn type_value_alias_output_digest(
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    semantic_mir_output_digest: &Digest,
    cfg_output_digest: &Digest,
    calls_output_digest: &Digest,
    abstract_domains_output_digest: &Digest,
    direct_summaries_output_digest: &Digest,
    entrypoints_output_digest: &Digest,
    symbol_graph_output_digest: &Digest,
    module_topology_output_digest: &Digest,
    upstream_syntax_output_digests: &[Digest],
    output: &TypeValueAliasOutput,
) -> Digest {
    let mut upstream = vec![
        semantic_mir_output_digest.clone(),
        cfg_output_digest.clone(),
        calls_output_digest.clone(),
        abstract_domains_output_digest.clone(),
        direct_summaries_output_digest.clone(),
        entrypoints_output_digest.clone(),
        symbol_graph_output_digest.clone(),
        module_topology_output_digest.clone(),
    ];
    upstream.extend(upstream_syntax_output_digests.iter().cloned());

    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!(
            "parameters={}",
            type_value_alias_provider_parameter_digest()
        ),
        format!(
            "input_parameters={}",
            type_value_alias_provider_parameter_digest_for_snapshot(input_snapshot, &upstream)
        ),
        format!("config={}", input_snapshot.config.digest),
        format!("semantic_mir={semantic_mir_output_digest}"),
        format!("cfg={cfg_output_digest}"),
        format!("calls={calls_output_digest}"),
        format!("abstract_domains={abstract_domains_output_digest}"),
        format!("direct_summaries={direct_summaries_output_digest}"),
        format!("entrypoints={entrypoints_output_digest}"),
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
    parts.extend(
        output
            .types
            .types
            .iter()
            .map(|row| format!("type={}", stable_fact_payload(row))),
    );
    parts.extend(
        output
            .types
            .narrowed
            .iter()
            .map(|row| format!("narrowed_type={}", stable_fact_payload(row))),
    );
    parts.extend(
        output
            .values
            .values
            .iter()
            .map(|row| format!("value={}", stable_fact_payload(row))),
    );
    parts.extend(
        output
            .values
            .allocations
            .iter()
            .map(|row| format!("allocation={}", stable_fact_payload(row))),
    );
    parts.extend(
        output
            .access_paths
            .access_paths
            .iter()
            .map(|row| format!("access_path={}", stable_fact_payload(row))),
    );
    parts.extend(
        output
            .points_to
            .constraints
            .iter()
            .map(|row| format!("points_to_constraint={}", stable_fact_payload(row))),
    );
    parts.extend(
        output
            .points_to
            .sets
            .iter()
            .map(|row| format!("points_to_set={}", stable_fact_payload(row))),
    );
    parts.extend(
        output
            .aliases
            .answers
            .iter()
            .map(|row| format!("alias_answer={}", stable_fact_payload(row))),
    );
    if output.types.types.is_empty()
        && output.types.narrowed.is_empty()
        && output.values.values.is_empty()
        && output.values.allocations.is_empty()
        && output.access_paths.access_paths.is_empty()
        && output.points_to.constraints.is_empty()
        && output.points_to.sets.is_empty()
        && output.aliases.answers.is_empty()
    {
        parts.push("type_value_alias_output=empty".to_string());
    }

    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "type_value_alias_output", &refs)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::AnalysisKernel;
    use crate::analysis_kernel::incremental::{Digest, DigestKind, InputSnapshot};
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::load_config;
    use tempfile::tempdir;

    #[test]
    fn type_value_alias_provider_accepts_empty_output_with_deterministic_digest() {
        let mut first_db = AnalysisDb::new();
        let mut second_db = AnalysisDb::new();
        let first = derive_for_test(&mut first_db);
        let second = derive_for_test(&mut second_db);

        assert_eq!(first.output_digest, second.output_digest);
        assert!(first_db.type_facts().is_empty());
        assert!(first_db.alias_answers().is_empty());
        assert_eq!(first.cache_stats.recomputes, 1);
        assert!(first.diagnostics.is_empty());
    }

    #[test]
    fn type_value_alias_provider_manifest_declares_private_outputs() {
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == TYPE_VALUE_ALIAS_PROVIDER_ID)
            .expect("type/value/alias manifest should exist");

        assert_eq!(
            manifest.primary_schema_label(),
            "type-value-alias-facts-1:1"
        );
        for output in [
            "type_facts",
            "narrowed_type_facts",
            "value_facts",
            "allocation_tokens",
            "access_paths",
            "points_to_constraints",
            "points_to_sets",
            "alias_answers",
        ] {
            assert!(manifest.outputs.contains(&output));
        }
    }

    #[test]
    fn type_value_alias_metadata_uses_provider_identity() {
        use crate::analysis::ids::{TypeFactId, TypeSetId};
        use crate::analysis::types::facts::{
            TypeConfidence, TypeFact, TypePhase, TypePrecision, TypeProvenance, TypeShape,
            TypeStatus, TypeSubject,
        };
        use crate::analysis::types::store::{TypeOutput, TypeValueAliasOutput};
        use crate::analysis_kernel::{FactFamily, FactRef};
        use crate::core::Language;

        let mut db = AnalysisDb::new();
        db.replace_type_value_alias_facts(TypeValueAliasOutput {
            types: TypeOutput {
                types: vec![TypeFact {
                    id: TypeFactId(7),
                    subject: TypeSubject::Synthetic("type:seed".to_string()),
                    type_set: TypeSetId(0),
                    shape: TypeShape::Unknown {
                        reason: "seed".to_string(),
                    },
                    phase: TypePhase::Unknown,
                    language: Language::Unknown,
                    file: None,
                    function: None,
                    body: None,
                    place: None,
                    cfg_block: None,
                    operation: None,
                    precision: TypePrecision::Unknown,
                    confidence: TypeConfidence::Low,
                    status: TypeStatus::Unknown,
                    provenance: TypeProvenance::Native,
                    stable_key: "type:seed".to_string(),
                }],
                narrowed: Vec::new(),
            },
            ..TypeValueAliasOutput::default()
        });

        let metadata = db
            .metadata_for(FactRef::new(FactFamily::Type, 0))
            .expect("type metadata");
        assert_eq!(metadata.producer_id, TYPE_VALUE_ALIAS_PROVIDER_ID);
        assert_eq!(metadata.layer_id, TYPE_VALUE_ALIAS_PROVIDER_ID);
    }

    fn derive_for_test(db: &mut AnalysisDb) -> TypeValueAliasProviderOutput {
        let temp = tempdir().expect("tempdir");
        std::fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        let input_snapshot = InputSnapshot::from_run_inputs(
            &loaded,
            db,
            "config",
            "rules",
            AnalysisPlan::empty().digest(),
            AnalysisKernel::provider_manifests(),
        );
        let absent = |label| Digest::absent(DigestKind::ProviderOutput, label);

        derive_type_value_alias_with_cache_stats(
            db,
            &input_snapshot,
            AnalysisKernel::provider_manifests()
                .iter()
                .find(|manifest| manifest.id == TYPE_VALUE_ALIAS_PROVIDER_ID)
                .expect("type/value/alias manifest should exist"),
            absent("semantic_mir"),
            absent("cfg"),
            absent("calls"),
            absent("abstract_domains"),
            absent("direct_summaries"),
            absent("entrypoints"),
            absent("symbol_graph"),
            absent("module_topology"),
            Vec::new(),
        )
    }
}

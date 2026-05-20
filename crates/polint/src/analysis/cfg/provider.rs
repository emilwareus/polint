use crate::analysis::cfg::derived::{
    derive_control_dependence, derive_dominators, derive_postdominators, derive_reachability,
};
use crate::analysis::cfg::facts::CfgView;
use crate::analysis::cfg::store::CfgOutput;
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{
    CacheStats, Digest, DigestKind, InputComponent, InputSnapshot,
};
use crate::core::AnalysisDb;
use crate::diagnostics::{Diagnostic, TextRange};

#[derive(Debug, Clone, Default)]
pub(crate) struct CfgProviderOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}

pub(crate) fn derive_cfg_with_cache_stats(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    semantic_mir_output_digest: Digest,
    upstream_syntax_output_digests: Vec<Digest>,
) -> CfgProviderOutput {
    let mut output = derive_cfg_output(db).normalized();
    append_derived_rows(&mut output, CfgView::NormalControl);
    let output = output.normalized();
    let output_digest = cfg_output_digest(
        manifest,
        input_snapshot,
        &semantic_mir_output_digest,
        &upstream_syntax_output_digests,
        &output,
    );
    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();

    match db.replace_cfg_facts(output) {
        Ok(()) => CfgProviderOutput {
            diagnostics: Vec::new(),
            cache_stats,
            output_digest: Some(output_digest),
        },
        Err(error) => CfgProviderOutput {
            diagnostics: vec![provider_error_diagnostic(error.to_string())],
            cache_stats,
            output_digest: Some(output_digest),
        },
    }
}

fn derive_cfg_output(_db: &AnalysisDb) -> CfgOutput {
    CfgOutput::empty()
}

fn append_derived_rows(output: &mut CfgOutput, view: CfgView) {
    if output.functions.is_empty() {
        return;
    }
    output.reachability = derive_reachability(output, view);
    output.dominators = derive_dominators(output, view);
    output.postdominators = derive_postdominators(output, view);
    output.control_dependence = derive_control_dependence(output, view);
}

fn cfg_output_digest(
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    semantic_mir_output_digest: &Digest,
    upstream_syntax_output_digests: &[Digest],
    output: &CfgOutput,
) -> Digest {
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!("config={}", input_snapshot.config.digest),
        format!("semantic_mir={semantic_mir_output_digest}"),
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
            .functions
            .iter()
            .map(|row| format!("cfg_function={}", row.stable_key)),
    );
    parts.extend(
        output
            .nodes
            .iter()
            .map(|row| format!("cfg_node={}", row.stable_key)),
    );
    parts.extend(
        output
            .blocks
            .iter()
            .map(|row| format!("basic_block={}", row.stable_key)),
    );
    parts.extend(output.edges.iter().map(|row| {
        format!(
            "cfg_edge={} kind={:?} view={:?}",
            row.stable_key, row.kind, row.view
        )
    }));
    parts.extend(
        output
            .reachability
            .iter()
            .map(|row| format!("cfg_reachability={}", row.stable_key)),
    );
    parts.extend(
        output
            .dominators
            .iter()
            .map(|row| format!("cfg_dominator={}", row.stable_key)),
    );
    parts.extend(
        output
            .postdominators
            .iter()
            .map(|row| format!("cfg_postdominator={}", row.stable_key)),
    );
    parts.extend(
        output
            .control_dependence
            .iter()
            .map(|row| format!("cfg_control_dependence={}", row.stable_key)),
    );
    parts.extend(
        output
            .unsupported
            .iter()
            .map(|row| format!("unsupported_control_flow={}", row.stable_key)),
    );

    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "cfg_output", &refs)
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
        format!("CFG provider failed: {message}"),
    )
}

#[cfg(test)]
mod cfg_provider {
    use super::*;
    use crate::analysis_kernel::AnalysisKernel;
    use crate::analysis_kernel::incremental::{Digest, DigestKind, InputSnapshot};
    use crate::analysis_plan::AnalysisPlan;
    use crate::cache::Cache;
    use crate::config::load_config;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn cfg_provider_accepts_empty_output_with_deterministic_digest() {
        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        let cache = Cache::new(temp.path().join(".polint/cache"), false);
        let plan = AnalysisPlan::empty();
        let first = AnalysisKernel::run(crate::analysis_kernel::KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "cfg-provider-config",
            rule_digest: "cfg-provider-rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel run");
        let second = AnalysisKernel::run(crate::analysis_kernel::KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "cfg-provider-config",
            rule_digest: "cfg-provider-rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel run");

        let first_cfg = AnalysisKernel::provider_output_report_for_test(&first)
            .into_iter()
            .find(|row| row.provider_id == "polint.cfg")
            .expect("cfg provider row");
        let second_cfg = AnalysisKernel::provider_output_report_for_test(&second)
            .into_iter()
            .find(|row| row.provider_id == "polint.cfg")
            .expect("cfg provider row");

        assert_eq!(first_cfg.output_digest, second_cfg.output_digest);
        assert_eq!(first_cfg.schema_version, "cfg-facts-1:1");
    }

    #[test]
    fn cfg_output_digest_changes_when_semantic_mir_digest_changes() {
        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        let db = AnalysisDb::new();
        let snapshot = InputSnapshot::from_run_inputs(
            &loaded,
            &db,
            "config-a",
            "rules-a",
            "plan-a",
            AnalysisKernel::provider_manifests(),
        );
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.cfg")
            .expect("cfg manifest");

        let first = cfg_output_digest(
            manifest,
            &snapshot,
            &Digest::from_parts(DigestKind::ProviderOutput, "mir", &["a"]),
            &[],
            &CfgOutput::empty(),
        );
        let second = cfg_output_digest(
            manifest,
            &snapshot,
            &Digest::from_parts(DigestKind::ProviderOutput, "mir", &["b"]),
            &[],
            &CfgOutput::empty(),
        );

        assert_ne!(first, second);
    }
}

#[cfg(test)]
use std::collections::BTreeMap;
#[cfg(test)]
use std::fs;
#[cfg(test)]
use std::path::Path;
#[cfg(test)]
use std::time::{Duration, Instant};

#[cfg(test)]
use anyhow::{Context, bail};
#[cfg(test)]
use serde_json::Value;

#[cfg(test)]
use crate::analysis_kernel::incremental::{
    CacheStats, InputComponent, InputComponentStatus, KernelRunReport,
};
#[cfg(test)]
use crate::analysis_kernel::{AnalysisKernel, KernelInput};
#[cfg(test)]
use crate::analysis_plan::AnalysisPlan;
#[cfg(test)]
use crate::cache::Cache;
#[cfg(test)]
use crate::cache::keys::{config_hash, rule_hash};
#[cfg(test)]
use crate::config::load_config;
#[cfg(test)]
use crate::core::AnalysisDb;
#[cfg(test)]
use crate::eval::fixtures::NativeFixture;
#[cfg(test)]
use crate::eval::model::{
    AssertionMode, ExpectedItem, ObservedDiagnostic, ObservedFact, ObservedInvariant, ObservedItem,
    ObservedRuntimeBudget, ObservedStatus,
};
#[cfg(test)]
use crate::module_graph::topology::{ImportToPackageStatus, TopologyPrecision, TopologyStatus};

#[cfg(test)]
const SEMANTIC_DEBUG_SECTIONS: &[&str] = &[
    "scopes",
    "imports",
    "exports",
    "aliases",
    "resolutions",
    "generated_symbols",
    "stable_exports",
];

#[cfg(test)]
const SEMANTIC_MIR_DEBUG_SECTIONS: &[&str] = &["bodies", "operations", "places", "unsupported"];

#[cfg(test)]
pub(crate) fn observe_kernel_fixture(fixture: &NativeFixture) -> anyhow::Result<Vec<ObservedItem>> {
    let temp = copy_fixture_repo_for_test(fixture)?;
    observe_kernel_fixture_repo_for_test(fixture, temp.path(), true)
}

#[cfg(test)]
pub(crate) fn copy_fixture_repo_for_test(
    fixture: &NativeFixture,
) -> anyhow::Result<tempfile::TempDir> {
    let temp = tempfile::tempdir().context("create temp fixture repo")?;
    copy_dir_contents(&fixture.repo_dir, temp.path())?;
    Ok(temp)
}

#[cfg(test)]
pub(crate) fn observe_kernel_fixture_repo_for_test(
    fixture: &NativeFixture,
    repo_root: &Path,
    cache_enabled: bool,
) -> anyhow::Result<Vec<ObservedItem>> {
    let plan = AnalysisPlan::empty();
    observe_kernel_fixture_repo_with_plan_for_test(fixture, repo_root, cache_enabled, &plan)
}

#[cfg(test)]
pub(crate) fn observe_kernel_fixture_repo_with_plan_for_test(
    fixture: &NativeFixture,
    repo_root: &Path,
    cache_enabled: bool,
    plan: &AnalysisPlan,
) -> anyhow::Result<Vec<ObservedItem>> {
    let started = Instant::now();
    let loaded = load_config(repo_root)?;
    let config_digest = config_hash(&loaded);
    let rule_digest = rule_hash(&[], None, &BTreeMap::new());
    let cache = Cache::default_for_repo(repo_root, cache_enabled);
    let output = AnalysisKernel::run(KernelInput {
        loaded: &loaded,
        cache: &cache,
        config_digest: &config_digest,
        rule_digest: &rule_digest,
        plan,
        parallel: true,
    })?;
    let elapsed = started.elapsed();

    let mut observed = Vec::new();
    observed.extend(observed_diagnostics(&output.diagnostics, repo_root));
    observed.extend(provider_order_invariants());
    observed.extend(metadata_debug_facts(
        &AnalysisKernel::metadata_debug_json_for_test(&output.db),
    ));
    observed.extend(topology_facts(&output.db));
    observed.extend(snapshot_invariants(&output.run_report));
    observed.extend(layer_key_invariants(&output.run_report));
    observed.extend(provider_output_invariants(&output.run_report));
    observed.extend(layer_cache_invariants(&output.run_report));
    if let Some(budget) = &fixture.manifest.budget {
        observed.push(ObservedItem::RuntimeBudget(ObservedRuntimeBudget {
            name: runtime_budget_name(&fixture.manifest.expected, &fixture.manifest.case_id),
            budget_passed: elapsed <= Duration::from_millis(budget.max_runtime_ms),
            observed_runtime_ms: Some(saturating_millis(elapsed)),
        }));
    }
    observed.sort_by_key(observed_sort_key);

    Ok(observed)
}

#[cfg(test)]
fn copy_dir_contents(source: &Path, destination: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(destination)
        .with_context(|| format!("create fixture copy destination {}", destination.display()))?;
    let mut entries = fs::read_dir(source)
        .with_context(|| format!("read fixture repo {}", source.display()))?
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            bail!(
                "fixture repos must not contain symlinks: {}",
                source_path.display()
            );
        }
        if file_type.is_dir() {
            copy_dir_contents(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            if let Some(parent) = destination_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&source_path, &destination_path).with_context(|| {
                format!(
                    "copy fixture file {} to {}",
                    source_path.display(),
                    destination_path.display()
                )
            })?;
        }
    }
    Ok(())
}

#[cfg(test)]
fn observed_diagnostics(
    diagnostics: &[crate::diagnostics::Diagnostic],
    repo_root: &Path,
) -> Vec<ObservedItem> {
    diagnostics
        .iter()
        .map(|diagnostic| {
            ObservedItem::Diagnostic(ObservedDiagnostic {
                rule_id: diagnostic.rule_id.clone(),
                relative_path: normalize_observed_path(&diagnostic.file, repo_root),
                line: Some(diagnostic.range.start_line),
                fingerprint: Some(diagnostic.stable_fingerprint.clone()),
                mode: AssertionMode::Exact,
                producer_id: Some("polint.eval".to_string()),
                provenance: Some("kernel.diagnostics".to_string()),
                precision: Some("exact".to_string()),
                status: Some(ObservedStatus::Present),
            })
        })
        .collect()
}

#[cfg(test)]
fn provider_order_invariants() -> Vec<ObservedItem> {
    AnalysisKernel::provider_manifests()
        .iter()
        .enumerate()
        .map(|(index, manifest)| {
            ObservedItem::Invariant(ObservedInvariant {
                name: format!("provider_order.{index}"),
                value: manifest.id.to_string(),
                mode: AssertionMode::Exact,
                producer_id: Some("polint.eval".to_string()),
                provenance: Some("kernel.provider_manifests".to_string()),
                precision: Some("exact".to_string()),
                status: Some(ObservedStatus::Present),
            })
        })
        .collect()
}

#[cfg(test)]
fn snapshot_invariants(run_report: &KernelRunReport) -> Vec<ObservedItem> {
    let snapshot = &run_report.input_snapshot;

    vec![
        observed_invariant(
            "snapshot.schema_version",
            snapshot.schema_version.as_str(),
            "kernel.run_report.input_snapshot",
        ),
        observed_invariant(
            "snapshot.source_text_digest.present",
            bool_string(
                !snapshot.files.is_empty()
                    && snapshot
                        .files
                        .iter()
                        .all(|file| !file.source_text_digest.value.is_empty()),
            ),
            "kernel.run_report.input_snapshot",
        ),
        observed_invariant(
            "snapshot.config_digest.present",
            bool_string(component_present(&snapshot.config)),
            "kernel.run_report.input_snapshot",
        ),
        observed_invariant(
            "snapshot.go_lifecycle.present",
            bool_string(any_component_present(&snapshot.go_lifecycle.components)),
            "kernel.run_report.input_snapshot",
        ),
        observed_invariant(
            "snapshot.ts_js_lifecycle.present",
            bool_string(any_component_present(&snapshot.ts_js_lifecycle.components)),
            "kernel.run_report.input_snapshot",
        ),
        observed_invariant(
            "snapshot.rule_digest.present",
            bool_string(snapshot.rules.iter().any(component_present)),
            "kernel.run_report.input_snapshot",
        ),
        observed_invariant(
            "snapshot.model_digest.absent",
            bool_string(
                snapshot
                    .models
                    .iter()
                    .any(|component| component.status == InputComponentStatus::Absent),
            ),
            "kernel.run_report.input_snapshot",
        ),
        observed_invariant(
            "snapshot.tool_invocation.unsupported",
            bool_string(
                snapshot
                    .tool_invocations
                    .iter()
                    .any(|component| component.status == InputComponentStatus::Unsupported),
            ),
            "kernel.run_report.input_snapshot",
        ),
    ]
}

#[cfg(test)]
fn layer_key_invariants(run_report: &KernelRunReport) -> Vec<ObservedItem> {
    let has_ts_output = run_report
        .provider_outputs
        .iter()
        .any(|output| output.provider_id == "polint.ts.syntax");
    vec![observed_invariant(
        "layer_key.polint.ts.syntax.has_config_digest",
        bool_string(has_ts_output && component_present(&run_report.input_snapshot.config)),
        "kernel.run_report.layer_key_inputs",
    )]
}

#[cfg(test)]
fn provider_output_invariants(run_report: &KernelRunReport) -> Vec<ObservedItem> {
    let mut invariants = Vec::new();
    let source_present = run_report
        .provider_outputs
        .iter()
        .any(|output| output.provider_id == "polint.source");
    invariants.push(observed_invariant(
        "provider_output.polint.source.present",
        bool_string(source_present),
        "kernel.run_report.provider_outputs",
    ));

    for output in &run_report.provider_outputs {
        if output.provider_id == "polint.abstract_domains" {
            let prefix = "provider_output.polint.abstract_domains";
            invariants.push(observed_invariant(
                format!("{prefix}.present"),
                "true",
                "kernel.run_report.provider_outputs",
            ));
            invariants.push(observed_invariant(
                format!("{prefix}.schema_version"),
                output.schema_version.as_str(),
                "kernel.run_report.provider_outputs",
            ));
            invariants.push(observed_invariant(
                format!("{prefix}.validation"),
                output.validation.as_str(),
                "kernel.run_report.provider_outputs",
            ));
            invariants.push(observed_invariant(
                format!("{prefix}.dependency_inputs.count"),
                output.dependency_inputs.len().to_string(),
                "kernel.run_report.provider_outputs",
            ));
        }
        if !matches!(
            output.provider_id.as_str(),
            "polint.go.syntax" | "polint.ts.syntax"
        ) {
            continue;
        }
        let prefix = format!("provider_output.{}", output.provider_id);
        invariants.push(observed_invariant(
            format!("{prefix}.cache_stats.recorded"),
            "true",
            "kernel.run_report.provider_outputs",
        ));
        invariants.extend(cache_stat_invariants(&prefix, &output.cache_stats));
    }

    invariants
}

#[cfg(test)]
fn layer_cache_invariants(run_report: &KernelRunReport) -> Vec<ObservedItem> {
    let mut invariants = Vec::new();
    for output in &run_report.provider_outputs {
        if !is_layer_cache_provider(&output.provider_id) {
            continue;
        }
        let prefix = format!("layer_cache.provider.{}", output.provider_id);
        invariants.extend(layer_cache_stat_invariants(
            &prefix,
            &output.cache_stats,
            "kernel.run_report.layer_cache.provider_outputs",
        ));
    }
    invariants.extend(layer_cache_stat_invariants(
        "layer_cache.aggregate",
        &run_report.cache_stats,
        "kernel.run_report.layer_cache.aggregate",
    ));
    invariants
}

#[cfg(test)]
fn is_layer_cache_provider(provider_id: &str) -> bool {
    matches!(
        provider_id,
        "polint.go.syntax"
            | "polint.ts.syntax"
            | "polint.module_graph"
            | "polint.symbol_graph"
            | "polint.module_topology"
            | "polint.cfg"
            | "polint.metrics"
    )
}

#[cfg(test)]
fn layer_cache_stat_invariants(
    prefix: &str,
    stats: &CacheStats,
    provenance: &'static str,
) -> Vec<ObservedItem> {
    [
        ("hits", stats.hits),
        ("misses", stats.misses),
        ("recomputes", stats.recomputes),
        ("writes", stats.writes),
        ("bypasses_disabled", stats.bypasses_disabled),
        ("invalid_evicted_reads", stats.invalid_evicted_reads),
        ("verified_reuse", stats.verified_reuse),
    ]
    .into_iter()
    .map(|(counter, value)| {
        observed_invariant(format!("{prefix}.{counter}"), value.to_string(), provenance)
    })
    .collect()
}

#[cfg(test)]
fn cache_stat_invariants(prefix: &str, stats: &CacheStats) -> Vec<ObservedItem> {
    [
        ("cache_stats.hits", stats.hits),
        ("cache_stats.misses", stats.misses),
        ("cache_stats.recomputes", stats.recomputes),
        ("cache_stats.writes", stats.writes),
        ("cache_stats.bypasses_disabled", stats.bypasses_disabled),
        (
            "cache_stats.invalid_evicted_reads",
            stats.invalid_evicted_reads,
        ),
        ("cache_stats.verified_reuse", stats.verified_reuse),
        ("cache_stats.quarantines", stats.quarantines),
    ]
    .into_iter()
    .map(|(counter, value)| {
        observed_invariant(
            format!("{prefix}.{counter}"),
            value.to_string(),
            "kernel.run_report.provider_outputs",
        )
    })
    .collect()
}

#[cfg(test)]
fn any_component_present(components: &[InputComponent]) -> bool {
    components.iter().any(component_present)
}

#[cfg(test)]
fn component_present(component: &InputComponent) -> bool {
    component.status == InputComponentStatus::Present && !component.digest.value.is_empty()
}

#[cfg(test)]
fn observed_invariant(
    name: impl Into<String>,
    value: impl Into<String>,
    provenance: &'static str,
) -> ObservedItem {
    ObservedItem::Invariant(ObservedInvariant {
        name: name.into(),
        value: value.into(),
        mode: AssertionMode::Exact,
        producer_id: Some("polint.eval".to_string()),
        provenance: Some(provenance.to_string()),
        precision: Some("exact".to_string()),
        status: Some(ObservedStatus::Present),
    })
}

#[cfg(test)]
fn bool_string(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

#[cfg(test)]
pub(crate) fn metadata_debug_facts_for_test(debug_json: &Value) -> Vec<ObservedItem> {
    metadata_debug_facts(debug_json)
}

#[cfg(test)]
pub(crate) fn semantic_mir_facts_for_test(debug_json: &Value) -> Vec<ObservedItem> {
    semantic_mir_facts(debug_json)
}

#[cfg(test)]
pub(crate) fn cfg_facts_for_test(debug_json: &Value) -> Vec<ObservedItem> {
    cfg_facts(debug_json)
}

#[cfg(test)]
pub(crate) fn call_facts_for_test(debug_json: &Value) -> Vec<ObservedItem> {
    call_facts(debug_json)
}

#[cfg(test)]
pub(crate) fn abstract_domain_facts_for_test(debug_json: &Value) -> Vec<ObservedItem> {
    abstract_domain_facts(debug_json)
}

#[cfg(test)]
pub(crate) fn topology_facts_for_test(db: &AnalysisDb) -> Vec<ObservedItem> {
    topology_facts(db)
}

#[cfg(test)]
fn topology_facts(db: &AnalysisDb) -> Vec<ObservedItem> {
    let mut facts = Vec::new();

    facts.extend(db.workspace_roots().iter().map(|row| {
        topology_fact(
            "WorkspaceRoot",
            row.stable_key.as_str(),
            row.producer_id,
            row.precision,
            topology_status(row.status),
            format!(
                "kind:{:?};root:{};manifest:{}",
                row.kind,
                row.root_path,
                row.manifest_path.as_deref().unwrap_or("")
            ),
        )
    }));
    facts.extend(db.topology_packages().iter().map(|row| {
        topology_fact(
            "TopologyPackage",
            row.stable_key.as_str(),
            row.producer_id,
            row.precision,
            topology_status(row.status),
            format!(
                "kind:{:?};name:{};path:{};root:{:?}",
                row.kind, row.name, row.path, row.workspace_root
            ),
        )
    }));
    facts.extend(db.source_sets().iter().map(|row| {
        topology_fact(
            "SourceSet",
            row.stable_key.as_str(),
            row.producer_id,
            row.precision,
            topology_status(row.status),
            format!(
                "kind:{:?};path:{};package:{:?};root:{:?}",
                row.kind, row.path, row.package, row.root
            ),
        )
    }));
    facts.extend(db.dependency_requirements().iter().map(|row| {
        topology_fact(
            "DependencyRequirement",
            row.stable_key.as_str(),
            row.producer_id,
            row.precision,
            topology_status(row.status),
            format!(
                "target:{};kind:{:?};from:{:?};to:{:?};manifest:{}",
                row.target_name,
                row.kind,
                row.from_package,
                row.target_package,
                row.manifest_path.as_deref().unwrap_or("")
            ),
        )
    }));
    facts.extend(db.resolved_dependency_edges().iter().map(|row| {
        topology_fact(
            "ResolvedDependencyEdge",
            row.stable_key.as_str(),
            row.producer_id,
            row.precision,
            topology_status(row.status),
            format!(
                "package:{};kind:{:?};requirement:{:?};from:{:?};to:{:?}",
                row.package_name, row.kind, row.requirement, row.from_package, row.to_package
            ),
        )
    }));
    facts.extend(db.import_to_package_edges().iter().map(|row| {
        topology_fact(
            "ImportToPackage",
            row.stable_key.as_str(),
            row.producer_id,
            row.precision,
            import_to_package_status(row.status),
            format!(
                "import:{};context:{:?};from:{};to:{};source_set:{};semantic:{}",
                row.import_path,
                row.context,
                row.from_package_stable_key.as_deref().unwrap_or(""),
                row.to_package_stable_key.as_deref().unwrap_or(""),
                row.source_set_stable_key.as_deref().unwrap_or(""),
                row.semantic_import_stable_key.as_deref().unwrap_or("")
            ),
        )
    }));
    facts.extend(db.repo_topology_overlays().iter().map(|row| {
        topology_fact(
            "RepoTopologyOverlay",
            row.stable_key.as_str(),
            row.producer_id,
            row.precision,
            topology_status(row.status),
            format!(
                "kind:{:?};label:{};path:{};root:{:?};package:{:?};source_set:{:?}",
                row.kind,
                row.label,
                row.path.as_deref().unwrap_or(""),
                row.root,
                row.package,
                row.source_set
            ),
        )
    }));

    facts
}

#[cfg(test)]
fn topology_fact(
    family: &str,
    stable_key: &str,
    producer_id: &'static str,
    precision: TopologyPrecision,
    status: ObservedStatus,
    payload: String,
) -> ObservedItem {
    ObservedItem::Fact(ObservedFact {
        family: family.to_string(),
        stable_key: stable_key.to_string(),
        mode: AssertionMode::Exact,
        producer_id: Some(producer_id.to_string()),
        provenance: Some(producer_id.to_string()),
        precision: Some(topology_precision_label(precision).to_string()),
        status: Some(status),
        payload: Some(payload),
    })
}

#[cfg(test)]
fn topology_precision_label(precision: TopologyPrecision) -> &'static str {
    match precision {
        TopologyPrecision::ExactStatic => "exact_static",
        TopologyPrecision::ExactLockfile => "exact_lockfile",
        TopologyPrecision::Heuristic => "heuristic",
        TopologyPrecision::Unknown => "unknown",
        TopologyPrecision::Unsupported => "unsupported",
    }
}

#[cfg(test)]
fn topology_status(status: TopologyStatus) -> ObservedStatus {
    match status {
        TopologyStatus::Present => ObservedStatus::Present,
        TopologyStatus::Resolved => ObservedStatus::Resolved,
        TopologyStatus::Ambiguous => ObservedStatus::Ambiguous,
        TopologyStatus::Unresolved => ObservedStatus::Unresolved,
        TopologyStatus::Unknown => ObservedStatus::Unknown,
        TopologyStatus::SetupMissing => ObservedStatus::SetupMissing,
        TopologyStatus::MissingLockfile => ObservedStatus::MissingLockfile,
        TopologyStatus::Generated => ObservedStatus::Generated,
        TopologyStatus::External => ObservedStatus::External,
        TopologyStatus::Unsupported => ObservedStatus::Unsupported,
    }
}

#[cfg(test)]
fn import_to_package_status(status: ImportToPackageStatus) -> ObservedStatus {
    match status {
        ImportToPackageStatus::Resolved => ObservedStatus::Resolved,
        ImportToPackageStatus::External => ObservedStatus::External,
        ImportToPackageStatus::Unresolved => ObservedStatus::Unresolved,
        ImportToPackageStatus::SetupMissing => ObservedStatus::SetupMissing,
        ImportToPackageStatus::Unsupported => ObservedStatus::Unsupported,
        ImportToPackageStatus::Dynamic => ObservedStatus::Dynamic,
        ImportToPackageStatus::Ambiguous => ObservedStatus::Ambiguous,
        ImportToPackageStatus::Undeclared => ObservedStatus::Undeclared,
        ImportToPackageStatus::OutsideWorkspace => ObservedStatus::OutsideWorkspace,
    }
}

#[cfg(test)]
fn metadata_debug_facts(debug_json: &Value) -> Vec<ObservedItem> {
    let mut facts = Vec::new();
    for section in ["files", "imports", "symbols", "references"] {
        let Some(rows) = debug_json.get(section).and_then(Value::as_array) else {
            continue;
        };
        for row in rows {
            if let Some(fact) = metadata_debug_fact(row) {
                facts.push(ObservedItem::Fact(fact));
            }
        }
    }
    let Some(semantic) = debug_json.get("semantic").and_then(Value::as_object) else {
        return facts;
    };
    for &section in SEMANTIC_DEBUG_SECTIONS {
        let Some(rows) = semantic.get(section).and_then(Value::as_array) else {
            continue;
        };
        for row in rows {
            if let Some(fact) = metadata_debug_fact(row) {
                facts.push(ObservedItem::Fact(fact));
            }
        }
    }
    facts.extend(semantic_mir_facts(debug_json));
    facts.extend(cfg_facts(debug_json));
    facts.extend(call_facts(debug_json));
    facts.extend(abstract_domain_facts(debug_json));
    facts
}

#[cfg(test)]
fn semantic_mir_facts(debug_json: &Value) -> Vec<ObservedItem> {
    let mut facts = Vec::new();
    let Some(mir) = debug_json.get("mir").and_then(Value::as_object) else {
        return facts;
    };
    for &section in SEMANTIC_MIR_DEBUG_SECTIONS {
        let Some(rows) = mir.get(section).and_then(Value::as_array) else {
            continue;
        };
        for row in rows {
            if let Some(fact) = semantic_mir_fact(row) {
                facts.push(ObservedItem::Fact(fact));
            }
        }
    }
    facts
}

#[cfg(test)]
fn semantic_mir_fact(row: &Value) -> Option<ObservedFact> {
    Some(ObservedFact {
        family: row.get("family")?.as_str()?.to_string(),
        stable_key: row.get("stable_key")?.as_str()?.to_string(),
        mode: AssertionMode::Exact,
        producer_id: row
            .get("producer_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        provenance: Some("kernel.metadata_debug_json.mir".to_string()),
        precision: row
            .get("precision")
            .and_then(Value::as_str)
            .map(str::to_string),
        status: Some(observed_status_from_metadata(row)),
        payload: semantic_mir_payload(row),
    })
}

#[cfg(test)]
fn semantic_mir_payload(row: &Value) -> Option<String> {
    let mut parts = Vec::new();
    push_str_fragment(&mut parts, "path", row.get("path"));
    push_span_fragment(&mut parts, row.get("span"));
    push_u64_fragment(&mut parts, "owner", row.get("owner_function"));
    push_str_fragment(&mut parts, "kind", row.get("operation_kind"));
    push_str_fragment(&mut parts, "root", row.get("place_root"));
    push_list_fragment(&mut parts, "projections", row.get("place_projections"));
    push_str_fragment(&mut parts, "construct", row.get("unsupported_construct"));
    push_str_fragment(
        &mut parts,
        "conservative_action",
        row.get("conservative_action"),
    );

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(";"))
    }
}

#[cfg(test)]
fn cfg_facts(debug_json: &Value) -> Vec<ObservedItem> {
    let mut facts = Vec::new();
    let Some(cfg) = debug_json.get("cfg").and_then(Value::as_object) else {
        return facts;
    };
    for section in [
        "functions",
        "nodes",
        "blocks",
        "edges",
        "reachability",
        "dominators",
        "postdominators",
        "control_dependence",
        "unsupported",
    ] {
        let Some(rows) = cfg.get(section).and_then(Value::as_array) else {
            continue;
        };
        for row in rows {
            if let Some(fact) = cfg_fact(row) {
                facts.push(ObservedItem::Fact(fact));
            }
        }
    }
    facts
}

#[cfg(test)]
fn cfg_fact(row: &Value) -> Option<ObservedFact> {
    Some(ObservedFact {
        family: row.get("family")?.as_str()?.to_string(),
        stable_key: row.get("stable_key")?.as_str()?.to_string(),
        mode: AssertionMode::Exact,
        producer_id: row
            .get("producer_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        provenance: Some("kernel.metadata_debug_json.cfg".to_string()),
        precision: row
            .get("precision")
            .and_then(Value::as_str)
            .map(str::to_string),
        status: Some(observed_status_from_metadata(row)),
        payload: cfg_payload(row),
    })
}

#[cfg(test)]
fn cfg_payload(row: &Value) -> Option<String> {
    let mut parts = Vec::new();
    push_str_fragment(&mut parts, "path", row.get("path"));
    push_span_fragment(&mut parts, row.get("span"));
    push_u64_fragment(&mut parts, "function", row.get("function"));
    push_str_fragment(&mut parts, "view", row.get("view"));
    push_str_fragment(&mut parts, "kind", row.get("node_kind"));
    push_str_fragment(&mut parts, "kind", row.get("block_kind"));
    push_str_fragment(&mut parts, "construct", row.get("unsupported_construct"));
    push_str_fragment(&mut parts, "payload", row.get("payload"));
    push_str_fragment(&mut parts, "status", row.get("status"));
    push_str_fragment(&mut parts, "precision", row.get("precision"));

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(";"))
    }
}

#[cfg(test)]
fn call_facts(debug_json: &Value) -> Vec<ObservedItem> {
    let mut facts = Vec::new();
    let Some(calls) = debug_json.get("calls").and_then(Value::as_object) else {
        return facts;
    };
    for section in ["sites", "targets", "unresolved"] {
        let Some(rows) = calls.get(section).and_then(Value::as_array) else {
            continue;
        };
        for row in rows {
            if let Some(fact) = call_fact(row) {
                facts.push(ObservedItem::Fact(fact));
            }
        }
    }
    facts.extend(call_count_invariants(calls));
    facts.extend(call_index_count_invariants(calls));
    facts
}

#[cfg(test)]
fn call_count_invariants(calls: &serde_json::Map<String, Value>) -> Vec<ObservedItem> {
    let Some(counts) = calls.get("counts").and_then(Value::as_object) else {
        return Vec::new();
    };

    let mut invariants = Vec::new();
    for group in [
        "by_language",
        "by_call_kind",
        "by_algorithm",
        "by_status",
        "by_unresolved_reason",
        "by_provider",
    ] {
        let Some(values) = counts.get(group).and_then(Value::as_object) else {
            continue;
        };
        for (label, count) in values {
            if count.as_u64().is_some_and(|count| count > 0) {
                invariants.push(observed_invariant(
                    format!("direct_calls.counts.{group}.{label}.nonzero"),
                    "true",
                    "kernel.metadata_debug_json.calls.counts",
                ));
            }
        }
    }
    invariants
}

#[cfg(test)]
fn call_index_count_invariants(calls: &serde_json::Map<String, Value>) -> Vec<ObservedItem> {
    let Some(index_counts) = calls.get("index_counts").and_then(Value::as_object) else {
        return Vec::new();
    };

    index_counts
        .iter()
        .filter(|(_, count)| count.as_u64().is_some_and(|count| count > 0))
        .map(|(index, _)| {
            observed_invariant(
                format!("direct_calls.index_counts.{index}.nonzero"),
                "true",
                "kernel.metadata_debug_json.calls.index_counts",
            )
        })
        .collect()
}

#[cfg(test)]
fn call_fact(row: &Value) -> Option<ObservedFact> {
    Some(ObservedFact {
        family: row.get("family")?.as_str()?.to_string(),
        stable_key: row.get("stable_key")?.as_str()?.to_string(),
        mode: AssertionMode::Exact,
        producer_id: row
            .get("producer_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        provenance: Some("kernel.metadata_debug_json.calls".to_string()),
        precision: row
            .get("precision")
            .and_then(Value::as_str)
            .map(call_precision_label),
        status: Some(call_status_from_row(row)),
        payload: call_payload(row),
    })
}

#[cfg(test)]
fn call_payload(row: &Value) -> Option<String> {
    let mut parts = Vec::new();
    push_str_fragment(&mut parts, "path", row.get("path"));
    push_span_fragment(&mut parts, row.get("span"));
    push_str_fragment(&mut parts, "kind", row.get("kind"));
    push_str_fragment(&mut parts, "callee", row.get("callee"));
    push_str_fragment(&mut parts, "edge_kind", row.get("edge_kind"));
    push_str_fragment(&mut parts, "algorithm", row.get("algorithm"));
    push_str_fragment(&mut parts, "status", row.get("status"));
    push_str_fragment(&mut parts, "reason", row.get("reason"));
    push_str_fragment(&mut parts, "provider", row.get("producer_id"));
    push_str_fragment(&mut parts, "provenance", row.get("provenance"));
    push_str_fragment(&mut parts, "site", row.get("site_stable_key"));
    push_str_fragment(&mut parts, "caller", row.get("caller_stable_key"));
    push_str_fragment(
        &mut parts,
        "target_function",
        row.get("target_function_stable_key"),
    );
    push_str_fragment(
        &mut parts,
        "target_symbol",
        row.get("target_symbol_stable_key"),
    );

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(";"))
    }
}

#[cfg(test)]
fn call_status_from_row(row: &Value) -> ObservedStatus {
    row.get("status")
        .and_then(Value::as_str)
        .and_then(call_status_label)
        .or_else(|| {
            row.get("precision")
                .and_then(Value::as_str)
                .and_then(call_status_label)
        })
        .unwrap_or(ObservedStatus::Present)
}

#[cfg(test)]
fn call_status_label(label: &str) -> Option<ObservedStatus> {
    match label {
        "Resolved" | "resolved" => Some(ObservedStatus::Resolved),
        "Ambiguous" | "ambiguous" => Some(ObservedStatus::Ambiguous),
        "Unresolved" | "unresolved" => Some(ObservedStatus::Unresolved),
        "Unsupported" | "unsupported" => Some(ObservedStatus::Unsupported),
        "SetupMissing" | "setup_missing" => Some(ObservedStatus::SetupMissing),
        "Rejected" | "rejected" => Some(ObservedStatus::Rejected),
        "BudgetExceeded" | "budget_exceeded" => Some(ObservedStatus::Unknown),
        "Unknown" | "unknown" => Some(ObservedStatus::Unknown),
        _ => None,
    }
}

#[cfg(test)]
fn call_precision_label(label: &str) -> String {
    match label {
        "Exact" | "exact" => "exact",
        "SetupAware" | "setup_aware" => "setup_aware",
        "Conservative" | "conservative" => "conservative",
        "Heuristic" | "heuristic" => "heuristic",
        "Ambiguous" | "ambiguous" => "ambiguous",
        "Unknown" | "unknown" => "unknown",
        "Unsupported" | "unsupported" => "unsupported",
        _ => label,
    }
    .to_string()
}

#[cfg(test)]
fn abstract_domain_facts(debug_json: &Value) -> Vec<ObservedItem> {
    let mut facts = Vec::new();
    let Some(domains) = debug_json
        .get("abstract_domains")
        .and_then(Value::as_object)
    else {
        return facts;
    };
    for section in ["observations", "events"] {
        let Some(rows) = domains.get(section).and_then(Value::as_array) else {
            continue;
        };
        for row in rows {
            if let Some(fact) = abstract_domain_fact(row) {
                facts.push(ObservedItem::Fact(fact));
            }
        }
    }
    facts.extend(abstract_domain_count_invariants(domains));
    facts.extend(abstract_domain_index_count_invariants(domains));
    facts
}

#[cfg(test)]
fn abstract_domain_fact(row: &Value) -> Option<ObservedFact> {
    Some(ObservedFact {
        family: row.get("family")?.as_str()?.to_string(),
        stable_key: row.get("stable_key")?.as_str()?.to_string(),
        mode: AssertionMode::Exact,
        producer_id: row
            .get("producer_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        provenance: Some("kernel.metadata_debug_json.abstract_domains".to_string()),
        precision: row
            .get("precision")
            .and_then(Value::as_str)
            .map(abstract_domain_precision_label),
        status: Some(abstract_domain_status_from_row(row)),
        payload: abstract_domain_payload(row),
    })
}

#[cfg(test)]
fn abstract_domain_payload(row: &Value) -> Option<String> {
    let mut parts = Vec::new();
    push_str_fragment(&mut parts, "path", row.get("path"));
    push_span_fragment(&mut parts, row.get("span"));
    push_str_fragment(&mut parts, "body", row.get("body_stable_key"));
    push_str_fragment(&mut parts, "block", row.get("block_stable_key"));
    push_str_fragment(&mut parts, "operation", row.get("operation_stable_key"));
    push_str_fragment(&mut parts, "place", row.get("place_stable_key"));
    push_str_fragment(&mut parts, "slot", row.get("slot"));
    push_str_fragment(&mut parts, "location", row.get("location"));
    push_str_fragment(&mut parts, "value", row.get("value"));
    push_str_fragment(&mut parts, "status", row.get("status"));
    push_str_fragment(&mut parts, "precision", row.get("precision"));
    push_str_fragment(&mut parts, "reason", row.get("reason"));

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(";"))
    }
}

#[cfg(test)]
fn abstract_domain_count_invariants(domains: &serde_json::Map<String, Value>) -> Vec<ObservedItem> {
    let Some(counts) = domains.get("counts").and_then(Value::as_object) else {
        return Vec::new();
    };

    let mut invariants = Vec::new();
    for group in [
        "by_slot",
        "by_status",
        "by_precision",
        "by_reason",
        "by_provider",
    ] {
        let Some(values) = counts.get(group).and_then(Value::as_object) else {
            continue;
        };
        for (label, count) in values {
            if count.as_u64().is_some_and(|count| count > 0) {
                invariants.push(observed_invariant(
                    format!("abstract_domains.counts.{group}.{label}.nonzero"),
                    "true",
                    "kernel.metadata_debug_json.abstract_domains.counts",
                ));
            }
        }
    }
    invariants
}

#[cfg(test)]
fn abstract_domain_index_count_invariants(
    domains: &serde_json::Map<String, Value>,
) -> Vec<ObservedItem> {
    let Some(index_counts) = domains.get("index_counts").and_then(Value::as_object) else {
        return Vec::new();
    };

    index_counts
        .iter()
        .filter(|(_, count)| count.as_u64().is_some_and(|count| count > 0))
        .map(|(index, _)| {
            observed_invariant(
                format!("abstract_domains.index_counts.{index}.nonzero"),
                "true",
                "kernel.metadata_debug_json.abstract_domains.index_counts",
            )
        })
        .collect()
}

#[cfg(test)]
fn abstract_domain_status_from_row(row: &Value) -> ObservedStatus {
    row.get("status")
        .and_then(Value::as_str)
        .and_then(abstract_domain_status_label)
        .or_else(|| {
            row.get("precision")
                .and_then(Value::as_str)
                .and_then(abstract_domain_status_label)
        })
        .unwrap_or(ObservedStatus::Present)
}

#[cfg(test)]
fn abstract_domain_status_label(label: &str) -> Option<ObservedStatus> {
    match label {
        "Present" | "present" => Some(ObservedStatus::Present),
        "Top" | "top" => Some(ObservedStatus::Top),
        "Unknown" | "unknown" => Some(ObservedStatus::Unknown),
        "Unsupported" | "unsupported" => Some(ObservedStatus::Unsupported),
        "SetupMissing" | "setup_missing" => Some(ObservedStatus::SetupMissing),
        "BudgetExceeded" | "budget_exceeded" => Some(ObservedStatus::BudgetExceeded),
        _ => None,
    }
}

#[cfg(test)]
fn abstract_domain_precision_label(label: &str) -> String {
    match label {
        "ExactLocal" | "exact_local" => "exact_local",
        "SetupAware" | "setup_aware" => "setup_aware",
        "Conservative" | "conservative" => "conservative",
        "Heuristic" | "heuristic" => "heuristic",
        "Unknown" | "unknown" => "unknown",
        "Unsupported" | "unsupported" => "unsupported",
        _ => label,
    }
    .to_string()
}

#[cfg(test)]
fn push_str_fragment(parts: &mut Vec<String>, key: &str, value: Option<&Value>) {
    if let Some(value) = value
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("{key}={value}"));
    }
}

#[cfg(test)]
fn push_u64_fragment(parts: &mut Vec<String>, key: &str, value: Option<&Value>) {
    if let Some(value) = value.and_then(Value::as_u64) {
        parts.push(format!("{key}={value}"));
    }
}

#[cfg(test)]
fn push_list_fragment(parts: &mut Vec<String>, key: &str, value: Option<&Value>) {
    let Some(values) = value.and_then(Value::as_array) else {
        return;
    };
    let labels = values
        .iter()
        .filter_map(Value::as_str)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if !labels.is_empty() {
        parts.push(format!("{key}={}", labels.join(">")));
    }
}

#[cfg(test)]
fn push_span_fragment(parts: &mut Vec<String>, value: Option<&Value>) {
    let Some(span) = value else {
        return;
    };
    let (Some(start_line), Some(start_col), Some(end_line), Some(end_col)) = (
        span.get("start_line").and_then(Value::as_u64),
        span.get("start_col").and_then(Value::as_u64),
        span.get("end_line").and_then(Value::as_u64),
        span.get("end_col").and_then(Value::as_u64),
    ) else {
        return;
    };
    parts.push(format!(
        "span={start_line}:{start_col}-{end_line}:{end_col}"
    ));
}

#[cfg(test)]
fn metadata_debug_fact(row: &Value) -> Option<ObservedFact> {
    Some(ObservedFact {
        family: row.get("family")?.as_str()?.to_string(),
        stable_key: row.get("stable_key")?.as_str()?.to_string(),
        mode: AssertionMode::Exact,
        producer_id: row
            .get("producer_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        provenance: row
            .get("layer_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        precision: row
            .get("precision")
            .or_else(|| row.pointer("/metadata/precision"))
            .and_then(Value::as_str)
            .map(str::to_string),
        status: Some(observed_status_from_metadata(row)),
        payload: observed_fact_payload(row),
    })
}

#[cfg(test)]
fn observed_fact_payload(row: &Value) -> Option<String> {
    let payload = serde_json::json!({
        "path": row.get("path").cloned(),
        "span": row.get("span").cloned(),
        "name": row.get("name").cloned(),
        "export_name": row.get("export_name").cloned(),
        "source_stable_key": row.get("source_stable_key").cloned(),
        "target_stable_keys": row.get("target_stable_keys").cloned(),
        "generated_discriminator": row.get("generated_discriminator").cloned(),
    });
    let object = payload.as_object()?;
    if object.values().all(Value::is_null) {
        return None;
    }
    Some(payload.to_string())
}

#[cfg(test)]
fn observed_status_from_metadata(row: &Value) -> ObservedStatus {
    for field in ["status", "precision"] {
        if let Some(status) = row
            .get(field)
            .and_then(Value::as_str)
            .and_then(observed_status_from_label)
        {
            return status;
        }
    }
    ObservedStatus::Present
}

#[cfg(test)]
fn observed_status_from_label(label: &str) -> Option<ObservedStatus> {
    match label {
        "resolved" => Some(ObservedStatus::Resolved),
        "partial" => Some(ObservedStatus::Partial),
        "top" => Some(ObservedStatus::Top),
        "unknown" => Some(ObservedStatus::Unknown),
        "unresolved" => Some(ObservedStatus::Unresolved),
        "ambiguous" => Some(ObservedStatus::Ambiguous),
        "dynamic" => Some(ObservedStatus::Dynamic),
        "setup_missing" => Some(ObservedStatus::SetupMissing),
        "missing_lockfile" => Some(ObservedStatus::MissingLockfile),
        "unsupported" => Some(ObservedStatus::Unsupported),
        "external" => Some(ObservedStatus::External),
        "cycle" => Some(ObservedStatus::Cycle),
        "generated" => Some(ObservedStatus::Generated),
        "undeclared" => Some(ObservedStatus::Undeclared),
        "outside_workspace" => Some(ObservedStatus::OutsideWorkspace),
        "budget_exceeded" => Some(ObservedStatus::BudgetExceeded),
        _ => None,
    }
}

#[cfg(test)]
fn runtime_budget_name(expected: &[ExpectedItem], case_id: &str) -> String {
    expected
        .iter()
        .find_map(|item| match item {
            ExpectedItem::RuntimeBudget(budget) => Some(budget.name.clone()),
            _ => None,
        })
        .unwrap_or_else(|| case_id.to_string())
}

#[cfg(test)]
fn normalize_observed_path(path: &str, repo_root: &Path) -> String {
    let path = Path::new(path);
    let relative = if path.is_absolute() {
        path.strip_prefix(repo_root).unwrap_or(path)
    } else {
        path
    };
    slash_path(relative)
}

#[cfg(test)]
fn slash_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
fn saturating_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
fn observed_sort_key(item: &ObservedItem) -> String {
    serde_json::to_string(item).unwrap_or_default()
}

#[cfg(test)]
mod cfg {
    use crate::eval::model::ObservedItem;

    use super::*;

    #[test]
    fn cfg_facts_include_all_debug_families_with_compact_payloads() {
        let debug_json = serde_json::json!({
            "cfg": {
                "functions": [cfg_row("CfgFunction", "cfg:function", "resolved", "exact_lowered")],
                "nodes": [cfg_row("CfgNode", "cfg:node", "resolved", "exact_lowered")],
                "blocks": [cfg_row("BasicBlock", "cfg:block", "resolved", "exact_lowered")],
                "edges": [cfg_row("CfgEdge", "cfg:edge", "resolved", "exact_lowered")],
                "reachability": [cfg_row("CfgReachability", "cfg:reachability", "resolved", "exact_lowered")],
                "dominators": [cfg_row("CfgDominator", "cfg:dominator", "resolved", "exact_lowered")],
                "postdominators": [cfg_row("CfgPostDominator", "cfg:postdominator", "resolved", "exact_lowered")],
                "control_dependence": [cfg_row("CfgControlDependence", "cfg:dependence", "resolved", "exact_lowered")],
                "unsupported": [cfg_row("UnsupportedControlFlow", "cfg:unsupported", "unsupported", "unsupported")]
            }
        });
        let facts = cfg_facts_for_test(&debug_json);
        let families = facts
            .iter()
            .filter_map(|item| match item {
                ObservedItem::Fact(fact) => Some(fact.family.as_str()),
                _ => None,
            })
            .collect::<std::collections::BTreeSet<_>>();

        for family in crate::eval::model::CFG_FACT_FAMILIES {
            assert!(families.contains(family), "missing CFG family {family}");
        }
        assert!(facts.iter().any(|item| {
            match item {
                ObservedItem::Fact(fact) => fact
                    .payload
                    .as_deref()
                    .is_some_and(|payload| payload.contains("view=normal_control")),
                _ => false,
            }
        }));
    }

    fn cfg_row(family: &str, stable_key: &str, status: &str, precision: &str) -> serde_json::Value {
        serde_json::json!({
            "family": family,
            "stable_key": stable_key,
            "producer_id": "polint.cfg",
            "status": status,
            "precision": precision,
            "path": "src/app.ts",
            "span": {"start_line": 1, "start_col": 1, "end_line": 1, "end_col": 2},
            "function": 1,
            "view": "normal_control",
            "node_kind": "operation",
            "block_kind": "straight_line",
            "payload": "kind=return;from=1;to=2",
            "unsupported_construct": "throw"
        })
    }
}

#[cfg(test)]
mod eval_observed_kernel_tests {
    use std::fs;
    use std::path::Path;

    use crate::eval::fixtures::{NativeFixture, load_native_fixture};
    use crate::eval::model::{ObservedItem, ObservedStatus};
    use crate::eval::report::{
        CaseResult, EvaluationRun, MetricSummary, RuntimeObservation, deterministic_output_hash,
    };

    use super::*;

    fn write_fixture(root: &Path, source: &str, budget: Option<u64>) {
        fs::create_dir_all(root.join("repo/src")).unwrap();
        fs::write(
            root.join("repo/.polint.toml"),
            r#"
[workspace]
include = ["src/**"]
exclude = []
"#,
        )
        .unwrap();
        fs::write(root.join("repo/src/app.ts"), source).unwrap();

        let budget = budget.map_or_else(String::new, |max_runtime_ms| {
            format!(
                r#"
[budget]
max_runtime_ms = {max_runtime_ms}
"#
            )
        });
        fs::write(
            root.join("expected.polint-eval.toml"),
            format!(
                r#"
schema_version = "polint-eval-fixture-1"
case_id = "provider-order"
area = "kernel"

[repo]
path = "repo"
{budget}
"#
            ),
        )
        .unwrap();
    }

    fn native_fixture(source: &str, budget: Option<u64>) -> (tempfile::TempDir, NativeFixture) {
        let temp = tempfile::tempdir().unwrap();
        let fixture_dir = temp
            .path()
            .join("tests/eval-fixtures/kernel/provider-order");
        write_fixture(&fixture_dir, source, budget);
        let fixture = load_native_fixture(&fixture_dir).unwrap();
        (temp, fixture)
    }

    fn observed_for(source: &str, budget: Option<u64>) -> (tempfile::TempDir, Vec<ObservedItem>) {
        let (temp, fixture) = native_fixture(source, budget);
        let observed = observe_kernel_fixture(&fixture).unwrap();
        (temp, observed)
    }

    mod observed_phase23_cache_counter_invariants {
        use std::collections::BTreeMap;

        use super::*;

        fn write_mixed_fixture(root: &Path) {
            fs::create_dir_all(root.join("repo/goapp")).unwrap();
            fs::create_dir_all(root.join("repo/web/src")).unwrap();
            fs::write(
                root.join("repo/.polint.toml"),
                r#"
[workspace]
include = ["goapp/**", "web/**"]
exclude = []

[languages.go]
module_roots = ["goapp"]
package_patterns = ["./..."]
build_tags = ["integration"]
include_tests = false

[languages.ts]
module_kind = "esnext"
target = "es2022"
"#,
            )
            .unwrap();
            fs::write(
                root.join("repo/goapp/go.mod"),
                "module example.com/payment\n\ngo 1.24\n",
            )
            .unwrap();
            fs::write(root.join("repo/goapp/payment.go"), "package payment\n").unwrap();
            fs::write(
                root.join("repo/web/package.json"),
                r#"{"name":"web","version":"1.0.0","type":"module"}"#,
            )
            .unwrap();
            fs::write(
                root.join("repo/web/package-lock.json"),
                r#"{"name":"web","lockfileVersion":3,"packages":{}}"#,
            )
            .unwrap();
            fs::write(
                root.join("repo/web/tsconfig.json"),
                r#"{"compilerOptions":{"module":"esnext","target":"es2022"}}"#,
            )
            .unwrap();
            fs::write(
                root.join("repo/web/src/app.ts"),
                "export const amount = 42;\n",
            )
            .unwrap();
            fs::write(
                root.join("expected.polint-eval.toml"),
                r#"
schema_version = "polint-eval-fixture-1"
case_id = "input-snapshots"
area = "cache"

[repo]
path = "repo"
"#,
            )
            .unwrap();
        }

        fn observed_for_mixed_fixture() -> (tempfile::TempDir, Vec<ObservedItem>) {
            let temp = tempfile::tempdir().unwrap();
            let fixture_dir = temp
                .path()
                .join("tests/eval-fixtures/cache/input-snapshots");
            write_mixed_fixture(&fixture_dir);
            let fixture = load_native_fixture(&fixture_dir).unwrap();
            let observed = observe_kernel_fixture(&fixture).unwrap();
            (temp, observed)
        }

        fn invariant_values(observed: &[ObservedItem]) -> BTreeMap<&str, &str> {
            observed
                .iter()
                .filter_map(|item| match item {
                    ObservedItem::Invariant(invariant) => {
                        Some((invariant.name.as_str(), invariant.value.as_str()))
                    }
                    _ => None,
                })
                .collect()
        }

        #[test]
        fn snapshot_invariants_emit_required_phase23_inputs() {
            let (_temp, observed) = observed_for_mixed_fixture();
            let invariants = invariant_values(&observed);

            assert_eq!(
                invariants.get("snapshot.schema_version").copied(),
                Some("polint-input-snapshot-1")
            );
            assert_eq!(
                invariants
                    .get("snapshot.source_text_digest.present")
                    .copied(),
                Some("true")
            );
            assert_eq!(
                invariants.get("snapshot.config_digest.present").copied(),
                Some("true")
            );
            assert_eq!(
                invariants.get("snapshot.go_lifecycle.present").copied(),
                Some("true")
            );
            assert_eq!(
                invariants.get("snapshot.ts_js_lifecycle.present").copied(),
                Some("true")
            );
            assert_eq!(
                invariants.get("snapshot.rule_digest.present").copied(),
                Some("true")
            );
            assert_eq!(
                invariants.get("snapshot.model_digest.absent").copied(),
                Some("true")
            );
            assert_eq!(
                invariants
                    .get("snapshot.tool_invocation.unsupported")
                    .copied(),
                Some("true")
            );
        }

        #[test]
        fn provider_and_layer_key_invariants_emit_required_phase23_rows() {
            let (_temp, observed) = observed_for_mixed_fixture();
            let invariants = invariant_values(&observed);

            assert_eq!(
                invariants
                    .get("provider_output.polint.source.present")
                    .copied(),
                Some("true")
            );
            assert_eq!(
                invariants
                    .get("provider_output.polint.go.syntax.cache_stats.recorded")
                    .copied(),
                Some("true")
            );
            assert_eq!(
                invariants
                    .get("provider_output.polint.ts.syntax.cache_stats.recorded")
                    .copied(),
                Some("true")
            );
            assert_eq!(
                invariants
                    .get("layer_key.polint.ts.syntax.has_config_digest")
                    .copied(),
                Some("true")
            );
        }

        #[test]
        fn provider_cache_counter_invariants_emit_first_run_current_cache_values() {
            let (_temp, observed) = observed_for_mixed_fixture();
            let invariants = invariant_values(&observed);

            for provider_id in ["go", "ts"] {
                let prefix = format!("provider_output.polint.{provider_id}.syntax.cache_stats");
                for (counter, expected) in [
                    ("hits", "0"),
                    ("misses", "1"),
                    ("recomputes", "1"),
                    ("writes", "1"),
                    ("bypasses_disabled", "0"),
                    ("invalid_evicted_reads", "0"),
                    ("verified_reuse", "0"),
                    ("quarantines", "0"),
                ] {
                    let name = format!("{prefix}.{counter}");
                    assert_eq!(
                        invariants.get(name.as_str()).copied(),
                        Some(expected),
                        "missing or wrong invariant {name}"
                    );
                }
            }
        }

        #[test]
        fn observed_invariants_do_not_claim_future_cache_layer_semantics() {
            let (_temp, observed) = observed_for_mixed_fixture();
            let rendered = serde_json::to_string(&observed).unwrap();

            for forbidden in [
                ["Layer", "Cache"].concat(),
                ["Dependency", "Index"].concat(),
                ["Invalidation", "Plan"].concat(),
                ["stale", " reuse"].concat(),
                ["verified", " reuse success"].concat(),
                ["quarantine", " event"].concat(),
            ] {
                assert!(
                    !rendered.contains(forbidden.as_str()),
                    "unexpected {forbidden}"
                );
            }
        }
    }

    #[test]
    fn eval_observed_kernel_collects_provider_order_invariants_from_real_kernel() {
        let (_temp, observed) = observed_for("export function answer() { return 42; }\n", None);

        let provider_order = observed
            .iter()
            .filter_map(|item| match item {
                ObservedItem::Invariant(invariant)
                    if invariant.name.starts_with("provider_order.") =>
                {
                    Some((invariant.name.as_str(), invariant.value.as_str()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut provider_order = provider_order;
        provider_order.sort_by_key(|(name, _)| {
            name.strip_prefix("provider_order.")
                .and_then(|index| index.parse::<usize>().ok())
                .expect("provider order invariant names use numeric suffixes")
        });

        assert_eq!(
            provider_order,
            vec![
                ("provider_order.0", "polint.source"),
                ("provider_order.1", "polint.go.syntax"),
                ("provider_order.2", "polint.ts.syntax"),
                ("provider_order.3", "polint.module_graph"),
                ("provider_order.4", "polint.symbol_graph"),
                ("provider_order.5", "polint.module_topology"),
                ("provider_order.6", "polint.semantic_mir"),
                ("provider_order.7", "polint.cfg"),
                ("provider_order.8", "polint.calls"),
                ("provider_order.9", "polint.abstract_domains"),
                ("provider_order.10", "polint.direct_summaries"),
                ("provider_order.11", "polint.metrics"),
            ]
        );
    }

    #[test]
    fn eval_observed_kernel_records_abstract_domain_provider_output_schema() {
        let (_temp, observed) = observed_for("export function answer() { return 42; }\n", None);
        let invariants = observed
            .iter()
            .filter_map(|item| match item {
                ObservedItem::Invariant(invariant) => {
                    Some((invariant.name.as_str(), invariant.value.as_str()))
                }
                _ => None,
            })
            .collect::<std::collections::BTreeMap<_, _>>();

        assert_eq!(
            invariants
                .get("provider_output.polint.abstract_domains.schema_version")
                .copied(),
            Some("abstract-domain-facts-1:1")
        );
    }

    #[test]
    fn eval_observed_kernel_normalizes_diagnostics_to_relative_paths_and_fingerprints() {
        let (temp, observed) = observed_for("export function broken(\n", None);
        let rendered = serde_json::to_string_pretty(&observed).unwrap();
        let temp_root = temp.path().to_string_lossy();

        let diagnostics = observed
            .iter()
            .filter_map(|item| match item {
                ObservedItem::Diagnostic(diagnostic) => Some(diagnostic),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.rule_id == "parser/ts"
                && diagnostic.relative_path == "src/app.ts"
                && diagnostic
                    .fingerprint
                    .as_deref()
                    .is_some_and(|fingerprint| !fingerprint.is_empty())
                && diagnostic.status == Some(ObservedStatus::Present)
        }));
        assert!(!rendered.contains(temp_root.as_ref()));
    }

    #[test]
    fn eval_observed_kernel_collects_metadata_debug_facts_with_stable_identity() {
        let (_temp, observed) = observed_for("export function answer() { return 42; }\n", None);

        assert!(observed.iter().any(|item| match item {
            ObservedItem::Fact(fact) => {
                fact.family == "SourceFile"
                    && fact.stable_key.contains("src/app.ts")
                    && fact.producer_id.as_deref() == Some("polint.source")
                    && fact.precision.as_deref() == Some("exact")
                    && fact.status == Some(ObservedStatus::Present)
            }
            _ => false,
        }));
    }

    #[test]
    fn eval_observed_kernel_json_excludes_absolute_temp_paths() {
        let (temp, observed) =
            observed_for("export function answer() { return 42; }\n", Some(60_000));
        let rendered = serde_json::to_string_pretty(&observed).unwrap();
        let temp_root = temp.path().to_string_lossy();

        assert!(!rendered.contains(temp_root.as_ref()));
        assert!(!rendered.contains("0x"));
    }

    #[test]
    fn eval_observed_kernel_runtime_budget_duration_does_not_change_output_hash() {
        let (_temp, observed) =
            observed_for("export function answer() { return 42; }\n", Some(60_000));
        let first = run_with_observed_runtime(observed.clone(), 1);
        let second = run_with_observed_runtime(observed, 999);

        assert_eq!(
            deterministic_output_hash(&first),
            deterministic_output_hash(&second)
        );
    }

    fn run_with_observed_runtime(
        mut observed: Vec<ObservedItem>,
        runtime_ms: u64,
    ) -> EvaluationRun {
        for item in &mut observed {
            if let ObservedItem::RuntimeBudget(budget) = item {
                budget.observed_runtime_ms = Some(runtime_ms);
            }
        }

        EvaluationRun {
            schema_version: "polint-eval-internal-1".to_string(),
            suite_id: "native-fixture".to_string(),
            cases: vec![CaseResult {
                case_id: "provider-order".to_string(),
                area: crate::eval::model::FixtureArea::Kernel,
                expected: Vec::new(),
                observed,
                matches: Vec::new(),
                runtime: RuntimeObservation {
                    budget_name: "provider-order".to_string(),
                    budget_passed: true,
                    observed_runtime_ms: Some(runtime_ms),
                },
            }],
            metrics: MetricSummary {
                true_positives: 0,
                false_positives: 0,
                false_negatives: 0,
                true_negatives: 0,
                unconfirmed: 0,
                false_positive_trap_hits: 0,
                forbidden_hits: 0,
                unknown_count: 0,
                facts_present: 0,
                facts_accepted: 0,
                facts_rejected: 0,
                graph_edges_expected: 0,
                graph_edges_observed: 0,
                graph_edges_unconfirmed: 0,
                paths_expected: 0,
                paths_observed: 0,
                paths_unconfirmed: 0,
                runtime_budget_passed: 1,
                runtime_budget_failed: 0,
                precision: None,
                recall: None,
                f1: None,
                f2: None,
                f3: None,
                false_positive_rate: None,
            },
            output_hash: String::new(),
        }
    }
}

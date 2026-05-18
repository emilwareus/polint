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
use crate::eval::fixtures::NativeFixture;
#[cfg(test)]
use crate::eval::model::{
    AssertionMode, ExpectedItem, ObservedDiagnostic, ObservedFact, ObservedInvariant, ObservedItem,
    ObservedRuntimeBudget, ObservedStatus,
};

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
    facts
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
            .and_then(Value::as_str)
            .map(str::to_string),
        status: Some(observed_status_from_metadata(row)),
    })
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
        "unknown" | "unresolved" | "ambiguous" => Some(ObservedStatus::Unknown),
        "setup_missing" => Some(ObservedStatus::SetupMissing),
        "unsupported" => Some(ObservedStatus::Unsupported),
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

        assert_eq!(
            provider_order,
            vec![
                ("provider_order.0", "polint.source"),
                ("provider_order.1", "polint.go.syntax"),
                ("provider_order.2", "polint.ts.syntax"),
                ("provider_order.3", "polint.module_graph"),
                ("provider_order.4", "polint.symbol_graph"),
                ("provider_order.5", "polint.metrics"),
            ]
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

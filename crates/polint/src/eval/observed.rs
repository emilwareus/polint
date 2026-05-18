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
    let started = Instant::now();
    let loaded = load_config(repo_root)?;
    let plan = AnalysisPlan::empty();
    let config_digest = config_hash(&loaded);
    let rule_digest = rule_hash(&[], None, &BTreeMap::new());
    let cache = Cache::default_for_repo(repo_root, cache_enabled);
    let output = AnalysisKernel::run(KernelInput {
        loaded: &loaded,
        cache: &cache,
        config_digest: &config_digest,
        rule_digest: &rule_digest,
        plan: &plan,
        parallel: true,
    })?;
    let elapsed = started.elapsed();

    let mut observed = Vec::new();
    observed.extend(observed_diagnostics(&output.diagnostics, repo_root));
    observed.extend(provider_order_invariants());
    observed.extend(metadata_debug_facts(
        &AnalysisKernel::metadata_debug_json_for_test(&output.db),
    ));
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

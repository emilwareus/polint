use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, bail, ensure};
use serde::Deserialize;

use crate::eval::model::{ExpectedItem, FixtureArea, ObservedItem};

const FIXTURE_SCHEMA_VERSION: &str = "polint-eval-fixture-1";
const FIXTURE_MANIFEST_FILE: &str = "expected.polint-eval.toml";

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct NativeFixtureManifest {
    pub(crate) schema_version: String,
    pub(crate) case_id: String,
    pub(crate) area: FixtureArea,
    #[serde(default)]
    pub(crate) synthetic_observed: bool,
    pub(crate) repo: FixtureRepo,
    #[serde(default)]
    pub(crate) expected: Vec<ExpectedItem>,
    #[serde(default)]
    pub(crate) observed: Vec<ObservedItem>,
    pub(crate) budget: Option<FixtureBudget>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct FixtureRepo {
    pub(crate) path: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct FixtureBudget {
    pub(crate) max_runtime_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NativeFixture {
    pub(crate) fixture_dir: PathBuf,
    pub(crate) repo_dir: PathBuf,
    pub(crate) manifest: NativeFixtureManifest,
}

pub(crate) fn load_native_fixture(fixture_dir: &Path) -> anyhow::Result<NativeFixture> {
    let fixture_dir = fixture_dir
        .canonicalize()
        .with_context(|| format!("canonicalize fixture dir {}", fixture_dir.display()))?;
    let manifest_path = fixture_dir.join(FIXTURE_MANIFEST_FILE);
    let manifest_toml = fs::read_to_string(&manifest_path)
        .with_context(|| format!("read fixture manifest {}", manifest_path.display()))?;
    let mut manifest: NativeFixtureManifest = toml::from_str(&manifest_toml)
        .with_context(|| format!("parse fixture manifest {}", manifest_path.display()))?;

    ensure!(
        manifest.schema_version == FIXTURE_SCHEMA_VERSION,
        "unsupported fixture schema_version `{}`; expected `{FIXTURE_SCHEMA_VERSION}`",
        manifest.schema_version
    );
    validate_manifest_relative_path("repo.path", &manifest.repo.path)?;
    normalize_expected_item_paths(&mut manifest.expected)?;
    normalize_observed_item_paths(&mut manifest.observed)?;
    validate_synthetic_observed_rows(&manifest)?;

    let repo_dir = fixture_dir.join(Path::new(&manifest.repo.path));
    let repo_dir = repo_dir
        .canonicalize()
        .with_context(|| format!("canonicalize fixture repo {}", repo_dir.display()))?;
    ensure!(
        repo_dir.starts_with(&fixture_dir),
        "fixture repo path must stay inside fixture directory"
    );

    Ok(NativeFixture {
        fixture_dir,
        repo_dir,
        manifest,
    })
}

fn normalize_expected_item_paths(expected: &mut [ExpectedItem]) -> anyhow::Result<()> {
    for item in expected {
        if let ExpectedItem::Diagnostic(diagnostic) = item {
            diagnostic.relative_path = normalize_manifest_relative_path(
                "expected.diagnostic.relative_path",
                &diagnostic.relative_path,
            )?;
        }
    }
    Ok(())
}

fn normalize_observed_item_paths(observed: &mut [ObservedItem]) -> anyhow::Result<()> {
    for item in observed {
        if let ObservedItem::Diagnostic(diagnostic) = item {
            diagnostic.relative_path = normalize_manifest_relative_path(
                "observed.diagnostic.relative_path",
                &diagnostic.relative_path,
            )?;
        }
    }
    Ok(())
}

fn validate_synthetic_observed_rows(manifest: &NativeFixtureManifest) -> anyhow::Result<()> {
    if manifest.synthetic_observed {
        ensure!(
            manifest.area == FixtureArea::Extension,
            "synthetic observed rows are only allowed for extension fixtures"
        );
    } else {
        ensure!(
            manifest.observed.is_empty(),
            "manifest observed rows require synthetic_observed = true"
        );
    }
    Ok(())
}

fn normalize_manifest_relative_path(field: &str, value: &str) -> anyhow::Result<String> {
    validate_manifest_relative_path(field, value)?;
    Ok(value.replace('\\', "/"))
}

fn validate_manifest_relative_path(field: &str, value: &str) -> anyhow::Result<()> {
    if is_absolute_manifest_path(value) {
        bail!("{field} must be relative, not absolute");
    }
    if has_parent_dir_component(value) {
        bail!("{field} must not contain a parent directory component");
    }
    Ok(())
}

fn is_absolute_manifest_path(value: &str) -> bool {
    Path::new(value).is_absolute()
        || value.starts_with('/')
        || value.starts_with('\\')
        || value.as_bytes().get(1) == Some(&b':')
}

fn has_parent_dir_component(value: &str) -> bool {
    Path::new(value)
        .components()
        .any(|component| matches!(component, Component::ParentDir))
        || value.split(['/', '\\']).any(|component| component == "..")
}

#[cfg(test)]
pub(crate) fn run_native_fixture_for_test(
    fixture_dir: &Path,
) -> anyhow::Result<crate::eval::report::EvaluationRun> {
    let fixture = load_native_fixture(fixture_dir)?;
    let observed = if fixture.manifest.synthetic_observed {
        fixture.manifest.observed.clone()
    } else {
        crate::eval::observed::observe_kernel_fixture(&fixture)?
    };
    Ok(evaluation_run_for_fixture(&fixture, observed))
}

#[cfg(test)]
pub(crate) fn run_cache_current_determinism_fixture_for_test(
    fixture_dir: &Path,
) -> anyhow::Result<crate::eval::report::EvaluationRun> {
    let fixture = load_native_fixture(fixture_dir)?;
    let temp = crate::eval::observed::copy_fixture_repo_for_test(&fixture)?;

    let cold_observed =
        crate::eval::observed::observe_kernel_fixture_repo_for_test(&fixture, temp.path(), true)?;
    let warm_observed =
        crate::eval::observed::observe_kernel_fixture_repo_for_test(&fixture, temp.path(), true)?;
    let no_cache_observed =
        crate::eval::observed::observe_kernel_fixture_repo_for_test(&fixture, temp.path(), false)?;

    let cold_run = evaluation_run_for_fixture(&fixture, cold_observed);
    let warm_run = evaluation_run_for_fixture(&fixture, warm_observed);
    let no_cache_run = evaluation_run_for_fixture(&fixture, no_cache_observed.clone());

    let cold_warm_no_cache_equal = cache_comparison_json(&cold_run)
        == cache_comparison_json(&warm_run)
        && cache_comparison_json(&cold_run) == cache_comparison_json(&no_cache_run);

    let mut observed = no_cache_observed;
    if cold_warm_no_cache_equal {
        observed.push(cache_current_determinism_observed_invariant());
    }

    Ok(evaluation_run_for_fixture(&fixture, observed))
}

#[cfg(test)]
pub(crate) fn run_layer_cache_fixture_for_test(
    fixture_dir: &Path,
) -> anyhow::Result<crate::eval::report::EvaluationRun> {
    let started = std::time::Instant::now();
    let fixture = load_native_fixture(fixture_dir)?;
    let temp = crate::eval::observed::copy_fixture_repo_for_test(&fixture)?;
    let plan = crate::analysis_plan::AnalysisPlan::from_capability_names_for_test(&[
        "resolved_imports",
        "module_graph",
        "symbols",
        "references",
        "file_metrics",
        "function_metrics",
        "complexity_metrics",
    ]);

    let cold_observed = crate::eval::observed::observe_kernel_fixture_repo_with_plan_for_test(
        &fixture,
        temp.path(),
        true,
        &plan,
    )?;
    let warm_observed = crate::eval::observed::observe_kernel_fixture_repo_with_plan_for_test(
        &fixture,
        temp.path(),
        true,
        &plan,
    )?;
    let layer_file_count_after_warm = managed_layer_file_count(temp.path())?;
    let disabled_observed = crate::eval::observed::observe_kernel_fixture_repo_with_plan_for_test(
        &fixture,
        temp.path(),
        false,
        &plan,
    )?;
    let layer_file_count_after_disabled = managed_layer_file_count(temp.path())?;

    apply_layer_cache_import_edit(temp.path())?;
    let import_edit_observed =
        crate::eval::observed::observe_kernel_fixture_repo_with_plan_for_test(
            &fixture,
            temp.path(),
            true,
            &plan,
        )?;

    let mut observed = Vec::new();
    observed.extend(tagged_layer_cache_invariants("cold", &cold_observed));
    observed.extend(tagged_layer_cache_invariants("warm", &warm_observed));
    observed.extend(tagged_layer_cache_invariants(
        "disabled",
        &disabled_observed,
    ));
    observed.extend(tagged_layer_cache_invariants(
        "import_edit",
        &import_edit_observed,
    ));
    observed.push(layer_cache_fixture_invariant(
        "layer_cache.disabled.no_new_files",
        bool_string(layer_file_count_after_warm == layer_file_count_after_disabled),
    ));
    if let Some(budget) = &fixture.manifest.budget {
        let elapsed = started.elapsed();
        observed.push(crate::eval::model::ObservedItem::RuntimeBudget(
            crate::eval::model::ObservedRuntimeBudget {
                name: runtime_budget_name(&fixture.manifest.expected, &fixture.manifest.case_id),
                budget_passed: elapsed <= std::time::Duration::from_millis(budget.max_runtime_ms),
                observed_runtime_ms: Some(saturating_millis(elapsed)),
            },
        ));
    }

    Ok(evaluation_run_for_fixture(&fixture, observed))
}

#[cfg(test)]
pub(crate) fn run_semantic_index_core_fixture_for_test(
    fixture_dir: &Path,
) -> anyhow::Result<crate::eval::report::EvaluationRun> {
    let started = std::time::Instant::now();
    let fixture = load_native_fixture(fixture_dir)?;
    let temp = crate::eval::observed::copy_fixture_repo_for_test(&fixture)?;
    let plan = crate::analysis_plan::AnalysisPlan::from_capability_names_for_test(&[
        "resolved_imports",
        "module_graph",
        "symbols",
        "references",
    ]);

    let cold_observed = crate::eval::observed::observe_kernel_fixture_repo_with_plan_for_test(
        &fixture,
        temp.path(),
        true,
        &plan,
    )?;
    let warm_observed = crate::eval::observed::observe_kernel_fixture_repo_with_plan_for_test(
        &fixture,
        temp.path(),
        true,
        &plan,
    )?;

    let mut observed = warm_observed
        .iter()
        .filter(|item| {
            !is_cache_stats_observed_invariant(item)
                && !matches!(item, crate::eval::model::ObservedItem::RuntimeBudget(_))
        })
        .cloned()
        .collect::<Vec<_>>();
    observed.extend(tagged_layer_cache_invariants("cold", &cold_observed));
    observed.extend(tagged_layer_cache_invariants("warm", &warm_observed));

    if let Some(budget) = &fixture.manifest.budget {
        let elapsed = started.elapsed();
        observed.push(crate::eval::model::ObservedItem::RuntimeBudget(
            crate::eval::model::ObservedRuntimeBudget {
                name: runtime_budget_name(&fixture.manifest.expected, &fixture.manifest.case_id),
                budget_passed: elapsed <= std::time::Duration::from_millis(budget.max_runtime_ms),
                observed_runtime_ms: Some(saturating_millis(elapsed)),
            },
        ));
    }

    Ok(evaluation_run_for_fixture(&fixture, observed))
}

#[cfg(test)]
pub(crate) fn run_module_topology_core_fixture_for_test(
    fixture_dir: &Path,
) -> anyhow::Result<crate::eval::report::EvaluationRun> {
    let started = std::time::Instant::now();
    let fixture = load_native_fixture(fixture_dir)?;
    let temp = crate::eval::observed::copy_fixture_repo_for_test(&fixture)?;
    let plan = crate::analysis_plan::AnalysisPlan::from_capability_names_for_test(&[
        "resolved_imports",
        "module_graph",
        "symbols",
        "references",
    ]);

    let cold_observed = crate::eval::observed::observe_kernel_fixture_repo_with_plan_for_test(
        &fixture,
        temp.path(),
        true,
        &plan,
    )?;
    let warm_observed = crate::eval::observed::observe_kernel_fixture_repo_with_plan_for_test(
        &fixture,
        temp.path(),
        true,
        &plan,
    )?;

    apply_module_topology_package_json_edit(temp.path())?;
    let package_json_observed =
        crate::eval::observed::observe_kernel_fixture_repo_with_plan_for_test(
            &fixture,
            temp.path(),
            true,
            &plan,
        )?;

    apply_module_topology_go_mod_edit(temp.path())?;
    let go_mod_observed = crate::eval::observed::observe_kernel_fixture_repo_with_plan_for_test(
        &fixture,
        temp.path(),
        true,
        &plan,
    )?;

    let mut observed = warm_observed
        .iter()
        .filter(|item| !is_cache_stats_observed_invariant(item))
        .cloned()
        .collect::<Vec<_>>();
    observed.extend(tagged_layer_cache_invariants("cold", &cold_observed));
    observed.extend(tagged_layer_cache_invariants("warm", &warm_observed));
    observed.extend(tagged_layer_cache_invariants(
        "package_json_edit",
        &package_json_observed,
    ));
    observed.extend(tagged_layer_cache_invariants(
        "go_mod_edit",
        &go_mod_observed,
    ));

    if let Some(budget) = &fixture.manifest.budget {
        let elapsed = started.elapsed();
        observed.push(crate::eval::model::ObservedItem::RuntimeBudget(
            crate::eval::model::ObservedRuntimeBudget {
                name: runtime_budget_name(&fixture.manifest.expected, &fixture.manifest.case_id),
                budget_passed: elapsed <= std::time::Duration::from_millis(budget.max_runtime_ms),
                observed_runtime_ms: Some(saturating_millis(elapsed)),
            },
        ));
    }

    Ok(evaluation_run_for_fixture(&fixture, observed))
}

#[cfg(test)]
pub(crate) fn run_semantic_mir_core_fixture_for_test(
    fixture_dir: &Path,
) -> anyhow::Result<crate::eval::report::EvaluationRun> {
    let started = std::time::Instant::now();
    let fixture = load_native_fixture(fixture_dir)?;
    let temp = crate::eval::observed::copy_fixture_repo_for_test(&fixture)?;
    let plan = crate::analysis_plan::AnalysisPlan::from_capability_names_for_test(&[
        "symbols",
        "references",
        "resolved_imports",
        "module_graph",
    ]);

    let cold_observed = crate::eval::observed::observe_kernel_fixture_repo_with_plan_for_test(
        &fixture,
        temp.path(),
        true,
        &plan,
    )?;
    let warm_observed = crate::eval::observed::observe_kernel_fixture_repo_with_plan_for_test(
        &fixture,
        temp.path(),
        true,
        &plan,
    )?;

    let cold_run = evaluation_run_for_fixture(&fixture, cold_observed);
    let warm_run = evaluation_run_for_fixture(&fixture, warm_observed.clone());
    let cold_warm_equal = cache_comparison_json(&cold_run) == cache_comparison_json(&warm_run);

    let mut observed = warm_observed
        .iter()
        .filter(|item| !is_cache_stats_observed_invariant(item))
        .cloned()
        .collect::<Vec<_>>();
    if cold_warm_equal {
        observed.push(semantic_mir_determinism_observed_invariant());
    }

    if let Some(budget) = &fixture.manifest.budget {
        let elapsed = started.elapsed();
        observed.push(crate::eval::model::ObservedItem::RuntimeBudget(
            crate::eval::model::ObservedRuntimeBudget {
                name: runtime_budget_name(&fixture.manifest.expected, &fixture.manifest.case_id),
                budget_passed: elapsed <= std::time::Duration::from_millis(budget.max_runtime_ms),
                observed_runtime_ms: Some(saturating_millis(elapsed)),
            },
        ));
    }

    Ok(evaluation_run_for_fixture(&fixture, observed))
}

#[cfg(test)]
pub(crate) fn run_cfg_core_fixture_for_test(
    fixture_dir: &Path,
) -> anyhow::Result<crate::eval::report::EvaluationRun> {
    let started = std::time::Instant::now();
    let fixture = load_native_fixture(fixture_dir)?;
    let temp = crate::eval::observed::copy_fixture_repo_for_test(&fixture)?;
    let plan = crate::analysis_plan::AnalysisPlan::from_capability_names_for_test(&[
        "symbols",
        "references",
        "resolved_imports",
        "module_graph",
    ]);

    let cold_observed = crate::eval::observed::observe_kernel_fixture_repo_with_plan_for_test(
        &fixture,
        temp.path(),
        true,
        &plan,
    )?;
    let warm_observed = crate::eval::observed::observe_kernel_fixture_repo_with_plan_for_test(
        &fixture,
        temp.path(),
        true,
        &plan,
    )?;
    let no_cache_observed = crate::eval::observed::observe_kernel_fixture_repo_with_plan_for_test(
        &fixture,
        temp.path(),
        false,
        &plan,
    )?;

    let cold_run = evaluation_run_for_fixture(&fixture, cold_observed);
    let warm_run = evaluation_run_for_fixture(&fixture, warm_observed);
    let no_cache_run = evaluation_run_for_fixture(&fixture, no_cache_observed.clone());
    let deterministic = cache_comparison_json(&cold_run) == cache_comparison_json(&warm_run)
        && cache_comparison_json(&cold_run) == cache_comparison_json(&no_cache_run);

    let mut observed = no_cache_observed
        .iter()
        .filter(|item| !is_cache_stats_observed_invariant(item))
        .cloned()
        .collect::<Vec<_>>();
    if deterministic {
        observed.push(cfg_determinism_observed_invariant());
    }

    if let Some(budget) = &fixture.manifest.budget {
        let elapsed = started.elapsed();
        observed.push(crate::eval::model::ObservedItem::RuntimeBudget(
            crate::eval::model::ObservedRuntimeBudget {
                name: runtime_budget_name(&fixture.manifest.expected, &fixture.manifest.case_id),
                budget_passed: elapsed <= std::time::Duration::from_millis(budget.max_runtime_ms),
                observed_runtime_ms: Some(saturating_millis(elapsed)),
            },
        ));
    }

    Ok(evaluation_run_for_fixture(&fixture, observed))
}

#[cfg(test)]
pub(crate) fn run_direct_calls_core_fixture_for_test(
    fixture_dir: &Path,
) -> anyhow::Result<crate::eval::report::EvaluationRun> {
    let started = std::time::Instant::now();
    let fixture = load_native_fixture(fixture_dir)?;
    let temp = crate::eval::observed::copy_fixture_repo_for_test(&fixture)?;
    let plan = crate::analysis_plan::AnalysisPlan::from_capability_names_for_test(&[
        "symbols",
        "references",
        "resolved_imports",
        "module_graph",
    ]);

    let cold_observed = crate::eval::observed::observe_kernel_fixture_repo_with_plan_for_test(
        &fixture,
        temp.path(),
        true,
        &plan,
    )?;
    let warm_observed = crate::eval::observed::observe_kernel_fixture_repo_with_plan_for_test(
        &fixture,
        temp.path(),
        true,
        &plan,
    )?;
    let no_cache_observed = crate::eval::observed::observe_kernel_fixture_repo_with_plan_for_test(
        &fixture,
        temp.path(),
        false,
        &plan,
    )?;

    let cold_run = evaluation_run_for_fixture(&fixture, cold_observed);
    let warm_run = evaluation_run_for_fixture(&fixture, warm_observed);
    let no_cache_run = evaluation_run_for_fixture(&fixture, no_cache_observed.clone());
    let deterministic = cache_comparison_json(&cold_run) == cache_comparison_json(&warm_run)
        && cache_comparison_json(&cold_run) == cache_comparison_json(&no_cache_run);

    let mut observed = no_cache_observed
        .iter()
        .filter(|item| !is_cache_stats_observed_invariant(item))
        .cloned()
        .collect::<Vec<_>>();
    if deterministic {
        observed.push(direct_calls_determinism_observed_invariant());
    }

    if let Some(budget) = &fixture.manifest.budget {
        let elapsed = started.elapsed();
        observed.push(crate::eval::model::ObservedItem::RuntimeBudget(
            crate::eval::model::ObservedRuntimeBudget {
                name: runtime_budget_name(&fixture.manifest.expected, &fixture.manifest.case_id),
                budget_passed: elapsed <= std::time::Duration::from_millis(budget.max_runtime_ms),
                observed_runtime_ms: Some(saturating_millis(elapsed)),
            },
        ));
    }

    Ok(evaluation_run_for_fixture(&fixture, observed))
}

#[cfg(test)]
fn evaluation_run_for_fixture(
    fixture: &NativeFixture,
    observed: Vec<crate::eval::model::ObservedItem>,
) -> crate::eval::report::EvaluationRun {
    let matches = crate::eval::matcher::match_case(
        &fixture.manifest.expected,
        &observed,
        crate::eval::matcher::MatcherConfig::default(),
    );
    let metrics: crate::eval::report::MetricSummary =
        crate::eval::metrics::compute_metrics(&matches).into();
    let runtime = runtime_observation(&fixture.manifest, &observed);
    let run = crate::eval::report::EvaluationRun {
        schema_version: crate::eval::report::EVALUATION_SCHEMA_VERSION.to_string(),
        suite_id: "native-fixtures".to_string(),
        cases: vec![crate::eval::report::CaseResult {
            case_id: fixture.manifest.case_id.clone(),
            area: fixture.manifest.area,
            expected: fixture.manifest.expected.clone(),
            observed,
            matches,
            runtime,
        }],
        metrics,
        output_hash: String::new(),
    };
    let mut run = crate::eval::report::normalize_run(&run);
    run.output_hash = crate::eval::report::deterministic_output_hash(&run);

    run
}

#[cfg(test)]
fn cache_current_determinism_observed_invariant() -> crate::eval::model::ObservedItem {
    crate::eval::model::ObservedItem::Invariant(crate::eval::model::ObservedInvariant {
        name: "cache.current_determinism".to_string(),
        value: "cold_warm_no_cache_equal".to_string(),
        mode: crate::eval::model::AssertionMode::Exact,
        producer_id: Some("polint.eval".to_string()),
        provenance: Some("cache.current_behavior".to_string()),
        precision: Some("exact".to_string()),
        status: Some(crate::eval::model::ObservedStatus::Present),
    })
}

#[cfg(test)]
fn semantic_mir_determinism_observed_invariant() -> crate::eval::model::ObservedItem {
    crate::eval::model::ObservedItem::Invariant(crate::eval::model::ObservedInvariant {
        name: "semantic_mir.current_determinism".to_string(),
        value: "cold_warm_equal".to_string(),
        mode: crate::eval::model::AssertionMode::Exact,
        producer_id: Some("polint.eval".to_string()),
        provenance: Some("semantic_mir.current_behavior".to_string()),
        precision: Some("exact".to_string()),
        status: Some(crate::eval::model::ObservedStatus::Present),
    })
}

#[cfg(test)]
fn cfg_determinism_observed_invariant() -> crate::eval::model::ObservedItem {
    crate::eval::model::ObservedItem::Invariant(crate::eval::model::ObservedInvariant {
        name: "cfg.current_determinism".to_string(),
        value: "cold_warm_no_cache_equal".to_string(),
        mode: crate::eval::model::AssertionMode::Exact,
        producer_id: Some("polint.eval".to_string()),
        provenance: Some("cfg.current_behavior".to_string()),
        precision: Some("exact".to_string()),
        status: Some(crate::eval::model::ObservedStatus::Present),
    })
}

#[cfg(test)]
fn direct_calls_determinism_observed_invariant() -> crate::eval::model::ObservedItem {
    crate::eval::model::ObservedItem::Invariant(crate::eval::model::ObservedInvariant {
        name: "direct_calls.current_determinism".to_string(),
        value: "cold_warm_no_cache_equal".to_string(),
        mode: crate::eval::model::AssertionMode::Exact,
        producer_id: Some("polint.eval".to_string()),
        provenance: Some("direct_calls.current_behavior".to_string()),
        precision: Some("exact".to_string()),
        status: Some(crate::eval::model::ObservedStatus::Present),
    })
}

#[cfg(test)]
fn cache_comparison_json(run: &crate::eval::report::EvaluationRun) -> String {
    let mut normalized = run_without_runtime_durations(run);
    normalized.output_hash = crate::eval::report::deterministic_output_hash(&normalized);
    serde_json::to_string_pretty(&normalized).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
fn run_without_runtime_durations(
    run: &crate::eval::report::EvaluationRun,
) -> crate::eval::report::EvaluationRun {
    let mut normalized = crate::eval::report::normalize_run(run);
    normalized.output_hash.clear();
    for case in &mut normalized.cases {
        case.runtime.observed_runtime_ms = None;
        case.observed
            .retain(|item| !is_cache_stats_observed_invariant(item));
        for item in &mut case.observed {
            if let crate::eval::model::ObservedItem::RuntimeBudget(budget) = item {
                budget.observed_runtime_ms = None;
            }
        }
        for summary in &mut case.matches {
            summary.observed_runtime_ms = None;
        }
        case.matches = crate::eval::matcher::match_case(
            &case.expected,
            &case.observed,
            crate::eval::matcher::MatcherConfig::default(),
        );
    }
    let matches = normalized
        .cases
        .iter()
        .flat_map(|case| case.matches.iter().cloned())
        .collect::<Vec<_>>();
    normalized.metrics = crate::eval::metrics::compute_metrics(&matches).into();
    normalized
}

#[cfg(test)]
fn is_cache_stats_observed_invariant(item: &crate::eval::model::ObservedItem) -> bool {
    match item {
        crate::eval::model::ObservedItem::Invariant(invariant) => {
            (invariant.name.starts_with("provider_output.polint.")
                && invariant.name.contains(".cache_stats."))
                || invariant.name.starts_with("layer_cache.")
        }
        _ => false,
    }
}

#[cfg(test)]
fn tagged_layer_cache_invariants(
    pass: &str,
    observed: &[crate::eval::model::ObservedItem],
) -> Vec<crate::eval::model::ObservedItem> {
    observed
        .iter()
        .filter_map(|item| {
            let crate::eval::model::ObservedItem::Invariant(invariant) = item else {
                return None;
            };
            if !matches!(
                invariant.name.as_str(),
                name if name.starts_with("layer_cache.provider.")
                    || name.starts_with("layer_cache.aggregate.")
            ) {
                return None;
            }
            let mut tagged = invariant.clone();
            tagged.value = format!("{pass}={}", invariant.value);
            Some(crate::eval::model::ObservedItem::Invariant(tagged))
        })
        .collect()
}

#[cfg(test)]
fn apply_layer_cache_import_edit(repo_root: &Path) -> anyhow::Result<()> {
    let app_path = repo_root.join("web/src/app.ts");
    let source = fs::read_to_string(&app_path)
        .with_context(|| format!("read layer-cache fixture {}", app_path.display()))?;
    let edited = source.replace("@billing/rates", "@billing/tax");
    ensure!(
        edited != source,
        "layer-cache fixture import edit did not change {}",
        app_path.display()
    );
    fs::write(&app_path, edited)
        .with_context(|| format!("write layer-cache fixture {}", app_path.display()))
}

#[cfg(test)]
fn apply_module_topology_package_json_edit(repo_root: &Path) -> anyhow::Result<()> {
    let manifest_path = repo_root.join("web/package.json");
    let source = fs::read_to_string(&manifest_path)
        .with_context(|| format!("read module topology fixture {}", manifest_path.display()))?;
    let edited = source.replace("\"^2.3.0\"", "\"^2.4.0\"");
    ensure!(
        edited != source,
        "module topology package.json edit did not change {}",
        manifest_path.display()
    );
    fs::write(&manifest_path, edited)
        .with_context(|| format!("write module topology fixture {}", manifest_path.display()))
}

#[cfg(test)]
fn apply_module_topology_go_mod_edit(repo_root: &Path) -> anyhow::Result<()> {
    let manifest_path = repo_root.join("services/api/go.mod");
    let source = fs::read_to_string(&manifest_path)
        .with_context(|| format!("read module topology fixture {}", manifest_path.display()))?;
    let edited = source.replace("github.com/acme/lib v1.2.3", "github.com/acme/lib v1.2.4");
    ensure!(
        edited != source,
        "module topology go.mod edit did not change {}",
        manifest_path.display()
    );
    fs::write(&manifest_path, edited)
        .with_context(|| format!("write module topology fixture {}", manifest_path.display()))
}

#[cfg(test)]
fn managed_layer_file_count(repo_root: &Path) -> anyhow::Result<usize> {
    let layer_root = repo_root.join(".polint/cache/layers");
    if !layer_root.exists() {
        return Ok(0);
    }
    managed_layer_file_count_in(&layer_root)
}

#[cfg(test)]
fn managed_layer_file_count_in(path: &Path) -> anyhow::Result<usize> {
    let mut count = 0;
    for entry in fs::read_dir(path).with_context(|| format!("read {}", path.display()))? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            count += managed_layer_file_count_in(&path)?;
        } else if file_type.is_file() {
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
fn layer_cache_fixture_invariant(
    name: impl Into<String>,
    value: impl Into<String>,
) -> crate::eval::model::ObservedItem {
    crate::eval::model::ObservedItem::Invariant(crate::eval::model::ObservedInvariant {
        name: name.into(),
        value: value.into(),
        mode: crate::eval::model::AssertionMode::Exact,
        producer_id: Some("polint.eval".to_string()),
        provenance: Some("kernel.run_report.layer_cache.fixture".to_string()),
        precision: Some("exact".to_string()),
        status: Some(crate::eval::model::ObservedStatus::Present),
    })
}

#[cfg(test)]
fn runtime_budget_name(expected: &[crate::eval::model::ExpectedItem], case_id: &str) -> String {
    expected
        .iter()
        .find_map(|item| match item {
            crate::eval::model::ExpectedItem::RuntimeBudget(budget) => Some(budget.name.clone()),
            _ => None,
        })
        .unwrap_or_else(|| case_id.to_string())
}

#[cfg(test)]
fn bool_string(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

#[cfg(test)]
fn saturating_millis(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
fn runtime_observation(
    manifest: &NativeFixtureManifest,
    observed: &[crate::eval::model::ObservedItem],
) -> crate::eval::report::RuntimeObservation {
    let runtime_budget = observed.iter().find_map(|item| match item {
        crate::eval::model::ObservedItem::RuntimeBudget(budget) => Some(budget),
        _ => None,
    });
    crate::eval::report::RuntimeObservation {
        budget_name: runtime_budget
            .map(|budget| budget.name.clone())
            .unwrap_or_else(|| manifest.case_id.clone()),
        budget_passed: runtime_budget
            .map(|budget| budget.budget_passed)
            .unwrap_or(true),
        observed_runtime_ms: runtime_budget.and_then(|budget| budget.observed_runtime_ms),
    }
}

#[cfg(test)]
mod eval_fixture_manifest_tests {
    use std::fs;
    use std::path::Path;

    use super::*;
    use crate::eval::model::{ExpectedItem, FixtureArea};

    fn write_fixture(root: &Path, manifest: &str) {
        fs::create_dir_all(root.join("repo/src")).unwrap();
        fs::write(root.join("repo/src/app.ts"), "export const answer = 42;\n").unwrap();
        fs::write(root.join("expected.polint-eval.toml"), manifest).unwrap();
    }

    #[test]
    fn eval_fixture_manifest_loads_native_fixture_from_expected_toml() {
        let temp = tempfile::tempdir().unwrap();
        let fixture_dir = temp
            .path()
            .join("tests/eval-fixtures/kernel/provider-order");
        write_fixture(
            &fixture_dir,
            r#"
schema_version = "polint-eval-fixture-1"
case_id = "provider-order"
area = "kernel"

[repo]
path = "repo"

[[expected]]
invariant = { name = "provider_order.0", value = "polint.source", mode = "exact" }
"#,
        );

        let fixture = load_native_fixture(&fixture_dir).unwrap();

        assert_eq!(fixture.manifest.schema_version, "polint-eval-fixture-1");
        assert_eq!(fixture.manifest.area, FixtureArea::Kernel);
        assert_eq!(fixture.manifest.case_id, "provider-order");
        assert_eq!(fixture.manifest.repo.path, "repo");
        assert_eq!(fixture.manifest.expected.len(), 1);
        assert!(matches!(
            fixture.manifest.expected.first(),
            Some(ExpectedItem::Invariant(_))
        ));
        assert_eq!(fixture.repo_dir, fixture.fixture_dir.join("repo"));
    }

    #[test]
    fn eval_fixture_manifest_rejects_absolute_repo_path() {
        let temp = tempfile::tempdir().unwrap();
        let fixture_dir = temp
            .path()
            .join("tests/eval-fixtures/kernel/provider-order");
        write_fixture(
            &fixture_dir,
            r#"
schema_version = "polint-eval-fixture-1"
case_id = "provider-order"
area = "kernel"

[repo]
path = "/tmp/polint-outside"
"#,
        );

        let err = load_native_fixture(&fixture_dir).unwrap_err();

        assert!(err.to_string().contains("absolute"));
    }

    #[test]
    fn eval_fixture_manifest_rejects_parent_repo_path() {
        let temp = tempfile::tempdir().unwrap();
        let fixture_dir = temp
            .path()
            .join("tests/eval-fixtures/kernel/provider-order");
        fs::create_dir_all(&fixture_dir).unwrap();
        fs::write(
            fixture_dir.join("expected.polint-eval.toml"),
            r#"
schema_version = "polint-eval-fixture-1"
case_id = "provider-order"
area = "kernel"

[repo]
path = "../outside"
"#,
        )
        .unwrap();

        let err = load_native_fixture(&fixture_dir).unwrap_err();

        assert!(err.to_string().contains("parent directory"));
    }

    #[test]
    fn eval_fixture_manifest_rejects_parent_expected_relative_path() {
        let temp = tempfile::tempdir().unwrap();
        let fixture_dir = temp
            .path()
            .join("tests/eval-fixtures/kernel/provider-order");
        write_fixture(
            &fixture_dir,
            r#"
schema_version = "polint-eval-fixture-1"
case_id = "provider-order"
area = "kernel"

[repo]
path = "repo"

[[expected]]
diagnostic = { rule_id = "local/rule", relative_path = "../outside.ts", line = 1, mode = "exact" }
"#,
        );

        let err = load_native_fixture(&fixture_dir).unwrap_err();

        assert!(err.to_string().contains("parent directory"));
    }

    #[test]
    fn eval_fixture_manifest_rejects_absolute_expected_relative_path() {
        let temp = tempfile::tempdir().unwrap();
        let fixture_dir = temp
            .path()
            .join("tests/eval-fixtures/kernel/provider-order");
        write_fixture(
            &fixture_dir,
            r#"
schema_version = "polint-eval-fixture-1"
case_id = "provider-order"
area = "kernel"

[repo]
path = "repo"

[[expected]]
diagnostic = { rule_id = "local/rule", relative_path = "/tmp/app.ts", line = 1, mode = "exact" }
"#,
        );

        let err = load_native_fixture(&fixture_dir).unwrap_err();

        assert!(err.to_string().contains("absolute"));
    }

    #[test]
    fn eval_fixture_manifest_normalizes_expected_relative_path_separators() {
        let temp = tempfile::tempdir().unwrap();
        let fixture_dir = temp
            .path()
            .join("tests/eval-fixtures/kernel/provider-order");
        write_fixture(
            &fixture_dir,
            r#"
schema_version = "polint-eval-fixture-1"
case_id = "provider-order"
area = "kernel"

[repo]
path = "repo"

[[expected]]
diagnostic = { rule_id = "local/rule", relative_path = "src\\app.ts", line = 1, mode = "exact" }
"#,
        );

        let fixture = load_native_fixture(&fixture_dir).unwrap();

        assert!(matches!(
            fixture.manifest.expected.first(),
            Some(ExpectedItem::Diagnostic(diagnostic)) if diagnostic.relative_path == "src/app.ts"
        ));
    }

    #[test]
    fn eval_fixture_manifest_rejects_alternate_fact_producer_field_name() {
        let temp = tempfile::tempdir().unwrap();
        let fixture_dir = temp.path().join("tests/eval-fixtures/provenance/metadata");
        write_fixture(
            &fixture_dir,
            r#"
schema_version = "polint-eval-fixture-1"
case_id = "provenance-metadata"
area = "provenance"

[repo]
path = "repo"

[[expected]]
fact = { family = "SourceFile", stable_key = "src/app.ts", mode = "partial", provider_id = "polint.source", precision = "exact", status = "present" }
"#,
        );

        let err = load_native_fixture(&fixture_dir).unwrap_err();
        let rendered = format!("{err:#}");

        assert!(
            rendered.contains("provider_id"),
            "alternate producer field should be rejected: {rendered}"
        );
    }

    #[test]
    fn eval_fixture_manifest_rejects_synthetic_observed_rows_outside_extension_area() {
        let temp = tempfile::tempdir().unwrap();
        let fixture_dir = temp.path().join("tests/eval-fixtures/kernel/synthetic");
        write_fixture(
            &fixture_dir,
            r#"
schema_version = "polint-eval-fixture-1"
case_id = "kernel-synthetic"
area = "kernel"
synthetic_observed = true

[repo]
path = "repo"

[[observed]]
invariant = { name = "kernel.synthetic", value = "true", mode = "exact", producer_id = "polint.eval", provenance = "synthetic_observed", precision = "exact", status = "present" }
"#,
        );

        let err = load_native_fixture(&fixture_dir).unwrap_err();
        let rendered = format!("{err:#}");

        assert!(
            rendered.contains("synthetic observed rows are only allowed for extension fixtures"),
            "synthetic observed rows outside extension area should be rejected: {rendered}"
        );
    }
}

#[cfg(test)]
mod eval_native_fixture_runner_tests {
    use std::path::{Path, PathBuf};

    use crate::eval::model::{ExpectedItem, FixtureArea, ObservedItem, ObservedStatus};
    use crate::eval::report::{deterministic_output_hash, to_deterministic_json_pretty};

    use super::*;

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("polint crate should live under crates/")
            .to_path_buf()
    }

    fn provider_order_fixture_dir() -> PathBuf {
        repo_root().join("tests/eval-fixtures/kernel/provider-order")
    }

    fn provenance_fixture_dir() -> PathBuf {
        repo_root().join("tests/eval-fixtures/provenance/metadata")
    }

    fn cache_current_determinism_fixture_dir() -> PathBuf {
        repo_root().join("tests/eval-fixtures/cache/current-determinism")
    }

    fn cache_input_snapshots_fixture_dir() -> PathBuf {
        repo_root().join("tests/eval-fixtures/cache/input-snapshots")
    }

    fn cache_layer_cache_fixture_dir() -> PathBuf {
        repo_root().join("tests/eval-fixtures/cache/layer-cache")
    }

    fn extension_rejection_delta_fixture_dir() -> PathBuf {
        repo_root().join("tests/eval-fixtures/extension/rejection-delta")
    }

    #[test]
    fn eval_native_fixture_runner_provider_order_fixture_passes() {
        let run = run_native_fixture_for_test(&provider_order_fixture_dir()).unwrap();

        assert_eq!(run.metrics.false_negatives, 0);
        assert_eq!(run.metrics.forbidden_hits, 0);
    }

    #[test]
    fn eval_native_fixture_runner_manifest_asserts_all_provider_order_invariants() {
        let fixture = load_native_fixture(&provider_order_fixture_dir()).unwrap();
        let invariants = fixture
            .manifest
            .expected
            .iter()
            .filter_map(|item| match item {
                ExpectedItem::Invariant(invariant) => {
                    Some((invariant.name.as_str(), invariant.value.as_str()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            invariants,
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
                ("provider_order.10", "polint.metrics"),
                (
                    "provider_output.polint.abstract_domains.schema_version",
                    "abstract-domain-facts-1:1",
                ),
            ]
        );
    }

    #[test]
    fn eval_native_fixture_runner_repeated_runs_produce_identical_json() {
        let first = run_native_fixture_for_test(&provider_order_fixture_dir()).unwrap();
        let second = run_native_fixture_for_test(&provider_order_fixture_dir()).unwrap();

        assert_eq!(
            to_deterministic_json_pretty(&first),
            to_deterministic_json_pretty(&second)
        );
    }

    #[test]
    fn eval_native_fixture_runner_repeated_runs_produce_identical_output_hash() {
        let first = run_native_fixture_for_test(&provider_order_fixture_dir()).unwrap();
        let second = run_native_fixture_for_test(&provider_order_fixture_dir()).unwrap();

        assert_eq!(first.output_hash, second.output_hash);
    }

    #[test]
    fn eval_provenance_fixture_passes() {
        let run = run_native_fixture_for_test(&provenance_fixture_dir()).unwrap();
        let case = run.cases.first().expect("provenance case");
        let rendered = to_deterministic_json_pretty(&run);

        assert_eq!(run.metrics.false_negatives, 0);
        assert_eq!(run.metrics.runtime_budget_failed, 0);
        assert!(case.observed.iter().any(|item| match item {
            ObservedItem::Fact(fact) => {
                fact.family == "SourceFile"
                    && fact.stable_key.contains("src/app.ts")
                    && fact.producer_id.as_deref() == Some("polint.source")
                    && fact.precision.as_deref() == Some("exact")
            }
            _ => false,
        }));
        assert!(case.observed.iter().any(|item| match item {
            ObservedItem::Fact(fact) => {
                fact.family == "Import"
                    && fact.stable_key.contains("./message")
                    && fact.producer_id.as_deref() == Some("polint.ts.syntax")
                    && fact.precision.as_deref() == Some("syntax")
            }
            _ => false,
        }));
        assert!(!rendered.contains(repo_root().to_string_lossy().as_ref()));
    }

    #[test]
    fn eval_provenance_fixture_expected_facts_use_producer_id_fields() {
        let fixture = load_native_fixture(&provenance_fixture_dir()).unwrap();
        let manifest =
            std::fs::read_to_string(provenance_fixture_dir().join("expected.polint-eval.toml"))
                .unwrap();
        let expected_facts = fixture
            .manifest
            .expected
            .iter()
            .filter_map(|item| match item {
                ExpectedItem::Fact(fact) => Some((
                    fact.family.as_str(),
                    fact.producer_id.as_deref(),
                    fact.precision.as_deref(),
                )),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            expected_facts,
            vec![
                ("SourceFile", Some("polint.source"), Some("exact")),
                ("Import", Some("polint.ts.syntax"), Some("syntax")),
            ]
        );
        assert!(manifest.contains("producer_id"));
        assert!(!manifest.contains("provider_id"));
    }

    #[test]
    fn eval_provenance_fixture_runtime_budget_hash_ignores_duration() {
        let mut first = run_native_fixture_for_test(&provenance_fixture_dir()).unwrap();
        let mut second = first.clone();
        first.cases[0].runtime.observed_runtime_ms = Some(1);
        second.cases[0].runtime.observed_runtime_ms = Some(999);
        for run in [&mut first, &mut second] {
            let observed_runtime_ms = run.cases[0].runtime.observed_runtime_ms;
            for item in &mut run.cases[0].observed {
                if let ObservedItem::RuntimeBudget(budget) = item {
                    budget.observed_runtime_ms = observed_runtime_ms;
                }
            }
        }

        assert_eq!(
            deterministic_output_hash(&first),
            deterministic_output_hash(&second)
        );
    }

    #[test]
    fn eval_cache_current_determinism_fixture_passes() {
        let run = run_cache_current_determinism_fixture_for_test(
            &cache_current_determinism_fixture_dir(),
        )
        .unwrap();
        let case = run.cases.first().expect("cache determinism case");

        assert_eq!(run.metrics.false_negatives, 0);
        assert_eq!(run.metrics.runtime_budget_failed, 0);
        assert!(case.observed.iter().any(|item| match item {
            ObservedItem::Invariant(invariant) => {
                invariant.name == "cache.current_determinism"
                    && invariant.value == "cold_warm_no_cache_equal"
            }
            _ => false,
        }));
    }

    #[test]
    fn eval_cache_current_determinism_manifest_limits_cache_claims() {
        let manifest = std::fs::read_to_string(
            cache_current_determinism_fixture_dir().join("expected.polint-eval.toml"),
        )
        .unwrap();

        assert!(manifest.contains("cache.current_determinism"));
        assert!(manifest.contains("cold_warm_no_cache_equal"));
        assert!(manifest.contains("max_runtime_ms = 5000"));
        for forbidden in [
            concat!("Input", "Snapshot"),
            concat!("Layer", "Key"),
            concat!("Query", "Key"),
            concat!("Summary", "Key"),
            concat!("Diagnostic", "Key"),
        ] {
            assert!(
                !manifest.contains(forbidden),
                "cache fixture must not claim future cache term `{forbidden}`"
            );
        }
    }

    #[test]
    fn eval_cache_current_determinism_runtime_budget_hash_ignores_duration() {
        let mut first = run_cache_current_determinism_fixture_for_test(
            &cache_current_determinism_fixture_dir(),
        )
        .unwrap();
        let mut second = first.clone();
        first.cases[0].runtime.observed_runtime_ms = Some(1);
        second.cases[0].runtime.observed_runtime_ms = Some(999);
        for run in [&mut first, &mut second] {
            let observed_runtime_ms = run.cases[0].runtime.observed_runtime_ms;
            for item in &mut run.cases[0].observed {
                if let ObservedItem::RuntimeBudget(budget) = item {
                    budget.observed_runtime_ms = observed_runtime_ms;
                }
            }
        }

        assert_eq!(
            deterministic_output_hash(&first),
            deterministic_output_hash(&second)
        );
    }

    #[test]
    fn eval_input_snapshot_fixture_passes() {
        let run = run_native_fixture_for_test(&cache_input_snapshots_fixture_dir()).unwrap();
        let case = run.cases.first().expect("input snapshot case");
        let rendered = to_deterministic_json_pretty(&run);

        assert_eq!(case.case_id, "input-snapshots");
        assert_eq!(case.area, FixtureArea::Cache);
        assert_eq!(run.metrics.false_negatives, 0, "{rendered}");
        assert_eq!(run.metrics.forbidden_hits, 0, "{rendered}");
        assert_eq!(run.metrics.runtime_budget_failed, 0, "{rendered}");
        assert!(!rendered.contains(repo_root().to_string_lossy().as_ref()));
        assert!(!rendered.contains("package payment"));
        assert!(!rendered.contains("export const amount"));
        assert!(!rendered.contains("mtime_hint"));
    }

    #[test]
    fn eval_layer_cache_fixture_passes() {
        let run = run_layer_cache_fixture_for_test(&cache_layer_cache_fixture_dir()).unwrap();
        let case = run.cases.first().expect("layer cache case");
        let rendered = to_deterministic_json_pretty(&run);

        assert_eq!(case.case_id, "layer-cache");
        assert_eq!(case.area, FixtureArea::Cache);
        assert_eq!(run.metrics.false_negatives, 0, "{rendered}");
        assert_eq!(run.metrics.forbidden_hits, 0, "{rendered}");
        assert_eq!(run.metrics.runtime_budget_failed, 0, "{rendered}");
        assert!(!rendered.contains(repo_root().to_string_lossy().as_ref()));
        assert!(!rendered.contains("package payment"));
        assert!(!rendered.contains("export function charge"));
    }

    #[test]
    fn eval_extension_synthetic_rejection_delta_fixture_passes() {
        let run = run_native_fixture_for_test(&extension_rejection_delta_fixture_dir()).unwrap();
        let case = run.cases.first().expect("extension rejection-delta case");
        let rendered = to_deterministic_json_pretty(&run);

        assert_eq!(run.metrics.false_negatives, 0);
        assert_eq!(run.metrics.runtime_budget_failed, 0);
        assert_eq!(run.metrics.false_positive_trap_hits, 1);
        assert!(rendered.contains("\"facts_accepted\": 1"));
        assert!(rendered.contains("\"facts_rejected\": 1"));
        assert!(case.observed.iter().any(|item| match item {
            ObservedItem::Fact(fact) => {
                fact.stable_key == "extension.synthetic_accepted_fact"
                    && fact.status == Some(ObservedStatus::Accepted)
            }
            _ => false,
        }));
        assert!(case.observed.iter().any(|item| match item {
            ObservedItem::Fact(fact) => {
                fact.stable_key == "extension.synthetic_rejected_fact"
                    && fact.status == Some(ObservedStatus::Rejected)
            }
            _ => false,
        }));
    }

    #[test]
    fn eval_extension_synthetic_fixture_covers_all_item_kinds() {
        let run = run_native_fixture_for_test(&extension_rejection_delta_fixture_dir()).unwrap();
        let case = run.cases.first().expect("extension rejection-delta case");

        assert_eq!(
            item_kinds(&case.expected),
            vec![
                "Diagnostic",
                "Fact",
                "GraphEdge",
                "Invariant",
                "Path",
                "RuntimeBudget",
            ]
        );
        assert_eq!(
            observed_item_kinds(&case.observed),
            vec![
                "Diagnostic",
                "Fact",
                "GraphEdge",
                "Invariant",
                "Path",
                "RuntimeBudget",
            ]
        );
    }

    #[test]
    fn eval_extension_synthetic_delta_uses_normalized_invariants() {
        let run = run_native_fixture_for_test(&extension_rejection_delta_fixture_dir()).unwrap();
        let case = run.cases.first().expect("extension rejection-delta case");

        assert!(case.observed.iter().any(|item| match item {
            ObservedItem::Invariant(invariant) => {
                invariant.name == "extension.default_vs_extension_delta"
                    && invariant.value == "changed_facts=2,rejected=1"
            }
            _ => false,
        }));
        assert!(case.observed.iter().any(|item| match item {
            ObservedItem::Invariant(invariant) => {
                invariant.name == "extension.real_sink_active" && invariant.value == "false"
            }
            _ => false,
        }));
    }

    #[test]
    fn eval_native_fixture_suite_covers_required_categories() {
        let fixture_dirs = collect_native_fixture_dirs(&repo_root().join("tests/eval-fixtures"));
        let mut passing_by_area = std::collections::BTreeMap::<FixtureArea, Vec<String>>::new();

        assert!(
            !fixture_dirs.is_empty(),
            "native fixture suite should contain fixture manifests"
        );

        for fixture_dir in fixture_dirs {
            let run = run_fixture_for_suite_coverage(&fixture_dir).unwrap_or_else(|error| {
                panic!(
                    "native fixture should run: {}\n{error:#}",
                    fixture_dir.display()
                )
            });
            let case = run
                .cases
                .first()
                .expect("native fixture run should have a case");

            assert_eq!(
                run.metrics.false_negatives, 0,
                "fixture should not miss expected rows: {}",
                case.case_id
            );
            assert_eq!(
                run.metrics.forbidden_hits, 0,
                "fixture should not hit forbidden rows: {}",
                case.case_id
            );
            assert_eq!(
                run.metrics.runtime_budget_failed, 0,
                "fixture should stay inside runtime budget: {}",
                case.case_id
            );

            passing_by_area
                .entry(case.area)
                .or_default()
                .push(case.case_id.clone());
        }

        for required_area in [
            FixtureArea::Kernel,
            FixtureArea::Provenance,
            FixtureArea::Cache,
            FixtureArea::Extension,
        ] {
            assert!(
                passing_by_area.contains_key(&required_area),
                "native fixture suite must include a passing {required_area:?} fixture; found {passing_by_area:#?}"
            );
        }
    }

    fn item_kinds(items: &[ExpectedItem]) -> Vec<&'static str> {
        let mut kinds = items
            .iter()
            .map(|item| match item {
                ExpectedItem::Diagnostic(_) => "Diagnostic",
                ExpectedItem::Fact(_) => "Fact",
                ExpectedItem::GraphEdge(_) => "GraphEdge",
                ExpectedItem::Path(_) => "Path",
                ExpectedItem::Invariant(_) => "Invariant",
                ExpectedItem::RuntimeBudget(_) => "RuntimeBudget",
            })
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        kinds.sort();
        kinds
    }

    fn observed_item_kinds(items: &[ObservedItem]) -> Vec<&'static str> {
        let mut kinds = items
            .iter()
            .map(|item| match item {
                ObservedItem::Diagnostic(_) => "Diagnostic",
                ObservedItem::Fact(_) => "Fact",
                ObservedItem::GraphEdge(_) => "GraphEdge",
                ObservedItem::Path(_) => "Path",
                ObservedItem::Invariant(_) => "Invariant",
                ObservedItem::RuntimeBudget(_) => "RuntimeBudget",
            })
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        kinds.sort();
        kinds
    }

    fn collect_native_fixture_dirs(root: &Path) -> Vec<PathBuf> {
        let mut fixture_dirs = Vec::new();
        collect_native_fixture_dirs_in(root, &mut fixture_dirs);
        fixture_dirs.sort();
        fixture_dirs
    }

    fn collect_native_fixture_dirs_in(current: &Path, fixture_dirs: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(current).unwrap_or_else(|error| {
            panic!("read native fixture dir {}: {error}", current.display())
        }) {
            let path = entry
                .unwrap_or_else(|error| panic!("read native fixture entry: {error}"))
                .path();
            if path.is_dir() {
                collect_native_fixture_dirs_in(&path, fixture_dirs);
            } else if path
                .file_name()
                .is_some_and(|name| name == std::ffi::OsStr::new(FIXTURE_MANIFEST_FILE))
            {
                fixture_dirs.push(current.to_path_buf());
            }
        }
    }

    fn run_fixture_for_suite_coverage(
        fixture_dir: &Path,
    ) -> anyhow::Result<crate::eval::report::EvaluationRun> {
        let fixture = load_native_fixture(fixture_dir)?;
        if fixture.manifest.area == FixtureArea::Cache
            && fixture.manifest.case_id == "cache-current-determinism"
        {
            run_cache_current_determinism_fixture_for_test(fixture_dir)
        } else if fixture.manifest.area == FixtureArea::Cache
            && fixture.manifest.case_id == "layer-cache"
        {
            run_layer_cache_fixture_for_test(fixture_dir)
        } else if fixture.manifest.area == FixtureArea::SemanticIndex
            && fixture.manifest.case_id == "semantic-index-core"
        {
            run_semantic_index_core_fixture_for_test(fixture_dir)
        } else if fixture.manifest.area == FixtureArea::ModuleTopology
            && fixture.manifest.case_id == "module-topology-core"
        {
            run_module_topology_core_fixture_for_test(fixture_dir)
        } else if fixture.manifest.area == FixtureArea::SemanticMir
            && fixture.manifest.case_id == "semantic-mir-core"
        {
            run_semantic_mir_core_fixture_for_test(fixture_dir)
        } else if fixture.manifest.area == FixtureArea::Cfg
            && fixture.manifest.case_id == "cfg-core"
        {
            run_cfg_core_fixture_for_test(fixture_dir)
        } else if fixture.manifest.area == FixtureArea::DirectCalls
            && fixture.manifest.case_id == "direct-calls-core"
        {
            run_direct_calls_core_fixture_for_test(fixture_dir)
        } else {
            run_native_fixture_for_test(fixture_dir)
        }
    }
}

#[cfg(test)]
mod semantic_index_core {
    use std::path::{Path, PathBuf};

    use crate::eval::model::{ExpectedItem, FixtureArea, ObservedItem, ObservedStatus};
    use crate::eval::report::to_deterministic_json_pretty;

    use super::*;

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("polint crate should live under crates/")
            .to_path_buf()
    }

    fn fixture_dir() -> PathBuf {
        repo_root().join("tests/eval-fixtures/semantic-index/core")
    }

    #[test]
    fn eval_semantic_index_core_fixture_passes() {
        let run = run_semantic_index_core_fixture_for_test(&fixture_dir()).unwrap();
        let case = run.cases.first().expect("semantic-index core case");
        let rendered = to_deterministic_json_pretty(&run);

        assert_eq!(case.case_id, "semantic-index-core");
        assert_eq!(case.area, FixtureArea::SemanticIndex);
        assert_eq!(run.metrics.false_negatives, 0, "{rendered}");
        assert_eq!(run.metrics.forbidden_hits, 0, "{rendered}");
        assert_eq!(run.metrics.runtime_budget_failed, 0, "{rendered}");
        assert!(!rendered.contains(repo_root().to_string_lossy().as_ref()));
    }

    #[test]
    fn eval_semantic_index_core_manifest_covers_required_taxonomy() {
        let fixture = load_native_fixture(&fixture_dir()).unwrap();
        let expected_facts = fixture
            .manifest
            .expected
            .iter()
            .filter_map(|item| match item {
                ExpectedItem::Fact(fact) => Some((
                    fact.family.as_str(),
                    fact.status,
                    fact.precision.as_deref(),
                    fact.producer_id.as_deref(),
                )),
                _ => None,
            })
            .collect::<Vec<_>>();

        for required in [
            ("Reference", Some(ObservedStatus::Ambiguous)),
            ("Reference", Some(ObservedStatus::Unresolved)),
            ("SemanticImport", Some(ObservedStatus::Dynamic)),
            ("Alias", Some(ObservedStatus::Resolved)),
            ("Export", Some(ObservedStatus::Resolved)),
            ("GeneratedSymbol", Some(ObservedStatus::Generated)),
            ("StableExport", Some(ObservedStatus::Resolved)),
        ] {
            assert!(
                expected_facts
                    .iter()
                    .any(|(family, status, _, _)| *family == required.0 && *status == required.1),
                "semantic fixture missing expected {required:?}: {expected_facts:#?}"
            );
        }
        assert!(
            expected_facts
                .iter()
                .any(|(_, status, precision, producer)| {
                    matches!(
                        status,
                        Some(
                            ObservedStatus::Ambiguous
                                | ObservedStatus::Unresolved
                                | ObservedStatus::Dynamic
                                | ObservedStatus::Generated
                        )
                    ) && precision.is_some()
                        && *producer == Some("polint.symbol_graph")
                })
        );
    }

    #[test]
    fn eval_semantic_index_core_observes_unknown_semantic_statuses() {
        let run = run_semantic_index_core_fixture_for_test(&fixture_dir()).unwrap();
        let case = run.cases.first().expect("semantic-index core case");

        for status in [
            ObservedStatus::Ambiguous,
            ObservedStatus::Unresolved,
            ObservedStatus::Dynamic,
            ObservedStatus::Generated,
        ] {
            assert!(
                case.observed.iter().any(|item| match item {
                    ObservedItem::Fact(fact) => fact.status == Some(status),
                    _ => false,
                }),
                "semantic fixture should observe {status:?}: {:#?}",
                case.observed
            );
        }
    }
}

#[cfg(test)]
mod module_topology_core {
    use std::path::{Path, PathBuf};

    use crate::eval::model::{ExpectedItem, FixtureArea, ObservedItem, ObservedStatus};
    use crate::eval::report::to_deterministic_json_pretty;

    use super::*;

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("polint crate should live under crates/")
            .to_path_buf()
    }

    fn fixture_dir() -> PathBuf {
        repo_root().join("tests/eval-fixtures/module-topology/core")
    }

    #[test]
    fn eval_module_topology_core_fixture_passes() {
        let run = run_module_topology_core_fixture_for_test(&fixture_dir()).unwrap();
        let case = run.cases.first().expect("module-topology core case");
        let rendered = to_deterministic_json_pretty(&run);

        assert_eq!(case.case_id, "module-topology-core");
        assert_eq!(case.area, FixtureArea::ModuleTopology);
        assert_eq!(run.metrics.false_negatives, 0, "{rendered}");
        assert_eq!(run.metrics.forbidden_hits, 0, "{rendered}");
        assert_eq!(run.metrics.runtime_budget_failed, 0, "{rendered}");
        assert!(!rendered.contains(repo_root().to_string_lossy().as_ref()));
    }

    #[test]
    fn eval_module_topology_core_manifest_covers_required_taxonomy() {
        let fixture = load_native_fixture(&fixture_dir()).unwrap();
        let expected_facts = fixture
            .manifest
            .expected
            .iter()
            .filter_map(|item| match item {
                ExpectedItem::Fact(fact) => {
                    Some((fact.family.as_str(), fact.stable_key.as_str(), fact.status))
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        for required in [
            (
                "WorkspaceRoot",
                "services/api",
                Some(ObservedStatus::Present),
            ),
            ("TopologyPackage", "web", Some(ObservedStatus::Present)),
            (
                "SourceSet",
                "web/src/app.test.ts",
                Some(ObservedStatus::Present),
            ),
            (
                "SourceSet",
                "web/generated/client.generated.ts",
                Some(ObservedStatus::Present),
            ),
            (
                "DependencyRequirement",
                "github.com/acme/lib",
                Some(ObservedStatus::Present),
            ),
            (
                "DependencyRequirement",
                "@scope/lib",
                Some(ObservedStatus::Present),
            ),
            (
                "ResolvedDependencyEdge",
                "@scope/lib",
                Some(ObservedStatus::Resolved),
            ),
            (
                "ImportToPackage",
                "@scope/lib",
                Some(ObservedStatus::External),
            ),
            (
                "RepoTopologyOverlay",
                "generated-zone",
                Some(ObservedStatus::Present),
            ),
            (
                "RepoTopologyOverlay",
                "test-only-visibility",
                Some(ObservedStatus::Present),
            ),
        ] {
            assert!(
                expected_facts.iter().any(|(family, key, status)| {
                    *family == required.0 && key.contains(required.1) && *status == required.2
                }),
                "module topology fixture missing expected {required:?}: {expected_facts:#?}"
            );
        }
    }

    #[test]
    fn eval_module_topology_core_observes_cache_and_edit_invalidation() {
        let run = run_module_topology_core_fixture_for_test(&fixture_dir()).unwrap();
        let case = run.cases.first().expect("module-topology core case");

        for required in [
            ("layer_cache.provider.polint.module_graph.hits", "warm=1"),
            ("layer_cache.provider.polint.module_topology.hits", "warm=1"),
            (
                "layer_cache.provider.polint.module_graph.misses",
                "package_json_edit=1",
            ),
            (
                "layer_cache.provider.polint.module_topology.misses",
                "go_mod_edit=1",
            ),
        ] {
            assert!(
                case.observed.iter().any(|item| match item {
                    ObservedItem::Invariant(invariant) => {
                        invariant.name == required.0 && invariant.value == required.1
                    }
                    _ => false,
                }),
                "module topology fixture should observe cache invariant {required:?}: {:#?}",
                case.observed
            );
        }
    }
}

#[cfg(test)]
mod semantic_mir_core {
    use std::path::{Path, PathBuf};

    use crate::eval::model::{ExpectedItem, FixtureArea, ObservedItem, ObservedStatus};
    use crate::eval::report::to_deterministic_json_pretty;

    use super::*;

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("polint crate should live under crates/")
            .to_path_buf()
    }

    fn fixture_dir() -> PathBuf {
        repo_root().join("tests/eval-fixtures/semantic-mir/core")
    }

    #[test]
    fn eval_semantic_mir_core_fixture_passes() {
        let run = run_semantic_mir_core_fixture_for_test(&fixture_dir()).unwrap();
        let case = run.cases.first().expect("semantic-MIR core case");
        let rendered = to_deterministic_json_pretty(&run);

        assert_eq!(case.case_id, "semantic-mir-core");
        assert_eq!(case.area, FixtureArea::SemanticMir);
        assert_eq!(run.metrics.false_negatives, 0, "{rendered}");
        assert_eq!(run.metrics.forbidden_hits, 0, "{rendered}");
        assert_eq!(run.metrics.runtime_budget_failed, 0, "{rendered}");
        assert!(!rendered.contains(repo_root().to_string_lossy().as_ref()));
    }

    #[test]
    fn eval_semantic_mir_core_manifest_covers_required_taxonomy() {
        let fixture = load_native_fixture(&fixture_dir()).unwrap();
        let expected_facts = fixture
            .manifest
            .expected
            .iter()
            .filter_map(|item| match item {
                ExpectedItem::Fact(fact) => Some((
                    fact.family.as_str(),
                    fact.stable_key.as_str(),
                    fact.status,
                    fact.precision.as_deref(),
                )),
                _ => None,
            })
            .collect::<Vec<_>>();

        for required in [
            ("MirBody", "service.go", Some(ObservedStatus::Partial)),
            ("MirBody", "app.ts", Some(ObservedStatus::Partial)),
            ("MirOperation", "call", Some(ObservedStatus::Partial)),
            ("Place", "parameter", Some(ObservedStatus::Resolved)),
            ("Place", "local", Some(ObservedStatus::Resolved)),
            ("Place", "global", Some(ObservedStatus::Partial)),
            ("Place", "temporary", Some(ObservedStatus::Partial)),
            ("Place", "call_return", Some(ObservedStatus::Partial)),
            ("Place", "unknown", Some(ObservedStatus::Unknown)),
            (
                "UnsupportedSemantic",
                "defer_statement",
                Some(ObservedStatus::Unsupported),
            ),
            (
                "UnsupportedSemantic",
                "eval",
                Some(ObservedStatus::Unsupported),
            ),
        ] {
            assert!(
                expected_facts.iter().any(|(family, key, status, _)| {
                    *family == required.0 && key.contains(required.1) && *status == required.2
                }),
                "semantic MIR fixture missing expected {required:?}: {expected_facts:#?}"
            );
        }
        assert!(expected_facts.iter().any(|(_, _, _, precision)| {
            matches!(
                *precision,
                Some("setup_aware" | "heuristic" | "unsupported")
            )
        }));
    }

    #[test]
    fn eval_semantic_mir_core_observes_unknown_and_unsupported_rows() {
        let run = run_semantic_mir_core_fixture_for_test(&fixture_dir()).unwrap();
        let case = run.cases.first().expect("semantic-MIR core case");

        for status in [
            ObservedStatus::Partial,
            ObservedStatus::Unknown,
            ObservedStatus::Unsupported,
        ] {
            assert!(
                case.observed.iter().any(|item| match item {
                    ObservedItem::Fact(fact) => fact.status == Some(status),
                    _ => false,
                }),
                "semantic MIR fixture should observe {status:?}: {:#?}",
                case.observed
            );
        }
        assert!(case.observed.iter().any(|item| match item {
            ObservedItem::Invariant(invariant) => {
                invariant.name == "semantic_mir.current_determinism"
                    && invariant.value == "cold_warm_equal"
            }
            _ => false,
        }));
    }
}

#[cfg(test)]
mod cfg_core {
    use std::path::{Path, PathBuf};

    use crate::eval::model::{
        CFG_FACT_FAMILIES, ExpectedItem, FixtureArea, ObservedItem, ObservedStatus,
    };
    use crate::eval::report::to_deterministic_json_pretty;

    use super::*;

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("polint crate should live under crates/")
            .to_path_buf()
    }

    fn fixture_dir() -> PathBuf {
        repo_root().join("tests/eval-fixtures/cfg-core")
    }

    #[test]
    fn eval_cfg_core_fixture_passes() {
        let run = run_cfg_core_fixture_for_test(&fixture_dir()).unwrap();
        let case = run.cases.first().expect("CFG core case");
        let rendered = to_deterministic_json_pretty(&run);

        assert_eq!(case.case_id, "cfg-core");
        assert_eq!(case.area, FixtureArea::Cfg);
        assert_eq!(run.metrics.false_negatives, 0, "{rendered}");
        assert_eq!(run.metrics.forbidden_hits, 0, "{rendered}");
        assert_eq!(run.metrics.runtime_budget_failed, 0, "{rendered}");
        assert!(!rendered.contains(repo_root().to_string_lossy().as_ref()));
        assert!(!rendered.contains("package cfgcore"));
        assert!(!rendered.contains("export function route"));
    }

    #[test]
    fn eval_cfg_core_manifest_covers_required_taxonomy() {
        let fixture = load_native_fixture(&fixture_dir()).unwrap();
        let expected_facts = fixture
            .manifest
            .expected
            .iter()
            .filter_map(|item| match item {
                ExpectedItem::Fact(fact) => Some((
                    fact.family.as_str(),
                    fact.stable_key.as_str(),
                    fact.status,
                    fact.precision.as_deref(),
                )),
                _ => None,
            })
            .collect::<Vec<_>>();

        for family in CFG_FACT_FAMILIES {
            assert!(
                expected_facts
                    .iter()
                    .any(|(observed, _, _, _)| observed == family),
                "CFG fixture missing expected family {family}: {expected_facts:#?}"
            );
        }
        assert!(
            expected_facts
                .iter()
                .any(|(family, key, status, precision)| {
                    *family == "UnsupportedControlFlow"
                        && key.contains("throw")
                        && *status == Some(ObservedStatus::Unsupported)
                        && *precision == Some("unsupported")
                })
        );
    }

    #[test]
    fn eval_cfg_core_observes_required_families_and_determinism() {
        let run = run_cfg_core_fixture_for_test(&fixture_dir()).unwrap();
        let case = run.cases.first().expect("CFG core case");

        for family in CFG_FACT_FAMILIES {
            assert!(
                case.observed.iter().any(|item| match item {
                    ObservedItem::Fact(fact) => fact.family == *family,
                    _ => false,
                }),
                "CFG fixture should observe {family}: {:#?}",
                case.observed
            );
        }
        assert!(case.observed.iter().any(|item| match item {
            ObservedItem::Invariant(invariant) => {
                invariant.name == "cfg.current_determinism"
                    && invariant.value == "cold_warm_no_cache_equal"
            }
            _ => false,
        }));
    }
}

#[cfg(test)]
mod direct_calls_core {
    use std::path::{Path, PathBuf};

    use crate::eval::model::{ExpectedItem, FixtureArea, ObservedItem, ObservedStatus};
    use crate::eval::report::to_deterministic_json_pretty;

    use super::*;

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("polint crate should live under crates/")
            .to_path_buf()
    }

    fn fixture_dir() -> PathBuf {
        repo_root().join("tests/eval-fixtures/direct-calls/core")
    }

    const REQUIRED_COUNT_INVARIANTS: &[&str] = &[
        "direct_calls.counts.by_language.Go.nonzero",
        "direct_calls.counts.by_language.TypeScript.nonzero",
        "direct_calls.counts.by_call_kind.Function.nonzero",
        "direct_calls.counts.by_call_kind.Member.nonzero",
        "direct_calls.counts.by_call_kind.Constructor.nonzero",
        "direct_calls.counts.by_algorithm.DirectReference.nonzero",
        "direct_calls.counts.by_algorithm.ImportBinding.nonzero",
        "direct_calls.counts.by_algorithm.ConstructorBinding.nonzero",
        "direct_calls.counts.by_algorithm.DirectMember.nonzero",
        "direct_calls.counts.by_status.Resolved.nonzero",
        "direct_calls.counts.by_status.Unresolved.nonzero",
        "direct_calls.counts.by_status.Unsupported.nonzero",
        "direct_calls.counts.by_status.SetupMissing.nonzero",
        "direct_calls.counts.by_unresolved_reason.FunctionValue.nonzero",
        "direct_calls.counts.by_unresolved_reason.DynamicProperty.nonzero",
        "direct_calls.counts.by_unresolved_reason.Reflection.nonzero",
        "direct_calls.counts.by_unresolved_reason.GoroutineBoundary.nonzero",
        "direct_calls.counts.by_unresolved_reason.Eval.nonzero",
        "direct_calls.counts.by_unresolved_reason.DynamicImport.nonzero",
        "direct_calls.counts.by_unresolved_reason.CallApplyBind.nonzero",
        "direct_calls.counts.by_unresolved_reason.SetupMissing.nonzero",
        "direct_calls.counts.by_provider.polint.calls.nonzero",
    ];

    const REQUIRED_INDEX_INVARIANTS: &[&str] = &[
        "direct_calls.index_counts.outgoing_by_function.nonzero",
        "direct_calls.index_counts.outgoing_by_symbol.nonzero",
        "direct_calls.index_counts.incoming_by_symbol.nonzero",
        "direct_calls.index_counts.incoming_by_function.nonzero",
        "direct_calls.index_counts.unresolved_by_reason.nonzero",
        "direct_calls.index_counts.unresolved_by_status.nonzero",
    ];

    #[test]
    fn eval_direct_calls_core_fixture_passes() {
        let run = run_direct_calls_core_fixture_for_test(&fixture_dir()).unwrap();
        let case = run.cases.first().expect("direct calls core case");
        let rendered = to_deterministic_json_pretty(&run);

        assert_eq!(case.case_id, "direct-calls-core");
        assert_eq!(case.area, FixtureArea::DirectCalls);
        assert_eq!(run.metrics.false_negatives, 0, "{rendered}");
        assert_eq!(run.metrics.forbidden_hits, 0, "{rendered}");
        assert_eq!(run.metrics.runtime_budget_failed, 0, "{rendered}");
        assert!(!rendered.contains(repo_root().to_string_lossy().as_ref()));
        assert!(!rendered.contains("package directcalls"));
        assert!(!rendered.contains("export function handler"));
    }

    #[test]
    fn eval_direct_calls_core_manifest_covers_required_taxonomy() {
        let fixture = load_native_fixture(&fixture_dir()).unwrap();
        let expected_facts = fixture
            .manifest
            .expected
            .iter()
            .filter_map(|item| match item {
                ExpectedItem::Fact(fact) => Some((
                    fact.family.as_str(),
                    fact.stable_key.as_str(),
                    fact.status,
                    fact.precision.as_deref(),
                    fact.producer_id.as_deref(),
                )),
                _ => None,
            })
            .collect::<Vec<_>>();

        for required in [
            ("CallSite", "directFunction", Some(ObservedStatus::Resolved)),
            ("CallSite", "handler", Some(ObservedStatus::Resolved)),
            (
                "CallTarget",
                "DirectReference",
                Some(ObservedStatus::Resolved),
            ),
            (
                "CallTarget",
                "ImportBinding",
                Some(ObservedStatus::Resolved),
            ),
            ("CallTarget", "Constructor", Some(ObservedStatus::Resolved)),
            ("CallTarget", "DirectMember", Some(ObservedStatus::Resolved)),
            (
                "UnresolvedCall",
                "FunctionValue",
                Some(ObservedStatus::Unresolved),
            ),
            (
                "UnresolvedCall",
                "DynamicProperty",
                Some(ObservedStatus::Unresolved),
            ),
            (
                "UnresolvedCall",
                "Reflection",
                Some(ObservedStatus::Unsupported),
            ),
            (
                "UnresolvedCall",
                "GoroutineBoundary",
                Some(ObservedStatus::Unsupported),
            ),
            ("UnresolvedCall", "Eval", Some(ObservedStatus::Unresolved)),
            (
                "UnresolvedCall",
                "DynamicImport",
                Some(ObservedStatus::Unresolved),
            ),
            (
                "UnresolvedCall",
                "CallApplyBind",
                Some(ObservedStatus::Unresolved),
            ),
            (
                "UnresolvedCall",
                "SetupMissing",
                Some(ObservedStatus::SetupMissing),
            ),
        ] {
            assert!(
                expected_facts
                    .iter()
                    .any(|(family, key, status, _, producer)| {
                        *family == required.0
                            && key.contains(required.1)
                            && *status == required.2
                            && *producer == Some("polint.calls")
                    }),
                "direct-call fixture missing expected {required:?}: {expected_facts:#?}"
            );
        }
        assert!(expected_facts.iter().any(|(_, _, _, precision, _)| {
            matches!(*precision, Some("setup_aware" | "unknown" | "unsupported"))
        }));
    }

    #[test]
    fn eval_direct_calls_core_manifest_covers_counts_and_indexes() {
        let fixture = load_native_fixture(&fixture_dir()).unwrap();
        let expected_invariants = fixture
            .manifest
            .expected
            .iter()
            .filter_map(|item| match item {
                ExpectedItem::Invariant(invariant) => Some(invariant.name.as_str()),
                _ => None,
            })
            .collect::<std::collections::BTreeSet<_>>();

        for required in REQUIRED_COUNT_INVARIANTS
            .iter()
            .chain(REQUIRED_INDEX_INVARIANTS)
        {
            assert!(
                expected_invariants.contains(required),
                "direct-call fixture manifest missing invariant {required}"
            );
        }
    }

    #[test]
    fn eval_direct_calls_core_observes_required_families_and_determinism() {
        let run = run_direct_calls_core_fixture_for_test(&fixture_dir()).unwrap();
        let case = run.cases.first().expect("direct calls core case");

        for family in crate::eval::model::CALL_FACT_FAMILIES {
            assert!(
                case.observed.iter().any(|item| match item {
                    ObservedItem::Fact(fact) => fact.family == *family,
                    _ => false,
                }),
                "direct-call fixture should observe {family}: {:#?}",
                case.observed
            );
        }
        assert!(case.observed.iter().any(|item| match item {
            ObservedItem::Invariant(invariant) => {
                invariant.name == "direct_calls.current_determinism"
                    && invariant.value == "cold_warm_no_cache_equal"
            }
            _ => false,
        }));
    }

    #[test]
    fn eval_direct_calls_core_observes_counts_and_indexes() {
        let run = run_direct_calls_core_fixture_for_test(&fixture_dir()).unwrap();
        let case = run.cases.first().expect("direct calls core case");

        for required in REQUIRED_COUNT_INVARIANTS
            .iter()
            .chain(REQUIRED_INDEX_INVARIANTS)
        {
            assert!(
                case.observed.iter().any(|item| match item {
                    ObservedItem::Invariant(invariant) => {
                        invariant.name == *required && invariant.value == "true"
                    }
                    _ => false,
                }),
                "direct-call fixture should observe invariant {required}: {:#?}",
                case.observed
            );
        }
    }
}

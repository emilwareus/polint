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
        && cache_comparison_json(&cold_run) == cache_comparison_json(&no_cache_run)
        && cold_run.output_hash == warm_run.output_hash
        && cold_run.output_hash == no_cache_run.output_hash;

    let mut observed = no_cache_observed;
    if cold_warm_no_cache_equal {
        observed.push(cache_current_determinism_observed_invariant());
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
        for item in &mut case.observed {
            if let crate::eval::model::ObservedItem::RuntimeBudget(budget) = item {
                budget.observed_runtime_ms = None;
            }
        }
        for summary in &mut case.matches {
            summary.observed_runtime_ms = None;
        }
    }
    normalized
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

    use crate::eval::model::{ExpectedItem, ObservedItem, ObservedStatus};
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
                ("provider_order.5", "polint.metrics"),
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
        panic!("native fixture category coverage proof is not implemented yet");
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
}

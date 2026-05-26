#[cfg(test)]
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::eval::adapter::BenchmarkAdapter;
#[cfg(test)]
use crate::eval::adapter::PreparedCase;
#[cfg(test)]
use crate::eval::competitors::{BenchmarkComparisonRow, ProductIdentity, ResultSource};
use crate::eval::matcher::{MatcherConfig, match_case};
use crate::eval::metrics::compute_metrics;
#[cfg(test)]
use crate::eval::model::{AssertionMode, ObservedFact, ObservedItem, ObservedStatus};
use crate::eval::model::{EvaluationCase, EvaluationMode};
use crate::eval::report::{
    CaseResult, EVALUATION_SCHEMA_VERSION, EvaluationRun, RuntimeObservation,
    deterministic_output_hash, normalize_run,
};
use crate::eval::suite::{SuiteLanguageSupport, SuiteManifest, SuiteTier};
use crate::eval::tiers::{TierSelection, select_case_ids};

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EvalRunArtifacts {
    pub(crate) json_path: PathBuf,
    pub(crate) markdown_path: PathBuf,
    pub(crate) output_hash: String,
}

#[derive(Clone, Debug)]
pub(crate) struct SuiteRunRequest<'a> {
    pub(crate) manifest: &'a SuiteManifest,
    pub(crate) tier: SuiteTier,
    pub(crate) mode: EvaluationMode,
    pub(crate) candidate_case_ids: Vec<String>,
    pub(crate) run_polint_analysis: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct SuiteRunPlan {
    pub(crate) suite_id: String,
    pub(crate) tier: SuiteTier,
    pub(crate) mode: EvaluationMode,
    pub(crate) language_support: SuiteLanguageSupport,
    pub(crate) should_run_polint_analysis: bool,
    pub(crate) selection: TierSelection,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) limitations: Vec<String>,
}

pub(crate) fn plan_suite_run(request: SuiteRunRequest<'_>) -> anyhow::Result<SuiteRunPlan> {
    request.manifest.validate()?;
    let selection = select_case_ids(request.manifest, request.tier, &request.candidate_case_ids)?;
    let should_run_polint_analysis = request.run_polint_analysis
        && request.manifest.language_support == SuiteLanguageSupport::Supported;
    let mut limitations = selection.limitations.clone();

    if request.manifest.language_support != SuiteLanguageSupport::Supported {
        limitations.push(format!(
            "suite {} is {:?}; polint analysis is disabled",
            request.manifest.id.0, request.manifest.language_support
        ));
    }
    if request.mode == EvaluationMode::AdapterOnly {
        limitations.push("adapter_only mode does not run scanner analysis".to_string());
    }

    Ok(SuiteRunPlan {
        suite_id: request.manifest.id.0.clone(),
        tier: request.tier,
        mode: request.mode,
        language_support: request.manifest.language_support,
        should_run_polint_analysis: should_run_polint_analysis
            && request.mode != EvaluationMode::AdapterOnly,
        selection,
        limitations,
    })
}

pub(crate) fn build_report_for_cases<A: BenchmarkAdapter>(
    adapter: &A,
    manifest: &SuiteManifest,
    plan: &SuiteRunPlan,
    cases: &[EvaluationCase],
) -> anyhow::Result<EvaluationRun> {
    let selected: std::collections::BTreeSet<_> = plan.selection.selected_case_ids.iter().collect();
    let mut case_results = Vec::new();
    for case in cases.iter().filter(|case| selected.contains(&case.case_id)) {
        let observed = case.observed.clone();
        let matches = match_case(&case.expected, &observed, MatcherConfig::default());
        case_results.push(CaseResult {
            case_id: case.case_id.clone(),
            area: case.area,
            expected: case.expected.clone(),
            observed,
            matches,
            runtime: RuntimeObservation {
                budget_name: "eval-runner".to_string(),
                budget_passed: true,
                observed_runtime_ms: None,
            },
        });
    }

    let all_matches = case_results
        .iter()
        .flat_map(|case| case.matches.iter().cloned())
        .collect::<Vec<_>>();
    let mut metrics = crate::eval::report::MetricSummary::from(compute_metrics(&all_matches));
    metrics.sections.suite_native = adapter.suite_native_metrics(manifest, &case_results)?;
    let mut run = EvaluationRun {
        schema_version: EVALUATION_SCHEMA_VERSION.to_string(),
        suite_id: manifest.id.0.clone(),
        mode: plan.mode,
        suite_manifest: Some(manifest.clone()),
        cases: case_results,
        metrics,
        performance: None,
        comparison_rows: Vec::new(),
        adaptation: None,
        adaptation_delta: None,
        limitations: plan.limitations.clone(),
        output_hash: String::new(),
    };
    run.output_hash = deterministic_output_hash(&run);
    Ok(normalize_run(&run))
}

#[cfg(test)]
pub(crate) fn run_external_suite_for_test<A: BenchmarkAdapter>(
    adapter: &A,
    manifest: &SuiteManifest,
    tier: SuiteTier,
    mode: EvaluationMode,
    output_dir: &Path,
) -> anyhow::Result<EvalRunArtifacts> {
    let cases = adapter.enumerate_cases(manifest)?;
    let plan = plan_suite_run(SuiteRunRequest {
        manifest,
        tier,
        mode,
        candidate_case_ids: cases.iter().map(|case| case.case_id.clone()).collect(),
        run_polint_analysis: true,
    })?;
    let run = build_external_suite_report_for_test(adapter, manifest, &plan, &cases)?;
    std::fs::create_dir_all(output_dir)?;
    let stem = report_stem(&manifest.id.0, mode);
    let json_path = output_dir.join(format!("{stem}.json"));
    let markdown_path = output_dir.join(format!("{stem}.md"));
    std::fs::write(
        &json_path,
        crate::eval::report::to_deterministic_json_pretty(&run),
    )?;
    std::fs::write(&markdown_path, crate::eval::markdown::render_markdown(&run))?;
    Ok(EvalRunArtifacts {
        json_path,
        markdown_path,
        output_hash: run.output_hash,
    })
}

#[cfg(test)]
fn build_external_suite_report_for_test<A: BenchmarkAdapter>(
    adapter: &A,
    manifest: &SuiteManifest,
    plan: &SuiteRunPlan,
    cases: &[EvaluationCase],
) -> anyhow::Result<EvaluationRun> {
    let scratch = tempfile::tempdir()?;
    let selected: std::collections::BTreeSet<_> = plan.selection.selected_case_ids.iter().collect();
    let mut case_results = Vec::new();
    let mut limitations = plan.limitations.clone();

    for case in cases.iter().filter(|case| selected.contains(&case.case_id)) {
        let started = std::time::Instant::now();
        let prepared = adapter.prepare_case_with_scratch(manifest, case, scratch.path())?;
        let observed = if plan.should_run_polint_analysis {
            run_polint_for_prepared_case(adapter, manifest, case, &prepared)
                .unwrap_or_else(|error| analysis_error_observed(case, error))
        } else {
            case.observed.clone()
        };
        let matches = match_case(&case.expected, &observed, MatcherConfig::default());
        if observed
            .iter()
            .any(|item| analysis_run_limitation_key(item).is_some())
        {
            limitations.push(format!(
                "case {} produced an analysis run limitation",
                case.case_id
            ));
        }
        case_results.push(CaseResult {
            case_id: case.case_id.clone(),
            area: case.area,
            expected: case.expected.clone(),
            observed,
            matches,
            runtime: RuntimeObservation {
                budget_name: "eval-runner".to_string(),
                budget_passed: true,
                observed_runtime_ms: Some(
                    started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
                ),
            },
        });
    }

    let all_matches = case_results
        .iter()
        .flat_map(|case| case.matches.iter().cloned())
        .collect::<Vec<_>>();
    let mut metrics = crate::eval::report::MetricSummary::from(compute_metrics(&all_matches));
    metrics.sections.suite_native = adapter.suite_native_metrics(manifest, &case_results)?;

    let mut run = EvaluationRun {
        schema_version: EVALUATION_SCHEMA_VERSION.to_string(),
        suite_id: manifest.id.0.clone(),
        mode: plan.mode,
        suite_manifest: Some(manifest.clone()),
        cases: case_results,
        metrics,
        performance: None,
        comparison_rows: Vec::new(),
        adaptation: None,
        adaptation_delta: None,
        limitations,
        output_hash: String::new(),
    };
    run.comparison_rows = graph_comparison_rows(&run, manifest, plan.mode);
    run.output_hash = deterministic_output_hash(&run);
    Ok(normalize_run(&run))
}

#[cfg(test)]
fn run_polint_for_prepared_case<A: BenchmarkAdapter>(
    adapter: &A,
    manifest: &SuiteManifest,
    case: &EvaluationCase,
    prepared: &PreparedCase,
) -> anyhow::Result<Vec<ObservedItem>> {
    let mut loaded = crate::config::load_config(&prepared.workspace_root)?;
    if !prepared.target_files.is_empty() {
        loaded.config.workspace.include = prepared
            .target_files
            .iter()
            .map(|path| check_path_pattern(&prepared.workspace_root, path))
            .collect();
    }
    let config_digest = crate::cache::keys::config_hash(&loaded);
    let rule_digest = crate::cache::keys::rule_hash(&[], None, &std::collections::BTreeMap::new());
    let cache = crate::cache::Cache::default_for_repo(&prepared.workspace_root, false);
    let plan = crate::analysis_plan::AnalysisPlan::empty();
    let output =
        crate::analysis_kernel::AnalysisKernel::run(crate::analysis_kernel::KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: &config_digest,
            rule_digest: &rule_digest,
            plan: &plan,
            parallel: true,
        })?;
    adapter.normalize_kernel_output(manifest, case, prepared, &output)
}

#[cfg(test)]
fn check_path_pattern(root: &Path, path: &Path) -> String {
    let display_path = if path.is_absolute() {
        path.strip_prefix(root).unwrap_or(path)
    } else {
        path
    };
    let normalized = display_path
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string();
    if normalized.contains(['*', '?', '[', ']', '{', '}']) {
        return normalized;
    }
    if root.join(&normalized).is_dir() || normalized.ends_with('/') {
        format!("{}/**", normalized.trim_end_matches('/'))
    } else {
        normalized
    }
}

#[cfg(test)]
fn analysis_error_observed(case: &EvaluationCase, error: anyhow::Error) -> Vec<ObservedItem> {
    vec![ObservedItem::Fact(ObservedFact {
        family: "AnalysisRunLimitation".to_string(),
        stable_key: format!("analysis_kernel_error:{}", case.case_id),
        mode: AssertionMode::Partial,
        producer_id: Some("polint.eval.runner".to_string()),
        provenance: Some("analysis_kernel".to_string()),
        precision: Some("none".to_string()),
        status: Some(ObservedStatus::SetupMissing),
        payload: Some(error.to_string()),
    })]
}

#[cfg(test)]
fn analysis_run_limitation_key(item: &ObservedItem) -> Option<&str> {
    match item {
        ObservedItem::Fact(fact) if fact.family == "AnalysisRunLimitation" => {
            Some(fact.stable_key.as_str())
        }
        _ => None,
    }
}

#[cfg(test)]
fn graph_comparison_rows(
    run: &EvaluationRun,
    manifest: &SuiteManifest,
    mode: EvaluationMode,
) -> Vec<BenchmarkComparisonRow> {
    let mut rows = Vec::new();
    let oracle_metrics = BTreeMap::from([
        ("precision".to_string(), 1.0),
        ("recall".to_string(), 1.0),
        ("f1".to_string(), 1.0),
        (
            "graph_edges_expected".to_string(),
            run.metrics.graph_edges_expected as f64,
        ),
        (
            "graph_edges_observed".to_string(),
            run.metrics.graph_edges_expected as f64,
        ),
    ]);
    rows.push(BenchmarkComparisonRow {
        suite_id: manifest.id.clone(),
        suite_commit: manifest.source_commit.clone(),
        mode: EvaluationMode::ImportedScanner,
        product: ProductIdentity {
            name: format!("{} oracle", manifest.name),
            version: None,
            vendor: None,
        },
        result_source: ResultSource::AdapterOnly {
            manifest_path: manifest.expected.path.clone(),
            reason: "suite-native expected graph edges are treated as the oracle row".to_string(),
        },
        metrics: oracle_metrics,
        limitations: vec!["oracle row is derived from suite-native expected edges".to_string()],
    });
    rows.push(BenchmarkComparisonRow {
        suite_id: manifest.id.clone(),
        suite_commit: manifest.source_commit.clone(),
        mode,
        product: ProductIdentity {
            name: "polint".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            vendor: Some("polint".to_string()),
        },
        result_source: ResultSource::PolintRun {
            report_path: format!(
                ".context/graph-benchmarks/{}.json",
                report_stem(&manifest.id.0, mode)
            ),
            config_digest: None,
        },
        metrics: run_metric_map(run),
        limitations: run.limitations.clone(),
    });
    rows
}

#[cfg(test)]
fn run_metric_map(run: &EvaluationRun) -> BTreeMap<String, f64> {
    BTreeMap::from([
        (
            "true_positives".to_string(),
            run.metrics.true_positives as f64,
        ),
        (
            "false_positives".to_string(),
            run.metrics.false_positives as f64,
        ),
        (
            "false_negatives".to_string(),
            run.metrics.false_negatives as f64,
        ),
        (
            "precision".to_string(),
            run.metrics.precision.unwrap_or_default(),
        ),
        ("recall".to_string(), run.metrics.recall.unwrap_or_default()),
        ("f1".to_string(), run.metrics.f1.unwrap_or_default()),
        ("unknowns".to_string(), run.metrics.unknown_count as f64),
        (
            "graph_edges_expected".to_string(),
            run.metrics.graph_edges_expected as f64,
        ),
        (
            "graph_edges_observed".to_string(),
            run.metrics.graph_edges_observed as f64,
        ),
    ])
}

#[cfg(test)]
fn report_stem(suite_id: &str, mode: EvaluationMode) -> String {
    let mode = match mode {
        EvaluationMode::PolintBaseline => "baseline",
        EvaluationMode::PolintAgentAdapted => "adapted",
        EvaluationMode::ImportedScanner => "imported",
        EvaluationMode::LocallyReproducedScanner => "local",
        EvaluationMode::AdapterOnly => "adapter-only",
    };
    format!("{suite_id}-{mode}")
}

pub(crate) fn safe_join_workspace(root: &Path, relative: &Path) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(
        !relative.is_absolute(),
        "workspace path must be relative: {}",
        relative.display()
    );
    anyhow::ensure!(
        !relative
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_))),
        "workspace path must not escape with parent or prefix components: {}",
        relative.display()
    );
    let joined = root.join(relative);
    if root.exists() && joined.exists() {
        let root = root.canonicalize()?;
        let joined = joined.canonicalize()?;
        anyhow::ensure!(
            joined.starts_with(&root),
            "workspace path escapes suite root: {}",
            joined.display()
        );
    }
    Ok(joined)
}

#[cfg(test)]
pub(crate) fn run_native_fixture_to_report_files_for_test(
    fixture_dir: &Path,
    output_dir: &Path,
) -> anyhow::Result<EvalRunArtifacts> {
    let run = crate::eval::fixtures::run_native_fixture_for_test(fixture_dir)?;
    std::fs::create_dir_all(output_dir)?;
    let json_path = output_dir.join("report.json");
    let markdown_path = output_dir.join("report.md");
    std::fs::write(
        &json_path,
        crate::eval::report::to_deterministic_json_pretty(&run),
    )?;
    std::fs::write(&markdown_path, crate::eval::markdown::render_markdown(&run))?;
    Ok(EvalRunArtifacts {
        json_path,
        markdown_path,
        output_hash: run.output_hash,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;

    use tempfile::tempdir;

    use super::*;
    use crate::eval::adapter::{PreparedCase, RawObservedOutput};
    use crate::eval::gates::{PromotionGateThresholds, SuiteGateConfig, evaluate_promotion_gates};
    use crate::eval::model::{FixtureArea, ObservedItem};
    use crate::eval::report::{CaseResult, deterministic_output_hash};
    use crate::eval::suite::{
        CaseSelector, ExpectedSource, ExpectedSourceFormat, LocalClonePolicy, SuiteCheckout,
        SuiteCheckoutStrategy, SuiteId, SuiteKind, SuiteScoring,
    };

    #[test]
    fn adapter_only_suites_never_run_polint_analysis() {
        let manifest = manifest(SuiteLanguageSupport::AdapterOnly);
        let plan = plan_suite_run(SuiteRunRequest {
            manifest: &manifest,
            tier: SuiteTier::Fast,
            mode: EvaluationMode::AdapterOnly,
            candidate_case_ids: vec!["case-a".to_string()],
            run_polint_analysis: true,
        })
        .unwrap();

        assert!(!plan.should_run_polint_analysis);
        assert!(
            plan.limitations
                .iter()
                .any(|limitation| limitation.contains("polint analysis is disabled"))
        );
    }

    #[test]
    fn supported_baseline_can_plan_polint_analysis() {
        let manifest = manifest(SuiteLanguageSupport::Supported);
        let plan = plan_suite_run(SuiteRunRequest {
            manifest: &manifest,
            tier: SuiteTier::Fast,
            mode: EvaluationMode::PolintBaseline,
            candidate_case_ids: vec!["case-a".to_string()],
            run_polint_analysis: true,
        })
        .unwrap();

        assert!(plan.should_run_polint_analysis);
        assert_eq!(plan.selection.selected_case_ids, ["case-a"]);
    }

    #[test]
    fn workspace_join_rejects_escape_paths_and_symlinks() {
        let root = tempdir().unwrap();
        let inside = root.path().join("inside");
        std::fs::create_dir(&inside).unwrap();

        assert!(safe_join_workspace(root.path(), Path::new("inside")).is_ok());
        assert!(safe_join_workspace(root.path(), Path::new("../outside")).is_err());
        let absolute_outside = tempdir().unwrap();
        assert!(safe_join_workspace(root.path(), absolute_outside.path()).is_err());

        #[cfg(unix)]
        {
            let outside = tempdir().unwrap();
            let link = root.path().join("link");
            std::os::unix::fs::symlink(outside.path(), &link).unwrap();
            assert!(safe_join_workspace(root.path(), Path::new("link")).is_err());
        }
    }

    #[test]
    fn report_builder_normalizes_selected_cases() {
        let adapter = NullAdapter;
        let manifest = manifest(SuiteLanguageSupport::Supported);
        let cases = adapter.enumerate_cases(&manifest).unwrap();
        let plan = plan_suite_run(SuiteRunRequest {
            manifest: &manifest,
            tier: SuiteTier::Fast,
            mode: EvaluationMode::PolintBaseline,
            candidate_case_ids: cases.iter().map(|case| case.case_id.clone()).collect(),
            run_polint_analysis: true,
        })
        .unwrap();

        let report = build_report_for_cases(&adapter, &manifest, &plan, &cases).unwrap();

        assert_eq!(report.suite_id, "runner-suite");
        assert_eq!(report.cases.len(), 1);
        assert!(!report.output_hash.is_empty());
    }

    #[test]
    fn internal_eval_helper_writes_deterministic_json_and_markdown() {
        let temp = tempdir().unwrap();
        let fixture_dir = repo_root().join("tests/eval-fixtures/promotion/cfg-call-flow-evidence");

        let first =
            run_native_fixture_to_report_files_for_test(&fixture_dir, &temp.path().join("first"))
                .unwrap();
        let second =
            run_native_fixture_to_report_files_for_test(&fixture_dir, &temp.path().join("second"))
                .unwrap();

        assert_eq!(first.output_hash, second.output_hash);
        assert_eq!(
            std::fs::read_to_string(&first.json_path).unwrap(),
            std::fs::read_to_string(&second.json_path).unwrap()
        );
        assert_eq!(
            std::fs::read_to_string(&first.markdown_path).unwrap(),
            std::fs::read_to_string(&second.markdown_path).unwrap()
        );
        assert!(
            std::fs::read_to_string(&first.json_path)
                .unwrap()
                .contains("polint-eval-internal-1")
        );
    }

    #[test]
    fn phase40_promotion_fixture_gates_pass_deterministically() {
        let fixture_dir = repo_root().join("tests/eval-fixtures/promotion/cfg-call-flow-evidence");
        let first = crate::eval::fixtures::run_native_fixture_for_test(&fixture_dir).unwrap();
        let second = crate::eval::fixtures::run_native_fixture_for_test(&fixture_dir).unwrap();
        let config = SuiteGateConfig {
            suite_id: "native-promotion".to_string(),
            tier: "fast".to_string(),
            thresholds: PromotionGateThresholds {
                max_unknowns: 1,
                warn_unknowns_above: 1,
                ..PromotionGateThresholds::default()
            },
        };

        let first_report = evaluate_promotion_gates(&first, Some(&second), &config);
        let second_report = evaluate_promotion_gates(&first, Some(&second), &config);

        assert_eq!(first.output_hash, second.output_hash);
        assert_eq!(first_report, second_report);
        assert_eq!(first_report.verdict, crate::eval::gates::GateVerdict::Pass);
        assert!(
            first_report
                .checks
                .iter()
                .any(|check| check.metric == "deterministic_output_hash"
                    && check.threshold == "true")
        );
    }

    #[test]
    fn runtime_duration_changes_do_not_affect_output_hash() {
        let fixture_dir = repo_root().join("tests/eval-fixtures/promotion/cfg-call-flow-evidence");
        let mut first = crate::eval::fixtures::run_native_fixture_for_test(&fixture_dir).unwrap();
        let mut second = first.clone();
        first.cases[0].runtime.observed_runtime_ms = Some(1);
        second.cases[0].runtime.observed_runtime_ms = Some(999);

        assert_eq!(
            deterministic_output_hash(&first),
            deterministic_output_hash(&second)
        );
    }

    #[test]
    fn phase40_public_boundary_does_not_advertise_hidden_eval_or_unpromoted_views() {
        let root = repo_root();
        let read = |path: &str| std::fs::read_to_string(root.join(path)).unwrap();
        let public_text = [
            read("README.md"),
            read("crates/polint/src/sdk/mod.rs"),
            read("crates/polint/src/runner/mod.rs"),
            docs_facts_text(&root),
        ]
        .join("\n");

        assert!(!public_text.contains("polint eval"));
        assert!(!public_text.contains("CallGraph<'_"));
        assert!(!public_text.contains("DataFlow<'_> is supported"));
        assert!(!public_text.contains("Evidence<'_"));
        assert!(!read("crates/polint/src/lib.rs").contains("pub mod eval"));
    }

    struct NullAdapter;

    impl BenchmarkAdapter for NullAdapter {
        fn adapter_id(&self) -> &'static str {
            "null"
        }

        fn enumerate_cases(
            &self,
            _manifest: &SuiteManifest,
        ) -> anyhow::Result<Vec<EvaluationCase>> {
            Ok(vec![EvaluationCase {
                case_id: "case-a".to_string(),
                area: FixtureArea::Diagnostics,
                repo_path: "case-a".to_string(),
                expected: Vec::new(),
                observed: Vec::new(),
            }])
        }

        fn prepare_case(
            &self,
            _manifest: &SuiteManifest,
            case: &EvaluationCase,
        ) -> anyhow::Result<PreparedCase> {
            Ok(PreparedCase {
                case_id: case.case_id.clone(),
                workspace_root: PathBuf::from("repo"),
                target_files: Vec::new(),
            })
        }

        fn normalize_observed(
            &self,
            _manifest: &SuiteManifest,
            _case: &EvaluationCase,
            raw: RawObservedOutput,
        ) -> anyhow::Result<Vec<ObservedItem>> {
            Ok(raw.observed)
        }

        fn suite_native_metrics(
            &self,
            _manifest: &SuiteManifest,
            _results: &[CaseResult],
        ) -> anyhow::Result<BTreeMap<String, f64>> {
            Ok(BTreeMap::from([("native.case_count".to_string(), 1.0)]))
        }
    }

    fn manifest(language_support: SuiteLanguageSupport) -> SuiteManifest {
        SuiteManifest {
            schema_version: "polint-eval-suite-1".to_string(),
            id: SuiteId("runner-suite".to_string()),
            name: "Runner suite".to_string(),
            kind: SuiteKind::ScannerVulnerability,
            languages: vec!["go".to_string()],
            adapter_id: "null".to_string(),
            source_url: Some("https://example.test/runner".to_string()),
            source_commit: Some("abc123".to_string()),
            license: "test".to_string(),
            language_support,
            checkout: SuiteCheckout {
                strategy: SuiteCheckoutStrategy::LocalClone,
                path: "research/evaluation-harness/repos/runner".to_string(),
                ignored_by_git: true,
                local_clone_policy: LocalClonePolicy::RepoRelativeOnly,
            },
            expected: ExpectedSource {
                format: ExpectedSourceFormat::SuiteNative,
                path: "expected.json".to_string(),
            },
            scoring: SuiteScoring {
                native: Vec::new(),
                unified: vec!["precision".to_string()],
            },
            tiers: BTreeMap::from([(
                SuiteTier::Fast,
                CaseSelector {
                    enabled: true,
                    selector: "all".to_string(),
                    max_cases: None,
                    deterministic_seed: None,
                },
            )]),
        }
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    fn docs_facts_text(root: &Path) -> String {
        let mut text = String::new();
        for entry in std::fs::read_dir(root.join("docs/facts")).unwrap() {
            let entry = entry.unwrap();
            if entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                == Some("md")
            {
                text.push_str(&std::fs::read_to_string(entry.path()).unwrap());
                text.push('\n');
            }
        }
        text
    }
}

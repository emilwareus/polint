use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::eval::adapter::BenchmarkAdapter;
use crate::eval::matcher::{MatcherConfig, match_case};
use crate::eval::metrics::compute_metrics;
use crate::eval::model::{EvaluationCase, EvaluationMode};
use crate::eval::report::{
    CaseResult, EVALUATION_SCHEMA_VERSION, EvaluationRun, RuntimeObservation,
    deterministic_output_hash, normalize_run,
};
use crate::eval::suite::{SuiteLanguageSupport, SuiteManifest, SuiteTier};
use crate::eval::tiers::{TierSelection, select_case_ids};

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
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;

    use tempfile::tempdir;

    use super::*;
    use crate::eval::adapter::{PreparedCase, RawObservedOutput};
    use crate::eval::model::{FixtureArea, ObservedItem};
    use crate::eval::report::CaseResult;
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
        assert!(safe_join_workspace(root.path(), Path::new("/tmp/outside")).is_err());

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
}

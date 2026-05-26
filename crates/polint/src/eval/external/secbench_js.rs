use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::eval::adapter::{BenchmarkAdapter, PreparedCase, RawObservedOutput};
use crate::eval::model::{
    AssertionMode, EvaluationCase, ExpectedDiagnostic, ExpectedItem, FixtureArea, ObservedItem,
};
use crate::eval::report::CaseResult;
use crate::eval::runner::safe_join_workspace;
use crate::eval::suite::{SuiteLanguageSupport, SuiteManifest, normalize_repo_relative_path};

pub(crate) struct SecbenchJsSmokeAdapter;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SecbenchEnumeration {
    pub(crate) cases: Vec<EvaluationCase>,
    pub(crate) limitations: Vec<String>,
}

impl BenchmarkAdapter for SecbenchJsSmokeAdapter {
    fn adapter_id(&self) -> &'static str {
        "secbench_js_smoke"
    }

    fn language_support(&self, manifest: &SuiteManifest) -> SuiteLanguageSupport {
        manifest.language_support
    }

    fn enumerate_cases(&self, manifest: &SuiteManifest) -> anyhow::Result<Vec<EvaluationCase>> {
        Ok(enumerate_secbench_cases(Path::new(&manifest.checkout.path))?.cases)
    }

    fn prepare_case(
        &self,
        manifest: &SuiteManifest,
        case: &EvaluationCase,
    ) -> anyhow::Result<PreparedCase> {
        let root = PathBuf::from(&manifest.checkout.path);
        let target = safe_join_workspace(&root, Path::new(&case.repo_path))?;
        Ok(PreparedCase {
            case_id: case.case_id.clone(),
            workspace_root: root,
            target_files: vec![target],
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
        results: &[CaseResult],
    ) -> anyhow::Result<BTreeMap<String, f64>> {
        Ok(BTreeMap::from([(
            "secbench_js.smoke_case_count".to_string(),
            results.len() as f64,
        )]))
    }
}

pub(crate) fn enumerate_secbench_cases(root: &Path) -> anyhow::Result<SecbenchEnumeration> {
    if !root.exists() {
        return Ok(SecbenchEnumeration {
            cases: Vec::new(),
            limitations: vec![format!(
                "SecBench.js local clone is absent at {}; suite can be planned but not executed",
                root.display()
            )],
        });
    }

    let mut files = Vec::new();
    collect_js_test_files(root, root, &mut files)?;
    files.sort();
    let cases = files
        .into_iter()
        .map(secbench_case)
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(SecbenchEnumeration {
        cases,
        limitations: vec![
            "SecBench.js package setup is not executed by the adapter; setup gaps are reported separately from polint findings".to_string(),
            "SecBench.js labels are executable exploit tests, not one-to-one polint rule expectations".to_string(),
        ],
    })
}

fn collect_js_test_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<String>,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if file_name == "node_modules" || file_name == ".git" {
            continue;
        }
        if path.is_dir() {
            collect_js_test_files(root, &path, files)?;
            continue;
        }
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".test.js"))
        {
            let relative = path.strip_prefix(root)?;
            files.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

fn secbench_case(relative_path: String) -> anyhow::Result<EvaluationCase> {
    let relative_path = normalize_repo_relative_path("secbench case path", &relative_path)?;
    Ok(EvaluationCase {
        case_id: relative_path.clone(),
        area: FixtureArea::Diagnostics,
        repo_path: relative_path.clone(),
        expected: vec![ExpectedItem::Diagnostic(ExpectedDiagnostic {
            rule_id: "secbench-js/unlabelled-executable-case".to_string(),
            relative_path,
            line: None,
            fingerprint: None,
            mode: AssertionMode::Partial,
            false_positive_trap: false,
        })],
        observed: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use tempfile::tempdir;

    use super::*;
    use crate::eval::suite::{
        CaseSelector, ExpectedSource, ExpectedSourceFormat, LocalClonePolicy, SuiteCheckout,
        SuiteCheckoutStrategy, SuiteId, SuiteKind, SuiteScoring, SuiteTier,
    };

    #[test]
    fn absent_clone_skips_gracefully() {
        let missing = tempdir().unwrap().path().join("missing");
        let enumeration = enumerate_secbench_cases(&missing).unwrap();

        assert!(enumeration.cases.is_empty());
        assert!(enumeration.limitations[0].contains("local clone is absent"));
    }

    #[test]
    fn enumerates_js_test_files_deterministically() {
        let root = tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("cases/nested")).unwrap();
        std::fs::write(root.path().join("cases/b.test.js"), "test()").unwrap();
        std::fs::write(root.path().join("cases/nested/a.test.js"), "test()").unwrap();
        std::fs::write(root.path().join("cases/ignore.js"), "not a test").unwrap();

        let enumeration = enumerate_secbench_cases(root.path()).unwrap();

        assert_eq!(
            enumeration
                .cases
                .iter()
                .map(|case| case.case_id.as_str())
                .collect::<Vec<_>>(),
            ["cases/b.test.js", "cases/nested/a.test.js"]
        );
        assert!(
            enumeration
                .limitations
                .iter()
                .any(|limitation| limitation.contains("package setup"))
        );
    }

    #[test]
    fn adapter_prepares_target_inside_clone() {
        let manifest = manifest("research/evaluation-harness/repos/SecBench.js");
        let adapter = SecbenchJsSmokeAdapter;
        let case = secbench_case("cases/a.test.js".to_string()).unwrap();

        let prepared = adapter.prepare_case(&manifest, &case).unwrap();

        assert_eq!(prepared.case_id, "cases/a.test.js");
        assert_eq!(prepared.target_files.len(), 1);
    }

    fn manifest(path: &str) -> SuiteManifest {
        SuiteManifest {
            schema_version: "polint-eval-suite-1".to_string(),
            id: SuiteId("secbench-js-smoke".to_string()),
            name: "SecBench.js smoke".to_string(),
            kind: SuiteKind::ScannerVulnerability,
            languages: vec!["javascript".to_string(), "typescript".to_string()],
            adapter_id: "secbench_js_smoke".to_string(),
            source_url: Some("https://github.com/SecBench/SecBench.js".to_string()),
            source_commit: Some("bc3156219138".to_string()),
            license: "license-review-needed".to_string(),
            language_support: SuiteLanguageSupport::Supported,
            checkout: SuiteCheckout {
                strategy: SuiteCheckoutStrategy::LocalClone,
                path: path.to_string(),
                ignored_by_git: true,
                local_clone_policy: LocalClonePolicy::RepoRelativeOnly,
            },
            expected: ExpectedSource {
                format: ExpectedSourceFormat::SuiteNative,
                path: "suite-native-secbench-js".to_string(),
            },
            scoring: SuiteScoring {
                native: vec!["secbench_js.smoke_case_count".to_string()],
                unified: vec!["precision".to_string(), "recall".to_string()],
            },
            tiers: BTreeMap::from([(
                SuiteTier::Fast,
                CaseSelector {
                    enabled: true,
                    selector: "sample:balanced:20".to_string(),
                    max_cases: Some(20),
                    deterministic_seed: Some("secbench-js-fast".to_string()),
                },
            )]),
        }
    }
}

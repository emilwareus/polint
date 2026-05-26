use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::eval::adapter::{BenchmarkAdapter, PreparedCase, RawObservedOutput};
use crate::eval::model::{
    AssertionMode, EvaluationCase, ExpectedDiagnostic, ExpectedItem, FixtureArea, ObservedItem,
};
use crate::eval::report::CaseResult;
use crate::eval::runner::safe_join_workspace;
use crate::eval::suite::{SuiteLanguageSupport, SuiteManifest, normalize_repo_relative_path};

pub(crate) struct GosecSampleAdapter;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct GosecEnumeration {
    pub(crate) cases: Vec<EvaluationCase>,
    pub(crate) limitations: Vec<String>,
}

impl BenchmarkAdapter for GosecSampleAdapter {
    fn adapter_id(&self) -> &'static str {
        "gosec_samples"
    }

    fn language_support(&self, manifest: &SuiteManifest) -> SuiteLanguageSupport {
        manifest.language_support
    }

    fn enumerate_cases(&self, manifest: &SuiteManifest) -> anyhow::Result<Vec<EvaluationCase>> {
        Ok(enumerate_gosec_cases(Path::new(&manifest.checkout.path))?.cases)
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
            "gosec.sample_case_count".to_string(),
            results.len() as f64,
        )]))
    }
}

pub(crate) fn enumerate_gosec_cases(root: &Path) -> anyhow::Result<GosecEnumeration> {
    if !root.exists() {
        return Ok(GosecEnumeration {
            cases: Vec::new(),
            limitations: vec![format!(
                "gosec local clone is absent at {}; suite can be planned but not executed",
                root.display()
            )],
        });
    }

    let mut files = Vec::new();
    collect_go_files(root, root, &mut files)?;
    files.sort();
    let cases = files
        .into_iter()
        .map(gosec_case)
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(GosecEnumeration {
        cases,
        limitations: vec![
            "gosec samples are a practical Go baseline, not broad independent ground truth".to_string(),
            "gosec sample labels are mapped as suite-native expectations until per-rule metadata is imported".to_string(),
        ],
    })
}

fn collect_go_files(root: &Path, current: &Path, files: &mut Vec<String>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if matches!(file_name.as_ref(), ".git" | "vendor" | "node_modules") {
            continue;
        }
        if path.is_dir() {
            collect_go_files(root, &path, files)?;
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) == Some("go") {
            let relative = path.strip_prefix(root)?;
            let relative = relative.to_string_lossy().replace('\\', "/");
            if likely_gosec_sample(&relative) {
                files.push(relative);
            }
        }
    }
    Ok(())
}

fn likely_gosec_sample(relative_path: &str) -> bool {
    relative_path.contains("test")
        || relative_path.contains("sample")
        || relative_path.contains("benchmark")
        || relative_path.contains("rule")
}

fn gosec_case(relative_path: String) -> anyhow::Result<EvaluationCase> {
    let relative_path = normalize_repo_relative_path("gosec case path", &relative_path)?;
    Ok(EvaluationCase {
        case_id: relative_path.clone(),
        area: FixtureArea::Diagnostics,
        repo_path: relative_path.clone(),
        expected: vec![ExpectedItem::Diagnostic(ExpectedDiagnostic {
            rule_id: "gosec/sample-native-label".to_string(),
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
        let enumeration = enumerate_gosec_cases(&missing).unwrap();

        assert!(enumeration.cases.is_empty());
        assert!(enumeration.limitations[0].contains("local clone is absent"));
    }

    #[test]
    fn enumerates_go_sample_files_deterministically() {
        let root = tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("rules")).unwrap();
        std::fs::create_dir_all(root.path().join("cmd")).unwrap();
        std::fs::write(root.path().join("rules/g101_test.go"), "package rules").unwrap();
        std::fs::write(root.path().join("rules/sample.go"), "package rules").unwrap();
        std::fs::write(root.path().join("cmd/main.go"), "package main").unwrap();

        let enumeration = enumerate_gosec_cases(root.path()).unwrap();

        assert_eq!(
            enumeration
                .cases
                .iter()
                .map(|case| case.case_id.as_str())
                .collect::<Vec<_>>(),
            ["rules/g101_test.go", "rules/sample.go"]
        );
        assert!(
            enumeration
                .limitations
                .iter()
                .any(|limitation| limitation.contains("practical Go baseline"))
        );
    }

    #[test]
    fn adapter_prepares_target_inside_clone() {
        let manifest = manifest("research/evaluation-harness/repos/gosec");
        let adapter = GosecSampleAdapter;
        let case = gosec_case("rules/g101_test.go".to_string()).unwrap();

        let prepared = adapter.prepare_case(&manifest, &case).unwrap();

        assert_eq!(prepared.case_id, "rules/g101_test.go");
        assert_eq!(prepared.target_files.len(), 1);
    }

    fn manifest(path: &str) -> SuiteManifest {
        SuiteManifest {
            schema_version: "polint-eval-suite-1".to_string(),
            id: SuiteId("gosec-samples".to_string()),
            name: "gosec samples".to_string(),
            kind: SuiteKind::ScannerVulnerability,
            languages: vec!["go".to_string()],
            adapter_id: "gosec_samples".to_string(),
            source_url: Some("https://github.com/securego/gosec".to_string()),
            source_commit: Some("de65614d10a6".to_string()),
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
                path: "suite-native-gosec-samples".to_string(),
            },
            scoring: SuiteScoring {
                native: vec!["gosec.sample_case_count".to_string()],
                unified: vec!["precision".to_string(), "recall".to_string()],
            },
            tiers: BTreeMap::from([(
                SuiteTier::Fast,
                CaseSelector {
                    enabled: true,
                    selector: "sample:balanced:20".to_string(),
                    max_cases: Some(20),
                    deterministic_seed: Some("gosec-fast".to_string()),
                },
            )]),
        }
    }
}

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::eval::model::{EvaluationCase, ObservedItem};
use crate::eval::report::CaseResult;
use crate::eval::suite::{SuiteLanguageSupport, SuiteManifest};

pub(crate) trait BenchmarkAdapter {
    fn adapter_id(&self) -> &'static str;

    fn load_manifest(&self, manifest_toml: &str) -> anyhow::Result<SuiteManifest> {
        let manifest: SuiteManifest = toml::from_str(manifest_toml)?;
        manifest.validate()?;
        Ok(manifest)
    }

    fn language_support(&self, manifest: &SuiteManifest) -> SuiteLanguageSupport {
        manifest.language_support
    }

    fn enumerate_cases(&self, manifest: &SuiteManifest) -> anyhow::Result<Vec<EvaluationCase>>;

    fn prepare_case(
        &self,
        manifest: &SuiteManifest,
        case: &EvaluationCase,
    ) -> anyhow::Result<PreparedCase>;

    fn normalize_observed(
        &self,
        manifest: &SuiteManifest,
        case: &EvaluationCase,
        raw: RawObservedOutput,
    ) -> anyhow::Result<Vec<ObservedItem>>;

    fn suite_native_metrics(
        &self,
        manifest: &SuiteManifest,
        results: &[CaseResult],
    ) -> anyhow::Result<BTreeMap<String, f64>>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PreparedCase {
    pub(crate) case_id: String,
    pub(crate) workspace_root: PathBuf,
    pub(crate) target_files: Vec<PathBuf>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct RawObservedOutput {
    pub(crate) observed: Vec<ObservedItem>,
    pub(crate) artifact_path: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::model::FixtureArea;
    use crate::eval::suite::SuiteLanguageSupport;

    #[test]
    fn adapter_trait_loads_manifest_and_preserves_adapter_only_status() {
        let adapter = NullAdapter;
        let manifest = adapter.load_manifest(manifest_toml()).unwrap();

        assert_eq!(adapter.adapter_id(), "null");
        assert_eq!(
            adapter.language_support(&manifest),
            SuiteLanguageSupport::AdapterOnly
        );
        assert_eq!(manifest.adapter_id, "null");
    }

    #[test]
    fn adapter_trait_can_enumerate_native_like_cases() {
        let adapter = NullAdapter;
        let manifest = adapter.load_manifest(manifest_toml()).unwrap();
        let cases = adapter.enumerate_cases(&manifest).unwrap();

        assert_eq!(cases.len(), 1);
        assert_eq!(manifest.id.0, "adapter-suite");
        assert_eq!(cases[0].area, FixtureArea::Diagnostics);

        let prepared = adapter.prepare_case(&manifest, &cases[0]).unwrap();
        assert_eq!(prepared.case_id, "case-1");
        assert_eq!(
            prepared.workspace_root,
            PathBuf::from("research/evaluation-harness/repos/SecBench.js")
        );
        assert!(prepared.target_files.is_empty());

        let observed = adapter
            .normalize_observed(&manifest, &cases[0], RawObservedOutput::default())
            .unwrap();
        assert!(observed.is_empty());
        assert!(
            adapter
                .suite_native_metrics(&manifest, &[])
                .unwrap()
                .is_empty()
        );
    }

    struct NullAdapter;

    impl BenchmarkAdapter for NullAdapter {
        fn adapter_id(&self) -> &'static str {
            "null"
        }

        fn enumerate_cases(&self, manifest: &SuiteManifest) -> anyhow::Result<Vec<EvaluationCase>> {
            Ok(vec![EvaluationCase {
                case_id: "case-1".to_string(),
                area: FixtureArea::Diagnostics,
                repo_path: manifest.checkout.path.clone(),
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
                workspace_root: PathBuf::from(&case.repo_path),
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
            Ok(BTreeMap::new())
        }
    }

    fn manifest_toml() -> &'static str {
        r#"
schema_version = "polint-eval-suite-1"
id = "adapter-suite"
name = "Adapter suite"
kind = "scanner_vulnerability"
languages = ["javascript"]
adapter_id = "null"
license = "license-review-needed"
language_support = "adapter_only"

[checkout]
strategy = "local_clone"
path = "research/evaluation-harness/repos/SecBench.js"

[expected]
format = "suite_native"
path = "expected.json"

[scoring]
unified = ["precision", "recall"]

[tiers.fast]
enabled = true
selector = "sample:balanced:10"
"#
    }
}

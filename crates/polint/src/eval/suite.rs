use std::collections::BTreeMap;
use std::path::{Component, Path};

use anyhow::{bail, ensure};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(transparent)]
pub(crate) struct SuiteId(pub(crate) String);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SuiteKind {
    NativeFixture,
    ScannerVulnerability,
    GraphPrecision,
    DataFlowPrecision,
    CallGraphPrecision,
    EvidenceQuality,
    Performance,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SuiteLanguageSupport {
    Supported,
    AdapterOnly,
    FutureLanguage,
    ResearchOnly,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SuiteTier {
    Fast,
    Nightly,
    Release,
    Research,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SuiteCheckoutStrategy {
    VendoredFixture,
    LocalClone,
    Generated,
    ExternalArtifact,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LocalClonePolicy {
    RepoRelativeOnly,
    AllowAbsolute,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExpectedSourceFormat {
    NativePolintToml,
    Sarif,
    Json,
    SuiteNative,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct SuiteManifest {
    pub(crate) schema_version: String,
    pub(crate) id: SuiteId,
    pub(crate) name: String,
    pub(crate) kind: SuiteKind,
    pub(crate) languages: Vec<String>,
    pub(crate) adapter_id: String,
    pub(crate) source_url: Option<String>,
    pub(crate) source_commit: Option<String>,
    pub(crate) license: String,
    pub(crate) language_support: SuiteLanguageSupport,
    pub(crate) checkout: SuiteCheckout,
    pub(crate) expected: ExpectedSource,
    pub(crate) scoring: SuiteScoring,
    pub(crate) tiers: BTreeMap<SuiteTier, CaseSelector>,
}

impl SuiteManifest {
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        ensure!(!self.id.0.trim().is_empty(), "suite id must not be empty");
        ensure!(
            !self.adapter_id.trim().is_empty(),
            "suite adapter_id must not be empty"
        );
        ensure!(
            !self.languages.is_empty(),
            "suite languages must not be empty"
        );
        ensure!(!self.tiers.is_empty(), "suite tiers must not be empty");
        self.checkout.validate("suite.checkout")?;
        self.expected.validate("suite.expected")?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct SuiteCheckout {
    pub(crate) strategy: SuiteCheckoutStrategy,
    pub(crate) path: String,
    #[serde(default)]
    pub(crate) ignored_by_git: bool,
    #[serde(default = "default_local_clone_policy")]
    pub(crate) local_clone_policy: LocalClonePolicy,
}

impl SuiteCheckout {
    pub(crate) fn validate(&self, field: &str) -> anyhow::Result<()> {
        validate_suite_path(
            &format!("{field}.path"),
            &self.path,
            self.local_clone_policy,
        )
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ExpectedSource {
    pub(crate) format: ExpectedSourceFormat,
    pub(crate) path: String,
}

impl ExpectedSource {
    pub(crate) fn validate(&self, field: &str) -> anyhow::Result<()> {
        validate_suite_path(
            &format!("{field}.path"),
            &self.path,
            LocalClonePolicy::RepoRelativeOnly,
        )
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct SuiteScoring {
    #[serde(default)]
    pub(crate) native: Vec<String>,
    #[serde(default)]
    pub(crate) unified: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct CaseSelector {
    pub(crate) enabled: bool,
    pub(crate) selector: String,
    #[serde(default)]
    pub(crate) max_cases: Option<usize>,
    #[serde(default)]
    pub(crate) deterministic_seed: Option<String>,
}

fn default_local_clone_policy() -> LocalClonePolicy {
    LocalClonePolicy::RepoRelativeOnly
}

pub(crate) fn validate_suite_path(
    field: &str,
    value: &str,
    local_clone_policy: LocalClonePolicy,
) -> anyhow::Result<()> {
    if value.trim().is_empty() {
        bail!("{field} must not be empty");
    }
    if is_absolute_suite_path(value) && local_clone_policy != LocalClonePolicy::AllowAbsolute {
        bail!("{field} must be repo-relative unless local_clone_policy allows absolute paths");
    }
    if has_parent_dir_component(value) {
        bail!("{field} must not contain a parent directory component");
    }
    Ok(())
}

pub(crate) fn normalize_repo_relative_path(field: &str, value: &str) -> anyhow::Result<String> {
    validate_suite_path(field, value, LocalClonePolicy::RepoRelativeOnly)?;
    Ok(value.replace('\\', "/"))
}

fn is_absolute_suite_path(value: &str) -> bool {
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
mod tests {
    use super::*;

    #[test]
    fn suite_manifest_accepts_supported_and_adapter_only_tiers() {
        let supported = manifest(SuiteLanguageSupport::Supported, "tests/eval-fixtures");
        let adapter_only = manifest(
            SuiteLanguageSupport::AdapterOnly,
            "research/evaluation-harness/repos/SecBench.js",
        );

        supported.validate().unwrap();
        adapter_only.validate().unwrap();
        assert_eq!(
            adapter_only.language_support,
            SuiteLanguageSupport::AdapterOnly
        );
        assert!(adapter_only.tiers.contains_key(&SuiteTier::Fast));
        assert!(adapter_only.tiers.contains_key(&SuiteTier::Release));
    }

    #[test]
    fn suite_manifest_rejects_unsafe_local_paths_by_default() {
        let absolute = manifest(SuiteLanguageSupport::Supported, "/tmp/SecBench.js");
        let parent = manifest(SuiteLanguageSupport::Supported, "../SecBench.js");

        assert!(absolute.validate().is_err());
        assert!(parent.validate().is_err());
    }

    #[test]
    fn suite_manifest_can_explicitly_allow_absolute_local_clone_paths() {
        let mut manifest = manifest(SuiteLanguageSupport::ResearchOnly, "/tmp/SecBench.js");
        manifest.checkout.local_clone_policy = LocalClonePolicy::AllowAbsolute;

        manifest.validate().unwrap();
    }

    #[test]
    fn suite_manifest_denies_unknown_fields() {
        let raw = r#"
schema_version = "polint-eval-suite-1"
id = "native-fixtures"
name = "Native fixtures"
kind = "native_fixture"
languages = ["go"]
adapter_id = "native"
license = "repo"
language_support = "supported"
unexpected = "field"

[checkout]
strategy = "vendored_fixture"
path = "tests/eval-fixtures"

[expected]
format = "native_polint_toml"
path = "expected.polint-eval.toml"

[scoring]
unified = ["precision"]

[tiers.fast]
enabled = true
selector = "all"
"#;

        let err = toml::from_str::<SuiteManifest>(raw).unwrap_err();
        assert!(err.to_string().contains("unknown field"));
    }

    fn manifest(language_support: SuiteLanguageSupport, checkout_path: &str) -> SuiteManifest {
        SuiteManifest {
            schema_version: "polint-eval-suite-1".to_string(),
            id: SuiteId("native-fixtures".to_string()),
            name: "Native fixtures".to_string(),
            kind: SuiteKind::NativeFixture,
            languages: vec!["go".to_string(), "typescript".to_string()],
            adapter_id: "native".to_string(),
            source_url: Some("https://example.test/polint".to_string()),
            source_commit: Some("abc123".to_string()),
            license: "repo".to_string(),
            language_support,
            checkout: SuiteCheckout {
                strategy: SuiteCheckoutStrategy::VendoredFixture,
                path: checkout_path.to_string(),
                ignored_by_git: false,
                local_clone_policy: LocalClonePolicy::RepoRelativeOnly,
            },
            expected: ExpectedSource {
                format: ExpectedSourceFormat::NativePolintToml,
                path: "expected.polint-eval.toml".to_string(),
            },
            scoring: SuiteScoring {
                native: Vec::new(),
                unified: vec!["precision".to_string(), "recall".to_string()],
            },
            tiers: [
                (
                    SuiteTier::Fast,
                    CaseSelector {
                        enabled: true,
                        selector: "sample:balanced:10".to_string(),
                        max_cases: Some(10),
                        deterministic_seed: Some("phase-40".to_string()),
                    },
                ),
                (
                    SuiteTier::Release,
                    CaseSelector {
                        enabled: true,
                        selector: "all".to_string(),
                        max_cases: None,
                        deterministic_seed: None,
                    },
                ),
            ]
            .into_iter()
            .collect(),
        }
    }
}

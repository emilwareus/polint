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

/// Per-suite scoring mode (D-14). Each suite was built against an oracle that
/// reports a specific edge set, so the scoring path must mirror that oracle:
///
/// - `OracleRta`: the Go x/tools RTA oracle reports only reachable-from-`main`
///   edges, so polint's scored edges are filtered to the reachable-from-roots set.
/// - `OracleJelly`: Jelly enumerates module-wide call edges independent of
///   main-reachability, so reachability is recorded but does NOT filter scoring.
/// - `WholeRepo`: no reachability filtering — every edge is scored (the security
///   suites, which are not reachability-filtered call-graph suites).
///
/// Closed enum: pinned declaration order + derived `Ord` + PER-VARIANT serde
/// renames make the wire representation byte-stable (D-04). The renames are
/// per-variant (`#[serde(rename = "...")]`), NOT `rename_all = "snake_case"`,
/// because the wire strings are kebab-case (`oracle-rta`, not `oracle_rta`).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ScoringMode {
    #[serde(rename = "oracle-rta")]
    OracleRta,
    #[serde(rename = "oracle-jelly")]
    OracleJelly,
    #[serde(rename = "whole-repo")]
    WholeRepo,
}

impl ScoringMode {
    /// Stable kebab-case wire label, matching the per-variant serde renames
    /// byte-for-byte. Used by the explicit `validate()` guard and any callers
    /// that need the wire string without a serde round-trip.
    pub(crate) fn as_wire_str(self) -> &'static str {
        match self {
            Self::OracleRta => "oracle-rta",
            Self::OracleJelly => "oracle-jelly",
            Self::WholeRepo => "whole-repo",
        }
    }
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
    // D-14/D-15: required (non-`Option`) field. A manifest missing `scoring_mode`
    // fails TOML deserialization structurally (non-`Option` + `deny_unknown_fields`
    // on this struct); `validate()` adds an explicit second-layer guard.
    pub(crate) scoring_mode: ScoringMode,
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
        // D-15 explicit gate layer on top of the structural (non-`Option` +
        // `deny_unknown_fields`) one: the scoring_mode must map to a recognized
        // kebab-case wire string. A closed `ScoringMode` always does, so this is a
        // defensive guard that fails closed if a future non-kebab variant slips in
        // without a serde rename — never silently coercing to a default mode that
        // could mis-score a suite (threat T-43-02-01).
        ensure!(
            matches!(
                self.scoring_mode.as_wire_str(),
                "oracle-rta" | "oracle-jelly" | "whole-repo"
            ),
            "suite scoring_mode must be one of oracle-rta, oracle-jelly, whole-repo"
        );
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
    fn committed_evaluation_suite_manifests_parse_and_validate() {
        let manifest_dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../research/evaluation-harness/suites");
        let mut paths = std::fs::read_dir(&manifest_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.extension().and_then(|extension| extension.to_str()) == Some("toml")
            })
            .collect::<Vec<_>>();
        paths.sort();

        assert!(!paths.is_empty());
        for path in paths {
            let raw = std::fs::read_to_string(&path).unwrap();
            let manifest: SuiteManifest = toml::from_str(&raw)
                .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
            manifest
                .validate()
                .unwrap_or_else(|error| panic!("validate {}: {error}", path.display()));
        }
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
scoring_mode = "whole-repo"
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

    #[test]
    fn scoring_mode_serializes_to_kebab_case_wire_strings() {
        // Byte-for-byte wire-string assertion (D-14). These would FAIL under
        // `rename_all = "snake_case"` (which emits `oracle_rta` / `oracle_jelly` /
        // `whole_repo`); the per-variant serde renames make them kebab-case.
        assert_eq!(
            serde_json::to_string(&ScoringMode::OracleRta).unwrap(),
            "\"oracle-rta\""
        );
        assert_eq!(
            serde_json::to_string(&ScoringMode::OracleJelly).unwrap(),
            "\"oracle-jelly\""
        );
        assert_eq!(
            serde_json::to_string(&ScoringMode::WholeRepo).unwrap(),
            "\"whole-repo\""
        );
        // The `as_wire_str` label mirrors the serde renames exactly.
        assert_eq!(ScoringMode::OracleRta.as_wire_str(), "oracle-rta");
        assert_eq!(ScoringMode::OracleJelly.as_wire_str(), "oracle-jelly");
        assert_eq!(ScoringMode::WholeRepo.as_wire_str(), "whole-repo");
    }

    #[test]
    fn scoring_mode_deserializes_from_kebab_case_wire_strings() {
        // A bare scalar deserializes through serde_json (TOML has no top-level
        // scalar form); the full TOML manifest path is covered by
        // `committed_suite_manifests_declare_the_expected_scoring_mode`.
        assert_eq!(
            serde_json::from_str::<ScoringMode>("\"oracle-rta\"").unwrap(),
            ScoringMode::OracleRta
        );
        assert_eq!(
            serde_json::from_str::<ScoringMode>("\"oracle-jelly\"").unwrap(),
            ScoringMode::OracleJelly
        );
        assert_eq!(
            serde_json::from_str::<ScoringMode>("\"whole-repo\"").unwrap(),
            ScoringMode::WholeRepo
        );
        // A snake_case form is NOT accepted — the wire contract is kebab-case.
        assert!(serde_json::from_str::<ScoringMode>("\"oracle_rta\"").is_err());
    }

    #[test]
    fn manifest_missing_scoring_mode_is_rejected_structurally() {
        // D-15 structural layer: a manifest TOML WITHOUT a `scoring_mode` key fails
        // deserialization because the field is non-`Option`.
        let raw = r#"
schema_version = "polint-eval-suite-1"
id = "native-fixtures"
name = "Native fixtures"
kind = "native_fixture"
languages = ["go"]
adapter_id = "native"
license = "repo"
language_support = "supported"

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
        assert!(
            err.to_string().contains("scoring_mode"),
            "missing scoring_mode error should name the field: {err}"
        );
    }

    #[test]
    fn manifest_with_invalid_scoring_mode_value_is_rejected_structurally() {
        // An unknown kebab value is not a `ScoringMode` variant — rejected at parse.
        let raw = r#"
schema_version = "polint-eval-suite-1"
id = "native-fixtures"
name = "Native fixtures"
kind = "native_fixture"
languages = ["go"]
adapter_id = "native"
scoring_mode = "not-a-mode"
license = "repo"
language_support = "supported"

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

        assert!(toml::from_str::<SuiteManifest>(raw).is_err());
    }

    #[test]
    fn validate_rejects_a_manifest_built_without_a_recognized_scoring_mode() {
        // D-15 explicit layer: even when a manifest is constructed in-memory (no
        // TOML parse), `validate()` guards the scoring_mode. A real `ScoringMode`
        // always maps to a recognized wire string, so validate() passes here — the
        // guard exists to fail closed if a future variant lacks a kebab rename.
        let manifest = manifest(SuiteLanguageSupport::Supported, "tests/eval-fixtures");
        assert_eq!(manifest.scoring_mode.as_wire_str(), "whole-repo");
        manifest
            .validate()
            .expect("recognized scoring_mode validates");
    }

    #[test]
    fn committed_suite_manifests_declare_the_expected_scoring_mode() {
        // D-16 round-trip: each committed suite TOML parses to its expected mode.
        let manifest_dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../research/evaluation-harness/suites");
        let load = |file: &str| -> ScoringMode {
            let raw = std::fs::read_to_string(manifest_dir.join(file)).unwrap();
            toml::from_str::<SuiteManifest>(&raw)
                .unwrap_or_else(|error| panic!("parse {file}: {error}"))
                .scoring_mode
        };
        assert_eq!(
            load("go-x-tools-rta-callgraph.toml"),
            ScoringMode::OracleRta
        );
        assert_eq!(load("jelly-callgraph-micro.toml"), ScoringMode::OracleJelly);
        assert_eq!(load("gosec-samples.toml"), ScoringMode::WholeRepo);
        assert_eq!(load("secbench-js-smoke.toml"), ScoringMode::WholeRepo);
    }

    fn manifest(language_support: SuiteLanguageSupport, checkout_path: &str) -> SuiteManifest {
        SuiteManifest {
            schema_version: "polint-eval-suite-1".to_string(),
            id: SuiteId("native-fixtures".to_string()),
            name: "Native fixtures".to_string(),
            kind: SuiteKind::NativeFixture,
            languages: vec!["go".to_string(), "typescript".to_string()],
            adapter_id: "native".to_string(),
            scoring_mode: ScoringMode::WholeRepo,
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
                        deterministic_seed: Some("determinism-seed".to_string()),
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

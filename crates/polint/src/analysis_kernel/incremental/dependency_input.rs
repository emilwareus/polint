#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "the complete typed input vocabulary includes dependency families that are emitted only by providers that declare them"
    )
)]

use serde::{Deserialize, Serialize};
use std::fmt;

use super::{Digest, DigestKind, InputComponentStatus};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InputDependencyKind {
    SourceFile,
    PackageProject,
    ProviderManifest,
    ProviderSchema,
    RequestedCapability,
    AnalysisSetting,
    LanguageLifecycle,
    ToolInvocation,
    Config,
    UpstreamLayer,
    SummaryDependency,
    QueryOption,
    BudgetProfile,
    SearchManifest,
    ExtensionCode,
    ExtensionDeclaredInput,
    Model,
    RuleCode,
    RuleOptions,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "InputDependencyKeyWire")]
pub(crate) struct InputDependencyKey {
    pub(crate) kind: InputDependencyKind,
    pub(crate) stable_key: String,
    pub(crate) digest: Digest,
    pub(crate) status: InputComponentStatus,
}

#[derive(Deserialize)]
struct InputDependencyKeyWire {
    kind: InputDependencyKind,
    stable_key: String,
    digest: Digest,
    status: InputComponentStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UnknownInputDependencyKindLabel {
    label: String,
}

impl fmt::Display for UnknownInputDependencyKindLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unknown input dependency kind label `{}`",
            self.label
        )
    }
}

impl std::error::Error for UnknownInputDependencyKindLabel {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InputDependencyDigestKindError {
    input_kind: InputDependencyKind,
    actual: DigestKind,
}

impl fmt::Display for InputDependencyDigestKindError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "input dependency kind `{}` does not accept digest kind `{}`",
            self.input_kind.label(),
            self.actual.label()
        )
    }
}

impl std::error::Error for InputDependencyDigestKindError {}

impl TryFrom<InputDependencyKeyWire> for InputDependencyKey {
    type Error = InputDependencyDigestKindError;

    fn try_from(wire: InputDependencyKeyWire) -> Result<Self, Self::Error> {
        Self::new(wire.kind, wire.stable_key, wire.digest, wire.status)
    }
}

impl InputDependencyKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::SourceFile => "source_file",
            Self::PackageProject => "package_project",
            Self::ProviderManifest => "provider_manifest",
            Self::ProviderSchema => "provider_schema",
            Self::RequestedCapability => "requested_capability",
            Self::AnalysisSetting => "analysis_setting",
            Self::LanguageLifecycle => "language_lifecycle",
            Self::ToolInvocation => "tool_invocation",
            Self::Config => "config",
            Self::UpstreamLayer => "upstream_layer",
            Self::SummaryDependency => "summary_dependency",
            Self::QueryOption => "query_option",
            Self::BudgetProfile => "budget_profile",
            Self::SearchManifest => "search_manifest",
            Self::ExtensionCode => "extension_code",
            Self::ExtensionDeclaredInput => "extension_declared_input",
            Self::Model => "model",
            Self::RuleCode => "rule_code",
            Self::RuleOptions => "rule_options",
        }
    }

    pub(crate) fn parse_label(label: &str) -> Result<Self, UnknownInputDependencyKindLabel> {
        match label {
            "source_file" => Ok(Self::SourceFile),
            "package_project" => Ok(Self::PackageProject),
            "provider_manifest" => Ok(Self::ProviderManifest),
            "provider_schema" => Ok(Self::ProviderSchema),
            "requested_capability" => Ok(Self::RequestedCapability),
            "analysis_setting" => Ok(Self::AnalysisSetting),
            "language_lifecycle" => Ok(Self::LanguageLifecycle),
            "tool_invocation" => Ok(Self::ToolInvocation),
            "config" => Ok(Self::Config),
            "upstream_layer" => Ok(Self::UpstreamLayer),
            "summary_dependency" => Ok(Self::SummaryDependency),
            "query_option" => Ok(Self::QueryOption),
            "budget_profile" => Ok(Self::BudgetProfile),
            "search_manifest" => Ok(Self::SearchManifest),
            "extension_code" => Ok(Self::ExtensionCode),
            "extension_declared_input" => Ok(Self::ExtensionDeclaredInput),
            "model" => Ok(Self::Model),
            "rule_code" => Ok(Self::RuleCode),
            "rule_options" => Ok(Self::RuleOptions),
            _ => Err(UnknownInputDependencyKindLabel {
                label: label.to_string(),
            }),
        }
    }

    fn accepts_digest_kind(self, digest_kind: DigestKind) -> bool {
        match self {
            Self::SourceFile => digest_kind == DigestKind::SourceText,
            Self::PackageProject => digest_kind == DigestKind::Workspace,
            Self::ProviderManifest | Self::ProviderSchema => {
                digest_kind == DigestKind::ProviderManifest
            }
            Self::RequestedCapability => digest_kind == DigestKind::AnalysisRequirements,
            Self::AnalysisSetting => digest_kind == DigestKind::AnalysisSettings,
            Self::LanguageLifecycle => {
                matches!(
                    digest_kind,
                    DigestKind::GoLifecycle | DigestKind::TsJsLifecycle
                )
            }
            Self::ToolInvocation => digest_kind == DigestKind::ToolInvocation,
            Self::Config => digest_kind == DigestKind::Config,
            Self::UpstreamLayer => digest_kind == DigestKind::DependencyLayer,
            Self::SummaryDependency => digest_kind == DigestKind::SummaryDependency,
            Self::QueryOption => digest_kind == DigestKind::QueryParameters,
            Self::BudgetProfile => digest_kind == DigestKind::Budget,
            Self::SearchManifest => digest_kind == DigestKind::Dependency,
            Self::ExtensionCode | Self::ExtensionDeclaredInput => {
                digest_kind == DigestKind::ExtensionCode
            }
            Self::Model => digest_kind == DigestKind::ModelFile,
            Self::RuleCode => digest_kind == DigestKind::RuleCode,
            Self::RuleOptions => digest_kind == DigestKind::RuleOptions,
        }
    }
}

impl InputDependencyKey {
    fn new(
        kind: InputDependencyKind,
        stable_key: impl Into<String>,
        digest: Digest,
        status: InputComponentStatus,
    ) -> Result<Self, InputDependencyDigestKindError> {
        if !kind.accepts_digest_kind(digest.kind) {
            return Err(InputDependencyDigestKindError {
                input_kind: kind,
                actual: digest.kind,
            });
        }

        Ok(Self {
            kind,
            stable_key: stable_key.into(),
            digest,
            status,
        })
    }

    pub(crate) fn source_file(
        stable_key: impl Into<String>,
        digest: Digest,
        status: InputComponentStatus,
    ) -> Result<Self, InputDependencyDigestKindError> {
        Self::new(InputDependencyKind::SourceFile, stable_key, digest, status)
    }

    pub(crate) fn package_project(
        stable_key: impl Into<String>,
        digest: Digest,
        status: InputComponentStatus,
    ) -> Result<Self, InputDependencyDigestKindError> {
        Self::new(
            InputDependencyKind::PackageProject,
            stable_key,
            digest,
            status,
        )
    }

    pub(crate) fn provider_manifest(
        stable_key: impl Into<String>,
        digest: Digest,
        status: InputComponentStatus,
    ) -> Result<Self, InputDependencyDigestKindError> {
        Self::new(
            InputDependencyKind::ProviderManifest,
            stable_key,
            digest,
            status,
        )
    }

    pub(crate) fn provider_schema(
        stable_key: impl Into<String>,
        digest: Digest,
        status: InputComponentStatus,
    ) -> Result<Self, InputDependencyDigestKindError> {
        Self::new(
            InputDependencyKind::ProviderSchema,
            stable_key,
            digest,
            status,
        )
    }

    pub(crate) fn requested_capability(
        stable_key: impl Into<String>,
        digest: Digest,
        status: InputComponentStatus,
    ) -> Result<Self, InputDependencyDigestKindError> {
        Self::new(
            InputDependencyKind::RequestedCapability,
            stable_key,
            digest,
            status,
        )
    }

    pub(crate) fn analysis_setting(
        stable_key: impl Into<String>,
        digest: Digest,
        status: InputComponentStatus,
    ) -> Result<Self, InputDependencyDigestKindError> {
        Self::new(
            InputDependencyKind::AnalysisSetting,
            stable_key,
            digest,
            status,
        )
    }

    pub(crate) fn language_lifecycle(
        stable_key: impl Into<String>,
        digest: Digest,
        status: InputComponentStatus,
    ) -> Result<Self, InputDependencyDigestKindError> {
        Self::new(
            InputDependencyKind::LanguageLifecycle,
            stable_key,
            digest,
            status,
        )
    }

    pub(crate) fn tool_invocation(
        stable_key: impl Into<String>,
        digest: Digest,
        status: InputComponentStatus,
    ) -> Result<Self, InputDependencyDigestKindError> {
        Self::new(
            InputDependencyKind::ToolInvocation,
            stable_key,
            digest,
            status,
        )
    }

    pub(crate) fn config(
        stable_key: impl Into<String>,
        digest: Digest,
        status: InputComponentStatus,
    ) -> Result<Self, InputDependencyDigestKindError> {
        Self::new(InputDependencyKind::Config, stable_key, digest, status)
    }

    pub(crate) fn upstream_layer(
        stable_key: impl Into<String>,
        digest: Digest,
        status: InputComponentStatus,
    ) -> Result<Self, InputDependencyDigestKindError> {
        Self::new(
            InputDependencyKind::UpstreamLayer,
            stable_key,
            digest,
            status,
        )
    }

    pub(crate) fn summary_dependency(
        stable_key: impl Into<String>,
        digest: Digest,
        status: InputComponentStatus,
    ) -> Result<Self, InputDependencyDigestKindError> {
        Self::new(
            InputDependencyKind::SummaryDependency,
            stable_key,
            digest,
            status,
        )
    }

    pub(crate) fn query_option(
        stable_key: impl Into<String>,
        digest: Digest,
        status: InputComponentStatus,
    ) -> Result<Self, InputDependencyDigestKindError> {
        Self::new(InputDependencyKind::QueryOption, stable_key, digest, status)
    }

    pub(crate) fn budget_profile(
        stable_key: impl Into<String>,
        digest: Digest,
        status: InputComponentStatus,
    ) -> Result<Self, InputDependencyDigestKindError> {
        Self::new(
            InputDependencyKind::BudgetProfile,
            stable_key,
            digest,
            status,
        )
    }

    pub(crate) fn search_manifest(
        stable_key: impl Into<String>,
        digest: Digest,
        status: InputComponentStatus,
    ) -> Result<Self, InputDependencyDigestKindError> {
        Self::new(
            InputDependencyKind::SearchManifest,
            stable_key,
            digest,
            status,
        )
    }

    pub(crate) fn extension_code(
        stable_key: impl Into<String>,
        digest: Digest,
        status: InputComponentStatus,
    ) -> Result<Self, InputDependencyDigestKindError> {
        Self::new(
            InputDependencyKind::ExtensionCode,
            stable_key,
            digest,
            status,
        )
    }

    pub(crate) fn extension_declared_input(
        stable_key: impl Into<String>,
        digest: Digest,
        status: InputComponentStatus,
    ) -> Result<Self, InputDependencyDigestKindError> {
        Self::new(
            InputDependencyKind::ExtensionDeclaredInput,
            stable_key,
            digest,
            status,
        )
    }

    pub(crate) fn model(
        stable_key: impl Into<String>,
        digest: Digest,
        status: InputComponentStatus,
    ) -> Result<Self, InputDependencyDigestKindError> {
        Self::new(InputDependencyKind::Model, stable_key, digest, status)
    }

    pub(crate) fn rule_code(
        stable_key: impl Into<String>,
        digest: Digest,
        status: InputComponentStatus,
    ) -> Result<Self, InputDependencyDigestKindError> {
        Self::new(InputDependencyKind::RuleCode, stable_key, digest, status)
    }

    pub(crate) fn rule_options(
        stable_key: impl Into<String>,
        digest: Digest,
        status: InputComponentStatus,
    ) -> Result<Self, InputDependencyDigestKindError> {
        Self::new(InputDependencyKind::RuleOptions, stable_key, digest, status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KINDS: &[(InputDependencyKind, &str, DigestKind)] = &[
        (
            InputDependencyKind::SourceFile,
            "source_file",
            DigestKind::SourceText,
        ),
        (
            InputDependencyKind::PackageProject,
            "package_project",
            DigestKind::Workspace,
        ),
        (
            InputDependencyKind::ProviderManifest,
            "provider_manifest",
            DigestKind::ProviderManifest,
        ),
        (
            InputDependencyKind::ProviderSchema,
            "provider_schema",
            DigestKind::ProviderManifest,
        ),
        (
            InputDependencyKind::RequestedCapability,
            "requested_capability",
            DigestKind::AnalysisRequirements,
        ),
        (
            InputDependencyKind::AnalysisSetting,
            "analysis_setting",
            DigestKind::AnalysisSettings,
        ),
        (
            InputDependencyKind::LanguageLifecycle,
            "language_lifecycle",
            DigestKind::GoLifecycle,
        ),
        (
            InputDependencyKind::ToolInvocation,
            "tool_invocation",
            DigestKind::ToolInvocation,
        ),
        (InputDependencyKind::Config, "config", DigestKind::Config),
        (
            InputDependencyKind::UpstreamLayer,
            "upstream_layer",
            DigestKind::DependencyLayer,
        ),
        (
            InputDependencyKind::SummaryDependency,
            "summary_dependency",
            DigestKind::SummaryDependency,
        ),
        (
            InputDependencyKind::QueryOption,
            "query_option",
            DigestKind::QueryParameters,
        ),
        (
            InputDependencyKind::BudgetProfile,
            "budget_profile",
            DigestKind::Budget,
        ),
        (
            InputDependencyKind::SearchManifest,
            "search_manifest",
            DigestKind::Dependency,
        ),
        (
            InputDependencyKind::ExtensionCode,
            "extension_code",
            DigestKind::ExtensionCode,
        ),
        (
            InputDependencyKind::ExtensionDeclaredInput,
            "extension_declared_input",
            DigestKind::ExtensionCode,
        ),
        (InputDependencyKind::Model, "model", DigestKind::ModelFile),
        (
            InputDependencyKind::RuleCode,
            "rule_code",
            DigestKind::RuleCode,
        ),
        (
            InputDependencyKind::RuleOptions,
            "rule_options",
            DigestKind::RuleOptions,
        ),
    ];

    const STATUSES: &[InputComponentStatus] = &[
        InputComponentStatus::Present,
        InputComponentStatus::Absent,
        InputComponentStatus::Unsupported,
        InputComponentStatus::SetupMissing,
    ];

    fn construct(
        kind: InputDependencyKind,
        stable_key: &str,
        digest: Digest,
        status: InputComponentStatus,
    ) -> Result<InputDependencyKey, InputDependencyDigestKindError> {
        match kind {
            InputDependencyKind::SourceFile => {
                InputDependencyKey::source_file(stable_key, digest, status)
            }
            InputDependencyKind::PackageProject => {
                InputDependencyKey::package_project(stable_key, digest, status)
            }
            InputDependencyKind::ProviderManifest => {
                InputDependencyKey::provider_manifest(stable_key, digest, status)
            }
            InputDependencyKind::ProviderSchema => {
                InputDependencyKey::provider_schema(stable_key, digest, status)
            }
            InputDependencyKind::RequestedCapability => {
                InputDependencyKey::requested_capability(stable_key, digest, status)
            }
            InputDependencyKind::AnalysisSetting => {
                InputDependencyKey::analysis_setting(stable_key, digest, status)
            }
            InputDependencyKind::LanguageLifecycle => {
                InputDependencyKey::language_lifecycle(stable_key, digest, status)
            }
            InputDependencyKind::ToolInvocation => {
                InputDependencyKey::tool_invocation(stable_key, digest, status)
            }
            InputDependencyKind::Config => InputDependencyKey::config(stable_key, digest, status),
            InputDependencyKind::UpstreamLayer => {
                InputDependencyKey::upstream_layer(stable_key, digest, status)
            }
            InputDependencyKind::SummaryDependency => {
                InputDependencyKey::summary_dependency(stable_key, digest, status)
            }
            InputDependencyKind::QueryOption => {
                InputDependencyKey::query_option(stable_key, digest, status)
            }
            InputDependencyKind::BudgetProfile => {
                InputDependencyKey::budget_profile(stable_key, digest, status)
            }
            InputDependencyKind::SearchManifest => {
                InputDependencyKey::search_manifest(stable_key, digest, status)
            }
            InputDependencyKind::ExtensionCode => {
                InputDependencyKey::extension_code(stable_key, digest, status)
            }
            InputDependencyKind::ExtensionDeclaredInput => {
                InputDependencyKey::extension_declared_input(stable_key, digest, status)
            }
            InputDependencyKind::Model => InputDependencyKey::model(stable_key, digest, status),
            InputDependencyKind::RuleCode => {
                InputDependencyKey::rule_code(stable_key, digest, status)
            }
            InputDependencyKind::RuleOptions => {
                InputDependencyKey::rule_options(stable_key, digest, status)
            }
        }
    }

    #[test]
    fn kind_labels_round_trip_and_reject_unknown_values() {
        for (kind, label, _) in KINDS {
            assert_eq!(kind.label(), *label);
            assert_eq!(InputDependencyKind::parse_label(label), Ok(*kind));
        }

        let error = InputDependencyKind::parse_label("source").unwrap_err();
        assert_eq!(
            error.to_string(),
            "unknown input dependency kind label `source`"
        );
    }

    #[test]
    fn constructors_preserve_kind_key_digest_and_every_status() {
        for (kind, label, digest_kind) in KINDS {
            for status in STATUSES {
                let digest = Digest::from_parts(*digest_kind, "dependency_input", &[*label]);
                let key = construct(*kind, label, digest.clone(), *status).unwrap();

                assert_eq!(key.kind, *kind);
                assert_eq!(key.stable_key, *label);
                assert_eq!(key.digest, digest);
                assert_eq!(key.status, *status);
            }
        }
    }

    #[test]
    fn every_constructor_rejects_a_mismatched_digest_purpose() {
        for (kind, label, _) in KINDS {
            let error = construct(
                *kind,
                label,
                Digest::from_parts(DigestKind::Evidence, "mismatch", &[*label]),
                InputComponentStatus::Present,
            )
            .unwrap_err();

            assert_eq!(
                error,
                InputDependencyDigestKindError {
                    input_kind: *kind,
                    actual: DigestKind::Evidence,
                }
            );
            assert_eq!(
                error.to_string(),
                format!("input dependency kind `{label}` does not accept digest kind `evidence`")
            );
        }
    }

    #[test]
    fn language_lifecycle_accepts_both_existing_language_digest_purposes() {
        for digest_kind in [DigestKind::GoLifecycle, DigestKind::TsJsLifecycle] {
            let digest = Digest::from_parts(digest_kind, "lifecycle", &[digest_kind.label()]);
            let key = InputDependencyKey::language_lifecycle(
                digest_kind.label(),
                digest.clone(),
                InputComponentStatus::Present,
            )
            .unwrap();

            assert_eq!(key.digest, digest);
        }
    }

    #[test]
    fn serde_round_trips_every_kind_and_status_with_exact_labels() {
        for (kind, label, digest_kind) in KINDS {
            for status in STATUSES {
                let key = construct(
                    *kind,
                    label,
                    Digest::from_parts(*digest_kind, "dependency_input", &[*label]),
                    *status,
                )
                .unwrap();

                let wire = serde_json::to_value(&key).expect("typed input serializes");
                assert_eq!(wire["kind"], *label);
                assert_eq!(
                    serde_json::from_value::<InputDependencyKey>(wire)
                        .expect("typed input deserializes"),
                    key
                );
            }
        }
    }

    #[test]
    fn deserialization_rejects_a_kind_digest_purpose_mismatch() {
        let wire = serde_json::json!({
            "kind": "source_file",
            "stable_key": "src/main.ts",
            "digest": Digest::from_parts(DigestKind::Config, "wrong", &["wrong"]),
            "status": "present",
        });

        let error = serde_json::from_value::<InputDependencyKey>(wire)
            .expect_err("mismatched typed input must fail closed");

        assert!(
            error.to_string().contains(
                "input dependency kind `source_file` does not accept digest kind `config`"
            )
        );
    }
}

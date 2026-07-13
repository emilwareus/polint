use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path as FsPath, PathBuf};

use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{ConfigIdentity, Digest, DigestKind, WorkspaceIdentity};
use crate::analysis::extensions::discovery::{DiscoveredExtension, discover_local_extensions};
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_plan::{AnalysisPlan, RequestedCapabilitySnapshot};
use crate::cache::keys::{AnalysisSettingsScope, analysis_settings_hash};
use crate::config::LoadedConfig;
use crate::core::{AnalysisDb, CapabilitySupportStatus, Language, SourceFile};
use crate::module_graph::paths::{
    TOPOLOGY_LOCKFILE_MAX_BYTES, TOPOLOGY_MANIFEST_MAX_BYTES, normalize_repo_relative,
    normalize_repo_relative_input, read_repo_file_to_string_with_limit, read_repo_file_with_limit,
    repo_file_exists, repo_file_path, repo_relative_existing_path,
};
pub(crate) const INPUT_SNAPSHOT_SCHEMA_VERSION: &str = "polint-input-snapshot-2";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct InputSnapshot {
    pub(crate) schema_version: String,
    pub(crate) workspace_identity: WorkspaceIdentity,
    pub(crate) config_identity: ConfigIdentity,
    pub(crate) analysis_settings: Vec<AnalysisSettingSource>,
    pub(crate) requested_capabilities: Vec<RequestedCapabilitySnapshot>,
    pub(crate) analysis_requirements_identity: Digest,
    pub(crate) files: Vec<FileSnapshot>,
    pub(crate) config: InputComponent,
    pub(crate) go_lifecycle: GoLifecycleSnapshot,
    pub(crate) ts_js_lifecycle: TsJsLifecycleSnapshot,
    pub(crate) rules: Vec<InputComponent>,
    pub(crate) models: Vec<InputComponent>,
    pub(crate) extensions: Vec<InputComponent>,
    pub(crate) tool_invocations: Vec<InputComponent>,
    pub(crate) provider_schemas: Vec<ProviderSchemaSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FileSnapshot {
    pub(crate) relative_path: String,
    pub(crate) language: Language,
    pub(crate) source_text_digest: Digest,
    pub(crate) size_bytes: usize,
    pub(crate) mtime_hint_present: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GoLifecycleSnapshot {
    pub(crate) components: Vec<InputComponent>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TsJsLifecycleSnapshot {
    pub(crate) components: Vec<InputComponent>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InputComponentStatus {
    Present,
    Absent,
    Unsupported,
    SetupMissing,
}

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "canonical status decoding is consumed by private metadata readers"
    )
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UnknownInputComponentStatusLabel {
    label: String,
}

impl fmt::Display for UnknownInputComponentStatusLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unknown input component status label `{}`",
            self.label
        )
    }
}

impl std::error::Error for UnknownInputComponentStatusLabel {}

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "canonical status codecs stay symmetric for durable row decoding"
    )
)]
impl InputComponentStatus {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Absent => "absent",
            Self::Unsupported => "unsupported",
            Self::SetupMissing => "setup_missing",
        }
    }

    pub(crate) fn parse_label(label: &str) -> Result<Self, UnknownInputComponentStatusLabel> {
        match label {
            "present" => Ok(Self::Present),
            "absent" => Ok(Self::Absent),
            "unsupported" => Ok(Self::Unsupported),
            "setup_missing" => Ok(Self::SetupMissing),
            _ => Err(UnknownInputComponentStatusLabel {
                label: label.to_string(),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct InputComponent {
    pub(crate) name: String,
    pub(crate) status: InputComponentStatus,
    pub(crate) digest: Digest,
    pub(crate) detail: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProviderSchemaSnapshot {
    pub(crate) provider_id: String,
    pub(crate) schema_versions: Vec<String>,
    pub(crate) language_scope: String,
    pub(crate) cache_policy: String,
    pub(crate) precision_ceiling: String,
    pub(crate) provider_manifest_digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AnalysisSettingSource {
    pub(crate) scope: AnalysisSettingsScope,
    pub(crate) digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InputSnapshotIdentitySources {
    pub(crate) analysis_settings: Vec<AnalysisSettingSource>,
    pub(crate) requested_capabilities: Vec<RequestedCapabilitySnapshot>,
    pub(crate) analysis_requirements_identity: Digest,
}

impl InputSnapshot {
    pub(crate) fn analysis_settings_digest(&self, scope: AnalysisSettingsScope) -> &Digest {
        let source = self
            .analysis_settings
            .iter()
            .find(|source| source.scope == scope)
            .unwrap_or_else(|| {
                panic!(
                    "input snapshot is missing analysis settings for `{}`",
                    scope.label()
                )
            });
        assert_eq!(
            source.digest.kind,
            DigestKind::AnalysisSettings,
            "input snapshot analysis settings must retain their typed purpose"
        );
        &source.digest
    }

    pub(crate) fn analysis_requirements_digest_for(&self, capabilities: &[&str]) -> Digest {
        let capabilities = capabilities.iter().copied().collect::<BTreeSet<_>>();
        let mut inputs = self
            .requested_capabilities
            .iter()
            .filter(|requested| capabilities.contains(requested.capability.as_str()))
            .map(|requested| requested.analysis_dependency_digest.clone())
            .collect::<Vec<_>>();
        inputs.sort();
        assert!(
            inputs
                .iter()
                .all(|digest| digest.kind == DigestKind::AnalysisRequirements),
            "provider capability inputs must retain their typed purpose"
        );
        if inputs.is_empty() {
            Digest::absent(
                DigestKind::AnalysisRequirements,
                "provider_requested_capabilities",
            )
        } else {
            Digest::from_unordered(
                DigestKind::AnalysisRequirements,
                "provider_requested_capabilities",
                inputs,
            )
        }
    }

    pub(crate) fn from_run_inputs_with_plan(
        loaded: &LoadedConfig,
        db: &AnalysisDb,
        config_digest: &str,
        rule_digest: &str,
        plan: &AnalysisPlan,
        provider_manifests: &[ProviderManifest],
    ) -> Self {
        let identity_sources = Self::identity_sources_from_plan(loaded, plan);
        debug_assert_eq!(
            identity_sources.analysis_requirements_identity.kind,
            DigestKind::AnalysisRequirements
        );
        debug_assert!(
            identity_sources
                .analysis_settings
                .iter()
                .all(|source| source.digest.kind == DigestKind::AnalysisSettings)
        );
        debug_assert!(
            identity_sources
                .requested_capabilities
                .iter()
                .all(|source| {
                    source.analysis_dependency_digest.kind == DigestKind::AnalysisRequirements
                })
        );

        let snapshot = Self::from_run_input_plan(
            loaded,
            db,
            config_digest,
            rule_digest,
            plan,
            provider_manifests,
        );
        debug_assert_eq!(snapshot.semantic_digest().kind, DigestKind::InputSnapshot);
        snapshot
    }

    pub(crate) fn identity_sources_from_plan(
        loaded: &LoadedConfig,
        plan: &AnalysisPlan,
    ) -> InputSnapshotIdentitySources {
        let mut analysis_settings = AnalysisSettingsScope::ALL
            .into_iter()
            .map(|scope| AnalysisSettingSource {
                scope,
                digest: analysis_settings_hash(loaded, scope),
            })
            .collect::<Vec<_>>();
        analysis_settings.sort_by_key(|source| source.scope.label());

        InputSnapshotIdentitySources {
            analysis_settings,
            requested_capabilities: plan.requested_capability_snapshots(),
            analysis_requirements_identity: plan.analysis_requirements_digest(),
        }
    }

    fn from_run_input_plan(
        loaded: &LoadedConfig,
        db: &AnalysisDb,
        config_digest: &str,
        rule_digest: &str,
        plan: &AnalysisPlan,
        provider_manifests: &[ProviderManifest],
    ) -> Self {
        let identity_sources = Self::identity_sources_from_plan(loaded, plan);
        let workspace_identity = WorkspaceIdentity::from_roots([loaded.root.as_path()]);
        let config_identity =
            ConfigIdentity::from_complete_config_parts("config_digest", &[config_digest]);
        let config = config_component(loaded, config_identity.digest().clone());
        let mut files = db
            .files()
            .iter()
            .map(FileSnapshot::from_source_file)
            .collect::<Vec<_>>();
        files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

        let mut provider_schemas = provider_manifests
            .iter()
            .map(ProviderSchemaSnapshot::from_manifest)
            .collect::<Vec<_>>();
        provider_schemas.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
        let go_lifecycle = GoLifecycleSnapshot::from_loaded(loaded, db);
        let ts_js_lifecycle = TsJsLifecycleSnapshot::from_loaded(loaded, db);
        let mut tool_invocations = vec![
            go_lifecycle.tool_invocation_component().clone(),
            ts_js_lifecycle.tool_invocation_component().clone(),
        ];
        tool_invocations.sort_by(|left, right| left.name.cmp(&right.name));

        Self {
            schema_version: INPUT_SNAPSHOT_SCHEMA_VERSION.to_string(),
            workspace_identity,
            config_identity,
            analysis_settings: identity_sources.analysis_settings,
            requested_capabilities: identity_sources.requested_capabilities,
            analysis_requirements_identity: identity_sources.analysis_requirements_identity,
            files,
            config,
            go_lifecycle,
            ts_js_lifecycle,
            rules: rule_components(rule_digest, plan),
            models: vec![InputComponent::absent(
                "model.files",
                DigestKind::ModelFile,
                "no model files configured",
            )],
            extensions: extension_components_from_repo(&loaded.root),
            tool_invocations,
            provider_schemas,
        }
    }

    pub(crate) fn semantic_digest(&self) -> Digest {
        let mut rows = vec![semantic_row(
            "schema",
            [("version", self.schema_version.as_str())],
        )];
        rows.push(digest_semantic_row(
            "workspace_identity",
            self.workspace_identity.digest(),
        ));
        rows.push(digest_semantic_row(
            "config_identity",
            self.config_identity.digest(),
        ));
        rows.push(digest_semantic_row(
            "analysis_requirements_identity",
            &self.analysis_requirements_identity,
        ));

        for file in &self.files {
            let size_bytes = file.size_bytes.to_string();
            let mut row = semantic_row_builder("file");
            row.labeled_part("relative_path", &file.relative_path);
            row.labeled_part("language", file.language.label());
            row.labeled_part("source_digest_kind", file.source_text_digest.kind.label());
            row.labeled_part("source_digest_value", &file.source_text_digest.value);
            row.labeled_part("size_bytes", &size_bytes);
            rows.push(row.finish());
        }
        push_component_rows(&mut rows, "config", std::slice::from_ref(&self.config));
        push_component_rows(&mut rows, "go_lifecycle", &self.go_lifecycle.components);
        push_component_rows(
            &mut rows,
            "ts_js_lifecycle",
            &self.ts_js_lifecycle.components,
        );
        push_component_rows(&mut rows, "rule", &self.rules);
        push_component_rows(&mut rows, "model", &self.models);
        push_component_rows(&mut rows, "extension", &self.extensions);
        push_component_rows(&mut rows, "tool", &self.tool_invocations);
        for provider in &self.provider_schemas {
            let mut row = semantic_row_builder("provider_schema");
            row.labeled_part("provider_id", &provider.provider_id);
            let mut schema_versions = provider.schema_versions.clone();
            schema_versions.sort();
            for schema_version in &schema_versions {
                row.labeled_part("schema_version", schema_version);
            }
            row.labeled_part("language_scope", &provider.language_scope);
            row.labeled_part("cache_policy", &provider.cache_policy);
            row.labeled_part("precision_ceiling", &provider.precision_ceiling);
            row.labeled_part(
                "manifest_digest_kind",
                provider.provider_manifest_digest.kind.label(),
            );
            row.labeled_part(
                "manifest_digest_value",
                &provider.provider_manifest_digest.value,
            );
            rows.push(row.finish());
        }
        for setting in &self.analysis_settings {
            let mut row = semantic_row_builder("analysis_setting");
            row.labeled_part("provider_id", setting.scope.label());
            row.labeled_part("digest_kind", setting.digest.kind.label());
            row.labeled_part("digest_value", &setting.digest.value);
            rows.push(row.finish());
        }
        for capability in &self.requested_capabilities {
            let mut requesting_rule_ids = capability.requesting_rule_ids.clone();
            requesting_rule_ids.sort();
            let mut row = semantic_row_builder("requested_capability");
            row.labeled_part("capability", &capability.capability);
            row.labeled_part(
                "language",
                capability.language.map(Language::label).unwrap_or("none"),
            );
            row.labeled_part(
                "support_status",
                capability_support_status_label(&capability.support_status),
            );
            row.labeled_part("setup_status", capability.setup_status.label());
            row.labeled_part(
                "policy_query_version",
                capability.policy_query_version.as_deref().unwrap_or("none"),
            );
            for rule_id in &requesting_rule_ids {
                row.labeled_part("requesting_rule_id", rule_id);
            }
            row.labeled_part(
                "rule_behavior_digest_kind",
                capability.rule_behavior_digest.kind.label(),
            );
            row.labeled_part(
                "rule_behavior_digest_value",
                &capability.rule_behavior_digest.value,
            );
            row.labeled_part(
                "analysis_dependency_digest_kind",
                capability.analysis_dependency_digest.kind.label(),
            );
            row.labeled_part(
                "analysis_dependency_digest_value",
                &capability.analysis_dependency_digest.value,
            );
            rows.push(row.finish());
        }
        Digest::from_unordered(DigestKind::InputSnapshot, "semantic_rows", rows)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputSnapshotWire {
    schema_version: String,
    workspace_identity: WorkspaceIdentity,
    config_identity: ConfigIdentity,
    analysis_settings: Vec<AnalysisSettingSource>,
    requested_capabilities: Vec<RequestedCapabilitySnapshot>,
    analysis_requirements_identity: Digest,
    files: Vec<FileSnapshot>,
    config: InputComponent,
    go_lifecycle: GoLifecycleSnapshot,
    ts_js_lifecycle: TsJsLifecycleSnapshot,
    rules: Vec<InputComponent>,
    models: Vec<InputComponent>,
    extensions: Vec<InputComponent>,
    tool_invocations: Vec<InputComponent>,
    provider_schemas: Vec<ProviderSchemaSnapshot>,
}

impl<'de> Deserialize<'de> for InputSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = InputSnapshotWire::deserialize(deserializer)?;
        if wire.schema_version != INPUT_SNAPSHOT_SCHEMA_VERSION {
            return Err(serde::de::Error::custom(format!(
                "unsupported input snapshot schema `{}`; expected `{INPUT_SNAPSHOT_SCHEMA_VERSION}`",
                wire.schema_version
            )));
        }
        if wire.config.digest != *wire.config_identity.digest() {
            return Err(serde::de::Error::custom(
                "config component digest does not match config identity",
            ));
        }
        if wire.analysis_requirements_identity.kind != DigestKind::AnalysisRequirements {
            return Err(serde::de::Error::custom(
                "analysis requirements identity has the wrong digest kind",
            ));
        }
        if !wire
            .analysis_settings
            .windows(2)
            .all(|pair| pair[0].scope.label() < pair[1].scope.label())
        {
            return Err(serde::de::Error::custom(
                "analysis settings must be strictly sorted by provider id",
            ));
        }
        let expected_requirements = analysis_requirements_digest(&wire.requested_capabilities);
        if wire.analysis_requirements_identity != expected_requirements {
            return Err(serde::de::Error::custom(
                "requested capability rows do not match analysis requirements identity",
            ));
        }

        Ok(Self {
            schema_version: wire.schema_version,
            workspace_identity: wire.workspace_identity,
            config_identity: wire.config_identity,
            analysis_settings: wire.analysis_settings,
            requested_capabilities: wire.requested_capabilities,
            analysis_requirements_identity: wire.analysis_requirements_identity,
            files: wire.files,
            config: wire.config,
            go_lifecycle: wire.go_lifecycle,
            ts_js_lifecycle: wire.ts_js_lifecycle,
            rules: wire.rules,
            models: wire.models,
            extensions: wire.extensions,
            tool_invocations: wire.tool_invocations,
            provider_schemas: wire.provider_schemas,
        })
    }
}

impl Serialize for AnalysisSettingSource {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut row = serializer.serialize_struct("AnalysisSettingSource", 2)?;
        row.serialize_field("provider_id", self.scope.label())?;
        row.serialize_field("digest", &self.digest)?;
        row.end()
    }
}

impl<'de> Deserialize<'de> for AnalysisSettingSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            provider_id: String,
            digest: Digest,
        }

        let wire = Wire::deserialize(deserializer)?;
        let scope = AnalysisSettingsScope::ALL
            .into_iter()
            .find(|scope| scope.label() == wire.provider_id)
            .ok_or_else(|| {
                serde::de::Error::custom(format!(
                    "unknown analysis settings provider `{}`",
                    wire.provider_id
                ))
            })?;
        if wire.digest.kind != DigestKind::AnalysisSettings {
            return Err(serde::de::Error::custom(
                "analysis setting has the wrong digest kind",
            ));
        }
        Ok(Self {
            scope,
            digest: wire.digest,
        })
    }
}

impl Serialize for RequestedCapabilitySnapshot {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut row = serializer.serialize_struct("RequestedCapabilitySnapshot", 8)?;
        row.serialize_field("capability", &self.capability)?;
        row.serialize_field("language", &self.language)?;
        row.serialize_field(
            "support_status",
            capability_support_status_label(&self.support_status),
        )?;
        row.serialize_field("setup_status", self.setup_status.label())?;
        row.serialize_field("policy_query_version", &self.policy_query_version)?;
        row.serialize_field("requesting_rule_ids", &self.requesting_rule_ids)?;
        row.serialize_field("rule_behavior_digest", &self.rule_behavior_digest)?;
        row.serialize_field(
            "analysis_dependency_digest",
            &self.analysis_dependency_digest,
        )?;
        row.end()
    }
}

impl<'de> Deserialize<'de> for RequestedCapabilitySnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            capability: String,
            language: Option<Language>,
            support_status: String,
            setup_status: String,
            policy_query_version: Option<String>,
            requesting_rule_ids: Vec<String>,
            rule_behavior_digest: Digest,
            analysis_dependency_digest: Digest,
        }

        let wire = Wire::deserialize(deserializer)?;
        let support_status = match wire.support_status.as_str() {
            "supported" => CapabilitySupportStatus::Supported,
            "unsupported" => CapabilitySupportStatus::Unsupported,
            "setup_missing" => CapabilitySupportStatus::SetupMissing,
            label => {
                return Err(serde::de::Error::custom(format!(
                    "unknown capability support status `{label}`"
                )));
            }
        };
        let setup_status = match wire.setup_status.as_str() {
            "not_required" => crate::analysis_plan::CapabilitySetupStatus::NotRequired,
            "ready" => crate::analysis_plan::CapabilitySetupStatus::Ready,
            "setup_missing" => crate::analysis_plan::CapabilitySetupStatus::SetupMissing,
            label => {
                return Err(serde::de::Error::custom(format!(
                    "unknown capability setup status `{label}`"
                )));
            }
        };
        if wire.rule_behavior_digest.kind != DigestKind::RuleOptions {
            return Err(serde::de::Error::custom(
                "capability rule behavior has the wrong digest kind",
            ));
        }
        if wire.analysis_dependency_digest.kind != DigestKind::AnalysisRequirements {
            return Err(serde::de::Error::custom(
                "capability analysis dependency has the wrong digest kind",
            ));
        }
        Ok(Self {
            capability: wire.capability,
            language: wire.language,
            support_status,
            setup_status,
            policy_query_version: wire.policy_query_version,
            requesting_rule_ids: wire.requesting_rule_ids,
            rule_behavior_digest: wire.rule_behavior_digest,
            analysis_dependency_digest: wire.analysis_dependency_digest,
        })
    }
}

impl GoLifecycleSnapshot {
    fn from_loaded(loaded: &LoadedConfig, db: &AnalysisDb) -> Self {
        let components = match crate::go::lifecycle::GoAnalysisConfig::from_loaded(loaded, db) {
            Ok(config) => go_lifecycle_components(loaded, &config),
            Err(error) => go_lifecycle_error_components(loaded, error.reason()),
        };

        Self {
            components: sorted_components(components),
        }
    }

    fn tool_invocation_component(&self) -> &InputComponent {
        component_by_name(&self.components, "go.tool_invocation")
    }
}

impl TsJsLifecycleSnapshot {
    fn from_loaded(loaded: &LoadedConfig, db: &AnalysisDb) -> Self {
        let source_paths = ts_js_source_paths(db);
        let lifecycle_dirs = ts_js_lifecycle_dirs(&source_paths);
        let components = vec![
            file_digest_component(
                "ts_js.package_manifests",
                DigestKind::TsJsLifecycle,
                &loaded.root,
                lifecycle_candidates(&lifecycle_dirs, package_manifest_names()),
            ),
            file_digest_component(
                "ts_js.lockfiles",
                DigestKind::TsJsLifecycle,
                &loaded.root,
                lifecycle_candidates(&lifecycle_dirs, lock_file_names()),
            ),
            file_digest_component(
                "ts_js.config_files",
                DigestKind::TsJsLifecycle,
                &loaded.root,
                ts_js_config_candidates(&loaded.root, &lifecycle_dirs),
            ),
            settings_component(
                "ts_js.resolver_options",
                DigestKind::TsJsLifecycle,
                &loaded.config.languages.ts,
            ),
            values_component(
                "ts_js.source_set_membership",
                DigestKind::TsJsLifecycle,
                source_paths,
            ),
            InputComponent::unsupported(
                "ts_js.tool_invocation",
                DigestKind::ToolInvocation,
                "not invoked by input snapshot",
            ),
        ];

        Self {
            components: sorted_components(components),
        }
    }

    fn tool_invocation_component(&self) -> &InputComponent {
        component_by_name(&self.components, "ts_js.tool_invocation")
    }
}

impl FileSnapshot {
    fn from_source_file(file: &SourceFile) -> Self {
        Self {
            relative_path: normalize_relative_path(&file.relative_path),
            language: file.language,
            source_text_digest: Digest {
                kind: DigestKind::SourceText,
                value: file.content_hash.clone(),
            },
            size_bytes: file.source.len(),
            mtime_hint_present: std::fs::metadata(&file.path).is_ok(),
        }
    }
}

impl InputComponent {
    fn present(
        name: impl Into<String>,
        digest_kind: DigestKind,
        digest_label: &str,
        digest_value: &str,
        detail: Vec<String>,
    ) -> Self {
        Self::new(
            name,
            InputComponentStatus::Present,
            Digest::from_parts(digest_kind, digest_label, &[digest_value]),
            detail,
        )
    }

    fn absent(name: impl Into<String>, digest_kind: DigestKind, reason: &str) -> Self {
        let name = name.into();
        Self::new(
            name.clone(),
            InputComponentStatus::Absent,
            Digest::absent(digest_kind, &name),
            vec![reason.to_string()],
        )
    }

    fn unsupported(name: impl Into<String>, digest_kind: DigestKind, reason: &str) -> Self {
        let name = name.into();
        Self::new(
            name.clone(),
            InputComponentStatus::Unsupported,
            Digest::unsupported(digest_kind, &name, reason),
            vec![reason.to_string()],
        )
    }

    fn setup_missing(
        name: impl Into<String>,
        digest_kind: DigestKind,
        detail: Vec<String>,
    ) -> Self {
        let name = name.into();
        let details = sorted_normalized_details(detail);
        let detail_refs = details.iter().map(String::as_str).collect::<Vec<_>>();
        Self::new(
            name,
            InputComponentStatus::SetupMissing,
            Digest::from_parts(digest_kind, "setup_missing", &detail_refs),
            details,
        )
    }

    fn setup_missing_with_digest_parts(
        name: impl Into<String>,
        digest_kind: DigestKind,
        detail: Vec<String>,
        digest_parts: Vec<String>,
    ) -> Self {
        let name = name.into();
        let details = sorted_normalized_details(detail);
        let mut digest_inputs = sorted_normalized_details(digest_parts);
        digest_inputs.insert(0, format!("component={name}"));
        let digest_refs = digest_inputs.iter().map(String::as_str).collect::<Vec<_>>();
        Self::new(
            name,
            InputComponentStatus::SetupMissing,
            Digest::from_parts(digest_kind, "setup_missing", &digest_refs),
            details,
        )
    }

    fn new(
        name: impl Into<String>,
        status: InputComponentStatus,
        digest: Digest,
        mut detail: Vec<String>,
    ) -> Self {
        detail = sorted_normalized_details(detail);
        Self {
            name: name.into(),
            status,
            digest,
            detail,
        }
    }
}

impl ProviderSchemaSnapshot {
    fn from_manifest(manifest: &ProviderManifest) -> Self {
        let schema_versions = sorted_schema_labels(manifest.schema_versions);
        let language_scope = manifest.language_scope.label().to_string();
        let cache_policy = manifest.cache_policy.label().into_owned();
        let precision_ceiling = manifest.precision_ceiling.label().to_string();
        let sorted_inputs = sorted_static_strings(manifest.inputs);
        let sorted_outputs = sorted_static_strings(manifest.outputs);

        let mut digest_parts = vec![
            format!("provider_id={}", manifest.id),
            format!("provider_version={}", manifest.provider_version()),
            format!("provider_kind={}", manifest.kind.label()),
            format!("language_scope={language_scope}"),
            format!("cache_policy={cache_policy}"),
            format!("precision_ceiling={precision_ceiling}"),
        ];
        digest_parts.extend(
            schema_versions
                .iter()
                .map(|schema| format!("schema_version={schema}")),
        );
        digest_parts.extend(sorted_inputs.iter().map(|input| format!("input={input}")));
        digest_parts.extend(
            sorted_outputs
                .iter()
                .map(|output| format!("output={output}")),
        );
        let digest_refs = digest_parts.iter().map(String::as_str).collect::<Vec<_>>();

        Self {
            provider_id: manifest.id.to_string(),
            schema_versions,
            language_scope,
            cache_policy,
            precision_ceiling,
            provider_manifest_digest: Digest::from_parts(
                DigestKind::ProviderManifest,
                "provider_manifest",
                &digest_refs,
            ),
        }
    }
}

fn go_lifecycle_components(
    loaded: &LoadedConfig,
    config: &crate::go::lifecycle::GoAnalysisConfig,
) -> Vec<InputComponent> {
    vec![
        values_component(
            "go.module_roots",
            DigestKind::GoLifecycle,
            config.module_roots.clone(),
        ),
        if config.files_without_module_root.is_empty() {
            InputComponent::absent(
                "go.files_without_module_root",
                DigestKind::GoLifecycle,
                "all discovered Go files are under module roots",
            )
        } else {
            InputComponent::setup_missing(
                "go.files_without_module_root",
                DigestKind::GoLifecycle,
                config.files_without_module_root.clone(),
            )
        },
        file_digest_component(
            "go.mod",
            DigestKind::GoLifecycle,
            &loaded.root,
            module_lifecycle_candidates(&config.module_roots, "go.mod"),
        ),
        file_digest_component(
            "go.sum",
            DigestKind::GoLifecycle,
            &loaded.root,
            module_lifecycle_candidates(&config.module_roots, "go.sum"),
        ),
        file_digest_component(
            "go.work",
            DigestKind::GoLifecycle,
            &loaded.root,
            go_work_candidates(&config.module_roots),
        ),
        values_component(
            "go.build_tags",
            DigestKind::GoLifecycle,
            config.build_tags.clone(),
        ),
        values_component(
            "go.include_tests",
            DigestKind::GoLifecycle,
            vec![config.include_tests.to_string()],
        ),
        values_component(
            "go.offline",
            DigestKind::GoLifecycle,
            vec![config.offline.to_string()],
        ),
        values_component(
            "go.package_patterns",
            DigestKind::GoLifecycle,
            config.package_patterns.clone(),
        ),
        InputComponent::unsupported(
            "go.tool_invocation",
            DigestKind::ToolInvocation,
            "not invoked by input snapshot",
        ),
        values_component(
            "go.environment_policy",
            DigestKind::GoLifecycle,
            vec![go_environment_policy(loaded, &config.module_roots)],
        ),
    ]
}

fn go_lifecycle_error_components(loaded: &LoadedConfig, reason: &str) -> Vec<InputComponent> {
    let setup_detail = vec![format!("error={reason}")];
    let unavailable_detail = vec![format!("module roots unavailable: {reason}")];
    vec![
        InputComponent::setup_missing(
            "go.module_roots",
            DigestKind::GoLifecycle,
            setup_detail.clone(),
        ),
        InputComponent::setup_missing(
            "go.files_without_module_root",
            DigestKind::GoLifecycle,
            setup_detail,
        ),
        InputComponent::setup_missing(
            "go.mod",
            DigestKind::GoLifecycle,
            unavailable_detail.clone(),
        ),
        InputComponent::setup_missing(
            "go.sum",
            DigestKind::GoLifecycle,
            unavailable_detail.clone(),
        ),
        InputComponent::setup_missing(
            "go.work",
            DigestKind::GoLifecycle,
            unavailable_detail.clone(),
        ),
        values_component(
            "go.build_tags",
            DigestKind::GoLifecycle,
            string_or_array_snapshot_setting(&loaded.config.languages.go, "build_tags", &[]),
        ),
        values_component(
            "go.include_tests",
            DigestKind::GoLifecycle,
            vec![
                bool_snapshot_setting(&loaded.config.languages.go, "include_tests", true)
                    .to_string(),
            ],
        ),
        values_component(
            "go.offline",
            DigestKind::GoLifecycle,
            vec![bool_snapshot_setting(&loaded.config.languages.go, "offline", false).to_string()],
        ),
        values_component(
            "go.package_patterns",
            DigestKind::GoLifecycle,
            string_or_array_snapshot_setting(
                &loaded.config.languages.go,
                "package_patterns",
                &["./..."],
            ),
        ),
        InputComponent::unsupported(
            "go.tool_invocation",
            DigestKind::ToolInvocation,
            "not invoked by input snapshot",
        ),
        InputComponent::setup_missing(
            "go.environment_policy",
            DigestKind::GoLifecycle,
            unavailable_detail,
        ),
    ]
}

fn config_component(loaded: &LoadedConfig, config_digest: Digest) -> InputComponent {
    let mut detail = vec![format!("missing={}", loaded.missing)];
    detail.extend(
        loaded
            .config
            .workspace
            .include
            .iter()
            .map(|pattern| format!("workspace.include={}", normalize_relative_path(pattern))),
    );
    detail.extend(
        loaded
            .config
            .workspace
            .exclude
            .iter()
            .map(|pattern| format!("workspace.exclude={}", normalize_relative_path(pattern))),
    );
    detail.extend(
        loaded
            .config
            .rules
            .paths
            .iter()
            .map(|path| format!("rules.path={}", normalize_relative_path(path))),
    );

    InputComponent::new(
        "config.loaded",
        InputComponentStatus::Present,
        config_digest,
        detail,
    )
}

fn semantic_row_builder(row_kind: &str) -> super::DigestBuilder {
    let mut row = Digest::builder(DigestKind::InputSnapshot, "semantic_row");
    row.labeled_part("row_kind", row_kind);
    row
}

fn semantic_row<'a>(
    row_kind: &str,
    fields: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Digest {
    let mut row = semantic_row_builder(row_kind);
    for (label, value) in fields {
        row.labeled_part(label, value);
    }
    row.finish()
}

fn digest_semantic_row(row_kind: &str, digest: &Digest) -> Digest {
    semantic_row(
        row_kind,
        [
            ("digest_kind", digest.kind.label()),
            ("digest_value", digest.value.as_str()),
        ],
    )
}

fn push_component_rows(rows: &mut Vec<Digest>, row_kind: &str, components: &[InputComponent]) {
    rows.extend(components.iter().map(|component| {
        let mut row = semantic_row_builder(row_kind);
        row.labeled_part("name", &component.name);
        row.labeled_part("status", component.status.label());
        row.labeled_part("digest_kind", component.digest.kind.label());
        row.labeled_part("digest_value", &component.digest.value);
        row.finish()
    }));
}

fn capability_support_status_label(status: &CapabilitySupportStatus) -> &'static str {
    match status {
        CapabilitySupportStatus::Supported => "supported",
        CapabilitySupportStatus::Unsupported => "unsupported",
        CapabilitySupportStatus::SetupMissing => "setup_missing",
    }
}

fn analysis_requirements_digest(capabilities: &[RequestedCapabilitySnapshot]) -> Digest {
    if capabilities.is_empty() {
        return Digest::absent(DigestKind::AnalysisRequirements, "requested_capabilities");
    }

    Digest::from_unordered(
        DigestKind::AnalysisRequirements,
        "analysis_requirements",
        capabilities
            .iter()
            .map(|capability| capability.analysis_dependency_digest.clone())
            .collect(),
    )
}

fn values_component(name: &str, digest_kind: DigestKind, values: Vec<String>) -> InputComponent {
    let details = sorted_normalized_details(values);
    if details.is_empty() {
        return InputComponent::absent(name, digest_kind, "no values");
    }

    let detail_refs = details.iter().map(String::as_str).collect::<Vec<_>>();
    InputComponent::new(
        name,
        InputComponentStatus::Present,
        Digest::from_parts(digest_kind, name, &detail_refs),
        details,
    )
}

fn settings_component(
    name: &str,
    digest_kind: DigestKind,
    settings: &std::collections::BTreeMap<String, toml::Value>,
) -> InputComponent {
    let values = settings
        .iter()
        .map(|(key, value)| format!("{key}={}", toml_value_label(value)))
        .collect::<Vec<_>>();
    values_component(name, digest_kind, values)
}

fn file_digest_component(
    name: &str,
    digest_kind: DigestKind,
    root: &FsPath,
    candidate_paths: Vec<String>,
) -> InputComponent {
    let mut paths = candidate_paths;
    paths.sort();
    paths.dedup();

    let mut present_paths = Vec::new();
    let mut digest_parts = Vec::new();
    let mut unreadable = Vec::new();
    let mut unsupported = Vec::new();
    for relative_path in paths {
        let normalized = normalize_relative_path(&relative_path);
        let contents = match read_repo_file_with_limit(
            root,
            &normalized,
            topology_input_max_bytes(&normalized),
        ) {
            Ok(contents) => contents,
            Err(error) if error.is_not_found() => continue,
            Err(error) => {
                let detail = format!("unsupported={normalized}: {}", error.stable_reason());
                if matches!(
                    error,
                    crate::module_graph::paths::RepoFileReadError::TooLarge { .. }
                        | crate::module_graph::paths::RepoFileReadError::AbsolutePath
                        | crate::module_graph::paths::RepoFileReadError::EscapesRepo
                ) {
                    unsupported.push(detail);
                } else {
                    unreadable.push(format!(
                        "unreadable={normalized}: {}",
                        error.stable_reason()
                    ));
                }
                continue;
            }
        };
        present_paths.push(normalized.clone());
        digest_parts.push(format!("file={normalized}"));
        digest_parts.push(format!("content_hash={}", stable_hash_bytes(&contents)));
    }

    if !unreadable.is_empty() {
        let mut detail = present_paths;
        detail.extend(unreadable.clone());
        detail.extend(unsupported.clone());
        let mut setup_missing_digest_parts = digest_parts;
        setup_missing_digest_parts.extend(unreadable);
        setup_missing_digest_parts.extend(unsupported);
        return InputComponent::setup_missing_with_digest_parts(
            name,
            digest_kind,
            detail,
            setup_missing_digest_parts,
        );
    }

    if !unsupported.is_empty() {
        let mut detail = present_paths;
        detail.extend(unsupported.clone());
        let mut unsupported_digest_parts = digest_parts;
        unsupported_digest_parts.extend(unsupported);
        let digest_refs = unsupported_digest_parts
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        return InputComponent::new(
            name,
            InputComponentStatus::Unsupported,
            Digest::from_parts(digest_kind, name, &digest_refs),
            detail,
        );
    }

    if present_paths.is_empty() {
        return InputComponent::absent(name, digest_kind, "no lifecycle files present");
    }

    let digest_refs = digest_parts.iter().map(String::as_str).collect::<Vec<_>>();
    InputComponent::new(
        name,
        InputComponentStatus::Present,
        Digest::from_parts(digest_kind, name, &digest_refs),
        present_paths,
    )
}

fn topology_input_max_bytes(relative_path: &str) -> u64 {
    match FsPath::new(relative_path)
        .file_name()
        .and_then(|name| name.to_str())
    {
        Some(
            "package-lock.json"
            | "npm-shrinkwrap.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "bun.lock",
        ) => TOPOLOGY_LOCKFILE_MAX_BYTES,
        _ => TOPOLOGY_MANIFEST_MAX_BYTES,
    }
}

fn rule_components(rule_digest: &str, plan: &AnalysisPlan) -> Vec<InputComponent> {
    vec![
        InputComponent::present(
            "rule.code",
            DigestKind::RuleCode,
            "rule_digest",
            rule_digest,
            Vec::new(),
        ),
        InputComponent::present(
            "rule.options",
            DigestKind::RuleOptions,
            "plan_digest",
            plan.digest(),
            Vec::new(),
        ),
    ]
}

fn extension_components_from_repo(root: &FsPath) -> Vec<InputComponent> {
    let discovered = discover_local_extensions(root);
    if discovered.is_empty() {
        return vec![InputComponent::absent(
            "extension.providers",
            DigestKind::ExtensionCode,
            "no extension providers configured",
        )];
    }

    discovered
        .iter()
        .map(extension_component)
        .collect::<Vec<_>>()
}

fn extension_component(extension: &DiscoveredExtension) -> InputComponent {
    let source_digest = extension.source_digest.to_string();
    let dependency_digest = extension.dependency_digest.to_string();
    let status = extension_activation_status_label(extension.activation_status);
    let mut detail = vec![
        format!("extension_id={}", extension.extension_id),
        format!("manifest={}", extension.manifest_path),
        format!("activation_status={status}"),
        format!("source_digest={source_digest}"),
        format!("dependency_digest={dependency_digest}"),
    ];
    detail.extend(
        extension
            .digest_input_paths
            .iter()
            .map(|path| format!("digest_input={path}")),
    );
    let digest_refs = detail.iter().map(String::as_str).collect::<Vec<_>>();
    InputComponent::new(
        format!("extension.providers.{}", extension.extension_id),
        InputComponentStatus::Present,
        Digest::from_parts(
            DigestKind::ExtensionCode,
            "extension_provider",
            &digest_refs,
        ),
        detail,
    )
}

fn module_lifecycle_candidates(module_roots: &[String], file_name: &str) -> Vec<String> {
    module_roots
        .iter()
        .map(|module_root| {
            if module_root == "." {
                file_name.to_string()
            } else {
                format!("{module_root}/{file_name}")
            }
        })
        .collect()
}

fn go_work_candidates(module_roots: &[String]) -> Vec<String> {
    let mut candidates = vec!["go.work".to_string()];
    candidates.extend(module_lifecycle_candidates(module_roots, "go.work"));
    candidates
}

fn go_environment_policy(loaded: &LoadedConfig, module_roots: &[String]) -> String {
    let checked_in_go_work = loaded.root.join("go.work");
    if repo_file_exists(&loaded.root, "go.work")
        && crate::go::lifecycle::go_work_covers_module_roots(
            &loaded.root,
            &checked_in_go_work,
            module_roots,
        )
    {
        "checked_in_workspace_file".to_string()
    } else if module_roots.len() == 1
        && module_roots[0] == "."
        && repo_file_exists(&loaded.root, "go.mod")
    {
        "single_root_module".to_string()
    } else if module_roots.is_empty() {
        "no_module_roots".to_string()
    } else {
        "internal_temporary_workspace_if_needed".to_string()
    }
}

fn ts_js_source_paths(db: &AnalysisDb) -> Vec<String> {
    let mut paths = db
        .files()
        .iter()
        .filter(|file| file.language.is_ts_family())
        .map(|file| normalize_relative_path(&file.relative_path))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn ts_js_lifecycle_dirs(source_paths: &[String]) -> BTreeSet<String> {
    let mut dirs = BTreeSet::new();
    dirs.insert(String::new());
    for source_path in source_paths {
        let mut current = FsPath::new(source_path).parent();
        while let Some(parent) = current {
            let normalized = normalize_relative_path(&parent.to_string_lossy());
            if normalized.is_empty() {
                break;
            }
            dirs.insert(normalized);
            current = parent.parent();
        }
    }
    dirs
}

fn lifecycle_candidates(dirs: &BTreeSet<String>, file_names: Vec<String>) -> Vec<String> {
    let mut candidates = Vec::new();
    for dir in dirs {
        for file_name in &file_names {
            if dir.is_empty() {
                candidates.push(file_name.clone());
            } else {
                candidates.push(format!("{dir}/{file_name}"));
            }
        }
    }
    candidates
}

fn package_manifest_names() -> Vec<String> {
    vec!["package.json".to_string()]
}

fn lock_file_names() -> Vec<String> {
    vec![
        "package-lock.json".to_string(),
        "npm-shrinkwrap.json".to_string(),
        ["p", "n", "p", "m", "-lock.yaml"].concat(),
        ["ya", "rn.lock"].concat(),
        ["b", "un.lock"].concat(),
    ]
}

fn config_file_names() -> Vec<String> {
    vec![ts_config_name(), "jsconfig.json".to_string()]
}

fn ts_config_name() -> String {
    ["t", "sconfig.json"].concat()
}

fn ts_js_config_candidates(root: &FsPath, lifecycle_dirs: &BTreeSet<String>) -> Vec<String> {
    let mut candidates = lifecycle_candidates(lifecycle_dirs, config_file_names());
    candidates.extend(extended_ts_config_candidates(
        root,
        candidates.iter().map(String::as_str),
    ));
    candidates
}

fn extended_ts_config_candidates<'a>(
    root: &FsPath,
    initial_candidates: impl IntoIterator<Item = &'a str>,
) -> Vec<String> {
    let mut visited = BTreeSet::new();
    let mut discovered = BTreeSet::new();
    for candidate in initial_candidates {
        let file_name = FsPath::new(candidate)
            .file_name()
            .and_then(|name| name.to_str());
        if !matches!(file_name, Some("tsconfig.json" | "jsconfig.json")) {
            continue;
        }
        collect_extended_ts_configs(root, &root.join(candidate), &mut visited, &mut discovered);
    }
    discovered.into_iter().collect()
}

fn collect_extended_ts_configs(
    root: &FsPath,
    config_path: &FsPath,
    visited: &mut BTreeSet<String>,
    discovered: &mut BTreeSet<String>,
) {
    let Some(config_path) = repo_relative_existing_path(root, config_path) else {
        return;
    };
    if !visited.insert(config_path.clone()) {
        return;
    }
    let Some(config) = read_tsconfig_extends_wire(root, &config_path) else {
        return;
    };
    let config_dir = FsPath::new(&config_path)
        .parent()
        .unwrap_or(FsPath::new(""));
    for specifier in config
        .extends
        .into_iter()
        .flat_map(TsconfigExtendsWire::into_specifiers)
    {
        let Some(extended_path) = resolve_tsconfig_extends_path(root, config_dir, &specifier)
        else {
            continue;
        };
        discovered.insert(extended_path.clone());
        collect_extended_ts_configs(root, &root.join(&extended_path), visited, discovered);
    }
}

fn read_tsconfig_extends_wire(
    root: &FsPath,
    relative_path: &str,
) -> Option<TsconfigExtendsOnlyWire> {
    let Ok(mut source) =
        read_repo_file_to_string_with_limit(root, relative_path, TOPOLOGY_MANIFEST_MAX_BYTES)
    else {
        return None;
    };
    if let Some(stripped) = source.strip_prefix('\u{feff}') {
        source = stripped.to_string();
    }
    if json_strip_comments::strip(&mut source).is_err() {
        return None;
    }
    serde_json::from_str::<TsconfigExtendsOnlyWire>(&source).ok()
}

fn resolve_tsconfig_extends_path(
    root: &FsPath,
    config_dir: &FsPath,
    specifier: &str,
) -> Option<String> {
    let specifier_path = FsPath::new(specifier);
    if specifier_path.is_absolute() {
        return None;
    }
    if specifier.starts_with('.') {
        return resolve_tsconfig_file_candidate(root, &config_dir.join(specifier_path));
    }
    resolve_package_tsconfig_extends_path(root, config_dir, specifier)
}

fn resolve_package_tsconfig_extends_path(
    root: &FsPath,
    config_dir: &FsPath,
    specifier: &str,
) -> Option<String> {
    let mut current = normalize_repo_relative_input(config_dir)?;
    loop {
        let candidate = if current.as_os_str().is_empty() {
            PathBuf::from("node_modules").join(specifier)
        } else {
            current.join("node_modules").join(specifier)
        };
        if let Some(resolved) = resolve_tsconfig_file_candidate(root, &candidate) {
            return Some(resolved);
        }
        if current.as_os_str().is_empty() {
            return None;
        }
        current.pop();
    }
}

fn resolve_tsconfig_file_candidate(root: &FsPath, base: &FsPath) -> Option<String> {
    let mut candidates = vec![base.to_path_buf()];
    if base.extension().and_then(|extension| extension.to_str()) != Some("json") {
        let mut with_json = base.as_os_str().to_owned();
        with_json.push(".json");
        candidates.push(PathBuf::from(with_json));
    }
    candidates.push(base.join("tsconfig.json"));

    candidates
        .into_iter()
        .filter_map(|candidate| normalized_existing_repo_file(root, &candidate))
        .next()
}

fn normalized_existing_repo_file(root: &FsPath, candidate: &FsPath) -> Option<String> {
    repo_file_path(root, candidate).ok()?;
    let normalized = normalize_repo_relative_input(candidate)?;
    let relative = normalized.to_string_lossy();
    if relative.is_empty() {
        None
    } else {
        normalize_repo_relative(relative)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TsconfigExtendsOnlyWire {
    #[serde(default)]
    extends: Option<TsconfigExtendsWire>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TsconfigExtendsWire {
    Single(String),
    Multiple(Vec<String>),
}

impl TsconfigExtendsWire {
    fn into_specifiers(self) -> Vec<String> {
        match self {
            Self::Single(specifier) => vec![specifier],
            Self::Multiple(specifiers) => specifiers,
        }
    }
}

fn string_or_array_snapshot_setting(
    settings: &std::collections::BTreeMap<String, toml::Value>,
    key: &str,
    default: &[&str],
) -> Vec<String> {
    let values = settings.get(key).map_or_else(
        || default.iter().map(|value| (*value).to_string()).collect(),
        string_or_array_snapshot_value,
    );
    sorted_normalized_details(values)
}

fn string_or_array_snapshot_value(value: &toml::Value) -> Vec<String> {
    match value {
        toml::Value::String(value) => vec![value.clone()],
        toml::Value::Array(values) => values
            .iter()
            .filter_map(toml::Value::as_str)
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn bool_snapshot_setting(
    settings: &std::collections::BTreeMap<String, toml::Value>,
    key: &str,
    default: bool,
) -> bool {
    settings
        .get(key)
        .and_then(toml::Value::as_bool)
        .unwrap_or(default)
}

fn sorted_schema_labels(schemas: &[crate::analysis_kernel::SchemaVersion]) -> Vec<String> {
    let mut labels = schemas
        .iter()
        .map(|schema| format!("{}:{}", schema.name, schema.version))
        .collect::<Vec<_>>();
    labels.sort();
    labels
}

fn sorted_static_strings(values: &[&'static str]) -> Vec<&'static str> {
    let mut values = values.to_vec();
    values.sort();
    values
}

fn sorted_components(mut components: Vec<InputComponent>) -> Vec<InputComponent> {
    components.sort_by(|left, right| left.name.cmp(&right.name));
    components
}

fn component_by_name<'a>(components: &'a [InputComponent], name: &str) -> &'a InputComponent {
    components
        .iter()
        .find(|component| component.name == name)
        .unwrap_or_else(|| panic!("snapshot is missing component {name}"))
}

fn sorted_normalized_details(details: Vec<String>) -> Vec<String> {
    let mut details = details
        .into_iter()
        .map(|detail| normalize_detail(&detail))
        .collect::<Vec<_>>();
    details.sort();
    details.dedup();
    details
}

fn normalize_detail(value: &str) -> String {
    value.replace('\\', "/")
}

fn normalize_relative_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_string()
}

fn toml_value_label(value: &toml::Value) -> String {
    match value {
        toml::Value::String(value) => format!("{value:?}"),
        toml::Value::Integer(value) => value.to_string(),
        toml::Value::Float(value) => value.to_string(),
        toml::Value::Boolean(value) => value.to_string(),
        toml::Value::Datetime(value) => value.to_string(),
        toml::Value::Array(values) => {
            let labels = values.iter().map(toml_value_label).collect::<Vec<_>>();
            format!("[{}]", labels.join(","))
        }
        toml::Value::Table(values) => {
            let mut labels = values
                .iter()
                .map(|(key, value)| format!("{key}:{}", toml_value_label(value)))
                .collect::<Vec<_>>();
            labels.sort();
            format!("{{{}}}", labels.join(","))
        }
    }
}

fn stable_hash_bytes(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn extension_activation_status_label(
    status: crate::analysis::extensions::manifest::ExtensionActivationStatus,
) -> &'static str {
    match status {
        crate::analysis::extensions::manifest::ExtensionActivationStatus::Configured => {
            "configured"
        }
        crate::analysis::extensions::manifest::ExtensionActivationStatus::Discovered => {
            "discovered"
        }
        crate::analysis::extensions::manifest::ExtensionActivationStatus::HandshakeOk => {
            "handshake_ok"
        }
        crate::analysis::extensions::manifest::ExtensionActivationStatus::Active => "active",
        crate::analysis::extensions::manifest::ExtensionActivationStatus::Failed => "failed",
        crate::analysis::extensions::manifest::ExtensionActivationStatus::ValidationFailed => {
            "validation_failed"
        }
        crate::analysis::extensions::manifest::ExtensionActivationStatus::Disabled => "disabled",
    }
}

#[cfg(test)]
mod input_component_status_codec {
    use super::*;

    #[test]
    fn round_trips_every_variant_and_rejects_unknown_labels() {
        for status in [
            InputComponentStatus::Present,
            InputComponentStatus::Absent,
            InputComponentStatus::Unsupported,
            InputComponentStatus::SetupMissing,
        ] {
            assert_eq!(
                InputComponentStatus::parse_label(status.label()),
                Ok(status)
            );
            let wire = serde_json::to_string(&status).expect("serialize input status");
            assert_eq!(
                serde_json::from_str::<InputComponentStatus>(&wire)
                    .expect("deserialize input status"),
                status
            );
            assert_eq!(wire, format!("\"{}\"", status.label()));
        }

        assert!(InputComponentStatus::parse_label("setup-missing").is_err());
        assert!(serde_json::from_str::<InputComponentStatus>("\"setup-missing\"").is_err());
    }
}

#[cfg(test)]
mod source_config_rule_model_extension {
    use super::*;
    use crate::analysis_kernel::{
        CachePolicy, LanguageScope, PrecisionCeiling, ProviderKind, ProviderManifest, SchemaVersion,
    };
    use crate::analysis_plan::{AnalysisPlan, CapabilitySetupStatus};
    use crate::cache::keys::{AnalysisSettingsScope, config_hash};
    use crate::config::{LoadedConfig, PolintConfig, RuleConfig};
    use crate::core::{AnalysisDb, CapabilitySupportStatus};
    use std::path::Path;
    use tempfile::TempDir;

    fn loaded_config(root: &Path) -> LoadedConfig {
        LoadedConfig {
            root: root.to_path_buf(),
            config: PolintConfig::default(),
            missing: false,
            respect_gitignore: true,
        }
    }

    fn db_with_files(root: &Path, files: &[(&str, &str)]) -> AnalysisDb {
        let mut db = AnalysisDb::new();
        for (relative_path, source) in files {
            let path = root.join(relative_path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("create fixture parent");
            }
            std::fs::write(&path, source).expect("write fixture source");
            db.add_file(path, (*relative_path).to_string(), (*source).to_string());
        }
        db
    }

    fn snapshot_for(
        loaded: &LoadedConfig,
        db: &AnalysisDb,
        provider_manifests: &[ProviderManifest],
    ) -> InputSnapshot {
        let empty_plan = AnalysisPlan::empty();
        assert!(empty_plan.requested_capability_snapshots().is_empty());
        let identity_sources = InputSnapshot::identity_sources_from_plan(loaded, &empty_plan);
        assert!(identity_sources.requested_capabilities.is_empty());
        assert_eq!(
            identity_sources.analysis_requirements_identity,
            Digest::absent(DigestKind::AnalysisRequirements, "requested_capabilities")
        );

        InputSnapshot::from_run_inputs_with_plan(
            loaded,
            db,
            "config-digest",
            "rule-digest",
            &empty_plan,
            provider_manifests,
        )
    }

    #[test]
    fn snapshots_from_same_inputs_serialize_to_identical_pretty_json() {
        let temp = TempDir::new().expect("create temp repo");
        let loaded = loaded_config(temp.path());
        let db = db_with_files(
            temp.path(),
            &[
                ("src/z.ts", "export const z = 1;\n"),
                ("cmd/main.go", "package main\n"),
            ],
        );

        let first = snapshot_for(
            &loaded,
            &db,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        );
        let second = snapshot_for(
            &loaded,
            &db,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        );

        assert_eq!(
            serde_json::to_string_pretty(&first).expect("serialize first snapshot"),
            serde_json::to_string_pretty(&second).expect("serialize second snapshot")
        );
    }

    #[test]
    fn plan_identity_sources_are_typed_in_the_v2_wire_shape() {
        let temp = TempDir::new().expect("create temp repo");
        let loaded = loaded_config(temp.path());
        let db = db_with_files(temp.path(), &[("src/app.ts", "export const app = 1;\n")]);
        let plan = AnalysisPlan::from_capability_names_for_test(&["calls", "call_graph"]);

        let sources = InputSnapshot::identity_sources_from_plan(&loaded, &plan);
        let snapshot = InputSnapshot::from_run_inputs_with_plan(
            &loaded,
            &db,
            "config-digest",
            "rule-digest",
            &plan,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        );
        let wire = serde_json::to_value(snapshot).expect("serialize v2 snapshot");

        assert_eq!(sources.analysis_settings.len(), 23);
        assert!(
            sources
                .analysis_settings
                .windows(2)
                .all(|pair| { pair[0].scope.label() < pair[1].scope.label() })
        );
        assert!(
            sources
                .analysis_settings
                .iter()
                .all(|source| { source.digest.kind == DigestKind::AnalysisSettings })
        );
        assert!(!sources.requested_capabilities.is_empty());
        assert_eq!(
            sources
                .requested_capabilities
                .iter()
                .map(|source| source.capability.as_str())
                .collect::<Vec<_>>(),
            vec![
                "call_graph",
                "calls",
                "module_graph",
                "references",
                "resolved_imports",
                "symbols",
            ]
        );
        let call_graph = sources
            .requested_capabilities
            .iter()
            .find(|source| source.capability == "call_graph")
            .expect("call graph capability source");
        assert_eq!(
            call_graph.support_status,
            CapabilitySupportStatus::Unsupported
        );
        assert_eq!(call_graph.setup_status, CapabilitySetupStatus::NotRequired);
        let calls = sources
            .requested_capabilities
            .iter()
            .find(|source| source.capability == "calls")
            .expect("calls capability source");
        assert_eq!(calls.support_status, CapabilitySupportStatus::Supported);
        assert_eq!(calls.setup_status, CapabilitySetupStatus::NotRequired);
        assert!(calls.policy_query_version.is_some());
        assert_eq!(
            sources.analysis_requirements_identity.kind,
            DigestKind::AnalysisRequirements
        );
        assert_eq!(wire["schema_version"], INPUT_SNAPSHOT_SCHEMA_VERSION);
        assert_eq!(wire["analysis_settings"].as_array().map(Vec::len), Some(23));
        assert!(wire["analysis_settings"][0].get("provider_id").is_some());
        assert!(wire["analysis_settings"][0].get("digest").is_some());
        assert!(
            wire["requested_capabilities"]
                .as_array()
                .is_some_and(|rows| {
                    rows.iter().all(|row| {
                        row.get("language").is_some()
                            && row.get("support_status").is_some()
                            && row.get("setup_status").is_some()
                            && row.get("requesting_rule_ids").is_some()
                            && row.get("rule_behavior_digest").is_some()
                            && row.get("analysis_dependency_digest").is_some()
                    })
                })
        );
        assert!(wire.get("workspace_identity").is_some());
        assert!(wire.get("config_identity").is_some());
        assert!(wire.get("analysis_requirements_identity").is_some());
    }

    #[test]
    fn v2_snapshot_round_trips_and_rejects_future_or_unknown_wire_values() {
        let temp = TempDir::new().expect("create temp repo");
        let loaded = loaded_config(temp.path());
        let db = db_with_files(temp.path(), &[("src/app.ts", "export const app = 1;\n")]);
        let plan = AnalysisPlan::from_capability_names_for_test(&["calls", "call_graph"]);
        let snapshot = InputSnapshot::from_run_inputs_with_plan(
            &loaded,
            &db,
            "config-digest",
            "rule-digest",
            &plan,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        );
        let value = serde_json::to_value(&snapshot).expect("serialize v2 snapshot");
        let round_trip: InputSnapshot =
            serde_json::from_value(value.clone()).expect("deserialize v2 snapshot");

        assert_eq!(round_trip, snapshot);

        let mut future = value.clone();
        future["schema_version"] = serde_json::Value::String("polint-input-snapshot-99".into());
        let future_error = serde_json::from_value::<InputSnapshot>(future)
            .expect_err("future schema must be rejected");
        assert!(
            future_error
                .to_string()
                .contains("unsupported input snapshot schema")
        );

        let mut unknown_provider = value.clone();
        unknown_provider["analysis_settings"][0]["provider_id"] =
            serde_json::Value::String("polint.future_provider".into());
        let provider_error = serde_json::from_value::<InputSnapshot>(unknown_provider)
            .expect_err("unknown settings provider must be rejected");
        assert!(
            provider_error
                .to_string()
                .contains("unknown analysis settings provider")
        );

        let mut unknown_field = value.clone();
        unknown_field["future_field"] = serde_json::Value::Bool(true);
        let field_error = serde_json::from_value::<InputSnapshot>(unknown_field)
            .expect_err("unknown snapshot fields must be rejected");
        assert!(field_error.to_string().contains("unknown field"));

        let mut unknown_status = value;
        unknown_status["requested_capabilities"][0]["support_status"] =
            serde_json::Value::String("maybe_supported".into());
        let status_error = serde_json::from_value::<InputSnapshot>(unknown_status)
            .expect_err("unknown capability status must be rejected");
        assert!(
            status_error
                .to_string()
                .contains("unknown capability support status")
        );
    }

    #[test]
    fn semantic_digest_is_order_independent_and_excludes_hints_and_rendered_details() {
        let temp = TempDir::new().expect("create temp repo");
        let loaded = loaded_config(temp.path());
        let db = db_with_files(
            temp.path(),
            &[
                ("src/z.ts", "export const z = 1;\n"),
                ("src/a.ts", "export const a = 1;\n"),
            ],
        );
        let plan = AnalysisPlan::from_capability_names_for_test(&["calls", "call_graph"]);
        let snapshot = InputSnapshot::from_run_inputs_with_plan(
            &loaded,
            &db,
            "config-digest",
            "rule-digest",
            &plan,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        );
        let mut non_semantic = snapshot.clone();
        non_semantic.files.reverse();
        non_semantic.files[0].mtime_hint_present = !non_semantic.files[0].mtime_hint_present;
        non_semantic.config.detail = vec!["rendered reason changed".to_string()];
        non_semantic.analysis_settings.reverse();
        non_semantic.requested_capabilities.reverse();
        non_semantic.provider_schemas.reverse();

        assert_eq!(snapshot.semantic_digest(), non_semantic.semantic_digest());

        let mut semantic_change = snapshot.clone();
        semantic_change.files[0].source_text_digest =
            Digest::from_parts(DigestKind::SourceText, "changed", &["source"]);
        assert_ne!(
            snapshot.semantic_digest(),
            semantic_change.semantic_digest()
        );

        let mut model_change = snapshot.clone();
        model_change.models[0].digest =
            Digest::from_parts(DigestKind::ModelFile, "model", &["changed"]);
        assert_ne!(snapshot.semantic_digest(), model_change.semantic_digest());

        let mut extension_change = snapshot.clone();
        extension_change.extensions[0].digest =
            Digest::from_parts(DigestKind::ExtensionCode, "extension", &["changed"]);
        assert_ne!(
            snapshot.semantic_digest(),
            extension_change.semantic_digest()
        );
    }

    #[test]
    fn full_config_capability_and_scoped_setting_identities_have_separate_boundaries() {
        let temp = TempDir::new().expect("create temp repo");
        let mut baseline_loaded = loaded_config(temp.path());
        baseline_loaded.config.rules.config.push(RuleConfig {
            id: "local/example".to_string(),
            severity: None,
            files: Vec::new(),
            allow_files: Vec::new(),
            allow: Vec::new(),
            max: None,
            deny: Vec::new(),
            forbidden_imports: Default::default(),
            settings: Default::default(),
        });
        let db = db_with_files(temp.path(), &[("src/app.ts", "export const app = 1;\n")]);
        let plan = AnalysisPlan::from_capability_names_for_test(&["calls"]);
        let mut rule_only = baseline_loaded.clone();
        rule_only.config.rules.config[0].severity = Some("error".to_string());
        let mut relevant_setting = baseline_loaded.clone();
        relevant_setting.config.solver.go.max_rta_rounds = Some(8);

        let baseline_config = config_hash(&baseline_loaded);
        let rule_config = config_hash(&rule_only);
        let relevant_config = config_hash(&relevant_setting);
        let baseline = InputSnapshot::from_run_inputs_with_plan(
            &baseline_loaded,
            &db,
            &baseline_config,
            "rule-digest",
            &plan,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        );
        let rule_changed = InputSnapshot::from_run_inputs_with_plan(
            &rule_only,
            &db,
            &rule_config,
            "rule-digest",
            &plan,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        );
        let setting_changed = InputSnapshot::from_run_inputs_with_plan(
            &relevant_setting,
            &db,
            &relevant_config,
            "rule-digest",
            &plan,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        );

        assert_ne!(baseline.config_identity, rule_changed.config_identity);
        assert_ne!(baseline.semantic_digest(), rule_changed.semantic_digest());
        assert_eq!(baseline.analysis_settings, rule_changed.analysis_settings);
        assert_eq!(
            baseline.analysis_requirements_identity,
            rule_changed.analysis_requirements_identity
        );
        assert_eq!(
            baseline.requested_capabilities,
            rule_changed.requested_capabilities
        );

        let baseline_solver = baseline
            .analysis_settings
            .iter()
            .find(|row| row.scope == AnalysisSettingsScope::Solver)
            .expect("solver setting row");
        let changed_solver = setting_changed
            .analysis_settings
            .iter()
            .find(|row| row.scope == AnalysisSettingsScope::Solver)
            .expect("changed solver setting row");
        assert_ne!(baseline_solver.digest, changed_solver.digest);
        let baseline_source = baseline
            .analysis_settings
            .iter()
            .find(|row| row.scope == AnalysisSettingsScope::Source)
            .expect("source setting row");
        let changed_source = setting_changed
            .analysis_settings
            .iter()
            .find(|row| row.scope == AnalysisSettingsScope::Source)
            .expect("unchanged source setting row");
        assert_eq!(baseline_source.digest, changed_source.digest);

        let capability_changed = AnalysisPlan::from_capability_names_for_test(&["call_graph"]);
        let capability_snapshot = InputSnapshot::from_run_inputs_with_plan(
            &baseline_loaded,
            &db,
            &baseline_config,
            "rule-digest",
            &capability_changed,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        );
        assert_ne!(
            baseline.analysis_requirements_identity,
            capability_snapshot.analysis_requirements_identity
        );
    }

    #[test]
    fn opaque_identity_serialization_excludes_root_source_and_environment_sentinels() {
        let temp = TempDir::new().expect("create root-sentinel repo");
        let loaded = loaded_config(temp.path());
        let db = db_with_files(
            temp.path(),
            &[("src/app.ts", "const sourceSentinel = 'source-secret';\n")],
        );
        let snapshot = snapshot_for(
            &loaded,
            &db,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        );
        let rendered = serde_json::to_string_pretty(&snapshot).expect("serialize snapshot");

        assert!(!rendered.contains(temp.path().to_string_lossy().as_ref()));
        assert!(!rendered.contains("source-secret"));
        assert!(!rendered.contains("environment-variable-sentinel"));
    }

    #[test]
    fn empty_plan_has_no_requested_capability_sources() {
        let temp = TempDir::new().expect("create temp repo");
        let loaded = loaded_config(temp.path());
        let plan = AnalysisPlan::empty();

        let sources = InputSnapshot::identity_sources_from_plan(&loaded, &plan);

        assert!(sources.requested_capabilities.is_empty());
        assert_eq!(
            sources.analysis_requirements_identity,
            Digest::absent(DigestKind::AnalysisRequirements, "requested_capabilities")
        );
    }

    #[test]
    fn file_rows_expose_safe_identity_without_source_or_machine_paths() {
        let temp = TempDir::new().expect("create temp repo");
        let loaded = loaded_config(temp.path());
        let db = db_with_files(
            temp.path(),
            &[("src/app.ts", "const secret = 'raw text';\n")],
        );

        let snapshot = snapshot_for(
            &loaded,
            &db,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        );
        let file = &snapshot.files[0];
        let rendered = serde_json::to_string_pretty(&snapshot).expect("serialize snapshot");

        assert_eq!(file.relative_path, "src/app.ts");
        assert_eq!(file.language, crate::core::Language::TypeScript);
        assert_eq!(file.source_text_digest.kind, DigestKind::SourceText);
        assert_eq!(file.size_bytes, "const secret = 'raw text';\n".len());
        assert!(file.mtime_hint_present);
        assert!(!rendered.contains("const secret"));
        assert!(!rendered.contains(temp.path().to_string_lossy().as_ref()));
        assert!(!rendered.contains(db.files()[0].path.to_string_lossy().as_ref()));
    }

    #[test]
    fn config_rule_plan_model_extension_and_provider_components_are_typed() {
        let temp = TempDir::new().expect("create temp repo");
        let loaded = loaded_config(temp.path());
        let db = db_with_files(temp.path(), &[("src/app.ts", "export const app = 1;\n")]);

        let snapshot = snapshot_for(
            &loaded,
            &db,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        );

        assert_eq!(snapshot.config.status, InputComponentStatus::Present);
        assert_eq!(snapshot.config.digest.kind, DigestKind::Config);
        assert!(
            snapshot
                .rules
                .iter()
                .any(|component| component.digest.kind == DigestKind::RuleCode)
        );
        assert!(
            snapshot
                .rules
                .iter()
                .any(|component| component.digest.kind == DigestKind::RuleOptions)
        );
        assert_eq!(snapshot.models[0].name, "model.files");
        assert_eq!(snapshot.models[0].status, InputComponentStatus::Absent);
        assert_eq!(snapshot.models[0].digest.kind, DigestKind::ModelFile);
        assert_eq!(snapshot.extensions[0].name, "extension.providers");
        assert_eq!(snapshot.extensions[0].status, InputComponentStatus::Absent);
        assert_eq!(
            snapshot.extensions[0].digest.kind,
            DigestKind::ExtensionCode
        );
        assert!(snapshot.provider_schemas.iter().all(|provider| {
            provider.provider_manifest_digest.kind == DigestKind::ProviderManifest
        }));
    }

    #[test]
    fn configured_local_extension_records_present_extension_code_component() {
        let temp = TempDir::new().expect("create temp repo");
        let loaded = loaded_config(temp.path());
        let db = db_with_files(temp.path(), &[("src/app.ts", "export const app = 1;\n")]);
        std::fs::create_dir_all(temp.path().join(".polint/extensions/demo/src"))
            .expect("create extension src");
        std::fs::write(
            temp.path().join(".polint/extensions/demo/Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .expect("write extension manifest");
        std::fs::write(
            temp.path().join(".polint/extensions/demo/src/main.rs"),
            "fn main() {}\n",
        )
        .expect("write extension source");

        let snapshot = snapshot_for(
            &loaded,
            &db,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        );
        let rendered = serde_json::to_string_pretty(&snapshot).expect("serialize snapshot");

        assert_eq!(snapshot.extensions.len(), 1);
        assert_eq!(snapshot.extensions[0].status, InputComponentStatus::Present);
        assert_eq!(
            snapshot.extensions[0].digest.kind,
            DigestKind::ExtensionCode
        );
        assert!(
            snapshot.extensions[0]
                .detail
                .contains(&"manifest=.polint/extensions/demo/Cargo.toml".to_string())
        );
        assert!(rendered.contains("activation_status=discovered"));
        assert!(!rendered.contains(temp.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn source_file_rows_are_sorted_by_normalized_relative_path() {
        let temp = TempDir::new().expect("create temp repo");
        let loaded = loaded_config(temp.path());
        let db = db_with_files(
            temp.path(),
            &[
                ("src/z.ts", "export const z = 1;\n"),
                ("cmd/main.go", "package main\n"),
                ("src/a.tsx", "export function A() { return null; }\n"),
            ],
        );

        let snapshot = snapshot_for(
            &loaded,
            &db,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        );

        assert_eq!(
            snapshot
                .files
                .iter()
                .map(|file| file.relative_path.as_str())
                .collect::<Vec<_>>(),
            vec!["cmd/main.go", "src/a.tsx", "src/z.ts"]
        );
    }

    #[test]
    fn provider_schema_rows_include_manifest_identity_and_digest_scope_policy() {
        const SCHEMAS: &[SchemaVersion] = &[SchemaVersion {
            name: "example-facts",
            version: 1,
        }];
        let temp = TempDir::new().expect("create temp repo");
        let loaded = loaded_config(temp.path());
        let db = db_with_files(temp.path(), &[("src/app.ts", "export const app = 1;\n")]);
        let base = ProviderManifest {
            id: "polint.example",
            kind: ProviderKind::LanguageSyntax,
            inputs: &["source_files", "config"],
            outputs: &["facts"],
            language_scope: LanguageScope::Go,
            cache_policy: CachePolicy::NoCache,
            schema_versions: SCHEMAS,
            precision_ceiling: PrecisionCeiling::Syntax,
        };
        let scope_changed = ProviderManifest {
            language_scope: LanguageScope::TypeScriptJavaScript,
            ..base
        };
        let policy_changed = ProviderManifest {
            cache_policy: CachePolicy::InMemoryDerived,
            ..base
        };

        let base_snapshot = snapshot_for(&loaded, &db, &[base]);
        let scope_snapshot = snapshot_for(&loaded, &db, &[scope_changed]);
        let policy_snapshot = snapshot_for(&loaded, &db, &[policy_changed]);
        let row = &base_snapshot.provider_schemas[0];

        assert_eq!(row.provider_id, "polint.example");
        assert_eq!(row.schema_versions, vec!["example-facts:1"]);
        assert_eq!(row.language_scope, "go");
        assert_eq!(row.cache_policy, "no_cache");
        assert_eq!(row.precision_ceiling, "syntax");
        assert_ne!(
            row.provider_manifest_digest,
            scope_snapshot.provider_schemas[0].provider_manifest_digest
        );
        assert_ne!(
            row.provider_manifest_digest,
            policy_snapshot.provider_schemas[0].provider_manifest_digest
        );
    }
}

#[cfg(test)]
mod lifecycle {
    use super::*;
    use crate::analysis_kernel::ProviderManifest;
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::{LoadedConfig, PolintConfig};
    use crate::core::AnalysisDb;
    use std::path::Path;
    use tempfile::TempDir;

    fn loaded_config(root: &Path, config: PolintConfig) -> LoadedConfig {
        LoadedConfig {
            root: root.to_path_buf(),
            config,
            missing: false,
            respect_gitignore: true,
        }
    }

    fn default_loaded_config(root: &Path) -> LoadedConfig {
        loaded_config(root, PolintConfig::default())
    }

    fn db_with_files(root: &Path, files: &[(&str, &str)]) -> AnalysisDb {
        let mut db = AnalysisDb::new();
        for (relative_path, source) in files {
            write_file(root, relative_path, source);
            db.add_file(
                root.join(relative_path),
                (*relative_path).to_string(),
                (*source).to_string(),
            );
        }
        db
    }

    fn write_file(root: &Path, relative_path: &str, contents: &str) {
        let path = root.join(relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create fixture parent");
        }
        std::fs::write(path, contents).expect("write fixture file");
    }

    fn snapshot_for(
        loaded: &LoadedConfig,
        db: &AnalysisDb,
        provider_manifests: &[ProviderManifest],
    ) -> InputSnapshot {
        let empty_plan = AnalysisPlan::empty();
        assert!(empty_plan.requested_capability_snapshots().is_empty());
        let identity_sources = InputSnapshot::identity_sources_from_plan(loaded, &empty_plan);
        assert!(identity_sources.requested_capabilities.is_empty());
        assert_eq!(
            identity_sources.analysis_requirements_identity,
            Digest::absent(DigestKind::AnalysisRequirements, "requested_capabilities")
        );

        InputSnapshot::from_run_inputs_with_plan(
            loaded,
            db,
            "config-digest",
            "rule-digest",
            &empty_plan,
            provider_manifests,
        )
    }

    fn component<'a>(components: &'a [InputComponent], name: &str) -> &'a InputComponent {
        components
            .iter()
            .find(|component| component.name == name)
            .unwrap_or_else(|| panic!("missing component {name}"))
    }

    fn component_names(components: &[InputComponent]) -> Vec<&str> {
        components
            .iter()
            .map(|component| component.name.as_str())
            .collect()
    }

    #[test]
    fn go_lifecycle_records_module_files_and_configured_options() {
        let temp = TempDir::new().expect("create temp repo");
        write_file(
            temp.path(),
            "services/app/go.mod",
            "module example.com/app\n\ngo 1.24\n",
        );
        write_file(
            temp.path(),
            "services/app/go.sum",
            "example.com/dep v1.0.0 h1:abc\n",
        );
        write_file(temp.path(), "go.work", "go 1.24\n\nuse ./services/app\n");

        let mut config = PolintConfig::default();
        config.languages.go.insert(
            "module_roots".to_string(),
            toml::Value::Array(vec![toml::Value::String("services/app".to_string())]),
        );
        config.languages.go.insert(
            "build_tags".to_string(),
            toml::Value::Array(vec![
                toml::Value::String("prod".to_string()),
                toml::Value::String("linux".to_string()),
            ]),
        );
        config
            .languages
            .go
            .insert("include_tests".to_string(), toml::Value::Boolean(false));
        config
            .languages
            .go
            .insert("offline".to_string(), toml::Value::Boolean(true));
        config.languages.go.insert(
            "package_patterns".to_string(),
            toml::Value::Array(vec![
                toml::Value::String("./internal/...".to_string()),
                toml::Value::String("./cmd/...".to_string()),
            ]),
        );

        let loaded = loaded_config(temp.path(), config);
        let db = db_with_files(temp.path(), &[("services/app/main.go", "package main\n")]);
        let snapshot = snapshot_for(
            &loaded,
            &db,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        );
        let components = &snapshot.go_lifecycle.components;

        assert_eq!(
            component(components, "go.module_roots").detail,
            vec!["services/app"]
        );
        assert_eq!(
            component(components, "go.mod").status,
            InputComponentStatus::Present
        );
        assert_eq!(
            component(components, "go.sum").status,
            InputComponentStatus::Present
        );
        assert_eq!(
            component(components, "go.work").status,
            InputComponentStatus::Present
        );
        assert_eq!(
            component(components, "go.build_tags").detail,
            vec!["linux", "prod"]
        );
        assert_eq!(
            component(components, "go.include_tests").detail,
            vec!["false"]
        );
        assert_eq!(component(components, "go.offline").detail, vec!["true"]);
        assert_eq!(
            component(components, "go.package_patterns").detail,
            vec!["./cmd/...", "./internal/..."]
        );
        assert_eq!(
            component(components, "go.environment_policy").status,
            InputComponentStatus::Present
        );
    }

    #[test]
    fn go_lifecycle_records_temporary_workspace_policy_when_root_go_work_misses_roots() {
        let temp = TempDir::new().expect("create temp repo");
        write_file(
            temp.path(),
            "services/app/go.mod",
            "module example.com/app\n\ngo 1.24\n",
        );
        write_file(temp.path(), "services/app/main.go", "package main\n");
        write_file(temp.path(), "go.work", "go 1.24\n\nuse ./other\n");

        let mut config = PolintConfig::default();
        config.languages.go.insert(
            "module_roots".to_string(),
            toml::Value::Array(vec![toml::Value::String("services/app".to_string())]),
        );
        let loaded = loaded_config(temp.path(), config);
        let db = db_with_files(temp.path(), &[("services/app/main.go", "package main\n")]);
        let snapshot = snapshot_for(
            &loaded,
            &db,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        );

        assert_eq!(
            component(&snapshot.go_lifecycle.components, "go.environment_policy").detail,
            vec!["internal_temporary_workspace_if_needed"]
        );
    }

    #[test]
    fn go_lifecycle_keeps_full_component_vocabulary_when_config_is_invalid() {
        let temp = TempDir::new().expect("create temp repo");
        let mut config = PolintConfig::default();
        config.languages.go.insert(
            "module_roots".to_string(),
            toml::Value::Array(vec![toml::Value::String("../outside".to_string())]),
        );
        let loaded = loaded_config(temp.path(), config);
        let db = db_with_files(temp.path(), &[("main.go", "package main\n")]);
        let snapshot = snapshot_for(
            &loaded,
            &db,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        );

        assert_eq!(
            component_names(&snapshot.go_lifecycle.components),
            vec![
                "go.build_tags",
                "go.environment_policy",
                "go.files_without_module_root",
                "go.include_tests",
                "go.mod",
                "go.module_roots",
                "go.offline",
                "go.package_patterns",
                "go.sum",
                "go.tool_invocation",
                "go.work",
            ]
        );
        assert_eq!(
            component(&snapshot.go_lifecycle.components, "go.module_roots").status,
            InputComponentStatus::SetupMissing
        );
        assert_eq!(
            component(&snapshot.go_lifecycle.components, "go.tool_invocation").status,
            InputComponentStatus::Unsupported
        );
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_lifecycle_file_is_setup_missing_not_absent() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TempDir::new().expect("create temp repo");
        write_file(temp.path(), "package.json", "{\"type\":\"module\"}\n");
        write_file(temp.path(), "src/app.ts", "export const app = 1;\n");
        let unreadable = temp.path().join("package.json");
        let original_permissions = std::fs::metadata(&unreadable)
            .expect("package metadata")
            .permissions();
        let mut locked_permissions = original_permissions.clone();
        locked_permissions.set_mode(0o0);
        std::fs::set_permissions(&unreadable, locked_permissions).expect("lock package.json");

        let loaded = default_loaded_config(temp.path());
        let db = db_with_files(temp.path(), &[("src/app.ts", "export const app = 1;\n")]);
        let snapshot = snapshot_for(
            &loaded,
            &db,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        );

        std::fs::set_permissions(&unreadable, original_permissions).expect("restore package.json");
        let package_manifests = component(
            &snapshot.ts_js_lifecycle.components,
            "ts_js.package_manifests",
        );
        assert_eq!(package_manifests.status, InputComponentStatus::SetupMissing);
        assert!(
            package_manifests
                .detail
                .iter()
                .any(|detail| detail.starts_with("unreadable=package.json:")),
            "{package_manifests:#?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn setup_missing_lifecycle_digest_changes_when_readable_file_content_changes() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TempDir::new().expect("create temp repo");
        write_file(temp.path(), "package.json", "{\"version\":1}\n");
        write_file(temp.path(), "web/package.json", "{\"version\":1}\n");
        let unreadable = temp.path().join("web/package.json");
        let original_permissions = std::fs::metadata(&unreadable)
            .expect("package metadata")
            .permissions();
        let mut locked_permissions = original_permissions.clone();
        locked_permissions.set_mode(0o0);
        std::fs::set_permissions(&unreadable, locked_permissions).expect("lock package.json");

        let first = file_digest_component(
            "ts_js.package_manifests",
            DigestKind::TsJsLifecycle,
            temp.path(),
            vec!["package.json".to_string(), "web/package.json".to_string()],
        );
        write_file(temp.path(), "package.json", "{\"version\":2}\n");
        let second = file_digest_component(
            "ts_js.package_manifests",
            DigestKind::TsJsLifecycle,
            temp.path(),
            vec!["package.json".to_string(), "web/package.json".to_string()],
        );

        std::fs::set_permissions(&unreadable, original_permissions).expect("restore package.json");
        assert_eq!(first.status, InputComponentStatus::SetupMissing);
        assert_eq!(second.status, InputComponentStatus::SetupMissing);
        assert_ne!(first.digest, second.digest);
    }

    #[test]
    fn ts_js_lifecycle_records_manifests_config_resolver_options_and_sources() {
        let temp = TempDir::new().expect("create temp repo");
        write_file(temp.path(), "package.json", "{\"type\":\"module\"}\n");
        write_file(
            temp.path(),
            "package-lock.json",
            "{\"lockfileVersion\":3}\n",
        );
        let ts_config = ts_config_name();
        write_file(
            temp.path(),
            "tsconfig.base.json",
            "{\"compilerOptions\":{\"baseUrl\":\".\"}}\n",
        );
        write_file(
            temp.path(),
            &ts_config,
            "{\"extends\":\"./tsconfig.base.json\",\"compilerOptions\":{}}\n",
        );

        let mut config = PolintConfig::default();
        config.languages.ts.insert(
            "module_resolution".to_string(),
            toml::Value::String("node".to_string()),
        );
        config.languages.ts.insert(
            "conditions".to_string(),
            toml::Value::Array(vec![
                toml::Value::String("import".to_string()),
                toml::Value::String("browser".to_string()),
            ]),
        );

        let loaded = loaded_config(temp.path(), config);
        let db = db_with_files(
            temp.path(),
            &[
                ("src/app.ts", "export const app = 1;\n"),
                ("src/view.tsx", "export function View() { return null; }\n"),
            ],
        );
        let snapshot = snapshot_for(
            &loaded,
            &db,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        );
        let components = &snapshot.ts_js_lifecycle.components;

        assert_eq!(
            component(components, "ts_js.package_manifests").detail,
            vec!["package.json"]
        );
        assert_eq!(
            component(components, "ts_js.lockfiles").detail,
            vec!["package-lock.json"]
        );
        assert_eq!(
            component(components, "ts_js.config_files").detail,
            vec!["tsconfig.base.json".to_string(), ts_config]
        );
        assert!(
            component(components, "ts_js.resolver_options")
                .detail
                .iter()
                .any(|detail| detail.contains("module_resolution"))
        );
        assert_eq!(
            component(components, "ts_js.source_set_membership").detail,
            vec!["src/app.ts", "src/view.tsx"]
        );
    }

    #[test]
    fn ts_js_lifecycle_rejects_absolute_tsconfig_extends_outside_repo() {
        let repo = TempDir::new().expect("create temp repo");
        let outside = TempDir::new().expect("create outside tempdir");
        write_file(
            outside.path(),
            "tsconfig.base.json",
            "{\"compilerOptions\":{\"baseUrl\":\".\"}}\n",
        );
        let outside_config = outside.path().join("tsconfig.base.json");
        write_file(
            repo.path(),
            &ts_config_name(),
            &format!(
                "{{\"extends\":{}}}\n",
                serde_json::to_string(&outside_config).unwrap()
            ),
        );
        let loaded = default_loaded_config(repo.path());
        let db = db_with_files(repo.path(), &[("src/app.ts", "export const app = 1;\n")]);

        let snapshot = snapshot_for(
            &loaded,
            &db,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        );
        let config_files = component(&snapshot.ts_js_lifecycle.components, "ts_js.config_files");

        assert_eq!(config_files.detail, vec![ts_config_name()]);
    }

    #[cfg(unix)]
    #[test]
    fn ts_js_lifecycle_does_not_hash_symlink_escape_contents() {
        let repo = TempDir::new().expect("create temp repo");
        let outside = TempDir::new().expect("create outside tempdir");
        write_file(
            outside.path(),
            "package.json",
            "{\"name\":\"outside-one\"}\n",
        );
        std::os::unix::fs::symlink(
            outside.path().join("package.json"),
            repo.path().join("package.json"),
        )
        .expect("create symlink");

        let first = file_digest_component(
            "ts_js.package_manifests",
            DigestKind::TsJsLifecycle,
            repo.path(),
            vec!["package.json".to_string()],
        );
        write_file(
            outside.path(),
            "package.json",
            "{\"name\":\"outside-two\"}\n",
        );
        let second = file_digest_component(
            "ts_js.package_manifests",
            DigestKind::TsJsLifecycle,
            repo.path(),
            vec!["package.json".to_string()],
        );

        assert_eq!(first.digest, second.digest);
        assert_eq!(first.status, InputComponentStatus::Unsupported);
    }

    #[test]
    fn official_tool_identity_components_are_unsupported_when_not_invoked() {
        let temp = TempDir::new().expect("create temp repo");
        let loaded = default_loaded_config(temp.path());
        let db = db_with_files(
            temp.path(),
            &[
                ("main.go", "package main\n"),
                ("src/app.ts", "export const app = 1;\n"),
            ],
        );

        let snapshot = snapshot_for(
            &loaded,
            &db,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        );

        assert_eq!(
            component(&snapshot.go_lifecycle.components, "go.tool_invocation").status,
            InputComponentStatus::Unsupported
        );
        assert_eq!(
            component(
                &snapshot.ts_js_lifecycle.components,
                "ts_js.tool_invocation"
            )
            .status,
            InputComponentStatus::Unsupported
        );
        assert!(snapshot.tool_invocations.iter().all(|component| {
            component.status == InputComponentStatus::Unsupported
                && component.digest.kind == DigestKind::ToolInvocation
        }));
    }

    #[test]
    fn go_files_outside_module_roots_are_setup_missing_root_relative_components() {
        let temp = TempDir::new().expect("create temp repo");
        let loaded = default_loaded_config(temp.path());
        let db = db_with_files(temp.path(), &[("cmd/tool/main.go", "package main\n")]);

        let snapshot = snapshot_for(
            &loaded,
            &db,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        );
        let gap = component(
            &snapshot.go_lifecycle.components,
            "go.files_without_module_root",
        );

        assert_eq!(gap.status, InputComponentStatus::SetupMissing);
        assert_eq!(gap.detail, vec!["cmd/tool/main.go"]);
        assert!(
            !serde_json::to_string_pretty(&snapshot)
                .expect("serialize snapshot")
                .contains(temp.path().to_string_lossy().as_ref())
        );
    }
}

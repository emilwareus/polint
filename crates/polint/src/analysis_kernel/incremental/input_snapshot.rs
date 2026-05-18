use serde::{Deserialize, Serialize};

use super::{Digest, DigestKind};
use crate::analysis_kernel::{CachePolicy, LanguageScope, PrecisionCeiling, ProviderManifest};
use crate::config::LoadedConfig;
use crate::core::{AnalysisDb, Language, SourceFile};

pub(crate) const INPUT_SNAPSHOT_SCHEMA_VERSION: &str = "polint-input-snapshot-1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct InputSnapshot {
    pub(crate) schema_version: String,
    pub(crate) files: Vec<FileSnapshot>,
    pub(crate) config: InputComponent,
    pub(crate) go_lifecycle: Vec<InputComponent>,
    pub(crate) ts_js_lifecycle: Vec<InputComponent>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum InputComponentStatus {
    Present,
    Absent,
    Unsupported,
    SetupMissing,
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

impl InputSnapshot {
    pub(crate) fn from_run_inputs(
        loaded: &LoadedConfig,
        db: &AnalysisDb,
        config_digest: &str,
        rule_digest: &str,
        plan_digest: &str,
        provider_manifests: &[ProviderManifest],
    ) -> Self {
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

        Self {
            schema_version: INPUT_SNAPSHOT_SCHEMA_VERSION.to_string(),
            files,
            config: config_component(loaded, config_digest),
            go_lifecycle: Vec::new(),
            ts_js_lifecycle: Vec::new(),
            rules: rule_components(rule_digest, plan_digest),
            models: vec![InputComponent::absent(
                "model.files",
                DigestKind::ModelFile,
                "no model files configured",
            )],
            extensions: vec![InputComponent::absent(
                "extension.providers",
                DigestKind::ExtensionCode,
                "no extension providers configured",
            )],
            tool_invocations: Vec::new(),
            provider_schemas,
        }
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
        let language_scope = language_scope_label(manifest.language_scope).to_string();
        let cache_policy = cache_policy_label(manifest.cache_policy);
        let precision_ceiling = precision_ceiling_label(manifest.precision_ceiling).to_string();
        let sorted_inputs = sorted_static_strings(manifest.inputs);
        let sorted_outputs = sorted_static_strings(manifest.outputs);

        let mut digest_parts = vec![
            format!("provider_id={}", manifest.id),
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
                DigestKind::ProviderParameters,
                "provider_manifest",
                &digest_refs,
            ),
        }
    }
}

fn config_component(loaded: &LoadedConfig, config_digest: &str) -> InputComponent {
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

    InputComponent::present(
        "config.loaded",
        DigestKind::Config,
        "config_digest",
        config_digest,
        detail,
    )
}

fn rule_components(rule_digest: &str, plan_digest: &str) -> Vec<InputComponent> {
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
            plan_digest,
            Vec::new(),
        ),
    ]
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

fn sorted_normalized_details(details: Vec<String>) -> Vec<String> {
    let mut details = details
        .into_iter()
        .map(|detail| normalize_relative_path(&detail))
        .collect::<Vec<_>>();
    details.sort();
    details.dedup();
    details
}

fn normalize_relative_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_string()
}

fn language_scope_label(scope: LanguageScope) -> &'static str {
    match scope {
        LanguageScope::Workspace => "workspace",
        LanguageScope::Go => "go",
        LanguageScope::TypeScriptJavaScript => "typescript_javascript",
        LanguageScope::MultiLanguage => "multi_language",
    }
}

fn cache_policy_label(policy: CachePolicy) -> String {
    match policy {
        CachePolicy::NoCache => "no_cache".to_string(),
        CachePolicy::ExistingFileFactCache { schema } => {
            format!("existing_file_fact_cache:{schema}")
        }
        CachePolicy::InMemoryDerived => "in_memory_derived".to_string(),
    }
}

fn precision_ceiling_label(precision: PrecisionCeiling) -> &'static str {
    match precision {
        PrecisionCeiling::Exact => "exact",
        PrecisionCeiling::Syntax => "syntax",
        PrecisionCeiling::SetupAware => "setup_aware",
    }
}

#[cfg(test)]
mod source_config_rule_model_extension {
    use super::*;
    use crate::analysis_kernel::{
        CachePolicy, LanguageScope, PrecisionCeiling, ProviderKind, ProviderManifest, SchemaVersion,
    };
    use crate::config::{LoadedConfig, PolintConfig};
    use crate::core::AnalysisDb;
    use std::path::Path;
    use tempfile::TempDir;

    fn loaded_config(root: &Path) -> LoadedConfig {
        LoadedConfig {
            root: root.to_path_buf(),
            config: PolintConfig::default(),
            missing: false,
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
        InputSnapshot::from_run_inputs(
            loaded,
            db,
            "config-digest",
            "rule-digest",
            "plan-digest",
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
            provider.provider_manifest_digest.kind == DigestKind::ProviderParameters
        }));
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

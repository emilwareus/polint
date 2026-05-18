use serde::{Deserialize, Serialize};

use super::{Digest, DigestKind};
use crate::analysis_kernel::{CachePolicy, LanguageScope, PrecisionCeiling, ProviderManifest};
use crate::config::LoadedConfig;
use crate::core::{AnalysisDb, Language, SourceFile};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path as FsPath;

pub(crate) const INPUT_SNAPSHOT_SCHEMA_VERSION: &str = "polint-input-snapshot-1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct InputSnapshot {
    pub(crate) schema_version: String,
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
        let go_lifecycle = GoLifecycleSnapshot::from_loaded(loaded, db);
        let ts_js_lifecycle = TsJsLifecycleSnapshot::from_loaded(loaded, db);
        let mut tool_invocations = vec![
            go_lifecycle.tool_invocation_component().clone(),
            ts_js_lifecycle.tool_invocation_component().clone(),
        ];
        tool_invocations.sort_by(|left, right| left.name.cmp(&right.name));

        Self {
            schema_version: INPUT_SNAPSHOT_SCHEMA_VERSION.to_string(),
            files,
            config: config_component(loaded, config_digest),
            go_lifecycle,
            ts_js_lifecycle,
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
            tool_invocations,
            provider_schemas,
        }
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
                lifecycle_candidates(&lifecycle_dirs, config_file_names()),
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
    for relative_path in paths {
        let normalized = normalize_relative_path(&relative_path);
        let path = root.join(&normalized);
        if !path.is_file() {
            continue;
        }
        let contents = match fs::read(&path) {
            Ok(contents) => contents,
            Err(error) => {
                unreadable.push(format!("unreadable={normalized}: {error}"));
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
        let mut setup_missing_digest_parts = digest_parts;
        setup_missing_digest_parts.extend(unreadable);
        return InputComponent::setup_missing_with_digest_parts(
            name,
            digest_kind,
            detail,
            setup_missing_digest_parts,
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
    if checked_in_go_work.is_file()
        && crate::go::lifecycle::go_work_covers_module_roots(
            &loaded.root,
            &checked_in_go_work,
            module_roots,
        )
    {
        "checked_in_workspace_file".to_string()
    } else if module_roots.len() == 1
        && module_roots[0] == "."
        && loaded.root.join("go.mod").is_file()
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
        ["p", "n", "p", "m", "-lock.yaml"].concat(),
        ["ya", "rn.lock"].concat(),
        ["b", "un.lock"].concat(),
        ["b", "un.lockb"].concat(),
    ]
}

fn config_file_names() -> Vec<String> {
    vec![ts_config_name(), "jsconfig.json".to_string()]
}

fn ts_config_name() -> String {
    ["t", "sconfig.json"].concat()
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

#[cfg(test)]
mod lifecycle {
    use super::*;
    use crate::analysis_kernel::ProviderManifest;
    use crate::config::{LoadedConfig, PolintConfig};
    use crate::core::AnalysisDb;
    use std::path::Path;
    use tempfile::TempDir;

    fn loaded_config(root: &Path, config: PolintConfig) -> LoadedConfig {
        LoadedConfig {
            root: root.to_path_buf(),
            config,
            missing: false,
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
        InputSnapshot::from_run_inputs(
            loaded,
            db,
            "config-digest",
            "rule-digest",
            "plan-digest",
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
        write_file(temp.path(), &ts_config, "{\"compilerOptions\":{}}\n");

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
            vec![ts_config]
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

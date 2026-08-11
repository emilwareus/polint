//! Digest-only input snapshot vocabulary shared by analysis providers.
//!
//! Construction from loaded config / module path IO stays in the facade kernel.

use serde::{Deserialize, Serialize};

use crate::digest::{Digest, DigestKind};
use crate::provider::ProviderManifest;
use crate::source_file::SourceFile;
use polint_core::Language;

pub const INPUT_SNAPSHOT_SCHEMA_VERSION: &str = "polint-input-snapshot-1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputSnapshot {
    pub schema_version: String,
    pub files: Vec<FileSnapshot>,
    pub config: InputComponent,
    pub go_lifecycle: GoLifecycleSnapshot,
    pub ts_js_lifecycle: TsJsLifecycleSnapshot,
    pub rules: Vec<InputComponent>,
    pub models: Vec<InputComponent>,
    pub extensions: Vec<InputComponent>,
    pub tool_invocations: Vec<InputComponent>,
    pub provider_schemas: Vec<ProviderSchemaSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSnapshot {
    pub relative_path: String,
    pub language: Language,
    pub source_text_digest: Digest,
    pub size_bytes: usize,
    pub mtime_hint_present: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoLifecycleSnapshot {
    pub components: Vec<InputComponent>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TsJsLifecycleSnapshot {
    pub components: Vec<InputComponent>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputComponentStatus {
    Present,
    Absent,
    Unsupported,
    SetupMissing,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputComponent {
    pub name: String,
    pub status: InputComponentStatus,
    pub digest: Digest,
    pub detail: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSchemaSnapshot {
    pub provider_id: String,
    pub schema_versions: Vec<String>,
    pub language_scope: String,
    pub cache_policy: String,
    pub precision_ceiling: String,
    pub provider_manifest_digest: Digest,
}

impl FileSnapshot {
    pub fn from_source_file(file: &SourceFile) -> Self {
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
    pub fn present(
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

    pub fn absent(name: impl Into<String>, digest_kind: DigestKind, reason: &str) -> Self {
        let name = name.into();
        Self::new(
            name.clone(),
            InputComponentStatus::Absent,
            Digest::absent(digest_kind, &name),
            vec![reason.to_string()],
        )
    }

    pub fn unsupported(name: impl Into<String>, digest_kind: DigestKind, reason: &str) -> Self {
        let name = name.into();
        Self::new(
            name.clone(),
            InputComponentStatus::Unsupported,
            Digest::unsupported(digest_kind, &name, reason),
            vec![reason.to_string()],
        )
    }

    pub fn setup_missing(
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

    pub fn setup_missing_with_digest_parts(
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

    pub fn new(
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
    pub fn from_manifest(manifest: &ProviderManifest) -> Self {
        let schema_versions = sorted_schema_labels(manifest.schema_versions);
        let language_scope = manifest.language_scope_label().to_string();
        let cache_policy = manifest.cache_policy_label();
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

fn sorted_normalized_details(details: Vec<String>) -> Vec<String> {
    let mut details = details
        .into_iter()
        .map(|detail| detail.replace('\\', "/"))
        .collect::<Vec<_>>();
    details.sort();
    details.dedup();
    details
}

fn normalize_relative_path(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_string()
}

fn sorted_schema_labels(schemas: &[crate::provider::SchemaVersion]) -> Vec<String> {
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

fn precision_ceiling_label(precision: crate::provider::PrecisionCeiling) -> &'static str {
    match precision {
        crate::provider::PrecisionCeiling::Exact => "exact",
        crate::provider::PrecisionCeiling::Syntax => "syntax",
        crate::provider::PrecisionCeiling::SetupAware => "setup_aware",
    }
}

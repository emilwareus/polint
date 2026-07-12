#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "canonical digest codecs and opaque identities are consumed across private persistence boundaries"
    )
)]

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct Digest {
    pub(crate) kind: DigestKind,
    pub(crate) value: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DigestKind {
    SourceText,
    Config,
    GoLifecycle,
    TsJsLifecycle,
    RuleCode,
    RuleOptions,
    ModelFile,
    ExtensionCode,
    ToolInvocation,
    ProviderParameters,
    ProviderOutput,
    LayerOutput,
    DependencyLayer,
    QueryParameters,
    Budget,
    Evidence,
    SummaryBody,
    SummaryDependency,
    Workspace,
    Run,
    Generation,
    InputSnapshot,
    AnalysisSettings,
    AnalysisRequirements,
    ProviderManifest,
    Layer,
    Summary,
    Query,
    FactMetadata,
    Dependency,
    ValidationEvent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UnknownDigestKindLabel {
    label: String,
}

impl fmt::Display for UnknownDigestKindLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown digest kind label `{}`", self.label)
    }
}

impl std::error::Error for UnknownDigestKindLabel {}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub(crate) struct WorkspaceIdentity(Digest);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub(crate) struct ConfigIdentity(Digest);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub(crate) struct RunIdentity(Digest);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub(crate) struct GenerationIdentity(Digest);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct IdentityDigestKindError {
    identity: &'static str,
    expected: &'static str,
    actual: DigestKind,
}

impl fmt::Display for IdentityDigestKindError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} requires a {} digest, found {}",
            self.identity,
            self.expected,
            self.actual.label()
        )
    }
}

impl std::error::Error for IdentityDigestKindError {}

impl Digest {
    pub(crate) fn from_parts(kind: DigestKind, label: &str, parts: &[&str]) -> Self {
        let kind_label = kind.label();
        let mut hash = FNV_OFFSET_BASIS;
        fingerprint_length_prefixed_part(&mut hash, "kind", kind_label);
        for part in parts {
            fingerprint_length_prefixed_part(&mut hash, label, part);
        }

        Self {
            kind,
            value: format!("{hash:016x}"),
        }
    }

    pub(crate) fn builder(kind: DigestKind, label: &'static str) -> DigestBuilder {
        DigestBuilder::new(kind, label)
    }

    pub(crate) fn from_unordered(kind: DigestKind, label: &str, mut digests: Vec<Digest>) -> Self {
        digests.sort();
        let digest_parts = digests.iter().map(ToString::to_string).collect::<Vec<_>>();
        let hash_parts = digest_parts.iter().map(String::as_str).collect::<Vec<_>>();

        Self::from_parts(kind, label, &hash_parts)
    }

    pub(crate) fn absent(kind: DigestKind, label: &str) -> Self {
        Self::from_parts(kind, "absent", &[label])
    }

    pub(crate) fn unsupported(kind: DigestKind, label: &str, reason: &str) -> Self {
        Self::from_parts(kind, "unsupported", &[label, reason])
    }
}

#[derive(Debug)]
pub(crate) struct DigestBuilder {
    kind: DigestKind,
    label: &'static str,
    hash: u64,
}

impl DigestBuilder {
    fn new(kind: DigestKind, label: &'static str) -> Self {
        let mut hash = FNV_OFFSET_BASIS;
        fingerprint_length_prefixed_part(&mut hash, "kind", kind.label());
        Self { kind, label, hash }
    }

    pub(crate) fn part(&mut self, value: &str) {
        fingerprint_length_prefixed_part(&mut self.hash, self.label, value);
    }

    pub(crate) fn labeled_part(&mut self, label: &str, value: &str) {
        fingerprint_length_prefixed_part(&mut self.hash, label, value);
    }

    pub(crate) fn debug_part(&mut self, value: impl fmt::Debug) {
        self.part(&format!("{value:?}"));
    }

    pub(crate) fn bool_part(&mut self, value: bool) {
        self.part(if value { "true" } else { "false" });
    }

    pub(crate) fn finish(self) -> Digest {
        Digest {
            kind: self.kind,
            value: format!("{:016x}", self.hash),
        }
    }
}

impl DigestKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::SourceText => "source_text",
            Self::Config => "config",
            Self::GoLifecycle => "go_lifecycle",
            Self::TsJsLifecycle => "ts_js_lifecycle",
            Self::RuleCode => "rule_code",
            Self::RuleOptions => "rule_options",
            Self::ModelFile => "model_file",
            Self::ExtensionCode => "extension_code",
            Self::ToolInvocation => "tool_invocation",
            Self::ProviderParameters => "provider_parameters",
            Self::ProviderOutput => "provider_output",
            Self::LayerOutput => "layer_output",
            Self::DependencyLayer => "dependency_layer",
            Self::QueryParameters => "query_parameters",
            Self::Budget => "budget",
            Self::Evidence => "evidence",
            Self::SummaryBody => "summary_body",
            Self::SummaryDependency => "summary_dependency",
            Self::Workspace => "workspace",
            Self::Run => "run",
            Self::Generation => "generation",
            Self::InputSnapshot => "input_snapshot",
            Self::AnalysisSettings => "analysis_settings",
            Self::AnalysisRequirements => "analysis_requirements",
            Self::ProviderManifest => "provider_manifest",
            Self::Layer => "layer",
            Self::Summary => "summary",
            Self::Query => "query",
            Self::FactMetadata => "fact_metadata",
            Self::Dependency => "dependency",
            Self::ValidationEvent => "validation_event",
        }
    }

    pub(crate) fn parse_label(label: &str) -> Result<Self, UnknownDigestKindLabel> {
        match label {
            "source_text" => Ok(Self::SourceText),
            "config" => Ok(Self::Config),
            "go_lifecycle" => Ok(Self::GoLifecycle),
            "ts_js_lifecycle" => Ok(Self::TsJsLifecycle),
            "rule_code" => Ok(Self::RuleCode),
            "rule_options" => Ok(Self::RuleOptions),
            "model_file" => Ok(Self::ModelFile),
            "extension_code" => Ok(Self::ExtensionCode),
            "tool_invocation" => Ok(Self::ToolInvocation),
            "provider_parameters" => Ok(Self::ProviderParameters),
            "provider_output" => Ok(Self::ProviderOutput),
            "layer_output" => Ok(Self::LayerOutput),
            "dependency_layer" => Ok(Self::DependencyLayer),
            "query_parameters" => Ok(Self::QueryParameters),
            "budget" => Ok(Self::Budget),
            "evidence" => Ok(Self::Evidence),
            "summary_body" => Ok(Self::SummaryBody),
            "summary_dependency" => Ok(Self::SummaryDependency),
            "workspace" => Ok(Self::Workspace),
            "run" => Ok(Self::Run),
            "generation" => Ok(Self::Generation),
            "input_snapshot" => Ok(Self::InputSnapshot),
            "analysis_settings" => Ok(Self::AnalysisSettings),
            "analysis_requirements" => Ok(Self::AnalysisRequirements),
            "provider_manifest" => Ok(Self::ProviderManifest),
            "layer" => Ok(Self::Layer),
            "summary" => Ok(Self::Summary),
            "query" => Ok(Self::Query),
            "fact_metadata" => Ok(Self::FactMetadata),
            "dependency" => Ok(Self::Dependency),
            "validation_event" => Ok(Self::ValidationEvent),
            _ => Err(UnknownDigestKindLabel {
                label: label.to_string(),
            }),
        }
    }
}

impl WorkspaceIdentity {
    pub(crate) fn from_roots<'a>(roots: impl IntoIterator<Item = &'a Path>) -> Self {
        let mut normalized_roots = roots
            .into_iter()
            .map(normalize_workspace_root)
            .collect::<Vec<_>>();
        normalized_roots.sort();
        normalized_roots.dedup();
        let root_parts = normalized_roots
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();

        Self(Digest::from_parts(
            DigestKind::Workspace,
            "workspace_root",
            &root_parts,
        ))
    }

    pub(crate) fn digest(&self) -> &Digest {
        &self.0
    }
}

impl ConfigIdentity {
    pub(crate) fn from_complete_config_digest(
        digest: Digest,
    ) -> Result<Self, IdentityDigestKindError> {
        require_digest_kind("ConfigIdentity", "config", &digest, DigestKind::Config)?;
        Ok(Self(digest))
    }

    pub(crate) fn digest(&self) -> &Digest {
        &self.0
    }
}

impl RunIdentity {
    pub(crate) fn new(
        workspace: &WorkspaceIdentity,
        full_config: &ConfigIdentity,
        input_snapshot: &Digest,
        provider_manifest: &Digest,
    ) -> Result<Self, IdentityDigestKindError> {
        require_digest_kind(
            "RunIdentity",
            "input_snapshot",
            input_snapshot,
            DigestKind::InputSnapshot,
        )?;
        require_digest_kind(
            "RunIdentity",
            "provider_manifest",
            provider_manifest,
            DigestKind::ProviderManifest,
        )?;

        let mut builder = Digest::builder(DigestKind::Run, "run_identity");
        builder.labeled_part("workspace_kind", workspace.digest().kind.label());
        builder.labeled_part("workspace_value", &workspace.digest().value);
        builder.labeled_part("config_kind", full_config.digest().kind.label());
        builder.labeled_part("config_value", &full_config.digest().value);
        builder.labeled_part("input_snapshot_kind", input_snapshot.kind.label());
        builder.labeled_part("input_snapshot_value", &input_snapshot.value);
        builder.labeled_part("provider_manifest_kind", provider_manifest.kind.label());
        builder.labeled_part("provider_manifest_value", &provider_manifest.value);
        Ok(Self(builder.finish()))
    }

    pub(crate) fn digest(&self) -> &Digest {
        &self.0
    }
}

impl GenerationIdentity {
    pub(crate) fn new(
        run: &RunIdentity,
        semantic_family_aggregates: &[Digest],
    ) -> Result<Self, IdentityDigestKindError> {
        for aggregate in semantic_family_aggregates {
            if !is_generation_semantic_aggregate(aggregate.kind) {
                return Err(IdentityDigestKindError {
                    identity: "GenerationIdentity",
                    expected: GENERATION_SEMANTIC_KINDS,
                    actual: aggregate.kind,
                });
            }
        }

        let mut aggregates = semantic_family_aggregates.iter().collect::<Vec<_>>();
        aggregates.sort();
        let mut builder = Digest::builder(DigestKind::Generation, "generation_identity");
        builder.labeled_part("run_kind", run.digest().kind.label());
        builder.labeled_part("run_value", &run.digest().value);
        for aggregate in aggregates {
            builder.labeled_part("semantic_family_kind", aggregate.kind.label());
            builder.labeled_part("semantic_family_value", &aggregate.value);
        }
        Ok(Self(builder.finish()))
    }

    pub(crate) fn digest(&self) -> &Digest {
        &self.0
    }
}

const GENERATION_SEMANTIC_KINDS: &str =
    "provider_output, layer, summary, query, fact_metadata, dependency, or validation_event";

fn require_digest_kind(
    identity: &'static str,
    expected_label: &'static str,
    digest: &Digest,
    expected: DigestKind,
) -> Result<(), IdentityDigestKindError> {
    if digest.kind == expected {
        Ok(())
    } else {
        Err(IdentityDigestKindError {
            identity,
            expected: expected_label,
            actual: digest.kind,
        })
    }
}

fn is_generation_semantic_aggregate(kind: DigestKind) -> bool {
    match kind {
        DigestKind::ProviderOutput
        | DigestKind::Layer
        | DigestKind::Summary
        | DigestKind::Query
        | DigestKind::FactMetadata
        | DigestKind::Dependency
        | DigestKind::ValidationEvent => true,
        DigestKind::SourceText
        | DigestKind::Config
        | DigestKind::GoLifecycle
        | DigestKind::TsJsLifecycle
        | DigestKind::RuleCode
        | DigestKind::RuleOptions
        | DigestKind::ModelFile
        | DigestKind::ExtensionCode
        | DigestKind::ToolInvocation
        | DigestKind::ProviderParameters
        | DigestKind::LayerOutput
        | DigestKind::DependencyLayer
        | DigestKind::QueryParameters
        | DigestKind::Budget
        | DigestKind::Evidence
        | DigestKind::SummaryBody
        | DigestKind::SummaryDependency
        | DigestKind::Workspace
        | DigestKind::Run
        | DigestKind::Generation
        | DigestKind::InputSnapshot
        | DigestKind::AnalysisSettings
        | DigestKind::AnalysisRequirements
        | DigestKind::ProviderManifest => false,
    }
}

fn normalize_workspace_root(root: &Path) -> String {
    let raw = root.to_string_lossy().replace('\\', "/");
    let is_absolute = raw.starts_with('/');
    let mut segments = Vec::new();
    for segment in raw.split('/') {
        match segment {
            "" | "." => {}
            ".." if segments.last().is_some_and(|last| *last != "..") => {
                segments.pop();
            }
            ".." if !is_absolute => segments.push(segment),
            ".." => {}
            _ => segments.push(segment),
        }
    }

    let normalized = segments.join("/");
    if is_absolute {
        format!("/{normalized}")
    } else if normalized.is_empty() {
        ".".to_string()
    } else {
        normalized
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.kind.label(), self.value)
    }
}

const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;
const PART_SEPARATOR: u8 = 0xfe;

fn fingerprint_length_prefixed_part(hash: &mut u64, label: &str, value: &str) {
    fingerprint_usize_decimal(hash, label.len());
    fingerprint_byte(hash, b':');
    fingerprint_bytes(hash, label.as_bytes());
    fingerprint_byte(hash, b'=');
    fingerprint_usize_decimal(hash, value.len());
    fingerprint_byte(hash, b':');
    fingerprint_bytes(hash, value.as_bytes());
    fingerprint_byte(hash, PART_SEPARATOR);
}

fn fingerprint_usize_decimal(hash: &mut u64, mut value: usize) {
    if value == 0 {
        fingerprint_byte(hash, b'0');
        return;
    }

    let mut buffer = [0u8; 20];
    let mut index = buffer.len();
    while value > 0 {
        index -= 1;
        buffer[index] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    fingerprint_bytes(hash, &buffer[index..]);
}

fn fingerprint_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        fingerprint_byte(hash, *byte);
    }
}

fn fingerprint_byte(hash: &mut u64, byte: u8) {
    *hash ^= u64::from(byte);
    *hash = hash.wrapping_mul(FNV_PRIME);
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_DIGEST_KINDS: &[DigestKind] = &[
        DigestKind::SourceText,
        DigestKind::Config,
        DigestKind::GoLifecycle,
        DigestKind::TsJsLifecycle,
        DigestKind::RuleCode,
        DigestKind::RuleOptions,
        DigestKind::ModelFile,
        DigestKind::ExtensionCode,
        DigestKind::ToolInvocation,
        DigestKind::ProviderParameters,
        DigestKind::ProviderOutput,
        DigestKind::LayerOutput,
        DigestKind::DependencyLayer,
        DigestKind::QueryParameters,
        DigestKind::Budget,
        DigestKind::Evidence,
        DigestKind::SummaryBody,
        DigestKind::SummaryDependency,
        DigestKind::Workspace,
        DigestKind::Run,
        DigestKind::Generation,
        DigestKind::InputSnapshot,
        DigestKind::AnalysisSettings,
        DigestKind::AnalysisRequirements,
        DigestKind::ProviderManifest,
        DigestKind::Layer,
        DigestKind::Summary,
        DigestKind::Query,
        DigestKind::FactMetadata,
        DigestKind::Dependency,
        DigestKind::ValidationEvent,
    ];

    #[test]
    fn from_parts_is_deterministic_and_kind_separated() {
        let first = Digest::from_parts(DigestKind::SourceText, "source_text", &["path", "hash"]);
        let second = Digest::from_parts(DigestKind::SourceText, "source_text", &["path", "hash"]);
        let config = Digest::from_parts(DigestKind::Config, "source_text", &["path", "hash"]);

        assert_eq!(first, second);
        assert_ne!(first, config);
    }

    #[test]
    fn from_unordered_sorts_input_digests_canonically() {
        let a = Digest::from_parts(DigestKind::SourceText, "file", &["a"]);
        let b = Digest::from_parts(DigestKind::SourceText, "file", &["b"]);

        assert_eq!(
            Digest::from_unordered(DigestKind::LayerOutput, "layer", vec![b.clone(), a.clone()]),
            Digest::from_unordered(DigestKind::LayerOutput, "layer", vec![a, b])
        );
    }

    #[test]
    fn serde_and_display_include_kind_and_value() {
        let digest = Digest::from_parts(DigestKind::SourceText, "source_text", &["path"]);
        let json = serde_json::to_string(&digest).expect("digest should serialize");

        assert!(json.contains("\"kind\""));
        assert!(json.contains("\"value\""));
        assert_eq!(digest.to_string(), format!("source_text:{}", digest.value));
    }

    #[test]
    fn absent_and_unsupported_helpers_are_explicit() {
        assert_ne!(
            Digest::absent(DigestKind::ToolInvocation, "go"),
            Digest::unsupported(DigestKind::ToolInvocation, "go", "not on path")
        );
    }

    #[test]
    fn digest_kind_stable_codec_round_trips_every_variant() {
        for kind in ALL_DIGEST_KINDS {
            assert_eq!(DigestKind::parse_label(kind.label()), Ok(*kind));
        }
    }

    #[test]
    fn digest_kind_stable_codec_rejects_unknown_labels() {
        let error = DigestKind::parse_label("provider-output").expect_err("label must be exact");

        assert_eq!(
            error.to_string(),
            "unknown digest kind label `provider-output`"
        );
    }

    #[test]
    fn workspace_identity_normalizes_sorts_and_discards_roots() {
        const SECRET_ROOT: &str = "/tmp/polint-workspace-sentinel/project";
        let first = WorkspaceIdentity::from_roots([
            Path::new("/tmp/polint-workspace-sentinel/other"),
            Path::new("/tmp/polint-workspace-sentinel/project/./src/.."),
        ]);
        let second = WorkspaceIdentity::from_roots([
            Path::new(SECRET_ROOT),
            Path::new("/tmp/polint-workspace-sentinel/other/"),
        ]);

        assert_eq!(first, second);
        assert_eq!(first.digest().kind, DigestKind::Workspace);
        let serialized = serde_json::to_string(&first).expect("workspace identity serializes");
        assert!(!serialized.contains("polint-workspace-sentinel"));
        assert!(!format!("{first:?}").contains("polint-workspace-sentinel"));
    }

    #[test]
    fn identity_construction_rejects_wrong_digest_purposes() {
        let workspace = WorkspaceIdentity::from_roots([Path::new("/repo")]);
        let config = ConfigIdentity::from_complete_config_digest(Digest::from_parts(
            DigestKind::Config,
            "config",
            &["complete"],
        ))
        .expect("complete config identity");
        let input = Digest::from_parts(DigestKind::InputSnapshot, "input", &["snapshot"]);
        let manifests =
            Digest::from_parts(DigestKind::ProviderManifest, "provider_manifests", &["all"]);
        let run = RunIdentity::new(&workspace, &config, &input, &manifests).expect("run identity");

        assert!(
            ConfigIdentity::from_complete_config_digest(Digest::from_parts(
                DigestKind::AnalysisSettings,
                "settings",
                &["scoped"],
            ))
            .is_err()
        );
        assert!(
            RunIdentity::new(
                &workspace,
                &config,
                &Digest::from_parts(DigestKind::Config, "wrong", &["input"]),
                &manifests,
            )
            .is_err()
        );
        assert!(
            GenerationIdentity::new(
                &run,
                &[Digest::from_parts(
                    DigestKind::Config,
                    "wrong",
                    &["semantic"]
                )],
            )
            .is_err()
        );
    }

    #[test]
    fn run_and_generation_identities_are_permutation_stable() {
        let workspace_a = WorkspaceIdentity::from_roots([
            Path::new("/repo/packages/b"),
            Path::new("/repo/packages/a"),
        ]);
        let workspace_b = WorkspaceIdentity::from_roots([
            Path::new("/repo/packages/a"),
            Path::new("/repo/packages/b"),
        ]);
        let config = ConfigIdentity::from_complete_config_digest(Digest::from_parts(
            DigestKind::Config,
            "config",
            &["complete"],
        ))
        .expect("complete config identity");
        let input = Digest::from_parts(DigestKind::InputSnapshot, "input", &["snapshot"]);
        let manifest_a = Digest::from_parts(DigestKind::ProviderManifest, "manifest", &["a"]);
        let manifest_b = Digest::from_parts(DigestKind::ProviderManifest, "manifest", &["b"]);
        let manifests_a = Digest::from_unordered(
            DigestKind::ProviderManifest,
            "provider_manifests",
            vec![manifest_b.clone(), manifest_a.clone()],
        );
        let manifests_b = Digest::from_unordered(
            DigestKind::ProviderManifest,
            "provider_manifests",
            vec![manifest_a, manifest_b],
        );
        let run_a = RunIdentity::new(&workspace_a, &config, &input, &manifests_a)
            .expect("first run identity");
        let run_b = RunIdentity::new(&workspace_b, &config, &input, &manifests_b)
            .expect("second run identity");
        let facts = Digest::from_parts(DigestKind::FactMetadata, "facts", &["all"]);
        let providers = Digest::from_parts(DigestKind::ProviderOutput, "providers", &["all"]);
        let generation_a = GenerationIdentity::new(&run_a, &[facts.clone(), providers.clone()])
            .expect("first generation identity");
        let generation_b = GenerationIdentity::new(&run_b, &[providers, facts])
            .expect("second generation identity");

        assert_eq!(run_a, run_b);
        assert_eq!(generation_a, generation_b);
        assert_eq!(run_a.digest().kind, DigestKind::Run);
        assert_eq!(generation_a.digest().kind, DigestKind::Generation);
    }
}

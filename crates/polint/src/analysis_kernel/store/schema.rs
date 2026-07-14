//! Typed relational vocabulary for the private semantic store.

#![allow(
    dead_code,
    reason = "typed codecs and lifecycle validation guard relational data before publication"
)]

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Component, Path};

use crate::analysis_kernel::incremental::{
    CacheNodeKind, DEPENDENCY_INDEX_SCHEMA, DependencyKind, Digest, DigestKind,
    INPUT_SNAPSHOT_SCHEMA_VERSION, InputComponentStatus, InputDependencyKey, InputDependencyKind,
    LayerKind, PrecisionTier, ProviderValidationStatus, ShapeKind, WorkspaceIdentity,
};
use crate::analysis_kernel::validation::{ValidationEventKind, ValidationEventStatus};
use crate::analysis_kernel::{
    CachePolicy, CachePolicyView, FactConfidence, FactFamily, FactPrecision, LanguageScope,
    PrecisionCeiling, ProviderKind, ValidationStatus,
};
use crate::analysis_plan::CapabilitySetupStatus;
use crate::cache::keys::AnalysisSettingsScope;
use crate::core::{CapabilitySupportStatus, Language};

use super::commit_plan::StoreInputGroup;

pub(super) const REQUIRED_V2_TABLES: [&str; 36] = [
    "store_manifest",
    "generations",
    "run_manifest_nodes",
    "input_snapshots",
    "input_files",
    "input_components",
    "input_component_details",
    "analysis_settings",
    "requested_capabilities",
    "capability_requesters",
    "provider_schema_snapshots",
    "provider_schema_versions",
    "provider_manifests",
    "provider_manifest_schemas",
    "provider_manifest_inputs",
    "provider_manifest_outputs",
    "provider_generations",
    "provider_dependencies",
    "layers",
    "layer_input_digests",
    "layer_dependency_digests",
    "layer_extension_digests",
    "layer_warnings",
    "summaries",
    "summary_dependency_digests",
    "queries",
    "query_inputs",
    "query_layer_digests",
    "fact_metadata",
    "diagnostic_nodes",
    "diagnostic_requested_view_digests",
    "dependency_edges",
    "validation_events",
    "generation_stats",
    "generation_telemetry",
    "generation_failure_events",
];

pub(super) const SEMANTIC_ORDER_BY: [(&str, &str); 33] = [
    (
        "generations",
        "ORDER BY workspace_kind, workspace_value, generation_kind, generation_value, reservation_ordinal",
    ),
    ("run_manifest_nodes", "ORDER BY generation_id, id"),
    ("input_files", "ORDER BY relative_path, language"),
    (
        "input_components",
        "ORDER BY component_group, name, status, digest_kind, digest_value",
    ),
    (
        "input_component_details",
        "ORDER BY component_group, component_name, ordinal",
    ),
    (
        "analysis_settings",
        "ORDER BY scope, digest_kind, digest_value",
    ),
    (
        "requested_capabilities",
        "ORDER BY capability, language, support_status, setup_status",
    ),
    (
        "capability_requesters",
        "ORDER BY capability, language, rule_id",
    ),
    ("provider_schema_snapshots", "ORDER BY provider_id"),
    (
        "provider_schema_versions",
        "ORDER BY provider_id, schema_version",
    ),
    (
        "provider_manifests",
        "ORDER BY provider_id, provider_version",
    ),
    (
        "provider_manifest_schemas",
        "ORDER BY provider_id, schema_version",
    ),
    (
        "provider_manifest_inputs",
        "ORDER BY provider_id, input_name",
    ),
    (
        "provider_manifest_outputs",
        "ORDER BY provider_id, output_name",
    ),
    (
        "provider_generations",
        "ORDER BY provider_id, provider_version, schema_version, output_digest_kind, output_digest_value",
    ),
    (
        "provider_dependencies",
        "ORDER BY provider.provider_id, provider.provider_version, provider.schema_version, \
         provider.output_digest_kind, provider.output_digest_value, dependency.ordinal",
    ),
    ("layers", "ORDER BY semantic_ordinal"),
    (
        "layer_input_digests",
        "ORDER BY layer.semantic_ordinal, child.ordinal",
    ),
    (
        "layer_dependency_digests",
        "ORDER BY layer.semantic_ordinal, child.ordinal",
    ),
    (
        "layer_extension_digests",
        "ORDER BY layer.semantic_ordinal, child.ordinal",
    ),
    (
        "layer_warnings",
        "ORDER BY layer.semantic_ordinal, child.ordinal, child.warning_code",
    ),
    ("summaries", "ORDER BY semantic_ordinal"),
    (
        "summary_dependency_digests",
        "ORDER BY summary.semantic_ordinal, child.ordinal",
    ),
    ("queries", "ORDER BY semantic_ordinal"),
    (
        "query_inputs",
        "ORDER BY query.semantic_ordinal, child.ordinal",
    ),
    (
        "query_layer_digests",
        "ORDER BY query.semantic_ordinal, child.ordinal",
    ),
    ("fact_metadata", "ORDER BY ordinal"),
    ("diagnostic_nodes", "ORDER BY semantic_ordinal"),
    (
        "diagnostic_requested_view_digests",
        "ORDER BY diagnostic.semantic_ordinal, child.ordinal",
    ),
    ("dependency_edges", "ORDER BY ordinal"),
    ("validation_events", "ORDER BY event_kind, status"),
    ("generation_telemetry", "ORDER BY relative_path"),
    ("generation_failure_events", "ORDER BY ordinal"),
];

pub(super) fn semantic_order_by(table: &str) -> Option<&'static str> {
    SEMANTIC_ORDER_BY
        .iter()
        .find_map(|(candidate, clause)| (*candidate == table).then_some(*clause))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SchemaCodecError {
    UnknownLabel {
        vocabulary: &'static str,
        label: String,
    },
    WrongDigestKind {
        expected: DigestKind,
        actual: DigestKind,
    },
    EmptyDigest,
    InvalidRelativePath(String),
    InvalidInputDependency,
}

impl fmt::Display for SchemaCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownLabel { vocabulary, label } => {
                write!(formatter, "unknown {vocabulary} label `{label}`")
            }
            Self::WrongDigestKind { expected, actual } => write!(
                formatter,
                "expected {} digest, found {}",
                expected.label(),
                actual.label()
            ),
            Self::EmptyDigest => formatter.write_str("digest values must not be empty"),
            Self::InvalidRelativePath(path) => {
                write!(
                    formatter,
                    "store path must be relative and normalized: `{path}`"
                )
            }
            Self::InvalidInputDependency => {
                formatter.write_str("input dependency key is not canonical")
            }
        }
    }
}

impl std::error::Error for SchemaCodecError {}

pub(super) fn encode_digest(digest: &Digest) -> (&'static str, &str) {
    (digest.kind.label(), digest.value.as_str())
}

pub(super) fn decode_digest(
    kind: &str,
    value: &str,
    expected: Option<DigestKind>,
) -> Result<Digest, SchemaCodecError> {
    let kind = DigestKind::parse_label(kind).map_err(|_| SchemaCodecError::UnknownLabel {
        vocabulary: "digest kind",
        label: kind.to_owned(),
    })?;
    if let Some(expected) = expected
        && kind != expected
    {
        return Err(SchemaCodecError::WrongDigestKind {
            expected,
            actual: kind,
        });
    }
    if value.is_empty() {
        return Err(SchemaCodecError::EmptyDigest);
    }
    Ok(Digest {
        kind,
        value: value.to_owned(),
    })
}

pub(super) fn validate_relative_path(path: &str) -> Result<(), SchemaCodecError> {
    let candidate = Path::new(path);
    let normalized = !path.is_empty()
        && !candidate.is_absolute()
        && candidate
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        && path
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
        && !path.contains('\\');
    if normalized {
        Ok(())
    } else {
        Err(SchemaCodecError::InvalidRelativePath(path.to_owned()))
    }
}

pub(super) fn validate_input_snapshot_schema(label: &str) -> Result<(), SchemaCodecError> {
    if label == INPUT_SNAPSHOT_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(SchemaCodecError::UnknownLabel {
            vocabulary: "input snapshot schema",
            label: label.to_owned(),
        })
    }
}

pub(super) fn validate_dependency_schema(label: &str) -> Result<(), SchemaCodecError> {
    if label == DEPENDENCY_INDEX_SCHEMA {
        Ok(())
    } else {
        Err(SchemaCodecError::UnknownLabel {
            vocabulary: "dependency index schema",
            label: label.to_owned(),
        })
    }
}

macro_rules! canonical_label_codec {
    ($encode:ident, $decode:ident, $ty:ty, $vocabulary:literal) => {
        pub(super) fn $encode(value: $ty) -> &'static str {
            value.label()
        }

        pub(super) fn $decode(label: &str) -> Result<$ty, SchemaCodecError> {
            <$ty>::parse_label(label).map_err(|_| SchemaCodecError::UnknownLabel {
                vocabulary: $vocabulary,
                label: label.to_owned(),
            })
        }
    };
}

canonical_label_codec!(encode_language, decode_language, Language, "language");
canonical_label_codec!(
    encode_input_component_status,
    decode_input_component_status,
    InputComponentStatus,
    "input component status"
);
canonical_label_codec!(
    encode_input_dependency_kind,
    decode_input_dependency_kind,
    InputDependencyKind,
    "input dependency kind"
);
canonical_label_codec!(
    encode_layer_kind,
    decode_layer_kind,
    LayerKind,
    "layer kind"
);
canonical_label_codec!(
    encode_precision_tier,
    decode_precision_tier,
    PrecisionTier,
    "precision tier"
);
canonical_label_codec!(
    encode_provider_validation_status,
    decode_provider_validation_status,
    ProviderValidationStatus,
    "provider validation status"
);
canonical_label_codec!(
    encode_provider_kind,
    decode_provider_kind,
    ProviderKind,
    "provider kind"
);
canonical_label_codec!(
    encode_language_scope,
    decode_language_scope,
    LanguageScope,
    "language scope"
);
canonical_label_codec!(
    encode_precision_ceiling,
    decode_precision_ceiling,
    PrecisionCeiling,
    "precision ceiling"
);
canonical_label_codec!(
    encode_fact_family,
    decode_fact_family,
    FactFamily,
    "fact family"
);
canonical_label_codec!(
    encode_fact_precision,
    decode_fact_precision,
    FactPrecision,
    "fact precision"
);
canonical_label_codec!(
    encode_fact_confidence,
    decode_fact_confidence,
    FactConfidence,
    "fact confidence"
);
canonical_label_codec!(
    encode_validation_status,
    decode_validation_status,
    ValidationStatus,
    "validation status"
);
canonical_label_codec!(
    encode_validation_event_kind,
    decode_validation_event_kind,
    ValidationEventKind,
    "validation event kind"
);
canonical_label_codec!(
    encode_validation_event_status,
    decode_validation_event_status,
    ValidationEventStatus,
    "validation event status"
);
canonical_label_codec!(
    encode_cache_node_kind,
    decode_cache_node_kind,
    CacheNodeKind,
    "cache node kind"
);
canonical_label_codec!(
    encode_dependency_kind,
    decode_dependency_kind,
    DependencyKind,
    "dependency kind"
);
canonical_label_codec!(
    encode_shape_kind,
    decode_shape_kind,
    ShapeKind,
    "shape kind"
);

pub(super) fn encode_analysis_settings_scope(value: AnalysisSettingsScope) -> &'static str {
    value.label()
}

pub(super) fn decode_analysis_settings_scope(
    label: &str,
) -> Result<AnalysisSettingsScope, SchemaCodecError> {
    AnalysisSettingsScope::ALL
        .into_iter()
        .find(|scope| scope.label() == label)
        .ok_or_else(|| SchemaCodecError::UnknownLabel {
            vocabulary: "analysis settings scope",
            label: label.to_owned(),
        })
}

pub(super) fn encode_input_group(value: StoreInputGroup) -> Result<String, SchemaCodecError> {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or(SchemaCodecError::UnknownLabel {
            vocabulary: "input group",
            label: String::new(),
        })
}

pub(super) fn decode_input_group(label: &str) -> Result<StoreInputGroup, SchemaCodecError> {
    [
        StoreInputGroup::Config,
        StoreInputGroup::GoLifecycle,
        StoreInputGroup::TsJsLifecycle,
        StoreInputGroup::Rule,
        StoreInputGroup::Model,
        StoreInputGroup::Extension,
        StoreInputGroup::ToolInvocation,
    ]
    .into_iter()
    .find(|group| encode_input_group(*group).as_deref() == Ok(label))
    .ok_or_else(|| SchemaCodecError::UnknownLabel {
        vocabulary: "input group",
        label: label.to_owned(),
    })
}

pub(super) fn encode_cache_policy(value: CachePolicy) -> Cow<'static, str> {
    value.label()
}

pub(super) fn decode_cache_policy(label: &str) -> Result<CachePolicyView<'_>, SchemaCodecError> {
    CachePolicy::parse_label(label).map_err(|_| SchemaCodecError::UnknownLabel {
        vocabulary: "cache policy",
        label: label.to_owned(),
    })
}

pub(super) fn encode_capability_setup_status(value: CapabilitySetupStatus) -> &'static str {
    value.label()
}

pub(super) fn decode_capability_setup_status(
    label: &str,
) -> Result<CapabilitySetupStatus, SchemaCodecError> {
    [
        CapabilitySetupStatus::NotRequired,
        CapabilitySetupStatus::Ready,
        CapabilitySetupStatus::SetupMissing,
    ]
    .into_iter()
    .find(|status| status.label() == label)
    .ok_or_else(|| SchemaCodecError::UnknownLabel {
        vocabulary: "capability setup status",
        label: label.to_owned(),
    })
}

pub(super) fn encode_capability_support_status(
    value: &CapabilitySupportStatus,
) -> Result<String, SchemaCodecError> {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .ok_or(SchemaCodecError::UnknownLabel {
            vocabulary: "capability support status",
            label: String::new(),
        })
}

pub(super) fn decode_capability_support_status(
    label: &str,
) -> Result<CapabilitySupportStatus, SchemaCodecError> {
    serde_json::from_value(serde_json::Value::String(label.to_owned())).map_err(|_| {
        SchemaCodecError::UnknownLabel {
            vocabulary: "capability support status",
            label: label.to_owned(),
        }
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EncodedInputDependency {
    pub(super) kind: String,
    pub(super) stable_key: String,
    pub(super) digest_kind: String,
    pub(super) digest_value: String,
    pub(super) status: String,
}

pub(super) fn encode_input_dependency(input: &InputDependencyKey) -> EncodedInputDependency {
    EncodedInputDependency {
        kind: input.kind.label().to_owned(),
        stable_key: input.stable_key.clone(),
        digest_kind: input.digest.kind.label().to_owned(),
        digest_value: input.digest.value.clone(),
        status: input.status.label().to_owned(),
    }
}

pub(super) fn decode_input_dependency(
    encoded: &EncodedInputDependency,
) -> Result<InputDependencyKey, SchemaCodecError> {
    let kind = decode_input_dependency_kind(&encoded.kind)?;
    let status = decode_input_component_status(&encoded.status)?;
    let digest = decode_digest(&encoded.digest_kind, &encoded.digest_value, None)?;
    if encoded.stable_key.is_empty() {
        return Err(SchemaCodecError::InvalidInputDependency);
    }

    serde_json::from_value(serde_json::json!({
        "kind": kind.label(),
        "stable_key": encoded.stable_key,
        "digest": {
            "kind": digest.kind.label(),
            "value": digest.value,
        },
        "status": status.label(),
    }))
    .map_err(|_| SchemaCodecError::InvalidInputDependency)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GenerationStatus {
    Pending,
    Complete,
    Failed,
}

impl GenerationStatus {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Complete => "complete",
            Self::Failed => "failed",
        }
    }

    pub(super) fn parse_label(label: &str) -> Result<Self, SchemaCodecError> {
        match label {
            "pending" => Ok(Self::Pending),
            "complete" => Ok(Self::Complete),
            "failed" => Ok(Self::Failed),
            _ => Err(SchemaCodecError::UnknownLabel {
                vocabulary: "generation status",
                label: label.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GenerationFailureEvent {
    CommitAttemptFailed,
}

impl GenerationFailureEvent {
    pub(super) const fn label(self) -> &'static str {
        "commit_attempt_failed"
    }

    pub(super) fn parse_label(label: &str) -> Result<Self, SchemaCodecError> {
        match label {
            "commit_attempt_failed" => Ok(Self::CommitAttemptFailed),
            _ => Err(SchemaCodecError::UnknownLabel {
                vocabulary: "generation failure event",
                label: label.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(
    clippy::enum_variant_names,
    reason = "the closed persisted failure vocabulary uses complete reason-code names"
)]
pub(super) enum GenerationFailureReason {
    WriteFailed,
    PostWriteValidationFailed,
    PublicationCommitFailed,
}

impl GenerationFailureReason {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::WriteFailed => "write_failed",
            Self::PostWriteValidationFailed => "post_write_validation_failed",
            Self::PublicationCommitFailed => "publication_commit_failed",
        }
    }

    pub(super) fn parse_label(label: &str) -> Result<Self, SchemaCodecError> {
        match label {
            "write_failed" => Ok(Self::WriteFailed),
            "post_write_validation_failed" => Ok(Self::PostWriteValidationFailed),
            "publication_commit_failed" => Ok(Self::PublicationCommitFailed),
            _ => Err(SchemaCodecError::UnknownLabel {
                vocabulary: "generation failure reason",
                label: label.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GenerationFailureStage {
    Reservation,
    StoreRunInput,
    Providers,
    LayersSummariesQueries,
    FactMetadata,
    DependencyEdges,
    ValidationEvents,
    Statistics,
    Completion,
    Activation,
    TransactionCommit,
}

impl GenerationFailureStage {
    pub(super) const ALL: [Self; 11] = [
        Self::Reservation,
        Self::StoreRunInput,
        Self::Providers,
        Self::LayersSummariesQueries,
        Self::FactMetadata,
        Self::DependencyEdges,
        Self::ValidationEvents,
        Self::Statistics,
        Self::Completion,
        Self::Activation,
        Self::TransactionCommit,
    ];

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Reservation => "reservation",
            Self::StoreRunInput => "store_run_input",
            Self::Providers => "providers",
            Self::LayersSummariesQueries => "layers_summaries_queries",
            Self::FactMetadata => "fact_metadata",
            Self::DependencyEdges => "dependency_edges",
            Self::ValidationEvents => "validation_events",
            Self::Statistics => "statistics",
            Self::Completion => "completion",
            Self::Activation => "activation",
            Self::TransactionCommit => "transaction_commit",
        }
    }

    pub(super) fn parse_label(label: &str) -> Result<Self, SchemaCodecError> {
        Self::ALL
            .into_iter()
            .find(|stage| stage.label() == label)
            .ok_or_else(|| SchemaCodecError::UnknownLabel {
                vocabulary: "generation failure stage",
                label: label.to_owned(),
            })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct StoredGenerationState {
    pub(super) handle: i64,
    pub(super) workspace: WorkspaceIdentity,
    pub(super) status: GenerationStatus,
    pub(super) failure_event_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct StoreManifestState {
    pub(super) workspace: Option<WorkspaceIdentity>,
    pub(super) active_generation: Option<i64>,
    pub(super) generations: Vec<StoredGenerationState>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ManifestLifecycle {
    PristineUnbound,
    BoundRecoverable,
    BoundActive,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ManifestStateError {
    UnboundStoreHasState,
    BoundStoreHasNoGeneration,
    DuplicateGenerationHandle(i64),
    WorkspaceMismatch(i64),
    CompleteGenerationWithoutActivation(i64),
    FailureEventOnNonFailedGeneration(i64),
    MissingActiveGeneration(i64),
    ActiveGenerationNotComplete(i64),
}

pub(super) fn validate_manifest_state(
    state: &StoreManifestState,
) -> Result<ManifestLifecycle, ManifestStateError> {
    let Some(workspace) = state.workspace.as_ref() else {
        return if state.active_generation.is_none() && state.generations.is_empty() {
            Ok(ManifestLifecycle::PristineUnbound)
        } else {
            Err(ManifestStateError::UnboundStoreHasState)
        };
    };

    if state.generations.is_empty() {
        return Err(ManifestStateError::BoundStoreHasNoGeneration);
    }

    let mut handles = BTreeSet::new();
    for generation in &state.generations {
        if !handles.insert(generation.handle) {
            return Err(ManifestStateError::DuplicateGenerationHandle(
                generation.handle,
            ));
        }
        if &generation.workspace != workspace {
            return Err(ManifestStateError::WorkspaceMismatch(generation.handle));
        }
        if generation.failure_event_count > 0 && generation.status != GenerationStatus::Failed {
            return Err(ManifestStateError::FailureEventOnNonFailedGeneration(
                generation.handle,
            ));
        }
    }

    let Some(active_handle) = state.active_generation else {
        if let Some(generation) = state
            .generations
            .iter()
            .find(|generation| generation.status == GenerationStatus::Complete)
        {
            return Err(ManifestStateError::CompleteGenerationWithoutActivation(
                generation.handle,
            ));
        }
        return Ok(ManifestLifecycle::BoundRecoverable);
    };

    let active = state
        .generations
        .iter()
        .find(|generation| generation.handle == active_handle)
        .ok_or(ManifestStateError::MissingActiveGeneration(active_handle))?;
    if active.status != GenerationStatus::Complete {
        return Err(ManifestStateError::ActiveGenerationNotComplete(
            active_handle,
        ));
    }

    Ok(ManifestLifecycle::BoundActive)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(label: &str) -> WorkspaceIdentity {
        WorkspaceIdentity::from_roots([Path::new(label)])
    }

    #[test]
    fn digest_codec_uses_canonical_labels_and_rejects_wrong_kinds() {
        let digest = Digest::from_parts(DigestKind::Generation, "generation", &["one"]);
        let encoded = encode_digest(&digest);
        assert_eq!(encoded.0, DigestKind::Generation.label());
        assert_eq!(
            decode_digest(encoded.0, encoded.1, Some(DigestKind::Generation)),
            Ok(digest)
        );
        assert!(matches!(
            decode_digest("workspace", "value", Some(DigestKind::Generation)),
            Err(SchemaCodecError::WrongDigestKind { .. })
        ));
        assert!(matches!(
            decode_digest("not_a_kind", "value", None),
            Err(SchemaCodecError::UnknownLabel { .. })
        ));
    }

    #[test]
    fn relative_path_codec_rejects_absolute_and_non_normalized_paths() {
        assert_eq!(validate_relative_path("src/lib.rs"), Ok(()));
        for invalid in [
            "",
            "/src/lib.rs",
            "../src/lib.rs",
            "src/./lib.rs",
            "src\\lib.rs",
        ] {
            assert!(
                validate_relative_path(invalid).is_err(),
                "accepted {invalid}"
            );
        }
    }

    #[test]
    fn canonical_label_codecs_round_trip_every_persisted_vocabulary() {
        validate_input_snapshot_schema(INPUT_SNAPSHOT_SCHEMA_VERSION)
            .expect("canonical input snapshot schema");
        validate_dependency_schema(DEPENDENCY_INDEX_SCHEMA).expect("canonical dependency schema");
        for group in [
            StoreInputGroup::Config,
            StoreInputGroup::GoLifecycle,
            StoreInputGroup::TsJsLifecycle,
            StoreInputGroup::Rule,
            StoreInputGroup::Model,
            StoreInputGroup::Extension,
            StoreInputGroup::ToolInvocation,
        ] {
            let label = encode_input_group(group).expect("serialize input group");
            assert_eq!(decode_input_group(&label), Ok(group));
        }
        for scope in AnalysisSettingsScope::ALL {
            assert_eq!(
                decode_analysis_settings_scope(encode_analysis_settings_scope(scope)),
                Ok(scope)
            );
        }
        for value in [
            Language::Go,
            Language::TypeScript,
            Language::Tsx,
            Language::JavaScript,
            Language::Jsx,
            Language::Unknown,
        ] {
            assert_eq!(decode_language(encode_language(value)), Ok(value));
        }
        for value in [
            InputComponentStatus::Present,
            InputComponentStatus::Absent,
            InputComponentStatus::Unsupported,
            InputComponentStatus::SetupMissing,
        ] {
            assert_eq!(
                decode_input_component_status(encode_input_component_status(value)),
                Ok(value)
            );
        }
        for value in [
            InputDependencyKind::SourceFile,
            InputDependencyKind::RequestedCapability,
            InputDependencyKind::AnalysisSetting,
            InputDependencyKind::UpstreamLayer,
            InputDependencyKind::ExtensionDeclaredInput,
            InputDependencyKind::RuleOptions,
        ] {
            assert_eq!(
                decode_input_dependency_kind(encode_input_dependency_kind(value)),
                Ok(value)
            );
        }
        for value in [
            LayerKind::SourceFiles,
            LayerKind::SemanticMir,
            LayerKind::DemandQuery,
        ] {
            assert_eq!(decode_layer_kind(encode_layer_kind(value)), Ok(value));
        }
        for value in [
            PrecisionTier::Syntax,
            PrecisionTier::SetupAware,
            PrecisionTier::Exact,
        ] {
            assert_eq!(
                decode_precision_tier(encode_precision_tier(value)),
                Ok(value)
            );
        }
        for value in [
            ProviderValidationStatus::NativeTrusted,
            ProviderValidationStatus::ProviderFailed,
        ] {
            assert_eq!(
                decode_provider_validation_status(encode_provider_validation_status(value)),
                Ok(value)
            );
        }
        for value in [
            ProviderKind::SourceDiscovery,
            ProviderKind::LanguageSyntax,
            ProviderKind::WholeRepoDerived,
            ProviderKind::MetricsDerived,
        ] {
            assert_eq!(decode_provider_kind(encode_provider_kind(value)), Ok(value));
        }
        for value in [
            LanguageScope::Workspace,
            LanguageScope::Go,
            LanguageScope::TypeScriptJavaScript,
            LanguageScope::MultiLanguage,
        ] {
            assert_eq!(
                decode_language_scope(encode_language_scope(value)),
                Ok(value)
            );
        }
        for value in [
            PrecisionCeiling::Exact,
            PrecisionCeiling::Syntax,
            PrecisionCeiling::SetupAware,
        ] {
            assert_eq!(
                decode_precision_ceiling(encode_precision_ceiling(value)),
                Ok(value)
            );
        }
        for value in [
            FactFamily::SourceFile,
            FactFamily::MirBody,
            FactFamily::ExtensionFact,
        ] {
            assert_eq!(decode_fact_family(encode_fact_family(value)), Ok(value));
        }
        for value in [
            FactPrecision::Exact,
            FactPrecision::SetupAware,
            FactPrecision::Heuristic,
            FactPrecision::SetupMissing,
            FactPrecision::Unsupported,
        ] {
            assert_eq!(
                decode_fact_precision(encode_fact_precision(value)),
                Ok(value)
            );
        }
        for value in [
            FactConfidence::High,
            FactConfidence::Medium,
            FactConfidence::Low,
        ] {
            assert_eq!(
                decode_fact_confidence(encode_fact_confidence(value)),
                Ok(value)
            );
        }
        for value in [
            ValidationStatus::NativeTrusted,
            ValidationStatus::SchemaValidated,
            ValidationStatus::ReferentiallyValidated,
            ValidationStatus::ConflictRejected,
        ] {
            assert_eq!(
                decode_validation_status(encode_validation_status(value)),
                Ok(value)
            );
        }
        for value in ValidationEventKind::ALL {
            assert_eq!(
                decode_validation_event_kind(encode_validation_event_kind(value)),
                Ok(value)
            );
        }
        for value in [ValidationEventStatus::Passed, ValidationEventStatus::Failed] {
            assert_eq!(
                decode_validation_event_status(encode_validation_event_status(value)),
                Ok(value)
            );
        }
        for value in [
            CacheNodeKind::DependencyInput,
            CacheNodeKind::RunManifest,
            CacheNodeKind::Layer,
            CacheNodeKind::Query,
            CacheNodeKind::Summary,
            CacheNodeKind::Diagnostic,
        ] {
            assert_eq!(
                decode_cache_node_kind(encode_cache_node_kind(value)),
                Ok(value)
            );
        }
        for value in [
            DependencyKind::Input,
            DependencyKind::SourceText,
            DependencyKind::ProviderSchema,
            DependencyKind::ToolInvocation,
        ] {
            assert_eq!(
                decode_dependency_kind(encode_dependency_kind(value)),
                Ok(value)
            );
        }
        for value in [
            ShapeKind::Content,
            ShapeKind::PublicApi,
            ShapeKind::ExtensionDeclaredInput,
            ShapeKind::Unknown,
        ] {
            assert_eq!(decode_shape_kind(encode_shape_kind(value)), Ok(value));
        }

        for status in [
            CapabilitySetupStatus::NotRequired,
            CapabilitySetupStatus::Ready,
            CapabilitySetupStatus::SetupMissing,
        ] {
            assert_eq!(
                decode_capability_setup_status(encode_capability_setup_status(status)),
                Ok(status)
            );
        }
        for status in [
            CapabilitySupportStatus::Supported,
            CapabilitySupportStatus::Unsupported,
            CapabilitySupportStatus::SetupMissing,
        ] {
            let label = encode_capability_support_status(&status).expect("serialize status");
            assert_eq!(decode_capability_support_status(&label), Ok(status));
        }

        for policy in [CachePolicy::NoCache, CachePolicy::InMemoryDerived] {
            let label = encode_cache_policy(policy);
            assert_eq!(
                decode_cache_policy(&label).expect("decode policy").label(),
                label
            );
        }
        let label = encode_cache_policy(CachePolicy::ExistingFileFactCache {
            schema: "polint.test-1",
        });
        assert_eq!(
            decode_cache_policy(&label).expect("decode policy").label(),
            label
        );
    }

    #[test]
    fn canonical_label_codecs_reject_unknown_labels() {
        assert!(validate_input_snapshot_schema("not-a-schema").is_err());
        assert!(validate_dependency_schema("not-a-schema").is_err());
        assert!(decode_input_group("not-a-group").is_err());
        assert!(decode_analysis_settings_scope("not-settings").is_err());
        assert!(decode_language("not-a-language").is_err());
        assert!(decode_input_component_status("not-a-status").is_err());
        assert!(decode_input_dependency_kind("not-an-input").is_err());
        assert!(decode_layer_kind("not-a-layer").is_err());
        assert!(decode_precision_tier("not-precision").is_err());
        assert!(decode_provider_validation_status("not-validation").is_err());
        assert!(decode_provider_kind("not-a-provider").is_err());
        assert!(decode_language_scope("not-a-scope").is_err());
        assert!(decode_cache_policy("existing_file_fact_cache:").is_err());
        assert!(decode_precision_ceiling("not-a-ceiling").is_err());
        assert!(decode_capability_setup_status("not-setup").is_err());
        assert!(decode_capability_support_status("not-support").is_err());
        assert!(decode_fact_family("not-a-family").is_err());
        assert!(decode_fact_precision("not-fact-precision").is_err());
        assert!(decode_fact_confidence("not-confidence").is_err());
        assert!(decode_validation_status("not-validation").is_err());
        assert!(decode_validation_event_kind("not-an-event").is_err());
        assert!(decode_validation_event_status("not-event-status").is_err());
        assert!(decode_cache_node_kind("not-a-node").is_err());
        assert!(decode_dependency_kind("not-an-edge").is_err());
        assert!(decode_shape_kind("not-a-shape").is_err());
    }

    #[test]
    fn input_dependency_codec_enforces_kind_digest_compatibility() {
        let input = InputDependencyKey::source_file(
            "src/lib.rs",
            Digest::from_parts(DigestKind::SourceText, "source", &["src/lib.rs"]),
            InputComponentStatus::Present,
        )
        .expect("source dependency");
        let encoded = encode_input_dependency(&input);
        assert_eq!(decode_input_dependency(&encoded), Ok(input));

        let mut wrong_kind = encoded;
        wrong_kind.digest_kind = DigestKind::Config.label().to_owned();
        assert_eq!(
            decode_input_dependency(&wrong_kind),
            Err(SchemaCodecError::InvalidInputDependency)
        );
    }

    #[test]
    fn every_multirow_family_declares_semantic_ordering() {
        assert_eq!(SEMANTIC_ORDER_BY.len(), 33);
        for table in REQUIRED_V2_TABLES {
            if matches!(
                table,
                "store_manifest" | "input_snapshots" | "generation_stats"
            ) {
                continue;
            }
            let clause = semantic_order_by(table).unwrap_or_else(|| panic!("missing {table}"));
            assert!(clause.starts_with("ORDER BY "));
            assert!(!clause.contains("DESC"));
            assert!(!clause.contains("rowid"));
        }
    }

    #[test]
    fn closed_failure_vocabulary_round_trips_and_rejects_unknown_labels() {
        assert_eq!(
            GenerationFailureEvent::parse_label(
                GenerationFailureEvent::CommitAttemptFailed.label()
            ),
            Ok(GenerationFailureEvent::CommitAttemptFailed)
        );
        for reason in [
            GenerationFailureReason::WriteFailed,
            GenerationFailureReason::PostWriteValidationFailed,
            GenerationFailureReason::PublicationCommitFailed,
        ] {
            assert_eq!(
                GenerationFailureReason::parse_label(reason.label()),
                Ok(reason)
            );
        }
        for stage in GenerationFailureStage::ALL {
            assert_eq!(
                GenerationFailureStage::parse_label(stage.label()),
                Ok(stage)
            );
        }
        assert!(GenerationStatus::parse_label("unknown").is_err());
        assert!(GenerationFailureReason::parse_label("unknown").is_err());
        assert!(GenerationFailureStage::parse_label("unknown").is_err());
    }

    #[test]
    fn lifecycle_selection_uses_explicit_active_identity() {
        let workspace = workspace("primary");
        let complete = StoredGenerationState {
            handle: 7,
            workspace: workspace.clone(),
            status: GenerationStatus::Complete,
            failure_event_count: 0,
        };
        let pending = StoredGenerationState {
            handle: 99,
            workspace: workspace.clone(),
            status: GenerationStatus::Pending,
            failure_event_count: 0,
        };
        let state = StoreManifestState {
            workspace: Some(workspace),
            active_generation: Some(7),
            generations: vec![pending, complete],
        };
        assert_eq!(
            validate_manifest_state(&state),
            Ok(ManifestLifecycle::BoundActive)
        );
    }

    #[test]
    fn lifecycle_accepts_only_pristine_recoverable_or_identity_selected_active_states() {
        assert_eq!(
            validate_manifest_state(&StoreManifestState {
                workspace: None,
                active_generation: None,
                generations: Vec::new(),
            }),
            Ok(ManifestLifecycle::PristineUnbound)
        );

        let workspace = workspace("primary");
        assert_eq!(
            validate_manifest_state(&StoreManifestState {
                workspace: Some(workspace.clone()),
                active_generation: None,
                generations: vec![StoredGenerationState {
                    handle: 1,
                    workspace: workspace.clone(),
                    status: GenerationStatus::Failed,
                    failure_event_count: 1,
                }],
            }),
            Ok(ManifestLifecycle::BoundRecoverable)
        );

        let illegal = [
            StoreManifestState {
                workspace: None,
                active_generation: Some(1),
                generations: Vec::new(),
            },
            StoreManifestState {
                workspace: Some(workspace.clone()),
                active_generation: None,
                generations: Vec::new(),
            },
            StoreManifestState {
                workspace: Some(workspace.clone()),
                active_generation: Some(404),
                generations: vec![StoredGenerationState {
                    handle: 1,
                    workspace: workspace.clone(),
                    status: GenerationStatus::Complete,
                    failure_event_count: 0,
                }],
            },
            StoreManifestState {
                workspace: Some(workspace.clone()),
                active_generation: Some(1),
                generations: vec![StoredGenerationState {
                    handle: 1,
                    workspace,
                    status: GenerationStatus::Pending,
                    failure_event_count: 0,
                }],
            },
        ];
        for state in illegal {
            assert!(validate_manifest_state(&state).is_err());
        }
    }
}

#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "the private generation publication and active-read boundary consumes this normalized plan"
    )
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use serde::Serialize;

use crate::analysis_kernel::incremental::{
    CacheNode, ConfigIdentity, DependencyKind, DiagnosticKey, Digest, DigestKind,
    GenerationIdentity, INPUT_SNAPSHOT_SCHEMA_VERSION, InputComponent, InputComponentStatus,
    InputDependencyKey, LayerKey, PrecisionTier, ProviderValidationStatus, QueryKey, RunIdentity,
    RunManifestKey, ShapeKind, SummaryKey, ValidatedRunMetadata, WorkspaceIdentity,
};
use crate::analysis_kernel::validation::{ValidationEventKind, ValidationEventStatus};
use crate::analysis_kernel::{FactConfidence, FactFamily, FactPrecision, ValidationStatus};
use crate::analysis_plan::CapabilitySetupStatus;
use crate::core::CapabilitySupportStatus;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct StoreCommitPlan {
    pub(super) semantic: StoreSemanticPlan,
    pub(super) telemetry: Vec<StoreTelemetryRow>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum StorePlanError {
    InvalidHandoff {
        message: String,
    },
    UnsupportedInputSnapshotSchema {
        found: String,
    },
    MissingValidationEvent {
        kind: ValidationEventKind,
    },
    FailedValidationEvent {
        kind: ValidationEventKind,
        issue_count: u64,
    },
    NonCanonicalRows {
        family: &'static str,
    },
    DuplicateFact {
        family: String,
        stable_key: String,
    },
    UnknownProvider {
        provider_id: String,
        family: &'static str,
    },
    UnknownProviderSchema {
        provider_id: String,
        schema_version: String,
    },
    DanglingQueryInput {
        query_kind: String,
        stable_key: String,
    },
    DanglingQueryEndpoint {
        query_kind: String,
    },
    DanglingDependencyEndpoint {
        endpoint: Box<CacheNode>,
    },
    IdentityMismatch {
        family: &'static str,
    },
    CountMismatch {
        family: &'static str,
        expected: u64,
        actual: u64,
    },
    InvalidStatus {
        family: &'static str,
        value: String,
    },
    AbsolutePath {
        path: String,
    },
    MissingPayloadDigest {
        family: String,
        stable_key: String,
    },
    IncompleteGeneration {
        family: &'static str,
    },
}

impl fmt::Display for StorePlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHandoff { message } => {
                write!(formatter, "invalid validated-run handoff: {message}")
            }
            Self::UnsupportedInputSnapshotSchema { found } => {
                write!(formatter, "unsupported input snapshot schema `{found}`")
            }
            Self::MissingValidationEvent { kind } => {
                write!(formatter, "missing validation event `{}`", kind.label())
            }
            Self::FailedValidationEvent { kind, issue_count } => write!(
                formatter,
                "validation event `{}` failed with {issue_count} issues",
                kind.label()
            ),
            Self::NonCanonicalRows { family } => {
                write!(formatter, "{family} rows are not sorted and unique")
            }
            Self::DuplicateFact { family, stable_key } => {
                write!(formatter, "duplicate {family} fact `{stable_key}`")
            }
            Self::UnknownProvider {
                provider_id,
                family,
            } => write!(
                formatter,
                "{family} row references unknown provider `{provider_id}`"
            ),
            Self::UnknownProviderSchema {
                provider_id,
                schema_version,
            } => write!(
                formatter,
                "provider `{provider_id}` references unknown schema `{schema_version}`"
            ),
            Self::DanglingQueryInput {
                query_kind,
                stable_key,
            } => write!(
                formatter,
                "query `{query_kind}` is missing declared input `{stable_key}`"
            ),
            Self::DanglingQueryEndpoint { query_kind } => {
                write!(formatter, "query `{query_kind}` has an incomplete edge set")
            }
            Self::DanglingDependencyEndpoint { endpoint } => {
                write!(
                    formatter,
                    "dependency endpoint `{endpoint:?}` is not retained"
                )
            }
            Self::IdentityMismatch { family } => {
                write!(
                    formatter,
                    "{family} identity does not match its copied source"
                )
            }
            Self::CountMismatch {
                family,
                expected,
                actual,
            } => write!(
                formatter,
                "{family} count mismatch: expected {expected}, found {actual}"
            ),
            Self::InvalidStatus { family, value } => {
                write!(formatter, "invalid {family} status `{value}`")
            }
            Self::AbsolutePath { path } => {
                write!(
                    formatter,
                    "input file path is not repository-relative: `{path}`"
                )
            }
            Self::MissingPayloadDigest { family, stable_key } => write!(
                formatter,
                "{family} fact `{stable_key}` has no payload digest"
            ),
            Self::IncompleteGeneration { family } => {
                write!(formatter, "generation is missing required {family} rows")
            }
        }
    }
}

impl std::error::Error for StorePlanError {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct StoreSemanticPlan {
    pub(super) identities: StoreIdentityRow,
    pub(super) input_snapshot: StoreInputSnapshotRow,
    pub(super) files: Vec<StoreFileRow>,
    pub(super) input_components: Vec<StoreInputComponentRow>,
    pub(super) input_details: Vec<StoreInputDetailRow>,
    pub(super) analysis_settings: Vec<StoreAnalysisSettingRow>,
    pub(super) capabilities: Vec<StoreCapabilityRow>,
    pub(super) capability_requesters: Vec<StoreCapabilityRequesterRow>,
    pub(super) provider_schemas: Vec<StoreProviderSchemaRow>,
    pub(super) provider_schema_versions: Vec<StoreProviderSchemaVersionRow>,
    pub(super) provider_manifests: Vec<StoreProviderManifestRow>,
    pub(super) provider_manifest_schemas: Vec<StoreProviderManifestSchemaRow>,
    pub(super) provider_manifest_inputs: Vec<StoreProviderManifestInputRow>,
    pub(super) provider_manifest_outputs: Vec<StoreProviderManifestOutputRow>,
    pub(super) provider_generations: Vec<StoreProviderGenerationRow>,
    pub(super) provider_dependencies: Vec<StoreProviderDependencyRow>,
    pub(super) layers: Vec<StoreLayerRow>,
    pub(super) layer_inputs: Vec<StoreLayerInputRow>,
    pub(super) layer_dependencies: Vec<StoreLayerDependencyRow>,
    pub(super) layer_extensions: Vec<StoreLayerExtensionRow>,
    pub(super) layer_warnings: Vec<StoreLayerWarningRow>,
    pub(super) summaries: Vec<StoreSummaryRow>,
    pub(super) summary_dependencies: Vec<StoreSummaryDependencyRow>,
    pub(super) queries: Vec<StoreQueryRow>,
    pub(super) query_inputs: Vec<StoreQueryInputRow>,
    pub(super) query_layers: Vec<StoreQueryLayerRow>,
    pub(super) facts: Vec<StoreFactRow>,
    pub(super) run_manifest: StoreRunManifestRow,
    pub(super) diagnostics: Vec<StoreDiagnosticRow>,
    pub(super) diagnostic_requested_views: Vec<StoreDiagnosticRequestedViewRow>,
    pub(super) dependency_schema: String,
    pub(super) dependency_edges: Vec<StoreDependencyEdgeRow>,
    pub(super) validation_events: Vec<StoreValidationEventRow>,
    pub(super) stats: StoreGenerationStats,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct StoreIdentityRow {
    pub(super) workspace: WorkspaceIdentity,
    pub(super) full_config: ConfigIdentity,
    pub(super) input_snapshot: Digest,
    pub(super) provider_manifest: Digest,
    pub(super) provider_output: Digest,
    pub(super) layer: Digest,
    pub(super) summary: Digest,
    pub(super) query: Digest,
    pub(super) fact: Digest,
    pub(super) dependency: Digest,
    pub(super) validation: Digest,
    pub(super) run: RunIdentity,
    pub(super) generation: GenerationIdentity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct StoreInputSnapshotRow {
    pub(super) schema_version: String,
    pub(super) workspace: WorkspaceIdentity,
    pub(super) full_config: ConfigIdentity,
    pub(super) input_digest: Digest,
    pub(super) analysis_requirements_digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreFileRow {
    pub(super) relative_path: String,
    pub(super) language: String,
    pub(super) source_digest: Digest,
    pub(super) size_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum StoreInputGroup {
    Config,
    GoLifecycle,
    TsJsLifecycle,
    Rule,
    Model,
    Extension,
    ToolInvocation,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreInputComponentRow {
    pub(super) group: StoreInputGroup,
    pub(super) name: String,
    pub(super) status: String,
    pub(super) digest: Digest,
    pub(super) detail_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreInputDetailRow {
    pub(super) group: StoreInputGroup,
    pub(super) component_name: String,
    pub(super) component_digest: Digest,
    pub(super) detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreAnalysisSettingRow {
    pub(super) scope: String,
    pub(super) digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreCapabilityRow {
    pub(super) capability: String,
    pub(super) language: Option<String>,
    pub(super) support_status: String,
    pub(super) setup_status: String,
    pub(super) policy_query_version: Option<String>,
    pub(super) rule_behavior_digest: Digest,
    pub(super) analysis_dependency_digest: Digest,
    pub(super) requester_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreCapabilityRequesterRow {
    pub(super) capability: String,
    pub(super) language: Option<String>,
    pub(super) rule_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreProviderSchemaRow {
    pub(super) provider_id: String,
    pub(super) language_scope: String,
    pub(super) cache_policy: String,
    pub(super) precision_ceiling: String,
    pub(super) manifest_digest: Digest,
    pub(super) version_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreProviderSchemaVersionRow {
    pub(super) provider_id: String,
    pub(super) schema_version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreProviderManifestRow {
    pub(super) provider_id: String,
    pub(super) provider_version: String,
    pub(super) provider_kind: String,
    pub(super) language_scope: String,
    pub(super) cache_policy: String,
    pub(super) precision_ceiling: String,
    pub(super) manifest_digest: Digest,
    pub(super) schema_count: u64,
    pub(super) input_count: u64,
    pub(super) output_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreProviderManifestSchemaRow {
    pub(super) provider_id: String,
    pub(super) schema_version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreProviderManifestInputRow {
    pub(super) provider_id: String,
    pub(super) input: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreProviderManifestOutputRow {
    pub(super) provider_id: String,
    pub(super) output: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreProviderGenerationRow {
    pub(super) provider_id: String,
    pub(super) provider_version: String,
    pub(super) schema_version: String,
    pub(super) output_digest: Digest,
    pub(super) precision: String,
    pub(super) validation: String,
    pub(super) dependency_count: u64,
    pub(super) layer_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreProviderDependencyRow {
    pub(super) provider_id: String,
    pub(super) provider_version: String,
    pub(super) schema_version: String,
    pub(super) output_digest: Digest,
    pub(super) dependency: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreLayerRow {
    pub(super) key: LayerKey,
    pub(super) output_digest: Digest,
    pub(super) payload_digest: Digest,
    pub(super) precision: String,
    pub(super) validation: String,
    pub(super) input_count: u64,
    pub(super) dependency_layer_count: u64,
    pub(super) extension_count: u64,
    pub(super) edge_count: u64,
    pub(super) warning_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreLayerInputRow {
    pub(super) key: LayerKey,
    pub(super) digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreLayerDependencyRow {
    pub(super) key: LayerKey,
    pub(super) digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreLayerExtensionRow {
    pub(super) key: LayerKey,
    pub(super) digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreLayerWarningRow {
    pub(super) key: LayerKey,
    pub(super) warning_code: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreSummaryRow {
    pub(super) key: SummaryKey,
    pub(super) dependency_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreSummaryDependencyRow {
    pub(super) key: SummaryKey,
    pub(super) dependency: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreQueryRow {
    pub(super) key: QueryKey,
    pub(super) result_digest: Digest,
    pub(super) precision: String,
    pub(super) provenance: String,
    pub(super) input_count: u64,
    pub(super) layer_count: u64,
    pub(super) edge_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreQueryInputRow {
    pub(super) key: QueryKey,
    pub(super) input: InputDependencyKey,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreQueryLayerRow {
    pub(super) key: QueryKey,
    pub(super) layer_digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreFactRow {
    pub(super) family: String,
    pub(super) stable_key: String,
    pub(super) producer_id: String,
    pub(super) layer_id: String,
    pub(super) precision: String,
    pub(super) confidence: String,
    pub(super) validation: String,
    pub(super) payload_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct StoreRunManifestRow {
    pub(super) key: RunManifestKey,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreDiagnosticRow {
    pub(super) key: DiagnosticKey,
    pub(super) requested_views_digest: Digest,
    pub(super) requested_view_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreDiagnosticRequestedViewRow {
    pub(super) key: DiagnosticKey,
    pub(super) digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreDependencyEdgeRow {
    pub(super) from: CacheNode,
    pub(super) to: CacheNode,
    pub(super) kind: DependencyKind,
    pub(super) required_shape: ShapeKind,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreValidationEventRow {
    pub(super) kind: String,
    pub(super) status: String,
    pub(super) issue_count: u64,
    pub(super) digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreTelemetryRow {
    pub(super) relative_path: String,
    pub(super) file_mtime_hint_present: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct StoreGenerationStats {
    pub(super) input_file_count: u64,
    pub(super) input_component_count: u64,
    pub(super) input_detail_count: u64,
    pub(super) analysis_setting_count: u64,
    pub(super) capability_count: u64,
    pub(super) provider_schema_count: u64,
    pub(super) provider_manifest_count: u64,
    pub(super) provider_generation_count: u64,
    pub(super) layer_count: u64,
    pub(super) summary_count: u64,
    pub(super) query_count: u64,
    pub(super) fact_count: u64,
    pub(super) diagnostic_count: u64,
    pub(super) dependency_edge_count: u64,
    pub(super) validation_event_count: u64,
    pub(super) input_digest: Digest,
    pub(super) provider_manifest_digest: Digest,
    pub(super) provider_output_digest: Digest,
    pub(super) layer_digest: Digest,
    pub(super) summary_digest: Digest,
    pub(super) query_digest: Digest,
    pub(super) fact_digest: Digest,
    pub(super) dependency_digest: Digest,
    pub(super) validation_digest: Digest,
    pub(super) input_logical_bytes: u64,
    pub(super) provider_logical_bytes: u64,
    pub(super) layer_logical_bytes: u64,
    pub(super) summary_logical_bytes: u64,
    pub(super) query_logical_bytes: u64,
    pub(super) fact_logical_bytes: u64,
    pub(super) diagnostic_logical_bytes: u64,
    pub(super) dependency_logical_bytes: u64,
    pub(super) validation_logical_bytes: u64,
    pub(super) semantic_logical_bytes: u64,
}

impl StoreCommitPlan {
    pub(super) fn from_validated_run(
        validated: &ValidatedRunMetadata,
    ) -> Result<Self, StorePlanError> {
        validated
            .validate_integrity()
            .map_err(|error| StorePlanError::InvalidHandoff {
                message: error.to_string(),
            })?;

        let telemetry = telemetry_rows(validated);
        let semantic = StoreSemanticPlan::from_validated(validated);
        let plan = Self {
            semantic,
            telemetry,
        };
        plan.validate()?;
        Ok(plan)
    }

    pub(super) fn validate(&self) -> Result<(), StorePlanError> {
        self.semantic.validate()
    }
}

impl StoreSemanticPlan {
    fn from_validated(validated: &ValidatedRunMetadata) -> Self {
        let source_identities = validated.identities();
        let identities = StoreIdentityRow {
            workspace: source_identities.workspace().clone(),
            full_config: source_identities.full_config().clone(),
            input_snapshot: source_identities.input_snapshot().clone(),
            provider_manifest: source_identities.provider_manifest().clone(),
            provider_output: source_identities.provider_output().clone(),
            layer: source_identities.layer().clone(),
            summary: source_identities.summary().clone(),
            query: source_identities.query().clone(),
            fact: source_identities.fact().clone(),
            dependency: source_identities.dependency().clone(),
            validation: source_identities.validation().clone(),
            run: source_identities.run().clone(),
            generation: source_identities.generation().clone(),
        };
        let snapshot = validated.input_snapshot();
        let input_snapshot = StoreInputSnapshotRow {
            schema_version: snapshot.schema_version.clone(),
            workspace: snapshot.workspace_identity.clone(),
            full_config: snapshot.config_identity.clone(),
            input_digest: source_identities.input_snapshot().clone(),
            analysis_requirements_digest: snapshot.analysis_requirements_identity.clone(),
        };

        let mut files = snapshot
            .files
            .iter()
            .map(|file| StoreFileRow {
                relative_path: file.relative_path.clone(),
                language: file.language.label().to_string(),
                source_digest: file.source_text_digest.clone(),
                size_bytes: row_count(file.size_bytes),
            })
            .collect::<Vec<_>>();
        files.sort();

        let mut input_components = Vec::new();
        let mut input_details = Vec::new();
        append_input_components(
            StoreInputGroup::Config,
            std::slice::from_ref(&snapshot.config),
            &mut input_components,
            &mut input_details,
        );
        append_input_components(
            StoreInputGroup::GoLifecycle,
            &snapshot.go_lifecycle.components,
            &mut input_components,
            &mut input_details,
        );
        append_input_components(
            StoreInputGroup::TsJsLifecycle,
            &snapshot.ts_js_lifecycle.components,
            &mut input_components,
            &mut input_details,
        );
        append_input_components(
            StoreInputGroup::Rule,
            &snapshot.rules,
            &mut input_components,
            &mut input_details,
        );
        append_input_components(
            StoreInputGroup::Model,
            &snapshot.models,
            &mut input_components,
            &mut input_details,
        );
        append_input_components(
            StoreInputGroup::Extension,
            &snapshot.extensions,
            &mut input_components,
            &mut input_details,
        );
        append_input_components(
            StoreInputGroup::ToolInvocation,
            &snapshot.tool_invocations,
            &mut input_components,
            &mut input_details,
        );
        input_components.sort();
        input_details.sort();

        let mut analysis_settings = snapshot
            .analysis_settings
            .iter()
            .map(|setting| StoreAnalysisSettingRow {
                scope: setting.scope.label().to_string(),
                digest: setting.digest.clone(),
            })
            .collect::<Vec<_>>();
        analysis_settings.sort();

        let mut capabilities = Vec::with_capacity(snapshot.requested_capabilities.len());
        let mut capability_requesters = Vec::new();
        for capability in &snapshot.requested_capabilities {
            let language = capability
                .language
                .map(|language| language.label().to_string());
            capabilities.push(StoreCapabilityRow {
                capability: capability.capability.clone(),
                language: language.clone(),
                support_status: serialized_enum_label(&capability.support_status),
                setup_status: capability.setup_status.label().to_string(),
                policy_query_version: capability.policy_query_version.clone(),
                rule_behavior_digest: capability.rule_behavior_digest.clone(),
                analysis_dependency_digest: capability.analysis_dependency_digest.clone(),
                requester_count: row_count(capability.requesting_rule_ids.len()),
            });
            capability_requesters.extend(capability.requesting_rule_ids.iter().map(|rule_id| {
                StoreCapabilityRequesterRow {
                    capability: capability.capability.clone(),
                    language: language.clone(),
                    rule_id: rule_id.clone(),
                }
            }));
        }
        capabilities.sort();
        capability_requesters.sort();

        let mut provider_schemas = Vec::with_capacity(snapshot.provider_schemas.len());
        let mut provider_schema_versions = Vec::new();
        for schema in &snapshot.provider_schemas {
            provider_schemas.push(StoreProviderSchemaRow {
                provider_id: schema.provider_id.clone(),
                language_scope: schema.language_scope.clone(),
                cache_policy: schema.cache_policy.clone(),
                precision_ceiling: schema.precision_ceiling.clone(),
                manifest_digest: schema.provider_manifest_digest.clone(),
                version_count: row_count(schema.schema_versions.len()),
            });
            provider_schema_versions.extend(schema.schema_versions.iter().map(|version| {
                StoreProviderSchemaVersionRow {
                    provider_id: schema.provider_id.clone(),
                    schema_version: version.clone(),
                }
            }));
        }
        provider_schemas.sort();
        provider_schema_versions.sort();

        let mut provider_manifests = Vec::with_capacity(validated.provider_manifests().len());
        let mut provider_manifest_schemas = Vec::new();
        let mut provider_manifest_inputs = Vec::new();
        let mut provider_manifest_outputs = Vec::new();
        for manifest in validated.provider_manifests() {
            provider_manifests.push(StoreProviderManifestRow {
                provider_id: manifest.provider_id().to_string(),
                provider_version: manifest.provider_version().to_string(),
                provider_kind: manifest.provider_kind().to_string(),
                language_scope: manifest.language_scope().to_string(),
                cache_policy: manifest.cache_policy().to_string(),
                precision_ceiling: manifest.precision_ceiling().to_string(),
                manifest_digest: manifest.manifest_digest().clone(),
                schema_count: row_count(manifest.schema_versions().len()),
                input_count: row_count(manifest.inputs().len()),
                output_count: row_count(manifest.outputs().len()),
            });
            provider_manifest_schemas.extend(manifest.schema_versions().iter().map(|version| {
                StoreProviderManifestSchemaRow {
                    provider_id: manifest.provider_id().to_string(),
                    schema_version: version.clone(),
                }
            }));
            provider_manifest_inputs.extend(manifest.inputs().iter().map(|input| {
                StoreProviderManifestInputRow {
                    provider_id: manifest.provider_id().to_string(),
                    input: input.clone(),
                }
            }));
            provider_manifest_outputs.extend(manifest.outputs().iter().map(|output| {
                StoreProviderManifestOutputRow {
                    provider_id: manifest.provider_id().to_string(),
                    output: output.clone(),
                }
            }));
        }
        provider_manifests.sort();
        provider_manifest_schemas.sort();
        provider_manifest_inputs.sort();
        provider_manifest_outputs.sort();

        let mut provider_generations = Vec::with_capacity(validated.provider_outputs().len());
        let mut provider_dependencies = Vec::new();
        let mut layers = Vec::new();
        let mut layer_inputs = Vec::new();
        let mut layer_dependencies = Vec::new();
        let mut layer_extensions = Vec::new();
        let mut layer_warnings = Vec::new();
        for provider in validated.provider_outputs() {
            provider_generations.push(StoreProviderGenerationRow {
                provider_id: provider.provider_id().to_string(),
                provider_version: provider.provider_version().to_string(),
                schema_version: provider.schema_version().to_string(),
                output_digest: provider.output_digest().clone(),
                precision: provider.precision().label().to_string(),
                validation: provider.validation().label().to_string(),
                dependency_count: row_count(provider.dependency_inputs().len()),
                layer_count: row_count(provider.layers().len()),
            });
            provider_dependencies.extend(provider.dependency_inputs().iter().map(|dependency| {
                StoreProviderDependencyRow {
                    provider_id: provider.provider_id().to_string(),
                    provider_version: provider.provider_version().to_string(),
                    schema_version: provider.schema_version().to_string(),
                    output_digest: provider.output_digest().clone(),
                    dependency: dependency.clone(),
                }
            }));
            for layer in provider.layers() {
                layers.push(StoreLayerRow {
                    key: layer.key.clone(),
                    output_digest: layer.output_digest.clone(),
                    payload_digest: layer.payload_digest.clone(),
                    precision: layer.precision.label().to_string(),
                    validation: layer.validation.label().to_string(),
                    input_count: row_count(layer.key.input_digests.len()),
                    dependency_layer_count: row_count(layer.key.dependency_layer_digests.len()),
                    extension_count: row_count(layer.key.extension_digests.len()),
                    edge_count: row_count(layer.dependencies.len()),
                    warning_count: row_count(layer.warning_codes.len()),
                });
                layer_inputs.extend(layer.key.input_digests.iter().map(|digest| {
                    StoreLayerInputRow {
                        key: layer.key.clone(),
                        digest: digest.clone(),
                    }
                }));
                layer_dependencies.extend(layer.key.dependency_layer_digests.iter().map(
                    |digest| StoreLayerDependencyRow {
                        key: layer.key.clone(),
                        digest: digest.clone(),
                    },
                ));
                layer_extensions.extend(layer.key.extension_digests.iter().map(|digest| {
                    StoreLayerExtensionRow {
                        key: layer.key.clone(),
                        digest: digest.clone(),
                    }
                }));
                layer_warnings.extend(layer.warning_codes.iter().map(|warning_code| {
                    StoreLayerWarningRow {
                        key: layer.key.clone(),
                        warning_code: warning_code.clone(),
                    }
                }));
            }
        }
        provider_generations.sort();
        provider_dependencies.sort();
        layers.sort();
        layer_inputs.sort();
        layer_dependencies.sort();
        layer_extensions.sort();
        layer_warnings.sort();

        let mut summaries = Vec::with_capacity(validated.summary_keys().len());
        let mut summary_dependencies = Vec::new();
        for key in validated.summary_keys() {
            summaries.push(StoreSummaryRow {
                key: key.clone(),
                dependency_count: row_count(key.dependency_summary_digests.len()),
            });
            summary_dependencies.extend(key.dependency_summary_digests.iter().map(|dependency| {
                StoreSummaryDependencyRow {
                    key: key.clone(),
                    dependency: dependency.clone(),
                }
            }));
        }
        summaries.sort();
        summary_dependencies.sort();

        let mut queries = Vec::with_capacity(validated.query_rows().len());
        let mut query_inputs = Vec::new();
        let mut query_layers = Vec::new();
        for query in validated.query_rows() {
            let key = query.query_key();
            queries.push(StoreQueryRow {
                key: key.clone(),
                result_digest: query.result_digest().clone(),
                precision: query.precision().label().to_string(),
                provenance: query.provenance().to_string(),
                input_count: row_count(key.dependency_inputs.as_slice().len()),
                layer_count: row_count(key.layer_digests.len()),
                edge_count: row_count(
                    validated
                        .dependency_index()
                        .canonical_edges()
                        .iter()
                        .filter(|edge| edge.from == CacheNode::Query(key.clone()))
                        .count(),
                ),
            });
            query_inputs.extend(key.dependency_inputs.as_slice().iter().map(|input| {
                StoreQueryInputRow {
                    key: key.clone(),
                    input: input.clone(),
                }
            }));
            query_layers.extend(
                key.layer_digests
                    .iter()
                    .map(|layer_digest| StoreQueryLayerRow {
                        key: key.clone(),
                        layer_digest: layer_digest.clone(),
                    }),
            );
        }
        queries.sort();
        query_inputs.sort();
        query_layers.sort();

        let mut facts = validated
            .fact_rows()
            .iter()
            .map(|fact| StoreFactRow {
                family: fact.family.label().to_string(),
                stable_key: fact.stable_key.clone(),
                producer_id: fact.producer_id.clone(),
                layer_id: fact.layer_id.clone(),
                precision: fact.precision.label().to_string(),
                confidence: fact.confidence.label().to_string(),
                validation: fact.validation.label().to_string(),
                payload_digest: fact.payload_digest.clone(),
            })
            .collect::<Vec<_>>();
        facts.sort();

        let run_manifest = StoreRunManifestRow {
            key: validated.run_manifest_key().clone(),
        };
        let mut diagnostics = Vec::with_capacity(validated.diagnostic_keys().len());
        let mut diagnostic_requested_views = Vec::new();
        for key in validated.diagnostic_keys() {
            diagnostics.push(StoreDiagnosticRow {
                key: key.clone(),
                requested_views_digest: key.requested_views_digest(),
                requested_view_count: row_count(key.requested_view_digests.len()),
            });
            diagnostic_requested_views.extend(key.requested_view_digests.iter().map(|digest| {
                StoreDiagnosticRequestedViewRow {
                    key: key.clone(),
                    digest: digest.clone(),
                }
            }));
        }
        diagnostics.sort();
        diagnostic_requested_views.sort();

        let mut dependency_edges = validated
            .dependency_index()
            .canonical_edges()
            .iter()
            .map(|edge| StoreDependencyEdgeRow {
                from: edge.from.clone(),
                to: edge.to.clone(),
                kind: edge.kind,
                required_shape: edge.required_shape,
            })
            .collect::<Vec<_>>();
        dependency_edges.sort();

        let mut validation_events = validated
            .validation_events()
            .iter()
            .map(|event| StoreValidationEventRow {
                kind: event.kind.label().to_string(),
                status: event.status.label().to_string(),
                issue_count: event.issue_count,
                digest: event.digest.clone(),
            })
            .collect::<Vec<_>>();
        validation_events.sort();

        let stats = StoreGenerationStats::empty(&identities);
        let mut plan = Self {
            identities,
            input_snapshot,
            files,
            input_components,
            input_details,
            analysis_settings,
            capabilities,
            capability_requesters,
            provider_schemas,
            provider_schema_versions,
            provider_manifests,
            provider_manifest_schemas,
            provider_manifest_inputs,
            provider_manifest_outputs,
            provider_generations,
            provider_dependencies,
            layers,
            layer_inputs,
            layer_dependencies,
            layer_extensions,
            layer_warnings,
            summaries,
            summary_dependencies,
            queries,
            query_inputs,
            query_layers,
            facts,
            run_manifest,
            diagnostics,
            diagnostic_requested_views,
            dependency_schema: validated.dependency_index().schema_version.clone(),
            dependency_edges,
            validation_events,
            stats,
        };
        plan.stats = StoreGenerationStats::from_plan(&plan);
        plan
    }

    fn validate(&self) -> Result<(), StorePlanError> {
        self.validate_schemas()?;
        self.validate_identity_copies()?;
        self.validate_paths()?;
        self.validate_statuses()?;
        self.validate_required_events()?;
        self.validate_provider_relationships()?;
        self.validate_query_declarations()?;
        self.validate_result_boundaries()?;
        self.validate_dependency_endpoints()?;
        self.validate_facts()?;
        self.validate_canonical_order()?;
        self.validate_stats()
    }

    fn validate_schemas(&self) -> Result<(), StorePlanError> {
        if self.input_snapshot.schema_version != INPUT_SNAPSHOT_SCHEMA_VERSION {
            return Err(StorePlanError::UnsupportedInputSnapshotSchema {
                found: self.input_snapshot.schema_version.clone(),
            });
        }
        Ok(())
    }

    fn validate_identity_copies(&self) -> Result<(), StorePlanError> {
        let checks = [
            (
                self.identities.workspace.digest(),
                DigestKind::Workspace,
                "workspace",
            ),
            (
                self.identities.full_config.digest(),
                DigestKind::Config,
                "full config",
            ),
            (
                &self.identities.input_snapshot,
                DigestKind::InputSnapshot,
                "input snapshot",
            ),
            (
                &self.identities.provider_manifest,
                DigestKind::ProviderManifest,
                "provider manifest",
            ),
            (
                &self.identities.provider_output,
                DigestKind::ProviderOutput,
                "provider output",
            ),
            (&self.identities.layer, DigestKind::Layer, "layer"),
            (&self.identities.summary, DigestKind::Summary, "summary"),
            (&self.identities.query, DigestKind::Query, "query"),
            (&self.identities.fact, DigestKind::FactMetadata, "fact"),
            (
                &self.identities.dependency,
                DigestKind::Dependency,
                "dependency",
            ),
            (
                &self.identities.validation,
                DigestKind::ValidationEvent,
                "validation",
            ),
            (self.identities.run.digest(), DigestKind::Run, "run"),
            (
                self.identities.generation.digest(),
                DigestKind::Generation,
                "generation",
            ),
        ];
        for (digest, expected, family) in checks {
            if digest.kind != expected {
                return Err(StorePlanError::IdentityMismatch { family });
            }
        }
        if self.input_snapshot.workspace != self.identities.workspace
            || self.input_snapshot.full_config != self.identities.full_config
            || self.input_snapshot.input_digest != self.identities.input_snapshot
        {
            return Err(StorePlanError::IdentityMismatch {
                family: "input snapshot",
            });
        }
        Ok(())
    }

    fn validate_paths(&self) -> Result<(), StorePlanError> {
        for file in &self.files {
            if Path::new(&file.relative_path).is_absolute()
                || file
                    .relative_path
                    .as_bytes()
                    .get(1)
                    .is_some_and(|byte| *byte == b':')
            {
                return Err(StorePlanError::AbsolutePath {
                    path: file.relative_path.clone(),
                });
            }
        }
        Ok(())
    }

    fn validate_statuses(&self) -> Result<(), StorePlanError> {
        for component in &self.input_components {
            InputComponentStatus::parse_label(&component.status).map_err(|_| {
                StorePlanError::InvalidStatus {
                    family: "input component",
                    value: component.status.clone(),
                }
            })?;
        }
        for capability in &self.capabilities {
            serde_json::from_value::<CapabilitySupportStatus>(serde_json::Value::String(
                capability.support_status.clone(),
            ))
            .map_err(|_| StorePlanError::InvalidStatus {
                family: "capability support",
                value: capability.support_status.clone(),
            })?;
            let valid_setup = [
                CapabilitySetupStatus::NotRequired,
                CapabilitySetupStatus::Ready,
                CapabilitySetupStatus::SetupMissing,
            ]
            .into_iter()
            .any(|status| status.label() == capability.setup_status);
            if !valid_setup {
                return Err(StorePlanError::InvalidStatus {
                    family: "capability setup",
                    value: capability.setup_status.clone(),
                });
            }
        }
        for provider in &self.provider_generations {
            PrecisionTier::parse_label(&provider.precision).map_err(|_| {
                StorePlanError::InvalidStatus {
                    family: "provider precision",
                    value: provider.precision.clone(),
                }
            })?;
            ProviderValidationStatus::parse_label(&provider.validation).map_err(|_| {
                StorePlanError::InvalidStatus {
                    family: "provider validation",
                    value: provider.validation.clone(),
                }
            })?;
        }
        for layer in &self.layers {
            PrecisionTier::parse_label(&layer.precision).map_err(|_| {
                StorePlanError::InvalidStatus {
                    family: "layer precision",
                    value: layer.precision.clone(),
                }
            })?;
            ProviderValidationStatus::parse_label(&layer.validation).map_err(|_| {
                StorePlanError::InvalidStatus {
                    family: "layer validation",
                    value: layer.validation.clone(),
                }
            })?;
        }
        for query in &self.queries {
            PrecisionTier::parse_label(&query.precision).map_err(|_| {
                StorePlanError::InvalidStatus {
                    family: "query precision",
                    value: query.precision.clone(),
                }
            })?;
            if query.precision != query.key.precision_tier.label() {
                return Err(StorePlanError::IdentityMismatch { family: "query" });
            }
        }
        for fact in &self.facts {
            FactFamily::parse_label(&fact.family).map_err(|_| StorePlanError::InvalidStatus {
                family: "fact family",
                value: fact.family.clone(),
            })?;
            FactPrecision::parse_label(&fact.precision).map_err(|_| {
                StorePlanError::InvalidStatus {
                    family: "fact precision",
                    value: fact.precision.clone(),
                }
            })?;
            FactConfidence::parse_label(&fact.confidence).map_err(|_| {
                StorePlanError::InvalidStatus {
                    family: "fact confidence",
                    value: fact.confidence.clone(),
                }
            })?;
            ValidationStatus::parse_label(&fact.validation).map_err(|_| {
                StorePlanError::InvalidStatus {
                    family: "fact validation",
                    value: fact.validation.clone(),
                }
            })?;
        }
        for event in &self.validation_events {
            ValidationEventKind::parse_label(&event.kind).map_err(|_| {
                StorePlanError::InvalidStatus {
                    family: "validation event kind",
                    value: event.kind.clone(),
                }
            })?;
            ValidationEventStatus::parse_label(&event.status).map_err(|_| {
                StorePlanError::InvalidStatus {
                    family: "validation event status",
                    value: event.status.clone(),
                }
            })?;
        }
        Ok(())
    }

    fn validate_required_events(&self) -> Result<(), StorePlanError> {
        let mut by_kind = BTreeMap::new();
        for event in &self.validation_events {
            let kind = ValidationEventKind::parse_label(&event.kind).map_err(|_| {
                StorePlanError::InvalidStatus {
                    family: "validation event kind",
                    value: event.kind.clone(),
                }
            })?;
            if by_kind.insert(kind, event).is_some() {
                return Err(StorePlanError::NonCanonicalRows {
                    family: "validation event",
                });
            }
        }
        for kind in ValidationEventKind::ALL {
            let Some(event) = by_kind.get(&kind) else {
                return Err(StorePlanError::MissingValidationEvent { kind });
            };
            if event.status != ValidationEventStatus::Passed.label() || event.issue_count != 0 {
                return Err(StorePlanError::FailedValidationEvent {
                    kind,
                    issue_count: event.issue_count,
                });
            }
            if event.digest.kind != DigestKind::ValidationEvent {
                return Err(StorePlanError::IdentityMismatch {
                    family: "validation event",
                });
            }
        }
        Ok(())
    }

    fn validate_provider_relationships(&self) -> Result<(), StorePlanError> {
        let manifests = unique_by_provider(
            &self.provider_manifests,
            |row| row.provider_id.as_str(),
            "provider manifest",
        )?;
        let schemas = unique_by_provider(
            &self.provider_schemas,
            |row| row.provider_id.as_str(),
            "provider schema",
        )?;
        let generations = unique_by_provider(
            &self.provider_generations,
            |row| row.provider_id.as_str(),
            "provider generation",
        )?;

        for provider_id in generations.keys() {
            if !manifests.contains_key(provider_id) {
                return Err(StorePlanError::UnknownProvider {
                    provider_id: (*provider_id).to_string(),
                    family: "provider generation",
                });
            }
        }
        for provider_id in schemas.keys() {
            if !manifests.contains_key(provider_id) {
                return Err(StorePlanError::UnknownProvider {
                    provider_id: (*provider_id).to_string(),
                    family: "provider schema",
                });
            }
        }
        if manifests.len() != schemas.len() || manifests.len() != generations.len() {
            return Err(StorePlanError::IncompleteGeneration { family: "provider" });
        }

        for (provider_id, manifest) in &manifests {
            let schema = schemas
                .get(provider_id)
                .expect("provider cardinality and keys were checked");
            let generation = generations
                .get(provider_id)
                .expect("provider cardinality and keys were checked");
            if schema.manifest_digest != manifest.manifest_digest {
                return Err(StorePlanError::IdentityMismatch {
                    family: "provider manifest",
                });
            }
            let manifest_versions = child_values(
                &self.provider_manifest_schemas,
                provider_id,
                |row| row.provider_id.as_str(),
                |row| row.schema_version.as_str(),
            );
            let snapshot_versions = child_values(
                &self.provider_schema_versions,
                provider_id,
                |row| row.provider_id.as_str(),
                |row| row.schema_version.as_str(),
            );
            if manifest_versions != snapshot_versions
                || row_count(manifest_versions.len()) != manifest.schema_count
                || row_count(snapshot_versions.len()) != schema.version_count
            {
                return Err(StorePlanError::UnknownProviderSchema {
                    provider_id: (*provider_id).to_string(),
                    schema_version: generation.schema_version.clone(),
                });
            }
            if generation.provider_version != manifest.provider_version
                || generation.schema_version != manifest_versions.join(",")
            {
                return Err(StorePlanError::UnknownProviderSchema {
                    provider_id: (*provider_id).to_string(),
                    schema_version: generation.schema_version.clone(),
                });
            }
            let manifest_input_count = self
                .provider_manifest_inputs
                .iter()
                .filter(|row| row.provider_id == **provider_id)
                .count();
            let manifest_output_count = self
                .provider_manifest_outputs
                .iter()
                .filter(|row| row.provider_id == **provider_id)
                .count();
            let provider_dependency_count = self
                .provider_dependencies
                .iter()
                .filter(|row| row.provider_id == **provider_id)
                .count();
            let provider_layer_count = self
                .layers
                .iter()
                .filter(|row| row.key.provider_id == **provider_id)
                .count();
            check_count(
                "provider manifest input",
                manifest.input_count,
                manifest_input_count,
            )?;
            check_count(
                "provider manifest output",
                manifest.output_count,
                manifest_output_count,
            )?;
            check_count(
                "provider dependency",
                generation.dependency_count,
                provider_dependency_count,
            )?;
            check_count(
                "provider layer",
                generation.layer_count,
                provider_layer_count,
            )?;
        }

        for layer in &self.layers {
            let Some(provider) = generations.get(layer.key.provider_id.as_str()) else {
                return Err(StorePlanError::UnknownProvider {
                    provider_id: layer.key.provider_id.clone(),
                    family: "layer",
                });
            };
            let schema_versions = child_values(
                &self.provider_manifest_schemas,
                &layer.key.provider_id,
                |row| row.provider_id.as_str(),
                |row| row.schema_version.as_str(),
            );
            let schema_is_declared = schema_versions.iter().any(|schema| {
                *schema == layer.key.schema_version
                    || schema
                        .split_once(':')
                        .is_some_and(|(name, _)| name == layer.key.schema_version)
            });
            if layer.key.provider_version != provider.provider_version || !schema_is_declared {
                return Err(StorePlanError::UnknownProviderSchema {
                    provider_id: layer.key.provider_id.clone(),
                    schema_version: layer.key.schema_version.clone(),
                });
            }
            check_count(
                "layer input",
                layer.input_count,
                self.layer_inputs
                    .iter()
                    .filter(|row| row.key == layer.key)
                    .count(),
            )?;
            check_count(
                "layer dependency",
                layer.dependency_layer_count,
                self.layer_dependencies
                    .iter()
                    .filter(|row| row.key == layer.key)
                    .count(),
            )?;
            check_count(
                "layer extension",
                layer.extension_count,
                self.layer_extensions
                    .iter()
                    .filter(|row| row.key == layer.key)
                    .count(),
            )?;
            check_count(
                "layer warning",
                layer.warning_count,
                self.layer_warnings
                    .iter()
                    .filter(|row| row.key == layer.key)
                    .count(),
            )?;
        }
        Ok(())
    }

    fn validate_query_declarations(&self) -> Result<(), StorePlanError> {
        let mut expected_inputs = Vec::new();
        let mut expected_layers = Vec::new();
        for query in &self.queries {
            expected_inputs.extend(query.key.dependency_inputs.as_slice().iter().map(|input| {
                StoreQueryInputRow {
                    key: query.key.clone(),
                    input: input.clone(),
                }
            }));
            expected_layers.extend(query.key.layer_digests.iter().map(|digest| {
                StoreQueryLayerRow {
                    key: query.key.clone(),
                    layer_digest: digest.clone(),
                }
            }));
            check_count(
                "query input",
                query.input_count,
                query.key.dependency_inputs.as_slice().len(),
            )?;
            check_count(
                "query layer",
                query.layer_count,
                query.key.layer_digests.len(),
            )?;
            let query_edges = self
                .dependency_edges
                .iter()
                .filter(|edge| edge.from == CacheNode::Query(query.key.clone()))
                .collect::<Vec<_>>();
            if query.edge_count != row_count(query_edges.len()) {
                return Err(StorePlanError::DanglingQueryEndpoint {
                    query_kind: query.key.query_kind.clone(),
                });
            }
            for input in query.key.dependency_inputs.as_slice() {
                if !query_edges
                    .iter()
                    .any(|edge| edge.to == CacheNode::DependencyInput(input.clone()))
                {
                    return Err(StorePlanError::DanglingQueryEndpoint {
                        query_kind: query.key.query_kind.clone(),
                    });
                }
            }
        }
        expected_inputs.sort();
        expected_layers.sort();
        if expected_inputs != self.query_inputs {
            let query = self
                .queries
                .first()
                .ok_or(StorePlanError::IncompleteGeneration { family: "query" })?;
            let stable_key = query
                .key
                .dependency_inputs
                .as_slice()
                .first()
                .map_or_else(|| "<none>".to_string(), |input| input.stable_key.clone());
            return Err(StorePlanError::DanglingQueryInput {
                query_kind: query.key.query_kind.clone(),
                stable_key,
            });
        }
        if expected_layers != self.query_layers {
            let query = self
                .queries
                .first()
                .ok_or(StorePlanError::IncompleteGeneration { family: "query" })?;
            return Err(StorePlanError::DanglingQueryEndpoint {
                query_kind: query.key.query_kind.clone(),
            });
        }
        Ok(())
    }

    fn validate_dependency_endpoints(&self) -> Result<(), StorePlanError> {
        let layers = self
            .layers
            .iter()
            .map(|row| row.key.clone())
            .collect::<BTreeSet<_>>();
        let queries = self
            .queries
            .iter()
            .map(|row| row.key.clone())
            .collect::<BTreeSet<_>>();
        let summaries = self
            .summaries
            .iter()
            .map(|row| row.key.clone())
            .collect::<BTreeSet<_>>();
        let diagnostics = self
            .diagnostics
            .iter()
            .map(|row| row.key.clone())
            .collect::<BTreeSet<_>>();
        for edge in &self.dependency_edges {
            for endpoint in [&edge.from, &edge.to] {
                let retained = match endpoint {
                    CacheNode::DependencyInput(_) => true,
                    CacheNode::RunManifest(key) => key == &self.run_manifest.key,
                    CacheNode::Layer(key) => layers.contains(key),
                    CacheNode::Query(key) => queries.contains(key),
                    CacheNode::Summary(key) => summaries.contains(key),
                    CacheNode::Diagnostic(key) => diagnostics.contains(key),
                };
                if !retained {
                    return Err(StorePlanError::DanglingDependencyEndpoint {
                        endpoint: Box::new(endpoint.clone()),
                    });
                }
            }
        }
        for layer in &self.layers {
            check_count(
                "layer edge",
                layer.edge_count,
                self.dependency_edges
                    .iter()
                    .filter(|edge| edge.from == CacheNode::Layer(layer.key.clone()))
                    .count(),
            )?;
        }
        Ok(())
    }

    fn validate_result_boundaries(&self) -> Result<(), StorePlanError> {
        if self.run_manifest.key.run != self.identities.run
            || self.run_manifest.key.full_config != self.identities.full_config
        {
            return Err(StorePlanError::IdentityMismatch {
                family: "run manifest",
            });
        }
        let mut expected_requested_views = Vec::new();
        for diagnostic in &self.diagnostics {
            if diagnostic.requested_views_digest != diagnostic.key.requested_views_digest() {
                return Err(StorePlanError::IdentityMismatch {
                    family: "diagnostic requested views",
                });
            }
            check_count(
                "diagnostic requested view",
                diagnostic.requested_view_count,
                diagnostic.key.requested_view_digests.len(),
            )?;
            expected_requested_views.extend(diagnostic.key.requested_view_digests.iter().map(
                |digest| StoreDiagnosticRequestedViewRow {
                    key: diagnostic.key.clone(),
                    digest: digest.clone(),
                },
            ));
        }
        expected_requested_views.sort();
        if expected_requested_views != self.diagnostic_requested_views {
            return Err(StorePlanError::NonCanonicalRows {
                family: "diagnostic requested view",
            });
        }
        Ok(())
    }

    fn validate_facts(&self) -> Result<(), StorePlanError> {
        let provider_ids = self
            .provider_manifests
            .iter()
            .map(|row| row.provider_id.as_str())
            .collect::<BTreeSet<_>>();
        for fact in &self.facts {
            if fact.payload_digest.is_empty() {
                return Err(StorePlanError::MissingPayloadDigest {
                    family: fact.family.clone(),
                    stable_key: fact.stable_key.clone(),
                });
            }
            if !provider_ids.contains(fact.producer_id.as_str()) {
                return Err(StorePlanError::UnknownProvider {
                    provider_id: fact.producer_id.clone(),
                    family: "fact producer",
                });
            }
            if !provider_ids.contains(fact.layer_id.as_str()) {
                return Err(StorePlanError::UnknownProvider {
                    provider_id: fact.layer_id.clone(),
                    family: "fact layer",
                });
            }
        }
        for pair in self.facts.windows(2) {
            if pair[0].family == pair[1].family && pair[0].stable_key == pair[1].stable_key {
                return Err(StorePlanError::DuplicateFact {
                    family: pair[0].family.clone(),
                    stable_key: pair[0].stable_key.clone(),
                });
            }
        }
        Ok(())
    }

    fn validate_canonical_order(&self) -> Result<(), StorePlanError> {
        require_strictly_sorted(&self.files, "file")?;
        require_strictly_sorted(&self.input_components, "input component")?;
        require_strictly_sorted(&self.input_details, "input detail")?;
        require_strictly_sorted(&self.analysis_settings, "analysis setting")?;
        require_strictly_sorted(&self.capabilities, "capability")?;
        require_strictly_sorted(&self.capability_requesters, "capability requester")?;
        require_strictly_sorted(&self.provider_schemas, "provider schema")?;
        require_strictly_sorted(&self.provider_schema_versions, "provider schema version")?;
        require_strictly_sorted(&self.provider_manifests, "provider manifest")?;
        require_strictly_sorted(&self.provider_manifest_schemas, "provider manifest schema")?;
        require_strictly_sorted(&self.provider_manifest_inputs, "provider manifest input")?;
        require_strictly_sorted(&self.provider_manifest_outputs, "provider manifest output")?;
        require_strictly_sorted(&self.provider_generations, "provider generation")?;
        require_strictly_sorted(&self.provider_dependencies, "provider dependency")?;
        require_strictly_sorted(&self.layers, "layer")?;
        require_strictly_sorted(&self.layer_inputs, "layer input")?;
        require_strictly_sorted(&self.layer_dependencies, "layer dependency")?;
        require_strictly_sorted(&self.layer_extensions, "layer extension")?;
        require_strictly_sorted(&self.layer_warnings, "layer warning")?;
        require_strictly_sorted(&self.summaries, "summary")?;
        require_strictly_sorted(&self.summary_dependencies, "summary dependency")?;
        require_strictly_sorted(&self.queries, "query")?;
        require_strictly_sorted(&self.query_inputs, "query input")?;
        require_strictly_sorted(&self.query_layers, "query layer")?;
        require_strictly_sorted(&self.facts, "fact")?;
        require_strictly_sorted(&self.diagnostics, "diagnostic")?;
        require_strictly_sorted(
            &self.diagnostic_requested_views,
            "diagnostic requested view",
        )?;
        require_strictly_sorted(&self.dependency_edges, "dependency edge")?;
        require_strictly_sorted(&self.validation_events, "validation event")
    }

    fn validate_stats(&self) -> Result<(), StorePlanError> {
        let expected = StoreGenerationStats::from_plan(self);
        for (family, expected_count, actual_count) in [
            (
                "input file",
                expected.input_file_count,
                self.stats.input_file_count,
            ),
            (
                "input component",
                expected.input_component_count,
                self.stats.input_component_count,
            ),
            (
                "input detail",
                expected.input_detail_count,
                self.stats.input_detail_count,
            ),
            (
                "analysis setting",
                expected.analysis_setting_count,
                self.stats.analysis_setting_count,
            ),
            (
                "capability",
                expected.capability_count,
                self.stats.capability_count,
            ),
            (
                "provider schema",
                expected.provider_schema_count,
                self.stats.provider_schema_count,
            ),
            (
                "provider manifest",
                expected.provider_manifest_count,
                self.stats.provider_manifest_count,
            ),
            (
                "provider generation",
                expected.provider_generation_count,
                self.stats.provider_generation_count,
            ),
            ("layer", expected.layer_count, self.stats.layer_count),
            ("summary", expected.summary_count, self.stats.summary_count),
            ("query", expected.query_count, self.stats.query_count),
            ("fact", expected.fact_count, self.stats.fact_count),
            (
                "diagnostic",
                expected.diagnostic_count,
                self.stats.diagnostic_count,
            ),
            (
                "dependency edge",
                expected.dependency_edge_count,
                self.stats.dependency_edge_count,
            ),
            (
                "validation event",
                expected.validation_event_count,
                self.stats.validation_event_count,
            ),
        ] {
            if expected_count != actual_count {
                return Err(StorePlanError::CountMismatch {
                    family,
                    expected: expected_count,
                    actual: actual_count,
                });
            }
        }
        if self.stats.input_digest != self.identities.input_snapshot
            || self.stats.provider_manifest_digest != self.identities.provider_manifest
            || self.stats.provider_output_digest != self.identities.provider_output
            || self.stats.layer_digest != self.identities.layer
            || self.stats.summary_digest != self.identities.summary
            || self.stats.query_digest != self.identities.query
            || self.stats.fact_digest != self.identities.fact
            || self.stats.dependency_digest != self.identities.dependency
            || self.stats.validation_digest != self.identities.validation
        {
            return Err(StorePlanError::IdentityMismatch {
                family: "generation stats",
            });
        }
        if self.stats.input_logical_bytes != expected.input_logical_bytes
            || self.stats.provider_logical_bytes != expected.provider_logical_bytes
            || self.stats.layer_logical_bytes != expected.layer_logical_bytes
            || self.stats.summary_logical_bytes != expected.summary_logical_bytes
            || self.stats.query_logical_bytes != expected.query_logical_bytes
            || self.stats.fact_logical_bytes != expected.fact_logical_bytes
            || self.stats.diagnostic_logical_bytes != expected.diagnostic_logical_bytes
            || self.stats.dependency_logical_bytes != expected.dependency_logical_bytes
            || self.stats.validation_logical_bytes != expected.validation_logical_bytes
            || self.stats.semantic_logical_bytes != expected.semantic_logical_bytes
        {
            return Err(StorePlanError::CountMismatch {
                family: "logical bytes",
                expected: expected.semantic_logical_bytes,
                actual: self.stats.semantic_logical_bytes,
            });
        }
        Ok(())
    }
}

impl StoreGenerationStats {
    fn empty(identities: &StoreIdentityRow) -> Self {
        Self {
            input_file_count: 0,
            input_component_count: 0,
            input_detail_count: 0,
            analysis_setting_count: 0,
            capability_count: 0,
            provider_schema_count: 0,
            provider_manifest_count: 0,
            provider_generation_count: 0,
            layer_count: 0,
            summary_count: 0,
            query_count: 0,
            fact_count: 0,
            diagnostic_count: 0,
            dependency_edge_count: 0,
            validation_event_count: 0,
            input_digest: identities.input_snapshot.clone(),
            provider_manifest_digest: identities.provider_manifest.clone(),
            provider_output_digest: identities.provider_output.clone(),
            layer_digest: identities.layer.clone(),
            summary_digest: identities.summary.clone(),
            query_digest: identities.query.clone(),
            fact_digest: identities.fact.clone(),
            dependency_digest: identities.dependency.clone(),
            validation_digest: identities.validation.clone(),
            input_logical_bytes: 0,
            provider_logical_bytes: 0,
            layer_logical_bytes: 0,
            summary_logical_bytes: 0,
            query_logical_bytes: 0,
            fact_logical_bytes: 0,
            diagnostic_logical_bytes: 0,
            dependency_logical_bytes: 0,
            validation_logical_bytes: 0,
            semantic_logical_bytes: 0,
        }
    }

    fn from_plan(plan: &StoreSemanticPlan) -> Self {
        let mut stats = Self::empty(&plan.identities);
        stats.input_file_count = row_count(plan.files.len());
        stats.input_component_count = row_count(plan.input_components.len());
        stats.input_detail_count = row_count(plan.input_details.len());
        stats.analysis_setting_count = row_count(plan.analysis_settings.len());
        stats.capability_count = row_count(plan.capabilities.len());
        stats.provider_schema_count = row_count(plan.provider_schemas.len());
        stats.provider_manifest_count = row_count(plan.provider_manifests.len());
        stats.provider_generation_count = row_count(plan.provider_generations.len());
        stats.layer_count = row_count(plan.layers.len());
        stats.summary_count = row_count(plan.summaries.len());
        stats.query_count = row_count(plan.queries.len());
        stats.fact_count = row_count(plan.facts.len());
        stats.diagnostic_count = row_count(plan.diagnostics.len());
        stats.dependency_edge_count = row_count(plan.dependency_edges.len());
        stats.validation_event_count = row_count(plan.validation_events.len());

        stats.input_logical_bytes = logical_size(&plan.input_snapshot)
            .saturating_add(logical_size(&plan.files))
            .saturating_add(logical_size(&plan.input_components))
            .saturating_add(logical_size(&plan.input_details))
            .saturating_add(logical_size(&plan.analysis_settings))
            .saturating_add(logical_size(&plan.capabilities))
            .saturating_add(logical_size(&plan.capability_requesters))
            .saturating_add(logical_size(&plan.provider_schemas))
            .saturating_add(logical_size(&plan.provider_schema_versions))
            .saturating_add(logical_size(&plan.run_manifest));
        stats.provider_logical_bytes = logical_size(&plan.provider_manifests)
            .saturating_add(logical_size(&plan.provider_manifest_schemas))
            .saturating_add(logical_size(&plan.provider_manifest_inputs))
            .saturating_add(logical_size(&plan.provider_manifest_outputs))
            .saturating_add(logical_size(&plan.provider_generations))
            .saturating_add(logical_size(&plan.provider_dependencies));
        stats.layer_logical_bytes = logical_size(&plan.layers)
            .saturating_add(logical_size(&plan.layer_inputs))
            .saturating_add(logical_size(&plan.layer_dependencies))
            .saturating_add(logical_size(&plan.layer_extensions))
            .saturating_add(logical_size(&plan.layer_warnings));
        stats.summary_logical_bytes =
            logical_size(&plan.summaries).saturating_add(logical_size(&plan.summary_dependencies));
        stats.query_logical_bytes = logical_size(&plan.queries)
            .saturating_add(logical_size(&plan.query_inputs))
            .saturating_add(logical_size(&plan.query_layers));
        stats.fact_logical_bytes = logical_size(&plan.facts);
        stats.diagnostic_logical_bytes = logical_rows_size(&plan.diagnostics)
            .saturating_add(logical_rows_size(&plan.diagnostic_requested_views));
        stats.dependency_logical_bytes = logical_size(&plan.dependency_edges);
        stats.validation_logical_bytes = logical_size(&plan.validation_events);
        stats.semantic_logical_bytes = logical_size(&plan.identities)
            .saturating_add(stats.input_logical_bytes)
            .saturating_add(stats.provider_logical_bytes)
            .saturating_add(stats.layer_logical_bytes)
            .saturating_add(stats.summary_logical_bytes)
            .saturating_add(stats.query_logical_bytes)
            .saturating_add(stats.fact_logical_bytes)
            .saturating_add(stats.diagnostic_logical_bytes)
            .saturating_add(stats.dependency_logical_bytes)
            .saturating_add(stats.validation_logical_bytes);
        stats
    }
}

fn append_input_components(
    group: StoreInputGroup,
    components: &[InputComponent],
    rows: &mut Vec<StoreInputComponentRow>,
    details: &mut Vec<StoreInputDetailRow>,
) {
    for component in components {
        rows.push(StoreInputComponentRow {
            group,
            name: component.name.clone(),
            status: component.status.label().to_string(),
            digest: component.digest.clone(),
            detail_count: row_count(component.detail.len()),
        });
        details.extend(component.detail.iter().map(|detail| StoreInputDetailRow {
            group,
            component_name: component.name.clone(),
            component_digest: component.digest.clone(),
            detail: detail.clone(),
        }));
    }
}

fn telemetry_rows(validated: &ValidatedRunMetadata) -> Vec<StoreTelemetryRow> {
    let mut rows = validated
        .input_snapshot()
        .files
        .iter()
        .map(|file| StoreTelemetryRow {
            relative_path: file.relative_path.clone(),
            file_mtime_hint_present: file.mtime_hint_present,
        })
        .collect::<Vec<_>>();
    rows.sort();
    rows
}

fn serialized_enum_label<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .expect("closed canonical enum serializes")
        .as_str()
        .expect("closed canonical enum serializes as a label")
        .to_string()
}

fn row_count(count: usize) -> u64 {
    u64::try_from(count).expect("semantic row count fits in u64")
}

fn logical_size<T: Serialize + ?Sized>(value: &T) -> u64 {
    row_count(
        serde_json::to_vec(value)
            .expect("normalized semantic rows serialize")
            .len(),
    )
}

fn logical_rows_size<T: Serialize>(rows: &[T]) -> u64 {
    rows.iter()
        .fold(0, |total, row| total.saturating_add(logical_size(row)))
}

fn check_count(family: &'static str, expected: u64, actual: usize) -> Result<(), StorePlanError> {
    let actual = row_count(actual);
    if expected == actual {
        Ok(())
    } else {
        Err(StorePlanError::CountMismatch {
            family,
            expected,
            actual,
        })
    }
}

fn require_strictly_sorted<T: Ord>(rows: &[T], family: &'static str) -> Result<(), StorePlanError> {
    if rows.windows(2).any(|pair| pair[0] >= pair[1]) {
        Err(StorePlanError::NonCanonicalRows { family })
    } else {
        Ok(())
    }
}

fn unique_by_provider<'a, T>(
    rows: &'a [T],
    provider_id: impl Fn(&'a T) -> &'a str,
    family: &'static str,
) -> Result<BTreeMap<&'a str, &'a T>, StorePlanError> {
    let mut by_provider = BTreeMap::new();
    for row in rows {
        if by_provider.insert(provider_id(row), row).is_some() {
            return Err(StorePlanError::NonCanonicalRows { family });
        }
    }
    Ok(by_provider)
}

fn child_values<'a, T>(
    rows: &'a [T],
    provider_id: &str,
    row_provider_id: impl Fn(&'a T) -> &'a str,
    value: impl Fn(&'a T) -> &'a str,
) -> Vec<&'a str> {
    rows.iter()
        .filter(|row| row_provider_id(row) == provider_id)
        .map(value)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::incremental::{
        DemandCacheStatus, DemandQueryTrace, DemandQueryTraceEntry, DigestKind,
        InputComponentStatus, InputDependencyKey, KernelRunReport, QueryDependencyInputs,
        dependency_free_test_query_key,
    };
    use crate::analysis_kernel::{
        AnalysisKernel, KernelInput, ProviderManifest, StableFactMetaRow,
    };
    use crate::analysis_plan::AnalysisPlan;
    use crate::cache::Cache;
    use crate::config::load_config;

    struct FinalizedFixture {
        report: KernelRunReport,
        manifests: Vec<ProviderManifest>,
        facts: Vec<StableFactMetaRow>,
    }

    fn finalized_fixture() -> FinalizedFixture {
        let temp = tempfile::tempdir().expect("temporary repository");
        std::fs::write(
            temp.path().join("main.go"),
            "package main\n\nimport \"fmt\"\n\nfunc main() { fmt.Println(\"hello\") }\n",
        )
        .expect("write Go fixture");
        let loaded = load_config(temp.path()).expect("default config loads");
        let cache = Cache::new("", false);
        let analysis_plan = AnalysisPlan::from_capability_names_for_test(&["resolved_imports"]);
        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "store-plan-config",
            rule_digest: "store-plan-rules",
            plan: &analysis_plan,
            parallel: false,
        })
        .expect("kernel fixture completes");
        let facts = output
            .db
            .fact_meta()
            .stable_rows()
            .expect("fact metadata is canonical");

        FinalizedFixture {
            report: with_query_rows(output.run_report),
            manifests: AnalysisKernel::provider_manifests().to_vec(),
            facts,
        }
    }

    fn query_trace_entry(label: &str, cache_status: DemandCacheStatus) -> DemandQueryTraceEntry {
        let dependency = InputDependencyKey::analysis_setting(
            format!("polint.{label}"),
            Digest::from_parts(DigestKind::AnalysisSettings, label, &[label]),
            InputComponentStatus::Present,
        )
        .expect("query fixture uses an analysis-settings digest");
        let mut query_key = dependency_free_test_query_key(
            format!("query.{label}"),
            "1",
            Digest::from_parts(DigestKind::QueryParameters, label, &[label]),
            Digest::from_parts(DigestKind::Budget, label, &["bounded"]),
            PrecisionTier::SetupAware,
        );
        query_key.dependency_inputs = QueryDependencyInputs::new(vec![dependency]);
        query_key.layer_digests = vec![Digest::from_parts(
            DigestKind::ProviderOutput,
            "upstream",
            &[label],
        )];
        DemandQueryTraceEntry {
            query_key,
            result_digest: Digest::from_parts(DigestKind::ProviderOutput, "query_result", &[label]),
            precision_tier: PrecisionTier::SetupAware,
            provenance: format!("native:{label}"),
            cache_status,
            compute_duration_micros: 10,
        }
    }

    fn with_query_rows(mut report: KernelRunReport) -> KernelRunReport {
        let mut trace = DemandQueryTrace::default();
        for (label, status) in [
            ("alpha", DemandCacheStatus::Computed),
            ("beta", DemandCacheStatus::Hit),
            ("gamma", DemandCacheStatus::Miss),
            ("delta", DemandCacheStatus::Computed),
        ] {
            trace.record_entry(query_trace_entry(label, status));
        }
        report.demand_query_trace = trace;
        report
    }

    fn metadata_from(fixture: &FinalizedFixture) -> ValidatedRunMetadata {
        ValidatedRunMetadata::from_finalized_run(
            &fixture.report.input_snapshot,
            &fixture.report.provider_outputs,
            &fixture.report.demand_query_trace,
            fixture.report.validation_events(),
            &fixture.manifests,
            fixture.facts.clone(),
        )
        .expect("fixture produces a validated handoff")
    }

    fn plan_fixture() -> StoreCommitPlan {
        let fixture = finalized_fixture();
        StoreCommitPlan::from_validated_run(&metadata_from(&fixture))
            .expect("validated handoff produces a complete plan")
    }

    #[test]
    fn complete_handoff_normalizes_every_semantic_family() {
        let plan = plan_fixture();
        plan.validate().expect("complete plan validates");

        assert!(!plan.semantic.files.is_empty());
        assert!(!plan.semantic.input_components.is_empty());
        assert!(!plan.semantic.analysis_settings.is_empty());
        assert!(!plan.semantic.capabilities.is_empty());
        assert!(!plan.semantic.provider_schemas.is_empty());
        assert!(!plan.semantic.provider_manifests.is_empty());
        assert!(!plan.semantic.provider_generations.is_empty());
        assert!(!plan.semantic.facts.is_empty());
        assert!(!plan.semantic.queries.is_empty());
        assert!(!plan.semantic.query_inputs.is_empty());
        assert!(!plan.semantic.dependency_edges.is_empty());
        assert_eq!(
            plan.semantic.validation_events.len(),
            ValidationEventKind::ALL.len()
        );
        assert!(plan.semantic.summaries.is_empty());
        assert!(plan.semantic.summary_dependencies.is_empty());
        assert_eq!(
            plan.semantic.stats.provider_manifest_count,
            row_count(plan.semantic.provider_manifests.len())
        );
        assert!(plan.semantic.stats.semantic_logical_bytes > 0);
        assert!(plan.semantic.diagnostics.is_empty());
        assert!(plan.semantic.diagnostic_requested_views.is_empty());
        assert_eq!(plan.semantic.stats.diagnostic_count, 0);
        assert_eq!(plan.semantic.stats.diagnostic_logical_bytes, 0);

        let groups = plan
            .semantic
            .input_components
            .iter()
            .map(|row| row.group)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            groups,
            BTreeSet::from([
                StoreInputGroup::Config,
                StoreInputGroup::GoLifecycle,
                StoreInputGroup::TsJsLifecycle,
                StoreInputGroup::Rule,
                StoreInputGroup::Model,
                StoreInputGroup::Extension,
                StoreInputGroup::ToolInvocation,
            ])
        );
    }

    #[test]
    fn twenty_four_source_permutations_produce_one_identical_plan() {
        let fixture = finalized_fixture();
        let baseline = StoreCommitPlan::from_validated_run(&metadata_from(&fixture))
            .expect("baseline plan validates");

        for seed in 0..24 {
            let mut report = fixture.report.clone();
            let provider_len = report.provider_outputs.len();
            report.provider_outputs.rotate_left(seed % provider_len);
            let entries = report.demand_query_trace.entries().to_vec();
            let mut trace = DemandQueryTrace::default();
            for entry in entries
                .iter()
                .cycle()
                .skip(seed % entries.len())
                .take(entries.len())
            {
                trace.record_entry(entry.clone());
            }
            report.demand_query_trace = trace;

            let mut manifests = fixture.manifests.clone();
            let manifest_len = manifests.len();
            manifests.rotate_left(seed % manifest_len);
            let mut facts = fixture.facts.clone();
            if !facts.is_empty() {
                let fact_len = facts.len();
                facts.rotate_left(seed % fact_len);
            }
            let metadata = ValidatedRunMetadata::from_finalized_run(
                &report.input_snapshot,
                &report.provider_outputs,
                &report.demand_query_trace,
                report.validation_events(),
                &manifests,
                facts,
            )
            .expect("permuted handoff validates");
            let candidate =
                StoreCommitPlan::from_validated_run(&metadata).expect("permuted plan validates");
            assert_eq!(candidate, baseline, "seed {seed}");
        }
    }

    #[test]
    fn runtime_telemetry_mutations_leave_the_semantic_plan_identical() {
        let fixture = finalized_fixture();
        let baseline = StoreCommitPlan::from_validated_run(&metadata_from(&fixture))
            .expect("baseline plan validates");

        let mut changed_report = fixture.report.clone();
        changed_report.cache_stats.hits = changed_report.cache_stats.hits.saturating_add(91);
        for provider in &mut changed_report.provider_outputs {
            provider.cache_stats.misses = provider.cache_stats.misses.saturating_add(17);
            provider.cache_stats.writes = provider.cache_stats.writes.saturating_add(23);
        }
        for file in &mut changed_report.input_snapshot.files {
            file.mtime_hint_present = !file.mtime_hint_present;
        }
        let mut changed_trace = DemandQueryTrace::default();
        for mut entry in changed_report.demand_query_trace.entries().iter().cloned() {
            entry.cache_status = match entry.cache_status {
                DemandCacheStatus::Computed => DemandCacheStatus::Hit,
                DemandCacheStatus::Hit | DemandCacheStatus::Miss => DemandCacheStatus::Computed,
            };
            entry.compute_duration_micros = entry.compute_duration_micros.saturating_add(999_999);
            changed_trace.record_entry(entry);
        }
        changed_report.demand_query_trace = changed_trace;
        let changed_metadata = ValidatedRunMetadata::from_finalized_run(
            &changed_report.input_snapshot,
            &changed_report.provider_outputs,
            &changed_report.demand_query_trace,
            changed_report.validation_events(),
            &fixture.manifests,
            fixture.facts.clone(),
        )
        .expect("telemetry-mutated handoff validates");
        let changed = StoreCommitPlan::from_validated_run(&changed_metadata)
            .expect("telemetry-mutated plan validates");

        assert_eq!(changed.semantic, baseline.semantic);
        assert_ne!(changed.telemetry, baseline.telemetry);
    }

    #[test]
    fn typed_negative_fixtures_reject_each_incomplete_family() {
        let baseline = plan_fixture();

        let mut candidate = baseline.clone();
        candidate.semantic.input_snapshot.schema_version = "future-input-shape".to_string();
        assert!(matches!(
            candidate.validate(),
            Err(StorePlanError::UnsupportedInputSnapshotSchema { .. })
        ));

        let mut candidate = baseline.clone();
        candidate.semantic.validation_events.pop();
        assert!(matches!(
            candidate.validate(),
            Err(StorePlanError::MissingValidationEvent { .. })
        ));

        let mut candidate = baseline.clone();
        candidate.semantic.validation_events[0].status =
            ValidationEventStatus::Failed.label().to_string();
        candidate.semantic.validation_events[0].issue_count = 1;
        assert!(matches!(
            candidate.validate(),
            Err(StorePlanError::FailedValidationEvent { .. })
        ));

        let mut candidate = baseline.clone();
        candidate.semantic.provider_manifest_inputs.reverse();
        assert!(matches!(
            candidate.validate(),
            Err(StorePlanError::NonCanonicalRows { .. })
        ));

        let mut candidate = baseline.clone();
        let duplicate = candidate.semantic.facts[0].clone();
        candidate.semantic.facts.push(duplicate);
        candidate.semantic.facts.sort();
        assert!(matches!(
            candidate.validate(),
            Err(StorePlanError::DuplicateFact { .. })
        ));

        let mut candidate = baseline.clone();
        candidate.semantic.provider_generations[0].provider_id = "unknown.provider".to_string();
        assert!(matches!(
            candidate.validate(),
            Err(StorePlanError::UnknownProvider { .. })
        ));

        let mut candidate = baseline.clone();
        candidate.semantic.provider_generations[0].schema_version = "unknown-schema".to_string();
        assert!(matches!(
            candidate.validate(),
            Err(StorePlanError::UnknownProviderSchema { .. })
        ));

        let mut candidate = baseline.clone();
        candidate.semantic.query_inputs.remove(0);
        assert!(matches!(
            candidate.validate(),
            Err(StorePlanError::DanglingQueryInput { .. })
        ));

        let mut candidate = baseline.clone();
        let query_key = candidate.semantic.queries[0].key.clone();
        let edge_position = candidate
            .semantic
            .dependency_edges
            .iter()
            .position(|edge| edge.from == CacheNode::Query(query_key.clone()))
            .expect("query has dependency edges");
        candidate.semantic.dependency_edges.remove(edge_position);
        assert!(matches!(
            candidate.validate(),
            Err(StorePlanError::DanglingQueryEndpoint { .. })
        ));

        let mut candidate = baseline.clone();
        let input = candidate.semantic.query_inputs[0].input.clone();
        let missing_summary = SummaryKey {
            callable_stable_key: "missing:callable".to_string(),
            summary_domain: "missing".to_string(),
            summary_version: "1".to_string(),
            body_shape_digest: Digest::absent(DigestKind::SummaryBody, "missing"),
            dependency_summary_digests: Vec::new(),
            extension_digest: Digest::absent(DigestKind::ExtensionCode, "missing"),
        };
        candidate
            .semantic
            .dependency_edges
            .push(StoreDependencyEdgeRow {
                from: CacheNode::DependencyInput(input),
                to: CacheNode::Summary(missing_summary),
                kind: DependencyKind::Input,
                required_shape: ShapeKind::Unknown,
            });
        assert!(matches!(
            candidate.validate(),
            Err(StorePlanError::DanglingDependencyEndpoint { .. })
        ));

        let mut candidate = baseline.clone();
        candidate
            .semantic
            .input_snapshot
            .input_digest
            .value
            .push('f');
        assert!(matches!(
            candidate.validate(),
            Err(StorePlanError::IdentityMismatch { .. })
        ));

        let mut candidate = baseline.clone();
        candidate.semantic.stats.query_count =
            candidate.semantic.stats.query_count.saturating_add(1);
        assert!(matches!(
            candidate.validate(),
            Err(StorePlanError::CountMismatch { .. })
        ));

        let mut candidate = baseline.clone();
        candidate.semantic.provider_generations[0].validation = "invalid".to_string();
        assert!(matches!(
            candidate.validate(),
            Err(StorePlanError::InvalidStatus { .. })
        ));

        let mut candidate = baseline.clone();
        candidate.semantic.files[0].relative_path = "/private/leak.go".to_string();
        assert!(matches!(
            candidate.validate(),
            Err(StorePlanError::AbsolutePath { .. })
        ));

        let mut candidate = baseline.clone();
        candidate.semantic.facts[0].payload_digest.clear();
        assert!(matches!(
            candidate.validate(),
            Err(StorePlanError::MissingPayloadDigest { .. })
        ));

        let mut candidate = baseline;
        candidate.semantic.provider_generations.clear();
        assert!(matches!(
            candidate.validate(),
            Err(StorePlanError::IncompleteGeneration { .. })
        ));

        let handoff_error = StorePlanError::InvalidHandoff {
            message: "fixture".to_string(),
        };
        assert!(handoff_error.to_string().contains("validated-run handoff"));
    }

    #[test]
    fn payload_digests_remain_and_body_fields_are_exactly_absent() {
        let plan = plan_fixture();
        assert!(
            plan.semantic
                .facts
                .iter()
                .all(|fact| !fact.payload_digest.is_empty())
        );
        let encoded = serde_json::to_value(&plan.semantic).expect("semantic plan serializes");
        assert!(contains_exact_key(&encoded, "payload_digest"));

        let forbidden_fields = [
            concat!("source", "_text"),
            concat!("source", "_bytes"),
            concat!("fact", "_payload"),
            concat!("payload", "_blob"),
            concat!("ast", "_blob"),
            concat!("mir", "_blob"),
            concat!("cfg", "_blob"),
            concat!("summary", "_body"),
            concat!("summary", "_blob"),
            concat!("graph", "_nodes"),
            concat!("graph", "_edges"),
        ];
        for forbidden in forbidden_fields {
            assert!(
                !contains_exact_key(&encoded, forbidden),
                "forbidden field `{forbidden}`"
            );
        }
    }

    #[test]
    fn source_keeps_the_plan_private_and_storage_independent() {
        let source = include_str!("commit_plan.rs");
        let production = source
            .split_once("#[cfg(test)]")
            .expect("test boundary exists")
            .0;
        assert!(production.contains("pub(super) struct StoreCommitPlan"));
        assert!(production.contains("pub(super) enum StorePlanError"));
        assert!(production.contains("pub(super) struct StoreGenerationStats"));
        assert!(production.contains("fn from_validated_run"));
        assert!(production.contains("validated: ValidatedRunMetadata"));
        assert!(production.contains("validate_integrity()"));
        assert!(!production.contains("pub(crate) struct StoreCommitPlan"));
        assert!(!production.contains("pub struct StoreCommitPlan"));
        assert!(!production.contains("Digest::from_parts"));
        assert!(!production.contains("Digest::from_unordered"));

        let lower = production.to_ascii_lowercase();
        for forbidden in [
            concat!("rusq", "lite"),
            concat!("stable", "_hash"),
            concat!("row", "_id"),
            concat!("create", " table"),
            concat!("select", " "),
            concat!("insert", " "),
        ] {
            assert!(
                !lower.contains(forbidden),
                "backend vocabulary `{forbidden}`"
            );
        }

        let facade = include_str!("mod.rs");
        assert!(facade.contains("mod commit_plan;"));
        assert!(!facade.contains("pub(crate) mod commit_plan"));
        assert!(!facade.contains("pub mod commit_plan"));

        let parent = include_str!("../mod.rs");
        assert!(!parent.contains("StoreCommitPlan"));
        assert!(!parent.contains("from_validated_run"));
    }

    fn contains_exact_key(value: &serde_json::Value, expected: &str) -> bool {
        match value {
            serde_json::Value::Object(fields) => fields
                .iter()
                .any(|(key, value)| key == expected || contains_exact_key(value, expected)),
            serde_json::Value::Array(values) => values
                .iter()
                .any(|value| contains_exact_key(value, expected)),
            serde_json::Value::Null
            | serde_json::Value::Bool(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::String(_) => false,
        }
    }
}

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

use rayon::prelude::*;
use serde::Serialize;

use crate::analysis_kernel::StableFactMetaRow;
use crate::analysis_kernel::incremental::{
    CacheNode, CanonicalRunIdentities, ConfigIdentity, DependencyIndex, DependencyKind,
    DiagnosticKey, Digest, DigestKind, GenerationIdentity, INPUT_SNAPSHOT_SCHEMA_VERSION,
    InputComponent, InputComponentStatus, InputDependencyKey, LayerKey, PrecisionTier,
    ProviderValidationStatus, QueryKey, RunIdentity, RunManifestKey, ShapeKind, SummaryKey,
    ValidatedRunMetadata, WorkspaceIdentity, dependency_rows_digest, fact_rows_digest,
    input_snapshot_digest_row, input_snapshot_rows_digest, input_snapshot_semantic_row,
    input_snapshot_semantic_row_builder, provider_manifest_digest_from_fields,
    provider_manifest_rows_digest, serialized_rows_digest,
};
use crate::analysis_kernel::metadata::{CanonicalFactStorageProof, StableFactRowBudget};
use crate::analysis_kernel::validation::{
    ValidationEvent, ValidationEventKind, ValidationEventStatus,
};
use crate::analysis_plan::CapabilitySetupStatus;
use crate::core::CapabilitySupportStatus;

pub(super) const MAX_GENERATION_STORAGE_ROWS: u64 = 1_000_000;
pub(super) const MAX_GENERATION_STORAGE_BYTES: u64 = 512 * 1024 * 1024;
pub(super) const GENERATION_STORAGE_ROW_OVERHEAD_BYTES: u64 = 256;

#[derive(Clone, Copy, Debug)]
pub(super) struct GenerationStorageLimits {
    pub(super) rows: u64,
    pub(super) bytes: u64,
}

pub(super) fn generation_storage_limits() -> GenerationStorageLimits {
    #[cfg(test)]
    if let Some(limits) = TEST_GENERATION_STORAGE_LIMITS.with(std::cell::Cell::get) {
        return limits;
    }
    GenerationStorageLimits {
        rows: MAX_GENERATION_STORAGE_ROWS,
        bytes: MAX_GENERATION_STORAGE_BYTES,
    }
}

#[cfg(test)]
std::thread_local! {
    static TEST_GENERATION_STORAGE_LIMITS: std::cell::Cell<Option<GenerationStorageLimits>> =
        const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub(super) struct GenerationStorageLimitsGuard {
    prior: Option<GenerationStorageLimits>,
}

#[cfg(test)]
impl Drop for GenerationStorageLimitsGuard {
    fn drop(&mut self) {
        TEST_GENERATION_STORAGE_LIMITS.with(|limits| limits.set(self.prior));
    }
}

#[cfg(test)]
pub(super) fn override_generation_storage_limits_for_test(
    rows: u64,
    bytes: u64,
) -> GenerationStorageLimitsGuard {
    let replacement = Some(GenerationStorageLimits { rows, bytes });
    let prior = TEST_GENERATION_STORAGE_LIMITS.with(|limits| limits.replace(replacement));
    GenerationStorageLimitsGuard { prior }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct StoreCommitPlan {
    pub(super) semantic: StoreSemanticPlan,
    pub(super) telemetry: Vec<StoreTelemetryRow>,
}

pub(super) struct ValidatedStoreCommitPlan(StoreCommitPlan);

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
        endpoint: StoreNodeRef,
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
    InvalidFactStorage,
    StorageBudgetExceeded {
        rows: u64,
        bytes: u64,
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
            Self::InvalidFactStorage => {
                formatter.write_str("fact metadata exceeds the stable storage budget")
            }
            Self::StorageBudgetExceeded { rows, bytes } => write!(
                formatter,
                "generation storage exceeds the durable budget ({rows} rows, {bytes} bytes)"
            ),
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

impl StoreIdentityRow {
    pub(super) fn matches_canonical(&self, identities: &CanonicalRunIdentities) -> bool {
        self.workspace == *identities.workspace()
            && self.full_config == *identities.full_config()
            && self.input_snapshot == *identities.input_snapshot()
            && self.provider_manifest == *identities.provider_manifest()
            && self.provider_output == *identities.provider_output()
            && self.layer == *identities.layer()
            && self.summary == *identities.summary()
            && self.query == *identities.query()
            && self.fact == *identities.fact()
            && self.dependency == *identities.dependency()
            && self.validation == *identities.validation()
            && self.run == *identities.run()
            && self.generation == *identities.generation()
    }
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
    pub(super) layer_ordinal: u64,
    pub(super) digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreLayerDependencyRow {
    pub(super) layer_ordinal: u64,
    pub(super) digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreLayerExtensionRow {
    pub(super) layer_ordinal: u64,
    pub(super) digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreLayerWarningRow {
    pub(super) layer_ordinal: u64,
    pub(super) warning_code: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreSummaryRow {
    pub(super) key: SummaryKey,
    pub(super) dependency_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreSummaryDependencyRow {
    pub(super) summary_ordinal: u64,
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
    pub(super) query_ordinal: u64,
    pub(super) input: InputDependencyKey,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreQueryLayerRow {
    pub(super) query_ordinal: u64,
    pub(super) layer_digest: Digest,
}

pub(super) type StoreFactRow = StableFactMetaRow;

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
    pub(super) diagnostic_ordinal: u64,
    pub(super) digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) struct StoreDependencyEdgeRow {
    pub(super) from: StoreNodeRef,
    pub(super) to: StoreNodeRef,
    pub(super) kind: DependencyKind,
    pub(super) required_shape: ShapeKind,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) enum StoreNodeRef {
    DependencyInput(InputDependencyKey),
    RunManifest,
    Layer(u64),
    Query(u64),
    Summary(u64),
    Diagnostic(u64),
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

#[derive(Serialize)]
struct PersistedProviderOutputIdentity<'a> {
    provider_id: &'a str,
    provider_version: &'a str,
    schema_version: &'a str,
    output_digest: &'a Digest,
    precision: PrecisionTier,
    validation: ProviderValidationStatus,
    dependency_inputs: &'a [Digest],
    layer_count: usize,
}

#[derive(Serialize)]
struct PersistedLayerIdentity<'a> {
    key: &'a LayerKey,
    output_digest: &'a Digest,
    payload_digest: &'a Digest,
    precision: PrecisionTier,
    validation: ProviderValidationStatus,
    edge_count: usize,
    warning_codes: &'a [String],
}

#[derive(Serialize)]
struct PersistedQueryIdentity<'a> {
    query_key: &'a QueryKey,
    result_digest: &'a Digest,
    precision: PrecisionTier,
    provenance: &'a str,
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
        let semantic = StoreSemanticPlan::from_validated(validated)?;
        let plan = Self {
            semantic,
            telemetry,
        };
        plan.semantic.validate_without_stats()?;
        Ok(plan)
    }

    pub(super) fn from_owned_validated_run(
        mut validated: ValidatedRunMetadata,
    ) -> Result<ValidatedStoreCommitPlan, StorePlanError> {
        let telemetry = telemetry_rows(&validated);
        let (facts, fact_storage_proof) = validated.take_fact_rows_with_storage_proof();
        let dependency_index = validated.take_dependency_index();
        let semantic =
            StoreSemanticPlan::from_validated_parts(&validated, facts, Some(dependency_index))?;
        let plan = Self {
            semantic,
            telemetry,
        };
        if let Some(proof) = &fact_storage_proof {
            plan.semantic
                .validate_without_stats_with_canonical_fact_proof(proof)?;
        } else {
            plan.semantic.validate_without_stats()?;
        }
        Ok(ValidatedStoreCommitPlan(plan))
    }

    pub(super) fn validate(&self) -> Result<(), StorePlanError> {
        self.semantic.validate()?;
        self.validate_storage_budget(&self.semantic.stats)
    }

    pub(super) fn validate_storage_budget(
        &self,
        stats: &StoreGenerationStats,
    ) -> Result<(), StorePlanError> {
        let rows = self
            .semantic
            .planned_semantic_row_count()
            .checked_add(row_count(self.telemetry.len()))
            .and_then(|rows| rows.checked_add(1))
            .ok_or(StorePlanError::StorageBudgetExceeded {
                rows: u64::MAX,
                bytes: u64::MAX,
            })?;
        let bytes = stats
            .semantic_logical_bytes
            .checked_add(logical_size(&self.telemetry))
            .and_then(|bytes| {
                rows.checked_mul(GENERATION_STORAGE_ROW_OVERHEAD_BYTES)
                    .and_then(|overhead| bytes.checked_add(overhead))
            })
            .ok_or(StorePlanError::StorageBudgetExceeded {
                rows,
                bytes: u64::MAX,
            })?;
        let limits = generation_storage_limits();
        if rows > limits.rows || bytes > limits.bytes {
            return Err(StorePlanError::StorageBudgetExceeded { rows, bytes });
        }
        Ok(())
    }

    pub(super) fn into_validated(self) -> Result<ValidatedStoreCommitPlan, StorePlanError> {
        self.validate()?;
        Ok(ValidatedStoreCommitPlan(self))
    }
}

impl ValidatedStoreCommitPlan {
    pub(super) fn planned_semantic_row_count(&self) -> u64 {
        self.0.semantic.planned_semantic_row_count()
    }

    pub(super) fn into_inner(self) -> StoreCommitPlan {
        self.0
    }
}

impl StoreSemanticPlan {
    pub(super) fn planned_semantic_row_count(&self) -> u64 {
        [
            1,
            1,
            self.files.len(),
            self.input_components.len(),
            self.input_details.len(),
            self.analysis_settings.len(),
            self.capabilities.len(),
            self.capability_requesters.len(),
            self.provider_schemas.len(),
            self.provider_schema_versions.len(),
            self.provider_manifests.len(),
            self.provider_manifest_schemas.len(),
            self.provider_manifest_inputs.len(),
            self.provider_manifest_outputs.len(),
            self.provider_generations.len(),
            self.provider_dependencies.len(),
            self.layers.len(),
            self.layer_inputs.len(),
            self.layer_dependencies.len(),
            self.layer_extensions.len(),
            self.layer_warnings.len(),
            self.summaries.len(),
            self.summary_dependencies.len(),
            self.queries.len(),
            self.query_inputs.len(),
            self.query_layers.len(),
            self.facts.len(),
            1,
            self.diagnostics.len(),
            self.diagnostic_requested_views.len(),
            self.dependency_edges.len(),
            self.validation_events.len(),
            1,
        ]
        .into_iter()
        .fold(0_u64, |total, count| {
            total.saturating_add(u64::try_from(count).unwrap_or(u64::MAX))
        })
    }

    pub(super) fn recomputed_identities(
        &self,
        dependency_index: &DependencyIndex,
    ) -> Result<CanonicalRunIdentities, StorePlanError> {
        let input_snapshot = self.recomputed_input_snapshot_digest();
        let mut manifest_digests = Vec::with_capacity(self.provider_manifests.len());
        for manifest in &self.provider_manifests {
            let mut schemas = self
                .provider_manifest_schemas
                .iter()
                .filter(|row| row.provider_id == manifest.provider_id)
                .map(|row| row.schema_version.clone())
                .collect::<Vec<_>>();
            let mut inputs = self
                .provider_manifest_inputs
                .iter()
                .filter(|row| row.provider_id == manifest.provider_id)
                .map(|row| row.input.clone())
                .collect::<Vec<_>>();
            let mut outputs = self
                .provider_manifest_outputs
                .iter()
                .filter(|row| row.provider_id == manifest.provider_id)
                .map(|row| row.output.clone())
                .collect::<Vec<_>>();
            schemas.sort_unstable();
            inputs.sort_unstable();
            outputs.sort_unstable();
            let digest = provider_manifest_digest_from_fields(
                &manifest.provider_id,
                &manifest.provider_version,
                &manifest.provider_kind,
                &manifest.language_scope,
                &manifest.cache_policy,
                &manifest.precision_ceiling,
                &schemas,
                &inputs,
                &outputs,
            );
            if digest != manifest.manifest_digest {
                return Err(StorePlanError::IdentityMismatch {
                    family: "provider manifest",
                });
            }
            manifest_digests.push(digest);
        }
        let provider_manifest = provider_manifest_rows_digest(manifest_digests);

        let provider_dependency_rows = self
            .provider_generations
            .iter()
            .map(|provider| {
                let mut dependencies = self
                    .provider_dependencies
                    .iter()
                    .filter(|row| {
                        row.provider_id == provider.provider_id
                            && row.provider_version == provider.provider_version
                            && row.schema_version == provider.schema_version
                            && row.output_digest == provider.output_digest
                    })
                    .map(|row| row.dependency.clone())
                    .collect::<Vec<_>>();
                dependencies.sort();
                let precision = PrecisionTier::parse_label(&provider.precision).map_err(|_| {
                    StorePlanError::InvalidStatus {
                        family: "provider precision",
                        value: provider.precision.clone(),
                    }
                })?;
                let validation = ProviderValidationStatus::parse_label(&provider.validation)
                    .map_err(|_| StorePlanError::InvalidStatus {
                        family: "provider validation",
                        value: provider.validation.clone(),
                    })?;
                let layer_count = usize::try_from(provider.layer_count).map_err(|_| {
                    StorePlanError::CountMismatch {
                        family: "provider layer",
                        expected: provider.layer_count,
                        actual: u64::MAX,
                    }
                })?;
                Ok((provider, dependencies, precision, validation, layer_count))
            })
            .collect::<Result<Vec<_>, StorePlanError>>()?;
        let provider_output_rows = provider_dependency_rows
            .iter()
            .map(
                |(provider, dependencies, precision, validation, layer_count)| {
                    PersistedProviderOutputIdentity {
                        provider_id: &provider.provider_id,
                        provider_version: &provider.provider_version,
                        schema_version: &provider.schema_version,
                        output_digest: &provider.output_digest,
                        precision: *precision,
                        validation: *validation,
                        dependency_inputs: dependencies,
                        layer_count: *layer_count,
                    }
                },
            )
            .collect::<Vec<_>>();
        let provider_output = serialized_rows_digest(
            DigestKind::ProviderOutput,
            "provider_output_rows",
            &provider_output_rows,
        );

        let layer_warning_rows =
            self.layers
                .iter()
                .enumerate()
                .map(|(ordinal, layer)| {
                    let ordinal = row_count(ordinal);
                    let mut warnings = self
                        .layer_warnings
                        .iter()
                        .filter(|row| row.layer_ordinal == ordinal)
                        .map(|row| row.warning_code.clone())
                        .collect::<Vec<_>>();
                    warnings.sort();
                    let precision = PrecisionTier::parse_label(&layer.precision).map_err(|_| {
                        StorePlanError::InvalidStatus {
                            family: "layer precision",
                            value: layer.precision.clone(),
                        }
                    })?;
                    let validation = ProviderValidationStatus::parse_label(&layer.validation)
                        .map_err(|_| StorePlanError::InvalidStatus {
                            family: "layer validation",
                            value: layer.validation.clone(),
                        })?;
                    let edge_count = usize::try_from(layer.edge_count).map_err(|_| {
                        StorePlanError::CountMismatch {
                            family: "layer edge",
                            expected: layer.edge_count,
                            actual: u64::MAX,
                        }
                    })?;
                    Ok((layer, warnings, precision, validation, edge_count))
                })
                .collect::<Result<Vec<_>, StorePlanError>>()?;
        let layer_rows = layer_warning_rows
            .iter()
            .map(
                |(layer, warnings, precision, validation, edge_count)| PersistedLayerIdentity {
                    key: &layer.key,
                    output_digest: &layer.output_digest,
                    payload_digest: &layer.payload_digest,
                    precision: *precision,
                    validation: *validation,
                    edge_count: *edge_count,
                    warning_codes: warnings,
                },
            )
            .collect::<Vec<_>>();
        let layer = serialized_rows_digest(DigestKind::Layer, "layer_rows", &layer_rows);
        let summary_keys = self
            .summaries
            .iter()
            .map(|row| &row.key)
            .collect::<Vec<_>>();
        let summary = serialized_rows_digest(DigestKind::Summary, "summary_rows", &summary_keys);
        let query_rows = self
            .queries
            .iter()
            .map(|row| {
                let precision = PrecisionTier::parse_label(&row.precision).map_err(|_| {
                    StorePlanError::InvalidStatus {
                        family: "query precision",
                        value: row.precision.clone(),
                    }
                })?;
                Ok(PersistedQueryIdentity {
                    query_key: &row.key,
                    result_digest: &row.result_digest,
                    precision,
                    provenance: &row.provenance,
                })
            })
            .collect::<Result<Vec<_>, StorePlanError>>()?;
        let query = serialized_rows_digest(DigestKind::Query, "query_rows", &query_rows);
        let fact = fact_rows_digest(&self.facts);
        let dependency = dependency_rows_digest(dependency_index.canonical_edges());
        let validation_events = self
            .validation_events
            .iter()
            .map(|row| {
                let kind = ValidationEventKind::parse_label(&row.kind).map_err(|_| {
                    StorePlanError::InvalidStatus {
                        family: "validation event kind",
                        value: row.kind.clone(),
                    }
                })?;
                let status = ValidationEventStatus::parse_label(&row.status).map_err(|_| {
                    StorePlanError::InvalidStatus {
                        family: "validation event status",
                        value: row.status.clone(),
                    }
                })?;
                Ok(ValidationEvent {
                    kind,
                    status,
                    issue_count: row.issue_count,
                    digest: row.digest.clone(),
                })
            })
            .collect::<Result<Vec<_>, StorePlanError>>()?;
        let validation = serialized_rows_digest(
            DigestKind::ValidationEvent,
            "validation_rows",
            &validation_events,
        );
        CanonicalRunIdentities::from_recomputed_families(
            self.input_snapshot.workspace.clone(),
            self.input_snapshot.full_config.clone(),
            input_snapshot,
            provider_manifest,
            provider_output,
            layer,
            summary,
            query,
            fact,
            dependency,
            validation,
        )
        .map_err(|_| StorePlanError::IdentityMismatch {
            family: "generation",
        })
    }

    fn recomputed_input_snapshot_digest(&self) -> Digest {
        input_snapshot_rows_digest(self.recomputed_input_snapshot_rows())
    }

    fn recomputed_input_snapshot_rows(&self) -> Vec<Digest> {
        let mut rows = vec![input_snapshot_semantic_row(
            "schema",
            [("version", self.input_snapshot.schema_version.as_str())],
        )];
        rows.push(input_snapshot_digest_row(
            "workspace_identity",
            self.input_snapshot.workspace.digest(),
        ));
        rows.push(input_snapshot_digest_row(
            "config_identity",
            self.input_snapshot.full_config.digest(),
        ));
        rows.push(input_snapshot_digest_row(
            "analysis_requirements_identity",
            &self.input_snapshot.analysis_requirements_digest,
        ));
        for file in &self.files {
            let size_bytes = file.size_bytes.to_string();
            let mut row = input_snapshot_semantic_row_builder("file");
            row.labeled_part("relative_path", &file.relative_path);
            row.labeled_part("language", &file.language);
            row.labeled_part("source_digest_kind", file.source_digest.kind.label());
            row.labeled_part("source_digest_value", &file.source_digest.value);
            row.labeled_part("size_bytes", &size_bytes);
            rows.push(row.finish());
        }
        for component in &self.input_components {
            let mut row = input_snapshot_semantic_row_builder(component.group.snapshot_row_kind());
            row.labeled_part("name", &component.name);
            row.labeled_part("status", &component.status);
            row.labeled_part("digest_kind", component.digest.kind.label());
            row.labeled_part("digest_value", &component.digest.value);
            rows.push(row.finish());
        }
        for provider in &self.provider_schemas {
            let mut row = input_snapshot_semantic_row_builder("provider_schema");
            row.labeled_part("provider_id", &provider.provider_id);
            let mut versions = self
                .provider_schema_versions
                .iter()
                .filter(|version| version.provider_id == provider.provider_id)
                .map(|version| version.schema_version.as_str())
                .collect::<Vec<_>>();
            versions.sort_unstable();
            for version in versions {
                row.labeled_part("schema_version", version);
            }
            row.labeled_part("language_scope", &provider.language_scope);
            row.labeled_part("cache_policy", &provider.cache_policy);
            row.labeled_part("precision_ceiling", &provider.precision_ceiling);
            row.labeled_part(
                "manifest_digest_kind",
                provider.manifest_digest.kind.label(),
            );
            row.labeled_part("manifest_digest_value", &provider.manifest_digest.value);
            rows.push(row.finish());
        }
        for setting in &self.analysis_settings {
            let mut row = input_snapshot_semantic_row_builder("analysis_setting");
            row.labeled_part("provider_id", &setting.scope);
            row.labeled_part("digest_kind", setting.digest.kind.label());
            row.labeled_part("digest_value", &setting.digest.value);
            rows.push(row.finish());
        }
        for capability in &self.capabilities {
            let mut row = input_snapshot_semantic_row_builder("requested_capability");
            row.labeled_part("capability", &capability.capability);
            row.labeled_part("language", capability.language.as_deref().unwrap_or("none"));
            let support_status = match capability.support_status.as_str() {
                "Supported" => "supported",
                "Unsupported" => "unsupported",
                "SetupMissing" => "setup_missing",
                label => label,
            };
            row.labeled_part("support_status", support_status);
            row.labeled_part("setup_status", &capability.setup_status);
            row.labeled_part(
                "policy_query_version",
                capability.policy_query_version.as_deref().unwrap_or("none"),
            );
            let mut requesters = self
                .capability_requesters
                .iter()
                .filter(|requester| {
                    requester.capability == capability.capability
                        && requester.language == capability.language
                })
                .map(|requester| requester.rule_id.as_str())
                .collect::<Vec<_>>();
            requesters.sort_unstable();
            for requester in requesters {
                row.labeled_part("requesting_rule_id", requester);
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
        rows
    }

    fn from_validated(validated: &ValidatedRunMetadata) -> Result<Self, StorePlanError> {
        Self::from_validated_parts(validated, validated.fact_rows().to_vec(), None)
    }

    fn from_validated_parts(
        validated: &ValidatedRunMetadata,
        fact_rows: Vec<StableFactMetaRow>,
        owned_dependency_index: Option<crate::analysis_kernel::incremental::DependencyIndex>,
    ) -> Result<Self, StorePlanError> {
        let defer_stats = owned_dependency_index.is_some();
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
            }
        }
        provider_generations.sort();
        provider_dependencies.sort();
        layers.sort();
        let layer_ordinals = layers
            .iter()
            .enumerate()
            .map(|(ordinal, row)| (&row.key, row_count(ordinal)))
            .collect::<BTreeMap<_, _>>();
        for provider in validated.provider_outputs() {
            for layer in provider.layers() {
                let layer_ordinal = *layer_ordinals
                    .get(&layer.key)
                    .expect("validated layer has a canonical parent ordinal");
                layer_inputs.extend(layer.key.input_digests.iter().map(|digest| {
                    StoreLayerInputRow {
                        layer_ordinal,
                        digest: digest.clone(),
                    }
                }));
                layer_dependencies.extend(layer.key.dependency_layer_digests.iter().map(
                    |digest| StoreLayerDependencyRow {
                        layer_ordinal,
                        digest: digest.clone(),
                    },
                ));
                layer_extensions.extend(layer.key.extension_digests.iter().map(|digest| {
                    StoreLayerExtensionRow {
                        layer_ordinal,
                        digest: digest.clone(),
                    }
                }));
                layer_warnings.extend(layer.warning_codes.iter().map(|warning_code| {
                    StoreLayerWarningRow {
                        layer_ordinal,
                        warning_code: warning_code.clone(),
                    }
                }));
            }
        }
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
        }
        summaries.sort();
        let summary_ordinals = summaries
            .iter()
            .enumerate()
            .map(|(ordinal, row)| (&row.key, row_count(ordinal)))
            .collect::<BTreeMap<_, _>>();
        for key in validated.summary_keys() {
            let summary_ordinal = *summary_ordinals
                .get(key)
                .expect("validated summary has a canonical parent ordinal");
            summary_dependencies.extend(key.dependency_summary_digests.iter().map(|dependency| {
                StoreSummaryDependencyRow {
                    summary_ordinal,
                    dependency: dependency.clone(),
                }
            }));
        }
        summary_dependencies.sort();

        let mut queries = Vec::with_capacity(validated.query_rows().len());
        let mut query_inputs = Vec::new();
        let mut query_layers = Vec::new();
        let mut query_edge_counts = BTreeMap::<&QueryKey, usize>::new();
        let dependency_index = owned_dependency_index
            .as_ref()
            .unwrap_or_else(|| validated.dependency_index());
        for edge in dependency_index.canonical_edges() {
            if let CacheNode::Query(key) = &edge.from {
                *query_edge_counts.entry(key).or_default() += 1;
            }
        }
        for query in validated.query_rows() {
            let key = query.query_key();
            queries.push(StoreQueryRow {
                key: key.clone(),
                result_digest: query.result_digest().clone(),
                precision: query.precision().label().to_string(),
                provenance: query.provenance().to_string(),
                input_count: row_count(key.dependency_inputs.as_slice().len()),
                layer_count: row_count(key.layer_digests.len()),
                edge_count: row_count(query_edge_counts.get(&key).copied().unwrap_or_default()),
            });
        }
        queries.sort();
        let query_ordinals = queries
            .iter()
            .enumerate()
            .map(|(ordinal, row)| (&row.key, row_count(ordinal)))
            .collect::<BTreeMap<_, _>>();
        for query in validated.query_rows() {
            let key = query.query_key();
            let query_ordinal = *query_ordinals
                .get(key)
                .expect("validated query has a canonical parent ordinal");
            query_inputs.extend(key.dependency_inputs.as_slice().iter().map(|input| {
                StoreQueryInputRow {
                    query_ordinal,
                    input: input.clone(),
                }
            }));
            query_layers.extend(
                key.layer_digests
                    .iter()
                    .map(|layer_digest| StoreQueryLayerRow {
                        query_ordinal,
                        layer_digest: layer_digest.clone(),
                    }),
            );
        }
        query_inputs.sort();
        query_layers.sort();

        let facts = fact_rows;
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
        }
        diagnostics.sort();
        let diagnostic_ordinals = diagnostics
            .iter()
            .enumerate()
            .map(|(ordinal, row)| (&row.key, row_count(ordinal)))
            .collect::<BTreeMap<_, _>>();
        for key in validated.diagnostic_keys() {
            let diagnostic_ordinal = *diagnostic_ordinals
                .get(key)
                .expect("validated diagnostic has a canonical parent ordinal");
            diagnostic_requested_views.extend(key.requested_view_digests.iter().map(|digest| {
                StoreDiagnosticRequestedViewRow {
                    diagnostic_ordinal,
                    digest: digest.clone(),
                }
            }));
        }
        diagnostic_requested_views.sort();
        let (dependency_schema, source_dependency_edges) = owned_dependency_index.map_or_else(
            || {
                (
                    validated.dependency_index().schema_version.clone(),
                    validated.dependency_index().canonical_edges().to_vec(),
                )
            },
            |index| index.into_persistence_parts(),
        );
        let dependency_edges = source_dependency_edges
            .into_iter()
            .map(|edge| {
                Ok(StoreDependencyEdgeRow {
                    from: store_node_ref_owned(
                        edge.from,
                        &layer_ordinals,
                        &query_ordinals,
                        &summary_ordinals,
                        &diagnostic_ordinals,
                    )?,
                    to: store_node_ref_owned(
                        edge.to,
                        &layer_ordinals,
                        &query_ordinals,
                        &summary_ordinals,
                        &diagnostic_ordinals,
                    )?,
                    kind: edge.kind,
                    required_shape: edge.required_shape,
                })
            })
            .collect::<Result<Vec<_>, StorePlanError>>()?;
        drop(layer_ordinals);
        drop(query_ordinals);
        drop(summary_ordinals);
        drop(diagnostic_ordinals);
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
            dependency_schema,
            dependency_edges,
            validation_events,
            stats,
        };
        if !defer_stats {
            plan.stats = StoreGenerationStats::from_plan(&plan);
        }
        Ok(plan)
    }

    fn validate(&self) -> Result<(), StorePlanError> {
        self.validate_without_stats()?;
        self.validate_stats()
    }

    fn validate_without_stats(&self) -> Result<(), StorePlanError> {
        self.validate_without_stats_inner(None)
    }

    fn validate_without_stats_with_canonical_fact_proof(
        &self,
        proof: &CanonicalFactStorageProof,
    ) -> Result<(), StorePlanError> {
        self.validate_without_stats_inner(Some(proof))
    }

    fn validate_without_stats_inner(
        &self,
        fact_storage_proof: Option<&CanonicalFactStorageProof>,
    ) -> Result<(), StorePlanError> {
        self.validate_schemas()?;
        self.validate_identity_copies()?;
        self.validate_paths()?;
        self.validate_statuses()?;
        self.validate_required_events()?;
        self.validate_input_relationships()?;
        self.validate_provider_relationships()?;
        self.validate_summary_relationships()?;
        self.validate_query_declarations()?;
        self.validate_result_boundaries()?;
        self.validate_dependency_endpoints()?;
        self.validate_facts(fact_storage_proof)?;
        self.validate_canonical_order(fact_storage_proof.is_some())
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

    fn validate_input_relationships(&self) -> Result<(), StorePlanError> {
        let components = self
            .input_components
            .iter()
            .map(|component| ((component.group, component.name.as_str()), component))
            .collect::<BTreeMap<_, _>>();
        for detail in &self.input_details {
            let component = components
                .get(&(detail.group, detail.component_name.as_str()))
                .ok_or(StorePlanError::IncompleteGeneration {
                    family: "input component parent",
                })?;
            if detail.component_digest != component.digest {
                return Err(StorePlanError::IdentityMismatch {
                    family: "input component detail",
                });
            }
        }
        for component in &self.input_components {
            check_count(
                "input component detail",
                component.detail_count,
                self.input_details
                    .iter()
                    .filter(|detail| {
                        detail.group == component.group && detail.component_name == component.name
                    })
                    .count(),
            )?;
        }

        let capabilities = self
            .capabilities
            .iter()
            .map(|capability| {
                (
                    (
                        capability.capability.as_str(),
                        capability.language.as_deref(),
                    ),
                    capability,
                )
            })
            .collect::<BTreeMap<_, _>>();
        for requester in &self.capability_requesters {
            if !capabilities
                .contains_key(&(requester.capability.as_str(), requester.language.as_deref()))
            {
                return Err(StorePlanError::IncompleteGeneration {
                    family: "capability requester parent",
                });
            }
        }
        for capability in &self.capabilities {
            check_count(
                "capability requester",
                capability.requester_count,
                self.capability_requesters
                    .iter()
                    .filter(|requester| {
                        requester.capability == capability.capability
                            && requester.language == capability.language
                    })
                    .count(),
            )?;
        }
        Ok(())
    }

    fn validate_summary_relationships(&self) -> Result<(), StorePlanError> {
        let mut expected_dependencies = Vec::new();
        for (ordinal, summary) in self.summaries.iter().enumerate() {
            let ordinal = row_count(ordinal);
            check_count(
                "summary dependency",
                summary.dependency_count,
                summary.key.dependency_summary_digests.len(),
            )?;
            expected_dependencies.extend(summary.key.dependency_summary_digests.iter().map(
                |dependency| StoreSummaryDependencyRow {
                    summary_ordinal: ordinal,
                    dependency: dependency.clone(),
                },
            ));
        }
        expected_dependencies.sort();
        if expected_dependencies != self.summary_dependencies {
            return Err(StorePlanError::NonCanonicalRows {
                family: "summary dependency",
            });
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

        for (layer_ordinal, layer) in self.layers.iter().enumerate() {
            let layer_ordinal = row_count(layer_ordinal);
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
                    .filter(|row| row.layer_ordinal == layer_ordinal)
                    .count(),
            )?;
            check_count(
                "layer dependency",
                layer.dependency_layer_count,
                self.layer_dependencies
                    .iter()
                    .filter(|row| row.layer_ordinal == layer_ordinal)
                    .count(),
            )?;
            check_count(
                "layer extension",
                layer.extension_count,
                self.layer_extensions
                    .iter()
                    .filter(|row| row.layer_ordinal == layer_ordinal)
                    .count(),
            )?;
            check_count(
                "layer warning",
                layer.warning_count,
                self.layer_warnings
                    .iter()
                    .filter(|row| row.layer_ordinal == layer_ordinal)
                    .count(),
            )?;
        }
        Ok(())
    }

    fn validate_query_declarations(&self) -> Result<(), StorePlanError> {
        let mut expected_inputs = Vec::new();
        let mut expected_layers = Vec::new();
        let mut edge_counts = vec![0_usize; self.queries.len()];
        let mut declared_inputs = BTreeSet::<(u64, &InputDependencyKey)>::new();
        for edge in &self.dependency_edges {
            let StoreNodeRef::Query(ordinal) = edge.from else {
                continue;
            };
            let index = usize::try_from(ordinal).map_err(|_| {
                StorePlanError::DanglingDependencyEndpoint {
                    endpoint: edge.from.clone(),
                }
            })?;
            let count = edge_counts.get_mut(index).ok_or_else(|| {
                StorePlanError::DanglingDependencyEndpoint {
                    endpoint: edge.from.clone(),
                }
            })?;
            *count += 1;
            if let StoreNodeRef::DependencyInput(input) = &edge.to {
                declared_inputs.insert((ordinal, input));
            }
        }
        for (ordinal, query) in self.queries.iter().enumerate() {
            let ordinal = row_count(ordinal);
            expected_inputs.extend(query.key.dependency_inputs.as_slice().iter().map(|input| {
                StoreQueryInputRow {
                    query_ordinal: ordinal,
                    input: input.clone(),
                }
            }));
            expected_layers.extend(query.key.layer_digests.iter().map(|digest| {
                StoreQueryLayerRow {
                    query_ordinal: ordinal,
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
            if query.edge_count
                != row_count(
                    edge_counts
                        .get(usize::try_from(ordinal).expect("query ordinal fits usize"))
                        .copied()
                        .unwrap_or_default(),
                )
            {
                return Err(StorePlanError::DanglingQueryEndpoint {
                    query_kind: query.key.query_kind.clone(),
                });
            }
            for input in query.key.dependency_inputs.as_slice() {
                if !declared_inputs.contains(&(ordinal, input)) {
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
        let mut layer_edge_counts = vec![0_usize; self.layers.len()];
        for edge in &self.dependency_edges {
            for endpoint in [&edge.from, &edge.to] {
                let retained = match endpoint {
                    StoreNodeRef::DependencyInput(_) | StoreNodeRef::RunManifest => true,
                    StoreNodeRef::Layer(ordinal) => usize::try_from(*ordinal)
                        .ok()
                        .is_some_and(|ordinal| ordinal < self.layers.len()),
                    StoreNodeRef::Query(ordinal) => usize::try_from(*ordinal)
                        .ok()
                        .is_some_and(|ordinal| ordinal < self.queries.len()),
                    StoreNodeRef::Summary(ordinal) => usize::try_from(*ordinal)
                        .ok()
                        .is_some_and(|ordinal| ordinal < self.summaries.len()),
                    StoreNodeRef::Diagnostic(ordinal) => usize::try_from(*ordinal)
                        .ok()
                        .is_some_and(|ordinal| ordinal < self.diagnostics.len()),
                };
                if !retained {
                    return Err(StorePlanError::DanglingDependencyEndpoint {
                        endpoint: endpoint.clone(),
                    });
                }
            }
            if let StoreNodeRef::Layer(ordinal) = edge.from {
                let count = usize::try_from(ordinal)
                    .ok()
                    .and_then(|ordinal| layer_edge_counts.get_mut(ordinal))
                    .ok_or_else(|| StorePlanError::DanglingDependencyEndpoint {
                        endpoint: edge.from.clone(),
                    })?;
                *count += 1;
            }
        }
        for (ordinal, layer) in self.layers.iter().enumerate() {
            check_count(
                "layer edge",
                layer.edge_count,
                layer_edge_counts.get(ordinal).copied().unwrap_or_default(),
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
        for (diagnostic_ordinal, diagnostic) in self.diagnostics.iter().enumerate() {
            let diagnostic_ordinal = row_count(diagnostic_ordinal);
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
                    diagnostic_ordinal,
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

    fn validate_facts(
        &self,
        fact_storage_proof: Option<&CanonicalFactStorageProof>,
    ) -> Result<(), StorePlanError> {
        match fact_storage_proof {
            Some(proof) if proof.matches(&self.facts, &self.identities.fact) => return Ok(()),
            Some(_) => return Err(StorePlanError::InvalidFactStorage),
            None => {}
        }
        let provider_ids = self
            .provider_manifests
            .iter()
            .map(|row| row.provider_id.as_str())
            .collect::<BTreeSet<_>>();
        let mut storage_budget = StableFactRowBudget::new();
        for fact in &self.facts {
            if fact.stable_key.is_empty() {
                return Err(StorePlanError::InvalidStatus {
                    family: "fact stable key",
                    value: String::new(),
                });
            }
            if fact.payload_digest.is_empty() {
                return Err(StorePlanError::MissingPayloadDigest {
                    family: fact.family.label().to_string(),
                    stable_key: fact.stable_key.to_string(),
                });
            }
            if !provider_ids.contains(fact.producer_id.as_ref()) {
                return Err(StorePlanError::UnknownProvider {
                    provider_id: fact.producer_id.to_string(),
                    family: "fact producer",
                });
            }
            if !provider_ids.contains(fact.layer_id.as_ref()) {
                return Err(StorePlanError::UnknownProvider {
                    provider_id: fact.layer_id.to_string(),
                    family: "fact layer",
                });
            }
            let lengths = fact
                .stable_key
                .storage_lengths(fact)
                .map_err(|_| StorePlanError::InvalidFactStorage)?;
            storage_budget
                .charge(lengths)
                .map_err(|_| StorePlanError::InvalidFactStorage)?;
        }
        for pair in self.facts.windows(2) {
            if pair[0].family == pair[1].family && pair[0].stable_key == pair[1].stable_key {
                return Err(StorePlanError::DuplicateFact {
                    family: pair[0].family.label().to_string(),
                    stable_key: pair[0].stable_key.to_string(),
                });
            }
        }
        Ok(())
    }

    fn validate_canonical_order(
        &self,
        fact_rows_are_canonical: bool,
    ) -> Result<(), StorePlanError> {
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
        if !fact_rows_are_canonical {
            require_strictly_sorted(&self.facts, "fact")?;
        }
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

    pub(super) fn from_plan(plan: &StoreSemanticPlan) -> Self {
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
        (stats.input_logical_bytes, stats.provider_logical_bytes) = rayon::join(
            || {
                logical_size(&plan.input_snapshot)
                    .saturating_add(logical_size(&plan.files))
                    .saturating_add(logical_size(&plan.input_components))
                    .saturating_add(logical_size(&plan.input_details))
                    .saturating_add(logical_size(&plan.analysis_settings))
                    .saturating_add(logical_size(&plan.capabilities))
                    .saturating_add(logical_size(&plan.capability_requesters))
                    .saturating_add(logical_size(&plan.provider_schemas))
                    .saturating_add(logical_size(&plan.provider_schema_versions))
                    .saturating_add(logical_size(&plan.run_manifest))
            },
            || {
                logical_size(&plan.provider_manifests)
                    .saturating_add(logical_size(&plan.provider_manifest_schemas))
                    .saturating_add(logical_size(&plan.provider_manifest_inputs))
                    .saturating_add(logical_size(&plan.provider_manifest_outputs))
                    .saturating_add(logical_size(&plan.provider_generations))
                    .saturating_add(logical_size(&plan.provider_dependencies))
            },
        );
        (stats.layer_logical_bytes, stats.query_logical_bytes) = rayon::join(
            || logical_layer_family_size(plan),
            || logical_query_family_size(plan),
        );
        stats.summary_logical_bytes =
            logical_size(&plan.summaries).saturating_add(logical_parented_rows_size(
                &plan.summary_dependencies,
                "dependency",
                &plan.summaries,
                |row| &row.key,
                |row| row.summary_ordinal,
                |row| &row.dependency,
            ));
        (stats.fact_logical_bytes, stats.dependency_logical_bytes) = rayon::join(
            || logical_fact_rows_size(&plan.facts),
            || logical_dependency_rows_size(&plan.dependency_edges),
        );
        stats.diagnostic_logical_bytes = logical_rows_size(&plan.diagnostics).saturating_add(
            logical_parented_individual_rows_size(
                &plan.diagnostic_requested_views,
                "digest",
                &plan.diagnostics,
                |row| &row.key,
                |row| row.diagnostic_ordinal,
                |row| &row.digest,
            ),
        );
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

impl StoreInputGroup {
    fn snapshot_row_kind(self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::GoLifecycle => "go_lifecycle",
            Self::TsJsLifecycle => "ts_js_lifecycle",
            Self::Rule => "rule",
            Self::Model => "model",
            Self::Extension => "extension",
            Self::ToolInvocation => "tool",
        }
    }
}

fn store_node_ref_owned(
    node: CacheNode,
    layers: &BTreeMap<&LayerKey, u64>,
    queries: &BTreeMap<&QueryKey, u64>,
    summaries: &BTreeMap<&SummaryKey, u64>,
    diagnostics: &BTreeMap<&DiagnosticKey, u64>,
) -> Result<StoreNodeRef, StorePlanError> {
    let missing = |family| StorePlanError::InvalidHandoff {
        message: format!("validated dependency {family} endpoint is not retained"),
    };
    Ok(match node {
        CacheNode::DependencyInput(input) => StoreNodeRef::DependencyInput(input),
        CacheNode::RunManifest(_) => StoreNodeRef::RunManifest,
        CacheNode::Layer(key) => {
            StoreNodeRef::Layer(*layers.get(key.as_ref()).ok_or_else(|| missing("layer"))?)
        }
        CacheNode::Query(key) => {
            StoreNodeRef::Query(*queries.get(key.as_ref()).ok_or_else(|| missing("query"))?)
        }
        CacheNode::Summary(key) => {
            StoreNodeRef::Summary(*summaries.get(&key).ok_or_else(|| missing("summary"))?)
        }
        CacheNode::Diagnostic(key) => {
            StoreNodeRef::Diagnostic(*diagnostics.get(&key).ok_or_else(|| missing("diagnostic"))?)
        }
    })
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

#[derive(Default)]
struct LogicalSizeCounter {
    bytes: u64,
}

impl std::io::Write for LogicalSizeCounter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.bytes = self.bytes.saturating_add(row_count(buffer.len()));
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn logical_size<T: Serialize + ?Sized>(value: &T) -> u64 {
    let mut counter = LogicalSizeCounter::default();
    serde_json::to_writer(&mut counter, value).expect("normalized semantic rows serialize");
    counter.bytes
}

const fn logical_ascii_struct_overhead(fields: &[&str]) -> u64 {
    if fields.is_empty() {
        return 2;
    }
    let mut total = (fields.len() as u64).saturating_mul(2).saturating_add(1);
    let mut index = 0;
    while index < fields.len() {
        total = total
            .saturating_add(fields[index].len() as u64)
            .saturating_add(2);
        index += 1;
    }
    total
}

fn logical_array_size(element_count: usize, element_bytes: u64) -> u64 {
    2_u64
        .saturating_add(row_count(element_count.saturating_sub(1)))
        .saturating_add(element_bytes)
}

fn logical_u64_size(value: u64) -> u64 {
    if value == 0 {
        1
    } else {
        u64::from(value.ilog10()).saturating_add(1)
    }
}

fn logical_digest_size(digest: &Digest) -> u64 {
    const OVERHEAD: u64 = logical_ascii_struct_overhead(&["kind", "value"]);
    OVERHEAD
        .saturating_add(logical_json_string_size(digest.kind.label()))
        .saturating_add(logical_json_string_size(&digest.value))
}

fn logical_digest_array_size(digests: &[Digest]) -> u64 {
    logical_array_size(
        digests.len(),
        digests.iter().fold(0_u64, |total, digest| {
            total.saturating_add(logical_digest_size(digest))
        }),
    )
}

fn logical_layer_key_size(key: &LayerKey) -> u64 {
    const OVERHEAD: u64 = logical_ascii_struct_overhead(&[
        "layer_kind",
        "provider_id",
        "provider_version",
        "schema_version",
        "parameter_digest",
        "lifecycle_digest",
        "analysis_settings_digest",
        "toolchain_digest",
        "input_digests",
        "dependency_layer_digests",
        "extension_digests",
    ]);
    OVERHEAD
        .saturating_add(logical_json_string_size(key.layer_kind.label()))
        .saturating_add(logical_json_string_size(&key.provider_id))
        .saturating_add(logical_json_string_size(&key.provider_version))
        .saturating_add(logical_json_string_size(&key.schema_version))
        .saturating_add(logical_digest_size(&key.parameter_digest))
        .saturating_add(logical_digest_size(&key.lifecycle_digest))
        .saturating_add(logical_digest_size(&key.analysis_settings_digest))
        .saturating_add(logical_digest_size(&key.toolchain_digest))
        .saturating_add(logical_digest_array_size(&key.input_digests))
        .saturating_add(logical_digest_array_size(&key.dependency_layer_digests))
        .saturating_add(logical_digest_array_size(&key.extension_digests))
}

fn logical_layer_row_size(row: &StoreLayerRow, key_size: u64) -> u64 {
    const OVERHEAD: u64 = logical_ascii_struct_overhead(&[
        "key",
        "output_digest",
        "payload_digest",
        "precision",
        "validation",
        "input_count",
        "dependency_layer_count",
        "extension_count",
        "edge_count",
        "warning_count",
    ]);
    OVERHEAD
        .saturating_add(key_size)
        .saturating_add(logical_digest_size(&row.output_digest))
        .saturating_add(logical_digest_size(&row.payload_digest))
        .saturating_add(logical_json_string_size(&row.precision))
        .saturating_add(logical_json_string_size(&row.validation))
        .saturating_add(logical_u64_size(row.input_count))
        .saturating_add(logical_u64_size(row.dependency_layer_count))
        .saturating_add(logical_u64_size(row.extension_count))
        .saturating_add(logical_u64_size(row.edge_count))
        .saturating_add(logical_u64_size(row.warning_count))
}

fn logical_input_dependency_size(input: &InputDependencyKey) -> u64 {
    const OVERHEAD: u64 =
        logical_ascii_struct_overhead(&["kind", "stable_key", "digest", "status"]);
    OVERHEAD
        .saturating_add(logical_json_string_size(input.kind.label()))
        .saturating_add(logical_json_string_size(&input.stable_key))
        .saturating_add(logical_digest_size(&input.digest))
        .saturating_add(logical_json_string_size(input.status.label()))
}

fn logical_query_key_size(key: &QueryKey) -> u64 {
    const OVERHEAD: u64 = logical_ascii_struct_overhead(&[
        "query_kind",
        "query_version",
        "parameter_digest",
        "dependency_inputs",
        "layer_digests",
        "budget_digest",
        "precision_tier",
    ]);
    let input_bytes = key
        .dependency_inputs
        .as_slice()
        .iter()
        .fold(0_u64, |total, input| {
            total.saturating_add(logical_input_dependency_size(input))
        });
    OVERHEAD
        .saturating_add(logical_json_string_size(&key.query_kind))
        .saturating_add(logical_json_string_size(&key.query_version))
        .saturating_add(logical_digest_size(&key.parameter_digest))
        .saturating_add(logical_array_size(
            key.dependency_inputs.as_slice().len(),
            input_bytes,
        ))
        .saturating_add(logical_digest_array_size(&key.layer_digests))
        .saturating_add(logical_digest_size(&key.budget_digest))
        .saturating_add(logical_json_string_size(key.precision_tier.label()))
}

fn logical_query_row_size(row: &StoreQueryRow, key_size: u64) -> u64 {
    const OVERHEAD: u64 = logical_ascii_struct_overhead(&[
        "key",
        "result_digest",
        "precision",
        "provenance",
        "input_count",
        "layer_count",
        "edge_count",
    ]);
    OVERHEAD
        .saturating_add(key_size)
        .saturating_add(logical_digest_size(&row.result_digest))
        .saturating_add(logical_json_string_size(&row.precision))
        .saturating_add(logical_json_string_size(&row.provenance))
        .saturating_add(logical_u64_size(row.input_count))
        .saturating_add(logical_u64_size(row.layer_count))
        .saturating_add(logical_u64_size(row.edge_count))
}

fn logical_parented_rows_size_from_keys<R>(
    rows: &[R],
    value_field: &str,
    key_sizes: &[u64],
    parent_ordinal: impl Fn(&R) -> u64,
    value_size: impl Fn(&R) -> u64,
) -> u64 {
    let row_overhead = logical_ascii_struct_overhead(&["key", value_field]);
    let element_bytes = rows.iter().fold(0_u64, |total, row| {
        let key_size = usize::try_from(parent_ordinal(row))
            .ok()
            .and_then(|ordinal| key_sizes.get(ordinal))
            .copied()
            .expect("normalized child row references a retained parent");
        total
            .saturating_add(row_overhead)
            .saturating_add(key_size)
            .saturating_add(value_size(row))
    });
    logical_array_size(rows.len(), element_bytes)
}

fn logical_layer_family_size(plan: &StoreSemanticPlan) -> u64 {
    let key_sizes = plan
        .layers
        .iter()
        .map(|row| logical_layer_key_size(&row.key))
        .collect::<Vec<_>>();
    let layer_bytes = plan
        .layers
        .iter()
        .zip(&key_sizes)
        .fold(0_u64, |total, (row, key_size)| {
            total.saturating_add(logical_layer_row_size(row, *key_size))
        });
    logical_array_size(plan.layers.len(), layer_bytes)
        .saturating_add(logical_parented_rows_size_from_keys(
            &plan.layer_inputs,
            "digest",
            &key_sizes,
            |row| row.layer_ordinal,
            |row| logical_digest_size(&row.digest),
        ))
        .saturating_add(logical_parented_rows_size_from_keys(
            &plan.layer_dependencies,
            "digest",
            &key_sizes,
            |row| row.layer_ordinal,
            |row| logical_digest_size(&row.digest),
        ))
        .saturating_add(logical_parented_rows_size_from_keys(
            &plan.layer_extensions,
            "digest",
            &key_sizes,
            |row| row.layer_ordinal,
            |row| logical_digest_size(&row.digest),
        ))
        .saturating_add(logical_parented_rows_size_from_keys(
            &plan.layer_warnings,
            "warning_code",
            &key_sizes,
            |row| row.layer_ordinal,
            |row| logical_json_string_size(&row.warning_code),
        ))
}

fn logical_query_family_size(plan: &StoreSemanticPlan) -> u64 {
    let key_sizes = plan
        .queries
        .iter()
        .map(|row| logical_query_key_size(&row.key))
        .collect::<Vec<_>>();
    let query_bytes = plan
        .queries
        .iter()
        .zip(&key_sizes)
        .fold(0_u64, |total, (row, key_size)| {
            total.saturating_add(logical_query_row_size(row, *key_size))
        });
    logical_array_size(plan.queries.len(), query_bytes)
        .saturating_add(logical_parented_rows_size_from_keys(
            &plan.query_inputs,
            "input",
            &key_sizes,
            |row| row.query_ordinal,
            |row| logical_input_dependency_size(&row.input),
        ))
        .saturating_add(logical_parented_rows_size_from_keys(
            &plan.query_layers,
            "layer_digest",
            &key_sizes,
            |row| row.query_ordinal,
            |row| logical_digest_size(&row.layer_digest),
        ))
}

fn logical_rows_size<T: Serialize>(rows: &[T]) -> u64 {
    rows.iter()
        .fold(0, |total, row| total.saturating_add(logical_size(row)))
}

fn logical_parented_rows_size<R, P, K, V>(
    rows: &[R],
    value_field: &str,
    parents: &[P],
    parent_key: impl Fn(&P) -> &K,
    parent_ordinal: impl Fn(&R) -> u64,
    value_of: impl Fn(&R) -> &V,
) -> u64
where
    K: Serialize,
    V: Serialize,
{
    let array_punctuation = 2_u64.saturating_add(row_count(rows.len().saturating_sub(1)));
    let row_punctuation = 5_u64
        .saturating_add(logical_size("key"))
        .saturating_add(logical_size(value_field));
    let key_sizes = parents
        .iter()
        .map(|parent| logical_size(parent_key(parent)))
        .collect::<Vec<_>>();
    rows.iter().fold(array_punctuation, |total, row| {
        let key_size = usize::try_from(parent_ordinal(row))
            .ok()
            .and_then(|ordinal| key_sizes.get(ordinal))
            .copied()
            .expect("normalized child row references a retained parent");
        total
            .saturating_add(row_punctuation)
            .saturating_add(key_size)
            .saturating_add(logical_size(value_of(row)))
    })
}

fn logical_parented_individual_rows_size<R, P, K, V>(
    rows: &[R],
    value_field: &str,
    parents: &[P],
    parent_key: impl Fn(&P) -> &K,
    parent_ordinal: impl Fn(&R) -> u64,
    value_of: impl Fn(&R) -> &V,
) -> u64
where
    K: Serialize,
    V: Serialize,
{
    let array_punctuation = 2_u64.saturating_add(row_count(rows.len().saturating_sub(1)));
    logical_parented_rows_size(
        rows,
        value_field,
        parents,
        parent_key,
        parent_ordinal,
        value_of,
    )
    .saturating_sub(array_punctuation)
}

fn logical_dependency_rows_size(rows: &[StoreDependencyEdgeRow]) -> u64 {
    let array_punctuation = 2_u64.saturating_add(row_count(rows.len().saturating_sub(1)));
    array_punctuation.saturating_add(
        rows.par_iter()
            .map(logical_size)
            .reduce(|| 0, u64::saturating_add),
    )
}

fn logical_fact_rows_size(rows: &[StoreFactRow]) -> u64 {
    let array_punctuation = 2_u64.saturating_add(row_count(rows.len().saturating_sub(1)));
    let field_names = [
        "family",
        "stable_key",
        "producer_id",
        "layer_id",
        "precision",
        "confidence",
        "validation",
        "payload_digest",
    ]
    .into_iter()
    .fold(0_u64, |total, field| {
        total.saturating_add(logical_size(field))
    });
    let row_punctuation = 2_u64
        .saturating_add(8)
        .saturating_add(7)
        .saturating_add(field_names);
    array_punctuation.saturating_add(
        rows.par_iter()
            .map(|row| {
                row_punctuation
                    .saturating_add(logical_json_string_size(row.family.label()))
                    .saturating_add(row.stable_key.json_bytes())
                    .saturating_add(logical_json_string_size(&row.producer_id))
                    .saturating_add(logical_json_string_size(&row.layer_id))
                    .saturating_add(logical_json_string_size(row.precision.label()))
                    .saturating_add(logical_json_string_size(row.confidence.label()))
                    .saturating_add(logical_json_string_size(row.validation.label()))
                    .saturating_add(logical_json_string_size(&row.payload_digest))
            })
            .reduce(|| 0, u64::saturating_add),
    )
}

fn logical_json_string_size(value: &str) -> u64 {
    value.as_bytes().iter().fold(2_u64, |total, byte| {
        total.saturating_add(match byte {
            b'"' | b'\\' | b'\x08' | b'\x0c' | b'\n' | b'\r' | b'\t' => 2,
            0..=0x1f => 6,
            _ => 1,
        })
    })
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
            query_key: query_key.into(),
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
    fn optimized_logical_sizes_match_serialized_byte_lengths() {
        let plan = plan_fixture();
        let dependency_bytes =
            serde_json::to_vec(&plan.semantic.dependency_edges).expect("dependency rows serialize");

        assert_eq!(
            logical_size(&plan.semantic.dependency_edges),
            row_count(dependency_bytes.len())
        );
        assert_eq!(
            logical_dependency_rows_size(&plan.semantic.dependency_edges),
            logical_size(&plan.semantic.dependency_edges)
        );
        assert_eq!(
            logical_fact_rows_size(&plan.semantic.facts),
            logical_size(&plan.semantic.facts)
        );
        let serialized_layer_family = logical_size(&plan.semantic.layers)
            .saturating_add(logical_parented_rows_size(
                &plan.semantic.layer_inputs,
                "digest",
                &plan.semantic.layers,
                |row| &row.key,
                |row| row.layer_ordinal,
                |row| &row.digest,
            ))
            .saturating_add(logical_parented_rows_size(
                &plan.semantic.layer_dependencies,
                "digest",
                &plan.semantic.layers,
                |row| &row.key,
                |row| row.layer_ordinal,
                |row| &row.digest,
            ))
            .saturating_add(logical_parented_rows_size(
                &plan.semantic.layer_extensions,
                "digest",
                &plan.semantic.layers,
                |row| &row.key,
                |row| row.layer_ordinal,
                |row| &row.digest,
            ))
            .saturating_add(logical_parented_rows_size(
                &plan.semantic.layer_warnings,
                "warning_code",
                &plan.semantic.layers,
                |row| &row.key,
                |row| row.layer_ordinal,
                |row| &row.warning_code,
            ));
        assert_eq!(
            logical_layer_family_size(&plan.semantic),
            serialized_layer_family
        );
        let serialized_query_family = logical_size(&plan.semantic.queries)
            .saturating_add(logical_parented_rows_size(
                &plan.semantic.query_inputs,
                "input",
                &plan.semantic.queries,
                |row| &row.key,
                |row| row.query_ordinal,
                |row| &row.input,
            ))
            .saturating_add(logical_parented_rows_size(
                &plan.semantic.query_layers,
                "layer_digest",
                &plan.semantic.queries,
                |row| &row.key,
                |row| row.query_ordinal,
                |row| &row.layer_digest,
            ));
        assert_eq!(
            logical_query_family_size(&plan.semantic),
            serialized_query_family
        );
        assert_eq!(
            logical_parented_rows_size(
                &plan.semantic.layer_inputs,
                "digest",
                &plan.semantic.layers,
                |row| &row.key,
                |row| row.layer_ordinal,
                |row| &row.digest,
            ),
            logical_size(
                &plan
                    .semantic
                    .layer_inputs
                    .iter()
                    .map(|row| serde_json::json!({
                        "key": plan.semantic.layers[row.layer_ordinal as usize].key,
                        "digest": row.digest,
                    }))
                    .collect::<Vec<_>>()
            )
        );
        assert_eq!(
            logical_parented_rows_size(
                &plan.semantic.layer_dependencies,
                "digest",
                &plan.semantic.layers,
                |row| &row.key,
                |row| row.layer_ordinal,
                |row| &row.digest,
            ),
            logical_size(
                &plan
                    .semantic
                    .layer_dependencies
                    .iter()
                    .map(|row| serde_json::json!({
                        "key": plan.semantic.layers[row.layer_ordinal as usize].key,
                        "digest": row.digest,
                    }))
                    .collect::<Vec<_>>()
            )
        );
        assert_eq!(
            logical_parented_rows_size(
                &plan.semantic.layer_extensions,
                "digest",
                &plan.semantic.layers,
                |row| &row.key,
                |row| row.layer_ordinal,
                |row| &row.digest,
            ),
            logical_size(
                &plan
                    .semantic
                    .layer_extensions
                    .iter()
                    .map(|row| serde_json::json!({
                        "key": plan.semantic.layers[row.layer_ordinal as usize].key,
                        "digest": row.digest,
                    }))
                    .collect::<Vec<_>>()
            )
        );
        assert_eq!(
            logical_parented_rows_size(
                &plan.semantic.layer_warnings,
                "warning_code",
                &plan.semantic.layers,
                |row| &row.key,
                |row| row.layer_ordinal,
                |row| &row.warning_code,
            ),
            logical_size(
                &plan
                    .semantic
                    .layer_warnings
                    .iter()
                    .map(|row| serde_json::json!({
                        "key": plan.semantic.layers[row.layer_ordinal as usize].key,
                        "warning_code": row.warning_code,
                    }))
                    .collect::<Vec<_>>()
            )
        );
        assert_eq!(
            logical_parented_rows_size(
                &plan.semantic.summary_dependencies,
                "dependency",
                &plan.semantic.summaries,
                |row| &row.key,
                |row| row.summary_ordinal,
                |row| &row.dependency,
            ),
            logical_size(
                &plan
                    .semantic
                    .summary_dependencies
                    .iter()
                    .map(|row| serde_json::json!({
                        "key": plan.semantic.summaries[row.summary_ordinal as usize].key,
                        "dependency": row.dependency,
                    }))
                    .collect::<Vec<_>>()
            )
        );
        assert_eq!(
            logical_parented_rows_size(
                &plan.semantic.query_inputs,
                "input",
                &plan.semantic.queries,
                |row| &row.key,
                |row| row.query_ordinal,
                |row| &row.input,
            ),
            logical_size(
                &plan
                    .semantic
                    .query_inputs
                    .iter()
                    .map(|row| serde_json::json!({
                        "key": plan.semantic.queries[row.query_ordinal as usize].key,
                        "input": row.input,
                    }))
                    .collect::<Vec<_>>()
            )
        );
        assert_eq!(
            logical_parented_rows_size(
                &plan.semantic.query_layers,
                "layer_digest",
                &plan.semantic.queries,
                |row| &row.key,
                |row| row.query_ordinal,
                |row| &row.layer_digest,
            ),
            logical_size(
                &plan
                    .semantic
                    .query_layers
                    .iter()
                    .map(|row| serde_json::json!({
                        "key": plan.semantic.queries[row.query_ordinal as usize].key,
                        "layer_digest": row.layer_digest,
                    }))
                    .collect::<Vec<_>>()
            )
        );
    }

    #[test]
    fn complete_handoff_normalizes_every_semantic_family() {
        let plan = plan_fixture();
        plan.validate().expect("complete plan validates");
        let mut independently_sorted_dependencies = plan.semantic.dependency_edges.clone();
        independently_sorted_dependencies.sort();

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
            plan.semantic.dependency_edges, independently_sorted_dependencies,
            "CacheNode-to-ordinal projection preserves canonical edge order"
        );
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
        let edge_position = candidate
            .semantic
            .dependency_edges
            .iter()
            .position(|edge| edge.from == StoreNodeRef::Query(0))
            .expect("query has dependency edges");
        candidate.semantic.dependency_edges.remove(edge_position);
        assert!(matches!(
            candidate.validate(),
            Err(StorePlanError::DanglingQueryEndpoint { .. })
        ));

        let mut candidate = baseline.clone();
        let input = candidate.semantic.query_inputs[0].input.clone();
        candidate
            .semantic
            .dependency_edges
            .push(StoreDependencyEdgeRow {
                from: StoreNodeRef::DependencyInput(input),
                to: StoreNodeRef::Summary(u64::MAX),
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
    fn input_and_summary_children_must_match_their_authenticated_parents() {
        let baseline = plan_fixture();
        assert!(!baseline.semantic.input_details.is_empty());
        assert!(!baseline.semantic.capability_requesters.is_empty());

        let mut candidate = baseline.clone();
        candidate.semantic.input_components[0].detail_count = candidate.semantic.input_components
            [0]
        .detail_count
        .saturating_add(1);
        assert!(matches!(
            candidate.validate(),
            Err(StorePlanError::CountMismatch {
                family: "input component detail",
                ..
            })
        ));

        let mut candidate = baseline.clone();
        let replacement = if candidate.semantic.input_details[0]
            .component_digest
            .value
            .starts_with('f')
        {
            "e"
        } else {
            "f"
        };
        candidate.semantic.input_details[0]
            .component_digest
            .value
            .replace_range(..1, replacement);
        assert!(matches!(
            candidate.validate(),
            Err(StorePlanError::IdentityMismatch {
                family: "input component detail"
            })
        ));

        let mut candidate = baseline;
        candidate.semantic.capabilities[0].requester_count = candidate.semantic.capabilities[0]
            .requester_count
            .saturating_add(1);
        assert!(matches!(
            candidate.validate(),
            Err(StorePlanError::CountMismatch {
                family: "capability requester",
                ..
            })
        ));
    }

    #[test]
    fn summary_children_reject_same_count_reassignment() {
        let mut plan = plan_fixture();
        let first_dependency = Digest::from_parts(
            DigestKind::SummaryDependency,
            "summary dependency",
            &["first"],
        );
        let second_dependency = Digest::from_parts(
            DigestKind::SummaryDependency,
            "summary dependency",
            &["second"],
        );
        plan.semantic.summaries = vec![
            StoreSummaryRow {
                key: SummaryKey::new(
                    "callable:first",
                    "effects",
                    "1",
                    Digest::from_parts(DigestKind::SummaryBody, "summary body", &["first"]),
                    vec![first_dependency],
                    Digest::absent(DigestKind::ExtensionCode, "summary extension"),
                ),
                dependency_count: 1,
            },
            StoreSummaryRow {
                key: SummaryKey::new(
                    "callable:second",
                    "effects",
                    "1",
                    Digest::from_parts(DigestKind::SummaryBody, "summary body", &["second"]),
                    vec![second_dependency],
                    Digest::absent(DigestKind::ExtensionCode, "summary extension"),
                ),
                dependency_count: 1,
            },
        ];
        plan.semantic.summaries.sort();
        plan.semantic.summary_dependencies = plan
            .semantic
            .summaries
            .iter()
            .enumerate()
            .map(|(ordinal, summary)| StoreSummaryDependencyRow {
                summary_ordinal: row_count(ordinal),
                dependency: summary.key.dependency_summary_digests[0].clone(),
            })
            .collect();
        plan.semantic.summary_dependencies.sort();
        assert_eq!(plan.semantic.validate_summary_relationships(), Ok(()));

        let first_ordinal = plan.semantic.summary_dependencies[0].summary_ordinal;
        plan.semantic.summary_dependencies[0].summary_ordinal =
            plan.semantic.summary_dependencies[1].summary_ordinal;
        plan.semantic.summary_dependencies[1].summary_ordinal = first_ordinal;
        plan.semantic.summary_dependencies.sort();

        assert!(matches!(
            plan.semantic.validate_summary_relationships(),
            Err(StorePlanError::NonCanonicalRows {
                family: "summary dependency"
            })
        ));
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
            .split_once("#[cfg(test)]\nmod tests")
            .expect("test boundary exists")
            .0;
        assert!(production.contains("pub(super) struct StoreCommitPlan"));
        assert!(production.contains("pub(super) struct ValidatedStoreCommitPlan"));
        assert!(production.contains("pub(super) enum StorePlanError"));
        assert!(production.contains("pub(super) struct StoreGenerationStats"));
        assert!(production.contains("fn from_validated_run"));
        assert!(production.contains("fn from_owned_validated_run"));
        assert!(production.contains("validate_without_stats_with_canonical_fact_proof"));
        assert!(production.contains("validated: &ValidatedRunMetadata"));
        assert!(production.contains("validate_integrity()"));
        assert!(!production.contains("from_owned_prevalidated_run"));
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

        let parent = include_str!("../mod.rs")
            .split_once("\n#[cfg(test)]\nmod tests")
            .expect("kernel test boundary exists")
            .0;
        assert!(!parent.contains("StoreCommitPlan"));
        assert!(!parent.contains("from_validated_run"));

        let generation = include_str!("generation.rs");
        assert!(generation.contains("plan: ValidatedStoreCommitPlan"));
        assert!(!generation.contains("validate_plan"));
        assert!(!generation.contains("prevalidated"));
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

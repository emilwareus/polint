//! Complete-generation publication and active-generation reads.

#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "generation publication and active reads are private to kernel store integration"
    )
)]

use std::collections::BTreeMap;
use std::sync::Arc;

use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};

use crate::analysis_kernel::incremental::{
    CacheNode, ConfigIdentity, DependencyEdge, DependencyIndex, Digest, DigestKind,
    GenerationIdentity, InputDependencyKey, LayerKey, QueryDependencyInputs, QueryKey, RunIdentity,
    SummaryKey, WorkspaceIdentity,
};

use super::commit_plan::{
    StoreAnalysisSettingRow, StoreCapabilityRequesterRow, StoreCapabilityRow, StoreCommitPlan,
    StoreDependencyEdgeRow, StoreFactRow, StoreFileRow, StoreGenerationStats, StoreIdentityRow,
    StoreInputComponentRow, StoreInputDetailRow, StoreInputSnapshotRow, StoreLayerDependencyRow,
    StoreLayerExtensionRow, StoreLayerInputRow, StoreLayerRow, StoreLayerWarningRow,
    StoreProviderDependencyRow, StoreProviderGenerationRow, StoreProviderManifestInputRow,
    StoreProviderManifestOutputRow, StoreProviderManifestRow, StoreProviderManifestSchemaRow,
    StoreProviderSchemaRow, StoreProviderSchemaVersionRow, StoreQueryInputRow, StoreQueryLayerRow,
    StoreQueryRow, StoreSemanticPlan, StoreSummaryDependencyRow, StoreSummaryRow,
    StoreTelemetryRow, StoreValidationEventRow,
};
use super::connection::{self, ConnectionError, ReadOnlyConnection, WriterConnection};
use super::schema::{
    EncodedInputDependency, GenerationFailureEvent, GenerationFailureReason,
    GenerationFailureStage, GenerationStatus, SchemaCodecError, decode_analysis_settings_scope,
    decode_cache_node_kind, decode_cache_policy, decode_capability_setup_status,
    decode_capability_support_status, decode_dependency_kind, decode_digest,
    decode_fact_confidence, decode_fact_family, decode_fact_precision,
    decode_input_component_status, decode_input_dependency, decode_input_group, decode_language,
    decode_language_scope, decode_layer_kind, decode_precision_ceiling, decode_precision_tier,
    decode_provider_kind, decode_provider_validation_status, decode_shape_kind,
    decode_validation_event_kind, decode_validation_event_status, decode_validation_status,
    encode_input_group, validate_dependency_schema, validate_input_snapshot_schema,
    validate_relative_path,
};
use super::{
    StoreConfig, StoreRebuildReason, StoreSkipReason, StoreStatus, map_connection_error,
    prepare_store_path,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ActiveCompleteGeneration {
    pub(super) plan: StoreCommitPlan,
    pub(super) dependency_index: DependencyIndex,
}

#[derive(Clone, Debug)]
struct Reservation {
    handle: i64,
    reservation_ordinal: i64,
    prior_active_generation: Option<i64>,
    identities: StoreIdentityRow,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct CommitControl {
    fail_after: Option<GenerationFailureStage>,
    #[cfg(test)]
    reservation_interlock: Option<&'static ReservationInterlock>,
    #[cfg(test)]
    publication_interlock: Option<&'static ReservationInterlock>,
    #[cfg(test)]
    provider_child_tamper: Option<ProviderChildTamper>,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(super) enum ProviderChildTamper {
    Missing,
    Mismatched,
}

#[cfg(test)]
#[derive(Debug)]
pub(super) struct ReservationInterlock {
    pub(super) entered: std::sync::Barrier,
    pub(super) release: std::sync::Barrier,
}

#[cfg(test)]
impl ReservationInterlock {
    pub(super) fn new() -> Self {
        Self {
            entered: std::sync::Barrier::new(2),
            release: std::sync::Barrier::new(2),
        }
    }
}

impl CommitControl {
    #[cfg(test)]
    pub(super) const fn fail_after(stage: GenerationFailureStage) -> Self {
        Self {
            fail_after: Some(stage),
            reservation_interlock: None,
            publication_interlock: None,
            provider_child_tamper: None,
        }
    }

    #[cfg(test)]
    pub(super) const fn with_reservation_interlock(
        interlock: &'static ReservationInterlock,
    ) -> Self {
        Self {
            fail_after: None,
            reservation_interlock: Some(interlock),
            publication_interlock: None,
            provider_child_tamper: None,
        }
    }

    #[cfg(test)]
    pub(super) const fn with_publication_interlock(
        interlock: &'static ReservationInterlock,
    ) -> Self {
        Self {
            fail_after: None,
            reservation_interlock: None,
            publication_interlock: Some(interlock),
            provider_child_tamper: None,
        }
    }

    #[cfg(test)]
    pub(super) const fn with_provider_child_tamper(tamper: ProviderChildTamper) -> Self {
        Self {
            fail_after: None,
            reservation_interlock: None,
            publication_interlock: None,
            provider_child_tamper: Some(tamper),
        }
    }
}

#[derive(Clone, Debug)]
enum ReservationError {
    Connection(ConnectionError),
    WorkspaceMismatch,
    InvalidMetadata,
    Injected,
}

#[derive(Debug)]
enum ReservationValidationError {
    Stale,
    Untrusted,
}

impl From<ProjectionError> for ReservationValidationError {
    fn from(_error: ProjectionError) -> Self {
        Self::Untrusted
    }
}

#[derive(Clone, Debug)]
struct PublicationError {
    status: StoreStatus,
    audit: Option<(GenerationFailureReason, GenerationFailureStage)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AuditOutcome {
    Recorded,
    Busy,
    NotTrusted,
}

#[derive(Debug)]
enum ProjectionError {
    Connection(ConnectionError),
    Sqlite(rusqlite::Error),
    Codec,
    WorkspaceMismatch,
    InvalidMetadata,
}

impl From<rusqlite::Error> for ProjectionError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<SchemaCodecError> for ProjectionError {
    fn from(_error: SchemaCodecError) -> Self {
        Self::Codec
    }
}

pub(super) fn commit_generation(config: &StoreConfig, plan: &StoreCommitPlan) -> StoreStatus {
    commit_generation_with_control(config, plan, CommitControl::default())
}

pub(super) fn commit_generation_with_control(
    config: &StoreConfig,
    plan: &StoreCommitPlan,
    control: CommitControl,
) -> StoreStatus {
    if !config.is_enabled() {
        return StoreStatus::Disabled;
    }
    if plan.validate().is_err() {
        return StoreStatus::Skipped(StoreSkipReason::InvalidPlan);
    }
    if let Err(status) = prepare_store_path(config.path()) {
        return status;
    }

    let mut writer = match connection::open_writer(config.path()) {
        Ok(writer) => writer,
        Err(error) => return map_connection_error(error),
    };
    let reservation = match reserve_generation(&mut writer, plan, &control) {
        Ok(reservation) => reservation,
        Err(ReservationError::Connection(error)) => return map_connection_error(error),
        Err(ReservationError::WorkspaceMismatch) => {
            return StoreStatus::Skipped(StoreSkipReason::WorkspaceMismatch);
        }
        Err(ReservationError::InvalidMetadata) => {
            return StoreStatus::RebuildNeeded(StoreRebuildReason::InvalidMetadata);
        }
        Err(ReservationError::Injected) => {
            return StoreStatus::Skipped(StoreSkipReason::CommitFailed);
        }
    };

    #[cfg(test)]
    if let Some(interlock) = control.publication_interlock {
        interlock.entered.wait();
        interlock.release.wait();
    }

    match publish_generation(&mut writer, plan, &reservation, &control) {
        Ok(()) => StoreStatus::Ready,
        Err(error) => {
            if let Some((reason, stage)) = error.audit {
                let _ = audit_failed_attempt(&mut writer, &reservation, reason, stage);
            }
            error.status
        }
    }
}

pub(super) fn read_active_complete(
    config: &StoreConfig,
    workspace: &WorkspaceIdentity,
) -> Result<Option<ActiveCompleteGeneration>, StoreStatus> {
    if !config.is_enabled() {
        return Err(StoreStatus::Disabled);
    }
    let reader = connection::open_read_only(config.path()).map_err(map_connection_error)?;
    read_active_complete_from_reader(&reader, workspace).map_err(map_read_error)
}

fn reserve_generation(
    writer: &mut WriterConnection,
    plan: &StoreCommitPlan,
    control: &CommitControl,
) -> Result<Reservation, ReservationError> {
    plan.validate()
        .map_err(|_| ReservationError::InvalidMetadata)?;
    let transaction = connection::begin_immediate(writer).map_err(ReservationError::Connection)?;
    connection::validate_schema(&transaction).map_err(ReservationError::Connection)?;

    #[cfg(test)]
    if let Some(interlock) = control.reservation_interlock {
        interlock.entered.wait();
        interlock.release.wait();
    }

    let workspace = &plan.semantic.identities.workspace;
    let manifest = read_manifest(&transaction).map_err(map_reservation_projection_error)?;
    let prior_active_generation = match manifest {
        ManifestRow {
            workspace: None,
            active_generation: None,
        } => {
            let changed = transaction
                .execute(
                    "UPDATE store_manifest
                     SET workspace_kind = ?1, workspace_value = ?2
                     WHERE id = 1 AND workspace_kind IS NULL AND workspace_value IS NULL
                       AND active_generation_id IS NULL",
                    params![workspace.digest().kind.label(), workspace.digest().value],
                )
                .map_err(|error| {
                    ReservationError::Connection(connection::classify_sqlite_error(error))
                })?;
            if changed != 1 {
                return Err(ReservationError::InvalidMetadata);
            }
            None
        }
        ManifestRow {
            workspace: Some(bound),
            active_generation,
        } if bound == *workspace => active_generation,
        ManifestRow {
            workspace: Some(_), ..
        } => return Err(ReservationError::WorkspaceMismatch),
        ManifestRow { .. } => return Err(ReservationError::InvalidMetadata),
    };

    let reservation_ordinal = next_reservation_ordinal(&transaction, &plan.semantic.identities)
        .map_err(map_reservation_projection_error)?;
    let identities = &plan.semantic.identities;
    let inserted = transaction
        .execute(
            "INSERT INTO generations (
                 reservation_ordinal,
                 workspace_kind, workspace_value,
                 generation_kind, generation_value,
                 run_kind, run_value,
                 full_config_kind, full_config_value,
                 input_snapshot_kind, input_snapshot_value,
                 provider_manifest_kind, provider_manifest_value,
                 provider_output_kind, provider_output_value,
                 layer_kind, layer_value,
                 summary_kind, summary_value,
                 query_kind, query_value,
                 fact_kind, fact_value,
                 dependency_kind, dependency_value,
                 validation_kind, validation_value,
                 dependency_schema, status
             ) VALUES (
                 ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                 ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
                 ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29
             )",
            params![
                reservation_ordinal,
                identities.workspace.digest().kind.label(),
                identities.workspace.digest().value,
                identities.generation.digest().kind.label(),
                identities.generation.digest().value,
                identities.run.digest().kind.label(),
                identities.run.digest().value,
                identities.full_config.digest().kind.label(),
                identities.full_config.digest().value,
                identities.input_snapshot.kind.label(),
                identities.input_snapshot.value,
                identities.provider_manifest.kind.label(),
                identities.provider_manifest.value,
                identities.provider_output.kind.label(),
                identities.provider_output.value,
                identities.layer.kind.label(),
                identities.layer.value,
                identities.summary.kind.label(),
                identities.summary.value,
                identities.query.kind.label(),
                identities.query.value,
                identities.fact.kind.label(),
                identities.fact.value,
                identities.dependency.kind.label(),
                identities.dependency.value,
                identities.validation.kind.label(),
                identities.validation.value,
                plan.semantic.dependency_schema,
                GenerationStatus::Pending.label(),
            ],
        )
        .map_err(|error| ReservationError::Connection(connection::classify_sqlite_error(error)))?;
    if inserted != 1 {
        return Err(ReservationError::InvalidMetadata);
    }
    let handle = transaction.last_insert_rowid();

    if control.fail_after == Some(GenerationFailureStage::Reservation) {
        return Err(ReservationError::Injected);
    }
    connection::validate_schema(&transaction).map_err(ReservationError::Connection)?;
    transaction
        .commit()
        .map_err(|error| ReservationError::Connection(connection::classify_sqlite_error(error)))?;

    Ok(Reservation {
        handle,
        reservation_ordinal,
        prior_active_generation,
        identities: identities.clone(),
    })
}

fn publish_generation(
    writer: &mut WriterConnection,
    plan: &StoreCommitPlan,
    reservation: &Reservation,
    control: &CommitControl,
) -> Result<(), PublicationError> {
    let transaction = connection::begin_immediate(writer).map_err(|error| {
        publication_connection_error(error, GenerationFailureStage::StoreRunInput, true)
    })?;
    connection::validate_schema(&transaction).map_err(|error| {
        publication_connection_error(error, GenerationFailureStage::StoreRunInput, false)
    })?;
    match validate_reservation(&transaction, reservation, plan) {
        Ok(()) => {}
        Err(ReservationValidationError::Stale) => {
            return Err(PublicationError {
                status: StoreStatus::Skipped(StoreSkipReason::StaleReservation),
                audit: None,
            });
        }
        Err(ReservationValidationError::Untrusted) => {
            return Err(PublicationError {
                status: StoreStatus::RebuildNeeded(StoreRebuildReason::InvalidMetadata),
                audit: None,
            });
        }
    }

    write_store_run_input(&transaction, reservation.handle, plan).map_err(|error| {
        publication_projection_error(error, GenerationFailureStage::StoreRunInput)
    })?;
    inject_failure(control, GenerationFailureStage::StoreRunInput)?;

    let _provider_handles = write_providers(&transaction, reservation.handle, plan)
        .map_err(|error| publication_projection_error(error, GenerationFailureStage::Providers))?;
    inject_failure(control, GenerationFailureStage::Providers)?;

    let product_handles =
        write_products(&transaction, reservation.handle, plan).map_err(|error| {
            publication_projection_error(error, GenerationFailureStage::LayersSummariesQueries)
        })?;
    inject_failure(control, GenerationFailureStage::LayersSummariesQueries)?;

    write_facts(&transaction, reservation.handle, &plan.semantic.facts).map_err(|error| {
        publication_projection_error(error, GenerationFailureStage::FactMetadata)
    })?;
    inject_failure(control, GenerationFailureStage::FactMetadata)?;

    write_dependency_edges(
        &transaction,
        reservation.handle,
        &plan.semantic.dependency_edges,
        &product_handles,
    )
    .map_err(|error| {
        publication_projection_error(error, GenerationFailureStage::DependencyEdges)
    })?;
    inject_failure(control, GenerationFailureStage::DependencyEdges)?;

    write_validation_events(
        &transaction,
        reservation.handle,
        &plan.semantic.validation_events,
    )
    .map_err(|error| {
        publication_projection_error(error, GenerationFailureStage::ValidationEvents)
    })?;
    inject_failure(control, GenerationFailureStage::ValidationEvents)?;

    write_stats(&transaction, reservation.handle, &plan.semantic.stats)
        .map_err(|error| publication_projection_error(error, GenerationFailureStage::Statistics))?;
    inject_failure(control, GenerationFailureStage::Statistics)?;

    #[cfg(test)]
    if let Some(tamper) = control.provider_child_tamper {
        tamper_provider_child(&transaction, reservation.handle, tamper).map_err(|error| {
            publication_projection_error(error, GenerationFailureStage::Providers)
        })?;
    }

    let projected = read_generation_projection(
        &transaction,
        reservation.handle,
        TelemetryReadPolicy::Strict,
    )
    .map_err(|error| publication_projection_error(error, GenerationFailureStage::Statistics))?;
    if projected.plan != *plan
        || projected.dependency_index.canonical_edges()
            != plan
                .semantic
                .dependency_edges
                .iter()
                .map(to_dependency_edge)
                .collect::<Vec<_>>()
    {
        return Err(PublicationError {
            status: StoreStatus::Skipped(StoreSkipReason::CommitFailed),
            audit: Some((
                GenerationFailureReason::PostWriteValidationFailed,
                GenerationFailureStage::Statistics,
            )),
        });
    }

    let completed = transaction
        .execute(
            "UPDATE generations SET status = ?1 WHERE id = ?2 AND status = ?3",
            params![
                GenerationStatus::Complete.label(),
                reservation.handle,
                GenerationStatus::Pending.label()
            ],
        )
        .map_err(|error| publication_sql_error(error, GenerationFailureStage::Completion))?;
    if completed != 1 {
        return Err(publication_integrity_error(
            GenerationFailureStage::Completion,
        ));
    }
    inject_failure(control, GenerationFailureStage::Completion)?;

    let activated = transaction
        .execute(
            "UPDATE store_manifest
             SET active_generation_id = ?1
             WHERE id = 1 AND workspace_kind = ?2 AND workspace_value = ?3
               AND active_generation_id IS ?4",
            params![
                reservation.handle,
                reservation.identities.workspace.digest().kind.label(),
                reservation.identities.workspace.digest().value,
                reservation.prior_active_generation,
            ],
        )
        .map_err(|error| publication_sql_error(error, GenerationFailureStage::Activation))?;
    if activated != 1 {
        return Err(PublicationError {
            status: StoreStatus::Skipped(StoreSkipReason::CommitFailed),
            audit: Some((
                GenerationFailureReason::WriteFailed,
                GenerationFailureStage::Activation,
            )),
        });
    }
    inject_failure(control, GenerationFailureStage::Activation)?;

    connection::validate_schema(&transaction).map_err(|error| {
        publication_connection_error(error, GenerationFailureStage::Activation, false)
    })?;
    let active =
        read_active_complete_from_connection(&transaction, &reservation.identities.workspace)
            .map_err(|error| {
                publication_projection_error(error, GenerationFailureStage::Activation)
            })?
            .ok_or_else(|| publication_integrity_error(GenerationFailureStage::Activation))?;
    if active.plan != *plan {
        return Err(PublicationError {
            status: StoreStatus::Skipped(StoreSkipReason::CommitFailed),
            audit: Some((
                GenerationFailureReason::PostWriteValidationFailed,
                GenerationFailureStage::Activation,
            )),
        });
    }
    inject_failure(control, GenerationFailureStage::TransactionCommit)?;

    transaction.commit().map_err(publication_commit_error)?;
    Ok(())
}

fn audit_failed_attempt(
    writer: &mut WriterConnection,
    reservation: &Reservation,
    reason: GenerationFailureReason,
    stage: GenerationFailureStage,
) -> AuditOutcome {
    let transaction = match connection::begin_immediate(writer) {
        Ok(transaction) => transaction,
        Err(ConnectionError::Busy) => return AuditOutcome::Busy,
        Err(_) => return AuditOutcome::NotTrusted,
    };
    if connection::validate_schema(&transaction).is_err()
        || validate_audit_target(&transaction, reservation).is_err()
    {
        return AuditOutcome::NotTrusted;
    }

    let failed = transaction.execute(
        "UPDATE generations SET status = ?1
         WHERE id = ?2 AND reservation_ordinal = ?3
           AND workspace_kind = ?4 AND workspace_value = ?5
           AND generation_kind = ?6 AND generation_value = ?7
           AND status = ?8",
        params![
            GenerationStatus::Failed.label(),
            reservation.handle,
            reservation.reservation_ordinal,
            reservation.identities.workspace.digest().kind.label(),
            reservation.identities.workspace.digest().value,
            reservation.identities.generation.digest().kind.label(),
            reservation.identities.generation.digest().value,
            GenerationStatus::Pending.label(),
        ],
    );
    if !matches!(failed, Ok(1)) {
        return AuditOutcome::NotTrusted;
    }
    let inserted = transaction.execute(
        "INSERT INTO generation_failure_events (
             generation_id, ordinal, event_code, reason_code, stage_code
         ) VALUES (?1, 0, ?2, ?3, ?4)",
        params![
            reservation.handle,
            GenerationFailureEvent::CommitAttemptFailed.label(),
            reason.label(),
            stage.label(),
        ],
    );
    if !matches!(inserted, Ok(1)) || connection::validate_schema(&transaction).is_err() {
        return AuditOutcome::NotTrusted;
    }
    if transaction.commit().is_ok() {
        AuditOutcome::Recorded
    } else {
        AuditOutcome::NotTrusted
    }
}

#[cfg(test)]
#[derive(Clone, Debug)]
pub(super) struct PendingReservationForTest(Reservation);

#[cfg(test)]
impl PendingReservationForTest {
    pub(super) fn handle(&self) -> i64 {
        self.0.handle
    }
}

#[cfg(test)]
pub(super) fn reserve_only_for_test(
    config: &StoreConfig,
    plan: &StoreCommitPlan,
) -> Result<PendingReservationForTest, StoreStatus> {
    prepare_store_path(config.path())?;
    let mut writer = connection::open_writer(config.path()).map_err(map_connection_error)?;
    reserve_generation(&mut writer, plan, &CommitControl::default())
        .map(PendingReservationForTest)
        .map_err(|error| match error {
            ReservationError::Connection(error) => map_connection_error(error),
            ReservationError::WorkspaceMismatch => {
                StoreStatus::Skipped(StoreSkipReason::WorkspaceMismatch)
            }
            ReservationError::InvalidMetadata => {
                StoreStatus::RebuildNeeded(StoreRebuildReason::InvalidMetadata)
            }
            ReservationError::Injected => StoreStatus::Skipped(StoreSkipReason::CommitFailed),
        })
}

#[cfg(test)]
pub(super) fn audit_reservation_for_test(
    writer: &mut WriterConnection,
    reservation: &PendingReservationForTest,
    reason: GenerationFailureReason,
    stage: GenerationFailureStage,
) -> AuditOutcome {
    audit_failed_attempt(writer, &reservation.0, reason, stage)
}

#[cfg(test)]
pub(super) fn payload_row_count_for_test(
    config: &StoreConfig,
    generation_id: i64,
) -> Result<i64, StoreStatus> {
    let reader = connection::open_read_only(config.path()).map_err(map_connection_error)?;
    generation_payload_row_count(connection::read_connection(&reader), generation_id)
        .map_err(map_read_error)
}

fn inject_failure(
    control: &CommitControl,
    stage: GenerationFailureStage,
) -> Result<(), PublicationError> {
    if control.fail_after == Some(stage) {
        let reason = if stage == GenerationFailureStage::TransactionCommit {
            GenerationFailureReason::PublicationCommitFailed
        } else {
            GenerationFailureReason::WriteFailed
        };
        Err(PublicationError {
            status: StoreStatus::Skipped(StoreSkipReason::CommitFailed),
            audit: Some((reason, stage)),
        })
    } else {
        Ok(())
    }
}

fn publication_connection_error(
    error: ConnectionError,
    stage: GenerationFailureStage,
    auditable: bool,
) -> PublicationError {
    let audit = (auditable && matches!(error, ConnectionError::Busy | ConnectionError::Other))
        .then_some((GenerationFailureReason::WriteFailed, stage));
    PublicationError {
        status: map_connection_error(error),
        audit,
    }
}

fn publication_commit_error(error: rusqlite::Error) -> PublicationError {
    let connection_error = connection::classify_sqlite_error(error);
    let audit = matches!(
        connection_error,
        ConnectionError::Busy | ConnectionError::Other
    )
    .then_some((
        GenerationFailureReason::PublicationCommitFailed,
        GenerationFailureStage::TransactionCommit,
    ));
    PublicationError {
        status: map_connection_error(connection_error),
        audit,
    }
}

fn publication_sql_error(
    error: rusqlite::Error,
    stage: GenerationFailureStage,
) -> PublicationError {
    let connection_error = connection::classify_sqlite_error(error);
    let status = match connection_error {
        ConnectionError::Busy => StoreStatus::BusySkipped,
        ConnectionError::Corrupt => StoreStatus::RebuildNeeded(StoreRebuildReason::Corrupt),
        ConnectionError::FutureSchema { found, supported } => {
            StoreStatus::Skipped(StoreSkipReason::FutureSchema { found, supported })
        }
        ConnectionError::InvalidSchema => {
            StoreStatus::RebuildNeeded(StoreRebuildReason::InvalidSchema)
        }
        ConnectionError::Policy | ConnectionError::Other => {
            StoreStatus::Skipped(StoreSkipReason::CommitFailed)
        }
    };
    let audit = matches!(
        connection_error,
        ConnectionError::Busy | ConnectionError::Other
    )
    .then_some((GenerationFailureReason::WriteFailed, stage));
    PublicationError { status, audit }
}

fn publication_projection_error(
    error: ProjectionError,
    stage: GenerationFailureStage,
) -> PublicationError {
    match error {
        ProjectionError::Connection(error) => publication_connection_error(error, stage, false),
        ProjectionError::Sqlite(error) => publication_sql_error(error, stage),
        ProjectionError::WorkspaceMismatch => PublicationError {
            status: StoreStatus::Skipped(StoreSkipReason::WorkspaceMismatch),
            audit: None,
        },
        ProjectionError::Codec | ProjectionError::InvalidMetadata => PublicationError {
            status: StoreStatus::Skipped(StoreSkipReason::CommitFailed),
            audit: Some((GenerationFailureReason::PostWriteValidationFailed, stage)),
        },
    }
}

fn publication_integrity_error(stage: GenerationFailureStage) -> PublicationError {
    PublicationError {
        status: StoreStatus::Skipped(StoreSkipReason::CommitFailed),
        audit: Some((GenerationFailureReason::PostWriteValidationFailed, stage)),
    }
}

fn map_read_error(error: ProjectionError) -> StoreStatus {
    match error {
        ProjectionError::Connection(error) => map_connection_error(error),
        ProjectionError::Sqlite(error) => {
            map_connection_error(connection::classify_sqlite_error(error))
        }
        ProjectionError::WorkspaceMismatch => {
            StoreStatus::Skipped(StoreSkipReason::WorkspaceMismatch)
        }
        ProjectionError::Codec | ProjectionError::InvalidMetadata => {
            StoreStatus::RebuildNeeded(StoreRebuildReason::InvalidMetadata)
        }
    }
}

fn map_reservation_projection_error(error: ProjectionError) -> ReservationError {
    match error {
        ProjectionError::Connection(error) => ReservationError::Connection(error),
        ProjectionError::Sqlite(error) => {
            ReservationError::Connection(connection::classify_sqlite_error(error))
        }
        ProjectionError::WorkspaceMismatch => ReservationError::WorkspaceMismatch,
        ProjectionError::Codec | ProjectionError::InvalidMetadata => {
            ReservationError::InvalidMetadata
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ManifestRow {
    workspace: Option<WorkspaceIdentity>,
    active_generation: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StoredGenerationHeader {
    handle: i64,
    reservation_ordinal: i64,
    identities: StoreIdentityRow,
    dependency_schema: String,
    status: GenerationStatus,
}

struct RawStoredGenerationHeader {
    reservation_ordinal: i64,
    workspace_kind: String,
    workspace_value: String,
    generation_kind: String,
    generation_value: String,
    run_kind: String,
    run_value: String,
    full_config_kind: String,
    full_config_value: String,
    input_snapshot_kind: String,
    input_snapshot_value: String,
    provider_manifest_kind: String,
    provider_manifest_value: String,
    provider_output_kind: String,
    provider_output_value: String,
    layer_kind: String,
    layer_value: String,
    summary_kind: String,
    summary_value: String,
    query_kind: String,
    query_value: String,
    fact_kind: String,
    fact_value: String,
    dependency_kind: String,
    dependency_value: String,
    validation_kind: String,
    validation_value: String,
    dependency_schema: String,
    status: String,
}

fn read_manifest(connection: &Connection) -> Result<ManifestRow, ProjectionError> {
    let (kind, value, active): (Option<String>, Option<String>, Option<i64>) = connection
        .query_row(
            "SELECT workspace_kind, workspace_value, active_generation_id
             FROM store_manifest WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
    let workspace = match (kind, value) {
        (None, None) => None,
        (Some(kind), Some(value)) => Some(decode_workspace_identity(&kind, &value)?),
        _ => return Err(ProjectionError::InvalidMetadata),
    };
    if workspace.is_none() && active.is_some() {
        return Err(ProjectionError::InvalidMetadata);
    }
    Ok(ManifestRow {
        workspace,
        active_generation: active,
    })
}

fn read_generation_header(
    connection: &Connection,
    handle: i64,
) -> Result<StoredGenerationHeader, ProjectionError> {
    let raw = connection
        .query_row(
            "SELECT
                 reservation_ordinal,
                 workspace_kind, workspace_value,
                 generation_kind, generation_value,
                 run_kind, run_value,
                 full_config_kind, full_config_value,
                 input_snapshot_kind, input_snapshot_value,
                 provider_manifest_kind, provider_manifest_value,
                 provider_output_kind, provider_output_value,
                 layer_kind, layer_value,
                 summary_kind, summary_value,
                 query_kind, query_value,
                 fact_kind, fact_value,
                 dependency_kind, dependency_value,
                 validation_kind, validation_value,
                 dependency_schema, status
             FROM generations WHERE id = ?1",
            [handle],
            |row| {
                Ok(RawStoredGenerationHeader {
                    reservation_ordinal: row.get(0)?,
                    workspace_kind: row.get(1)?,
                    workspace_value: row.get(2)?,
                    generation_kind: row.get(3)?,
                    generation_value: row.get(4)?,
                    run_kind: row.get(5)?,
                    run_value: row.get(6)?,
                    full_config_kind: row.get(7)?,
                    full_config_value: row.get(8)?,
                    input_snapshot_kind: row.get(9)?,
                    input_snapshot_value: row.get(10)?,
                    provider_manifest_kind: row.get(11)?,
                    provider_manifest_value: row.get(12)?,
                    provider_output_kind: row.get(13)?,
                    provider_output_value: row.get(14)?,
                    layer_kind: row.get(15)?,
                    layer_value: row.get(16)?,
                    summary_kind: row.get(17)?,
                    summary_value: row.get(18)?,
                    query_kind: row.get(19)?,
                    query_value: row.get(20)?,
                    fact_kind: row.get(21)?,
                    fact_value: row.get(22)?,
                    dependency_kind: row.get(23)?,
                    dependency_value: row.get(24)?,
                    validation_kind: row.get(25)?,
                    validation_value: row.get(26)?,
                    dependency_schema: row.get(27)?,
                    status: row.get(28)?,
                })
            },
        )
        .optional()?
        .ok_or(ProjectionError::InvalidMetadata)?;
    let RawStoredGenerationHeader {
        reservation_ordinal,
        workspace_kind,
        workspace_value,
        generation_kind,
        generation_value,
        run_kind,
        run_value,
        full_config_kind,
        full_config_value,
        input_snapshot_kind,
        input_snapshot_value,
        provider_manifest_kind,
        provider_manifest_value,
        provider_output_kind,
        provider_output_value,
        layer_kind,
        layer_value,
        summary_kind,
        summary_value,
        query_kind,
        query_value,
        fact_kind,
        fact_value,
        dependency_kind,
        dependency_value,
        validation_kind,
        validation_value,
        dependency_schema,
        status,
    } = raw;

    let workspace = decode_workspace_identity(&workspace_kind, &workspace_value)?;
    let full_config_digest = decode_digest(
        &full_config_kind,
        &full_config_value,
        Some(DigestKind::Config),
    )?;
    let full_config = ConfigIdentity::from_complete_config_digest(full_config_digest)
        .map_err(|_| ProjectionError::InvalidMetadata)?;
    let input_snapshot = decode_digest(
        &input_snapshot_kind,
        &input_snapshot_value,
        Some(DigestKind::InputSnapshot),
    )?;
    let provider_manifest = decode_digest(
        &provider_manifest_kind,
        &provider_manifest_value,
        Some(DigestKind::ProviderManifest),
    )?;
    let provider_output = decode_digest(
        &provider_output_kind,
        &provider_output_value,
        Some(DigestKind::ProviderOutput),
    )?;
    let layer = decode_digest(&layer_kind, &layer_value, Some(DigestKind::Layer))?;
    let summary = decode_digest(&summary_kind, &summary_value, Some(DigestKind::Summary))?;
    let query = decode_digest(&query_kind, &query_value, Some(DigestKind::Query))?;
    let fact = decode_digest(&fact_kind, &fact_value, Some(DigestKind::FactMetadata))?;
    let dependency = decode_digest(
        &dependency_kind,
        &dependency_value,
        Some(DigestKind::Dependency),
    )?;
    let validation = decode_digest(
        &validation_kind,
        &validation_value,
        Some(DigestKind::ValidationEvent),
    )?;
    let stored_run = decode_digest(&run_kind, &run_value, Some(DigestKind::Run))?;
    let run = RunIdentity::new(
        &workspace,
        &full_config,
        &input_snapshot,
        &provider_manifest,
    )
    .map_err(|_| ProjectionError::InvalidMetadata)?;
    if run.digest() != &stored_run {
        return Err(ProjectionError::InvalidMetadata);
    }
    let stored_generation = decode_digest(
        &generation_kind,
        &generation_value,
        Some(DigestKind::Generation),
    )?;
    let generation = GenerationIdentity::new(
        &run,
        &[
            provider_output.clone(),
            layer.clone(),
            summary.clone(),
            query.clone(),
            fact.clone(),
            dependency.clone(),
            validation.clone(),
        ],
    )
    .map_err(|_| ProjectionError::InvalidMetadata)?;
    if generation.digest() != &stored_generation {
        return Err(ProjectionError::InvalidMetadata);
    }
    validate_dependency_schema(&dependency_schema)?;

    Ok(StoredGenerationHeader {
        handle,
        reservation_ordinal,
        identities: StoreIdentityRow {
            workspace,
            full_config,
            input_snapshot,
            provider_manifest,
            provider_output,
            layer,
            summary,
            query,
            fact,
            dependency,
            validation,
            run,
            generation,
        },
        dependency_schema,
        status: GenerationStatus::parse_label(&status)?,
    })
}

fn decode_workspace_identity(
    kind: &str,
    value: &str,
) -> Result<WorkspaceIdentity, ProjectionError> {
    let digest = decode_digest(kind, value, Some(DigestKind::Workspace))?;
    serde_json::from_value(
        serde_json::to_value(digest).map_err(|_| ProjectionError::InvalidMetadata)?,
    )
    .map_err(|_| ProjectionError::InvalidMetadata)
}

fn next_reservation_ordinal(
    connection: &Connection,
    identities: &StoreIdentityRow,
) -> Result<i64, ProjectionError> {
    let mut statement = connection.prepare(
        "SELECT id, reservation_ordinal
         FROM generations
         WHERE workspace_kind = ?1 AND workspace_value = ?2
           AND generation_kind = ?3 AND generation_value = ?4
         ORDER BY reservation_ordinal",
    )?;
    let existing = statement
        .query_map(
            params![
                identities.workspace.digest().kind.label(),
                identities.workspace.digest().value,
                identities.generation.digest().kind.label(),
                identities.generation.digest().value,
            ],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);

    let mut next = 0_i64;
    for (handle, ordinal) in existing {
        let header = read_generation_header(connection, handle)?;
        if header.identities != *identities
            || header.reservation_ordinal != ordinal
            || ordinal != next
        {
            return Err(ProjectionError::InvalidMetadata);
        }
        next = ordinal
            .checked_add(1)
            .ok_or(ProjectionError::InvalidMetadata)?;
    }
    Ok(next)
}

fn validate_reservation(
    connection: &Connection,
    reservation: &Reservation,
    plan: &StoreCommitPlan,
) -> Result<(), ReservationValidationError> {
    let manifest = read_manifest(connection)?;
    if manifest.workspace.as_ref() != Some(&reservation.identities.workspace)
        || plan.semantic.identities != reservation.identities
    {
        return Err(ProjectionError::InvalidMetadata.into());
    }
    if manifest.active_generation != reservation.prior_active_generation {
        let Some(active_handle) = manifest.active_generation else {
            return Err(ProjectionError::InvalidMetadata.into());
        };
        let active_header = read_generation_header(connection, active_handle)?;
        let failure_count: i64 = connection
            .query_row(
                "SELECT count(*) FROM generation_failure_events WHERE generation_id = ?1",
                [active_handle],
                |row| row.get(0),
            )
            .map_err(ProjectionError::from)?;
        return if active_handle != reservation.handle
            && active_header.status == GenerationStatus::Complete
            && active_header.identities.workspace == reservation.identities.workspace
            && failure_count == 0
        {
            Err(ReservationValidationError::Stale)
        } else {
            Err(ProjectionError::InvalidMetadata.into())
        };
    }
    let header = read_generation_header(connection, reservation.handle)?;
    if header.reservation_ordinal != reservation.reservation_ordinal
        || header.identities != reservation.identities
        || header.status != GenerationStatus::Pending
        || header.dependency_schema != plan.semantic.dependency_schema
    {
        return Err(ProjectionError::InvalidMetadata.into());
    }
    Ok(())
}

fn validate_audit_target(
    connection: &Connection,
    reservation: &Reservation,
) -> Result<(), ProjectionError> {
    let manifest = read_manifest(connection)?;
    if manifest.workspace.as_ref() != Some(&reservation.identities.workspace)
        || manifest.active_generation != reservation.prior_active_generation
        || manifest.active_generation == Some(reservation.handle)
    {
        return Err(ProjectionError::InvalidMetadata);
    }
    let header = read_generation_header(connection, reservation.handle)?;
    if header.reservation_ordinal != reservation.reservation_ordinal
        || header.identities != reservation.identities
        || header.status != GenerationStatus::Pending
    {
        return Err(ProjectionError::InvalidMetadata);
    }
    let failure_count: i64 = connection.query_row(
        "SELECT count(*) FROM generation_failure_events WHERE generation_id = ?1",
        [reservation.handle],
        |row| row.get(0),
    )?;
    if failure_count != 0 {
        return Err(ProjectionError::InvalidMetadata);
    }
    if generation_payload_row_count(connection, reservation.handle)? != 0 {
        return Err(ProjectionError::InvalidMetadata);
    }
    Ok(())
}

fn generation_payload_row_count(
    connection: &Connection,
    generation_id: i64,
) -> Result<i64, ProjectionError> {
    let mut total = 0_i64;
    for table in [
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
        "dependency_edges",
        "validation_events",
        "generation_stats",
        "generation_telemetry",
    ] {
        let query = format!("SELECT count(*) FROM {table} WHERE generation_id = ?1");
        let count: i64 = connection.query_row(&query, [generation_id], |row| row.get(0))?;
        total = total
            .checked_add(count)
            .ok_or(ProjectionError::InvalidMetadata)?;
    }
    Ok(total)
}

#[cfg(test)]
fn tamper_provider_child(
    transaction: &Transaction<'_>,
    generation_id: i64,
    tamper: ProviderChildTamper,
) -> Result<(), ProjectionError> {
    let child = transaction
        .query_row(
            "SELECT provider_id, schema_version
             FROM provider_manifest_schemas
             WHERE generation_id = ?1
             ORDER BY provider_id, schema_version
             LIMIT 1",
            [generation_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or(ProjectionError::InvalidMetadata)?;
    let changed = match tamper {
        ProviderChildTamper::Missing => transaction.execute(
            "DELETE FROM provider_manifest_schemas
             WHERE generation_id = ?1 AND provider_id = ?2 AND schema_version = ?3",
            params![generation_id, child.0, child.1],
        )?,
        ProviderChildTamper::Mismatched => transaction.execute(
            "UPDATE provider_manifest_schemas
             SET schema_version = schema_version || '.mismatch'
             WHERE generation_id = ?1 AND provider_id = ?2 AND schema_version = ?3",
            params![generation_id, child.0, child.1],
        )?,
    };
    if changed == 1 {
        Ok(())
    } else {
        Err(ProjectionError::InvalidMetadata)
    }
}

fn write_store_run_input(
    transaction: &Transaction<'_>,
    generation_id: i64,
    plan: &StoreCommitPlan,
) -> Result<(), ProjectionError> {
    let snapshot = &plan.semantic.input_snapshot;
    transaction.execute(
        "INSERT INTO input_snapshots (
             generation_id, schema_version, workspace_kind, workspace_value,
             full_config_kind, full_config_value, input_digest_kind, input_digest_value,
             requirements_digest_kind, requirements_digest_value
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            generation_id,
            snapshot.schema_version,
            snapshot.workspace.digest().kind.label(),
            snapshot.workspace.digest().value,
            snapshot.full_config.digest().kind.label(),
            snapshot.full_config.digest().value,
            snapshot.input_digest.kind.label(),
            snapshot.input_digest.value,
            snapshot.analysis_requirements_digest.kind.label(),
            snapshot.analysis_requirements_digest.value,
        ],
    )?;

    for row in &plan.semantic.files {
        transaction.execute(
            "INSERT INTO input_files (
                 generation_id, relative_path, language, source_digest_kind,
                 source_digest_value, size_bytes
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                generation_id,
                row.relative_path,
                row.language,
                row.source_digest.kind.label(),
                row.source_digest.value,
                sql_i64(row.size_bytes)?,
            ],
        )?;
    }
    for row in &plan.semantic.input_components {
        transaction.execute(
            "INSERT INTO input_components (
                 generation_id, component_group, name, status, digest_kind, digest_value,
                 detail_count
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                generation_id,
                encode_input_group(row.group)?,
                row.name,
                row.status,
                row.digest.kind.label(),
                row.digest.value,
                sql_i64(row.detail_count)?,
            ],
        )?;
    }
    let mut detail_ordinals = BTreeMap::new();
    for row in &plan.semantic.input_details {
        let ordinal = next_child_ordinal(
            &mut detail_ordinals,
            (row.group, row.component_name.clone()),
        )?;
        transaction.execute(
            "INSERT INTO input_component_details (
                 generation_id, component_group, component_name, ordinal,
                 component_digest_kind, component_digest_value, detail
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                generation_id,
                encode_input_group(row.group)?,
                row.component_name,
                ordinal,
                row.component_digest.kind.label(),
                row.component_digest.value,
                row.detail,
            ],
        )?;
    }
    for row in &plan.semantic.analysis_settings {
        transaction.execute(
            "INSERT INTO analysis_settings (generation_id, scope, digest_kind, digest_value)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                generation_id,
                row.scope,
                row.digest.kind.label(),
                row.digest.value,
            ],
        )?;
    }
    for row in &plan.semantic.capabilities {
        transaction.execute(
            "INSERT INTO requested_capabilities (
                 generation_id, capability, language, support_status, setup_status,
                 policy_query_version, rule_behavior_kind, rule_behavior_value,
                 analysis_dependency_kind, analysis_dependency_value, requester_count
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                generation_id,
                row.capability,
                row.language.as_deref().unwrap_or_default(),
                row.support_status,
                row.setup_status,
                row.policy_query_version,
                row.rule_behavior_digest.kind.label(),
                row.rule_behavior_digest.value,
                row.analysis_dependency_digest.kind.label(),
                row.analysis_dependency_digest.value,
                sql_i64(row.requester_count)?,
            ],
        )?;
    }
    for row in &plan.semantic.capability_requesters {
        transaction.execute(
            "INSERT INTO capability_requesters (
                 generation_id, capability, language, rule_id
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                generation_id,
                row.capability,
                row.language.as_deref().unwrap_or_default(),
                row.rule_id,
            ],
        )?;
    }
    for row in &plan.semantic.provider_schemas {
        transaction.execute(
            "INSERT INTO provider_schema_snapshots (
                 generation_id, provider_id, language_scope, cache_policy, precision_ceiling,
                 manifest_digest_kind, manifest_digest_value, version_count
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                generation_id,
                row.provider_id,
                row.language_scope,
                row.cache_policy,
                row.precision_ceiling,
                row.manifest_digest.kind.label(),
                row.manifest_digest.value,
                sql_i64(row.version_count)?,
            ],
        )?;
    }
    for row in &plan.semantic.provider_schema_versions {
        transaction.execute(
            "INSERT INTO provider_schema_versions (generation_id, provider_id, schema_version)
             VALUES (?1, ?2, ?3)",
            params![generation_id, row.provider_id, row.schema_version],
        )?;
    }
    for row in &plan.telemetry {
        transaction.execute(
            "INSERT INTO generation_telemetry (
                 generation_id, relative_path, file_mtime_hint_present
             ) VALUES (?1, ?2, ?3)",
            params![
                generation_id,
                row.relative_path,
                i64::from(row.file_mtime_hint_present),
            ],
        )?;
    }
    Ok(())
}

type ProviderGenerationKey = (String, String, String, Digest);

fn provider_generation_key(row: &StoreProviderGenerationRow) -> ProviderGenerationKey {
    (
        row.provider_id.clone(),
        row.provider_version.clone(),
        row.schema_version.clone(),
        row.output_digest.clone(),
    )
}

fn write_providers(
    transaction: &Transaction<'_>,
    generation_id: i64,
    plan: &StoreCommitPlan,
) -> Result<BTreeMap<ProviderGenerationKey, i64>, ProjectionError> {
    for row in &plan.semantic.provider_manifests {
        transaction.execute(
            "INSERT INTO provider_manifests (
                 generation_id, provider_id, provider_version, provider_kind, language_scope,
                 cache_policy, precision_ceiling, manifest_digest_kind, manifest_digest_value,
                 schema_count, input_count, output_count
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                generation_id,
                row.provider_id,
                row.provider_version,
                row.provider_kind,
                row.language_scope,
                row.cache_policy,
                row.precision_ceiling,
                row.manifest_digest.kind.label(),
                row.manifest_digest.value,
                sql_i64(row.schema_count)?,
                sql_i64(row.input_count)?,
                sql_i64(row.output_count)?,
            ],
        )?;
    }
    for row in &plan.semantic.provider_manifest_schemas {
        transaction.execute(
            "INSERT INTO provider_manifest_schemas (generation_id, provider_id, schema_version)
             VALUES (?1, ?2, ?3)",
            params![generation_id, row.provider_id, row.schema_version],
        )?;
    }
    for row in &plan.semantic.provider_manifest_inputs {
        transaction.execute(
            "INSERT INTO provider_manifest_inputs (generation_id, provider_id, input_name)
             VALUES (?1, ?2, ?3)",
            params![generation_id, row.provider_id, row.input],
        )?;
    }
    for row in &plan.semantic.provider_manifest_outputs {
        transaction.execute(
            "INSERT INTO provider_manifest_outputs (generation_id, provider_id, output_name)
             VALUES (?1, ?2, ?3)",
            params![generation_id, row.provider_id, row.output],
        )?;
    }

    let mut handles = BTreeMap::new();
    for row in &plan.semantic.provider_generations {
        transaction.execute(
            "INSERT INTO provider_generations (
                 generation_id, provider_id, provider_version, schema_version,
                 output_digest_kind, output_digest_value, precision, validation,
                 dependency_count, layer_count
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                generation_id,
                row.provider_id,
                row.provider_version,
                row.schema_version,
                row.output_digest.kind.label(),
                row.output_digest.value,
                row.precision,
                row.validation,
                sql_i64(row.dependency_count)?,
                sql_i64(row.layer_count)?,
            ],
        )?;
        if handles
            .insert(
                provider_generation_key(row),
                transaction.last_insert_rowid(),
            )
            .is_some()
        {
            return Err(ProjectionError::InvalidMetadata);
        }
    }

    let mut ordinals = BTreeMap::new();
    for row in &plan.semantic.provider_dependencies {
        let key = (
            row.provider_id.clone(),
            row.provider_version.clone(),
            row.schema_version.clone(),
            row.output_digest.clone(),
        );
        let handle = handles
            .get(&key)
            .copied()
            .ok_or(ProjectionError::InvalidMetadata)?;
        let ordinal = next_child_ordinal(&mut ordinals, key)?;
        transaction.execute(
            "INSERT INTO provider_dependencies (
                 generation_id, provider_generation_id, ordinal,
                 dependency_digest_kind, dependency_digest_value
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                generation_id,
                handle,
                ordinal,
                row.dependency.kind.label(),
                row.dependency.value,
            ],
        )?;
    }
    Ok(handles)
}

#[derive(Default)]
struct ProductHandles {
    layers: BTreeMap<LayerKey, i64>,
    summaries: BTreeMap<SummaryKey, i64>,
    queries: BTreeMap<QueryKey, i64>,
}

fn write_products(
    transaction: &Transaction<'_>,
    generation_id: i64,
    plan: &StoreCommitPlan,
) -> Result<ProductHandles, ProjectionError> {
    let mut handles = ProductHandles::default();
    for (ordinal, row) in plan.semantic.layers.iter().enumerate() {
        transaction.execute(
            "INSERT INTO layers (
                 generation_id, semantic_ordinal, layer_type, provider_id, provider_version,
                 schema_version, parameter_digest_kind, parameter_digest_value,
                 lifecycle_digest_kind, lifecycle_digest_value, settings_digest_kind,
                 settings_digest_value, toolchain_digest_kind, toolchain_digest_value,
                 output_digest_kind, output_digest_value, payload_digest_kind,
                 payload_digest_value, precision, validation, input_count,
                 dependency_layer_count, extension_count, edge_count, warning_count
             ) VALUES (
                 ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                 ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25
             )",
            params![
                generation_id,
                sql_ordinal(ordinal)?,
                row.key.layer_kind.label(),
                row.key.provider_id,
                row.key.provider_version,
                row.key.schema_version,
                row.key.parameter_digest.kind.label(),
                row.key.parameter_digest.value,
                row.key.lifecycle_digest.kind.label(),
                row.key.lifecycle_digest.value,
                row.key.analysis_settings_digest.kind.label(),
                row.key.analysis_settings_digest.value,
                row.key.toolchain_digest.kind.label(),
                row.key.toolchain_digest.value,
                row.output_digest.kind.label(),
                row.output_digest.value,
                row.payload_digest.kind.label(),
                row.payload_digest.value,
                row.precision,
                row.validation,
                sql_i64(row.input_count)?,
                sql_i64(row.dependency_layer_count)?,
                sql_i64(row.extension_count)?,
                sql_i64(row.edge_count)?,
                sql_i64(row.warning_count)?,
            ],
        )?;
        if handles
            .layers
            .insert(row.key.clone(), transaction.last_insert_rowid())
            .is_some()
        {
            return Err(ProjectionError::InvalidMetadata);
        }
    }
    write_layer_children(transaction, generation_id, plan, &handles.layers)?;

    for (ordinal, row) in plan.semantic.summaries.iter().enumerate() {
        transaction.execute(
            "INSERT INTO summaries (
                 generation_id, semantic_ordinal, callable_stable_key, summary_domain,
                 summary_version, body_shape_digest_kind, body_shape_digest_value,
                 extension_digest_kind, extension_digest_value, dependency_count
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                generation_id,
                sql_ordinal(ordinal)?,
                row.key.callable_stable_key,
                row.key.summary_domain,
                row.key.summary_version,
                row.key.body_shape_digest.kind.label(),
                row.key.body_shape_digest.value,
                row.key.extension_digest.kind.label(),
                row.key.extension_digest.value,
                sql_i64(row.dependency_count)?,
            ],
        )?;
        if handles
            .summaries
            .insert(row.key.clone(), transaction.last_insert_rowid())
            .is_some()
        {
            return Err(ProjectionError::InvalidMetadata);
        }
    }
    let mut summary_ordinals = BTreeMap::new();
    for row in &plan.semantic.summary_dependencies {
        let handle = handles
            .summaries
            .get(&row.key)
            .copied()
            .ok_or(ProjectionError::InvalidMetadata)?;
        let ordinal = next_child_ordinal(&mut summary_ordinals, row.key.clone())?;
        transaction.execute(
            "INSERT INTO summary_dependency_digests (
                 generation_id, summary_id, ordinal, digest_kind, digest_value
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                generation_id,
                handle,
                ordinal,
                row.dependency.kind.label(),
                row.dependency.value,
            ],
        )?;
    }

    for (ordinal, row) in plan.semantic.queries.iter().enumerate() {
        transaction.execute(
            "INSERT INTO queries (
                 generation_id, semantic_ordinal, query_type, query_version,
                 parameter_digest_kind, parameter_digest_value, budget_digest_kind,
                 budget_digest_value, precision_tier, result_digest_kind,
                 result_digest_value, precision, provenance, input_count, layer_count,
                 edge_count
             ) VALUES (
                 ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                 ?14, ?15, ?16
             )",
            params![
                generation_id,
                sql_ordinal(ordinal)?,
                row.key.query_kind,
                row.key.query_version,
                row.key.parameter_digest.kind.label(),
                row.key.parameter_digest.value,
                row.key.budget_digest.kind.label(),
                row.key.budget_digest.value,
                row.key.precision_tier.label(),
                row.result_digest.kind.label(),
                row.result_digest.value,
                row.precision,
                row.provenance,
                sql_i64(row.input_count)?,
                sql_i64(row.layer_count)?,
                sql_i64(row.edge_count)?,
            ],
        )?;
        if handles
            .queries
            .insert(row.key.clone(), transaction.last_insert_rowid())
            .is_some()
        {
            return Err(ProjectionError::InvalidMetadata);
        }
    }
    write_query_children(transaction, generation_id, plan, &handles.queries)?;
    Ok(handles)
}

fn write_layer_children(
    transaction: &Transaction<'_>,
    generation_id: i64,
    plan: &StoreCommitPlan,
    handles: &BTreeMap<LayerKey, i64>,
) -> Result<(), ProjectionError> {
    let mut input_ordinals = BTreeMap::new();
    for row in &plan.semantic.layer_inputs {
        write_layer_digest_child(
            transaction,
            generation_id,
            row,
            handles,
            &mut input_ordinals,
        )?;
    }
    let mut dependency_ordinals = BTreeMap::new();
    for row in &plan.semantic.layer_dependencies {
        let handle = handles
            .get(&row.key)
            .copied()
            .ok_or(ProjectionError::InvalidMetadata)?;
        let ordinal = next_child_ordinal(&mut dependency_ordinals, row.key.clone())?;
        transaction.execute(
            "INSERT INTO layer_dependency_digests (
                 generation_id, layer_id, ordinal, digest_kind, digest_value
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                generation_id,
                handle,
                ordinal,
                row.digest.kind.label(),
                row.digest.value,
            ],
        )?;
    }
    let mut extension_ordinals = BTreeMap::new();
    for row in &plan.semantic.layer_extensions {
        let handle = handles
            .get(&row.key)
            .copied()
            .ok_or(ProjectionError::InvalidMetadata)?;
        let ordinal = next_child_ordinal(&mut extension_ordinals, row.key.clone())?;
        transaction.execute(
            "INSERT INTO layer_extension_digests (
                 generation_id, layer_id, ordinal, digest_kind, digest_value
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                generation_id,
                handle,
                ordinal,
                row.digest.kind.label(),
                row.digest.value,
            ],
        )?;
    }
    let mut warning_ordinals = BTreeMap::new();
    for row in &plan.semantic.layer_warnings {
        let handle = handles
            .get(&row.key)
            .copied()
            .ok_or(ProjectionError::InvalidMetadata)?;
        let ordinal = next_child_ordinal(&mut warning_ordinals, row.key.clone())?;
        transaction.execute(
            "INSERT INTO layer_warnings (generation_id, layer_id, ordinal, warning_code)
             VALUES (?1, ?2, ?3, ?4)",
            params![generation_id, handle, ordinal, row.warning_code],
        )?;
    }
    Ok(())
}

fn write_layer_digest_child(
    transaction: &Transaction<'_>,
    generation_id: i64,
    row: &StoreLayerInputRow,
    handles: &BTreeMap<LayerKey, i64>,
    ordinals: &mut BTreeMap<LayerKey, i64>,
) -> Result<(), ProjectionError> {
    let handle = handles
        .get(&row.key)
        .copied()
        .ok_or(ProjectionError::InvalidMetadata)?;
    let ordinal = next_child_ordinal(ordinals, row.key.clone())?;
    transaction.execute(
        "INSERT INTO layer_input_digests (
             generation_id, layer_id, ordinal, digest_kind, digest_value
         ) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            generation_id,
            handle,
            ordinal,
            row.digest.kind.label(),
            row.digest.value,
        ],
    )?;
    Ok(())
}

fn write_query_children(
    transaction: &Transaction<'_>,
    generation_id: i64,
    plan: &StoreCommitPlan,
    handles: &BTreeMap<QueryKey, i64>,
) -> Result<(), ProjectionError> {
    let mut input_ordinals = BTreeMap::new();
    for row in &plan.semantic.query_inputs {
        let handle = handles
            .get(&row.key)
            .copied()
            .ok_or(ProjectionError::InvalidMetadata)?;
        let ordinal = next_child_ordinal(&mut input_ordinals, row.key.clone())?;
        transaction.execute(
            "INSERT INTO query_inputs (
                 generation_id, query_id, ordinal, input_kind, stable_key,
                 digest_kind, digest_value, status
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                generation_id,
                handle,
                ordinal,
                row.input.kind.label(),
                row.input.stable_key,
                row.input.digest.kind.label(),
                row.input.digest.value,
                row.input.status.label(),
            ],
        )?;
    }
    let mut layer_ordinals = BTreeMap::new();
    for row in &plan.semantic.query_layers {
        let handle = handles
            .get(&row.key)
            .copied()
            .ok_or(ProjectionError::InvalidMetadata)?;
        let ordinal = next_child_ordinal(&mut layer_ordinals, row.key.clone())?;
        transaction.execute(
            "INSERT INTO query_layer_digests (
                 generation_id, query_id, ordinal, digest_kind, digest_value
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                generation_id,
                handle,
                ordinal,
                row.layer_digest.kind.label(),
                row.layer_digest.value,
            ],
        )?;
    }
    Ok(())
}

fn write_facts(
    transaction: &Transaction<'_>,
    generation_id: i64,
    rows: &[StoreFactRow],
) -> Result<(), ProjectionError> {
    for row in rows {
        transaction.execute(
            "INSERT INTO fact_metadata (
                 generation_id, family, stable_key, producer_id, producer_layer_key,
                 precision, confidence, validation, payload_digest
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                generation_id,
                row.family,
                row.stable_key,
                row.producer_id,
                row.layer_id,
                row.precision,
                row.confidence,
                row.validation,
                row.payload_digest,
            ],
        )?;
    }
    Ok(())
}

struct EncodedEndpoint {
    node_kind: &'static str,
    input_kind: Option<String>,
    input_stable_key: Option<String>,
    input_digest_kind: Option<&'static str>,
    input_digest_value: Option<String>,
    input_status: Option<&'static str>,
    layer_id: Option<i64>,
    query_id: Option<i64>,
    summary_id: Option<i64>,
}

fn encode_endpoint(
    node: &CacheNode,
    handles: &ProductHandles,
) -> Result<EncodedEndpoint, ProjectionError> {
    match node {
        CacheNode::DependencyInput(input) => Ok(EncodedEndpoint {
            node_kind: node.kind().label(),
            input_kind: Some(input.kind.label().to_string()),
            input_stable_key: Some(input.stable_key.clone()),
            input_digest_kind: Some(input.digest.kind.label()),
            input_digest_value: Some(input.digest.value.clone()),
            input_status: Some(input.status.label()),
            layer_id: None,
            query_id: None,
            summary_id: None,
        }),
        CacheNode::Layer(key) => Ok(EncodedEndpoint {
            node_kind: node.kind().label(),
            input_kind: None,
            input_stable_key: None,
            input_digest_kind: None,
            input_digest_value: None,
            input_status: None,
            layer_id: Some(
                handles
                    .layers
                    .get(key)
                    .copied()
                    .ok_or(ProjectionError::InvalidMetadata)?,
            ),
            query_id: None,
            summary_id: None,
        }),
        CacheNode::Query(key) => Ok(EncodedEndpoint {
            node_kind: node.kind().label(),
            input_kind: None,
            input_stable_key: None,
            input_digest_kind: None,
            input_digest_value: None,
            input_status: None,
            layer_id: None,
            query_id: Some(
                handles
                    .queries
                    .get(key)
                    .copied()
                    .ok_or(ProjectionError::InvalidMetadata)?,
            ),
            summary_id: None,
        }),
        CacheNode::Summary(key) => Ok(EncodedEndpoint {
            node_kind: node.kind().label(),
            input_kind: None,
            input_stable_key: None,
            input_digest_kind: None,
            input_digest_value: None,
            input_status: None,
            layer_id: None,
            query_id: None,
            summary_id: Some(
                handles
                    .summaries
                    .get(key)
                    .copied()
                    .ok_or(ProjectionError::InvalidMetadata)?,
            ),
        }),
        CacheNode::Diagnostic(_) => Err(ProjectionError::InvalidMetadata),
    }
}

fn write_dependency_edges(
    transaction: &Transaction<'_>,
    generation_id: i64,
    rows: &[StoreDependencyEdgeRow],
    handles: &ProductHandles,
) -> Result<(), ProjectionError> {
    for (ordinal, row) in rows.iter().enumerate() {
        let from = encode_endpoint(&row.from, handles)?;
        let to = encode_endpoint(&row.to, handles)?;
        transaction.execute(
            "INSERT INTO dependency_edges (
                 generation_id, ordinal,
                 from_node_kind, from_input_kind, from_input_stable_key,
                 from_input_digest_kind, from_input_digest_value, from_input_status,
                 from_layer_id, from_query_id, from_summary_id,
                 to_node_kind, to_input_kind, to_input_stable_key,
                 to_input_digest_kind, to_input_digest_value, to_input_status,
                 to_layer_id, to_query_id, to_summary_id,
                 dependency_kind, required_shape
             ) VALUES (
                 ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                 ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22
             )",
            params![
                generation_id,
                sql_ordinal(ordinal)?,
                from.node_kind,
                from.input_kind,
                from.input_stable_key,
                from.input_digest_kind,
                from.input_digest_value,
                from.input_status,
                from.layer_id,
                from.query_id,
                from.summary_id,
                to.node_kind,
                to.input_kind,
                to.input_stable_key,
                to.input_digest_kind,
                to.input_digest_value,
                to.input_status,
                to.layer_id,
                to.query_id,
                to.summary_id,
                row.kind.label(),
                row.required_shape.label(),
            ],
        )?;
    }
    Ok(())
}

fn write_validation_events(
    transaction: &Transaction<'_>,
    generation_id: i64,
    rows: &[StoreValidationEventRow],
) -> Result<(), ProjectionError> {
    for row in rows {
        transaction.execute(
            "INSERT INTO validation_events (
                 generation_id, event_kind, status, issue_count, digest_kind, digest_value
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                generation_id,
                row.kind,
                row.status,
                sql_i64(row.issue_count)?,
                row.digest.kind.label(),
                row.digest.value,
            ],
        )?;
    }
    Ok(())
}

fn write_stats(
    transaction: &Transaction<'_>,
    generation_id: i64,
    stats: &StoreGenerationStats,
) -> Result<(), ProjectionError> {
    transaction.execute(
        "INSERT INTO generation_stats (
             generation_id,
             input_file_count, input_component_count, input_detail_count,
             analysis_setting_count, capability_count, provider_schema_count,
             provider_manifest_count, provider_generation_count, layer_count,
             summary_count, query_count, fact_count, dependency_edge_count,
             validation_event_count,
             input_digest_kind, input_digest_value,
             provider_manifest_digest_kind, provider_manifest_digest_value,
             provider_output_digest_kind, provider_output_digest_value,
             layer_digest_kind, layer_digest_value,
             summary_digest_kind, summary_digest_value,
             query_digest_kind, query_digest_value,
             fact_digest_kind, fact_digest_value,
             dependency_digest_kind, dependency_digest_value,
             validation_digest_kind, validation_digest_value,
             input_logical_bytes, provider_logical_bytes, layer_logical_bytes,
             summary_logical_bytes, query_logical_bytes, fact_logical_bytes,
             dependency_logical_bytes, validation_logical_bytes, semantic_logical_bytes
         ) VALUES (
             ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
             ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27,
             ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40,
             ?41, ?42
         )",
        params![
            generation_id,
            sql_i64(stats.input_file_count)?,
            sql_i64(stats.input_component_count)?,
            sql_i64(stats.input_detail_count)?,
            sql_i64(stats.analysis_setting_count)?,
            sql_i64(stats.capability_count)?,
            sql_i64(stats.provider_schema_count)?,
            sql_i64(stats.provider_manifest_count)?,
            sql_i64(stats.provider_generation_count)?,
            sql_i64(stats.layer_count)?,
            sql_i64(stats.summary_count)?,
            sql_i64(stats.query_count)?,
            sql_i64(stats.fact_count)?,
            sql_i64(stats.dependency_edge_count)?,
            sql_i64(stats.validation_event_count)?,
            stats.input_digest.kind.label(),
            stats.input_digest.value,
            stats.provider_manifest_digest.kind.label(),
            stats.provider_manifest_digest.value,
            stats.provider_output_digest.kind.label(),
            stats.provider_output_digest.value,
            stats.layer_digest.kind.label(),
            stats.layer_digest.value,
            stats.summary_digest.kind.label(),
            stats.summary_digest.value,
            stats.query_digest.kind.label(),
            stats.query_digest.value,
            stats.fact_digest.kind.label(),
            stats.fact_digest.value,
            stats.dependency_digest.kind.label(),
            stats.dependency_digest.value,
            stats.validation_digest.kind.label(),
            stats.validation_digest.value,
            sql_i64(stats.input_logical_bytes)?,
            sql_i64(stats.provider_logical_bytes)?,
            sql_i64(stats.layer_logical_bytes)?,
            sql_i64(stats.summary_logical_bytes)?,
            sql_i64(stats.query_logical_bytes)?,
            sql_i64(stats.fact_logical_bytes)?,
            sql_i64(stats.dependency_logical_bytes)?,
            sql_i64(stats.validation_logical_bytes)?,
            sql_i64(stats.semantic_logical_bytes)?,
        ],
    )?;
    Ok(())
}

fn next_child_ordinal<K: Ord>(
    ordinals: &mut BTreeMap<K, i64>,
    key: K,
) -> Result<i64, ProjectionError> {
    let ordinal = ordinals.entry(key).or_insert(0);
    let current = *ordinal;
    *ordinal = ordinal
        .checked_add(1)
        .ok_or(ProjectionError::InvalidMetadata)?;
    Ok(current)
}

fn sql_i64(value: u64) -> Result<i64, ProjectionError> {
    i64::try_from(value).map_err(|_| ProjectionError::InvalidMetadata)
}

fn sql_ordinal(value: usize) -> Result<i64, ProjectionError> {
    i64::try_from(value).map_err(|_| ProjectionError::InvalidMetadata)
}

fn read_active_complete_from_reader(
    reader: &ReadOnlyConnection,
    workspace: &WorkspaceIdentity,
) -> Result<Option<ActiveCompleteGeneration>, ProjectionError> {
    read_active_complete_from_connection(connection::read_connection(reader), workspace)
}

fn read_active_complete_from_connection(
    connection: &Connection,
    workspace: &WorkspaceIdentity,
) -> Result<Option<ActiveCompleteGeneration>, ProjectionError> {
    connection::validate_schema(connection).map_err(ProjectionError::Connection)?;
    let manifest = read_manifest(connection)?;
    let Some(bound_workspace) = manifest.workspace else {
        return if manifest.active_generation.is_none() {
            Ok(None)
        } else {
            Err(ProjectionError::InvalidMetadata)
        };
    };
    if &bound_workspace != workspace {
        return Err(ProjectionError::WorkspaceMismatch);
    }
    let Some(active_handle) = manifest.active_generation else {
        return Ok(None);
    };
    let header = read_generation_header(connection, active_handle)?;
    if header.status != GenerationStatus::Complete || &header.identities.workspace != workspace {
        return Err(ProjectionError::InvalidMetadata);
    }
    let failure_count: i64 = connection.query_row(
        "SELECT count(*) FROM generation_failure_events WHERE generation_id = ?1",
        [active_handle],
        |row| row.get(0),
    )?;
    if failure_count != 0 {
        return Err(ProjectionError::InvalidMetadata);
    }
    read_generation_projection(connection, active_handle, TelemetryReadPolicy::BestEffort).map(Some)
}

#[derive(Clone, Copy)]
enum TelemetryReadPolicy {
    Strict,
    BestEffort,
}

fn read_generation_projection(
    connection: &Connection,
    generation_id: i64,
    telemetry_policy: TelemetryReadPolicy,
) -> Result<ActiveCompleteGeneration, ProjectionError> {
    let header = read_generation_header(connection, generation_id)?;
    let input_snapshot = read_input_snapshot(connection, generation_id)?;
    let files = read_files(connection, generation_id)?;
    let input_components = read_input_components(connection, generation_id)?;
    let input_details = read_input_details(connection, generation_id)?;
    let analysis_settings = read_analysis_settings(connection, generation_id)?;
    let capabilities = read_capabilities(connection, generation_id)?;
    let capability_requesters = read_capability_requesters(connection, generation_id)?;
    let provider_schemas = read_provider_schemas(connection, generation_id)?;
    let provider_schema_versions = read_provider_schema_versions(connection, generation_id)?;
    let provider_manifests = read_provider_manifests(connection, generation_id)?;
    let provider_manifest_schemas = read_provider_manifest_schemas(connection, generation_id)?;
    let provider_manifest_inputs = read_provider_manifest_inputs(connection, generation_id)?;
    let provider_manifest_outputs = read_provider_manifest_outputs(connection, generation_id)?;
    let (provider_generations, provider_generation_handles) =
        read_provider_generations(connection, generation_id)?;
    let provider_dependencies =
        read_provider_dependencies(connection, generation_id, &provider_generation_handles)?;
    let (layers, layer_inputs, layer_dependencies, layer_extensions, layer_warnings, layer_handles) =
        read_layers(connection, generation_id)?;
    let (summaries, summary_dependencies, summary_handles) =
        read_summaries(connection, generation_id)?;
    let (queries, query_inputs, query_layers, query_handles) =
        read_queries(connection, generation_id)?;
    let facts = read_facts(connection, generation_id)?;
    let dependency_edges = read_dependency_edges(
        connection,
        generation_id,
        &layer_handles,
        &query_handles,
        &summary_handles,
    )?;
    let validation_events = read_validation_events(connection, generation_id)?;
    let stats = read_stats(connection, generation_id)?;
    let telemetry = match (telemetry_policy, read_telemetry(connection, generation_id)) {
        (_, Ok(telemetry)) => telemetry,
        (TelemetryReadPolicy::BestEffort, Err(_)) => Vec::new(),
        (TelemetryReadPolicy::Strict, Err(error)) => return Err(error),
    };

    let semantic = StoreSemanticPlan {
        identities: header.identities,
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
        dependency_schema: header.dependency_schema,
        dependency_edges,
        validation_events,
        stats,
    };
    let plan = StoreCommitPlan {
        semantic,
        telemetry,
    };
    plan.validate()
        .map_err(|_| ProjectionError::InvalidMetadata)?;
    let dependency_index = DependencyIndex::from_edges(
        plan.semantic
            .dependency_edges
            .iter()
            .map(to_dependency_edge)
            .collect(),
    );
    if dependency_index.schema_version != plan.semantic.dependency_schema {
        return Err(ProjectionError::InvalidMetadata);
    }
    Ok(ActiveCompleteGeneration {
        plan,
        dependency_index,
    })
}

fn query_rows<T>(
    connection: &Connection,
    sql: &str,
    generation_id: i64,
    mut decode: impl FnMut(&Row<'_>) -> Result<T, ProjectionError>,
) -> Result<Vec<T>, ProjectionError> {
    let mut statement = connection.prepare(sql)?;
    let mut rows = statement.query([generation_id])?;
    let mut decoded = Vec::new();
    while let Some(row) = rows.next()? {
        decoded.push(decode(row)?);
    }
    Ok(decoded)
}

fn to_dependency_edge(row: &StoreDependencyEdgeRow) -> DependencyEdge {
    DependencyEdge {
        from: row.from.clone(),
        to: row.to.clone(),
        kind: row.kind,
        required_shape: row.required_shape,
    }
}

fn read_input_snapshot(
    connection: &Connection,
    generation_id: i64,
) -> Result<StoreInputSnapshotRow, ProjectionError> {
    let raw: (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
    ) = connection
        .query_row(
            "SELECT schema_version, workspace_kind, workspace_value,
                    full_config_kind, full_config_value, input_digest_kind,
                    input_digest_value, requirements_digest_kind,
                    requirements_digest_value
             FROM input_snapshots WHERE generation_id = ?1",
            [generation_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            },
        )
        .optional()?
        .ok_or(ProjectionError::InvalidMetadata)?;
    validate_input_snapshot_schema(&raw.0)?;
    let workspace = decode_workspace_identity(&raw.1, &raw.2)?;
    let full_config = ConfigIdentity::from_complete_config_digest(decode_digest(
        &raw.3,
        &raw.4,
        Some(DigestKind::Config),
    )?)
    .map_err(|_| ProjectionError::InvalidMetadata)?;
    Ok(StoreInputSnapshotRow {
        schema_version: raw.0,
        workspace,
        full_config,
        input_digest: decode_digest(&raw.5, &raw.6, Some(DigestKind::InputSnapshot))?,
        analysis_requirements_digest: decode_digest(
            &raw.7,
            &raw.8,
            Some(DigestKind::AnalysisRequirements),
        )?,
    })
}

fn read_files(
    connection: &Connection,
    generation_id: i64,
) -> Result<Vec<StoreFileRow>, ProjectionError> {
    query_rows(
        connection,
        "SELECT relative_path, language, source_digest_kind, source_digest_value, size_bytes
         FROM input_files WHERE generation_id = ?1
         ORDER BY relative_path, language",
        generation_id,
        |row| {
            let relative_path: String = row.get(0)?;
            validate_relative_path(&relative_path)?;
            let language_label: String = row.get(1)?;
            let language = decode_language(&language_label)?;
            Ok(StoreFileRow {
                relative_path,
                language: language.label().to_string(),
                source_digest: decode_digest(
                    &row.get::<_, String>(2)?,
                    &row.get::<_, String>(3)?,
                    Some(DigestKind::SourceText),
                )?,
                size_bytes: read_u64(row, 4)?,
            })
        },
    )
}

fn read_input_components(
    connection: &Connection,
    generation_id: i64,
) -> Result<Vec<StoreInputComponentRow>, ProjectionError> {
    let mut rows = query_rows(
        connection,
        "SELECT component_group, name, status, digest_kind, digest_value, detail_count
         FROM input_components WHERE generation_id = ?1
         ORDER BY component_group, name, status, digest_kind, digest_value",
        generation_id,
        |row| {
            let group_label: String = row.get(0)?;
            let status_label: String = row.get(2)?;
            let status = decode_input_component_status(&status_label)?;
            Ok(StoreInputComponentRow {
                group: decode_input_group(&group_label)?,
                name: row.get(1)?,
                status: status.label().to_string(),
                digest: decode_digest(&row.get::<_, String>(3)?, &row.get::<_, String>(4)?, None)?,
                detail_count: read_u64(row, 5)?,
            })
        },
    )?;
    rows.sort();
    Ok(rows)
}

fn read_input_details(
    connection: &Connection,
    generation_id: i64,
) -> Result<Vec<StoreInputDetailRow>, ProjectionError> {
    let mut rows = query_rows(
        connection,
        "SELECT component_group, component_name, component_digest_kind,
                component_digest_value, detail
         FROM input_component_details WHERE generation_id = ?1
         ORDER BY component_group, component_name, ordinal",
        generation_id,
        |row| {
            Ok(StoreInputDetailRow {
                group: decode_input_group(&row.get::<_, String>(0)?)?,
                component_name: row.get(1)?,
                component_digest: decode_digest(
                    &row.get::<_, String>(2)?,
                    &row.get::<_, String>(3)?,
                    None,
                )?,
                detail: row.get(4)?,
            })
        },
    )?;
    rows.sort();
    Ok(rows)
}

fn read_analysis_settings(
    connection: &Connection,
    generation_id: i64,
) -> Result<Vec<StoreAnalysisSettingRow>, ProjectionError> {
    query_rows(
        connection,
        "SELECT scope, digest_kind, digest_value
         FROM analysis_settings WHERE generation_id = ?1
         ORDER BY scope, digest_kind, digest_value",
        generation_id,
        |row| {
            let scope_label: String = row.get(0)?;
            let scope = decode_analysis_settings_scope(&scope_label)?;
            Ok(StoreAnalysisSettingRow {
                scope: scope.label().to_string(),
                digest: decode_digest(
                    &row.get::<_, String>(1)?,
                    &row.get::<_, String>(2)?,
                    Some(DigestKind::AnalysisSettings),
                )?,
            })
        },
    )
}

fn read_capabilities(
    connection: &Connection,
    generation_id: i64,
) -> Result<Vec<StoreCapabilityRow>, ProjectionError> {
    query_rows(
        connection,
        "SELECT capability, language, support_status, setup_status, policy_query_version,
                rule_behavior_kind, rule_behavior_value, analysis_dependency_kind,
                analysis_dependency_value, requester_count
         FROM requested_capabilities WHERE generation_id = ?1
         ORDER BY capability, language, support_status, setup_status",
        generation_id,
        |row| {
            let language_label: String = row.get(1)?;
            let language = if language_label.is_empty() {
                None
            } else {
                Some(decode_language(&language_label)?.label().to_string())
            };
            let support_status: String = row.get(2)?;
            decode_capability_support_status(&support_status)?;
            let setup_status: String = row.get(3)?;
            decode_capability_setup_status(&setup_status)?;
            Ok(StoreCapabilityRow {
                capability: row.get(0)?,
                language,
                support_status,
                setup_status,
                policy_query_version: row.get(4)?,
                rule_behavior_digest: decode_digest(
                    &row.get::<_, String>(5)?,
                    &row.get::<_, String>(6)?,
                    Some(DigestKind::RuleOptions),
                )?,
                analysis_dependency_digest: decode_digest(
                    &row.get::<_, String>(7)?,
                    &row.get::<_, String>(8)?,
                    Some(DigestKind::AnalysisRequirements),
                )?,
                requester_count: read_u64(row, 9)?,
            })
        },
    )
}

fn read_capability_requesters(
    connection: &Connection,
    generation_id: i64,
) -> Result<Vec<StoreCapabilityRequesterRow>, ProjectionError> {
    query_rows(
        connection,
        "SELECT capability, language, rule_id
         FROM capability_requesters WHERE generation_id = ?1
         ORDER BY capability, language, rule_id",
        generation_id,
        |row| {
            let language_label: String = row.get(1)?;
            let language = if language_label.is_empty() {
                None
            } else {
                Some(decode_language(&language_label)?.label().to_string())
            };
            Ok(StoreCapabilityRequesterRow {
                capability: row.get(0)?,
                language,
                rule_id: row.get(2)?,
            })
        },
    )
}

fn read_provider_schemas(
    connection: &Connection,
    generation_id: i64,
) -> Result<Vec<StoreProviderSchemaRow>, ProjectionError> {
    query_rows(
        connection,
        "SELECT provider_id, language_scope, cache_policy, precision_ceiling,
                manifest_digest_kind, manifest_digest_value, version_count
         FROM provider_schema_snapshots WHERE generation_id = ?1
         ORDER BY provider_id",
        generation_id,
        |row| {
            let language_scope: String = row.get(1)?;
            decode_language_scope(&language_scope)?;
            let cache_policy: String = row.get(2)?;
            decode_cache_policy(&cache_policy)?;
            let precision_ceiling: String = row.get(3)?;
            decode_precision_ceiling(&precision_ceiling)?;
            Ok(StoreProviderSchemaRow {
                provider_id: row.get(0)?,
                language_scope,
                cache_policy,
                precision_ceiling,
                manifest_digest: decode_digest(
                    &row.get::<_, String>(4)?,
                    &row.get::<_, String>(5)?,
                    Some(DigestKind::ProviderManifest),
                )?,
                version_count: read_u64(row, 6)?,
            })
        },
    )
}

fn read_provider_schema_versions(
    connection: &Connection,
    generation_id: i64,
) -> Result<Vec<StoreProviderSchemaVersionRow>, ProjectionError> {
    query_rows(
        connection,
        "SELECT provider_id, schema_version
         FROM provider_schema_versions WHERE generation_id = ?1
         ORDER BY provider_id, schema_version",
        generation_id,
        |row| {
            Ok(StoreProviderSchemaVersionRow {
                provider_id: row.get(0)?,
                schema_version: row.get(1)?,
            })
        },
    )
}

fn read_telemetry(
    connection: &Connection,
    generation_id: i64,
) -> Result<Vec<StoreTelemetryRow>, ProjectionError> {
    query_rows(
        connection,
        "SELECT relative_path, file_mtime_hint_present
         FROM generation_telemetry WHERE generation_id = ?1
         ORDER BY relative_path",
        generation_id,
        |row| {
            let relative_path: String = row.get(0)?;
            validate_relative_path(&relative_path)?;
            let flag: i64 = row.get(1)?;
            let file_mtime_hint_present = match flag {
                0 => false,
                1 => true,
                _ => return Err(ProjectionError::InvalidMetadata),
            };
            Ok(StoreTelemetryRow {
                relative_path,
                file_mtime_hint_present,
            })
        },
    )
}

fn read_u64(row: &Row<'_>, index: usize) -> Result<u64, ProjectionError> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(|_| ProjectionError::InvalidMetadata)
}

fn read_provider_manifests(
    connection: &Connection,
    generation_id: i64,
) -> Result<Vec<StoreProviderManifestRow>, ProjectionError> {
    query_rows(
        connection,
        "SELECT provider_id, provider_version, provider_kind, language_scope, cache_policy,
                precision_ceiling, manifest_digest_kind, manifest_digest_value,
                schema_count, input_count, output_count
         FROM provider_manifests WHERE generation_id = ?1
         ORDER BY provider_id, provider_version",
        generation_id,
        |row| {
            let provider_kind: String = row.get(2)?;
            decode_provider_kind(&provider_kind)?;
            let language_scope: String = row.get(3)?;
            decode_language_scope(&language_scope)?;
            let cache_policy: String = row.get(4)?;
            decode_cache_policy(&cache_policy)?;
            let precision_ceiling: String = row.get(5)?;
            decode_precision_ceiling(&precision_ceiling)?;
            Ok(StoreProviderManifestRow {
                provider_id: row.get(0)?,
                provider_version: row.get(1)?,
                provider_kind,
                language_scope,
                cache_policy,
                precision_ceiling,
                manifest_digest: decode_digest(
                    &row.get::<_, String>(6)?,
                    &row.get::<_, String>(7)?,
                    Some(DigestKind::ProviderManifest),
                )?,
                schema_count: read_u64(row, 8)?,
                input_count: read_u64(row, 9)?,
                output_count: read_u64(row, 10)?,
            })
        },
    )
}

fn read_provider_manifest_schemas(
    connection: &Connection,
    generation_id: i64,
) -> Result<Vec<StoreProviderManifestSchemaRow>, ProjectionError> {
    query_rows(
        connection,
        "SELECT provider_id, schema_version
         FROM provider_manifest_schemas WHERE generation_id = ?1
         ORDER BY provider_id, schema_version",
        generation_id,
        |row| {
            Ok(StoreProviderManifestSchemaRow {
                provider_id: row.get(0)?,
                schema_version: row.get(1)?,
            })
        },
    )
}

fn read_provider_manifest_inputs(
    connection: &Connection,
    generation_id: i64,
) -> Result<Vec<StoreProviderManifestInputRow>, ProjectionError> {
    query_rows(
        connection,
        "SELECT provider_id, input_name
         FROM provider_manifest_inputs WHERE generation_id = ?1
         ORDER BY provider_id, input_name",
        generation_id,
        |row| {
            Ok(StoreProviderManifestInputRow {
                provider_id: row.get(0)?,
                input: row.get(1)?,
            })
        },
    )
}

fn read_provider_manifest_outputs(
    connection: &Connection,
    generation_id: i64,
) -> Result<Vec<StoreProviderManifestOutputRow>, ProjectionError> {
    query_rows(
        connection,
        "SELECT provider_id, output_name
         FROM provider_manifest_outputs WHERE generation_id = ?1
         ORDER BY provider_id, output_name",
        generation_id,
        |row| {
            Ok(StoreProviderManifestOutputRow {
                provider_id: row.get(0)?,
                output: row.get(1)?,
            })
        },
    )
}

fn read_provider_generations(
    connection: &Connection,
    generation_id: i64,
) -> Result<
    (
        Vec<StoreProviderGenerationRow>,
        BTreeMap<i64, StoreProviderGenerationRow>,
    ),
    ProjectionError,
> {
    let decoded = query_rows(
        connection,
        "SELECT id, provider_id, provider_version, schema_version,
                output_digest_kind, output_digest_value, precision, validation,
                dependency_count, layer_count
         FROM provider_generations WHERE generation_id = ?1
         ORDER BY provider_id, provider_version, schema_version,
                  output_digest_kind, output_digest_value",
        generation_id,
        |row| {
            let precision: String = row.get(6)?;
            decode_precision_tier(&precision)?;
            let validation: String = row.get(7)?;
            decode_provider_validation_status(&validation)?;
            Ok((
                row.get::<_, i64>(0)?,
                StoreProviderGenerationRow {
                    provider_id: row.get(1)?,
                    provider_version: row.get(2)?,
                    schema_version: row.get(3)?,
                    output_digest: decode_digest(
                        &row.get::<_, String>(4)?,
                        &row.get::<_, String>(5)?,
                        Some(DigestKind::ProviderOutput),
                    )?,
                    precision,
                    validation,
                    dependency_count: read_u64(row, 8)?,
                    layer_count: read_u64(row, 9)?,
                },
            ))
        },
    )?;
    let mut rows = Vec::with_capacity(decoded.len());
    let mut handles = BTreeMap::new();
    for (handle, row) in decoded {
        if handles.insert(handle, row.clone()).is_some() {
            return Err(ProjectionError::InvalidMetadata);
        }
        rows.push(row);
    }
    Ok((rows, handles))
}

fn read_provider_dependencies(
    connection: &Connection,
    generation_id: i64,
    handles: &BTreeMap<i64, StoreProviderGenerationRow>,
) -> Result<Vec<StoreProviderDependencyRow>, ProjectionError> {
    query_rows(
        connection,
        "SELECT dependency.provider_generation_id,
                dependency.dependency_digest_kind, dependency.dependency_digest_value
         FROM provider_dependencies AS dependency
         JOIN provider_generations AS provider
           ON provider.generation_id = dependency.generation_id
          AND provider.id = dependency.provider_generation_id
         WHERE dependency.generation_id = ?1
         ORDER BY provider.provider_id, provider.provider_version, provider.schema_version,
                  provider.output_digest_kind, provider.output_digest_value,
                  dependency.ordinal",
        generation_id,
        |row| {
            let handle: i64 = row.get(0)?;
            let provider = handles
                .get(&handle)
                .ok_or(ProjectionError::InvalidMetadata)?;
            Ok(StoreProviderDependencyRow {
                provider_id: provider.provider_id.clone(),
                provider_version: provider.provider_version.clone(),
                schema_version: provider.schema_version.clone(),
                output_digest: provider.output_digest.clone(),
                dependency: decode_digest(
                    &row.get::<_, String>(1)?,
                    &row.get::<_, String>(2)?,
                    None,
                )?,
            })
        },
    )
}

type LayerReadProjection = (
    Vec<StoreLayerRow>,
    Vec<StoreLayerInputRow>,
    Vec<StoreLayerDependencyRow>,
    Vec<StoreLayerExtensionRow>,
    Vec<StoreLayerWarningRow>,
    BTreeMap<i64, LayerKey>,
);

fn read_layers(
    connection: &Connection,
    generation_id: i64,
) -> Result<LayerReadProjection, ProjectionError> {
    let mut inputs = read_layer_digest_groups(
        connection,
        generation_id,
        "SELECT child.layer_id, child.digest_kind, child.digest_value
         FROM layer_input_digests AS child
         JOIN layers AS layer ON layer.generation_id = child.generation_id AND layer.id = child.layer_id
         WHERE child.generation_id = ?1
         ORDER BY layer.semantic_ordinal, child.ordinal",
        None,
    )?;
    let mut dependencies = read_layer_digest_groups(
        connection,
        generation_id,
        "SELECT child.layer_id, child.digest_kind, child.digest_value
         FROM layer_dependency_digests AS child
         JOIN layers AS layer ON layer.generation_id = child.generation_id AND layer.id = child.layer_id
         WHERE child.generation_id = ?1
         ORDER BY layer.semantic_ordinal, child.ordinal",
        Some(DigestKind::DependencyLayer),
    )?;
    let mut extensions = read_layer_digest_groups(
        connection,
        generation_id,
        "SELECT child.layer_id, child.digest_kind, child.digest_value
         FROM layer_extension_digests AS child
         JOIN layers AS layer ON layer.generation_id = child.generation_id AND layer.id = child.layer_id
         WHERE child.generation_id = ?1
         ORDER BY layer.semantic_ordinal, child.ordinal",
        Some(DigestKind::ExtensionCode),
    )?;
    let mut warnings = BTreeMap::<i64, Vec<String>>::new();
    for (handle, warning) in query_rows(
        connection,
        "SELECT child.layer_id, child.warning_code
         FROM layer_warnings AS child
         JOIN layers AS layer ON layer.generation_id = child.generation_id AND layer.id = child.layer_id
         WHERE child.generation_id = ?1
         ORDER BY layer.semantic_ordinal, child.ordinal, child.warning_code",
        generation_id,
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
    )? {
        warnings.entry(handle).or_default().push(warning);
    }

    let decoded = query_rows(
        connection,
        "SELECT id, layer_type, provider_id, provider_version, schema_version,
                parameter_digest_kind, parameter_digest_value,
                lifecycle_digest_kind, lifecycle_digest_value,
                settings_digest_kind, settings_digest_value,
                toolchain_digest_kind, toolchain_digest_value,
                output_digest_kind, output_digest_value,
                payload_digest_kind, payload_digest_value,
                precision, validation, input_count, dependency_layer_count,
                extension_count, edge_count, warning_count
         FROM layers WHERE generation_id = ?1
         ORDER BY semantic_ordinal",
        generation_id,
        |row| {
            let handle: i64 = row.get(0)?;
            let layer_kind = decode_layer_kind(&row.get::<_, String>(1)?)?;
            let precision: String = row.get(17)?;
            decode_precision_tier(&precision)?;
            let validation: String = row.get(18)?;
            decode_provider_validation_status(&validation)?;
            let key = LayerKey {
                layer_kind,
                provider_id: row.get(2)?,
                provider_version: row.get(3)?,
                schema_version: row.get(4)?,
                parameter_digest: decode_digest(
                    &row.get::<_, String>(5)?,
                    &row.get::<_, String>(6)?,
                    None,
                )?,
                lifecycle_digest: decode_digest(
                    &row.get::<_, String>(7)?,
                    &row.get::<_, String>(8)?,
                    None,
                )?,
                analysis_settings_digest: decode_digest(
                    &row.get::<_, String>(9)?,
                    &row.get::<_, String>(10)?,
                    Some(DigestKind::AnalysisSettings),
                )?,
                toolchain_digest: decode_digest(
                    &row.get::<_, String>(11)?,
                    &row.get::<_, String>(12)?,
                    None,
                )?,
                input_digests: Arc::new(inputs.remove(&handle).unwrap_or_default()),
                dependency_layer_digests: Arc::new(
                    dependencies.remove(&handle).unwrap_or_default(),
                ),
                extension_digests: Arc::new(extensions.remove(&handle).unwrap_or_default()),
            };
            let warning_codes = warnings.remove(&handle).unwrap_or_default();
            Ok((
                handle,
                StoreLayerRow {
                    key,
                    output_digest: decode_digest(
                        &row.get::<_, String>(13)?,
                        &row.get::<_, String>(14)?,
                        Some(DigestKind::ProviderOutput),
                    )?,
                    payload_digest: decode_digest(
                        &row.get::<_, String>(15)?,
                        &row.get::<_, String>(16)?,
                        Some(DigestKind::LayerOutput),
                    )?,
                    precision,
                    validation,
                    input_count: read_u64(row, 19)?,
                    dependency_layer_count: read_u64(row, 20)?,
                    extension_count: read_u64(row, 21)?,
                    edge_count: read_u64(row, 22)?,
                    warning_count: read_u64(row, 23)?,
                },
                warning_codes,
            ))
        },
    )?;
    if !inputs.is_empty()
        || !dependencies.is_empty()
        || !extensions.is_empty()
        || !warnings.is_empty()
    {
        return Err(ProjectionError::InvalidMetadata);
    }

    let mut rows = Vec::with_capacity(decoded.len());
    let mut input_rows = Vec::new();
    let mut dependency_rows = Vec::new();
    let mut extension_rows = Vec::new();
    let mut warning_rows = Vec::new();
    let mut handles = BTreeMap::new();
    for (handle, row, warning_codes) in decoded {
        if handles.insert(handle, row.key.clone()).is_some() {
            return Err(ProjectionError::InvalidMetadata);
        }
        input_rows.extend(
            row.key
                .input_digests
                .iter()
                .map(|digest| StoreLayerInputRow {
                    key: row.key.clone(),
                    digest: digest.clone(),
                }),
        );
        dependency_rows.extend(row.key.dependency_layer_digests.iter().map(|digest| {
            StoreLayerDependencyRow {
                key: row.key.clone(),
                digest: digest.clone(),
            }
        }));
        extension_rows.extend(row.key.extension_digests.iter().map(|digest| {
            StoreLayerExtensionRow {
                key: row.key.clone(),
                digest: digest.clone(),
            }
        }));
        warning_rows.extend(
            warning_codes
                .into_iter()
                .map(|warning_code| StoreLayerWarningRow {
                    key: row.key.clone(),
                    warning_code,
                }),
        );
        rows.push(row);
    }
    Ok((
        rows,
        input_rows,
        dependency_rows,
        extension_rows,
        warning_rows,
        handles,
    ))
}

fn read_layer_digest_groups(
    connection: &Connection,
    generation_id: i64,
    sql: &str,
    expected_kind: Option<DigestKind>,
) -> Result<BTreeMap<i64, Vec<Digest>>, ProjectionError> {
    let mut groups = BTreeMap::new();
    for (handle, digest) in query_rows(connection, sql, generation_id, |row| {
        Ok((
            row.get::<_, i64>(0)?,
            decode_digest(
                &row.get::<_, String>(1)?,
                &row.get::<_, String>(2)?,
                expected_kind,
            )?,
        ))
    })? {
        groups.entry(handle).or_insert_with(Vec::new).push(digest);
    }
    Ok(groups)
}

type SummaryReadProjection = (
    Vec<StoreSummaryRow>,
    Vec<StoreSummaryDependencyRow>,
    BTreeMap<i64, SummaryKey>,
);

fn read_summaries(
    connection: &Connection,
    generation_id: i64,
) -> Result<SummaryReadProjection, ProjectionError> {
    let mut dependencies = BTreeMap::<i64, Vec<Digest>>::new();
    for (handle, digest) in query_rows(
        connection,
        "SELECT child.summary_id, child.digest_kind, child.digest_value
         FROM summary_dependency_digests AS child
         JOIN summaries AS summary
           ON summary.generation_id = child.generation_id AND summary.id = child.summary_id
         WHERE child.generation_id = ?1
         ORDER BY summary.semantic_ordinal, child.ordinal",
        generation_id,
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                decode_digest(
                    &row.get::<_, String>(1)?,
                    &row.get::<_, String>(2)?,
                    Some(DigestKind::SummaryDependency),
                )?,
            ))
        },
    )? {
        dependencies.entry(handle).or_default().push(digest);
    }
    let decoded = query_rows(
        connection,
        "SELECT id, callable_stable_key, summary_domain, summary_version,
                body_shape_digest_kind, body_shape_digest_value,
                extension_digest_kind, extension_digest_value, dependency_count
         FROM summaries WHERE generation_id = ?1
         ORDER BY semantic_ordinal",
        generation_id,
        |row| {
            let handle: i64 = row.get(0)?;
            Ok((
                handle,
                StoreSummaryRow {
                    key: SummaryKey {
                        callable_stable_key: row.get(1)?,
                        summary_domain: row.get(2)?,
                        summary_version: row.get(3)?,
                        body_shape_digest: decode_digest(
                            &row.get::<_, String>(4)?,
                            &row.get::<_, String>(5)?,
                            None,
                        )?,
                        dependency_summary_digests: dependencies
                            .remove(&handle)
                            .unwrap_or_default(),
                        extension_digest: decode_digest(
                            &row.get::<_, String>(6)?,
                            &row.get::<_, String>(7)?,
                            Some(DigestKind::ExtensionCode),
                        )?,
                    },
                    dependency_count: read_u64(row, 8)?,
                },
            ))
        },
    )?;
    if !dependencies.is_empty() {
        return Err(ProjectionError::InvalidMetadata);
    }
    let mut rows = Vec::with_capacity(decoded.len());
    let mut child_rows = Vec::new();
    let mut handles = BTreeMap::new();
    for (handle, row) in decoded {
        if handles.insert(handle, row.key.clone()).is_some() {
            return Err(ProjectionError::InvalidMetadata);
        }
        child_rows.extend(row.key.dependency_summary_digests.iter().map(|dependency| {
            StoreSummaryDependencyRow {
                key: row.key.clone(),
                dependency: dependency.clone(),
            }
        }));
        rows.push(row);
    }
    Ok((rows, child_rows, handles))
}

type QueryReadProjection = (
    Vec<StoreQueryRow>,
    Vec<StoreQueryInputRow>,
    Vec<StoreQueryLayerRow>,
    BTreeMap<i64, QueryKey>,
);

fn read_queries(
    connection: &Connection,
    generation_id: i64,
) -> Result<QueryReadProjection, ProjectionError> {
    let mut inputs = BTreeMap::<i64, Vec<InputDependencyKey>>::new();
    for (handle, input) in query_rows(
        connection,
        "SELECT child.query_id, child.input_kind, child.stable_key,
                child.digest_kind, child.digest_value, child.status
         FROM query_inputs AS child
         JOIN queries AS query
           ON query.generation_id = child.generation_id AND query.id = child.query_id
         WHERE child.generation_id = ?1
         ORDER BY query.semantic_ordinal, child.ordinal",
        generation_id,
        |row| {
            let encoded = EncodedInputDependency {
                kind: row.get(1)?,
                stable_key: row.get(2)?,
                digest_kind: row.get(3)?,
                digest_value: row.get(4)?,
                status: row.get(5)?,
            };
            Ok((row.get::<_, i64>(0)?, decode_input_dependency(&encoded)?))
        },
    )? {
        inputs.entry(handle).or_default().push(input);
    }
    let mut layers = BTreeMap::<i64, Vec<Digest>>::new();
    for (handle, digest) in query_rows(
        connection,
        "SELECT child.query_id, child.digest_kind, child.digest_value
         FROM query_layer_digests AS child
         JOIN queries AS query
           ON query.generation_id = child.generation_id AND query.id = child.query_id
         WHERE child.generation_id = ?1
         ORDER BY query.semantic_ordinal, child.ordinal",
        generation_id,
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                decode_digest(
                    &row.get::<_, String>(1)?,
                    &row.get::<_, String>(2)?,
                    Some(DigestKind::ProviderOutput),
                )?,
            ))
        },
    )? {
        layers.entry(handle).or_default().push(digest);
    }
    let decoded = query_rows(
        connection,
        "SELECT id, query_type, query_version, parameter_digest_kind,
                parameter_digest_value, budget_digest_kind, budget_digest_value,
                precision_tier, result_digest_kind, result_digest_value, precision,
                provenance, input_count, layer_count, edge_count
         FROM queries WHERE generation_id = ?1
         ORDER BY semantic_ordinal",
        generation_id,
        |row| {
            let handle: i64 = row.get(0)?;
            let raw_inputs = inputs.remove(&handle).unwrap_or_default();
            let dependency_inputs: QueryDependencyInputs = serde_json::from_value(
                serde_json::to_value(&raw_inputs).map_err(|_| ProjectionError::InvalidMetadata)?,
            )
            .map_err(|_| ProjectionError::InvalidMetadata)?;
            if dependency_inputs.as_slice() != raw_inputs {
                return Err(ProjectionError::InvalidMetadata);
            }
            let precision_tier = decode_precision_tier(&row.get::<_, String>(7)?)?;
            let precision: String = row.get(10)?;
            decode_precision_tier(&precision)?;
            Ok((
                handle,
                StoreQueryRow {
                    key: QueryKey {
                        query_kind: row.get(1)?,
                        query_version: row.get(2)?,
                        parameter_digest: decode_digest(
                            &row.get::<_, String>(3)?,
                            &row.get::<_, String>(4)?,
                            Some(DigestKind::QueryParameters),
                        )?,
                        dependency_inputs,
                        layer_digests: layers.remove(&handle).unwrap_or_default(),
                        budget_digest: decode_digest(
                            &row.get::<_, String>(5)?,
                            &row.get::<_, String>(6)?,
                            Some(DigestKind::Budget),
                        )?,
                        precision_tier,
                    },
                    result_digest: decode_digest(
                        &row.get::<_, String>(8)?,
                        &row.get::<_, String>(9)?,
                        Some(DigestKind::ProviderOutput),
                    )?,
                    precision,
                    provenance: row.get(11)?,
                    input_count: read_u64(row, 12)?,
                    layer_count: read_u64(row, 13)?,
                    edge_count: read_u64(row, 14)?,
                },
            ))
        },
    )?;
    if !inputs.is_empty() || !layers.is_empty() {
        return Err(ProjectionError::InvalidMetadata);
    }
    let mut rows = Vec::with_capacity(decoded.len());
    let mut input_rows = Vec::new();
    let mut layer_rows = Vec::new();
    let mut handles = BTreeMap::new();
    for (handle, row) in decoded {
        if handles.insert(handle, row.key.clone()).is_some() {
            return Err(ProjectionError::InvalidMetadata);
        }
        input_rows.extend(row.key.dependency_inputs.as_slice().iter().map(|input| {
            StoreQueryInputRow {
                key: row.key.clone(),
                input: input.clone(),
            }
        }));
        layer_rows.extend(
            row.key
                .layer_digests
                .iter()
                .map(|layer_digest| StoreQueryLayerRow {
                    key: row.key.clone(),
                    layer_digest: layer_digest.clone(),
                }),
        );
        rows.push(row);
    }
    Ok((rows, input_rows, layer_rows, handles))
}

fn read_facts(
    connection: &Connection,
    generation_id: i64,
) -> Result<Vec<StoreFactRow>, ProjectionError> {
    query_rows(
        connection,
        "SELECT family, stable_key, producer_id, producer_layer_key,
                precision, confidence, validation, payload_digest
         FROM fact_metadata WHERE generation_id = ?1
         ORDER BY family, stable_key",
        generation_id,
        |row| {
            let family: String = row.get(0)?;
            decode_fact_family(&family)?;
            let precision: String = row.get(4)?;
            decode_fact_precision(&precision)?;
            let confidence: String = row.get(5)?;
            decode_fact_confidence(&confidence)?;
            let validation: String = row.get(6)?;
            decode_validation_status(&validation)?;
            Ok(StoreFactRow {
                family,
                stable_key: row.get(1)?,
                producer_id: row.get(2)?,
                layer_id: row.get(3)?,
                precision,
                confidence,
                validation,
                payload_digest: row.get(7)?,
            })
        },
    )
}

#[derive(Debug)]
struct RawEndpoint {
    node_kind: String,
    input_kind: Option<String>,
    input_stable_key: Option<String>,
    input_digest_kind: Option<String>,
    input_digest_value: Option<String>,
    input_status: Option<String>,
    layer_id: Option<i64>,
    query_id: Option<i64>,
    summary_id: Option<i64>,
}

fn read_dependency_edges(
    connection: &Connection,
    generation_id: i64,
    layers: &BTreeMap<i64, LayerKey>,
    queries: &BTreeMap<i64, QueryKey>,
    summaries: &BTreeMap<i64, SummaryKey>,
) -> Result<Vec<StoreDependencyEdgeRow>, ProjectionError> {
    query_rows(
        connection,
        "SELECT
             from_node_kind, from_input_kind, from_input_stable_key,
             from_input_digest_kind, from_input_digest_value, from_input_status,
             from_layer_id, from_query_id, from_summary_id,
             to_node_kind, to_input_kind, to_input_stable_key,
             to_input_digest_kind, to_input_digest_value, to_input_status,
             to_layer_id, to_query_id, to_summary_id,
             dependency_kind, required_shape
         FROM dependency_edges WHERE generation_id = ?1
         ORDER BY ordinal",
        generation_id,
        |row| {
            let from = RawEndpoint {
                node_kind: row.get(0)?,
                input_kind: row.get(1)?,
                input_stable_key: row.get(2)?,
                input_digest_kind: row.get(3)?,
                input_digest_value: row.get(4)?,
                input_status: row.get(5)?,
                layer_id: row.get(6)?,
                query_id: row.get(7)?,
                summary_id: row.get(8)?,
            };
            let to = RawEndpoint {
                node_kind: row.get(9)?,
                input_kind: row.get(10)?,
                input_stable_key: row.get(11)?,
                input_digest_kind: row.get(12)?,
                input_digest_value: row.get(13)?,
                input_status: row.get(14)?,
                layer_id: row.get(15)?,
                query_id: row.get(16)?,
                summary_id: row.get(17)?,
            };
            Ok(StoreDependencyEdgeRow {
                from: decode_endpoint(from, layers, queries, summaries)?,
                to: decode_endpoint(to, layers, queries, summaries)?,
                kind: decode_dependency_kind(&row.get::<_, String>(18)?)?,
                required_shape: decode_shape_kind(&row.get::<_, String>(19)?)?,
            })
        },
    )
}

fn decode_endpoint(
    endpoint: RawEndpoint,
    layers: &BTreeMap<i64, LayerKey>,
    queries: &BTreeMap<i64, QueryKey>,
    summaries: &BTreeMap<i64, SummaryKey>,
) -> Result<CacheNode, ProjectionError> {
    let kind = decode_cache_node_kind(&endpoint.node_kind)?;
    match kind {
        crate::analysis_kernel::incremental::CacheNodeKind::DependencyInput => {
            if endpoint.layer_id.is_some()
                || endpoint.query_id.is_some()
                || endpoint.summary_id.is_some()
            {
                return Err(ProjectionError::InvalidMetadata);
            }
            let encoded = EncodedInputDependency {
                kind: endpoint
                    .input_kind
                    .ok_or(ProjectionError::InvalidMetadata)?,
                stable_key: endpoint
                    .input_stable_key
                    .ok_or(ProjectionError::InvalidMetadata)?,
                digest_kind: endpoint
                    .input_digest_kind
                    .ok_or(ProjectionError::InvalidMetadata)?,
                digest_value: endpoint
                    .input_digest_value
                    .ok_or(ProjectionError::InvalidMetadata)?,
                status: endpoint
                    .input_status
                    .ok_or(ProjectionError::InvalidMetadata)?,
            };
            Ok(CacheNode::DependencyInput(decode_input_dependency(
                &encoded,
            )?))
        }
        crate::analysis_kernel::incremental::CacheNodeKind::Layer => {
            require_empty_input_endpoint(&endpoint)?;
            let handle = endpoint.layer_id.ok_or(ProjectionError::InvalidMetadata)?;
            if endpoint.query_id.is_some() || endpoint.summary_id.is_some() {
                return Err(ProjectionError::InvalidMetadata);
            }
            Ok(CacheNode::Layer(
                layers
                    .get(&handle)
                    .cloned()
                    .ok_or(ProjectionError::InvalidMetadata)?,
            ))
        }
        crate::analysis_kernel::incremental::CacheNodeKind::Query => {
            require_empty_input_endpoint(&endpoint)?;
            let handle = endpoint.query_id.ok_or(ProjectionError::InvalidMetadata)?;
            if endpoint.layer_id.is_some() || endpoint.summary_id.is_some() {
                return Err(ProjectionError::InvalidMetadata);
            }
            Ok(CacheNode::Query(
                queries
                    .get(&handle)
                    .cloned()
                    .ok_or(ProjectionError::InvalidMetadata)?,
            ))
        }
        crate::analysis_kernel::incremental::CacheNodeKind::Summary => {
            require_empty_input_endpoint(&endpoint)?;
            let handle = endpoint
                .summary_id
                .ok_or(ProjectionError::InvalidMetadata)?;
            if endpoint.layer_id.is_some() || endpoint.query_id.is_some() {
                return Err(ProjectionError::InvalidMetadata);
            }
            Ok(CacheNode::Summary(
                summaries
                    .get(&handle)
                    .cloned()
                    .ok_or(ProjectionError::InvalidMetadata)?,
            ))
        }
        crate::analysis_kernel::incremental::CacheNodeKind::Diagnostic => {
            Err(ProjectionError::InvalidMetadata)
        }
    }
}

fn require_empty_input_endpoint(endpoint: &RawEndpoint) -> Result<(), ProjectionError> {
    if endpoint.input_kind.is_some()
        || endpoint.input_stable_key.is_some()
        || endpoint.input_digest_kind.is_some()
        || endpoint.input_digest_value.is_some()
        || endpoint.input_status.is_some()
    {
        Err(ProjectionError::InvalidMetadata)
    } else {
        Ok(())
    }
}

fn read_validation_events(
    connection: &Connection,
    generation_id: i64,
) -> Result<Vec<StoreValidationEventRow>, ProjectionError> {
    query_rows(
        connection,
        "SELECT event_kind, status, issue_count, digest_kind, digest_value
         FROM validation_events WHERE generation_id = ?1
         ORDER BY event_kind, status",
        generation_id,
        |row| {
            let kind: String = row.get(0)?;
            decode_validation_event_kind(&kind)?;
            let status: String = row.get(1)?;
            decode_validation_event_status(&status)?;
            Ok(StoreValidationEventRow {
                kind,
                status,
                issue_count: read_u64(row, 2)?,
                digest: decode_digest(
                    &row.get::<_, String>(3)?,
                    &row.get::<_, String>(4)?,
                    Some(DigestKind::ValidationEvent),
                )?,
            })
        },
    )
}

fn read_stats(
    connection: &Connection,
    generation_id: i64,
) -> Result<StoreGenerationStats, ProjectionError> {
    let mut rows = query_rows(
        connection,
        "SELECT
             input_file_count, input_component_count, input_detail_count,
             analysis_setting_count, capability_count, provider_schema_count,
             provider_manifest_count, provider_generation_count, layer_count,
             summary_count, query_count, fact_count, dependency_edge_count,
             validation_event_count,
             input_digest_kind, input_digest_value,
             provider_manifest_digest_kind, provider_manifest_digest_value,
             provider_output_digest_kind, provider_output_digest_value,
             layer_digest_kind, layer_digest_value,
             summary_digest_kind, summary_digest_value,
             query_digest_kind, query_digest_value,
             fact_digest_kind, fact_digest_value,
             dependency_digest_kind, dependency_digest_value,
             validation_digest_kind, validation_digest_value,
             input_logical_bytes, provider_logical_bytes, layer_logical_bytes,
             summary_logical_bytes, query_logical_bytes, fact_logical_bytes,
             dependency_logical_bytes, validation_logical_bytes, semantic_logical_bytes
         FROM generation_stats WHERE generation_id = ?1",
        generation_id,
        |row| {
            Ok(StoreGenerationStats {
                input_file_count: read_u64(row, 0)?,
                input_component_count: read_u64(row, 1)?,
                input_detail_count: read_u64(row, 2)?,
                analysis_setting_count: read_u64(row, 3)?,
                capability_count: read_u64(row, 4)?,
                provider_schema_count: read_u64(row, 5)?,
                provider_manifest_count: read_u64(row, 6)?,
                provider_generation_count: read_u64(row, 7)?,
                layer_count: read_u64(row, 8)?,
                summary_count: read_u64(row, 9)?,
                query_count: read_u64(row, 10)?,
                fact_count: read_u64(row, 11)?,
                dependency_edge_count: read_u64(row, 12)?,
                validation_event_count: read_u64(row, 13)?,
                input_digest: decode_digest(
                    &row.get::<_, String>(14)?,
                    &row.get::<_, String>(15)?,
                    Some(DigestKind::InputSnapshot),
                )?,
                provider_manifest_digest: decode_digest(
                    &row.get::<_, String>(16)?,
                    &row.get::<_, String>(17)?,
                    Some(DigestKind::ProviderManifest),
                )?,
                provider_output_digest: decode_digest(
                    &row.get::<_, String>(18)?,
                    &row.get::<_, String>(19)?,
                    Some(DigestKind::ProviderOutput),
                )?,
                layer_digest: decode_digest(
                    &row.get::<_, String>(20)?,
                    &row.get::<_, String>(21)?,
                    Some(DigestKind::Layer),
                )?,
                summary_digest: decode_digest(
                    &row.get::<_, String>(22)?,
                    &row.get::<_, String>(23)?,
                    Some(DigestKind::Summary),
                )?,
                query_digest: decode_digest(
                    &row.get::<_, String>(24)?,
                    &row.get::<_, String>(25)?,
                    Some(DigestKind::Query),
                )?,
                fact_digest: decode_digest(
                    &row.get::<_, String>(26)?,
                    &row.get::<_, String>(27)?,
                    Some(DigestKind::FactMetadata),
                )?,
                dependency_digest: decode_digest(
                    &row.get::<_, String>(28)?,
                    &row.get::<_, String>(29)?,
                    Some(DigestKind::Dependency),
                )?,
                validation_digest: decode_digest(
                    &row.get::<_, String>(30)?,
                    &row.get::<_, String>(31)?,
                    Some(DigestKind::ValidationEvent),
                )?,
                input_logical_bytes: read_u64(row, 32)?,
                provider_logical_bytes: read_u64(row, 33)?,
                layer_logical_bytes: read_u64(row, 34)?,
                summary_logical_bytes: read_u64(row, 35)?,
                query_logical_bytes: read_u64(row, 36)?,
                fact_logical_bytes: read_u64(row, 37)?,
                dependency_logical_bytes: read_u64(row, 38)?,
                validation_logical_bytes: read_u64(row, 39)?,
                semantic_logical_bytes: read_u64(row, 40)?,
            })
        },
    )?;
    if rows.len() != 1 {
        return Err(ProjectionError::InvalidMetadata);
    }
    Ok(rows.remove(0))
}

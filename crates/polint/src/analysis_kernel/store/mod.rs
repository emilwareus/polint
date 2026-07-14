//! Private durable semantic-store boundary.
//!
//! This module owns database paths, connection policy, migrations, and raw SQL.
//! Callers receive typed configuration and status values only; rusqlite types do
//! not cross this boundary.

use std::path::{Path, PathBuf};

#[cfg(test)]
use std::cell::Cell;

use crate::analysis_kernel::incremental::{Digest, ValidatedRunMetadata};

mod commit_plan;
mod connection;
mod generation;
mod migrations;
mod schema;

#[cfg(test)]
pub(crate) use connection::{HeldWriterConnection, StoreFixtureSnapshot};
#[cfg(test)]
pub(crate) const CURRENT_SCHEMA_VERSION_FOR_TEST: i32 = migrations::CURRENT_SCHEMA_VERSION;

#[cfg(test)]
pub(crate) fn install_future_fixture_for_test(path: &Path) -> Result<(), ()> {
    connection::install_future_fixture_for_test(path).map_err(|_| ())
}

#[cfg(test)]
pub(crate) fn install_invalid_fixture_for_test(path: &Path) -> Result<(), ()> {
    connection::install_invalid_fixture_for_test(path).map_err(|_| ())
}

#[cfg(test)]
pub(crate) fn fixture_snapshot_for_test(path: &Path) -> Result<StoreFixtureSnapshot, ()> {
    connection::fixture_snapshot_for_test(path).map_err(|_| ())
}

#[cfg(test)]
pub(crate) fn hold_writer_connection_for_test(path: &Path) -> Result<HeldWriterConnection, ()> {
    connection::hold_writer_connection_for_test(path).map_err(|_| ())
}

#[cfg(test)]
pub(crate) fn current_schema_is_valid_for_test(path: &Path) -> bool {
    connection::current_schema_is_valid_for_test(path)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StoreConfig {
    path: PathBuf,
    enabled: bool,
}

impl StoreConfig {
    pub(crate) fn new(path: impl Into<PathBuf>, enabled: bool) -> Self {
        Self {
            path: path.into(),
            enabled,
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StoreStatus {
    Disabled,
    Ready,
    BusySkipped,
    Skipped(StoreSkipReason),
    RebuildNeeded(StoreRebuildReason),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StoreSkipReason {
    FutureSchema { found: i32, supported: i32 },
    UnsafePath,
    OpenFailed,
    WorkspaceMismatch,
    StaleReservation,
    InvalidPlan,
    CommitFailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StoreRebuildReason {
    Corrupt,
    InvalidSchema,
    InvalidMetadata,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::analysis_kernel) struct StoreStatistics {
    pub(in crate::analysis_kernel) planned_semantic_row_count: u64,
    pub(in crate::analysis_kernel) semantic_logical_bytes: u64,
    pub(in crate::analysis_kernel) input_digest: Digest,
    pub(in crate::analysis_kernel) dependency_digest: Digest,
    pub(in crate::analysis_kernel) validation_digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::analysis_kernel) struct StoreOutcome {
    pub(in crate::analysis_kernel) status: StoreStatus,
    pub(in crate::analysis_kernel) statistics: Option<StoreStatistics>,
}

impl StoreOutcome {
    pub(in crate::analysis_kernel) fn disabled() -> Self {
        Self {
            status: StoreStatus::Disabled,
            statistics: None,
        }
    }

    pub(in crate::analysis_kernel) fn invalid_metadata() -> Self {
        Self {
            status: StoreStatus::Skipped(StoreSkipReason::InvalidPlan),
            statistics: None,
        }
    }
}

impl StoreStatistics {
    fn from_plan(plan: &commit_plan::StoreCommitPlan) -> Self {
        let semantic = &plan.semantic;
        let planned_semantic_row_count = [
            1,
            1,
            semantic.files.len(),
            semantic.input_components.len(),
            semantic.input_details.len(),
            semantic.analysis_settings.len(),
            semantic.capabilities.len(),
            semantic.capability_requesters.len(),
            semantic.provider_schemas.len(),
            semantic.provider_schema_versions.len(),
            semantic.provider_manifests.len(),
            semantic.provider_manifest_schemas.len(),
            semantic.provider_manifest_inputs.len(),
            semantic.provider_manifest_outputs.len(),
            semantic.provider_generations.len(),
            semantic.provider_dependencies.len(),
            semantic.layers.len(),
            semantic.layer_inputs.len(),
            semantic.layer_dependencies.len(),
            semantic.layer_extensions.len(),
            semantic.layer_warnings.len(),
            semantic.summaries.len(),
            semantic.summary_dependencies.len(),
            semantic.queries.len(),
            semantic.query_inputs.len(),
            semantic.query_layers.len(),
            semantic.facts.len(),
            1,
            semantic.diagnostics.len(),
            semantic.diagnostic_requested_views.len(),
            semantic.dependency_edges.len(),
            semantic.validation_events.len(),
            1,
        ]
        .into_iter()
        .fold(0_u64, |total, count| {
            total.saturating_add(u64::try_from(count).unwrap_or(u64::MAX))
        });
        let stats = &semantic.stats;
        Self {
            planned_semantic_row_count,
            semantic_logical_bytes: stats.semantic_logical_bytes,
            input_digest: stats.input_digest.clone(),
            dependency_digest: stats.dependency_digest.clone(),
            validation_digest: stats.validation_digest.clone(),
        }
    }
}

#[cfg(test)]
thread_local! {
    static MATERIALIZATION_COUNTERS: Cell<(usize, usize, usize)> = const {
        Cell::new((0, 0, 0))
    };
    static NEXT_COMMIT_FAILURE: Cell<Option<schema::GenerationFailureStage>> = const {
        Cell::new(None)
    };
}

pub(in crate::analysis_kernel) fn record_handoff_materialization() {
    #[cfg(test)]
    MATERIALIZATION_COUNTERS.with(|counters| {
        let (_, plans, io) = counters.get();
        counters.set((counters.get().0.saturating_add(1), plans, io));
    });
}

fn record_plan_materialization() {
    #[cfg(test)]
    MATERIALIZATION_COUNTERS.with(|counters| {
        let (handoffs, plans, io) = counters.get();
        counters.set((handoffs, plans.saturating_add(1), io));
    });
}

pub(super) fn record_store_io_attempt() {
    #[cfg(test)]
    MATERIALIZATION_COUNTERS.with(|counters| {
        let (handoffs, plans, io) = counters.get();
        counters.set((handoffs, plans, io.saturating_add(1)));
    });
}

#[cfg(test)]
pub(crate) fn reset_materialization_counters_for_test() {
    MATERIALIZATION_COUNTERS.with(|counters| counters.set((0, 0, 0)));
}

#[cfg(test)]
pub(crate) fn materialization_counters_for_test() -> (usize, usize, usize) {
    MATERIALIZATION_COUNTERS.with(Cell::get)
}

#[cfg(test)]
pub(crate) fn inject_next_commit_failure_for_test() {
    NEXT_COMMIT_FAILURE.with(|next| next.set(Some(schema::GenerationFailureStage::StoreRunInput)));
}

#[cfg(test)]
#[derive(Clone, Copy)]
pub(crate) enum StoredGenerationFixtureState {
    Pending,
    AuditedFailed,
}

#[cfg(test)]
pub(crate) fn install_generation_fixture_for_test(
    config: &StoreConfig,
    validated: &ValidatedRunMetadata,
    state: StoredGenerationFixtureState,
) -> Result<(), StoreStatus> {
    let plan = commit_plan::StoreCommitPlan::from_validated_run(validated)
        .map_err(|_| StoreStatus::Skipped(StoreSkipReason::InvalidPlan))?;
    let reservation = generation::reserve_only_for_test(config, &plan)?;
    if matches!(state, StoredGenerationFixtureState::Pending) {
        return Ok(());
    }
    let mut writer = connection::open_writer(config.path()).map_err(map_connection_error)?;
    match generation::audit_reservation_for_test(
        &mut writer,
        &reservation,
        schema::GenerationFailureReason::WriteFailed,
        schema::GenerationFailureStage::StoreRunInput,
    ) {
        generation::AuditOutcome::Recorded => Ok(()),
        generation::AuditOutcome::Busy => Err(StoreStatus::BusySkipped),
        generation::AuditOutcome::NotTrusted => Err(StoreStatus::RebuildNeeded(
            StoreRebuildReason::InvalidMetadata,
        )),
    }
}

pub(crate) struct SemanticStore;

impl SemanticStore {
    pub(in crate::analysis_kernel) fn commit_validated_run(
        config: &StoreConfig,
        validated: &ValidatedRunMetadata,
    ) -> StoreOutcome {
        if !config.is_enabled() {
            return StoreOutcome::disabled();
        }

        record_plan_materialization();
        let plan = match commit_plan::StoreCommitPlan::from_validated_run(validated) {
            Ok(plan) => plan,
            Err(_) => return StoreOutcome::invalid_metadata(),
        };
        #[cfg(test)]
        let status = NEXT_COMMIT_FAILURE.with(|next| {
            next.take().map_or_else(
                || generation::commit_generation(config, &plan),
                |stage| {
                    generation::commit_generation_with_control(
                        config,
                        &plan,
                        generation::CommitControl::fail_after(stage),
                    )
                },
            )
        });
        #[cfg(not(test))]
        let status = generation::commit_generation(config, &plan);
        if status != StoreStatus::Ready {
            return StoreOutcome {
                status,
                statistics: None,
            };
        }

        match generation::read_active_complete(config, &plan.semantic.identities.workspace) {
            Ok(Some(active)) if active.plan == plan => StoreOutcome {
                status: StoreStatus::Ready,
                statistics: Some(StoreStatistics::from_plan(&active.plan)),
            },
            Ok(Some(_)) | Ok(None) => StoreOutcome {
                status: StoreStatus::RebuildNeeded(StoreRebuildReason::InvalidMetadata),
                statistics: None,
            },
            Err(status) => StoreOutcome {
                status,
                statistics: None,
            },
        }
    }

    fn maintain(config: &StoreConfig) -> StoreStatus {
        if !config.is_enabled() {
            return StoreStatus::Disabled;
        }

        if let Err(status) = prepare_store_path(config.path()) {
            return status;
        }

        let mut writer = match connection::open_writer(config.path()) {
            Ok(writer) => writer,
            Err(error) => return map_connection_error(error),
        };
        match connection::try_writer_lease(&mut writer) {
            Ok(connection::LeaseStatus::Acquired) => StoreStatus::Ready,
            Ok(connection::LeaseStatus::Busy) => StoreStatus::BusySkipped,
            Err(error) => map_connection_error(error),
        }
    }
}

#[cfg(test)]
pub(crate) fn initialize_empty_fixture_for_test(config: &StoreConfig) -> StoreStatus {
    SemanticStore::maintain(config)
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Explicit rebuild remains the controlled recovery path for an owned invalid cache."
    )
)]
pub(crate) fn rebuild_owned_cache_store(config: &StoreConfig, candidate: &Path) -> StoreStatus {
    if !config.is_enabled() {
        return StoreStatus::Disabled;
    }
    if candidate != config.path() {
        return StoreStatus::Skipped(StoreSkipReason::UnsafePath);
    }

    let status = SemanticStore::maintain(config);
    if !matches!(status, StoreStatus::RebuildNeeded(_)) {
        return status;
    }

    let Some((cache_root, store_dir)) = store_ownership(config.path()) else {
        return StoreStatus::Skipped(StoreSkipReason::UnsafePath);
    };
    let Some(owned_path) =
        crate::repo_fs::managed_existing_file(cache_root, store_dir, config.path())
    else {
        return StoreStatus::Skipped(StoreSkipReason::UnsafePath);
    };
    if std::fs::remove_file(owned_path).is_err() {
        return StoreStatus::Skipped(StoreSkipReason::OpenFailed);
    }

    SemanticStore::maintain(config)
}

fn prepare_store_path(path: &Path) -> Result<(), StoreStatus> {
    let Some((cache_root, store_dir)) = store_ownership(path) else {
        return Err(StoreStatus::Skipped(StoreSkipReason::UnsafePath));
    };
    crate::repo_fs::ensure_managed_path(cache_root, store_dir).map_err(map_path_error)?;
    crate::repo_fs::create_dir_all_no_symlink(store_dir).map_err(map_path_error)?;
    crate::repo_fs::ensure_managed_path(cache_root, path).map_err(map_path_error)?;
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(StoreStatus::Skipped(StoreSkipReason::UnsafePath))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(StoreStatus::Skipped(StoreSkipReason::OpenFailed)),
    }
}

fn store_ownership(path: &Path) -> Option<(&Path, &Path)> {
    let store_dir = path.parent()?;
    let cache_root = store_dir.parent()?;
    Some((cache_root, store_dir))
}

fn map_path_error(error: crate::repo_fs::RepoFileReadError) -> StoreStatus {
    use crate::repo_fs::RepoFileReadError;

    match error {
        RepoFileReadError::AbsolutePath
        | RepoFileReadError::EscapesRepo
        | RepoFileReadError::NotFile
        | RepoFileReadError::NotDirectory => StoreStatus::Skipped(StoreSkipReason::UnsafePath),
        _ => StoreStatus::Skipped(StoreSkipReason::OpenFailed),
    }
}

fn map_connection_error(error: connection::ConnectionError) -> StoreStatus {
    match error {
        connection::ConnectionError::Busy => StoreStatus::BusySkipped,
        connection::ConnectionError::FutureSchema { found, supported } => {
            StoreStatus::Skipped(StoreSkipReason::FutureSchema { found, supported })
        }
        connection::ConnectionError::InvalidSchema => {
            StoreStatus::RebuildNeeded(StoreRebuildReason::InvalidSchema)
        }
        connection::ConnectionError::Corrupt => {
            StoreStatus::RebuildNeeded(StoreRebuildReason::Corrupt)
        }
        connection::ConnectionError::Policy | connection::ConnectionError::Other => {
            StoreStatus::Skipped(StoreSkipReason::OpenFailed)
        }
    }
}

#[cfg(test)]
mod tests;

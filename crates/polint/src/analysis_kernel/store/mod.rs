//! Private durable semantic-store boundary.
//!
//! This module owns database paths, connection policy, migrations, and raw SQL.
//! Callers receive typed configuration and status values only; rusqlite types do
//! not cross this boundary.

use std::path::{Path, PathBuf};

mod connection;
mod generation;
mod migrations;

pub(crate) use generation::{GenerationError, GenerationHandle};
#[cfg(test)]
pub(crate) use generation::{GenerationStatus, PublicationFailurePoint};

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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StoreRebuildReason {
    Corrupt,
    InvalidSchema,
}

pub(crate) struct SemanticStore;

impl SemanticStore {
    pub(crate) fn maintain(config: &StoreConfig) -> StoreStatus {
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

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the private lifecycle is reserved for semantic metadata publication"
        )
    )]
    pub(crate) fn reserve_generation(
        config: &StoreConfig,
    ) -> Result<GenerationHandle, GenerationError> {
        prepare_generation_store(config)?;
        let mut writer = connection::open_writer(config.path()).map_err(GenerationError::from)?;
        generation::reserve(&mut writer)
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the private lifecycle is reserved for semantic metadata publication"
        )
    )]
    pub(crate) fn publish_generation(
        config: &StoreConfig,
        handle: GenerationHandle,
    ) -> Result<GenerationHandle, GenerationError> {
        prepare_generation_store(config)?;
        let mut writer = connection::open_writer(config.path()).map_err(GenerationError::from)?;
        generation::publish(&mut writer, handle)
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the private lifecycle is reserved for semantic metadata publication"
        )
    )]
    pub(crate) fn active_generation(
        config: &StoreConfig,
    ) -> Result<Option<GenerationHandle>, GenerationError> {
        prepare_generation_store(config)?;
        let reader = connection::open_read_only(config.path()).map_err(GenerationError::from)?;
        generation::active(&reader)
    }

    #[cfg(test)]
    pub(crate) fn publish_generation_with_failure_for_test(
        config: &StoreConfig,
        handle: GenerationHandle,
        failure_point: PublicationFailurePoint,
    ) -> Result<GenerationHandle, GenerationError> {
        prepare_generation_store(config)?;
        let mut writer = connection::open_writer(config.path()).map_err(GenerationError::from)?;
        generation::publish_with_failure_for_test(&mut writer, handle, failure_point)
    }
}

fn prepare_generation_store(config: &StoreConfig) -> Result<(), GenerationError> {
    if !config.is_enabled() {
        return Err(GenerationError::Store(StoreStatus::Disabled));
    }
    prepare_store_path(config.path()).map_err(GenerationError::Store)
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Explicit store rebuild is retained for a later controlled recovery entry point."
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

//! Private durable semantic-store boundary.
//!
//! This module owns database paths, connection policy, migrations, and raw SQL.
//! Callers receive typed configuration and status values only; rusqlite types do
//! not cross this boundary.

use std::path::{Path, PathBuf};

mod connection;
mod migrations;

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

        if prepare_store_path(config.path()).is_err() {
            return StoreStatus::Skipped(StoreSkipReason::UnsafePath);
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

fn prepare_store_path(path: &Path) -> Result<(), ()> {
    let parent = path.parent().ok_or(())?;
    crate::repo_fs::create_dir_all_no_symlink(parent).map_err(|_| ())?;
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => Err(()),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(()),
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
        connection::ConnectionError::Other => StoreStatus::Skipped(StoreSkipReason::OpenFailed),
    }
}

#[cfg(test)]
mod tests;

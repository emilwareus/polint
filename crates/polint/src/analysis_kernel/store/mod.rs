//! Private durable semantic-store boundary.
//!
//! This module owns database paths, connection policy, migrations, and raw SQL.
//! Callers receive typed configuration and status values only; rusqlite types do
//! not cross this boundary.

use std::path::{Path, PathBuf};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_preserves_path_and_disabled_state() {
        let config = StoreConfig::new("cache/semantic-store/store.sqlite3", false);

        assert_eq!(
            config.path(),
            Path::new("cache/semantic-store/store.sqlite3")
        );
        assert!(!config.is_enabled());
    }

    #[test]
    fn status_vocabulary_is_typed_and_comparable() {
        let statuses = [
            StoreStatus::Disabled,
            StoreStatus::Ready,
            StoreStatus::BusySkipped,
            StoreStatus::Skipped(StoreSkipReason::FutureSchema {
                found: 2,
                supported: 1,
            }),
            StoreStatus::Skipped(StoreSkipReason::UnsafePath),
            StoreStatus::Skipped(StoreSkipReason::OpenFailed),
            StoreStatus::RebuildNeeded(StoreRebuildReason::Corrupt),
            StoreStatus::RebuildNeeded(StoreRebuildReason::InvalidSchema),
        ];

        assert_eq!(statuses.len(), 8);
    }

    #[test]
    fn semantic_store_is_zero_sized_facade() {
        assert_eq!(std::mem::size_of::<SemanticStore>(), 0);
    }
}

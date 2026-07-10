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

mod connection_policy {
    use super::*;

    #[test]
    fn disabled_maintenance_returns_before_creating_or_opening_the_path() {
        let temp = tempfile::tempdir().expect("temp directory");
        let path = temp.path().join("cache/semantic-store/store.sqlite3");
        let config = StoreConfig::new(&path, false);

        assert_eq!(SemanticStore::maintain(&config), StoreStatus::Disabled);
        assert!(!temp.path().join("cache").exists());
        assert!(!path.exists());
    }

    #[test]
    fn writer_enforces_locked_pragmas_and_acquires_immediate_lease() {
        let temp = tempfile::tempdir().expect("temp directory");
        let path = temp.path().join("cache/semantic-store/store.sqlite3");
        let config = StoreConfig::new(&path, true);

        assert_eq!(SemanticStore::maintain(&config), StoreStatus::Ready);
        let mut writer = connection::open_writer(&path).expect("open writer");
        let policy = connection::writer_policy(&writer).expect("read policy");

        assert_eq!(policy.foreign_keys, 1);
        assert_eq!(policy.journal_mode, "wal");
        assert_eq!(policy.busy_timeout_ms, 250);
        assert_eq!(
            connection::try_writer_lease(&mut writer).expect("writer lease"),
            connection::LeaseStatus::Acquired
        );
    }

    #[test]
    fn read_only_connection_is_independent_and_rejects_writes() {
        let temp = tempfile::tempdir().expect("temp directory");
        let path = temp.path().join("semantic-store/store.sqlite3");
        let config = StoreConfig::new(&path, true);
        assert_eq!(SemanticStore::maintain(&config), StoreStatus::Ready);

        let reader = connection::open_read_only(&path).expect("open read-only connection");

        assert_eq!(
            connection::read_schema_version(&reader).expect("schema version"),
            migrations::CURRENT_SCHEMA_VERSION
        );
        assert!(connection::read_only_write_is_rejected(&reader));
    }
}

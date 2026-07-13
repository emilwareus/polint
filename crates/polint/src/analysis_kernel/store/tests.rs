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
            found: migrations::CURRENT_SCHEMA_VERSION + 1,
            supported: migrations::CURRENT_SCHEMA_VERSION,
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
        assert_eq!(policy.synchronous, 1);
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

    #[test]
    fn journal_mode_validation_accepts_wal_case_insensitively() {
        assert_eq!(connection::validate_journal_mode("WaL"), Ok(()));
    }

    #[test]
    fn journal_mode_validation_rejects_a_successful_non_wal_result() {
        assert_eq!(
            connection::validate_journal_mode("delete"),
            Err(connection::ConnectionError::Policy)
        );
    }
}

mod writer_contention {
    use std::fs;
    use std::time::{Duration, Instant};

    use super::*;

    // The exact SQLite policy is asserted as 250 ms in `connection_policy`.
    // This wall-clock check is only an anti-hang guard, so leave headroom for a
    // loaded CI runner to deschedule the test around the busy-handler sleeps.
    const CONTENTION_ANTI_HANG_LIMIT: Duration = Duration::from_secs(2);

    #[test]
    fn losing_writer_skips_within_bound_then_acquires_after_release() {
        let temp = tempfile::tempdir().expect("temp directory");
        let path = temp.path().join("semantic-store/store.sqlite3");
        let config = StoreConfig::new(&path, true);
        assert_eq!(SemanticStore::maintain(&config), StoreStatus::Ready);

        let mut first = connection::open_writer(&path).expect("open first writer");
        let mut second = connection::open_writer(&path).expect("open second writer");
        let first_lease = connection::hold_writer_lease(&mut first).expect("hold first lease");

        let started = Instant::now();
        let losing_status = connection::try_writer_lease(&mut second).expect("bounded result");
        let elapsed = started.elapsed();

        assert_eq!(losing_status, connection::LeaseStatus::Busy);
        assert!(elapsed < CONTENTION_ANTI_HANG_LIMIT, "elapsed: {elapsed:?}");
        assert_eq!(
            connection::bootstrap_marker_count(&second).expect("marker count"),
            1
        );

        first_lease.release().expect("release first lease");
        assert_eq!(
            connection::try_writer_lease(&mut second).expect("second acquisition"),
            connection::LeaseStatus::Acquired
        );
        assert_eq!(
            connection::integrity_check(&second).expect("integrity check"),
            "ok"
        );
        assert_eq!(
            connection::bootstrap_marker_count(&second).expect("marker count"),
            1
        );
    }

    #[test]
    fn absent_store_initialization_is_serialized_by_the_immediate_lease() {
        let temp = tempfile::tempdir().expect("temp directory");
        let path = temp.path().join("semantic-store/store.sqlite3");
        fs::create_dir_all(path.parent().expect("store parent")).expect("create store directory");
        assert!(!path.exists());

        let mut first = connection::open_uninitialized_writer_for_test(&path)
            .expect("prepare first absent-store writer");
        let mut second = connection::open_uninitialized_writer_for_test(&path)
            .expect("prepare second absent-store writer");
        let first_lease = connection::hold_initialization_lease(&mut first)
            .expect("hold first initialization lease");

        let started = Instant::now();
        let losing_status = connection::try_initialize_writer_for_test(&mut second)
            .expect("bounded initialization result");
        let elapsed = started.elapsed();

        assert_eq!(losing_status, connection::LeaseStatus::Busy);
        assert!(elapsed < CONTENTION_ANTI_HANG_LIMIT, "elapsed: {elapsed:?}");

        first_lease
            .initialize_and_release()
            .expect("finish first initialization");
        assert_eq!(
            connection::try_initialize_writer_for_test(&mut second)
                .expect("second initialization after release"),
            connection::LeaseStatus::Acquired
        );
        assert_eq!(
            connection::bootstrap_marker_count(&second).expect("marker count"),
            1
        );
        assert_eq!(
            connection::integrity_check(&second).expect("integrity check"),
            "ok"
        );
    }
}

mod recovery {
    use std::fs;

    use rusqlite::Connection;

    use super::*;

    fn store_path(temp: &tempfile::TempDir) -> std::path::PathBuf {
        temp.path().join("cache/semantic-store/store.sqlite3")
    }

    #[test]
    fn corrupt_and_invalid_stores_are_preserved_as_rebuild_needed() {
        let corrupt_temp = tempfile::tempdir().expect("temp directory");
        let corrupt_path = store_path(&corrupt_temp);
        fs::create_dir_all(corrupt_path.parent().expect("store parent"))
            .expect("create store directory");
        let corrupt_bytes = b"not a sqlite database";
        fs::write(&corrupt_path, corrupt_bytes).expect("write corrupt store");
        let corrupt_config = StoreConfig::new(&corrupt_path, true);

        assert_eq!(
            SemanticStore::maintain(&corrupt_config),
            StoreStatus::RebuildNeeded(StoreRebuildReason::Corrupt)
        );
        assert_eq!(
            fs::read(&corrupt_path).expect("read corrupt store"),
            corrupt_bytes
        );

        let invalid_temp = tempfile::tempdir().expect("temp directory");
        let invalid_path = store_path(&invalid_temp);
        fs::create_dir_all(invalid_path.parent().expect("store parent"))
            .expect("create store directory");
        let invalid = Connection::open(&invalid_path).expect("open invalid store");
        invalid
            .pragma_update(None, "user_version", migrations::CURRENT_SCHEMA_VERSION)
            .expect("set current version without marker");
        drop(invalid);
        let invalid_config = StoreConfig::new(&invalid_path, true);

        assert_eq!(
            SemanticStore::maintain(&invalid_config),
            StoreStatus::RebuildNeeded(StoreRebuildReason::InvalidSchema)
        );
        assert!(invalid_path.exists());
    }

    #[test]
    fn future_store_is_preserved_and_explicit_rebuild_refuses_it() {
        let temp = tempfile::tempdir().expect("temp directory");
        let path = store_path(&temp);
        fs::create_dir_all(path.parent().expect("store parent")).expect("create store directory");
        let future = Connection::open(&path).expect("open future store");
        future
            .execute_batch(
                "CREATE TABLE sentinel (value TEXT NOT NULL);\
                 INSERT INTO sentinel (value) VALUES ('future-data');",
            )
            .expect("create future fixture");
        let future_version = migrations::CURRENT_SCHEMA_VERSION + 1;
        future
            .pragma_update(None, "user_version", future_version)
            .expect("set future schema version");
        drop(future);
        let original_bytes = fs::read(&path).expect("read original future store");
        let config = StoreConfig::new(&path, true);
        let future_status = StoreStatus::Skipped(StoreSkipReason::FutureSchema {
            found: future_version,
            supported: migrations::CURRENT_SCHEMA_VERSION,
        });

        assert_eq!(SemanticStore::maintain(&config), future_status);
        assert_eq!(
            fs::read(&path).expect("read future store after maintenance"),
            original_bytes
        );
        assert_eq!(rebuild_owned_cache_store(&config, &path), future_status);
        assert!(path.exists());

        let preserved =
            Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .expect("reopen future store");
        let version: i32 = preserved
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("future version");
        let value: String = preserved
            .query_row("SELECT value FROM sentinel", [], |row| row.get(0))
            .expect("sentinel value");
        assert_eq!(version, future_version);
        assert_eq!(value, "future-data");
    }

    #[test]
    fn rebuild_refuses_outside_candidate_and_rebuilds_exact_corrupt_owned_file() {
        let temp = tempfile::tempdir().expect("temp directory");
        let path = store_path(&temp);
        fs::create_dir_all(path.parent().expect("store parent")).expect("create store directory");
        fs::write(&path, b"corrupt owned store").expect("write corrupt owned store");
        let config = StoreConfig::new(&path, true);
        let outside = temp.path().join("outside.sqlite3");
        fs::write(&outside, b"outside data").expect("write outside file");

        assert_eq!(
            rebuild_owned_cache_store(&config, &outside),
            StoreStatus::Skipped(StoreSkipReason::UnsafePath)
        );
        assert_eq!(
            fs::read(&outside).expect("read outside file"),
            b"outside data"
        );

        assert_eq!(
            rebuild_owned_cache_store(&config, &path),
            StoreStatus::Ready
        );
        let rebuilt = connection::open_writer(&path).expect("open rebuilt store");
        assert_eq!(
            connection::integrity_check(&rebuilt).expect("integrity check"),
            "ok"
        );
        drop(rebuilt);
        let reader = connection::open_read_only(&path).expect("open rebuilt reader");
        assert_eq!(
            connection::read_schema_version(&reader).expect("schema version"),
            migrations::CURRENT_SCHEMA_VERSION
        );
    }

    #[cfg(unix)]
    #[test]
    fn rebuild_refuses_symlink_target_and_symlinked_store_directory() {
        let target_temp = tempfile::tempdir().expect("target temp directory");
        let target_path = store_path(&target_temp);
        fs::create_dir_all(target_path.parent().expect("store parent"))
            .expect("create store directory");
        let outside = target_temp.path().join("outside.sqlite3");
        fs::write(&outside, b"outside target").expect("write outside target");
        std::os::unix::fs::symlink(&outside, &target_path).expect("symlink store target");
        let target_config = StoreConfig::new(&target_path, true);

        assert_eq!(
            rebuild_owned_cache_store(&target_config, &target_path),
            StoreStatus::Skipped(StoreSkipReason::UnsafePath)
        );
        assert_eq!(
            fs::read(&outside).expect("read outside target"),
            b"outside target"
        );

        let ancestor_temp = tempfile::tempdir().expect("ancestor temp directory");
        let cache_root = ancestor_temp.path().join("cache");
        fs::create_dir_all(&cache_root).expect("create cache root");
        let outside_dir = ancestor_temp.path().join("outside-store");
        fs::create_dir_all(&outside_dir).expect("create outside store");
        let outside_db = outside_dir.join("store.sqlite3");
        fs::write(&outside_db, b"outside ancestor target").expect("write outside database");
        std::os::unix::fs::symlink(&outside_dir, cache_root.join("semantic-store"))
            .expect("symlink store directory");
        let ancestor_path = cache_root.join("semantic-store/store.sqlite3");
        let ancestor_config = StoreConfig::new(&ancestor_path, true);

        assert_eq!(
            rebuild_owned_cache_store(&ancestor_config, &ancestor_path),
            StoreStatus::Skipped(StoreSkipReason::UnsafePath)
        );
        assert_eq!(
            fs::read(&outside_db).expect("read outside database"),
            b"outside ancestor target"
        );
    }
}

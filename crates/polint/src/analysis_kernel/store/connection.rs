//! SQLite connection policy and the single-writer lease.

use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::config::DbConfig;
use rusqlite::{Connection, ErrorCode, OpenFlags, Transaction, TransactionBehavior};

#[cfg(test)]
use super::migrations::preflight_schema;
use super::migrations::{MigrationError, apply_migrations_in_transaction, preflight_schema_shape};

const BUSY_TIMEOUT: Duration = Duration::from_millis(250);
const SYNCHRONOUS_NORMAL: i64 = 1;
const WAL_AUTOCHECKPOINT_DISABLED: i64 = 0;
const STORE_PAGE_SIZE_BYTES: i64 = 16 * 1024;

pub(super) struct WriterConnection {
    connection: Connection,
}

pub(super) struct ReadOnlyConnection {
    connection: Connection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LeaseStatus {
    Acquired,
    Busy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ConnectionError {
    Busy,
    FutureSchema { found: i32, supported: i32 },
    InvalidSchema,
    Corrupt,
    Policy,
    Other,
}

pub(super) fn open_writer(path: &Path) -> Result<WriterConnection, ConnectionError> {
    let mut writer = open_uninitialized_writer(path)?;
    match try_initialize_writer(&mut writer)? {
        LeaseStatus::Acquired => Ok(writer),
        LeaseStatus::Busy => Err(ConnectionError::Busy),
    }
}

fn open_uninitialized_writer(path: &Path) -> Result<WriterConnection, ConnectionError> {
    let path = sqlite_open_path(path)?;
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .map_err(classify_sqlite_error)?;
    // New metadata stores use larger pages to keep their wide append-heavy
    // B-trees shallow. SQLite leaves an existing database's page format
    // unchanged, so reopening a compatible store remains non-destructive.
    connection
        .pragma_update(None, "page_size", STORE_PAGE_SIZE_BYTES)
        .map_err(classify_sqlite_error)?;
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(classify_sqlite_error)?;
    preflight_schema_shape(&connection).map_err(classify_migration_error)?;
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(classify_sqlite_error)?;
    // This database is a recoverable analysis cache, so WAL's NORMAL policy is
    // the right safety/performance boundary: SQLite keeps the database
    // consistent and preserves transactions across application crashes, while
    // avoiding FULL's extra durable WAL flush on every commit. A power or OS
    // failure may roll back the newest cache transaction, which the normal
    // schema validation/rebuild path already treats as disposable cache state.
    connection
        .pragma_update(None, "synchronous", SYNCHRONOUS_NORMAL)
        .map_err(classify_sqlite_error)?;
    let journal_mode: String = connection
        .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
        .map_err(classify_sqlite_error)?;
    validate_journal_mode(&journal_mode)?;
    connection
        .pragma_update(None, "wal_autocheckpoint", WAL_AUTOCHECKPOINT_DISABLED)
        .map_err(classify_sqlite_error)?;
    connection
        .set_db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE, true)
        .map_err(classify_sqlite_error)?;
    Ok(WriterConnection { connection })
}

pub(super) fn validate_journal_mode(journal_mode: &str) -> Result<(), ConnectionError> {
    if journal_mode.eq_ignore_ascii_case("wal") {
        Ok(())
    } else {
        Err(ConnectionError::Policy)
    }
}

pub(super) fn open_read_only(path: &Path) -> Result<ReadOnlyConnection, ConnectionError> {
    let path = sqlite_open_path(path)?;
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .map_err(classify_sqlite_error)?;
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(classify_sqlite_error)?;
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(classify_sqlite_error)?;
    Ok(ReadOnlyConnection { connection })
}

/// Resolve only the already-certified containing directory before asking
/// SQLite to open the database. SQLite's Unix `NOFOLLOW` implementation rejects
/// any symlink component in the supplied path, including platform-managed
/// ancestors such as macOS's `/var -> private/var`. Passing the canonical
/// parent preserves those legitimate ancestors while leaving the final database
/// component unresolved so `NOFOLLOW` still closes the target-swap window.
fn sqlite_open_path(path: &Path) -> Result<PathBuf, ConnectionError> {
    let file_name = path.file_name().ok_or(ConnectionError::Policy)?;
    let parent = path.parent().ok_or(ConnectionError::Policy)?;
    let parent = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };
    let canonical_parent = parent.canonicalize().map_err(|_| ConnectionError::Other)?;
    Ok(canonical_parent.join(file_name))
}

pub(super) fn try_writer_lease(
    writer: &mut WriterConnection,
) -> Result<LeaseStatus, ConnectionError> {
    let transaction = match writer
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
    {
        Ok(transaction) => transaction,
        Err(error) if is_busy(&error) => return Ok(LeaseStatus::Busy),
        Err(error) => return Err(classify_sqlite_error(error)),
    };
    transaction.commit().map_err(classify_sqlite_error)?;
    Ok(LeaseStatus::Acquired)
}

pub(super) fn begin_immediate(
    writer: &mut WriterConnection,
) -> Result<Transaction<'_>, ConnectionError> {
    writer
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(classify_sqlite_error)
}

#[cfg(test)]
pub(super) fn set_writer_foreign_keys_for_test(
    writer: &WriterConnection,
    enabled: bool,
) -> Result<(), ConnectionError> {
    writer
        .connection
        .pragma_update(None, "foreign_keys", enabled)
        .map_err(classify_sqlite_error)?;
    let actual: i64 = writer
        .connection
        .pragma_query_value(None, "foreign_keys", |row| row.get(0))
        .map_err(classify_sqlite_error)?;
    if actual == i64::from(enabled) {
        Ok(())
    } else {
        Err(ConnectionError::Policy)
    }
}

pub(super) fn begin_read(
    reader: &mut ReadOnlyConnection,
) -> Result<Transaction<'_>, ConnectionError> {
    reader
        .connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(classify_sqlite_error)
}

pub(super) fn read_connection(reader: &ReadOnlyConnection) -> &Connection {
    &reader.connection
}

pub(super) fn validate_schema_shape(connection: &Connection) -> Result<(), ConnectionError> {
    preflight_schema_shape(connection).map_err(classify_migration_error)
}

fn try_initialize_writer(writer: &mut WriterConnection) -> Result<LeaseStatus, ConnectionError> {
    let transaction = match writer
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
    {
        Ok(transaction) => transaction,
        Err(error) if is_busy(&error) => return Ok(LeaseStatus::Busy),
        Err(error) => return Err(classify_sqlite_error(error)),
    };
    apply_migrations_in_transaction(&transaction).map_err(classify_migration_error)?;
    transaction.commit().map_err(classify_sqlite_error)?;
    Ok(LeaseStatus::Acquired)
}

fn classify_migration_error(error: MigrationError) -> ConnectionError {
    match error {
        MigrationError::FutureSchema { found, supported } => {
            ConnectionError::FutureSchema { found, supported }
        }
        MigrationError::InvalidSchema { .. } => ConnectionError::InvalidSchema,
        MigrationError::Sqlite(error) => classify_sqlite_error(error),
    }
}

pub(super) fn classify_sqlite_error(error: rusqlite::Error) -> ConnectionError {
    match error.sqlite_error_code() {
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => ConnectionError::Busy,
        Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase) => ConnectionError::Corrupt,
        _ => ConnectionError::Other,
    }
}

fn is_busy(error: &rusqlite::Error) -> bool {
    matches!(
        error.sqlite_error_code(),
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
    )
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ConnectionPolicySnapshot {
    pub(super) foreign_keys: i64,
    pub(super) journal_mode: String,
    pub(super) synchronous: i64,
    pub(super) busy_timeout_ms: i64,
    pub(super) wal_autocheckpoint_pages: i64,
    pub(super) no_checkpoint_on_close: bool,
    pub(super) page_size_bytes: i64,
}

#[cfg(test)]
pub(super) fn writer_policy(
    writer: &WriterConnection,
) -> Result<ConnectionPolicySnapshot, ConnectionError> {
    let foreign_keys = writer
        .connection
        .pragma_query_value(None, "foreign_keys", |row| row.get(0))
        .map_err(classify_sqlite_error)?;
    let journal_mode = writer
        .connection
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .map_err(classify_sqlite_error)?;
    let synchronous = writer
        .connection
        .pragma_query_value(None, "synchronous", |row| row.get(0))
        .map_err(classify_sqlite_error)?;
    let busy_timeout_ms = writer
        .connection
        .pragma_query_value(None, "busy_timeout", |row| row.get(0))
        .map_err(classify_sqlite_error)?;
    let wal_autocheckpoint_pages = writer
        .connection
        .pragma_query_value(None, "wal_autocheckpoint", |row| row.get(0))
        .map_err(classify_sqlite_error)?;
    let no_checkpoint_on_close = writer
        .connection
        .db_config(DbConfig::SQLITE_DBCONFIG_NO_CKPT_ON_CLOSE)
        .map_err(classify_sqlite_error)?;
    let page_size_bytes = writer
        .connection
        .pragma_query_value(None, "page_size", |row| row.get(0))
        .map_err(classify_sqlite_error)?;
    Ok(ConnectionPolicySnapshot {
        foreign_keys,
        journal_mode,
        synchronous,
        busy_timeout_ms,
        wal_autocheckpoint_pages,
        no_checkpoint_on_close,
        page_size_bytes,
    })
}

#[cfg(test)]
pub(super) fn read_schema_version(reader: &ReadOnlyConnection) -> Result<i32, ConnectionError> {
    reader
        .connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(classify_sqlite_error)
}

#[cfg(test)]
pub(super) fn read_only_write_is_rejected(reader: &ReadOnlyConnection) -> bool {
    reader
        .connection
        .execute("CREATE TABLE forbidden_write (id INTEGER)", [])
        .is_err()
}

#[cfg(test)]
pub(super) struct HeldWriterLease<'connection> {
    transaction: Transaction<'connection>,
}

#[cfg(test)]
impl HeldWriterLease<'_> {
    pub(super) fn release(self) -> Result<(), ConnectionError> {
        self.transaction.commit().map_err(classify_sqlite_error)
    }
}

#[cfg(test)]
pub(super) fn hold_writer_lease(
    writer: &mut WriterConnection,
) -> Result<HeldWriterLease<'_>, ConnectionError> {
    let transaction = writer
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(classify_sqlite_error)?;
    Ok(HeldWriterLease { transaction })
}

#[cfg(test)]
pub(super) struct HeldInitializationLease<'connection> {
    transaction: Transaction<'connection>,
}

#[cfg(test)]
impl HeldInitializationLease<'_> {
    pub(super) fn initialize_and_release(self) -> Result<(), ConnectionError> {
        apply_migrations_in_transaction(&self.transaction).map_err(classify_migration_error)?;
        self.transaction.commit().map_err(classify_sqlite_error)
    }
}

#[cfg(test)]
pub(super) fn open_uninitialized_writer_for_test(
    path: &Path,
) -> Result<WriterConnection, ConnectionError> {
    open_uninitialized_writer(path)
}

#[cfg(test)]
pub(super) fn try_initialize_writer_for_test(
    writer: &mut WriterConnection,
) -> Result<LeaseStatus, ConnectionError> {
    try_initialize_writer(writer)
}

#[cfg(test)]
pub(super) fn hold_initialization_lease(
    writer: &mut WriterConnection,
) -> Result<HeldInitializationLease<'_>, ConnectionError> {
    let transaction = writer
        .connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(classify_sqlite_error)?;
    Ok(HeldInitializationLease { transaction })
}

#[cfg(test)]
pub(super) fn bootstrap_marker_count(writer: &WriterConnection) -> Result<i64, ConnectionError> {
    writer
        .connection
        .query_row(
            "SELECT count(*) FROM _polint_schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(classify_sqlite_error)
}

#[cfg(test)]
pub(super) fn integrity_check(writer: &WriterConnection) -> Result<String, ConnectionError> {
    writer
        .connection
        .pragma_query_value(None, "integrity_check", |row| row.get(0))
        .map_err(classify_sqlite_error)
}

#[cfg(test)]
pub(crate) struct HeldWriterConnection {
    connection: Connection,
}

#[cfg(test)]
pub(super) fn hold_writer_connection_for_test(
    path: &Path,
) -> Result<HeldWriterConnection, ConnectionError> {
    let writer = open_writer(path)?;
    writer
        .connection
        .execute_batch("BEGIN IMMEDIATE")
        .map_err(classify_sqlite_error)?;
    Ok(HeldWriterConnection {
        connection: writer.connection,
    })
}

#[cfg(test)]
impl Drop for HeldWriterConnection {
    fn drop(&mut self) {
        let _ = self.connection.execute_batch("ROLLBACK");
    }
}

#[cfg(test)]
pub(super) fn install_future_fixture_for_test(path: &Path) -> Result<(), ConnectionError> {
    let connection = Connection::open(path).map_err(classify_sqlite_error)?;
    connection
        .execute_batch(
            "CREATE TABLE sentinel (value TEXT NOT NULL);\
             INSERT INTO sentinel (value) VALUES ('future-data');",
        )
        .map_err(classify_sqlite_error)?;
    connection
        .pragma_update(
            None,
            "user_version",
            super::migrations::CURRENT_SCHEMA_VERSION + 1,
        )
        .map_err(classify_sqlite_error)
}

#[cfg(test)]
pub(super) fn install_invalid_fixture_for_test(path: &Path) -> Result<(), ConnectionError> {
    let connection = Connection::open(path).map_err(classify_sqlite_error)?;
    connection
        .pragma_update(
            None,
            "user_version",
            super::migrations::CURRENT_SCHEMA_VERSION,
        )
        .map_err(classify_sqlite_error)
}

#[cfg(test)]
pub(super) fn install_incomplete_current_v2_fixture_for_test(
    path: &Path,
) -> Result<(), ConnectionError> {
    let writer = open_writer(path)?;
    writer
        .connection
        .pragma_update(None, "foreign_keys", false)
        .map_err(classify_sqlite_error)?;
    writer
        .connection
        .execute_batch(
            r#"
            DROP TABLE dependency_edges;
            DROP TABLE diagnostic_requested_view_digests;
            DROP TABLE diagnostic_nodes;
            DROP TABLE run_manifest_nodes;
            DROP TABLE generation_stats;

            CREATE TABLE dependency_edges (
                generation_id INTEGER NOT NULL,
                ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
                from_node_kind TEXT NOT NULL CHECK (from_node_kind IN ('dependency_input', 'layer', 'query', 'summary')),
                from_input_kind TEXT,
                from_input_stable_key TEXT,
                from_input_digest_kind TEXT,
                from_input_digest_value TEXT,
                from_input_status TEXT,
                from_layer_id INTEGER,
                from_query_id INTEGER,
                from_summary_id INTEGER,
                to_node_kind TEXT NOT NULL CHECK (to_node_kind IN ('dependency_input', 'layer', 'query', 'summary')),
                to_input_kind TEXT,
                to_input_stable_key TEXT,
                to_input_digest_kind TEXT,
                to_input_digest_value TEXT,
                to_input_status TEXT,
                to_layer_id INTEGER,
                to_query_id INTEGER,
                to_summary_id INTEGER,
                dependency_kind TEXT NOT NULL CHECK (dependency_kind <> ''),
                required_shape TEXT NOT NULL CHECK (required_shape <> ''),
                PRIMARY KEY (generation_id, ordinal),
                CHECK (
                    (from_node_kind = 'dependency_input'
                        AND from_input_kind IS NOT NULL AND from_input_stable_key IS NOT NULL
                        AND from_input_digest_kind IS NOT NULL AND from_input_digest_value IS NOT NULL
                        AND from_input_status IS NOT NULL
                        AND from_layer_id IS NULL AND from_query_id IS NULL AND from_summary_id IS NULL)
                    OR (from_node_kind = 'layer' AND from_layer_id IS NOT NULL
                        AND from_input_kind IS NULL AND from_input_stable_key IS NULL
                        AND from_input_digest_kind IS NULL AND from_input_digest_value IS NULL
                        AND from_input_status IS NULL AND from_query_id IS NULL AND from_summary_id IS NULL)
                    OR (from_node_kind = 'query' AND from_query_id IS NOT NULL
                        AND from_input_kind IS NULL AND from_input_stable_key IS NULL
                        AND from_input_digest_kind IS NULL AND from_input_digest_value IS NULL
                        AND from_input_status IS NULL AND from_layer_id IS NULL AND from_summary_id IS NULL)
                    OR (from_node_kind = 'summary' AND from_summary_id IS NOT NULL
                        AND from_input_kind IS NULL AND from_input_stable_key IS NULL
                        AND from_input_digest_kind IS NULL AND from_input_digest_value IS NULL
                        AND from_input_status IS NULL AND from_layer_id IS NULL AND from_query_id IS NULL)
                ),
                CHECK (
                    (to_node_kind = 'dependency_input'
                        AND to_input_kind IS NOT NULL AND to_input_stable_key IS NOT NULL
                        AND to_input_digest_kind IS NOT NULL AND to_input_digest_value IS NOT NULL
                        AND to_input_status IS NOT NULL
                        AND to_layer_id IS NULL AND to_query_id IS NULL AND to_summary_id IS NULL)
                    OR (to_node_kind = 'layer' AND to_layer_id IS NOT NULL
                        AND to_input_kind IS NULL AND to_input_stable_key IS NULL
                        AND to_input_digest_kind IS NULL AND to_input_digest_value IS NULL
                        AND to_input_status IS NULL AND to_query_id IS NULL AND to_summary_id IS NULL)
                    OR (to_node_kind = 'query' AND to_query_id IS NOT NULL
                        AND to_input_kind IS NULL AND to_input_stable_key IS NULL
                        AND to_input_digest_kind IS NULL AND to_input_digest_value IS NULL
                        AND to_input_status IS NULL AND to_layer_id IS NULL AND to_summary_id IS NULL)
                    OR (to_node_kind = 'summary' AND to_summary_id IS NOT NULL
                        AND to_input_kind IS NULL AND to_input_stable_key IS NULL
                        AND to_input_digest_kind IS NULL AND to_input_digest_value IS NULL
                        AND to_input_status IS NULL AND to_layer_id IS NULL AND to_query_id IS NULL)
                ),
                FOREIGN KEY (generation_id) REFERENCES generations(id) ON DELETE CASCADE,
                FOREIGN KEY (generation_id, from_layer_id) REFERENCES layers(generation_id, id),
                FOREIGN KEY (generation_id, from_query_id) REFERENCES queries(generation_id, id),
                FOREIGN KEY (generation_id, from_summary_id) REFERENCES summaries(generation_id, id),
                FOREIGN KEY (generation_id, to_layer_id) REFERENCES layers(generation_id, id),
                FOREIGN KEY (generation_id, to_query_id) REFERENCES queries(generation_id, id),
                FOREIGN KEY (generation_id, to_summary_id) REFERENCES summaries(generation_id, id)
            );

            CREATE TABLE generation_stats (
                generation_id INTEGER PRIMARY KEY,
                input_file_count INTEGER NOT NULL CHECK (input_file_count >= 0),
                input_component_count INTEGER NOT NULL CHECK (input_component_count >= 0),
                input_detail_count INTEGER NOT NULL CHECK (input_detail_count >= 0),
                analysis_setting_count INTEGER NOT NULL CHECK (analysis_setting_count >= 0),
                capability_count INTEGER NOT NULL CHECK (capability_count >= 0),
                provider_schema_count INTEGER NOT NULL CHECK (provider_schema_count >= 0),
                provider_manifest_count INTEGER NOT NULL CHECK (provider_manifest_count >= 0),
                provider_generation_count INTEGER NOT NULL CHECK (provider_generation_count >= 0),
                layer_count INTEGER NOT NULL CHECK (layer_count >= 0),
                summary_count INTEGER NOT NULL CHECK (summary_count >= 0),
                query_count INTEGER NOT NULL CHECK (query_count >= 0),
                fact_count INTEGER NOT NULL CHECK (fact_count >= 0),
                dependency_edge_count INTEGER NOT NULL CHECK (dependency_edge_count >= 0),
                validation_event_count INTEGER NOT NULL CHECK (validation_event_count >= 0),
                input_digest_kind TEXT NOT NULL CHECK (input_digest_kind = 'input_snapshot'),
                input_digest_value TEXT NOT NULL CHECK (input_digest_value <> ''),
                provider_manifest_digest_kind TEXT NOT NULL CHECK (provider_manifest_digest_kind = 'provider_manifest'),
                provider_manifest_digest_value TEXT NOT NULL CHECK (provider_manifest_digest_value <> ''),
                provider_output_digest_kind TEXT NOT NULL CHECK (provider_output_digest_kind = 'provider_output'),
                provider_output_digest_value TEXT NOT NULL CHECK (provider_output_digest_value <> ''),
                layer_digest_kind TEXT NOT NULL CHECK (layer_digest_kind = 'layer'),
                layer_digest_value TEXT NOT NULL CHECK (layer_digest_value <> ''),
                summary_digest_kind TEXT NOT NULL CHECK (summary_digest_kind = 'summary'),
                summary_digest_value TEXT NOT NULL CHECK (summary_digest_value <> ''),
                query_digest_kind TEXT NOT NULL CHECK (query_digest_kind = 'query'),
                query_digest_value TEXT NOT NULL CHECK (query_digest_value <> ''),
                fact_digest_kind TEXT NOT NULL CHECK (fact_digest_kind = 'fact_metadata'),
                fact_digest_value TEXT NOT NULL CHECK (fact_digest_value <> ''),
                dependency_digest_kind TEXT NOT NULL CHECK (dependency_digest_kind = 'dependency'),
                dependency_digest_value TEXT NOT NULL CHECK (dependency_digest_value <> ''),
                validation_digest_kind TEXT NOT NULL CHECK (validation_digest_kind = 'validation_event'),
                validation_digest_value TEXT NOT NULL CHECK (validation_digest_value <> ''),
                input_logical_bytes INTEGER NOT NULL CHECK (input_logical_bytes >= 0),
                provider_logical_bytes INTEGER NOT NULL CHECK (provider_logical_bytes >= 0),
                layer_logical_bytes INTEGER NOT NULL CHECK (layer_logical_bytes >= 0),
                summary_logical_bytes INTEGER NOT NULL CHECK (summary_logical_bytes >= 0),
                query_logical_bytes INTEGER NOT NULL CHECK (query_logical_bytes >= 0),
                fact_logical_bytes INTEGER NOT NULL CHECK (fact_logical_bytes >= 0),
                dependency_logical_bytes INTEGER NOT NULL CHECK (dependency_logical_bytes >= 0),
                validation_logical_bytes INTEGER NOT NULL CHECK (validation_logical_bytes >= 0),
                semantic_logical_bytes INTEGER NOT NULL CHECK (semantic_logical_bytes >= 0),
                FOREIGN KEY (generation_id) REFERENCES generations(id) ON DELETE CASCADE
            );

            CREATE INDEX dependency_edges_from_endpoint_idx
                ON dependency_edges(
                    generation_id, from_node_kind, from_layer_id, from_query_id, from_summary_id,
                    from_input_kind, from_input_stable_key, from_input_digest_kind,
                    from_input_digest_value, from_input_status
                );
            CREATE INDEX dependency_edges_to_endpoint_idx
                ON dependency_edges(
                    generation_id, to_node_kind, to_layer_id, to_query_id, to_summary_id,
                    to_input_kind, to_input_stable_key, to_input_digest_kind,
                    to_input_digest_value, to_input_status
                );
            "#,
        )
        .map_err(classify_sqlite_error)?;
    writer
        .connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(classify_sqlite_error)
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StoreFixtureSnapshot {
    version: i32,
    bootstrap_markers: Option<i64>,
    sentinel: Option<String>,
    schema_objects: Vec<(String, String, String)>,
}

#[cfg(test)]
pub(super) fn fixture_snapshot_for_test(
    path: &Path,
) -> Result<StoreFixtureSnapshot, ConnectionError> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(classify_sqlite_error)?;
    let version = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(classify_sqlite_error)?;
    let bootstrap_table_count: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_master \
             WHERE type = 'table' AND name = '_polint_schema_migrations'",
            [],
            |row| row.get(0),
        )
        .map_err(classify_sqlite_error)?;
    let bootstrap_markers = if bootstrap_table_count == 1 {
        Some(
            connection
                .query_row(
                    "SELECT count(*) FROM _polint_schema_migrations",
                    [],
                    |row| row.get(0),
                )
                .map_err(classify_sqlite_error)?,
        )
    } else {
        None
    };
    let sentinel_table_count: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'sentinel'",
            [],
            |row| row.get(0),
        )
        .map_err(classify_sqlite_error)?;
    let sentinel = if sentinel_table_count == 1 {
        Some(
            connection
                .query_row("SELECT value FROM sentinel", [], |row| row.get(0))
                .map_err(classify_sqlite_error)?,
        )
    } else {
        None
    };
    let mut statement = connection
        .prepare(
            "SELECT type, name, coalesce(sql, '') FROM sqlite_schema
             WHERE name NOT LIKE 'sqlite_%'
             ORDER BY type, name, sql",
        )
        .map_err(classify_sqlite_error)?;
    let schema_objects = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .map_err(classify_sqlite_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(classify_sqlite_error)?;
    Ok(StoreFixtureSnapshot {
        version,
        bootstrap_markers,
        sentinel,
        schema_objects,
    })
}

#[cfg(test)]
pub(super) fn current_schema_is_valid_for_test(path: &Path) -> bool {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(classify_sqlite_error)
        .and_then(|connection| preflight_schema(&connection).map_err(classify_migration_error))
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn future_fixture_tracks_the_current_schema_without_mutation() {
        let temp = tempfile::tempdir().expect("temp directory");
        let path = temp.path().join("store.sqlite3");
        install_future_fixture_for_test(&path).expect("install future fixture");
        let before = fixture_snapshot_for_test(&path).expect("snapshot future fixture");

        let future = super::super::migrations::CURRENT_SCHEMA_VERSION + 1;
        assert!(matches!(
            open_writer(&path),
            Err(ConnectionError::FutureSchema {
                found,
                supported: super::super::migrations::CURRENT_SCHEMA_VERSION,
            }) if found == future
        ));
        assert_eq!(
            fixture_snapshot_for_test(&path).expect("snapshot preserved fixture"),
            before
        );
    }

    #[test]
    fn current_schema_probe_uses_strict_shape_validation() {
        let temp = tempfile::tempdir().expect("temp directory");
        let path = temp.path().join("store.sqlite3");
        let writer = open_writer(&path).expect("initialize store");
        assert!(current_schema_is_valid_for_test(&path));
        writer
            .connection
            .execute_batch("ALTER TABLE fact_metadata ADD COLUMN payload_blob TEXT;")
            .expect("tamper schema");
        drop(writer);

        assert!(!current_schema_is_valid_for_test(&path));
    }
}

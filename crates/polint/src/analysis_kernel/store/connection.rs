//! SQLite connection policy and the single-writer lease.

use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, ErrorCode, OpenFlags, TransactionBehavior};

#[cfg(test)]
use rusqlite::Transaction;

use super::migrations::{MigrationError, apply_migrations_in_transaction, preflight_schema};

const BUSY_TIMEOUT: Duration = Duration::from_millis(250);

pub(super) struct WriterConnection {
    connection: Connection,
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Read-only queries are established before Phase 65 introduces persisted facts."
    )
)]
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
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )
    .map_err(classify_sqlite_error)?;
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(classify_sqlite_error)?;
    preflight_schema(&connection).map_err(classify_migration_error)?;
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(classify_sqlite_error)?;
    let journal_mode: String = connection
        .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
        .map_err(classify_sqlite_error)?;
    validate_journal_mode(&journal_mode)?;
    Ok(WriterConnection { connection })
}

pub(super) fn validate_journal_mode(journal_mode: &str) -> Result<(), ConnectionError> {
    if journal_mode.eq_ignore_ascii_case("wal") {
        Ok(())
    } else {
        Err(ConnectionError::Policy)
    }
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Read-only queries are established before Phase 65 introduces persisted facts."
    )
)]
pub(super) fn open_read_only(path: &Path) -> Result<ReadOnlyConnection, ConnectionError> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(classify_sqlite_error)?;
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(classify_sqlite_error)?;
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(classify_sqlite_error)?;
    Ok(ReadOnlyConnection { connection })
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

fn classify_sqlite_error(error: rusqlite::Error) -> ConnectionError {
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
    pub(super) busy_timeout_ms: i64,
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
    let busy_timeout_ms = writer
        .connection
        .pragma_query_value(None, "busy_timeout", |row| row.get(0))
        .map_err(classify_sqlite_error)?;
    Ok(ConnectionPolicySnapshot {
        foreign_keys,
        journal_mode,
        busy_timeout_ms,
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
             INSERT INTO sentinel (value) VALUES ('future-data');\
             PRAGMA user_version = 2;",
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StoreFixtureSnapshot {
    version: i32,
    bootstrap_markers: Option<i64>,
    sentinel: Option<String>,
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
    Ok(StoreFixtureSnapshot {
        version,
        bootstrap_markers,
        sentinel,
    })
}

#[cfg(test)]
pub(super) fn current_schema_is_valid_for_test(path: &Path) -> bool {
    fixture_snapshot_for_test(path).is_ok_and(|snapshot| {
        snapshot.version == super::migrations::CURRENT_SCHEMA_VERSION
            && snapshot.bootstrap_markers == Some(1)
    })
}

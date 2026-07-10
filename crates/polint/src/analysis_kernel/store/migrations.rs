//! Strict, private schema migrations for the durable semantic store.

use rusqlite::Connection;
use thiserror::Error;

pub(super) const CURRENT_SCHEMA_VERSION: i32 = 1;

const BOOTSTRAP_TABLE: &str = "_polint_schema_migrations";
const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    sql: "CREATE TABLE _polint_schema_migrations (\
              version INTEGER PRIMARY KEY CHECK (version > 0)\
          );\
          INSERT INTO _polint_schema_migrations (version) VALUES (1);",
}];

struct Migration {
    version: i32,
    sql: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MigrationStatus {
    Current,
    Migrated { from: i32, to: i32 },
}

#[derive(Debug, Error)]
pub(super) enum MigrationError {
    #[error("semantic store schema version {found} is newer than supported version {supported}")]
    FutureSchema { found: i32, supported: i32 },
    #[error("semantic store schema version {version} is missing its bootstrap invariant")]
    InvalidSchema { version: i32 },
    #[error("semantic store migration failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

pub(super) fn apply_migrations(
    connection: &mut Connection,
) -> Result<MigrationStatus, MigrationError> {
    let found = schema_version(connection)?;
    if found > CURRENT_SCHEMA_VERSION {
        return Err(MigrationError::FutureSchema {
            found,
            supported: CURRENT_SCHEMA_VERSION,
        });
    }

    if found == CURRENT_SCHEMA_VERSION {
        validate_current_schema(connection)?;
        return Ok(MigrationStatus::Current);
    }

    let transaction = connection.transaction()?;
    for migration in MIGRATIONS
        .iter()
        .filter(|migration| migration.version > found)
    {
        transaction.execute_batch(migration.sql)?;
        transaction.pragma_update(None, "user_version", migration.version)?;
    }
    validate_current_schema(&transaction)?;
    transaction.commit()?;

    Ok(MigrationStatus::Migrated {
        from: found,
        to: CURRENT_SCHEMA_VERSION,
    })
}

fn schema_version(connection: &Connection) -> Result<i32, rusqlite::Error> {
    connection.pragma_query_value(None, "user_version", |row| row.get(0))
}

fn validate_current_schema(connection: &Connection) -> Result<(), MigrationError> {
    let version = schema_version(connection)?;
    let table_count: i64 = connection.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [BOOTSTRAP_TABLE],
        |row| row.get(0),
    )?;
    if table_count != 1 {
        return Err(MigrationError::InvalidSchema { version });
    }

    let marker_count: i64 = connection.query_row(
        "SELECT count(*) FROM _polint_schema_migrations WHERE version = ?1",
        [CURRENT_SCHEMA_VERSION],
        |row| row.get(0),
    )?;
    if marker_count != 1 || version != CURRENT_SCHEMA_VERSION {
        return Err(MigrationError::InvalidSchema { version });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Transaction;
    use tempfile::TempDir;

    fn database_path(temp: &TempDir) -> std::path::PathBuf {
        temp.path().join("store.sqlite3")
    }

    fn open_temp_database() -> (TempDir, Connection) {
        let temp = TempDir::new().expect("temp directory");
        let connection = Connection::open(database_path(&temp)).expect("open database");
        (temp, connection)
    }

    #[test]
    fn empty_database_migrates_to_version_one() {
        let (_temp, mut connection) = open_temp_database();

        let status = apply_migrations(&mut connection).expect("migration succeeds");

        assert_eq!(status, MigrationStatus::Migrated { from: 0, to: 1 });
        assert_eq!(schema_version(&connection).expect("schema version"), 1);
        let marker: i32 = connection
            .query_row("SELECT version FROM _polint_schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("migration marker");
        assert_eq!(marker, 1);
    }

    #[test]
    fn explicit_version_zero_fixture_migrates_without_losing_existing_data() {
        let (_temp, mut connection) = open_temp_database();
        connection
            .execute_batch(
                "CREATE TABLE sentinel (value TEXT NOT NULL);\
                 INSERT INTO sentinel (value) VALUES ('preserve-me');\
                 PRAGMA user_version = 0;",
            )
            .expect("create prior fixture");

        let status = apply_migrations(&mut connection).expect("migration succeeds");

        assert_eq!(status, MigrationStatus::Migrated { from: 0, to: 1 });
        let value: String = connection
            .query_row("SELECT value FROM sentinel", [], |row| row.get(0))
            .expect("sentinel row");
        assert_eq!(value, "preserve-me");
    }

    #[test]
    fn reopening_current_schema_is_idempotent() {
        let (temp, mut connection) = open_temp_database();
        apply_migrations(&mut connection).expect("initial migration");
        drop(connection);

        let mut reopened = Connection::open(database_path(&temp)).expect("reopen database");
        let status = apply_migrations(&mut reopened).expect("current schema accepted");

        assert_eq!(status, MigrationStatus::Current);
        let marker_count: i64 = reopened
            .query_row(
                "SELECT count(*) FROM _polint_schema_migrations",
                [],
                |row| row.get(0),
            )
            .expect("marker count");
        let table_count: i64 = reopened
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table'",
                [],
                |row| row.get(0),
            )
            .expect("table count");
        assert_eq!(marker_count, 1);
        assert_eq!(table_count, 1);
    }

    #[test]
    fn future_schema_is_refused_without_mutation() {
        let (_temp, mut connection) = open_temp_database();
        connection
            .execute_batch(
                "CREATE TABLE sentinel (value TEXT NOT NULL);\
                 INSERT INTO sentinel (value) VALUES ('future-data');\
                 PRAGMA user_version = 2;",
            )
            .expect("create future fixture");

        let error = apply_migrations(&mut connection).expect_err("future schema refused");

        assert!(matches!(
            error,
            MigrationError::FutureSchema {
                found: 2,
                supported: 1
            }
        ));
        assert_eq!(schema_version(&connection).expect("schema version"), 2);
        let value: String = connection
            .query_row("SELECT value FROM sentinel", [], |row| row.get(0))
            .expect("sentinel row");
        assert_eq!(value, "future-data");
    }

    #[test]
    fn current_version_without_bootstrap_invariant_is_invalid() {
        let (_temp, mut connection) = open_temp_database();
        connection
            .pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)
            .expect("set current version");

        let error = apply_migrations(&mut connection).expect_err("invalid schema refused");

        assert!(matches!(
            error,
            MigrationError::InvalidSchema { version: 1 }
        ));
        let table_count: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table'",
                [],
                |row| row.get(0),
            )
            .expect("table count");
        assert_eq!(table_count, 0);
    }

    #[test]
    fn migration_error_display_is_actionable() {
        assert_eq!(
            MigrationError::FutureSchema {
                found: 3,
                supported: 1,
            }
            .to_string(),
            "semantic store schema version 3 is newer than supported version 1"
        );
    }

    #[test]
    fn transaction_type_remains_private_to_store_module() {
        fn accepts_private_transaction(_transaction: &Transaction<'_>) {}
        let _ = accepts_private_transaction;
    }
}

//! Strict, private schema migrations for the durable semantic store.

#[cfg(test)]
use rusqlite::TransactionBehavior;
use rusqlite::{Connection, ErrorCode, Transaction};
use thiserror::Error;

pub(super) const CURRENT_SCHEMA_VERSION: i32 = 2;

const BOOTSTRAP_TABLE: &str = "_polint_schema_migrations";
const GENERATIONS_TABLE: &str = "generations";
const ACTIVE_GENERATION_TABLE: &str = "active_generation";

const BOOTSTRAP_TABLE_SQL: &str = "CREATE TABLE _polint_schema_migrations (\
    version INTEGER PRIMARY KEY CHECK (version > 0)\
)";
const GENERATIONS_TABLE_SQL: &str = "CREATE TABLE generations (\
    generation_id INTEGER PRIMARY KEY CHECK (generation_id > 0),\
    status TEXT NOT NULL CHECK (status IN ('pending', 'complete')),\
    UNIQUE (generation_id, status)\
)";
const ACTIVE_GENERATION_TABLE_SQL: &str = "CREATE TABLE active_generation (\
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),\
    generation_id INTEGER NOT NULL,\
    required_status TEXT NOT NULL CHECK (required_status = 'complete'),\
    FOREIGN KEY (generation_id, required_status)\
        REFERENCES generations (generation_id, status)\
)";

const MIGRATION_ONE_STATEMENTS: &[&str] = &[
    BOOTSTRAP_TABLE_SQL,
    "INSERT INTO _polint_schema_migrations (version) VALUES (1)",
];
const MIGRATION_TWO_STATEMENTS: &[&str] = &[
    GENERATIONS_TABLE_SQL,
    ACTIVE_GENERATION_TABLE_SQL,
    "UPDATE _polint_schema_migrations SET version = 2 WHERE version = 1",
];
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        statements: MIGRATION_ONE_STATEMENTS,
    },
    Migration {
        version: 2,
        statements: MIGRATION_TWO_STATEMENTS,
    },
];

struct Migration {
    version: i32,
    statements: &'static [&'static str],
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

#[cfg(test)]
pub(super) fn apply_migrations(
    connection: &mut Connection,
) -> Result<MigrationStatus, MigrationError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let status = apply_migrations_in_transaction(&transaction)?;
    transaction.commit()?;
    Ok(status)
}

pub(super) fn apply_migrations_in_transaction(
    transaction: &Transaction<'_>,
) -> Result<MigrationStatus, MigrationError> {
    let found = schema_version(transaction)?;
    if found > CURRENT_SCHEMA_VERSION {
        return Err(MigrationError::FutureSchema {
            found,
            supported: CURRENT_SCHEMA_VERSION,
        });
    }

    validate_supported_schema(transaction, found)?;
    if found == CURRENT_SCHEMA_VERSION {
        return Ok(MigrationStatus::Current);
    }

    for migration in MIGRATIONS
        .iter()
        .filter(|migration| migration.version > found)
    {
        for statement in migration.statements {
            transaction.execute_batch(statement)?;
        }
        transaction.pragma_update(None, "user_version", migration.version)?;
    }
    validate_current_schema(transaction)?;

    Ok(MigrationStatus::Migrated {
        from: found,
        to: CURRENT_SCHEMA_VERSION,
    })
}

pub(super) fn preflight_schema(connection: &Connection) -> Result<(), MigrationError> {
    let found = schema_version(connection)?;
    if found > CURRENT_SCHEMA_VERSION {
        return Err(MigrationError::FutureSchema {
            found,
            supported: CURRENT_SCHEMA_VERSION,
        });
    }
    validate_supported_schema(connection, found)
}

fn schema_version(connection: &Connection) -> Result<i32, rusqlite::Error> {
    connection.pragma_query_value(None, "user_version", |row| row.get(0))
}

fn validate_supported_schema(connection: &Connection, version: i32) -> Result<(), MigrationError> {
    match version {
        0 => validate_empty_owned_schema(connection, version),
        1 => {
            validate_bootstrap_schema(connection, version, 1)?;
            validate_tables_absent(
                connection,
                version,
                &[GENERATIONS_TABLE, ACTIVE_GENERATION_TABLE],
            )
        }
        CURRENT_SCHEMA_VERSION => validate_current_schema(connection),
        _ => Err(MigrationError::InvalidSchema { version }),
    }
}

fn validate_empty_owned_schema(
    connection: &Connection,
    version: i32,
) -> Result<(), MigrationError> {
    validate_tables_absent(
        connection,
        version,
        &[BOOTSTRAP_TABLE, GENERATIONS_TABLE, ACTIVE_GENERATION_TABLE],
    )
}

fn validate_tables_absent(
    connection: &Connection,
    version: i32,
    table_names: &[&str],
) -> Result<(), MigrationError> {
    for table_name in table_names {
        let table_count: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table_name],
                |row| row.get(0),
            )
            .map_err(|error| classify_invariant_error(error, version))?;
        if table_count != 0 {
            return Err(MigrationError::InvalidSchema { version });
        }
    }
    Ok(())
}

fn validate_current_schema(connection: &Connection) -> Result<(), MigrationError> {
    let version = schema_version(connection)?;
    if version != CURRENT_SCHEMA_VERSION {
        return Err(MigrationError::InvalidSchema { version });
    }
    validate_bootstrap_schema(connection, version, CURRENT_SCHEMA_VERSION)?;
    validate_table_sql(
        connection,
        version,
        GENERATIONS_TABLE,
        GENERATIONS_TABLE_SQL,
    )?;
    validate_table_sql(
        connection,
        version,
        ACTIVE_GENERATION_TABLE,
        ACTIVE_GENERATION_TABLE_SQL,
    )?;
    validate_generations_index(connection, version)?;
    validate_no_indexes(connection, version, ACTIVE_GENERATION_TABLE)?;
    validate_active_foreign_key(connection, version)?;
    validate_lifecycle_rows(connection, version)?;
    validate_foreign_keys(connection, version)
}

fn validate_bootstrap_schema(
    connection: &Connection,
    version: i32,
    expected_marker: i32,
) -> Result<(), MigrationError> {
    validate_table_sql(connection, version, BOOTSTRAP_TABLE, BOOTSTRAP_TABLE_SQL)?;
    validate_no_indexes(connection, version, BOOTSTRAP_TABLE)?;

    let marker: Option<i32> = connection
        .query_row(
            "SELECT min(version) FROM _polint_schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(|error| classify_invariant_error(error, version))?;
    let marker_count: i64 = connection
        .query_row(
            "SELECT count(*) FROM _polint_schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(|error| classify_invariant_error(error, version))?;
    if marker != Some(expected_marker) || marker_count != 1 {
        return Err(MigrationError::InvalidSchema { version });
    }
    Ok(())
}

fn validate_table_sql(
    connection: &Connection,
    version: i32,
    table_name: &str,
    expected_sql: &str,
) -> Result<(), MigrationError> {
    let mut statement = connection
        .prepare("SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1")
        .map_err(|error| classify_invariant_error(error, version))?;
    let mut rows = statement
        .query([table_name])
        .map_err(|error| classify_invariant_error(error, version))?;
    let Some(row) = rows
        .next()
        .map_err(|error| classify_invariant_error(error, version))?
    else {
        return Err(MigrationError::InvalidSchema { version });
    };
    let actual_sql: String = row
        .get(0)
        .map_err(|error| classify_invariant_error(error, version))?;
    if rows
        .next()
        .map_err(|error| classify_invariant_error(error, version))?
        .is_some()
        || normalize_schema_sql(&actual_sql) != normalize_schema_sql(expected_sql)
    {
        return Err(MigrationError::InvalidSchema { version });
    }
    Ok(())
}

fn normalize_schema_sql(sql: &str) -> String {
    sql.chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect()
}

fn validate_no_indexes(
    connection: &Connection,
    version: i32,
    table_name: &str,
) -> Result<(), MigrationError> {
    let sql = format!("PRAGMA index_list('{table_name}')");
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| classify_invariant_error(error, version))?;
    let mut rows = statement
        .query([])
        .map_err(|error| classify_invariant_error(error, version))?;
    if rows
        .next()
        .map_err(|error| classify_invariant_error(error, version))?
        .is_some()
    {
        return Err(MigrationError::InvalidSchema { version });
    }
    Ok(())
}

fn validate_generations_index(connection: &Connection, version: i32) -> Result<(), MigrationError> {
    let mut statement = connection
        .prepare("PRAGMA index_list('generations')")
        .map_err(|error| classify_invariant_error(error, version))?;
    let mut rows = statement
        .query([])
        .map_err(|error| classify_invariant_error(error, version))?;
    let Some(row) = rows
        .next()
        .map_err(|error| classify_invariant_error(error, version))?
    else {
        return Err(MigrationError::InvalidSchema { version });
    };
    let index_name: String = row
        .get(1)
        .map_err(|error| classify_invariant_error(error, version))?;
    let unique: i64 = row
        .get(2)
        .map_err(|error| classify_invariant_error(error, version))?;
    let origin: String = row
        .get(3)
        .map_err(|error| classify_invariant_error(error, version))?;
    let partial: i64 = row
        .get(4)
        .map_err(|error| classify_invariant_error(error, version))?;
    if unique != 1
        || origin != "u"
        || partial != 0
        || rows
            .next()
            .map_err(|error| classify_invariant_error(error, version))?
            .is_some()
    {
        return Err(MigrationError::InvalidSchema { version });
    }
    drop(rows);
    drop(statement);

    let pragma = format!("PRAGMA index_info('{index_name}')");
    let mut index_statement = connection
        .prepare(&pragma)
        .map_err(|error| classify_invariant_error(error, version))?;
    let columns = index_statement
        .query_map([], |row| row.get::<_, String>(2))
        .map_err(|error| classify_invariant_error(error, version))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| classify_invariant_error(error, version))?;
    if columns != ["generation_id", "status"] {
        return Err(MigrationError::InvalidSchema { version });
    }
    Ok(())
}

fn validate_active_foreign_key(
    connection: &Connection,
    version: i32,
) -> Result<(), MigrationError> {
    let mut statement = connection
        .prepare("PRAGMA foreign_key_list('active_generation')")
        .map_err(|error| classify_invariant_error(error, version))?;
    let relationships = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
            ))
        })
        .map_err(|error| classify_invariant_error(error, version))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| classify_invariant_error(error, version))?;
    let expected = [
        (
            0,
            0,
            "generations".to_owned(),
            "generation_id".to_owned(),
            "generation_id".to_owned(),
            "NO ACTION".to_owned(),
            "NO ACTION".to_owned(),
            "NONE".to_owned(),
        ),
        (
            0,
            1,
            "generations".to_owned(),
            "required_status".to_owned(),
            "status".to_owned(),
            "NO ACTION".to_owned(),
            "NO ACTION".to_owned(),
            "NONE".to_owned(),
        ),
    ];
    if relationships != expected {
        return Err(MigrationError::InvalidSchema { version });
    }
    Ok(())
}

fn validate_lifecycle_rows(connection: &Connection, version: i32) -> Result<(), MigrationError> {
    let invalid_generation_count: i64 = connection
        .query_row(
            "SELECT count(*) FROM generations \
             WHERE generation_id <= 0 OR status NOT IN ('pending', 'complete')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| classify_invariant_error(error, version))?;
    let invalid_active_count: i64 = connection
        .query_row(
            "SELECT count(*) FROM active_generation AS active \
             LEFT JOIN generations AS generation \
               ON generation.generation_id = active.generation_id \
              AND generation.status = active.required_status \
             WHERE active.singleton != 1 \
                OR active.required_status != 'complete' \
                OR generation.generation_id IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(|error| classify_invariant_error(error, version))?;
    let active_count: i64 = connection
        .query_row("SELECT count(*) FROM active_generation", [], |row| {
            row.get(0)
        })
        .map_err(|error| classify_invariant_error(error, version))?;
    if invalid_generation_count != 0 || invalid_active_count != 0 || active_count > 1 {
        return Err(MigrationError::InvalidSchema { version });
    }
    Ok(())
}

fn validate_foreign_keys(connection: &Connection, version: i32) -> Result<(), MigrationError> {
    let mut statement = connection
        .prepare("PRAGMA foreign_key_check")
        .map_err(|error| classify_invariant_error(error, version))?;
    let mut rows = statement
        .query([])
        .map_err(|error| classify_invariant_error(error, version))?;
    if rows
        .next()
        .map_err(|error| classify_invariant_error(error, version))?
        .is_some()
    {
        return Err(MigrationError::InvalidSchema { version });
    }
    Ok(())
}

fn classify_invariant_error(error: rusqlite::Error, version: i32) -> MigrationError {
    match error.sqlite_error_code() {
        Some(
            ErrorCode::DatabaseBusy
            | ErrorCode::DatabaseLocked
            | ErrorCode::DatabaseCorrupt
            | ErrorCode::NotADatabase
            | ErrorCode::SystemIoFailure
            | ErrorCode::CannotOpen,
        ) => MigrationError::Sqlite(error),
        _ => MigrationError::InvalidSchema { version },
    }
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
        connection
            .pragma_update(None, "foreign_keys", true)
            .expect("enable foreign keys");
        (temp, connection)
    }

    fn install_version_one_fixture(connection: &Connection) {
        connection
            .execute_batch(BOOTSTRAP_TABLE_SQL)
            .expect("create bootstrap table");
        connection
            .execute(
                "INSERT INTO _polint_schema_migrations (version) VALUES (1)",
                [],
            )
            .expect("insert version-one marker");
        connection
            .execute_batch(
                "CREATE TABLE sentinel (value TEXT NOT NULL);\
                 INSERT INTO sentinel (value) VALUES ('preserve-me');\
                 PRAGMA user_version = 1;",
            )
            .expect("create exact version-one fixture");
    }

    fn install_claimed_current_schema(
        connection: &Connection,
        generations_sql: &str,
        active_generation_sql: &str,
    ) {
        connection
            .execute_batch(BOOTSTRAP_TABLE_SQL)
            .expect("create bootstrap table");
        connection
            .execute(
                "INSERT INTO _polint_schema_migrations (version) VALUES (?1)",
                [CURRENT_SCHEMA_VERSION],
            )
            .expect("insert current marker");
        connection
            .execute_batch(generations_sql)
            .expect("create generations table");
        connection
            .execute_batch(active_generation_sql)
            .expect("create active-generation table");
        connection
            .pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)
            .expect("set current version");
    }

    fn owned_schema_snapshot(connection: &Connection) -> (i32, Vec<(String, String)>, Vec<String>) {
        let version = schema_version(connection).expect("schema version");
        let mut schema_statement = connection
            .prepare(
                "SELECT name, sql FROM sqlite_master \
                 WHERE type = 'table' \
                   AND name IN ('_polint_schema_migrations', 'generations', 'active_generation') \
                 ORDER BY name",
            )
            .expect("prepare schema snapshot");
        let schema = schema_statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("query schema snapshot")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect schema snapshot");
        let mut generation_statement = connection
            .prepare("SELECT status FROM generations ORDER BY generation_id")
            .expect("prepare generation snapshot");
        let generations = generation_statement
            .query_map([], |row| row.get(0))
            .expect("query generation snapshot")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect generation snapshot");
        (version, schema, generations)
    }

    #[test]
    fn migration_list_contains_only_bootstrap_and_generation_lifecycle() {
        let versions = MIGRATIONS
            .iter()
            .map(|migration| migration.version)
            .collect::<Vec<_>>();

        assert_eq!(versions, [1, 2]);
    }

    #[test]
    fn empty_database_migrates_to_current_schema() {
        let (_temp, mut connection) = open_temp_database();

        let status = apply_migrations(&mut connection).expect("migration succeeds");

        assert_eq!(
            status,
            MigrationStatus::Migrated {
                from: 0,
                to: CURRENT_SCHEMA_VERSION
            }
        );
        validate_current_schema(&connection).expect("current schema authenticates");
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

        assert_eq!(
            status,
            MigrationStatus::Migrated {
                from: 0,
                to: CURRENT_SCHEMA_VERSION
            }
        );
        let value: String = connection
            .query_row("SELECT value FROM sentinel", [], |row| row.get(0))
            .expect("sentinel row");
        assert_eq!(value, "preserve-me");
    }

    #[test]
    fn exact_version_one_fixture_migrates_without_losing_existing_data() {
        let (_temp, mut connection) = open_temp_database();
        install_version_one_fixture(&connection);

        let status = apply_migrations(&mut connection).expect("migration succeeds");

        assert_eq!(
            status,
            MigrationStatus::Migrated {
                from: 1,
                to: CURRENT_SCHEMA_VERSION
            }
        );
        let value: String = connection
            .query_row("SELECT value FROM sentinel", [], |row| row.get(0))
            .expect("sentinel row");
        assert_eq!(value, "preserve-me");
        validate_current_schema(&connection).expect("migrated schema authenticates");
    }

    #[test]
    fn malformed_version_one_fixture_is_refused_before_lifecycle_tables_are_added() {
        let (_temp, mut connection) = open_temp_database();
        connection
            .execute_batch(
                "CREATE TABLE _polint_schema_migrations (version INTEGER PRIMARY KEY);\
                 INSERT INTO _polint_schema_migrations (version) VALUES (1);\
                 PRAGMA user_version = 1;",
            )
            .expect("create malformed prior fixture");

        let error = apply_migrations(&mut connection).expect_err("malformed prior refused");

        assert!(matches!(
            error,
            MigrationError::InvalidSchema { version: 1 }
        ));
        let lifecycle_table_count: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_master \
                 WHERE type = 'table' AND name IN ('generations', 'active_generation')",
                [],
                |row| row.get(0),
            )
            .expect("lifecycle table count");
        assert_eq!(lifecycle_table_count, 0);
    }

    #[test]
    fn reopening_current_schema_is_idempotent_without_schema_or_data_drift() {
        let (temp, mut connection) = open_temp_database();
        apply_migrations(&mut connection).expect("initial migration");
        connection
            .execute("INSERT INTO generations (status) VALUES ('pending')", [])
            .expect("insert sentinel generation");
        let before = owned_schema_snapshot(&connection);
        drop(connection);

        let mut reopened = Connection::open(database_path(&temp)).expect("reopen database");
        reopened
            .pragma_update(None, "foreign_keys", true)
            .expect("enable foreign keys");
        let status = apply_migrations(&mut reopened).expect("current schema accepted");

        assert_eq!(status, MigrationStatus::Current);
        assert_eq!(owned_schema_snapshot(&reopened), before);
    }

    #[test]
    fn future_schema_is_refused_without_mutation() {
        let (_temp, mut connection) = open_temp_database();
        let future_version = CURRENT_SCHEMA_VERSION + 1;
        connection
            .execute_batch(
                "CREATE TABLE sentinel (value TEXT NOT NULL);\
                 INSERT INTO sentinel (value) VALUES ('future-data');",
            )
            .expect("create future fixture");
        connection
            .pragma_update(None, "user_version", future_version)
            .expect("set future version");

        let error = apply_migrations(&mut connection).expect_err("future schema refused");

        assert!(matches!(
            error,
            MigrationError::FutureSchema { found, supported }
                if found == future_version && supported == CURRENT_SCHEMA_VERSION
        ));
        assert_eq!(
            schema_version(&connection).expect("schema version"),
            future_version
        );
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
            MigrationError::InvalidSchema {
                version: CURRENT_SCHEMA_VERSION
            }
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
    fn current_version_with_wrong_bootstrap_shape_is_invalid() {
        let (_temp, mut connection) = open_temp_database();
        connection
            .execute_batch(
                "CREATE TABLE _polint_schema_migrations (wrong_column INTEGER);\
                 INSERT INTO _polint_schema_migrations (wrong_column) VALUES (2);\
                 PRAGMA user_version = 2;",
            )
            .expect("create wrong-shape fixture");

        let error = apply_migrations(&mut connection).expect_err("wrong shape refused");

        assert!(matches!(
            error,
            MigrationError::InvalidSchema {
                version: CURRENT_SCHEMA_VERSION
            }
        ));
    }

    #[test]
    fn current_version_with_extra_marker_row_is_invalid() {
        let (_temp, mut connection) = open_temp_database();
        connection
            .execute_batch(
                "CREATE TABLE _polint_schema_migrations (\
                     version INTEGER PRIMARY KEY CHECK (version > 0)\
                 );\
                 INSERT INTO _polint_schema_migrations (version) VALUES (1), (2);\
                 PRAGMA user_version = 2;",
            )
            .expect("create extra-marker fixture");

        let error = apply_migrations(&mut connection).expect_err("extra marker refused");

        assert!(matches!(
            error,
            MigrationError::InvalidSchema {
                version: CURRENT_SCHEMA_VERSION
            }
        ));
    }

    #[test]
    fn claimed_current_schema_rejects_wrong_lifecycle_shapes() {
        let malformed_schemas = [
            (
                "CREATE TABLE generations (\
                     generation_id INTEGER PRIMARY KEY CHECK (generation_id > 0),\
                     status TEXT NOT NULL CHECK (status IN ('pending', 'complete', 'failed')),\
                     UNIQUE (generation_id, status)\
                 )",
                ACTIVE_GENERATION_TABLE_SQL,
            ),
            (
                "CREATE TABLE generations (\
                     generation_id INTEGER PRIMARY KEY CHECK (generation_id > 0),\
                     status TEXT NOT NULL CHECK (status IN ('pending', 'complete'))\
                 )",
                ACTIVE_GENERATION_TABLE_SQL,
            ),
            (
                GENERATIONS_TABLE_SQL,
                "CREATE TABLE active_generation (\
                     singleton INTEGER PRIMARY KEY CHECK (singleton IN (1, 2)),\
                     generation_id INTEGER NOT NULL,\
                     required_status TEXT NOT NULL CHECK (required_status = 'complete'),\
                     FOREIGN KEY (generation_id, required_status)\
                         REFERENCES generations (generation_id, status)\
                 )",
            ),
            (
                GENERATIONS_TABLE_SQL,
                "CREATE TABLE active_generation (\
                     singleton INTEGER PRIMARY KEY CHECK (singleton = 1),\
                     generation_id INTEGER NOT NULL,\
                     required_status TEXT NOT NULL CHECK (required_status IN ('pending', 'complete')),\
                     FOREIGN KEY (generation_id, required_status)\
                         REFERENCES generations (generation_id, status)\
                 )",
            ),
            (
                GENERATIONS_TABLE_SQL,
                "CREATE TABLE active_generation (\
                     singleton INTEGER PRIMARY KEY CHECK (singleton = 1),\
                     generation_id INTEGER NOT NULL,\
                     required_status TEXT NOT NULL CHECK (required_status = 'complete')\
                 )",
            ),
        ];

        for (generations_sql, active_generation_sql) in malformed_schemas {
            let (_temp, mut connection) = open_temp_database();
            install_claimed_current_schema(&connection, generations_sql, active_generation_sql);

            let error = apply_migrations(&mut connection).expect_err("malformed lifecycle refused");

            assert!(matches!(
                error,
                MigrationError::InvalidSchema {
                    version: CURRENT_SCHEMA_VERSION
                }
            ));
        }
    }

    #[test]
    fn claimed_current_schema_rejects_extra_lifecycle_index() {
        let (_temp, mut connection) = open_temp_database();
        install_claimed_current_schema(
            &connection,
            GENERATIONS_TABLE_SQL,
            ACTIVE_GENERATION_TABLE_SQL,
        );
        connection
            .execute(
                "CREATE INDEX generations_status_extra ON generations (status)",
                [],
            )
            .expect("create extra index");

        let error = apply_migrations(&mut connection).expect_err("extra index refused");

        assert!(matches!(
            error,
            MigrationError::InvalidSchema {
                version: CURRENT_SCHEMA_VERSION
            }
        ));
    }

    #[test]
    fn claimed_current_schema_rejects_foreign_key_violations() {
        let (_temp, mut connection) = open_temp_database();
        connection
            .pragma_update(None, "foreign_keys", false)
            .expect("disable foreign keys for malformed fixture");
        install_claimed_current_schema(
            &connection,
            GENERATIONS_TABLE_SQL,
            ACTIVE_GENERATION_TABLE_SQL,
        );
        connection
            .execute(
                "INSERT INTO active_generation \
                 (singleton, generation_id, required_status) VALUES (1, 99, 'complete')",
                [],
            )
            .expect("insert dangling active relationship");

        let error = apply_migrations(&mut connection).expect_err("dangling relationship refused");

        assert!(matches!(
            error,
            MigrationError::InvalidSchema {
                version: CURRENT_SCHEMA_VERSION
            }
        ));
    }

    #[test]
    fn relational_constraint_rejects_pending_active_generation() {
        let (_temp, mut connection) = open_temp_database();
        apply_migrations(&mut connection).expect("migration succeeds");
        let pending_id = connection
            .query_row(
                "INSERT INTO generations (status) VALUES ('pending') \
                 RETURNING generation_id",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("insert pending generation");

        let error = connection
            .execute(
                "INSERT INTO active_generation \
                 (singleton, generation_id, required_status) VALUES (1, ?1, 'complete')",
                [pending_id],
            )
            .expect_err("pending relationship rejected");

        assert!(matches!(
            error.sqlite_error_code(),
            Some(ErrorCode::ConstraintViolation)
        ));
    }

    #[test]
    fn migration_error_display_is_actionable() {
        assert_eq!(
            MigrationError::FutureSchema {
                found: 3,
                supported: CURRENT_SCHEMA_VERSION,
            }
            .to_string(),
            "semantic store schema version 3 is newer than supported version 2"
        );
    }

    #[test]
    fn transaction_type_remains_private_to_store_module() {
        fn accepts_private_transaction(_transaction: &Transaction<'_>) {}
        let _ = accepts_private_transaction;
    }
}

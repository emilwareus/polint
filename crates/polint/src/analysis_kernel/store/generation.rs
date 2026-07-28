//! Opaque generation reservation and publication for the semantic store.

use std::num::NonZeroI64;

use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::connection::{self, ReadOnlyConnection, WriterConnection};
use super::{StoreStatus, map_connection_error};
use crate::analysis_kernel::incremental::{
    EncodedRunManifest, EncodedRunManifestSource, RunManifest, RunManifestError,
};

const MAX_MANIFEST_SOURCES: i64 = 1_000_000;
const MAX_MANIFEST_TEXT_BYTES: i64 = 4_096;
#[cfg(not(test))]
const MAX_MANIFEST_AGGREGATE_BYTES: i64 = 512 * 1024 * 1024;
#[cfg(test)]
const MAX_MANIFEST_AGGREGATE_BYTES: i64 = 256;
const MANIFEST_ROW_OVERHEAD_BYTES: i64 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GenerationHandle(NonZeroI64);

impl GenerationHandle {
    pub(super) fn from_scalar(value: i64) -> Result<Self, GenerationError> {
        NonZeroI64::new(value)
            .filter(|value| value.get().is_positive())
            .map(Self)
            .ok_or(GenerationError::InvalidSelection)
    }

    fn scalar(self) -> i64 {
        self.0.get()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GenerationError {
    Store(StoreStatus),
    InvalidTransition,
    InvalidSelection,
    InvalidManifest,
    #[cfg(test)]
    InjectedFailure(PublicationFailurePoint),
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GenerationStatus {
    Pending,
    Complete,
}

#[cfg(test)]
impl GenerationStatus {
    pub(super) fn from_stored(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "complete" => Some(Self::Complete),
            _ => None,
        }
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PublicationFailurePoint {
    BeforeUpdate,
    AfterUpdate,
    BeforeSelection,
    AfterSelection,
    BeforeCommit,
}

#[cfg(test)]
impl PublicationFailurePoint {
    pub(crate) const ALL: [Self; 5] = [
        Self::BeforeUpdate,
        Self::AfterUpdate,
        Self::BeforeSelection,
        Self::AfterSelection,
        Self::BeforeCommit,
    ];
}

impl From<connection::ConnectionError> for GenerationError {
    fn from(error: connection::ConnectionError) -> Self {
        Self::Store(map_connection_error(error))
    }
}

impl From<RunManifestError> for GenerationError {
    fn from(_: RunManifestError) -> Self {
        Self::InvalidManifest
    }
}

pub(super) fn reserve(writer: &mut WriterConnection) -> Result<GenerationHandle, GenerationError> {
    connection::with_immediate_transaction(writer, |transaction| {
        let scalar = transaction
            .query_row(
                "INSERT INTO generations (status) VALUES ('pending') \
                 RETURNING generation_id",
                [],
                |row| row.get(0),
            )
            .map_err(connection::classify_sqlite_error)?;
        let handle = GenerationHandle::from_scalar(scalar)?;
        validate_reservation(transaction, handle)?;
        Ok(handle)
    })
}

pub(super) fn publish(
    writer: &mut WriterConnection,
    handle: GenerationHandle,
) -> Result<GenerationHandle, GenerationError> {
    #[cfg(test)]
    {
        publish_transaction(writer, handle, None)
    }
    #[cfg(not(test))]
    {
        publish_transaction(writer, handle)
    }
}

fn publish_transaction(
    writer: &mut WriterConnection,
    handle: GenerationHandle,
    #[cfg(test)] failure_point: Option<PublicationFailurePoint>,
) -> Result<GenerationHandle, GenerationError> {
    connection::with_immediate_transaction(writer, |transaction| {
        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::BeforeUpdate)?;

        let changed = transaction
            .execute(
                "UPDATE generations SET status = 'complete' \
                 WHERE generation_id = ?1 AND status = 'pending'",
                [handle.scalar()],
            )
            .map_err(connection::classify_sqlite_error)?;
        if changed != 1 {
            return Err(GenerationError::InvalidTransition);
        }
        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::AfterUpdate)?;

        require_complete(transaction, handle)?;
        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::BeforeSelection)?;

        replace_selection(transaction, handle)?;
        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::AfterSelection)?;

        validate_selection(transaction, handle)?;
        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::BeforeCommit)?;
        Ok(handle)
    })
}

pub(super) fn active(
    reader: &ReadOnlyConnection,
) -> Result<Option<GenerationHandle>, GenerationError> {
    connection::with_read_transaction(reader, |connection| {
        let selected = connection
            .query_row(
                "SELECT active.singleton, active.generation_id, active.required_status, \
                        generation.generation_id, generation.status \
                 FROM active_generation AS active \
                 LEFT JOIN generations AS generation \
                   ON generation.generation_id = active.generation_id \
                  AND generation.status = active.required_status",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(connection::classify_sqlite_error)?;

        let Some((singleton, scalar, required_status, joined_scalar, generation_status)) = selected
        else {
            return Ok(None);
        };
        if singleton != 1
            || required_status != "complete"
            || joined_scalar != Some(scalar)
            || generation_status.as_deref() != Some("complete")
        {
            return Err(GenerationError::InvalidSelection);
        }
        GenerationHandle::from_scalar(scalar).map(Some)
    })
}

#[cfg_attr(not(test), expect(dead_code, reason = "Used by manifest publication."))]
pub(super) fn write_manifest(
    transaction: &Transaction<'_>,
    handle: GenerationHandle,
    manifest: &RunManifest,
) -> Result<(), GenerationError> {
    let encoded = manifest.encode()?;
    let inserted = transaction
        .execute(
            "INSERT INTO run_manifests (\
                 generation_id, manifest_schema, workspace_purpose, workspace_value, \
                 config_purpose, config_value, source_count, run_purpose, run_value\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                handle.scalar(),
                encoded.schema,
                encoded.workspace_purpose,
                encoded.workspace_value,
                encoded.config_purpose,
                encoded.config_value,
                encoded.source_count,
                encoded.run_purpose,
                encoded.run_value,
            ],
        )
        .map_err(connection::classify_sqlite_error)?;
    if inserted != 1 {
        return Err(GenerationError::InvalidManifest);
    }
    for source in encoded.sources {
        let inserted = transaction
            .execute(
                "INSERT INTO run_manifest_sources (\
                     generation_id, relative_path, language, source_purpose, \
                     source_value, size_bytes\
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    handle.scalar(),
                    source.relative_path,
                    source.language,
                    source.source_purpose,
                    source.source_value,
                    source.size_bytes,
                ],
            )
            .map_err(connection::classify_sqlite_error)?;
        if inserted != 1 {
            return Err(GenerationError::InvalidManifest);
        }
    }
    Ok(())
}

#[cfg_attr(not(test), expect(dead_code, reason = "Used by manifest publication."))]
pub(super) fn read_manifest(
    connection: &Connection,
    handle: GenerationHandle,
) -> Result<RunManifest, GenerationError> {
    let header_is_bounded = connection
        .query_row(
            "SELECT count(*) = 1 AND coalesce(sum(CASE WHEN \
                 typeof(manifest_schema) = 'text' AND length(CAST(manifest_schema AS BLOB)) BETWEEN 1 AND 64 AND \
                 typeof(workspace_purpose) = 'text' AND length(CAST(workspace_purpose AS BLOB)) BETWEEN 1 AND 64 AND \
                 typeof(workspace_value) = 'text' AND length(CAST(workspace_value AS BLOB)) = 16 AND \
                 typeof(config_purpose) = 'text' AND length(CAST(config_purpose AS BLOB)) BETWEEN 1 AND 64 AND \
                 typeof(config_value) = 'text' AND length(CAST(config_value AS BLOB)) = 16 AND \
                 typeof(source_count) = 'integer' AND source_count BETWEEN 0 AND ?2 AND \
                 typeof(run_purpose) = 'text' AND length(CAST(run_purpose AS BLOB)) BETWEEN 1 AND 64 AND \
                 typeof(run_value) = 'text' AND length(CAST(run_value AS BLOB)) = 16 \
                 THEN 0 ELSE 1 END), 0) = 0 \
             FROM run_manifests WHERE generation_id = ?1",
            params![handle.scalar(), MAX_MANIFEST_SOURCES],
            |row| row.get::<_, bool>(0),
        )
        .map_err(connection::classify_sqlite_error)?;
    if !header_is_bounded {
        return Err(GenerationError::InvalidManifest);
    }

    let header = connection
        .query_row(
            "SELECT manifest_schema, workspace_purpose, workspace_value, \
                    config_purpose, config_value, source_count, run_purpose, run_value \
             FROM run_manifests WHERE generation_id = ?1",
            [handle.scalar()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            },
        )
        .map_err(connection::classify_sqlite_error)?;
    let (actual_count, invalid_rows, aggregate_bytes) = connection
        .query_row(
            "SELECT count(*), coalesce(sum(CASE WHEN \
                 typeof(relative_path) = 'text' AND length(CAST(relative_path AS BLOB)) BETWEEN 1 AND ?2 AND \
                 typeof(language) = 'text' AND length(CAST(language AS BLOB)) BETWEEN 1 AND 16 AND \
                 typeof(source_purpose) = 'text' AND length(CAST(source_purpose AS BLOB)) BETWEEN 1 AND 32 AND \
                 typeof(source_value) = 'text' AND length(CAST(source_value AS BLOB)) = 16 AND \
                 typeof(size_bytes) = 'integer' AND size_bytes >= 0 \
                 THEN 0 ELSE 1 END), 0), \
                 coalesce(sum(length(CAST(relative_path AS BLOB)) + \
                              length(CAST(language AS BLOB)) + \
                              length(CAST(source_purpose AS BLOB)) + \
                              length(CAST(source_value AS BLOB)) + ?3), 0) \
             FROM run_manifest_sources WHERE generation_id = ?1",
            params![
                handle.scalar(),
                MAX_MANIFEST_TEXT_BYTES,
                MANIFEST_ROW_OVERHEAD_BYTES
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .map_err(connection::classify_sqlite_error)?;
    if actual_count != header.5
        || actual_count > MAX_MANIFEST_SOURCES
        || invalid_rows != 0
        || aggregate_bytes > MAX_MANIFEST_AGGREGATE_BYTES
    {
        return Err(GenerationError::InvalidManifest);
    }

    let capacity = usize::try_from(actual_count).map_err(|_| GenerationError::InvalidManifest)?;
    let mut sources = Vec::with_capacity(capacity);
    let mut statement = connection
        .prepare(
            "SELECT relative_path, language, source_purpose, source_value, size_bytes \
             FROM run_manifest_sources WHERE generation_id = ?1 \
             ORDER BY relative_path COLLATE BINARY",
        )
        .map_err(connection::classify_sqlite_error)?;
    let mut rows = statement
        .query([handle.scalar()])
        .map_err(connection::classify_sqlite_error)?;
    while let Some(row) = rows.next().map_err(connection::classify_sqlite_error)? {
        sources.push(EncodedRunManifestSource {
            relative_path: row.get(0).map_err(connection::classify_sqlite_error)?,
            language: row.get(1).map_err(connection::classify_sqlite_error)?,
            source_purpose: row.get(2).map_err(connection::classify_sqlite_error)?,
            source_value: row.get(3).map_err(connection::classify_sqlite_error)?,
            size_bytes: row.get(4).map_err(connection::classify_sqlite_error)?,
        });
    }
    RunManifest::decode(EncodedRunManifest {
        schema: header.0,
        workspace_purpose: header.1,
        workspace_value: header.2,
        config_purpose: header.3,
        config_value: header.4,
        source_count: header.5,
        run_purpose: header.6,
        run_value: header.7,
        sources,
    })
    .map_err(Into::into)
}

#[cfg(test)]
fn select_complete(
    transaction: &Transaction<'_>,
    handle: GenerationHandle,
) -> Result<(), GenerationError> {
    require_complete(transaction, handle)?;
    replace_selection(transaction, handle)?;
    validate_selection(transaction, handle)
}

fn require_complete(
    transaction: &Transaction<'_>,
    handle: GenerationHandle,
) -> Result<(), GenerationError> {
    let is_complete = transaction
        .query_row(
            "SELECT status = 'complete' FROM generations WHERE generation_id = ?1",
            [handle.scalar()],
            |row| row.get::<_, bool>(0),
        )
        .optional()
        .map_err(connection::classify_sqlite_error)?;
    if is_complete != Some(true) {
        return Err(GenerationError::InvalidTransition);
    }
    Ok(())
}

fn validate_reservation(
    transaction: &Transaction<'_>,
    handle: GenerationHandle,
) -> Result<(), GenerationError> {
    let reservation = transaction
        .query_row(
            "SELECT generation.status, \
                    (SELECT count(*) FROM active_generation AS active \
                     WHERE active.generation_id = generation.generation_id) \
             FROM generations AS generation \
             WHERE generation.generation_id = ?1",
            [handle.scalar()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(connection::classify_sqlite_error)?;
    if reservation != Some(("pending".to_owned(), 0)) {
        return Err(GenerationError::InvalidTransition);
    }
    Ok(())
}

fn replace_selection(
    transaction: &Transaction<'_>,
    handle: GenerationHandle,
) -> Result<(), GenerationError> {
    transaction
        .execute(
            "INSERT INTO active_generation (singleton, generation_id, required_status) \
             VALUES (1, ?1, 'complete') \
             ON CONFLICT(singleton) DO UPDATE SET \
                 generation_id = excluded.generation_id, \
                 required_status = excluded.required_status",
            [handle.scalar()],
        )
        .map_err(connection::classify_sqlite_error)?;
    Ok(())
}

fn validate_selection(
    transaction: &Transaction<'_>,
    handle: GenerationHandle,
) -> Result<(), GenerationError> {
    let authentic: i64 = transaction
        .query_row(
            "SELECT count(*) \
             FROM active_generation AS active \
             JOIN generations AS generation \
               ON generation.generation_id = active.generation_id \
              AND generation.status = active.required_status \
             WHERE active.singleton = 1 \
               AND active.generation_id = ?1 \
               AND active.required_status = 'complete' \
               AND generation.status = 'complete'",
            [handle.scalar()],
            |row| row.get(0),
        )
        .map_err(connection::classify_sqlite_error)?;
    if authentic != 1 {
        return Err(GenerationError::InvalidSelection);
    }
    Ok(())
}

#[cfg(test)]
fn fail_at(
    requested: Option<PublicationFailurePoint>,
    current: PublicationFailurePoint,
) -> Result<(), GenerationError> {
    if requested == Some(current) {
        Err(GenerationError::InjectedFailure(current))
    } else {
        Ok(())
    }
}

#[cfg(test)]
pub(super) fn publish_with_failure_for_test(
    writer: &mut WriterConnection,
    handle: GenerationHandle,
    failure_point: PublicationFailurePoint,
) -> Result<GenerationHandle, GenerationError> {
    publish_transaction(writer, handle, Some(failure_point))
}

#[cfg(test)]
pub(super) fn select_for_test(
    writer: &mut WriterConnection,
    handle: GenerationHandle,
) -> Result<(), GenerationError> {
    connection::with_immediate_transaction(writer, |transaction| {
        select_complete(transaction, handle)
    })
}

#[cfg(test)]
pub(super) fn select_without_validation_for_test(
    writer: &mut WriterConnection,
    handle: GenerationHandle,
) -> Result<(), GenerationError> {
    connection::with_immediate_transaction(writer, |transaction| {
        transaction
            .execute(
                "INSERT INTO active_generation (singleton, generation_id, required_status) \
                 VALUES (1, ?1, 'complete') \
                 ON CONFLICT(singleton) DO UPDATE SET \
                     generation_id = excluded.generation_id, \
                     required_status = excluded.required_status",
                [handle.scalar()],
            )
            .map_err(connection::classify_sqlite_error)?;
        Ok(())
    })
}

#[cfg(test)]
pub(super) fn publish_manifest_storage_for_test(
    writer: &mut WriterConnection,
    handle: GenerationHandle,
    manifest: &RunManifest,
) -> Result<RunManifest, GenerationError> {
    connection::with_immediate_transaction(writer, |transaction| {
        write_manifest(transaction, handle, manifest)?;
        let decoded = read_manifest(transaction, handle)?;
        if !decoded.exact_match(manifest) {
            return Err(GenerationError::InvalidManifest);
        }
        let changed = transaction
            .execute(
                "UPDATE generations SET status = 'complete' \
                 WHERE generation_id = ?1 AND status = 'pending'",
                [handle.scalar()],
            )
            .map_err(connection::classify_sqlite_error)?;
        if changed != 1 {
            return Err(GenerationError::InvalidTransition);
        }
        require_complete(transaction, handle)?;
        replace_selection(transaction, handle)?;
        validate_selection(transaction, handle)?;
        Ok(decoded)
    })
}

#[cfg(test)]
pub(super) fn read_manifest_for_test(
    reader: &ReadOnlyConnection,
    handle: GenerationHandle,
) -> Result<RunManifest, GenerationError> {
    connection::with_read_transaction(reader, |transaction| read_manifest(transaction, handle))
}

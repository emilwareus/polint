//! Opaque generation reservation and publication for the semantic store.

use std::num::NonZeroI64;

use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::connection::{self, ReadOnlyConnection, WriterConnection};
use super::go_syntax_mirror;
use super::provider_mirror;
use super::{StoreStatus, map_connection_error};
use crate::analysis_kernel::go_syntax_projection::GoSyntaxProviderProjection;
use crate::analysis_kernel::incremental::{
    EncodedRunManifest, EncodedRunManifestSource, RunManifest, RunManifestError,
};
use crate::analysis_kernel::metrics_projection::MetricsProviderProjection;

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

    pub(super) fn scalar(self) -> i64 {
        self.0.get()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GenerationError {
    Store(StoreStatus),
    InvalidTransition,
    InvalidSelection,
    InvalidManifest,
    InvalidProviderMirror,
    WorkspaceOwnershipMismatch,
    #[cfg(test)]
    InjectedFailure(PublicationFailurePoint),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ManifestMatch {
    NoActiveManifest,
    Exact(GenerationHandle),
    Mismatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MetricsMatch {
    NoActive,
    Exact(GenerationHandle),
    SemanticMiss,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GoSyntaxMatch {
    NoActive,
    Exact(GenerationHandle),
    SemanticMiss,
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
    BeforeHeaderWrite,
    AfterHeaderWrite,
    BeforeSourceWrite,
    AfterSourceWrite,
    BeforeProviderHeader,
    AfterProviderHeader,
    BeforeProviderMembers,
    AfterProviderMembers,
    BeforeProviderBlockers,
    AfterProviderBlockers,
    BeforeProviderSources,
    AfterProviderSources,
    BeforeProviderFunctions,
    AfterProviderFunctions,
    BeforeGoHeader,
    AfterGoHeader,
    BeforeGoMembers,
    AfterGoMembers,
    BeforeGoBlockers,
    AfterGoBlockers,
    BeforeGoSources,
    AfterGoSources,
    BeforeGoParser,
    AfterGoParser,
    BeforeGoReadback,
    AfterGoReadback,
    BeforeProviderReadback,
    AfterProviderReadback,
    BeforeStoredRead,
    AfterStoredRead,
    BeforeCompletion,
    AfterCompletion,
    BeforeSelection,
    AfterSelection,
    BeforeCommit,
}

#[cfg(test)]
impl PublicationFailurePoint {
    pub(crate) const ALL: [Self; 35] = [
        Self::BeforeHeaderWrite,
        Self::AfterHeaderWrite,
        Self::BeforeSourceWrite,
        Self::AfterSourceWrite,
        Self::BeforeProviderHeader,
        Self::AfterProviderHeader,
        Self::BeforeProviderMembers,
        Self::AfterProviderMembers,
        Self::BeforeProviderBlockers,
        Self::AfterProviderBlockers,
        Self::BeforeProviderSources,
        Self::AfterProviderSources,
        Self::BeforeProviderFunctions,
        Self::AfterProviderFunctions,
        Self::BeforeGoHeader,
        Self::AfterGoHeader,
        Self::BeforeGoMembers,
        Self::AfterGoMembers,
        Self::BeforeGoBlockers,
        Self::AfterGoBlockers,
        Self::BeforeGoSources,
        Self::AfterGoSources,
        Self::BeforeGoParser,
        Self::AfterGoParser,
        Self::BeforeGoReadback,
        Self::AfterGoReadback,
        Self::BeforeProviderReadback,
        Self::AfterProviderReadback,
        Self::BeforeStoredRead,
        Self::AfterStoredRead,
        Self::BeforeCompletion,
        Self::AfterCompletion,
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

impl From<rusqlite::Error> for GenerationError {
    fn from(error: rusqlite::Error) -> Self {
        connection::classify_sqlite_error(error).into()
    }
}

impl From<RunManifestError> for GenerationError {
    fn from(_: RunManifestError) -> Self {
        Self::InvalidManifest
    }
}

fn map_manifest_write_error(error: rusqlite::Error) -> GenerationError {
    if error.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation) {
        GenerationError::InvalidManifest
    } else {
        connection::classify_sqlite_error(error).into()
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
    manifest: &RunManifest,
    metrics: &MetricsProviderProjection,
    go_syntax: &GoSyntaxProviderProjection,
) -> Result<GenerationHandle, GenerationError> {
    #[cfg(test)]
    {
        publish_transaction(writer, handle, manifest, metrics, go_syntax, None)
    }
    #[cfg(not(test))]
    {
        publish_transaction(writer, handle, manifest, metrics, go_syntax)
    }
}

fn publish_transaction(
    writer: &mut WriterConnection,
    handle: GenerationHandle,
    manifest: &RunManifest,
    metrics: &MetricsProviderProjection,
    go_syntax: &GoSyntaxProviderProjection,
    #[cfg(test)] failure_point: Option<PublicationFailurePoint>,
) -> Result<GenerationHandle, GenerationError> {
    connection::with_immediate_transaction(writer, |transaction| {
        if active_manifest_in_connection(transaction)?.is_some_and(|(_, active, _, _)| {
            active.workspace_identity() != manifest.workspace_identity()
        }) {
            return Err(GenerationError::WorkspaceOwnershipMismatch);
        }
        validate_pending_candidate(transaction, handle)?;
        let encoded = manifest.encode()?;

        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::BeforeHeaderWrite)?;
        insert_manifest_header(transaction, handle, &encoded)?;
        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::AfterHeaderWrite)?;

        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::BeforeSourceWrite)?;
        insert_manifest_sources(transaction, handle, &encoded.sources)?;
        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::AfterSourceWrite)?;

        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::BeforeProviderHeader)?;
        provider_mirror::write_header(transaction, handle, metrics)?;
        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::AfterProviderHeader)?;
        #[cfg(test)]
        fail_at(
            failure_point,
            PublicationFailurePoint::BeforeProviderMembers,
        )?;
        provider_mirror::write_members(transaction, handle, metrics)?;
        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::AfterProviderMembers)?;
        #[cfg(test)]
        fail_at(
            failure_point,
            PublicationFailurePoint::BeforeProviderBlockers,
        )?;
        provider_mirror::write_blockers(transaction, handle, metrics)?;
        #[cfg(test)]
        fail_at(
            failure_point,
            PublicationFailurePoint::AfterProviderBlockers,
        )?;
        #[cfg(test)]
        fail_at(
            failure_point,
            PublicationFailurePoint::BeforeProviderSources,
        )?;
        provider_mirror::write_sources(transaction, handle, metrics)?;
        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::AfterProviderSources)?;
        #[cfg(test)]
        fail_at(
            failure_point,
            PublicationFailurePoint::BeforeProviderFunctions,
        )?;
        provider_mirror::write_functions(transaction, handle, metrics)?;
        #[cfg(test)]
        fail_at(
            failure_point,
            PublicationFailurePoint::AfterProviderFunctions,
        )?;
        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::BeforeGoHeader)?;
        go_syntax_mirror::write_header(transaction, handle, go_syntax)?;
        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::AfterGoHeader)?;
        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::BeforeGoMembers)?;
        go_syntax_mirror::write_members(transaction, handle, go_syntax)?;
        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::AfterGoMembers)?;
        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::BeforeGoBlockers)?;
        go_syntax_mirror::write_blockers(transaction, handle, go_syntax)?;
        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::AfterGoBlockers)?;
        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::BeforeGoSources)?;
        go_syntax_mirror::write_sources(transaction, handle, go_syntax)?;
        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::AfterGoSources)?;
        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::BeforeGoParser)?;
        go_syntax_mirror::write_parser(transaction, handle, go_syntax)?;
        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::AfterGoParser)?;

        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::BeforeStoredRead)?;
        let decoded = read_manifest(transaction, handle)?;
        if !decoded.exact_match(manifest) {
            return Err(GenerationError::InvalidManifest);
        }
        #[cfg(test)]
        fail_at(
            failure_point,
            PublicationFailurePoint::BeforeProviderReadback,
        )?;
        if provider_mirror::read(transaction, handle)? != *metrics {
            return Err(GenerationError::InvalidProviderMirror);
        }
        #[cfg(test)]
        fail_at(
            failure_point,
            PublicationFailurePoint::AfterProviderReadback,
        )?;
        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::BeforeGoReadback)?;
        if go_syntax_mirror::read(transaction, handle)? != *go_syntax {
            return Err(GenerationError::InvalidProviderMirror);
        }
        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::AfterGoReadback)?;
        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::AfterStoredRead)?;

        #[cfg(test)]
        fail_at(failure_point, PublicationFailurePoint::BeforeCompletion)?;
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
        fail_at(failure_point, PublicationFailurePoint::AfterCompletion)?;

        validate_complete_manifest(transaction, handle)?;
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
    active_manifest(reader).map(|active| active.map(|(handle, _, _, _)| handle))
}

pub(super) fn match_active(
    reader: &ReadOnlyConnection,
    requested: &RunManifest,
) -> Result<ManifestMatch, GenerationError> {
    connection::with_read_transaction(reader, |connection| {
        match active_manifest_in_connection(connection)? {
            None => Ok(ManifestMatch::NoActiveManifest),
            Some((handle, active, _, _)) if active.exact_match(requested) => {
                Ok(ManifestMatch::Exact(handle))
            }
            Some(_) => Ok(ManifestMatch::Mismatch),
        }
    })
}

type ActiveProjection = (
    GenerationHandle,
    RunManifest,
    MetricsProviderProjection,
    GoSyntaxProviderProjection,
);

fn active_manifest(
    reader: &ReadOnlyConnection,
) -> Result<Option<ActiveProjection>, GenerationError> {
    connection::with_read_transaction(reader, |transaction| {
        active_manifest_in_connection(transaction)
    })
}

fn active_manifest_in_connection(
    connection: &Connection,
) -> Result<Option<ActiveProjection>, GenerationError> {
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
    let handle = GenerationHandle::from_scalar(scalar)?;
    let manifest = read_manifest(connection, handle)?;
    let metrics = provider_mirror::read(connection, handle)?;
    let go_syntax = go_syntax_mirror::read(connection, handle)?;
    Ok(Some((handle, manifest, metrics, go_syntax)))
}

pub(super) fn active_metrics(
    reader: &ReadOnlyConnection,
) -> Result<Option<(GenerationHandle, MetricsProviderProjection)>, GenerationError> {
    active_manifest(reader).map(|row| row.map(|(handle, _, metrics, _)| (handle, metrics)))
}

pub(super) fn match_active_metrics(
    reader: &ReadOnlyConnection,
    requested_manifest: &RunManifest,
    requested: &MetricsProviderProjection,
) -> Result<MetricsMatch, GenerationError> {
    connection::with_read_transaction(reader, |connection| {
        let Some((handle, manifest, active, _)) = active_manifest_in_connection(connection)? else {
            return Ok(MetricsMatch::NoActive);
        };
        if manifest.workspace_identity() != requested_manifest.workspace_identity()
            || active.outcome.status != crate::analysis_kernel::ProviderOutcomeStatus::Succeeded
            || active != *requested
        {
            Ok(MetricsMatch::SemanticMiss)
        } else {
            Ok(MetricsMatch::Exact(handle))
        }
    })
}

pub(super) fn active_go_syntax(
    reader: &ReadOnlyConnection,
) -> Result<Option<(GenerationHandle, GoSyntaxProviderProjection)>, GenerationError> {
    active_manifest(reader).map(|row| row.map(|(handle, _, _, go)| (handle, go)))
}

pub(super) fn match_active_go_syntax(
    reader: &ReadOnlyConnection,
    requested_manifest: &RunManifest,
    requested: &GoSyntaxProviderProjection,
) -> Result<GoSyntaxMatch, GenerationError> {
    connection::with_read_transaction(reader, |connection| {
        let Some((handle, manifest, _, active)) = active_manifest_in_connection(connection)? else {
            return Ok(GoSyntaxMatch::NoActive);
        };
        if manifest.workspace_identity() != requested_manifest.workspace_identity()
            || active.outcome.status != crate::analysis_kernel::ProviderOutcomeStatus::Succeeded
            || active != *requested
        {
            Ok(GoSyntaxMatch::SemanticMiss)
        } else {
            Ok(GoSyntaxMatch::Exact(handle))
        }
    })
}

fn insert_manifest_header(
    transaction: &Transaction<'_>,
    handle: GenerationHandle,
    encoded: &EncodedRunManifest,
) -> Result<(), GenerationError> {
    let inserted = transaction
        .execute(
            "INSERT INTO run_manifests (\
                 generation_id, manifest_schema, workspace_purpose, workspace_value, \
                 config_purpose, config_value, source_count, run_purpose, run_value\
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                handle.scalar(),
                &encoded.schema,
                &encoded.workspace_purpose,
                &encoded.workspace_value,
                &encoded.config_purpose,
                &encoded.config_value,
                encoded.source_count,
                &encoded.run_purpose,
                &encoded.run_value,
            ],
        )
        .map_err(map_manifest_write_error)?;
    if inserted != 1 {
        return Err(GenerationError::InvalidManifest);
    }
    Ok(())
}

fn insert_manifest_sources(
    transaction: &Transaction<'_>,
    handle: GenerationHandle,
    sources: &[EncodedRunManifestSource],
) -> Result<(), GenerationError> {
    for source in sources {
        let inserted = transaction
            .execute(
                "INSERT INTO run_manifest_sources (\
                     generation_id, relative_path, language, source_purpose, \
                     source_value, size_bytes\
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    handle.scalar(),
                    &source.relative_path,
                    &source.language,
                    &source.source_purpose,
                    &source.source_value,
                    source.size_bytes,
                ],
            )
            .map_err(map_manifest_write_error)?;
        if inserted != 1 {
            return Err(GenerationError::InvalidManifest);
        }
    }
    Ok(())
}

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

fn validate_pending_candidate(
    transaction: &Transaction<'_>,
    handle: GenerationHandle,
) -> Result<(), GenerationError> {
    let candidate = transaction
        .query_row(
            "SELECT generation.status, count(manifest.generation_id) \
             FROM generations AS generation \
             LEFT JOIN run_manifests AS manifest \
               ON manifest.generation_id = generation.generation_id \
             WHERE generation.generation_id = ?1 \
             GROUP BY generation.generation_id, generation.status",
            [handle.scalar()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(connection::classify_sqlite_error)?;
    if candidate != Some(("pending".to_owned(), 0)) {
        return Err(GenerationError::InvalidTransition);
    }
    Ok(())
}

fn validate_complete_manifest(
    transaction: &Transaction<'_>,
    handle: GenerationHandle,
) -> Result<(), GenerationError> {
    require_complete(transaction, handle)?;
    let authentic: bool = transaction
        .query_row(
            "SELECT count(*) = 1 FROM generations AS generation \
             JOIN run_manifests AS manifest \
               ON manifest.generation_id = generation.generation_id \
             WHERE generation.generation_id = ?1 \
               AND generation.status = 'complete' \
               AND manifest.source_count = (\
                   SELECT count(*) FROM run_manifest_sources AS source \
                   WHERE source.generation_id = manifest.generation_id\
               )",
            [handle.scalar()],
            |row| row.get(0),
        )
        .map_err(connection::classify_sqlite_error)?;
    if !authentic {
        return Err(GenerationError::InvalidManifest);
    }
    provider_mirror::read(transaction, handle)?;
    go_syntax_mirror::read(transaction, handle)?;
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
             JOIN run_manifests AS manifest \
               ON manifest.generation_id = generation.generation_id \
             WHERE active.singleton = 1 \
               AND active.generation_id = ?1 \
               AND active.required_status = 'complete' \
               AND generation.status = 'complete' \
               AND manifest.source_count = (\
                   SELECT count(*) FROM run_manifest_sources AS source \
                   WHERE source.generation_id = manifest.generation_id\
               )",
            [handle.scalar()],
            |row| row.get(0),
        )
        .map_err(connection::classify_sqlite_error)?;
    if authentic != 1 {
        return Err(GenerationError::InvalidSelection);
    }
    provider_mirror::read(transaction, handle)?;
    go_syntax_mirror::read(transaction, handle)?;
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
    manifest: &RunManifest,
    metrics: &MetricsProviderProjection,
    go_syntax: &GoSyntaxProviderProjection,
    failure_point: PublicationFailurePoint,
) -> Result<GenerationHandle, GenerationError> {
    publish_transaction(
        writer,
        handle,
        manifest,
        metrics,
        go_syntax,
        Some(failure_point),
    )
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
pub(super) fn read_manifest_for_test(
    reader: &ReadOnlyConnection,
    handle: GenerationHandle,
) -> Result<RunManifest, GenerationError> {
    connection::with_read_transaction(reader, |transaction| read_manifest(transaction, handle))
}

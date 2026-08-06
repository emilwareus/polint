//! Relational codec for the single durable `polint.metrics` metadata mirror.

use rusqlite::{Connection, Transaction, params};

use super::connection;
use super::generation::{GenerationError, GenerationHandle};
use crate::analysis_kernel::incremental::{Digest, DigestKind, PrecisionTier};
use crate::analysis_kernel::metrics_projection::{
    CanonicalMetricFunction, CanonicalMetricSource, CanonicalMetricsInputs,
    MetricsProviderProjection, language_label,
};
use crate::analysis_kernel::provider::ProviderKind;
use crate::analysis_kernel::{
    PrecisionCeiling, ProviderFailureReason, ProviderFailureStage, ProviderOutcome,
    ProviderOutcomeStatus,
};
use crate::core::Language;

const MIRROR_SCHEMA: &str = "polint-metrics-provider-mirror-1";
const MAX_ROWS: i64 = 1_000_000;

#[rustfmt::skip]
struct RawHeader {
    mirror_schema: String, provider_id: String, provider_version: String,
    provider_kind: String, language_scope: String, cache_policy: String, precision_ceiling: String,
    status: String, stage: Option<String>, reason: Option<String>,
    member_count: i64, blocker_count: i64, source_count: i64, function_count: i64,
    witness_kind: String, witness_value: String,
    identity_provider: Option<String>, identity_version: Option<String>, identity_schema: Option<String>,
    identity_kind: Option<String>, identity_value: Option<String>, identity_precision: Option<String>,
}
#[rustfmt::skip]
pub(super) fn write_header(
    transaction: &Transaction<'_>,
    handle: GenerationHandle,
    supplied: &MetricsProviderProjection,
) -> Result<(), GenerationError> {
    let projection = checked_projection(supplied)?;
    let manifest = projection.manifest;
    let outcome = &projection.outcome;
    let identity = outcome.output_identity.as_ref();
    let (source_count, function_count) = projection.inputs.as_ref().map_or((0, 0), |inputs| {
        (inputs.sources.len() as i64, inputs.functions.len() as i64)
    });
    let inserted = transaction
        .execute(
            "INSERT INTO metrics_provider_mirror (generation_id, mirror_schema, provider_id, provider_version, provider_kind, language_scope, cache_policy, precision_ceiling, outcome_status, failure_stage, failure_reason, member_count, blocker_count, source_count, function_count, witness_kind, witness_value, identity_provider_id, identity_provider_version, identity_schema_version, identity_digest_kind, identity_digest_value, identity_precision) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)",
            params![handle.scalar(), MIRROR_SCHEMA, manifest.id, manifest.provider_version(),
                kind_label(manifest.kind), manifest.language_scope_label(), manifest.cache_policy_label(),
                precision_ceiling_label(manifest.precision_ceiling), outcome.status.label(),
                outcome.failure_stage.map(ProviderFailureStage::label), outcome.failure_reason.map(ProviderFailureReason::label),
                member_rows(&manifest).len() as i64, outcome.blockers.len() as i64, source_count, function_count,
                DigestKind::ProviderParameters.as_str(), witness(&projection).value,
                identity.map(|row| row.provider_id.as_str()), identity.map(|row| row.provider_version.as_str()),
                identity.map(|row| row.schema_version.as_str()), identity.map(|row| row.output_digest.kind.as_str()),
                identity.map(|row| row.output_digest.value.as_str()), identity.map(|row| precision_label(row.precision))],
        )
        .map_err(map_write_error)?;
    (inserted == 1)
        .then_some(())
        .ok_or(GenerationError::InvalidProviderMirror)
}
pub(super) fn write_members(
    transaction: &Transaction<'_>,
    handle: GenerationHandle,
    projection: &MetricsProviderProjection,
) -> Result<(), GenerationError> {
    for (category, ordinal, name, version) in member_rows(&projection.manifest) {
        transaction
            .execute(
                "INSERT INTO metrics_provider_members \
             (generation_id, category, ordinal, name, version) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![handle.scalar(), category, ordinal, name, version],
            )
            .map_err(map_write_error)?;
    }
    Ok(())
}
pub(super) fn write_blockers(
    transaction: &Transaction<'_>,
    handle: GenerationHandle,
    projection: &MetricsProviderProjection,
) -> Result<(), GenerationError> {
    for (ordinal, blocker) in projection.outcome.blockers.iter().enumerate() {
        transaction
            .execute(
                "INSERT INTO metrics_provider_blockers \
             (generation_id, ordinal, provider_id) VALUES (?1, ?2, ?3)",
                params![handle.scalar(), ordinal as i64, blocker],
            )
            .map_err(map_write_error)?;
    }
    Ok(())
}
pub(super) fn write_sources(
    transaction: &Transaction<'_>,
    handle: GenerationHandle,
    projection: &MetricsProviderProjection,
) -> Result<(), GenerationError> {
    if let Some(inputs) = &projection.inputs {
        for (ordinal, source) in inputs.sources.iter().enumerate() {
            transaction.execute(
                "INSERT INTO metrics_provider_sources (generation_id, ordinal, relative_path, \
                 language, digest_kind, digest_value, byte_count, line_count, non_empty_line_count) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    handle.scalar(), ordinal as i64, source.path, language_label(source.language),
                    source.source_digest.kind.as_str(), source.source_digest.value,
                    source.byte_count, source.line_count, source.non_empty_line_count,
                ],
            ).map_err(map_write_error)?;
        }
    }
    Ok(())
}
pub(super) fn write_functions(
    transaction: &Transaction<'_>,
    handle: GenerationHandle,
    projection: &MetricsProviderProjection,
) -> Result<(), GenerationError> {
    if let Some(inputs) = &projection.inputs {
        for (ordinal, function) in inputs.functions.iter().enumerate() {
            transaction.execute(
                "INSERT INTO metrics_provider_functions (generation_id, ordinal, relative_path, \
                 name, start_byte, end_byte, start_line, end_line, language, complexity) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    handle.scalar(), ordinal as i64, function.path, function.name, function.start_byte,
                    function.end_byte, function.start_line, function.end_line,
                    language_label(function.language), function.cyclomatic_complexity,
                ],
            ).map_err(map_write_error)?;
        }
    }
    Ok(())
}
#[rustfmt::skip]
pub(super) fn read(connection: &Connection, handle: GenerationHandle) -> Result<MetricsProviderProjection, GenerationError> {
    preflight_header(connection, handle)?;
    let raw = read_header(connection, handle)?;
    let status = ProviderOutcomeStatus::decode(&raw.status).ok_or(GenerationError::InvalidProviderMirror)?;
    if status != ProviderOutcomeStatus::Succeeded && (raw.source_count != 0 || raw.function_count != 0) {
        return Err(GenerationError::InvalidProviderMirror);
    }
    let members = read_members(connection, handle, raw.member_count)?;
    let blockers = read_blockers(connection, handle, raw.blocker_count)?;
    let sources = read_sources(connection, handle, raw.source_count)?;
    let functions = read_functions(connection, handle, raw.function_count)?;
    let stage = decode_optional(raw.stage.as_deref(), ProviderFailureStage::decode)?;
    let reason = decode_optional(raw.reason.as_deref(), ProviderFailureReason::decode)?;
    let identity = decode_identity(&raw)?;
    let outcome = ProviderOutcome::from_closed_parts(raw.provider_id.clone(), status, identity, stage, reason, blockers)
        .map_err(|_| GenerationError::InvalidProviderMirror)?;
    let inputs = (status == ProviderOutcomeStatus::Succeeded).then_some(CanonicalMetricsInputs { sources, functions });
    let projection = MetricsProviderProjection::from_durable_parts(outcome, inputs)
        .map_err(|_| GenerationError::InvalidProviderMirror)?;
    let manifest = projection.manifest;
    if raw.mirror_schema != MIRROR_SCHEMA
        || raw.provider_id != manifest.id
        || raw.provider_version != manifest.provider_version()
        || raw.provider_kind != kind_label(manifest.kind)
        || raw.language_scope != manifest.language_scope_label()
        || raw.cache_policy != manifest.cache_policy_label()
        || raw.precision_ceiling != precision_ceiling_label(manifest.precision_ceiling)
        || members != member_rows(&manifest)
        || raw.witness_kind != DigestKind::ProviderParameters.as_str()
        || raw.witness_value != witness(&projection).value
    {
        return Err(GenerationError::InvalidProviderMirror);
    }
    Ok(projection)
}
fn checked_projection(
    supplied: &MetricsProviderProjection,
) -> Result<MetricsProviderProjection, GenerationError> {
    let checked = MetricsProviderProjection::from_durable_parts(
        supplied.outcome.clone(),
        supplied.inputs.clone(),
    )
    .map_err(|_| GenerationError::InvalidProviderMirror)?;
    (checked == *supplied)
        .then_some(checked)
        .ok_or(GenerationError::InvalidProviderMirror)
}
fn preflight_header(
    connection: &Connection,
    handle: GenerationHandle,
) -> Result<(), GenerationError> {
    let valid: bool = connection.query_row(
        "SELECT count(*) = 1 AND coalesce(sum(CASE WHEN typeof(mirror_schema)='text' AND length(CAST(mirror_schema AS BLOB)) BETWEEN 1 AND 64 AND typeof(provider_id)='text' AND length(CAST(provider_id AS BLOB)) BETWEEN 1 AND 128 AND typeof(provider_version)='text' AND length(CAST(provider_version AS BLOB)) BETWEEN 1 AND 64 AND typeof(provider_kind)='text' AND length(CAST(provider_kind AS BLOB)) BETWEEN 1 AND 32 AND typeof(language_scope)='text' AND length(CAST(language_scope AS BLOB)) BETWEEN 1 AND 32 AND typeof(cache_policy)='text' AND length(CAST(cache_policy AS BLOB)) BETWEEN 1 AND 64 AND typeof(precision_ceiling)='text' AND length(CAST(precision_ceiling AS BLOB)) BETWEEN 1 AND 16 AND typeof(outcome_status)='text' AND length(CAST(outcome_status AS BLOB)) BETWEEN 1 AND 32 AND (failure_stage IS NULL OR (typeof(failure_stage)='text' AND length(CAST(failure_stage AS BLOB)) BETWEEN 1 AND 32)) AND (failure_reason IS NULL OR (typeof(failure_reason)='text' AND length(CAST(failure_reason AS BLOB)) BETWEEN 1 AND 32)) AND typeof(member_count)='integer' AND member_count BETWEEN 0 AND ?2 AND typeof(blocker_count)='integer' AND blocker_count BETWEEN 0 AND ?2 AND typeof(source_count)='integer' AND source_count BETWEEN 0 AND ?2 AND typeof(function_count)='integer' AND function_count BETWEEN 0 AND ?2 AND typeof(witness_kind)='text' AND length(CAST(witness_kind AS BLOB)) BETWEEN 1 AND 32 AND typeof(witness_value)='text' AND length(CAST(witness_value AS BLOB))=16 AND (identity_provider_id IS NULL OR (typeof(identity_provider_id)='text' AND length(CAST(identity_provider_id AS BLOB)) BETWEEN 1 AND 128)) AND (identity_provider_version IS NULL OR (typeof(identity_provider_version)='text' AND length(CAST(identity_provider_version AS BLOB)) BETWEEN 1 AND 64)) AND (identity_schema_version IS NULL OR (typeof(identity_schema_version)='text' AND length(CAST(identity_schema_version AS BLOB)) BETWEEN 1 AND 128)) AND (identity_digest_kind IS NULL OR (typeof(identity_digest_kind)='text' AND length(CAST(identity_digest_kind AS BLOB)) BETWEEN 1 AND 32)) AND (identity_digest_value IS NULL OR (typeof(identity_digest_value)='text' AND length(CAST(identity_digest_value AS BLOB))=16)) AND (identity_precision IS NULL OR (typeof(identity_precision)='text' AND length(CAST(identity_precision AS BLOB)) BETWEEN 1 AND 16)) THEN 0 ELSE 1 END),0)=0 FROM metrics_provider_mirror WHERE generation_id=?1",
        params![handle.scalar(), MAX_ROWS], |row| row.get(0),
    ).map_err(connection::classify_sqlite_error)?;
    valid
        .then_some(())
        .ok_or(GenerationError::InvalidProviderMirror)
}
#[rustfmt::skip]
fn read_header(connection: &Connection, handle: GenerationHandle) -> Result<RawHeader, GenerationError> {
    Ok(connection
        .query_row(
            "SELECT mirror_schema, provider_id, provider_version, provider_kind, language_scope, cache_policy, precision_ceiling, outcome_status, failure_stage, failure_reason, member_count, blocker_count, source_count, function_count, witness_kind, witness_value, identity_provider_id, identity_provider_version, identity_schema_version, identity_digest_kind, identity_digest_value, identity_precision FROM metrics_provider_mirror WHERE generation_id=?1",
            [handle.scalar()],
            |row| {
                Ok(RawHeader {
                    mirror_schema: row.get(0)?, provider_id: row.get(1)?, provider_version: row.get(2)?,
                    provider_kind: row.get(3)?, language_scope: row.get(4)?, cache_policy: row.get(5)?, precision_ceiling: row.get(6)?,
                    status: row.get(7)?, stage: row.get(8)?, reason: row.get(9)?,
                    member_count: row.get(10)?, blocker_count: row.get(11)?, source_count: row.get(12)?, function_count: row.get(13)?,
                    witness_kind: row.get(14)?, witness_value: row.get(15)?,
                    identity_provider: row.get(16)?, identity_version: row.get(17)?, identity_schema: row.get(18)?,
                    identity_kind: row.get(19)?, identity_value: row.get(20)?, identity_precision: row.get(21)?, })
            },
        )
        .map_err(connection::classify_sqlite_error)?)
}
fn read_members(
    connection: &Connection,
    handle: GenerationHandle,
    expected: i64,
) -> Result<Vec<(String, i64, String, i64)>, GenerationError> {
    preflight(
        connection,
        "metrics_provider_members",
        handle,
        expected,
        "typeof(category)='text' AND length(category) BETWEEN 1 AND 16 AND typeof(ordinal)='integer' AND ordinal>=0 AND typeof(name)='text' AND length(CAST(name AS BLOB)) BETWEEN 1 AND 4096 AND typeof(version)='integer' AND version>=0",
        "length(CAST(category AS BLOB))+length(CAST(name AS BLOB))+32",
    )?;
    let mut statement = connection.prepare(
        "SELECT category, ordinal, name, version FROM metrics_provider_members \
         WHERE generation_id=?1 ORDER BY CASE category WHEN 'input' THEN 0 WHEN 'output' THEN 1 ELSE 2 END, ordinal"
    ).map_err(connection::classify_sqlite_error)?;
    Ok(statement
        .query_map([handle.scalar()], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .map_err(connection::classify_sqlite_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(connection::classify_sqlite_error)?)
}
fn read_blockers(
    connection: &Connection,
    handle: GenerationHandle,
    expected: i64,
) -> Result<Vec<String>, GenerationError> {
    preflight(
        connection,
        "metrics_provider_blockers",
        handle,
        expected,
        "typeof(ordinal)='integer' AND ordinal>=0 AND typeof(provider_id)='text' AND length(CAST(provider_id AS BLOB)) BETWEEN 1 AND 128",
        "length(CAST(provider_id AS BLOB))+32",
    )?;
    read_one_text(
        connection,
        "SELECT provider_id FROM metrics_provider_blockers WHERE generation_id=?1 ORDER BY ordinal",
        handle,
    )
}
fn read_sources(
    connection: &Connection,
    handle: GenerationHandle,
    expected: i64,
) -> Result<Vec<CanonicalMetricSource>, GenerationError> {
    preflight(
        connection,
        "metrics_provider_sources",
        handle,
        expected,
        "typeof(ordinal)='integer' AND ordinal>=0 AND typeof(relative_path)='text' AND length(CAST(relative_path AS BLOB)) BETWEEN 1 AND 4096 AND typeof(language)='text' AND length(language) BETWEEN 1 AND 16 AND typeof(digest_kind)='text' AND length(digest_kind) BETWEEN 1 AND 32 AND typeof(digest_value)='text' AND length(digest_value)=16 AND typeof(byte_count)='integer' AND byte_count BETWEEN 0 AND 4294967295 AND typeof(line_count)='integer' AND line_count BETWEEN 0 AND 4294967295 AND typeof(non_empty_line_count)='integer' AND non_empty_line_count BETWEEN 0 AND 4294967295",
        "length(CAST(relative_path AS BLOB))+length(language)+length(digest_kind)+length(digest_value)+64",
    )?;
    let mut statement = connection.prepare(
        "SELECT relative_path, language, digest_kind, digest_value, byte_count, line_count, \
         non_empty_line_count FROM metrics_provider_sources WHERE generation_id=?1 ORDER BY ordinal"
    ).map_err(connection::classify_sqlite_error)?;
    let rows = statement
        .query_and_then([handle.scalar()], |row| {
            let kind: String = cell(row, 2)?;
            if kind != DigestKind::SourceText.as_str() {
                return Err(GenerationError::InvalidProviderMirror);
            }
            Ok(CanonicalMetricSource {
                path: cell(row, 0)?,
                language: decode_language(&cell::<String>(row, 1)?)?,
                source_digest: Digest {
                    kind: DigestKind::SourceText,
                    value: cell(row, 3)?,
                },
                byte_count: cell(row, 4)?,
                line_count: cell(row, 5)?,
                non_empty_line_count: cell(row, 6)?,
            })
        })
        .map_err(connection::classify_sqlite_error)?;
    rows.collect()
}

fn read_functions(
    connection: &Connection,
    handle: GenerationHandle,
    expected: i64,
) -> Result<Vec<CanonicalMetricFunction>, GenerationError> {
    preflight(
        connection,
        "metrics_provider_functions",
        handle,
        expected,
        "typeof(ordinal)='integer' AND ordinal>=0 AND typeof(relative_path)='text' AND length(CAST(relative_path AS BLOB)) BETWEEN 1 AND 4096 AND typeof(name)='text' AND length(CAST(name AS BLOB))<=4096 AND typeof(start_byte)='integer' AND start_byte BETWEEN 0 AND 4294967295 AND typeof(end_byte)='integer' AND end_byte BETWEEN 0 AND 4294967295 AND typeof(start_line)='integer' AND start_line BETWEEN 0 AND 4294967295 AND typeof(end_line)='integer' AND end_line BETWEEN 0 AND 4294967295 AND typeof(language)='text' AND length(language) BETWEEN 1 AND 16 AND typeof(complexity)='integer' AND complexity BETWEEN 0 AND 4294967295",
        "length(CAST(relative_path AS BLOB))+length(CAST(name AS BLOB))+length(language)+80",
    )?;
    let mut statement = connection.prepare(
        "SELECT relative_path, name, start_byte, end_byte, start_line, end_line, language, complexity \
         FROM metrics_provider_functions WHERE generation_id=?1 ORDER BY ordinal"
    ).map_err(connection::classify_sqlite_error)?;
    let rows = statement
        .query_and_then([handle.scalar()], |row| {
            Ok(CanonicalMetricFunction {
                path: cell(row, 0)?,
                name: cell(row, 1)?,
                start_byte: cell(row, 2)?,
                end_byte: cell(row, 3)?,
                start_line: cell(row, 4)?,
                end_line: cell(row, 5)?,
                language: decode_language(&cell::<String>(row, 6)?)?,
                cyclomatic_complexity: cell(row, 7)?,
            })
        })
        .map_err(connection::classify_sqlite_error)?;
    rows.collect()
}

fn cell<T: rusqlite::types::FromSql>(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> Result<T, GenerationError> {
    row.get(index)
        .map_err(|error| connection::classify_sqlite_error(error).into())
}

fn preflight(
    connection: &Connection,
    table: &str,
    handle: GenerationHandle,
    expected: i64,
    valid: &str,
    aggregate: &str,
) -> Result<(), GenerationError> {
    if !(0..=MAX_ROWS).contains(&expected) {
        return Err(GenerationError::InvalidProviderMirror);
    }
    let sql = format!(
        "SELECT count(*), coalesce(sum(CASE WHEN {valid} THEN 0 ELSE 1 END),0), \
        coalesce(sum({aggregate}),0) FROM {table} WHERE generation_id=?1"
    );
    let (count, invalid, bytes): (i64, i64, i64) = connection
        .query_row(&sql, [handle.scalar()], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(connection::classify_sqlite_error)?;
    let ordinals_are_dense = if table == "metrics_provider_members" || expected == 0 {
        true
    } else {
        connection
            .query_row(
                &format!(
                    "SELECT min(ordinal)=0 AND max(ordinal)=?2-1 FROM {table} \
                     WHERE generation_id=?1"
                ),
                params![handle.scalar(), expected],
                |row| row.get(0),
            )
            .map_err(connection::classify_sqlite_error)?
    };
    (count == expected
        && invalid == 0
        && bytes <= super::max_aggregate_bytes()
        && ordinals_are_dense)
        .then_some(())
        .ok_or(GenerationError::InvalidProviderMirror)
}

fn read_one_text(
    connection: &Connection,
    sql: &str,
    handle: GenerationHandle,
) -> Result<Vec<String>, GenerationError> {
    let mut statement = connection
        .prepare(sql)
        .map_err(connection::classify_sqlite_error)?;
    Ok(statement
        .query_map([handle.scalar()], |row| row.get(0))
        .map_err(connection::classify_sqlite_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(connection::classify_sqlite_error)?)
}

#[rustfmt::skip]
fn decode_identity(raw: &RawHeader) -> Result<Option<crate::analysis_kernel::ProviderOutputIdentity>, GenerationError> {
    match (
        &raw.identity_provider,
        &raw.identity_version,
        &raw.identity_schema,
        &raw.identity_kind,
        &raw.identity_value,
        &raw.identity_precision,
    ) {
        (Some(provider_id), Some(provider_version), Some(schema_version), Some(kind), Some(value), Some(precision))
            if kind == DigestKind::ProviderOutput.as_str() && precision == "syntax" => {
            Ok(Some(crate::analysis_kernel::ProviderOutputIdentity {
                provider_id: provider_id.clone(), provider_version: provider_version.clone(),
                schema_version: schema_version.clone(), output_digest: Digest {
                    kind: DigestKind::ProviderOutput, value: value.clone() }, precision: PrecisionTier::Syntax,
            }))
        }
        (None, None, None, None, None, None) => Ok(None),
        _ => Err(GenerationError::InvalidProviderMirror),
    }
}

#[rustfmt::skip]
fn decode_optional<T>(value: Option<&str>, decode: fn(&str) -> Option<T>) -> Result<Option<T>, GenerationError> {
    value
        .map(|value| decode(value).ok_or(GenerationError::InvalidProviderMirror))
        .transpose()
}

#[rustfmt::skip]
fn member_rows(manifest: &crate::analysis_kernel::ProviderManifest) -> Vec<(String, i64, String, i64)> {
    let mut rows = Vec::new();
    for (ordinal, name) in manifest.inputs.iter().enumerate() {
        rows.push(("input".into(), ordinal as i64, (*name).into(), 0));
    }
    for (ordinal, name) in manifest.outputs.iter().enumerate() {
        rows.push(("output".into(), ordinal as i64, (*name).into(), 0));
    }
    let mut schemas = manifest.schema_versions.to_vec();
    schemas.sort_by_key(|schema| (schema.name, schema.version));
    for (ordinal, schema) in schemas.iter().enumerate() {
        rows.push(("schema".into(), ordinal as i64, schema.name.into(), i64::from(schema.version)));
    }
    rows
}

#[rustfmt::skip]
fn witness(projection: &MetricsProviderProjection) -> Digest {
    let mut digest = Digest::builder(DigestKind::ProviderParameters, "metrics-provider-mirror-v1");
    let manifest = projection.manifest;
    for value in [manifest.id, manifest.provider_version(), kind_label(manifest.kind),
        manifest.language_scope_label(), precision_ceiling_label(manifest.precision_ceiling)] {
        digest.field("manifest", value);
    }
    digest.field("cache-policy", &manifest.cache_policy_label());
    for (category, ordinal, name, version) in member_rows(&manifest) {
        digest.field("member-category", &category);
        digest.u64_field("member-ordinal", ordinal as u64);
        digest.field("member-name", &name);
        digest.u64_field("member-version", version as u64);
    }
    let outcome = &projection.outcome;
    digest.field("status", outcome.status.label());
    digest.field("stage", outcome.failure_stage.map_or("absent", ProviderFailureStage::label));
    digest.field("reason", outcome.failure_reason.map_or("absent", ProviderFailureReason::label));
    if let Some(identity) = &outcome.output_identity {
        for value in [&identity.provider_id, &identity.provider_version, &identity.schema_version,
            identity.output_digest.kind.as_str(), &identity.output_digest.value, precision_label(identity.precision)] {
            digest.field("identity", value);
        }
    }
    for blocker in &outcome.blockers {
        digest.field("blocker", blocker);
    }
    if let Some(inputs) = &projection.inputs {
        for row in inputs.source_digests().into_iter().chain(inputs.function_digests()) {
            digest.field("dependency-kind", row.kind.as_str());
            digest.field("dependency-value", &row.value);
        }
    }
    digest.finish()
}

fn decode_language(value: &str) -> Result<Language, GenerationError> {
    match value {
        "go" => Ok(Language::Go),
        "typescript" => Ok(Language::TypeScript),
        "tsx" => Ok(Language::Tsx),
        "javascript" => Ok(Language::JavaScript),
        "jsx" => Ok(Language::Jsx),
        _ => Err(GenerationError::InvalidProviderMirror),
    }
}
const fn kind_label(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::MetricsDerived => "metrics_derived",
        ProviderKind::SourceDiscovery => "source_discovery",
        ProviderKind::LanguageSyntax => "language_syntax",
        ProviderKind::WholeRepoDerived => "whole_repo_derived",
    }
}
const fn precision_ceiling_label(value: PrecisionCeiling) -> &'static str {
    match value {
        PrecisionCeiling::Exact => "exact",
        PrecisionCeiling::Syntax => "syntax",
        PrecisionCeiling::SetupAware => "setup_aware",
    }
}
const fn precision_label(value: PrecisionTier) -> &'static str {
    match value {
        PrecisionTier::Exact => "exact",
        PrecisionTier::Syntax => "syntax",
        PrecisionTier::SetupAware => "setup_aware",
    }
}
fn map_write_error(error: rusqlite::Error) -> GenerationError {
    if error.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation) {
        GenerationError::InvalidProviderMirror
    } else {
        connection::classify_sqlite_error(error).into()
    }
}

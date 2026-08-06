//! Relational codec for the single durable Go syntax metadata mirror.

use rusqlite::{Connection, Transaction, params};

use super::connection;
use super::generation::{GenerationError, GenerationHandle};
use crate::analysis_kernel::go_syntax_projection::{
    CanonicalGoSyntaxInputs, CanonicalGoSyntaxSource, GoSyntaxParserContract,
    GoSyntaxProviderProjection,
};
use crate::analysis_kernel::incremental::{Digest, DigestKind, PrecisionTier};
use crate::analysis_kernel::provider::ProviderKind;
use crate::analysis_kernel::{
    PrecisionCeiling, ProviderFailureReason, ProviderFailureStage, ProviderOutcome,
    ProviderOutcomeStatus, ProviderOutputIdentity,
};
use crate::core::Language;

const MIRROR_SCHEMA: &str = "polint-go-syntax-provider-mirror-1";
const MAX_ROWS: i64 = 1_000_000;

#[rustfmt::skip]
struct RawHeader {
    mirror_schema: String, provider_id: String, provider_version: String,
    provider_kind: String, language_scope: String, cache_policy: String, precision_ceiling: String,
    status: String, stage: Option<String>, reason: Option<String>,
    member_count: i64, blocker_count: i64, source_count: i64, parser_count: i64,
    witness_kind: String, witness_value: String,
    identity_provider: Option<String>, identity_version: Option<String>, identity_schema: Option<String>,
    identity_kind: Option<String>, identity_value: Option<String>, identity_precision: Option<String>,
}

#[rustfmt::skip]
pub(super) fn write_header(transaction: &Transaction<'_>, handle: GenerationHandle, supplied: &GoSyntaxProviderProjection) -> Result<(), GenerationError> {
    let projection = checked_projection(supplied)?;
    let manifest = projection.manifest;
    let outcome = &projection.outcome;
    let identity = outcome.output_identity.as_ref();
    let source_count = projection.inputs.as_ref().map_or(0, |inputs| inputs.sources.len() as i64);
    let parser_count = i64::from(projection.parser.is_some());
    let inserted = transaction.execute(
        "INSERT INTO go_syntax_provider_mirror (generation_id,mirror_schema,provider_id,provider_version,provider_kind,language_scope,cache_policy,precision_ceiling,outcome_status,failure_stage,failure_reason,member_count,blocker_count,source_count,parser_count,witness_kind,witness_value,identity_provider_id,identity_provider_version,identity_schema_version,identity_digest_kind,identity_digest_value,identity_precision) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23)",
        params![handle.scalar(),MIRROR_SCHEMA,manifest.id,manifest.provider_version(),kind_label(manifest.kind),manifest.language_scope_label(),manifest.cache_policy_label(),precision_ceiling_label(manifest.precision_ceiling),outcome.status.label(),outcome.failure_stage.map(ProviderFailureStage::label),outcome.failure_reason.map(ProviderFailureReason::label),member_rows(&manifest).len() as i64,outcome.blockers.len() as i64,source_count,parser_count,DigestKind::ProviderParameters.as_str(),witness(&projection).value,identity.map(|row| row.provider_id.as_str()),identity.map(|row| row.provider_version.as_str()),identity.map(|row| row.schema_version.as_str()),identity.map(|row| row.output_digest.kind.as_str()),identity.map(|row| row.output_digest.value.as_str()),identity.map(|row| precision_label(row.precision))],
    ).map_err(map_write_error)?;
    (inserted == 1).then_some(()).ok_or(GenerationError::InvalidProviderMirror)
}

#[rustfmt::skip]
pub(super) fn write_members(transaction: &Transaction<'_>, handle: GenerationHandle, projection: &GoSyntaxProviderProjection) -> Result<(), GenerationError> {
    for (category, ordinal, name, version) in member_rows(&projection.manifest) {
        transaction.execute(
            "INSERT INTO go_syntax_provider_members (generation_id,category,ordinal,name,version) VALUES (?1,?2,?3,?4,?5)",
            params![handle.scalar(), category, ordinal, name, version],
        ).map_err(map_write_error)?;
    }
    Ok(())
}

#[rustfmt::skip]
pub(super) fn write_blockers(transaction: &Transaction<'_>, handle: GenerationHandle, projection: &GoSyntaxProviderProjection) -> Result<(), GenerationError> {
    for (ordinal, blocker) in projection.outcome.blockers.iter().enumerate() {
        transaction.execute(
            "INSERT INTO go_syntax_provider_blockers (generation_id,ordinal,provider_id) VALUES (?1,?2,?3)",
            params![handle.scalar(), ordinal as i64, blocker],
        ).map_err(map_write_error)?;
    }
    Ok(())
}

#[rustfmt::skip]
pub(super) fn write_sources(transaction: &Transaction<'_>, handle: GenerationHandle, projection: &GoSyntaxProviderProjection) -> Result<(), GenerationError> {
    if let Some(inputs) = &projection.inputs {
        for (ordinal, source) in inputs.sources.iter().enumerate() {
            transaction.execute(
                "INSERT INTO go_syntax_provider_sources (generation_id,ordinal,relative_path,language,digest_kind,digest_value) VALUES (?1,?2,?3,'go',?4,?5)",
                params![handle.scalar(),ordinal as i64,source.path,source.source_digest.kind.as_str(),source.source_digest.value],
            ).map_err(map_write_error)?;
        }
    }
    Ok(())
}

#[rustfmt::skip]
pub(super) fn write_parser(transaction: &Transaction<'_>, handle: GenerationHandle, projection: &GoSyntaxProviderProjection) -> Result<(), GenerationError> {
    if let Some(parser) = &projection.parser {
        transaction.execute(
            "INSERT INTO go_syntax_provider_parser (generation_id,provider_id,provider_version,fact_schema,payload_schema,backend,grammar,digest_kind,digest_value) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![handle.scalar(),parser.provider_id,parser.provider_version,parser.fact_schema,parser.payload_schema,parser.backend,parser.grammar,parser.digest().kind.as_str(),parser.digest().value],
        ).map_err(map_write_error)?;
    }
    Ok(())
}

#[rustfmt::skip]
pub(super) fn read(connection: &Connection, handle: GenerationHandle) -> Result<GoSyntaxProviderProjection, GenerationError> {
    preflight_header(connection, handle)?;
    let raw = read_header(connection, handle)?;
    let status = ProviderOutcomeStatus::decode(&raw.status).ok_or(GenerationError::InvalidProviderMirror)?;
    if status != ProviderOutcomeStatus::Succeeded && (raw.source_count != 0 || raw.parser_count != 0) {
        return Err(GenerationError::InvalidProviderMirror);
    }
    if status == ProviderOutcomeStatus::Succeeded {
        preflight_source_relationships(connection, handle, raw.source_count)?;
    }
    let members = read_members(connection, handle, raw.member_count)?;
    let blockers = read_blockers(connection, handle, raw.blocker_count)?;
    let sources = read_sources(connection, handle, raw.source_count)?;
    let parser = read_parser(connection, handle, raw.parser_count)?;
    let stage = decode_optional(raw.stage.as_deref(), ProviderFailureStage::decode)?;
    let reason = decode_optional(raw.reason.as_deref(), ProviderFailureReason::decode)?;
    let identity = decode_identity(&raw)?;
    let outcome = ProviderOutcome::from_closed_parts(raw.provider_id.clone(),status,identity,stage,reason,blockers)
        .map_err(|_| GenerationError::InvalidProviderMirror)?;
    let inputs = (status == ProviderOutcomeStatus::Succeeded).then_some(CanonicalGoSyntaxInputs { sources });
    let projection = GoSyntaxProviderProjection::from_durable_parts(outcome,inputs,parser)
        .map_err(|_| GenerationError::InvalidProviderMirror)?;
    let manifest = projection.manifest;
    if raw.mirror_schema != MIRROR_SCHEMA || raw.provider_id != manifest.id
        || raw.provider_version != manifest.provider_version() || raw.provider_kind != kind_label(manifest.kind)
        || raw.language_scope != manifest.language_scope_label() || raw.cache_policy != manifest.cache_policy_label()
        || raw.precision_ceiling != precision_ceiling_label(manifest.precision_ceiling)
        || members != member_rows(&manifest) || raw.witness_kind != DigestKind::ProviderParameters.as_str()
        || raw.witness_value != witness(&projection).value {
        return Err(GenerationError::InvalidProviderMirror);
    }
    Ok(projection)
}

#[rustfmt::skip]
fn checked_projection(supplied: &GoSyntaxProviderProjection) -> Result<GoSyntaxProviderProjection, GenerationError> {
    let checked = GoSyntaxProviderProjection::from_durable_parts(
        supplied.outcome.clone(), supplied.inputs.clone(), supplied.parser.clone(),
    ).map_err(|_| GenerationError::InvalidProviderMirror)?;
    (checked == *supplied).then_some(checked).ok_or(GenerationError::InvalidProviderMirror)
}

#[rustfmt::skip]
fn preflight_header(connection: &Connection, handle: GenerationHandle) -> Result<(), GenerationError> {
    let valid: bool = connection.query_row(
        "SELECT count(*)=1 AND coalesce(sum(CASE WHEN typeof(mirror_schema)='text' AND length(CAST(mirror_schema AS BLOB)) BETWEEN 1 AND 64 AND typeof(provider_id)='text' AND length(CAST(provider_id AS BLOB)) BETWEEN 1 AND 128 AND typeof(provider_version)='text' AND length(CAST(provider_version AS BLOB)) BETWEEN 1 AND 64 AND typeof(provider_kind)='text' AND length(CAST(provider_kind AS BLOB)) BETWEEN 1 AND 32 AND typeof(language_scope)='text' AND length(CAST(language_scope AS BLOB)) BETWEEN 1 AND 32 AND typeof(cache_policy)='text' AND length(CAST(cache_policy AS BLOB)) BETWEEN 1 AND 64 AND typeof(precision_ceiling)='text' AND length(CAST(precision_ceiling AS BLOB)) BETWEEN 1 AND 16 AND typeof(outcome_status)='text' AND length(CAST(outcome_status AS BLOB)) BETWEEN 1 AND 32 AND (failure_stage IS NULL OR (typeof(failure_stage)='text' AND length(CAST(failure_stage AS BLOB)) BETWEEN 1 AND 32)) AND (failure_reason IS NULL OR (typeof(failure_reason)='text' AND length(CAST(failure_reason AS BLOB)) BETWEEN 1 AND 32)) AND typeof(member_count)='integer' AND member_count BETWEEN 0 AND ?2 AND typeof(blocker_count)='integer' AND blocker_count BETWEEN 0 AND ?2 AND typeof(source_count)='integer' AND source_count BETWEEN 0 AND ?2 AND typeof(parser_count)='integer' AND parser_count BETWEEN 0 AND 1 AND typeof(witness_kind)='text' AND length(CAST(witness_kind AS BLOB)) BETWEEN 1 AND 32 AND typeof(witness_value)='text' AND length(CAST(witness_value AS BLOB))=16 AND (identity_provider_id IS NULL OR (typeof(identity_provider_id)='text' AND length(CAST(identity_provider_id AS BLOB)) BETWEEN 1 AND 128)) AND (identity_provider_version IS NULL OR (typeof(identity_provider_version)='text' AND length(CAST(identity_provider_version AS BLOB)) BETWEEN 1 AND 64)) AND (identity_schema_version IS NULL OR (typeof(identity_schema_version)='text' AND length(CAST(identity_schema_version AS BLOB)) BETWEEN 1 AND 64)) AND (identity_digest_kind IS NULL OR (typeof(identity_digest_kind)='text' AND length(CAST(identity_digest_kind AS BLOB)) BETWEEN 1 AND 32)) AND (identity_digest_value IS NULL OR (typeof(identity_digest_value)='text' AND length(CAST(identity_digest_value AS BLOB))=16)) AND (identity_precision IS NULL OR (typeof(identity_precision)='text' AND length(CAST(identity_precision AS BLOB)) BETWEEN 1 AND 16)) THEN 0 ELSE 1 END),0)=0 FROM go_syntax_provider_mirror WHERE generation_id=?1",
        params![handle.scalar(),MAX_ROWS], |row| row.get(0),
    ).map_err(connection::classify_sqlite_error)?;
    valid.then_some(()).ok_or(GenerationError::InvalidProviderMirror)
}

#[rustfmt::skip]
fn preflight_source_relationships(connection:&Connection,handle:GenerationHandle,expected:i64)->Result<(),GenerationError>{
    let valid:bool=connection.query_row(
        "SELECT (SELECT count(*) FROM run_manifest_sources WHERE generation_id=?1 AND language='go')=?2 AND NOT EXISTS(SELECT 1 FROM go_syntax_provider_sources AS source LEFT JOIN run_manifest_sources AS manifest ON manifest.generation_id=source.generation_id AND manifest.relative_path=source.relative_path AND manifest.language=source.language AND manifest.source_purpose='source-text-v1' AND manifest.source_value=source.digest_value WHERE source.generation_id=?1 AND manifest.generation_id IS NULL)",
        params![handle.scalar(),expected],|row|row.get(0),
    ).map_err(connection::classify_sqlite_error)?;
    valid.then_some(()).ok_or(GenerationError::InvalidProviderMirror)
}

#[rustfmt::skip]
fn read_header(connection: &Connection, handle: GenerationHandle) -> Result<RawHeader, GenerationError> {
    connection.query_row(
        "SELECT mirror_schema,provider_id,provider_version,provider_kind,language_scope,cache_policy,precision_ceiling,outcome_status,failure_stage,failure_reason,member_count,blocker_count,source_count,parser_count,witness_kind,witness_value,identity_provider_id,identity_provider_version,identity_schema_version,identity_digest_kind,identity_digest_value,identity_precision FROM go_syntax_provider_mirror WHERE generation_id=?1",
        [handle.scalar()], |row| Ok(RawHeader {
            mirror_schema:row.get(0)?,provider_id:row.get(1)?,provider_version:row.get(2)?,provider_kind:row.get(3)?,
            language_scope:row.get(4)?,cache_policy:row.get(5)?,precision_ceiling:row.get(6)?,status:row.get(7)?,
            stage:row.get(8)?,reason:row.get(9)?,member_count:row.get(10)?,blocker_count:row.get(11)?,
            source_count:row.get(12)?,parser_count:row.get(13)?,witness_kind:row.get(14)?,witness_value:row.get(15)?,
            identity_provider:row.get(16)?,identity_version:row.get(17)?,identity_schema:row.get(18)?,
            identity_kind:row.get(19)?,identity_value:row.get(20)?,identity_precision:row.get(21)?,
        }),
    ).map_err(|error| connection::classify_sqlite_error(error).into())
}

#[rustfmt::skip]
fn read_members(connection: &Connection, handle: GenerationHandle, expected: i64) -> Result<Vec<(String,i64,String,i64)>, GenerationError> {
    preflight(connection,"go_syntax_provider_members",handle,expected,
        "typeof(category)='text' AND length(category) BETWEEN 1 AND 16 AND typeof(ordinal)='integer' AND ordinal>=0 AND typeof(name)='text' AND length(CAST(name AS BLOB)) BETWEEN 1 AND 4096 AND typeof(version)='integer' AND version>=0",
        "length(CAST(category AS BLOB))+length(CAST(name AS BLOB))+32")?;
    let mut statement = connection.prepare(
        "SELECT category,ordinal,name,version FROM go_syntax_provider_members WHERE generation_id=?1 ORDER BY CASE category WHEN 'input' THEN 0 WHEN 'output' THEN 1 ELSE 2 END,ordinal"
    ).map_err(connection::classify_sqlite_error)?;
    statement.query_map([handle.scalar()],|row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)))
        .map_err(connection::classify_sqlite_error)?.collect::<Result<Vec<_>,_>>()
        .map_err(|error| connection::classify_sqlite_error(error).into())
}

#[rustfmt::skip]
fn read_blockers(connection: &Connection, handle: GenerationHandle, expected: i64) -> Result<Vec<String>, GenerationError> {
    preflight(connection,"go_syntax_provider_blockers",handle,expected,
        "typeof(ordinal)='integer' AND ordinal>=0 AND typeof(provider_id)='text' AND length(CAST(provider_id AS BLOB)) BETWEEN 1 AND 128",
        "length(CAST(provider_id AS BLOB))+32")?;
    let mut statement = connection.prepare(
        "SELECT provider_id FROM go_syntax_provider_blockers WHERE generation_id=?1 ORDER BY ordinal"
    ).map_err(connection::classify_sqlite_error)?;
    statement.query_map([handle.scalar()],|row| row.get(0))
        .map_err(connection::classify_sqlite_error)?.collect::<Result<Vec<_>,_>>()
        .map_err(|error| connection::classify_sqlite_error(error).into())
}

#[rustfmt::skip]
fn read_sources(connection: &Connection, handle: GenerationHandle, expected: i64) -> Result<Vec<CanonicalGoSyntaxSource>, GenerationError> {
    preflight(connection,"go_syntax_provider_sources",handle,expected,
        "typeof(ordinal)='integer' AND ordinal>=0 AND typeof(relative_path)='text' AND length(CAST(relative_path AS BLOB)) BETWEEN 1 AND 4096 AND typeof(language)='text' AND length(language) BETWEEN 1 AND 16 AND typeof(digest_kind)='text' AND length(digest_kind) BETWEEN 1 AND 32 AND typeof(digest_value)='text' AND length(digest_value)=16",
        "length(CAST(relative_path AS BLOB))+length(language)+length(digest_kind)+length(digest_value)+48")?;
    let mut statement = connection.prepare(
        "SELECT relative_path,language,digest_kind,digest_value FROM go_syntax_provider_sources WHERE generation_id=?1 ORDER BY ordinal"
    ).map_err(connection::classify_sqlite_error)?;
    let rows = statement.query_and_then([handle.scalar()],|row| {
        let language:String=cell(row,1)?; let kind:String=cell(row,2)?;
        if language!="go" || kind!=DigestKind::SourceText.as_str() { return Err(GenerationError::InvalidProviderMirror); }
        Ok(CanonicalGoSyntaxSource { path:cell(row,0)?,language:Language::Go,
            source_digest:Digest{kind:DigestKind::SourceText,value:cell(row,3)?} })
    }).map_err(connection::classify_sqlite_error)?;
    rows.collect()
}

#[rustfmt::skip]
fn read_parser(connection: &Connection, handle: GenerationHandle, expected: i64) -> Result<Option<GoSyntaxParserContract>, GenerationError> {
    preflight(connection,"go_syntax_provider_parser",handle,expected,
        "typeof(provider_id)='text' AND length(provider_id) BETWEEN 1 AND 128 AND typeof(provider_version)='text' AND length(provider_version) BETWEEN 1 AND 64 AND typeof(fact_schema)='text' AND length(fact_schema) BETWEEN 1 AND 64 AND typeof(payload_schema)='text' AND length(payload_schema) BETWEEN 1 AND 64 AND typeof(backend)='text' AND length(backend) BETWEEN 1 AND 64 AND typeof(grammar)='text' AND length(grammar) BETWEEN 1 AND 64 AND typeof(digest_kind)='text' AND length(digest_kind) BETWEEN 1 AND 32 AND typeof(digest_value)='text' AND length(digest_value)=16",
        "length(provider_id)+length(provider_version)+length(fact_schema)+length(payload_schema)+length(backend)+length(grammar)+length(digest_kind)+length(digest_value)+64")?;
    let mut statement = connection.prepare(
        "SELECT provider_id,provider_version,fact_schema,payload_schema,backend,grammar,digest_kind,digest_value FROM go_syntax_provider_parser WHERE generation_id=?1"
    ).map_err(connection::classify_sqlite_error)?;
    let mut rows=statement.query([handle.scalar()]).map_err(connection::classify_sqlite_error)?;
    let Some(row)=rows.next().map_err(connection::classify_sqlite_error)? else { return Ok(None) };
    let parser=GoSyntaxParserContract{provider_id:row.get(0).map_err(connection::classify_sqlite_error)?,provider_version:row.get(1).map_err(connection::classify_sqlite_error)?,fact_schema:row.get(2).map_err(connection::classify_sqlite_error)?,payload_schema:row.get(3).map_err(connection::classify_sqlite_error)?,backend:row.get(4).map_err(connection::classify_sqlite_error)?,grammar:row.get(5).map_err(connection::classify_sqlite_error)?};
    let kind:String=row.get(6).map_err(connection::classify_sqlite_error)?;
    let value:String=row.get(7).map_err(connection::classify_sqlite_error)?;
    if kind!=parser.digest().kind.as_str() || value!=parser.digest().value || rows.next().map_err(connection::classify_sqlite_error)?.is_some() {
        return Err(GenerationError::InvalidProviderMirror);
    }
    Ok(Some(parser))
}

#[rustfmt::skip]
fn cell<T: rusqlite::types::FromSql>(row:&rusqlite::Row<'_>,index:usize)->Result<T,GenerationError>{
    row.get(index).map_err(|error|connection::classify_sqlite_error(error).into())
}

#[rustfmt::skip]
fn preflight(connection:&Connection,table:&str,handle:GenerationHandle,expected:i64,valid:&str,aggregate:&str)->Result<(),GenerationError>{
    if !(0..=MAX_ROWS).contains(&expected){return Err(GenerationError::InvalidProviderMirror);}
    let sql=format!("SELECT count(*),coalesce(sum(CASE WHEN {valid} THEN 0 ELSE 1 END),0),coalesce(sum({aggregate}),0) FROM {table} WHERE generation_id=?1");
    let (count,invalid,bytes):(i64,i64,i64)=connection.query_row(&sql,[handle.scalar()],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).map_err(connection::classify_sqlite_error)?;
    let dense=if table=="go_syntax_provider_members"||table=="go_syntax_provider_parser"||expected==0{true}else{connection.query_row(&format!("SELECT min(ordinal)=0 AND max(ordinal)=?2-1 FROM {table} WHERE generation_id=?1"),params![handle.scalar(),expected],|row|row.get(0)).map_err(connection::classify_sqlite_error)?};
    (count==expected&&invalid==0&&bytes<=super::max_aggregate_bytes()&&dense).then_some(()).ok_or(GenerationError::InvalidProviderMirror)
}

#[rustfmt::skip]
fn decode_identity(raw:&RawHeader)->Result<Option<ProviderOutputIdentity>,GenerationError>{
    match (&raw.identity_provider,&raw.identity_version,&raw.identity_schema,&raw.identity_kind,&raw.identity_value,&raw.identity_precision){
        (Some(provider_id),Some(provider_version),Some(schema_version),Some(kind),Some(value),Some(precision))
            if kind==DigestKind::ProviderOutput.as_str()&&precision=="syntax"=>Ok(Some(ProviderOutputIdentity{
                provider_id:provider_id.clone(),provider_version:provider_version.clone(),schema_version:schema_version.clone(),
                output_digest:Digest{kind:DigestKind::ProviderOutput,value:value.clone()},precision:PrecisionTier::Syntax})),
        (None,None,None,None,None,None)=>Ok(None),_=>Err(GenerationError::InvalidProviderMirror),
    }
}

#[rustfmt::skip]
fn decode_optional<T>(value:Option<&str>,decode:fn(&str)->Option<T>)->Result<Option<T>,GenerationError>{
    value.map(|value|decode(value).ok_or(GenerationError::InvalidProviderMirror)).transpose()
}

#[rustfmt::skip]
fn member_rows(manifest:&crate::analysis_kernel::ProviderManifest)->Vec<(String,i64,String,i64)>{
    let mut rows=Vec::new();
    for (ordinal,name) in manifest.inputs.iter().enumerate(){rows.push(("input".into(),ordinal as i64,(*name).into(),0));}
    for (ordinal,name) in manifest.outputs.iter().enumerate(){rows.push(("output".into(),ordinal as i64,(*name).into(),0));}
    // Sorted before ordinals are assigned, matching `provider_mirror`: the
    // manifest's declaration order is not a canonical order, and the two
    // mirrors must canonicalise identically.
    let mut schemas = manifest.schema_versions.to_vec();
    schemas.sort_by_key(|schema| (schema.name, schema.version));
    for (ordinal,schema) in schemas.iter().enumerate(){rows.push(("schema".into(),ordinal as i64,schema.name.into(),i64::from(schema.version)));}
    rows
}

#[rustfmt::skip]
fn witness(projection:&GoSyntaxProviderProjection)->Digest{
    let mut digest=Digest::builder(DigestKind::ProviderParameters,"go-syntax-provider-mirror-v1");
    let manifest=projection.manifest;
    for value in [manifest.id,manifest.provider_version(),kind_label(manifest.kind),manifest.language_scope_label(),precision_ceiling_label(manifest.precision_ceiling)]{digest.field("manifest",value);}
    digest.field("cache-policy",&manifest.cache_policy_label());
    for (category,ordinal,name,version) in member_rows(&manifest){digest.field("member-category",&category);digest.u64_field("member-ordinal",ordinal as u64);digest.field("member-name",&name);digest.u64_field("member-version",version as u64);}
    let outcome=&projection.outcome;
    digest.field("status",outcome.status.label());digest.field("stage",outcome.failure_stage.map_or("absent",ProviderFailureStage::label));digest.field("reason",outcome.failure_reason.map_or("absent",ProviderFailureReason::label));
    if let Some(identity)=&outcome.output_identity{for value in [&identity.provider_id,&identity.provider_version,&identity.schema_version,identity.output_digest.kind.as_str(),&identity.output_digest.value,precision_label(identity.precision)]{digest.field("identity",value);}}
    for blocker in &outcome.blockers{digest.field("blocker",blocker);}
    if let Some(inputs)=&projection.inputs{for source in &inputs.sources{let row=source.digest();digest.field("dependency-kind",row.kind.as_str());digest.field("dependency-value",&row.value);}}
    if let Some(parser)=&projection.parser{let row=parser.digest();digest.field("dependency-kind",row.kind.as_str());digest.field("dependency-value",&row.value);}
    digest.finish()
}

#[rustfmt::skip]
const fn kind_label(kind:ProviderKind)->&'static str{match kind{ProviderKind::LanguageSyntax=>"language_syntax",ProviderKind::SourceDiscovery=>"source_discovery",ProviderKind::WholeRepoDerived=>"whole_repo_derived",ProviderKind::MetricsDerived=>"metrics_derived"}}
#[rustfmt::skip]
const fn precision_ceiling_label(value:PrecisionCeiling)->&'static str{match value{PrecisionCeiling::Exact=>"exact",PrecisionCeiling::Syntax=>"syntax",PrecisionCeiling::SetupAware=>"setup_aware"}}
#[rustfmt::skip]
const fn precision_label(value:PrecisionTier)->&'static str{match value{PrecisionTier::Exact=>"exact",PrecisionTier::Syntax=>"syntax",PrecisionTier::SetupAware=>"setup_aware"}}
#[rustfmt::skip]
fn map_write_error(error:rusqlite::Error)->GenerationError{if error.sqlite_error_code()==Some(rusqlite::ErrorCode::ConstraintViolation){GenerationError::InvalidProviderMirror}else{connection::classify_sqlite_error(error).into()}}

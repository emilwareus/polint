#![cfg(test)]

use crate::analysis_kernel::{
    FactConfidence, FactFamily, FactMeta, FactPrecision, FactRef, ValidationStatus,
};
use crate::analysis::mir::body::MirStatus;
use crate::analysis::mir::op::{AssignMode, ConservativeAction, MirOperationKind};
use crate::analysis::places::{PlaceProjection, PlaceRoot, PlaceStatus};
use crate::core::{
    AnalysisDb, FileId, Language, ReferenceFact, Span, SymbolPrecision, SymbolResolutionStatus,
};
use crate::symbol_graph::semantic::SemanticStatus;
use serde::Serialize;
use serde_json::Value;

pub(crate) fn metadata_debug_json_for_test(db: &AnalysisDb) -> Value {
    let report = MetadataDebugReport {
        files: file_rows(db),
        imports: import_rows(db),
        symbols: symbol_rows(db),
        references: reference_rows(db),
        semantic: semantic_report(db),
        mir: mir_report(db),
    };
    serde_json::to_value(report).expect("metadata debug report should serialize")
}

#[derive(Serialize)]
struct MetadataDebugReport<'a> {
    files: Vec<FileDebugRow<'a>>,
    imports: Vec<ImportDebugRow<'a>>,
    symbols: Vec<SymbolDebugRow<'a>>,
    references: Vec<ReferenceDebugRow<'a>>,
    semantic: SemanticDebugReport,
    mir: SemanticMirDebugReport,
}

#[derive(Serialize)]
struct MetadataDebugFields<'a> {
    family: &'static str,
    run_id: u64,
    stable_key: &'a str,
    producer_id: &'a str,
    layer_id: &'a str,
    precision: &'static str,
    confidence: &'static str,
    validation: &'static str,
}

#[derive(Serialize)]
struct FileDebugRow<'a> {
    #[serde(flatten)]
    metadata: MetadataDebugFields<'a>,
    path: &'a str,
    language: Language,
    content_hash: &'a str,
}

#[derive(Serialize)]
struct ImportDebugRow<'a> {
    #[serde(flatten)]
    metadata: MetadataDebugFields<'a>,
    path: &'a str,
    language: Language,
    import_path: &'a str,
    span: DebugSpan,
}

#[derive(Serialize)]
struct SymbolDebugRow<'a> {
    #[serde(flatten)]
    metadata: MetadataDebugFields<'a>,
    name: &'a str,
    qualified_name: &'a str,
    kind: &'static str,
    namespace: &'static str,
    path: Option<&'a str>,
    span: Option<DebugSpan>,
    fact_precision: &'static str,
}

#[derive(Serialize)]
struct ReferenceDebugRow<'a> {
    #[serde(flatten)]
    metadata: MetadataDebugFields<'a>,
    name: &'a str,
    qualified_name: &'a str,
    status: &'static str,
    target: Option<u64>,
    candidates: Vec<u64>,
    path: Option<&'a str>,
    span: Option<DebugSpan>,
    fact_precision: &'static str,
}

#[derive(Serialize)]
struct SemanticDebugReport {
    scopes: Vec<SemanticDebugRow>,
    imports: Vec<SemanticDebugRow>,
    exports: Vec<SemanticDebugRow>,
    aliases: Vec<SemanticDebugRow>,
    resolutions: Vec<SemanticDebugRow>,
    generated_symbols: Vec<SemanticDebugRow>,
    stable_exports: Vec<SemanticDebugRow>,
}

#[derive(Serialize)]
struct SemanticDebugRow {
    family: &'static str,
    run_id: u64,
    stable_key: String,
    producer_id: &'static str,
    layer_id: &'static str,
    status: &'static str,
    fact_precision: &'static str,
    metadata: SemanticMetadataDebugFields,
    path: Option<String>,
    span: Option<DebugSpan>,
    name: Option<String>,
    export_name: Option<String>,
    source_stable_key: Option<String>,
    target_stable_keys: Vec<String>,
    generated_discriminator: Option<String>,
}

#[derive(Serialize)]
struct SemanticMetadataDebugFields {
    precision: &'static str,
    confidence: &'static str,
    validation: &'static str,
}

#[derive(Serialize)]
struct SemanticMirDebugReport {
    bodies: Vec<SemanticMirDebugRow>,
    operations: Vec<SemanticMirDebugRow>,
    places: Vec<SemanticMirDebugRow>,
    unsupported: Vec<SemanticMirDebugRow>,
}

#[derive(Serialize)]
struct SemanticMirDebugRow {
    family: &'static str,
    run_id: u64,
    stable_key: String,
    producer_id: &'static str,
    layer_id: &'static str,
    status: String,
    precision: &'static str,
    path: Option<String>,
    span: Option<DebugSpan>,
    owner_function: Option<u64>,
    operation_kind: Option<String>,
    place_root: Option<String>,
    place_projections: Vec<String>,
    unsupported_construct: Option<String>,
    conservative_action: Option<String>,
}

#[derive(Clone, Copy, Serialize)]
struct DebugSpan {
    start_byte: u32,
    end_byte: u32,
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
}

fn file_rows(db: &AnalysisDb) -> Vec<FileDebugRow<'_>> {
    let mut rows = db
        .files()
        .iter()
        .filter_map(|file| {
            let run_id = u64::from(file.id.0);
            metadata_fields(db, FactFamily::SourceFile, run_id).map(|metadata| FileDebugRow {
                metadata,
                path: file.relative_path.as_str(),
                language: file.language,
                content_hash: file.content_hash.as_str(),
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        (left.path, left.metadata.stable_key, left.metadata.run_id).cmp(&(
            right.path,
            right.metadata.stable_key,
            right.metadata.run_id,
        ))
    });
    rows
}

fn import_rows(db: &AnalysisDb) -> Vec<ImportDebugRow<'_>> {
    let mut rows = db
        .imports()
        .iter()
        .filter_map(|fact| {
            let file = db.file(fact.file)?;
            metadata_fields(db, FactFamily::Import, fact.id.0).map(|metadata| ImportDebugRow {
                metadata,
                path: file.relative_path.as_str(),
                language: fact.language,
                import_path: fact.path.as_str(),
                span: debug_span(&fact.span),
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        (
            left.path,
            left.span.start_byte,
            left.import_path,
            left.metadata.stable_key,
            left.metadata.run_id,
        )
            .cmp(&(
                right.path,
                right.span.start_byte,
                right.import_path,
                right.metadata.stable_key,
                right.metadata.run_id,
            ))
    });
    rows
}

fn symbol_rows(db: &AnalysisDb) -> Vec<SymbolDebugRow<'_>> {
    let mut rows = db
        .symbols()
        .iter()
        .filter_map(|fact| {
            metadata_fields(db, FactFamily::Symbol, fact.id.0).map(|metadata| SymbolDebugRow {
                metadata,
                name: fact.name.as_str(),
                qualified_name: fact.qualified_name.as_str(),
                kind: symbol_kind_label(fact.kind),
                namespace: symbol_namespace_label(fact.namespace),
                path: relative_path(db, fact.file),
                span: fact.primary_span.as_ref().map(debug_span),
                fact_precision: symbol_precision_label(fact.precision),
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        (
            left.path.unwrap_or(""),
            span_start(left.span),
            left.name,
            left.metadata.stable_key,
            left.metadata.run_id,
        )
            .cmp(&(
                right.path.unwrap_or(""),
                span_start(right.span),
                right.name,
                right.metadata.stable_key,
                right.metadata.run_id,
            ))
    });
    rows
}

fn reference_rows(db: &AnalysisDb) -> Vec<ReferenceDebugRow<'_>> {
    let mut rows = db
        .references()
        .iter()
        .filter_map(|fact| {
            metadata_fields(db, FactFamily::Reference, fact.id.0).map(|metadata| {
                ReferenceDebugRow {
                    metadata,
                    name: fact.name.as_str(),
                    qualified_name: fact.qualified_name.as_str(),
                    status: symbol_resolution_status_label(fact.status),
                    target: fact.target.map(|target| target.0),
                    candidates: sorted_candidate_ids(fact),
                    path: relative_path(db, fact.file),
                    span: fact.primary_span.as_ref().map(debug_span),
                    fact_precision: symbol_precision_label(fact.precision),
                }
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        (
            left.path.unwrap_or(""),
            span_start(left.span),
            left.name,
            left.metadata.stable_key,
            left.metadata.run_id,
        )
            .cmp(&(
                right.path.unwrap_or(""),
                span_start(right.span),
                right.name,
                right.metadata.stable_key,
                right.metadata.run_id,
            ))
    });
    rows
}

fn semantic_report(db: &AnalysisDb) -> SemanticDebugReport {
    let mut scopes = db
        .scopes()
        .iter()
        .filter_map(|fact| {
            semantic_row(
                db,
                FactFamily::Scope,
                fact.id.0,
                fact.stable_key.as_str(),
                fact.status,
                fact.file,
                None,
                Some(fact.scope_path.join("::")),
                None,
                None,
                Vec::new(),
                None,
            )
        })
        .collect::<Vec<_>>();
    let mut imports = db
        .semantic_imports()
        .iter()
        .filter_map(|fact| {
            semantic_row(
                db,
                FactFamily::SemanticImport,
                fact.id.0,
                fact.stable_key.as_str(),
                fact.status,
                fact.file,
                None,
                fact.local_name
                    .as_ref()
                    .or(fact.imported_name.as_ref())
                    .cloned()
                    .or_else(|| Some(fact.import_path.clone())),
                None,
                None,
                Vec::new(),
                None,
            )
        })
        .collect::<Vec<_>>();
    let mut exports = db
        .exports()
        .iter()
        .filter_map(|fact| {
            semantic_row(
                db,
                FactFamily::Export,
                fact.id.0,
                fact.stable_key.as_str(),
                fact.status,
                fact.file,
                None,
                None,
                Some(fact.export_name.clone()),
                None,
                Vec::new(),
                None,
            )
        })
        .collect::<Vec<_>>();
    let mut aliases = db
        .aliases()
        .iter()
        .filter_map(|fact| {
            semantic_row(
                db,
                FactFamily::Alias,
                fact.id.0,
                fact.stable_key.as_str(),
                fact.status,
                fact.file,
                None,
                None,
                None,
                Some(fact.source_symbol_stable_key.clone()),
                fact.target_symbol_stable_keys.clone(),
                None,
            )
        })
        .collect::<Vec<_>>();
    let mut resolutions = db
        .resolution_facts()
        .iter()
        .filter_map(|fact| {
            semantic_row(
                db,
                FactFamily::Resolution,
                fact.id.0,
                fact.stable_key.as_str(),
                fact.status,
                fact.file,
                None,
                None,
                None,
                Some(fact.source_stable_key.clone()),
                fact.target_stable_keys.clone(),
                None,
            )
        })
        .collect::<Vec<_>>();
    let mut generated_symbols = db
        .generated_symbols()
        .iter()
        .filter_map(|fact| {
            semantic_row(
                db,
                FactFamily::GeneratedSymbol,
                fact.id.0,
                fact.stable_key.as_str(),
                fact.status,
                fact.file,
                fact.span.as_ref().map(debug_span),
                Some(fact.symbol_stable_key.clone()),
                None,
                Some(fact.source_stable_key.clone()),
                Vec::new(),
                Some(fact.generated_discriminator.clone()),
            )
        })
        .collect::<Vec<_>>();
    let mut stable_exports = db
        .stable_exports()
        .iter()
        .filter_map(|fact| {
            semantic_row(
                db,
                FactFamily::StableExport,
                fact.id.0,
                fact.stable_key.as_str(),
                fact.status,
                None,
                None,
                None,
                Some(fact.export_name.clone()),
                Some(fact.symbol_stable_key.clone()),
                Vec::new(),
                fact.generated_discriminator.clone(),
            )
        })
        .collect::<Vec<_>>();

    sort_semantic_rows(&mut scopes);
    sort_semantic_rows(&mut imports);
    sort_semantic_rows(&mut exports);
    sort_semantic_rows(&mut aliases);
    sort_semantic_rows(&mut resolutions);
    sort_semantic_rows(&mut generated_symbols);
    sort_semantic_rows(&mut stable_exports);

    SemanticDebugReport {
        scopes,
        imports,
        exports,
        aliases,
        resolutions,
        generated_symbols,
        stable_exports,
    }
}

fn mir_report(db: &AnalysisDb) -> SemanticMirDebugReport {
    SemanticMirDebugReport {
        bodies: mir_body_rows(db),
        operations: mir_operation_rows(db),
        places: mir_place_rows(db),
        unsupported: mir_unsupported_rows(db),
    }
}

fn mir_body_rows(db: &AnalysisDb) -> Vec<SemanticMirDebugRow> {
    let mut rows = db
        .mir_bodies()
        .iter()
        .filter_map(|body| {
            mir_metadata_row(db, FactFamily::MirBody, body.id.0).map(|metadata| {
                SemanticMirDebugRow {
                    family: FactFamily::MirBody.label(),
                    run_id: body.id.0,
                    stable_key: body.stable_key.clone(),
                    producer_id: metadata.producer_id,
                    layer_id: metadata.layer_id,
                    status: mir_status_label(body.status).to_string(),
                    precision: fact_precision_label(metadata.precision),
                    path: relative_path(db, Some(body.file)).map(str::to_string),
                    span: Some(debug_span(&body.span)),
                    owner_function: Some(body.function.0),
                    operation_kind: None,
                    place_root: None,
                    place_projections: Vec::new(),
                    unsupported_construct: None,
                    conservative_action: None,
                }
            })
        })
        .collect::<Vec<_>>();
    sort_mir_rows(&mut rows);
    rows
}

fn mir_operation_rows(db: &AnalysisDb) -> Vec<SemanticMirDebugRow> {
    let mut rows = db
        .mir_operations()
        .iter()
        .filter_map(|operation| {
            mir_metadata_row(db, FactFamily::MirOperation, operation.id.0).map(|metadata| {
                let body = db
                    .mir_bodies()
                    .iter()
                    .find(|body| body.id == operation.body);
                SemanticMirDebugRow {
                    family: FactFamily::MirOperation.label(),
                    run_id: operation.id.0,
                    stable_key: operation.stable_key.clone(),
                    producer_id: metadata.producer_id,
                    layer_id: metadata.layer_id,
                    status: mir_status_label(operation.status).to_string(),
                    precision: fact_precision_label(metadata.precision),
                    path: body
                        .and_then(|body| relative_path(db, Some(body.file)))
                        .map(str::to_string),
                    span: Some(debug_span(&operation.span)),
                    owner_function: body.map(|body| body.function.0),
                    operation_kind: Some(operation_kind_label(&operation.kind).to_string()),
                    place_root: None,
                    place_projections: Vec::new(),
                    unsupported_construct: None,
                    conservative_action: None,
                }
            })
        })
        .collect::<Vec<_>>();
    sort_mir_rows(&mut rows);
    rows
}

fn mir_place_rows(db: &AnalysisDb) -> Vec<SemanticMirDebugRow> {
    let mut rows = db
        .mir_places()
        .iter()
        .filter_map(|place| {
            mir_metadata_row(db, FactFamily::Place, place.id.0).map(|metadata| {
                SemanticMirDebugRow {
                    family: FactFamily::Place.label(),
                    run_id: place.id.0,
                    stable_key: place.stable_key.clone(),
                    producer_id: metadata.producer_id,
                    layer_id: metadata.layer_id,
                    status: place_status_label(place.status).to_string(),
                    precision: fact_precision_label(metadata.precision),
                    path: relative_path(db, place.file).map(str::to_string),
                    span: None,
                    owner_function: place.function.map(|function| function.0),
                    operation_kind: None,
                    place_root: Some(place_root_label(&place.root)),
                    place_projections: place
                        .projections
                        .iter()
                        .map(place_projection_label)
                        .collect(),
                    unsupported_construct: None,
                    conservative_action: None,
                }
            })
        })
        .collect::<Vec<_>>();
    sort_mir_rows(&mut rows);
    rows
}

fn mir_unsupported_rows(db: &AnalysisDb) -> Vec<SemanticMirDebugRow> {
    let mut rows = db
        .unsupported_semantics()
        .iter()
        .filter_map(|row| {
            mir_metadata_row(db, FactFamily::UnsupportedSemantic, row.id.0).map(|metadata| {
                SemanticMirDebugRow {
                    family: FactFamily::UnsupportedSemantic.label(),
                    run_id: row.id.0,
                    stable_key: row.stable_key.clone(),
                    producer_id: metadata.producer_id,
                    layer_id: metadata.layer_id,
                    status: mir_status_label(row.status).to_string(),
                    precision: fact_precision_label(metadata.precision),
                    path: relative_path(db, Some(row.file)).map(str::to_string),
                    span: Some(debug_span(&row.span)),
                    owner_function: row
                        .body
                        .and_then(|body_id| db.mir_bodies().iter().find(|body| body.id == body_id))
                        .map(|body| body.function.0),
                    operation_kind: None,
                    place_root: None,
                    place_projections: Vec::new(),
                    unsupported_construct: Some(row.construct.clone()),
                    conservative_action: Some(
                        conservative_action_label(row.conservative_action).to_string(),
                    ),
                }
            })
        })
        .collect::<Vec<_>>();
    sort_mir_rows(&mut rows);
    rows
}

fn mir_metadata_row(
    db: &AnalysisDb,
    family: FactFamily,
    run_id: u64,
) -> Option<&FactMeta> {
    db.metadata_for(FactRef::new(family, run_id))
}

#[expect(
    clippy::too_many_arguments,
    reason = "semantic debug rows normalize several distinct internal fact families"
)]
fn semantic_row(
    db: &AnalysisDb,
    family: FactFamily,
    run_id: u64,
    stable_key: &str,
    status: SemanticStatus,
    file: Option<FileId>,
    span: Option<DebugSpan>,
    name: Option<String>,
    export_name: Option<String>,
    source_stable_key: Option<String>,
    target_stable_keys: Vec<String>,
    generated_discriminator: Option<String>,
) -> Option<SemanticDebugRow> {
    let meta = db.metadata_for(FactRef::new(family, run_id))?;
    Some(SemanticDebugRow {
        family: family.label(),
        run_id,
        stable_key: stable_key.to_string(),
        producer_id: meta.producer_id,
        layer_id: meta.layer_id,
        status: semantic_status_label(status),
        fact_precision: fact_precision_label(meta.precision),
        metadata: semantic_metadata_debug_fields(meta),
        path: relative_path(db, file).map(str::to_string),
        span,
        name,
        export_name,
        source_stable_key,
        target_stable_keys,
        generated_discriminator,
    })
}

fn semantic_metadata_debug_fields(meta: &FactMeta) -> SemanticMetadataDebugFields {
    SemanticMetadataDebugFields {
        precision: fact_precision_label(meta.precision),
        confidence: fact_confidence_label(meta.confidence),
        validation: validation_status_label(meta.validation),
    }
}

fn sort_semantic_rows(rows: &mut [SemanticDebugRow]) {
    rows.sort_by(|left, right| {
        (
            left.path.as_deref().unwrap_or(""),
            span_start(left.span),
            left.name.as_deref().unwrap_or(""),
            left.export_name.as_deref().unwrap_or(""),
            left.status,
            left.stable_key.as_str(),
            left.run_id,
        )
            .cmp(&(
                right.path.as_deref().unwrap_or(""),
                span_start(right.span),
                right.name.as_deref().unwrap_or(""),
                right.export_name.as_deref().unwrap_or(""),
                right.status,
                right.stable_key.as_str(),
                right.run_id,
            ))
    });
}

fn sort_mir_rows(rows: &mut [SemanticMirDebugRow]) {
    rows.sort_by(|left, right| {
        (
            left.path.as_deref().unwrap_or(""),
            span_start(left.span),
            left.stable_key.as_str(),
            left.run_id,
        )
            .cmp(&(
                right.path.as_deref().unwrap_or(""),
                span_start(right.span),
                right.stable_key.as_str(),
                right.run_id,
            ))
    });
}

fn metadata_fields(
    db: &AnalysisDb,
    family: FactFamily,
    run_id: u64,
) -> Option<MetadataDebugFields<'_>> {
    let meta = db.metadata_for(FactRef::new(family, run_id))?;
    Some(metadata_debug_fields(family, run_id, meta))
}

fn metadata_debug_fields<'a>(
    family: FactFamily,
    run_id: u64,
    meta: &'a FactMeta,
) -> MetadataDebugFields<'a> {
    MetadataDebugFields {
        family: family.label(),
        run_id,
        stable_key: meta.stable_key.as_str(),
        producer_id: meta.producer_id,
        layer_id: meta.layer_id,
        precision: fact_precision_label(meta.precision),
        confidence: fact_confidence_label(meta.confidence),
        validation: validation_status_label(meta.validation),
    }
}

fn relative_path(db: &AnalysisDb, file: Option<FileId>) -> Option<&str> {
    file.and_then(|file| db.file(file))
        .map(|file| file.relative_path.as_str())
}

fn debug_span(span: &Span) -> DebugSpan {
    DebugSpan {
        start_byte: span.start_byte,
        end_byte: span.end_byte,
        start_line: span.start_line,
        start_col: span.start_col,
        end_line: span.end_line,
        end_col: span.end_col,
    }
}

fn span_start(span: Option<DebugSpan>) -> u32 {
    span.map(|span| span.start_byte).unwrap_or(u32::MAX)
}

fn sorted_candidate_ids(reference: &ReferenceFact) -> Vec<u64> {
    let mut candidates = reference
        .candidates
        .iter()
        .map(|candidate| candidate.0)
        .collect::<Vec<_>>();
    candidates.sort_unstable();
    candidates
}

fn fact_precision_label(precision: FactPrecision) -> &'static str {
    match precision {
        FactPrecision::Exact => "exact",
        FactPrecision::Syntax => "syntax",
        FactPrecision::SetupAware => "setup_aware",
        FactPrecision::Heuristic => "heuristic",
        FactPrecision::Unresolved => "unresolved",
        FactPrecision::Ambiguous => "ambiguous",
        FactPrecision::SetupMissing => "setup_missing",
        FactPrecision::Unsupported => "unsupported",
    }
}

fn fact_confidence_label(confidence: FactConfidence) -> &'static str {
    match confidence {
        FactConfidence::High => "high",
        FactConfidence::Medium => "medium",
        FactConfidence::Low => "low",
    }
}

fn validation_status_label(status: ValidationStatus) -> &'static str {
    match status {
        ValidationStatus::NativeTrusted => "native_trusted",
        ValidationStatus::SchemaValidated => "schema_validated",
        ValidationStatus::ReferentiallyValidated => "referentially_validated",
        ValidationStatus::SpanValidated => "span_validated",
        ValidationStatus::StableKeyValidated => "stable_key_validated",
        ValidationStatus::ConflictRejected => "conflict_rejected",
    }
}

fn symbol_precision_label(precision: SymbolPrecision) -> &'static str {
    match precision {
        SymbolPrecision::ExactSemantic => "exact_semantic",
        SymbolPrecision::ExactLocal => "exact_local",
        SymbolPrecision::ModuleLinked => "module_linked",
        SymbolPrecision::Heuristic => "heuristic",
        SymbolPrecision::Unresolved => "unresolved",
        SymbolPrecision::Ambiguous => "ambiguous",
        SymbolPrecision::SetupMissing => "setup_missing",
        SymbolPrecision::Unsupported => "unsupported",
    }
}

fn symbol_resolution_status_label(status: SymbolResolutionStatus) -> &'static str {
    match status {
        SymbolResolutionStatus::Resolved => "resolved",
        SymbolResolutionStatus::Unresolved => "unresolved",
        SymbolResolutionStatus::Ambiguous => "ambiguous",
        SymbolResolutionStatus::SetupMissing => "setup_missing",
        SymbolResolutionStatus::Unsupported => "unsupported",
    }
}

fn semantic_status_label(status: SemanticStatus) -> &'static str {
    match status {
        SemanticStatus::Resolved => "resolved",
        SemanticStatus::Ambiguous => "ambiguous",
        SemanticStatus::Unresolved => "unresolved",
        SemanticStatus::Cycle => "cycle",
        SemanticStatus::Generated => "generated",
        SemanticStatus::Dynamic => "dynamic",
        SemanticStatus::External => "external",
        SemanticStatus::SetupMissing => "setup_missing",
        SemanticStatus::Unsupported => "unsupported",
    }
}

fn mir_status_label(status: MirStatus) -> &'static str {
    match status {
        MirStatus::Resolved => "resolved",
        MirStatus::Partial => "partial",
        MirStatus::Unknown => "unknown",
        MirStatus::Unsupported => "unsupported",
    }
}

fn place_status_label(status: PlaceStatus) -> &'static str {
    match status {
        PlaceStatus::Resolved => "resolved",
        PlaceStatus::Partial => "partial",
        PlaceStatus::Unknown => "unknown",
        PlaceStatus::Unsupported => "unsupported",
    }
}

fn operation_kind_label(kind: &MirOperationKind) -> &'static str {
    match kind {
        MirOperationKind::StorageLive { .. } => "storage_live",
        MirOperationKind::Bind { .. } => "bind",
        MirOperationKind::Assign { mode, .. } => match mode {
            AssignMode::DeclarationBinding => "assign:declaration_binding",
            AssignMode::Overwrite => "assign:overwrite",
            AssignMode::PartialWrite => "assign:partial_write",
            AssignMode::Simultaneous => "assign:simultaneous",
            AssignMode::ProjectionMutation => "assign:projection_mutation",
            AssignMode::UnknownWrite => "assign:unknown_write",
        },
        MirOperationKind::Read { .. } => "read",
        MirOperationKind::Write { .. } => "write",
        MirOperationKind::Branch { .. } => "branch",
        MirOperationKind::Call { .. } => "call",
        MirOperationKind::Return { .. } => "return",
        MirOperationKind::Unsupported { .. } => "unsupported",
    }
}

fn place_root_label(root: &PlaceRoot) -> String {
    match root {
        PlaceRoot::Local { name, .. } => format!("local:{name}"),
        PlaceRoot::Parameter { index, name, .. } => {
            format!(
                "parameter:{index}:{}",
                name.as_deref().unwrap_or("<anonymous>")
            )
        }
        PlaceRoot::Global { symbol, name } => {
            format!(
                "global:{}:{name}",
                symbol.map_or_else(|| "none".to_string(), |symbol| symbol.0.to_string())
            )
        }
        PlaceRoot::Temporary { body, ordinal } => {
            format!("temporary:{}:{ordinal}", body.0)
        }
        PlaceRoot::CallReturn { call } => format!("call_return:{}", call.0),
        PlaceRoot::Unknown { evidence } => format!("unknown:{evidence}"),
    }
}

fn place_projection_label(projection: &PlaceProjection) -> String {
    match projection {
        PlaceProjection::Field(name) => format!("field:{name}"),
        PlaceProjection::Property(name) => format!("property:{name}"),
        PlaceProjection::IndexKnown(index) => format!("index_known:{index}"),
        PlaceProjection::IndexUnknown { evidence } => format!("index_unknown:{evidence}"),
        PlaceProjection::Deref => "deref".to_string(),
        PlaceProjection::AwaitResult => "await_result".to_string(),
        PlaceProjection::CallReturn(call) => format!("call_return:{}", call.0),
        PlaceProjection::Unknown { evidence } => format!("unknown:{evidence}"),
    }
}

fn conservative_action_label(action: ConservativeAction) -> &'static str {
    match action {
        ConservativeAction::SkipOperation => "skip_operation",
        ConservativeAction::HavocAffectedPlaces => "havoc_affected_places",
        ConservativeAction::PreserveWithUnknownValue => "preserve_with_unknown_value",
        ConservativeAction::StopLowering => "stop_lowering",
    }
}

fn symbol_kind_label(kind: crate::core::SymbolKind) -> &'static str {
    match kind {
        crate::core::SymbolKind::Package => "package",
        crate::core::SymbolKind::Module => "module",
        crate::core::SymbolKind::File => "file",
        crate::core::SymbolKind::Function => "function",
        crate::core::SymbolKind::Method => "method",
        crate::core::SymbolKind::Class => "class",
        crate::core::SymbolKind::Interface => "interface",
        crate::core::SymbolKind::TypeAlias => "type_alias",
        crate::core::SymbolKind::Enum => "enum",
        crate::core::SymbolKind::EnumMember => "enum_member",
        crate::core::SymbolKind::Variable => "variable",
        crate::core::SymbolKind::Constant => "constant",
        crate::core::SymbolKind::Parameter => "parameter",
        crate::core::SymbolKind::Field => "field",
        crate::core::SymbolKind::Property => "property",
        crate::core::SymbolKind::Namespace => "namespace",
        crate::core::SymbolKind::Import => "import",
        crate::core::SymbolKind::Export => "export",
        crate::core::SymbolKind::Unknown => "unknown",
    }
}

fn symbol_namespace_label(namespace: crate::core::SymbolNamespace) -> &'static str {
    match namespace {
        crate::core::SymbolNamespace::Value => "value",
        crate::core::SymbolNamespace::Type => "type",
        crate::core::SymbolNamespace::Namespace => "namespace",
        crate::core::SymbolNamespace::Package => "package",
        crate::core::SymbolNamespace::Module => "module",
        crate::core::SymbolNamespace::Unknown => "unknown",
    }
}

mod semantic_debug_json {
    use super::super::{AnalysisKernel, KernelInput};
    use crate::analysis_plan::AnalysisPlan;
    use crate::cache::Cache;
    use crate::config::load_config;
    use serde_json::Value;
    use std::fs;

    #[test]
    fn metadata_debug_json_contains_semantic_index_families() {
        let report = debug_report_from_kernel_run();
        let semantic = report["semantic"]
            .as_object()
            .unwrap_or_else(|| panic!("missing semantic debug object: {report:#?}"));

        for key in [
            "scopes",
            "imports",
            "exports",
            "aliases",
            "resolutions",
            "generated_symbols",
            "stable_exports",
        ] {
            assert!(
                semantic
                    .get(key)
                    .and_then(serde_json::Value::as_array)
                    .is_some(),
                "semantic debug object missing `{key}` array: {semantic:#?}"
            );
        }
        assert!(
            !semantic["stable_exports"].as_array().unwrap().is_empty(),
            "debug fixture should expose stable export identities: {semantic:#?}"
        );
        assert!(
            !semantic["generated_symbols"].as_array().unwrap().is_empty(),
            "debug fixture should expose native generated symbol hooks: {semantic:#?}"
        );
    }

    #[test]
    fn semantic_debug_json_rows_include_status_fact_precision_and_nested_metadata() {
        let report = debug_report_from_kernel_run();
        let row = report["semantic"]["generated_symbols"]
            .as_array()
            .and_then(|rows| rows.first())
            .unwrap_or_else(|| panic!("missing generated symbol semantic debug row: {report:#?}"));

        for field in [
            "status",
            "fact_precision",
            "stable_key",
            "producer_id",
            "layer_id",
        ] {
            assert!(
                row.get(field).is_some(),
                "semantic generated row missing `{field}`: {row:#?}"
            );
        }
        let metadata = row["metadata"]
            .as_object()
            .unwrap_or_else(|| panic!("missing nested metadata object: {row:#?}"));
        for field in ["precision", "confidence", "validation"] {
            assert!(
                metadata.get(field).is_some(),
                "semantic metadata missing `{field}`: {row:#?}"
            );
        }
    }

    fn debug_report_from_kernel_run() -> Value {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(temp.path().join("src")).expect("create src directory");
        fs::write(
            temp.path().join(".polint.toml"),
            r#"
[workspace]
include = ["src/**"]
exclude = []
"#,
        )
        .expect("write config");
        fs::write(
            temp.path().join("src/tokens.ts"),
            r#"export const token = "ok";"#,
        )
        .expect("write tokens");
        fs::write(
            temp.path().join("src/app.ts"),
            r#"import { token as importedToken } from "./tokens";

export function answer() {
  return importedToken;
}

export const value = answer();
"#,
        )
        .expect("write app");

        let loaded = load_config(temp.path()).expect("load config");
        let cache = Cache::new("", false);
        let plan =
            AnalysisPlan::from_capability_names_for_test(&["imports", "symbols", "references"]);
        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "semantic-debug-config",
            rule_digest: "semantic-debug-rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel run");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.rule_id != "polint/internal"),
            "clean semantic debug fixture should not emit internal diagnostics: {:#?}",
            output.diagnostics
        );

        AnalysisKernel::metadata_debug_json_for_test(&output.db)
    }
}

mod semantic_mir_debug_json {
    use super::super::AnalysisKernel;
    use crate::analysis::ids::{MirBodyId, MirOpId, PlaceId, UnsupportedId};
    use crate::analysis::mir::body::{MirBody, MirOutput, MirStatus};
    use crate::analysis::mir::op::{
        AssignMode, ConservativeAction, MirOperation, MirOperationKind, MirValue,
        UnsupportedDomain, UnsupportedPrecision, UnsupportedSemanticFact,
    };
    use crate::analysis::places::{PlaceFact, PlaceProjection, PlaceRoot, PlaceStatus};
    use crate::core::{AnalysisDb, FileId, FunctionFact, FunctionId, Language, Span};
    use std::path::PathBuf;

    #[test]
    fn metadata_debug_json_contains_deterministic_semantic_mir_rows() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "export function app(value: number) { return value + 1; }\n".to_string(),
        );
        db.push_function(FunctionFact {
            id: FunctionId(0),
            file,
            name: "app".to_string(),
            span: span(file, 0, 54),
            language: Language::TypeScript,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        db.replace_semantic_mir(MirOutput {
            bodies: vec![MirBody {
                id: MirBodyId(0),
                language: Language::TypeScript,
                file,
                function: FunctionId(0),
                package: None,
                module: None,
                owner_stable_key: "function:app".to_string(),
                span: span(file, 0, 54),
                stable_key: "body:app".to_string(),
                status: MirStatus::Partial,
            }],
            places: vec![PlaceFact {
                id: PlaceId(0),
                language: Language::TypeScript,
                file: Some(file),
                function: Some(FunctionId(0)),
                root: PlaceRoot::Local {
                    function: FunctionId(0),
                    name: "value".to_string(),
                },
                projections: vec![PlaceProjection::Property("count".to_string())],
                stable_key: "place:value".to_string(),
                status: PlaceStatus::Partial,
            }],
            operations: vec![MirOperation {
                id: MirOpId(0),
                body: MirBodyId(0),
                ordinal: 0,
                span: span(file, 38, 51),
                kind: MirOperationKind::Assign {
                    place: PlaceId(0),
                    value: MirValue::Literal {
                        value: "1".to_string(),
                    },
                    mode: AssignMode::Overwrite,
                },
                stable_key: "op:assign".to_string(),
                status: MirStatus::Partial,
            }],
            unsupported: vec![UnsupportedSemanticFact {
                id: UnsupportedId(0),
                body: Some(MirBodyId(0)),
                operation: Some(MirOpId(0)),
                language: Language::TypeScript,
                file,
                span: span(file, 38, 51),
                construct: "dynamic-write".to_string(),
                source_evidence: "value + 1".to_string(),
                affected_places: vec![PlaceId(0)],
                affected_domains: vec![UnsupportedDomain::Mir],
                conservative_action: ConservativeAction::HavocAffectedPlaces,
                precision: UnsupportedPrecision::Unsupported,
                status: MirStatus::Unsupported,
                stable_key: "unsupported:dynamic-write".to_string(),
            }],
        })
        .expect("store semantic MIR rows");

        let report = AnalysisKernel::metadata_debug_json_for_test(&db);
        let mir = report["mir"]
            .as_object()
            .unwrap_or_else(|| panic!("missing semantic MIR debug object: {report:#?}"));

        for key in ["bodies", "operations", "places", "unsupported"] {
            let rows = mir
                .get(key)
                .and_then(serde_json::Value::as_array)
                .unwrap_or_else(|| panic!("missing mir.{key} rows: {mir:#?}"));
            assert_eq!(rows.len(), 1, "unexpected mir.{key} row count: {mir:#?}");
        }
        assert_eq!(mir["bodies"][0]["path"], "src/app.ts");
        assert_eq!(mir["operations"][0]["operation_kind"], "assign:overwrite");
        assert_eq!(mir["places"][0]["place_root"], "local:value");
        assert_eq!(
            mir["unsupported"][0]["conservative_action"],
            "havoc_affected_places"
        );
        assert!(
            !serde_json::to_string(&report)
                .expect("serialize report")
                .contains(env!("CARGO_MANIFEST_DIR")),
            "debug JSON should not leak absolute paths: {report:#?}"
        );
    }

    fn span(file: FileId, start_byte: u32, end_byte: u32) -> Span {
        Span {
            file,
            start_byte,
            end_byte,
            start_line: 1,
            start_col: start_byte + 1,
            end_line: 1,
            end_col: end_byte + 1,
        }
    }
}

mod tests {
    use super::super::{AnalysisKernel, KernelInput};
    use crate::analysis_plan::AnalysisPlan;
    use crate::cache::Cache;
    use crate::config::load_config;
    use crate::core::AnalysisDb;
    use serde_json::Value;
    use std::fs;
    use std::path::{Path, PathBuf};

    struct DebugFixture {
        temp: tempfile::TempDir,
        db: AnalysisDb,
    }

    fn debug_fixture() -> DebugFixture {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(temp.path().join("src")).expect("create src directory");
        fs::write(
            temp.path().join(".polint.toml"),
            r#"
[workspace]
include = ["src/**"]
exclude = []
"#,
        )
        .expect("write config");
        fs::write(
            temp.path().join("src/tokens.ts"),
            r#"export const token = "ok";"#,
        )
        .expect("write tokens");
        fs::write(
            temp.path().join("src/app.ts"),
            r#"import { token as importedToken } from "./tokens";

export function answer() {
  return importedToken;
}

export const value = answer();
"#,
        )
        .expect("write app");

        let loaded = load_config(temp.path()).expect("load config");
        let cache = Cache::new("", false);
        let plan =
            AnalysisPlan::from_capability_names_for_test(&["imports", "symbols", "references"]);
        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "metadata-debug-config",
            rule_digest: "metadata-debug-rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel run");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.rule_id != "polint/internal"),
            "clean debug fixture should not emit internal diagnostics: {:#?}",
            output.diagnostics
        );

        DebugFixture {
            temp,
            db: output.db,
        }
    }

    fn debug_report_from_kernel_run() -> (tempfile::TempDir, Value) {
        let fixture = debug_fixture();
        let report = AnalysisKernel::metadata_debug_json_for_test(&fixture.db);
        (fixture.temp, report)
    }

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("polint crate should live under crates/")
            .to_path_buf()
    }

    fn read_source_tree(path: &Path) -> String {
        let mut files = Vec::new();
        collect_files(path, &mut files);
        files.sort();

        files
            .into_iter()
            .map(|file| fs::read_to_string(&file).expect("read public source file"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn collect_files(path: &Path, files: &mut Vec<PathBuf>) {
        if path.is_file() {
            files.push(path.to_path_buf());
            return;
        }

        let mut entries = fs::read_dir(path)
            .expect("read source directory")
            .map(|entry| entry.expect("read source entry").path())
            .collect::<Vec<_>>();
        entries.sort();

        for entry in entries {
            collect_files(&entry, files);
        }
    }

    #[test]
    fn metadata_debug_json_contains_files_imports_symbols_and_references() {
        let (_temp, report) = debug_report_from_kernel_run();

        for key in ["files", "imports", "symbols", "references"] {
            let rows = report[key]
                .as_array()
                .unwrap_or_else(|| panic!("missing debug array `{key}`: {report:#?}"));
            assert!(!rows.is_empty(), "debug array `{key}` should not be empty");
        }
    }

    #[test]
    fn metadata_debug_json_rows_include_required_metadata_fields() {
        let (_temp, report) = debug_report_from_kernel_run();

        for key in ["files", "imports", "symbols", "references"] {
            for row in report[key]
                .as_array()
                .unwrap_or_else(|| panic!("missing debug array `{key}`: {report:#?}"))
            {
                for field in [
                    "family",
                    "run_id",
                    "stable_key",
                    "producer_id",
                    "layer_id",
                    "precision",
                    "confidence",
                    "validation",
                ] {
                    assert!(
                        row.get(field).is_some(),
                        "debug row in `{key}` missing `{field}`: {row:#?}"
                    );
                }
            }
        }
    }

    #[test]
    fn metadata_debug_json_serializes_byte_identically_for_same_database() {
        let fixture = debug_fixture();
        let first = AnalysisKernel::metadata_debug_json_for_test(&fixture.db);
        let second = AnalysisKernel::metadata_debug_json_for_test(&fixture.db);

        let first_json = serde_json::to_string_pretty(&first).expect("serialize first");
        let second_json = serde_json::to_string_pretty(&second).expect("serialize second");

        assert_eq!(first_json, second_json);
    }

    #[test]
    fn metadata_debug_json_excludes_absolute_paths_and_transient_runtime_details() {
        let (temp, report) = debug_report_from_kernel_run();
        let rendered = serde_json::to_string_pretty(&report).expect("serialize report");
        let temp_root = temp.path().to_string_lossy();

        for forbidden in [
            temp_root.as_ref(),
            concat!("System", "Time"),
            concat!("Inst", "ant"),
            "0x",
            "timestamp",
            "created_at",
            "updated_at",
        ] {
            assert!(
                !rendered.contains(forbidden),
                "debug JSON should not contain `{forbidden}`:\n{rendered}"
            );
        }
    }

    #[test]
    fn metadata_debug_helpers_are_not_public() {
        let root = repo_root();
        let lib_source =
            fs::read_to_string(root.join("crates/polint/src/lib.rs")).expect("read lib.rs");
        assert!(lib_source.contains("pub(crate) mod analysis_kernel;"));
        assert!(!lib_source.contains("pub mod analysis_kernel;"));

        let public_sources = [
            lib_source,
            read_source_tree(&root.join("crates/polint/src/sdk")),
            read_source_tree(&root.join("crates/polint/src/runner")),
        ]
        .join("\n");

        for forbidden in [
            "FactMeta",
            "FactMetaStore",
            "metadata_debug_json_for_test",
            "producer_id",
            "layer_id",
            "confidence",
            "validation",
            "provenance metadata",
        ] {
            assert!(
                !public_sources.contains(forbidden),
                "metadata internals must stay out of public surfaces: `{forbidden}`"
            );
        }
    }
}

#![cfg(test)]

use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::calls::facts::{
    CallAlgorithm, CallCallee, CallEdgeKind, CallPrecision, CallProvenance, CallSyntaxKind,
    CallTargetStatus, UnresolvedCallReason,
};
use crate::analysis::cfg::facts::{CfgEdgeKind, CfgPrecision, CfgStatus, CfgView};
use crate::analysis::domains::facts::{
    DomainLocation, DomainPrecision, DomainSlot, DomainStatus, DomainValue,
};
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
        cfg: cfg_report(db),
        calls: calls_report(db),
        abstract_domains: abstract_domains_report(db),
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
    cfg: CfgDebugReport,
    calls: CallDebugReport,
    abstract_domains: AbstractDomainDebugReport,
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
struct CfgDebugReport {
    functions: Vec<CfgDebugRow>,
    nodes: Vec<CfgDebugRow>,
    blocks: Vec<CfgDebugRow>,
    edges: Vec<CfgDebugRow>,
    reachability: Vec<CfgDebugRow>,
    dominators: Vec<CfgDebugRow>,
    postdominators: Vec<CfgDebugRow>,
    control_dependence: Vec<CfgDebugRow>,
    unsupported: Vec<CfgDebugRow>,
}

#[derive(Serialize)]
struct CallDebugReport {
    sites: Vec<CallSiteDebugRow>,
    targets: Vec<CallTargetDebugRow>,
    unresolved: Vec<UnresolvedCallDebugRow>,
    index_counts: BTreeMap<&'static str, usize>,
    counts: CallDebugCounts,
}

#[derive(Serialize)]
struct AbstractDomainDebugReport {
    observations: Vec<AbstractDomainObservationDebugRow>,
    events: Vec<AbstractDomainEventDebugRow>,
    counts: AbstractDomainDebugCounts,
    index_counts: BTreeMap<&'static str, usize>,
}

#[derive(Default, Serialize)]
struct AbstractDomainDebugCounts {
    by_slot: BTreeMap<String, usize>,
    by_status: BTreeMap<String, usize>,
    by_precision: BTreeMap<String, usize>,
    by_reason: BTreeMap<String, usize>,
    by_provider: BTreeMap<String, usize>,
}

#[derive(Serialize)]
struct AbstractDomainMetadata {
    family: &'static str,
    stable_key: String,
    producer_id: &'static str,
    layer_id: &'static str,
    status: String,
    precision: String,
    path: Option<String>,
    span: Option<DebugSpan>,
}

#[derive(Serialize)]
struct AbstractDomainObservationDebugRow {
    #[serde(flatten)]
    metadata: AbstractDomainMetadata,
    body_stable_key: Option<String>,
    block_stable_key: Option<String>,
    operation_stable_key: Option<String>,
    place_stable_key: Option<String>,
    slot: String,
    location: String,
    value: String,
    reason: Option<String>,
}

#[derive(Serialize)]
struct AbstractDomainEventDebugRow {
    #[serde(flatten)]
    metadata: AbstractDomainMetadata,
    body_stable_key: Option<String>,
    block_stable_key: Option<String>,
    operation_stable_key: Option<String>,
    slot: Option<String>,
    reason: String,
}

#[derive(Default, Serialize)]
struct CallDebugCounts {
    by_language: BTreeMap<String, usize>,
    by_call_kind: BTreeMap<String, usize>,
    by_algorithm: BTreeMap<String, usize>,
    by_status: BTreeMap<String, usize>,
    by_unresolved_reason: BTreeMap<String, usize>,
    by_provider: BTreeMap<String, usize>,
}

#[derive(Serialize)]
struct CallDebugMetadata {
    family: &'static str,
    stable_key: String,
    producer_id: &'static str,
    layer_id: &'static str,
    status: String,
    precision: String,
    path: Option<String>,
    span: Option<DebugSpan>,
}

#[derive(Serialize)]
struct CallSiteDebugRow {
    #[serde(flatten)]
    metadata: CallDebugMetadata,
    language: String,
    kind: String,
    callee: String,
}

#[derive(Serialize)]
struct CallTargetDebugRow {
    #[serde(flatten)]
    metadata: CallDebugMetadata,
    site_stable_key: Option<String>,
    caller_stable_key: Option<String>,
    target_function_stable_key: Option<String>,
    target_symbol_stable_key: Option<String>,
    edge_kind: String,
    algorithm: String,
    reason: Option<String>,
    provenance: String,
}

#[derive(Serialize)]
struct UnresolvedCallDebugRow {
    #[serde(flatten)]
    metadata: CallDebugMetadata,
    site_stable_key: Option<String>,
    caller_stable_key: Option<String>,
    algorithm: String,
    reason: String,
    provenance: String,
}

#[derive(Serialize)]
struct CfgDebugRow {
    family: &'static str,
    run_id: u64,
    stable_key: String,
    producer_id: &'static str,
    layer_id: &'static str,
    status: &'static str,
    precision: &'static str,
    path: Option<String>,
    span: Option<DebugSpan>,
    function: Option<u64>,
    view: Option<&'static str>,
    payload: String,
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

fn cfg_report(db: &AnalysisDb) -> CfgDebugReport {
    CfgDebugReport {
        functions: cfg_function_rows(db),
        nodes: cfg_node_rows(db),
        blocks: cfg_block_rows(db),
        edges: cfg_edge_rows(db),
        reachability: cfg_reachability_rows(db),
        dominators: cfg_dominator_rows(db),
        postdominators: cfg_postdominator_rows(db),
        control_dependence: cfg_control_dependence_rows(db),
        unsupported: cfg_unsupported_rows(db),
    }
}

fn calls_report(db: &AnalysisDb) -> CallDebugReport {
    CallDebugReport {
        sites: call_site_rows(db),
        targets: call_target_rows(db),
        unresolved: unresolved_call_rows(db),
        index_counts: call_index_counts(db),
        counts: call_counts(db),
    }
}

fn abstract_domains_report(db: &AnalysisDb) -> AbstractDomainDebugReport {
    AbstractDomainDebugReport {
        observations: abstract_domain_observation_rows(db),
        events: abstract_domain_event_rows(db),
        counts: abstract_domain_counts(db),
        index_counts: abstract_domain_index_counts(db),
    }
}

fn abstract_domain_observation_rows(db: &AnalysisDb) -> Vec<AbstractDomainObservationDebugRow> {
    let mut rows = db
        .abstract_domain_observations()
        .iter()
        .filter_map(|row| {
            abstract_domain_metadata_row(
                db,
                FactFamily::DomainObservation,
                row.id.0,
                row.stable_key.as_str(),
                domain_status_label(row.status),
                domain_precision_label(row.precision),
                domain_file(db, row.body),
                domain_span(db, row.operation),
            )
            .map(|metadata| AbstractDomainObservationDebugRow {
                metadata,
                body_stable_key: stable_key_for(db, FactFamily::MirBody, row.body.0),
                block_stable_key: row.block.and_then(|block| {
                    stable_key_for(db, FactFamily::BasicBlock, block.0)
                }),
                operation_stable_key: row.operation.and_then(|operation| {
                    stable_key_for(db, FactFamily::MirOperation, operation.0)
                }),
                place_stable_key: row
                    .place
                    .and_then(|place| stable_key_for(db, FactFamily::Place, place.0)),
                slot: domain_slot_label(row.slot).to_string(),
                location: domain_location_label(row.location).to_string(),
                value: domain_value_fragment(&row.value),
                reason: domain_value_reason(&row.value).map(str::to_string),
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        abstract_domain_metadata_order(&left.metadata)
            .cmp(&abstract_domain_metadata_order(&right.metadata))
    });
    rows
}

fn abstract_domain_event_rows(db: &AnalysisDb) -> Vec<AbstractDomainEventDebugRow> {
    let mut rows = db
        .abstract_domain_events()
        .iter()
        .filter_map(|row| {
            abstract_domain_metadata_row(
                db,
                FactFamily::DomainEvent,
                row.id.0,
                row.stable_key.as_str(),
                domain_status_label(row.status),
                domain_precision_label(row.precision),
                domain_file(db, row.body),
                domain_span(db, row.operation),
            )
            .map(|metadata| AbstractDomainEventDebugRow {
                metadata,
                body_stable_key: stable_key_for(db, FactFamily::MirBody, row.body.0),
                block_stable_key: row.block.and_then(|block| {
                    stable_key_for(db, FactFamily::BasicBlock, block.0)
                }),
                operation_stable_key: row.operation.and_then(|operation| {
                    stable_key_for(db, FactFamily::MirOperation, operation.0)
                }),
                slot: row.slot.map(domain_slot_label).map(str::to_string),
                reason: row.reason.clone(),
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        abstract_domain_metadata_order(&left.metadata)
            .cmp(&abstract_domain_metadata_order(&right.metadata))
    });
    rows
}

fn abstract_domain_metadata_row(
    db: &AnalysisDb,
    family: FactFamily,
    run_id: u64,
    stable_key: &str,
    status: &'static str,
    precision: &'static str,
    file: Option<FileId>,
    span: Option<DebugSpan>,
) -> Option<AbstractDomainMetadata> {
    let meta = db.metadata_for(FactRef::new(family, run_id))?;
    Some(AbstractDomainMetadata {
        family: family.label(),
        stable_key: stable_key.to_string(),
        producer_id: meta.producer_id,
        layer_id: meta.layer_id,
        status: status.to_string(),
        precision: precision.to_string(),
        path: relative_path(db, file).map(str::to_string),
        span,
    })
}

fn abstract_domain_metadata_order(row: &AbstractDomainMetadata) -> (&str, u32, &str) {
    (
        row.path.as_deref().unwrap_or(""),
        span_start(row.span),
        row.stable_key.as_str(),
    )
}

fn abstract_domain_counts(db: &AnalysisDb) -> AbstractDomainDebugCounts {
    let mut counts = AbstractDomainDebugCounts::default();
    for row in db.abstract_domain_observations() {
        increment(&mut counts.by_slot, domain_slot_label(row.slot));
        increment(&mut counts.by_status, domain_status_label(row.status));
        increment(&mut counts.by_precision, domain_precision_label(row.precision));
        if let Some(reason) = domain_value_reason(&row.value) {
            increment(&mut counts.by_reason, reason);
        }
        if let Some(metadata) =
            db.metadata_for(FactRef::new(FactFamily::DomainObservation, row.id.0))
        {
            increment(&mut counts.by_provider, metadata.producer_id);
        }
    }
    for row in db.abstract_domain_events() {
        if let Some(slot) = row.slot {
            increment(&mut counts.by_slot, domain_slot_label(slot));
        }
        increment(&mut counts.by_status, domain_status_label(row.status));
        increment(&mut counts.by_precision, domain_precision_label(row.precision));
        increment(&mut counts.by_reason, row.reason.as_str());
        if let Some(metadata) = db.metadata_for(FactRef::new(FactFamily::DomainEvent, row.id.0)) {
            increment(&mut counts.by_provider, metadata.producer_id);
        }
    }
    counts
}

fn abstract_domain_index_counts(db: &AnalysisDb) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    let Some(store) = db.abstract_domain_store() else {
        return counts;
    };
    let bodies = db
        .abstract_domain_observations()
        .iter()
        .map(|row| row.body)
        .chain(db.abstract_domain_events().iter().map(|row| row.body))
        .collect::<BTreeSet<_>>();
    let blocks = db
        .abstract_domain_observations()
        .iter()
        .filter_map(|row| row.block)
        .collect::<BTreeSet<_>>();
    let operations = db
        .abstract_domain_observations()
        .iter()
        .filter_map(|row| row.operation)
        .collect::<BTreeSet<_>>();
    let places = db
        .abstract_domain_observations()
        .iter()
        .filter_map(|row| row.place)
        .collect::<BTreeSet<_>>();
    let slots = db
        .abstract_domain_observations()
        .iter()
        .map(|row| row.slot)
        .collect::<BTreeSet<_>>();
    let statuses = db
        .abstract_domain_observations()
        .iter()
        .map(|row| row.status)
        .chain(db.abstract_domain_events().iter().map(|row| row.status))
        .collect::<BTreeSet<_>>();

    counts.insert(
        "observations_by_body",
        bodies
            .iter()
            .filter(|body| !store.observations_by_body(**body).is_empty())
            .count(),
    );
    counts.insert(
        "observations_by_block",
        blocks
            .iter()
            .filter(|block| !store.observations_by_block(**block).is_empty())
            .count(),
    );
    counts.insert(
        "observations_by_operation",
        operations
            .iter()
            .filter(|operation| !store.observations_by_operation(**operation).is_empty())
            .count(),
    );
    counts.insert(
        "observations_by_place",
        places
            .iter()
            .filter(|place| !store.observations_by_place(**place).is_empty())
            .count(),
    );
    counts.insert(
        "observations_by_slot",
        slots
            .iter()
            .filter(|slot| !store.observations_by_slot(**slot).is_empty())
            .count(),
    );
    counts.insert(
        "observations_by_status",
        statuses
            .iter()
            .filter(|status| !store.observations_by_status(**status).is_empty())
            .count(),
    );
    counts.insert(
        "events_by_status",
        statuses
            .iter()
            .filter(|status| !store.events_by_status(**status).is_empty())
            .count(),
    );
    counts
}

fn domain_file(db: &AnalysisDb, body: crate::analysis::ids::MirBodyId) -> Option<FileId> {
    db.mir_bodies()
        .iter()
        .find(|row| row.id == body)
        .map(|row| row.file)
}

fn domain_span(db: &AnalysisDb, operation: Option<crate::analysis::ids::MirOpId>) -> Option<DebugSpan> {
    operation.and_then(|operation| {
        db.mir_operations()
            .iter()
            .find(|row| row.id == operation)
            .map(|row| debug_span(&row.span))
    })
}

fn call_site_rows(db: &AnalysisDb) -> Vec<CallSiteDebugRow> {
    let mut rows = db
        .call_sites()
        .iter()
        .filter_map(|row| {
            call_metadata_row(
                db,
                FactFamily::CallSite,
                row.id.0,
                row.stable_key.as_str(),
                call_status_label(row.status),
                call_precision_label(row.precision),
                Some(row.file),
                Some(debug_span(&row.span)),
            )
            .map(|metadata| CallSiteDebugRow {
                metadata,
                language: call_language_label(row.language).to_string(),
                kind: call_syntax_kind_label(row.kind).to_string(),
                callee: call_callee_label(&row.callee),
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| call_metadata_order(&left.metadata).cmp(&call_metadata_order(&right.metadata)));
    rows
}

fn call_target_rows(db: &AnalysisDb) -> Vec<CallTargetDebugRow> {
    let site_keys = db
        .call_sites()
        .iter()
        .map(|site| (site.id, site.stable_key.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut rows = db
        .call_targets()
        .iter()
        .filter_map(|row| {
            let site = db.call_sites().iter().find(|site| site.id == row.site);
            call_metadata_row(
                db,
                FactFamily::CallTarget,
                row.id.0,
                row.stable_key.as_str(),
                call_status_label(row.status),
                call_precision_label(row.precision),
                site.map(|site| site.file),
                site.map(|site| debug_span(&site.span)),
            )
            .map(|metadata| CallTargetDebugRow {
                metadata,
                site_stable_key: site_keys.get(&row.site).cloned(),
                caller_stable_key: stable_key_for(db, FactFamily::Function, row.caller.0),
                target_function_stable_key: row
                    .target_function
                    .and_then(|function| stable_key_for(db, FactFamily::Function, function.0)),
                target_symbol_stable_key: row
                    .target_symbol
                    .and_then(|symbol| stable_key_for(db, FactFamily::Symbol, symbol.0)),
                edge_kind: call_edge_kind_label(row.edge_kind).to_string(),
                algorithm: call_algorithm_label(row.algorithm).to_string(),
                reason: row.reason.map(call_unresolved_reason_label).map(str::to_string),
                provenance: call_provenance_label(row.provenance).to_string(),
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| call_metadata_order(&left.metadata).cmp(&call_metadata_order(&right.metadata)));
    rows
}

fn unresolved_call_rows(db: &AnalysisDb) -> Vec<UnresolvedCallDebugRow> {
    let site_keys = db
        .call_sites()
        .iter()
        .map(|site| (site.id, site.stable_key.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut rows = db
        .unresolved_calls()
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            let site = db.call_sites().iter().find(|site| site.id == row.site);
            call_metadata_row(
                db,
                FactFamily::UnresolvedCall,
                index as u64,
                row.stable_key.as_str(),
                call_status_label(row.status),
                call_precision_label(row.precision),
                site.map(|site| site.file),
                site.map(|site| debug_span(&site.span)),
            )
            .map(|metadata| UnresolvedCallDebugRow {
                metadata,
                site_stable_key: site_keys.get(&row.site).cloned(),
                caller_stable_key: stable_key_for(db, FactFamily::Function, row.caller.0),
                algorithm: call_algorithm_label(row.algorithm).to_string(),
                reason: call_unresolved_reason_label(row.reason).to_string(),
                provenance: call_provenance_label(row.provenance).to_string(),
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| call_metadata_order(&left.metadata).cmp(&call_metadata_order(&right.metadata)));
    rows
}

fn call_metadata_row(
    db: &AnalysisDb,
    family: FactFamily,
    run_id: u64,
    stable_key: &str,
    status: &'static str,
    precision: &'static str,
    file: Option<FileId>,
    span: Option<DebugSpan>,
) -> Option<CallDebugMetadata> {
    let meta = db.metadata_for(FactRef::new(family, run_id))?;
    Some(CallDebugMetadata {
        family: family.label(),
        stable_key: stable_key.to_string(),
        producer_id: meta.producer_id,
        layer_id: meta.layer_id,
        status: status.to_string(),
        precision: precision.to_string(),
        path: relative_path(db, file).map(str::to_string),
        span,
    })
}

fn call_metadata_order(row: &CallDebugMetadata) -> (&str, u32, &str) {
    (
        row.path.as_deref().unwrap_or(""),
        span_start(row.span),
        row.stable_key.as_str(),
    )
}

fn stable_key_for(db: &AnalysisDb, family: FactFamily, run_id: u64) -> Option<String> {
    db.metadata_for(FactRef::new(family, run_id))
        .map(|metadata| metadata.stable_key.clone())
}

fn call_index_counts(db: &AnalysisDb) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    let caller_functions = db
        .call_sites()
        .iter()
        .map(|site| site.caller)
        .collect::<BTreeSet<_>>();
    let sites = db
        .call_sites()
        .iter()
        .map(|site| site.id)
        .collect::<BTreeSet<_>>();
    let outgoing_functions = db
        .call_targets()
        .iter()
        .map(|target| target.caller)
        .collect::<BTreeSet<_>>();
    let outgoing_symbols = db
        .call_sites()
        .iter()
        .filter_map(|site| site.owner_symbol)
        .collect::<BTreeSet<_>>();
    let incoming_symbols = db
        .call_targets()
        .iter()
        .filter_map(|target| target.target_symbol)
        .collect::<BTreeSet<_>>();
    let incoming_functions = db
        .call_targets()
        .iter()
        .filter_map(|target| target.target_function)
        .collect::<BTreeSet<_>>();
    let unresolved_reasons = db
        .unresolved_calls()
        .iter()
        .map(|row| row.reason)
        .collect::<BTreeSet<_>>();
    let unresolved_statuses = db
        .unresolved_calls()
        .iter()
        .map(|row| row.status)
        .collect::<BTreeSet<_>>();

    counts.insert(
        "sites_by_caller",
        caller_functions
            .iter()
            .filter(|caller| !db.call_sites_by_caller(**caller).is_empty())
            .count(),
    );
    counts.insert(
        "targets_by_site",
        sites.iter()
            .filter(|site| !db.call_targets_by_site(**site).is_empty())
            .count(),
    );
    counts.insert(
        "outgoing_by_function",
        outgoing_functions
            .iter()
            .filter(|function| !db.outgoing_calls_by_function(**function).is_empty())
            .count(),
    );
    counts.insert(
        "outgoing_by_symbol",
        outgoing_symbols
            .iter()
            .filter(|symbol| !db.outgoing_calls_by_symbol(**symbol).is_empty())
            .count(),
    );
    counts.insert(
        "incoming_by_symbol",
        incoming_symbols
            .iter()
            .filter(|symbol| !db.incoming_calls_by_symbol(**symbol).is_empty())
            .count(),
    );
    counts.insert(
        "incoming_by_function",
        incoming_functions
            .iter()
            .filter(|function| !db.incoming_calls_by_function(**function).is_empty())
            .count(),
    );
    counts.insert(
        "unresolved_by_reason",
        unresolved_reasons
            .iter()
            .filter(|reason| !db.unresolved_calls_by_reason(**reason).is_empty())
            .count(),
    );
    counts.insert(
        "unresolved_by_status",
        unresolved_statuses
            .iter()
            .filter(|status| !db.unresolved_calls_by_status(**status).is_empty())
            .count(),
    );
    counts
}

fn call_counts(db: &AnalysisDb) -> CallDebugCounts {
    let mut counts = CallDebugCounts::default();
    for site in db.call_sites() {
        increment(&mut counts.by_language, call_language_label(site.language));
        increment(&mut counts.by_call_kind, call_syntax_kind_label(site.kind));
        increment(&mut counts.by_status, call_status_label(site.status));
        if let Some(metadata) = db.metadata_for(FactRef::new(FactFamily::CallSite, site.id.0)) {
            increment(&mut counts.by_provider, metadata.producer_id);
        }
    }
    for target in db.call_targets() {
        increment(&mut counts.by_algorithm, call_algorithm_label(target.algorithm));
        increment(&mut counts.by_status, call_status_label(target.status));
        if let Some(reason) = target.reason {
            increment(&mut counts.by_unresolved_reason, call_unresolved_reason_label(reason));
        }
        if let Some(metadata) = db.metadata_for(FactRef::new(FactFamily::CallTarget, target.id.0)) {
            increment(&mut counts.by_provider, metadata.producer_id);
        }
    }
    for (index, unresolved) in db.unresolved_calls().iter().enumerate() {
        increment(&mut counts.by_algorithm, call_algorithm_label(unresolved.algorithm));
        increment(&mut counts.by_status, call_status_label(unresolved.status));
        increment(
            &mut counts.by_unresolved_reason,
            call_unresolved_reason_label(unresolved.reason),
        );
        if let Some(metadata) =
            db.metadata_for(FactRef::new(FactFamily::UnresolvedCall, index as u64))
        {
            increment(&mut counts.by_provider, metadata.producer_id);
        }
    }
    counts
}

fn increment(counts: &mut BTreeMap<String, usize>, key: &str) {
    *counts.entry(key.to_string()).or_default() += 1;
}

fn cfg_function_rows(db: &AnalysisDb) -> Vec<CfgDebugRow> {
    let mut rows = db
        .cfg_functions()
        .iter()
        .filter_map(|row| {
            cfg_metadata_row(db, FactFamily::CfgFunction, row.id.0).map(|metadata| CfgDebugRow {
                family: FactFamily::CfgFunction.label(),
                run_id: row.id.0,
                stable_key: row.stable_key.clone(),
                producer_id: metadata.producer_id,
                layer_id: metadata.layer_id,
                status: cfg_status_label(row.status),
                precision: cfg_precision_label(row.precision),
                path: relative_path(db, Some(row.file)).map(str::to_string),
                span: Some(debug_span(&row.span)),
                function: Some(row.function.0),
                view: None,
                payload: format!("entry={};normal_exit={}", row.entry_node.0, row.normal_exit_node.0),
            })
        })
        .collect::<Vec<_>>();
    sort_cfg_rows(&mut rows);
    rows
}

fn cfg_node_rows(db: &AnalysisDb) -> Vec<CfgDebugRow> {
    let mut rows = db
        .cfg_nodes()
        .iter()
        .filter_map(|row| {
            cfg_metadata_row(db, FactFamily::CfgNode, row.id.0).map(|metadata| CfgDebugRow {
                family: FactFamily::CfgNode.label(),
                run_id: row.id.0,
                stable_key: row.stable_key.clone(),
                producer_id: metadata.producer_id,
                layer_id: metadata.layer_id,
                status: cfg_status_label(row.status),
                precision: cfg_precision_label(row.precision),
                path: None,
                span: row.span.as_ref().map(debug_span),
                function: Some(row.cfg_function.0),
                view: None,
                payload: format!("kind={:?};block={};operation={:?}", row.kind, row.block.0, row.operation),
            })
        })
        .collect::<Vec<_>>();
    sort_cfg_rows(&mut rows);
    rows
}

fn cfg_block_rows(db: &AnalysisDb) -> Vec<CfgDebugRow> {
    let mut rows = db
        .cfg_blocks()
        .iter()
        .filter_map(|row| {
            cfg_metadata_row(db, FactFamily::BasicBlock, row.id.0).map(|metadata| CfgDebugRow {
                family: FactFamily::BasicBlock.label(),
                run_id: row.id.0,
                stable_key: row.stable_key.clone(),
                producer_id: metadata.producer_id,
                layer_id: metadata.layer_id,
                status: cfg_status_label(row.status),
                precision: cfg_precision_label(row.precision),
                path: None,
                span: None,
                function: Some(row.cfg_function.0),
                view: None,
                payload: format!("kind={:?};reachable={};rpo={}", row.kind, row.reachable, row.reverse_postorder),
            })
        })
        .collect::<Vec<_>>();
    sort_cfg_rows(&mut rows);
    rows
}

fn cfg_edge_rows(db: &AnalysisDb) -> Vec<CfgDebugRow> {
    let mut rows = db
        .cfg_edges()
        .iter()
        .filter_map(|row| {
            cfg_metadata_row(db, FactFamily::CfgEdge, row.id.0).map(|metadata| CfgDebugRow {
                family: FactFamily::CfgEdge.label(),
                run_id: row.id.0,
                stable_key: row.stable_key.clone(),
                producer_id: metadata.producer_id,
                layer_id: metadata.layer_id,
                status: cfg_status_label(row.status),
                precision: cfg_precision_label(row.precision),
                path: None,
                span: None,
                function: Some(row.cfg_function.0),
                view: Some(cfg_view_label(row.view)),
                payload: format!(
                    "kind={};from_block={};to_block={};from_node={};to_node={}",
                    cfg_edge_kind_label(row.kind), row.from_block.0, row.to_block.0, row.from.0, row.to.0
                ),
            })
        })
        .collect::<Vec<_>>();
    sort_cfg_rows(&mut rows);
    rows
}

fn cfg_reachability_rows(db: &AnalysisDb) -> Vec<CfgDebugRow> {
    let mut rows = db.cfg_reachability().iter().filter_map(|row| {
        cfg_metadata_row(db, FactFamily::CfgReachability, row.id.0).map(|metadata| CfgDebugRow {
            family: FactFamily::CfgReachability.label(),
            run_id: row.id.0,
            stable_key: row.stable_key.clone(),
            producer_id: metadata.producer_id,
            layer_id: metadata.layer_id,
            status: cfg_status_label(row.status),
            precision: cfg_precision_label(row.precision),
            path: None,
            span: None,
            function: Some(row.cfg_function.0),
            view: Some(cfg_view_label(row.view)),
            payload: format!("block={};reachable={}", row.block.0, row.reachable),
        })
    }).collect::<Vec<_>>();
    sort_cfg_rows(&mut rows);
    rows
}

fn cfg_dominator_rows(db: &AnalysisDb) -> Vec<CfgDebugRow> {
    let mut rows = db.cfg_dominators().iter().filter_map(|row| {
        cfg_metadata_row(db, FactFamily::CfgDominator, row.id.0).map(|metadata| CfgDebugRow {
            family: FactFamily::CfgDominator.label(),
            run_id: row.id.0,
            stable_key: row.stable_key.clone(),
            producer_id: metadata.producer_id,
            layer_id: metadata.layer_id,
            status: cfg_status_label(row.status),
            precision: cfg_precision_label(row.precision),
            path: None,
            span: None,
            function: Some(row.cfg_function.0),
            view: Some(cfg_view_label(row.view)),
            payload: format!("dominator={};dominated={};immediate={}", row.dominator.0, row.dominated.0, row.immediate),
        })
    }).collect::<Vec<_>>();
    sort_cfg_rows(&mut rows);
    rows
}

fn cfg_postdominator_rows(db: &AnalysisDb) -> Vec<CfgDebugRow> {
    let mut rows = db.cfg_postdominators().iter().filter_map(|row| {
        cfg_metadata_row(db, FactFamily::CfgPostDominator, row.id.0).map(|metadata| CfgDebugRow {
            family: FactFamily::CfgPostDominator.label(),
            run_id: row.id.0,
            stable_key: row.stable_key.clone(),
            producer_id: metadata.producer_id,
            layer_id: metadata.layer_id,
            status: cfg_status_label(row.status),
            precision: cfg_precision_label(row.precision),
            path: None,
            span: None,
            function: Some(row.cfg_function.0),
            view: Some(cfg_view_label(row.view)),
            payload: format!("postdominator={};postdominated={};immediate={}", row.postdominator.0, row.postdominated.0, row.immediate),
        })
    }).collect::<Vec<_>>();
    sort_cfg_rows(&mut rows);
    rows
}

fn cfg_control_dependence_rows(db: &AnalysisDb) -> Vec<CfgDebugRow> {
    let mut rows = db.cfg_control_dependence().iter().filter_map(|row| {
        cfg_metadata_row(db, FactFamily::CfgControlDependence, row.id.0).map(|metadata| CfgDebugRow {
            family: FactFamily::CfgControlDependence.label(),
            run_id: row.id.0,
            stable_key: row.stable_key.clone(),
            producer_id: metadata.producer_id,
            layer_id: metadata.layer_id,
            status: cfg_status_label(row.status),
            precision: cfg_precision_label(row.precision),
            path: None,
            span: None,
            function: Some(row.cfg_function.0),
            view: Some(cfg_view_label(row.view)),
            payload: format!("controlling_edge={};edge_kind={};controlled_block={}", row.controlling_edge.0, cfg_edge_kind_label(row.controlling_edge_kind), row.controlled_block.0),
        })
    }).collect::<Vec<_>>();
    sort_cfg_rows(&mut rows);
    rows
}

fn cfg_unsupported_rows(db: &AnalysisDb) -> Vec<CfgDebugRow> {
    let mut rows = db.unsupported_control_flow().iter().filter_map(|row| {
        cfg_metadata_row(db, FactFamily::UnsupportedControlFlow, row.id.0).map(|metadata| CfgDebugRow {
            family: FactFamily::UnsupportedControlFlow.label(),
            run_id: row.id.0,
            stable_key: row.stable_key.clone(),
            producer_id: metadata.producer_id,
            layer_id: metadata.layer_id,
            status: cfg_status_label(row.status),
            precision: cfg_precision_label(row.precision),
            path: relative_path(db, Some(row.file)).map(str::to_string),
            span: Some(debug_span(&row.span)),
            function: row.cfg_function.map(|function| function.0),
            view: None,
            payload: format!("construct={};action={:?}", row.construct, row.conservative_action),
        })
    }).collect::<Vec<_>>();
    sort_cfg_rows(&mut rows);
    rows
}

fn cfg_metadata_row(db: &AnalysisDb, family: FactFamily, run_id: u64) -> Option<&FactMeta> {
    db.metadata_for(FactRef::new(family, run_id))
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

fn sort_cfg_rows(rows: &mut [CfgDebugRow]) {
    rows.sort_by(|left, right| {
        (
            left.path.as_deref().unwrap_or(""),
            span_start(left.span),
            left.function,
            left.view.unwrap_or(""),
            left.stable_key.as_str(),
            left.run_id,
        )
            .cmp(&(
                right.path.as_deref().unwrap_or(""),
                span_start(right.span),
                right.function,
                right.view.unwrap_or(""),
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

fn cfg_status_label(status: CfgStatus) -> &'static str {
    match status {
        CfgStatus::Resolved => "resolved",
        CfgStatus::Partial => "partial",
        CfgStatus::Unknown => "unknown",
        CfgStatus::Unsupported => "unsupported",
    }
}

fn cfg_precision_label(precision: CfgPrecision) -> &'static str {
    match precision {
        CfgPrecision::ExactSyntax => "exact_syntax",
        CfgPrecision::ExactLowered => "exact_lowered",
        CfgPrecision::SetupAware => "setup_aware",
        CfgPrecision::Conservative => "conservative",
        CfgPrecision::Heuristic => "heuristic",
        CfgPrecision::Unknown => "unknown",
        CfgPrecision::Unsupported => "unsupported",
    }
}

fn cfg_view_label(view: CfgView) -> &'static str {
    match view {
        CfgView::NormalControl => "normal_control",
        CfgView::AbruptAware => "abrupt_aware",
        CfgView::ExceptionConservative => "exception_conservative",
    }
}

fn cfg_edge_kind_label(kind: CfgEdgeKind) -> &'static str {
    match kind {
        CfgEdgeKind::Normal => "normal",
        CfgEdgeKind::True => "true",
        CfgEdgeKind::False => "false",
        CfgEdgeKind::SwitchCase => "switch_case",
        CfgEdgeKind::DefaultCase => "default_case",
        CfgEdgeKind::LoopEnter => "loop_enter",
        CfgEdgeKind::LoopBack => "loop_back",
        CfgEdgeKind::LoopExit => "loop_exit",
        CfgEdgeKind::Break => "break",
        CfgEdgeKind::Continue => "continue",
        CfgEdgeKind::Goto => "goto",
        CfgEdgeKind::Return => "return",
        CfgEdgeKind::Throw => "throw",
        CfgEdgeKind::ImplicitThrow => "implicit_throw",
        CfgEdgeKind::Panic => "panic",
        CfgEdgeKind::Recover => "recover",
        CfgEdgeKind::Finally => "finally",
        CfgEdgeKind::Cleanup => "cleanup",
        CfgEdgeKind::Defer => "defer",
        CfgEdgeKind::ShortCircuit => "short_circuit",
        CfgEdgeKind::OptionalChain => "optional_chain",
        CfgEdgeKind::Nullish => "nullish",
        CfgEdgeKind::YieldSuspend => "yield_suspend",
        CfgEdgeKind::YieldResume => "yield_resume",
        CfgEdgeKind::AwaitSuspend => "await_suspend",
        CfgEdgeKind::AwaitResume => "await_resume",
        CfgEdgeKind::Spawn => "spawn",
        CfgEdgeKind::Unreachable => "unreachable",
        CfgEdgeKind::Unknown => "unknown",
        CfgEdgeKind::Synthetic => "synthetic",
        CfgEdgeKind::Extension => "extension",
    }
}

fn call_language_label(language: Language) -> &'static str {
    match language {
        Language::Go => "Go",
        Language::TypeScript => "TypeScript",
        Language::Tsx => "Tsx",
        Language::JavaScript => "JavaScript",
        Language::Jsx => "Jsx",
        Language::Unknown => "Unknown",
    }
}

fn call_status_label(status: CallTargetStatus) -> &'static str {
    match status {
        CallTargetStatus::Resolved => "Resolved",
        CallTargetStatus::Ambiguous => "Ambiguous",
        CallTargetStatus::Unresolved => "Unresolved",
        CallTargetStatus::Unsupported => "Unsupported",
        CallTargetStatus::SetupMissing => "SetupMissing",
        CallTargetStatus::BudgetExceeded => "BudgetExceeded",
        CallTargetStatus::Rejected => "Rejected",
    }
}

fn call_precision_label(precision: CallPrecision) -> &'static str {
    match precision {
        CallPrecision::Exact => "Exact",
        CallPrecision::SetupAware => "SetupAware",
        CallPrecision::Conservative => "Conservative",
        CallPrecision::Heuristic => "Heuristic",
        CallPrecision::Ambiguous => "Ambiguous",
        CallPrecision::Unknown => "Unknown",
        CallPrecision::Unsupported => "Unsupported",
    }
}

fn domain_slot_label(slot: DomainSlot) -> &'static str {
    match slot {
        DomainSlot::Reachability => "reachability",
        DomainSlot::Nilness => "nilness",
        DomainSlot::Truthiness => "truthiness",
        DomainSlot::Constants => "constants",
        DomainSlot::Strings => "strings",
        DomainSlot::Initializedness => "initializedness",
    }
}

fn domain_location_label(location: DomainLocation) -> &'static str {
    match location {
        DomainLocation::FunctionEntry => "function_entry",
        DomainLocation::BlockEntry => "block_entry",
        DomainLocation::BeforeOperation => "before_operation",
        DomainLocation::AfterOperation => "after_operation",
        DomainLocation::BlockExit => "block_exit",
    }
}

fn domain_status_label(status: DomainStatus) -> &'static str {
    match status {
        DomainStatus::Present => "present",
        DomainStatus::Top => "top",
        DomainStatus::Unknown => "unknown",
        DomainStatus::Unsupported => "unsupported",
        DomainStatus::SetupMissing => "setup_missing",
        DomainStatus::BudgetExceeded => "budget_exceeded",
    }
}

fn domain_precision_label(precision: DomainPrecision) -> &'static str {
    match precision {
        DomainPrecision::ExactLocal => "exact_local",
        DomainPrecision::SetupAware => "setup_aware",
        DomainPrecision::Conservative => "conservative",
        DomainPrecision::Heuristic => "heuristic",
        DomainPrecision::Unknown => "unknown",
        DomainPrecision::Unsupported => "unsupported",
    }
}

fn domain_value_reason(value: &DomainValue) -> Option<&str> {
    match value {
        DomainValue::TopReason(reason) => Some(reason.as_str()),
        DomainValue::Label(_) | DomainValue::DigestParts(_) => None,
    }
}

fn domain_value_fragment(value: &DomainValue) -> String {
    match value {
        DomainValue::Label(value) => format!("label={value}"),
        DomainValue::DigestParts(parts) => parts.join(";"),
        DomainValue::TopReason(reason) => format!("top_reason={reason}"),
    }
}

fn call_syntax_kind_label(kind: CallSyntaxKind) -> &'static str {
    match kind {
        CallSyntaxKind::Function => "Function",
        CallSyntaxKind::Method => "Method",
        CallSyntaxKind::Constructor => "Constructor",
        CallSyntaxKind::StaticMember => "StaticMember",
        CallSyntaxKind::Member => "Member",
        CallSyntaxKind::Index => "Index",
        CallSyntaxKind::Super => "Super",
        CallSyntaxKind::Import => "Import",
        CallSyntaxKind::New => "New",
        CallSyntaxKind::TaggedTemplate => "TaggedTemplate",
        CallSyntaxKind::GoRoutine => "GoRoutine",
        CallSyntaxKind::Deferred => "Deferred",
        CallSyntaxKind::DynamicImport => "DynamicImport",
        CallSyntaxKind::Require => "Require",
        CallSyntaxKind::FunctionValue => "FunctionValue",
        CallSyntaxKind::Unknown => "Unknown",
    }
}

fn call_edge_kind_label(kind: CallEdgeKind) -> &'static str {
    match kind {
        CallEdgeKind::Direct => "Direct",
        CallEdgeKind::Constructor => "Constructor",
        CallEdgeKind::StaticMember => "StaticMember",
        CallEdgeKind::MethodDirect => "MethodDirect",
        CallEdgeKind::Method => "Method",
        CallEdgeKind::FunctionValue => "FunctionValue",
        CallEdgeKind::Synthetic => "Synthetic",
        CallEdgeKind::Spawn => "Spawn",
        CallEdgeKind::Deferred => "Deferred",
        CallEdgeKind::Unknown => "Unknown",
    }
}

fn call_algorithm_label(algorithm: CallAlgorithm) -> &'static str {
    match algorithm {
        CallAlgorithm::SyntaxOnly => "SyntaxOnly",
        CallAlgorithm::DirectReference => "DirectReference",
        CallAlgorithm::ImportBinding => "ImportBinding",
        CallAlgorithm::ConstructorBinding => "ConstructorBinding",
        CallAlgorithm::StaticMember => "StaticMember",
        CallAlgorithm::DirectMember => "DirectMember",
        CallAlgorithm::GoStatic => "GoStatic",
        CallAlgorithm::GoCha => "GoCha",
        CallAlgorithm::GoRta => "GoRta",
        CallAlgorithm::GoVta => "GoVta",
        CallAlgorithm::FunctionTokenFlow => "FunctionTokenFlow",
        CallAlgorithm::TypeHierarchy => "TypeHierarchy",
        CallAlgorithm::PointsTo => "PointsTo",
        CallAlgorithm::SummaryAssisted => "SummaryAssisted",
        CallAlgorithm::FrameworkModel => "FrameworkModel",
        CallAlgorithm::RepoModel => "RepoModel",
        CallAlgorithm::Unsupported => "Unsupported",
    }
}

fn call_unresolved_reason_label(reason: UnresolvedCallReason) -> &'static str {
    match reason {
        UnresolvedCallReason::FunctionValue => "FunctionValue",
        UnresolvedCallReason::DynamicProperty => "DynamicProperty",
        UnresolvedCallReason::InterfaceDispatch => "InterfaceDispatch",
        UnresolvedCallReason::Eval => "Eval",
        UnresolvedCallReason::CallApplyBind => "CallApplyBind",
        UnresolvedCallReason::FrameworkDispatch => "FrameworkDispatch",
        UnresolvedCallReason::Reflection => "Reflection",
        UnresolvedCallReason::GoroutineBoundary => "GoroutineBoundary",
        UnresolvedCallReason::DynamicImport => "DynamicImport",
        UnresolvedCallReason::ProxyOrAccessor => "ProxyOrAccessor",
        UnresolvedCallReason::MissingSemanticReference => "MissingSemanticReference",
        UnresolvedCallReason::MissingImportResolution => "MissingImportResolution",
        UnresolvedCallReason::SetupMissing => "SetupMissing",
        UnresolvedCallReason::UnsupportedSyntax => "UnsupportedSyntax",
        UnresolvedCallReason::BudgetExceeded => "BudgetExceeded",
        UnresolvedCallReason::UnknownCallee => "UnknownCallee",
        UnresolvedCallReason::Unknown => "Unknown",
    }
}

fn call_provenance_label(provenance: CallProvenance) -> &'static str {
    match provenance {
        CallProvenance::NativeDirect => "NativeDirect",
        CallProvenance::Native => "Native",
        CallProvenance::SemanticReference => "SemanticReference",
        CallProvenance::ImportBinding => "ImportBinding",
        CallProvenance::MirShape => "MirShape",
        CallProvenance::Topology => "Topology",
        CallProvenance::Extension => "Extension",
        CallProvenance::Model => "Model",
    }
}

fn call_callee_label(callee: &CallCallee) -> String {
    match callee {
        CallCallee::Identifier { name, .. } => format!("identifier:{name}"),
        CallCallee::Member { property, .. } => format!("member:{property}"),
        CallCallee::Index { .. } => "index".to_string(),
        CallCallee::Super => "super".to_string(),
        CallCallee::Import => "import".to_string(),
        CallCallee::FunctionValue { .. } => "function_value".to_string(),
        CallCallee::Constructor { name, .. } => {
            format!("constructor:{}", name.as_deref().unwrap_or("<unknown>"))
        }
        CallCallee::Unknown { reason } => {
            format!("unknown:{}", call_unresolved_reason_label(*reason))
        }
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

mod cfg_debug_json {
    use super::super::AnalysisKernel;
    use crate::analysis::cfg::facts::{
        BasicBlockFact, BasicBlockKind, CfgEdgeFact, CfgEdgeKind, CfgFunctionFact, CfgNodeFact,
        CfgNodeKind, CfgPrecision, CfgStatus, CfgView, ControlDependenceFact,
    };
    use crate::analysis::cfg::ids::{
        BasicBlockId, CfgEdgeId, CfgFunctionId, CfgNodeId, ControlDependenceId,
    };
    use crate::analysis::cfg::store::CfgOutput;
    use crate::analysis::ids::MirBodyId;
    use crate::core::{AnalysisDb, FileId, FunctionId, Language, Span};
    use std::path::PathBuf;

    #[test]
    fn cfg_debug_json_contains_deterministic_cfg_arrays() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "export function app() { return 1; }\n".to_string(),
        );
        db.replace_cfg_facts(CfgOutput {
            functions: vec![CfgFunctionFact {
                id: CfgFunctionId(0),
                body: MirBodyId(0),
                function: FunctionId(0),
                language: Language::TypeScript,
                file,
                span: span(file),
                entry_node: CfgNodeId(0),
                normal_exit_node: CfgNodeId(1),
                exceptional_exit_node: None,
                stable_key: "cfg:function:app".to_string(),
                status: CfgStatus::Resolved,
                precision: CfgPrecision::ExactLowered,
            }],
            nodes: vec![
                node(0, CfgNodeKind::Entry, "cfg:node:entry"),
                node(1, CfgNodeKind::ExitNormal, "cfg:node:exit"),
            ],
            blocks: vec![
                block(0, BasicBlockKind::Entry, CfgNodeId(0), "cfg:block:entry"),
                block(1, BasicBlockKind::ExitNormal, CfgNodeId(1), "cfg:block:exit"),
            ],
            edges: vec![CfgEdgeFact {
                id: CfgEdgeId(0),
                cfg_function: CfgFunctionId(0),
                view: CfgView::NormalControl,
                from: CfgNodeId(0),
                to: CfgNodeId(1),
                from_block: BasicBlockId(0),
                to_block: BasicBlockId(1),
                kind: CfgEdgeKind::Normal,
                label: None,
                stable_key: "cfg:edge:entry-exit".to_string(),
                status: CfgStatus::Resolved,
                precision: CfgPrecision::ExactLowered,
            }],
            control_dependence: vec![ControlDependenceFact {
                id: ControlDependenceId(0),
                cfg_function: CfgFunctionId(0),
                view: CfgView::NormalControl,
                controlling_edge: CfgEdgeId(0),
                controlling_edge_kind: CfgEdgeKind::Normal,
                controlled_block: BasicBlockId(1),
                stable_key: "cfg:dependence:entry-exit".to_string(),
                status: CfgStatus::Resolved,
                precision: CfgPrecision::ExactLowered,
            }],
            ..CfgOutput::empty()
        })
        .expect("cfg rows should store");

        let report = AnalysisKernel::metadata_debug_json_for_test(&db);
        let cfg = report["cfg"]
            .as_object()
            .unwrap_or_else(|| panic!("missing cfg debug object: {report:#?}"));

        for dotted_key in [
            "cfg.functions",
            "cfg.nodes",
            "cfg.blocks",
            "cfg.edges",
            "cfg.reachability",
            "cfg.dominators",
            "cfg.postdominators",
            "cfg.control_dependence",
            "cfg.unsupported",
        ] {
            let key = dotted_key
                .strip_prefix("cfg.")
                .expect("test keys use cfg prefix");
            assert!(
                cfg.get(key).and_then(serde_json::Value::as_array).is_some(),
                "cfg debug object missing `{dotted_key}` array: {cfg:#?}"
            );
        }
        assert_eq!(
            cfg["edges"][0]["payload"].as_str(),
            Some("kind=normal;from_block=0;to_block=1;from_node=0;to_node=1")
        );
        assert!(!report.to_string().contains(env!("CARGO_MANIFEST_DIR")));
    }

    fn node(id: u64, kind: CfgNodeKind, stable_key: &str) -> CfgNodeFact {
        CfgNodeFact {
            id: CfgNodeId(id),
            cfg_function: CfgFunctionId(0),
            body: MirBodyId(0),
            operation: None,
            block: BasicBlockId(id),
            kind,
            span: Some(span(FileId(0))),
            generated: true,
            operation_ordinal: id as u32,
            stable_key: stable_key.to_string(),
            status: CfgStatus::Resolved,
            precision: CfgPrecision::ExactLowered,
        }
    }

    fn block(
        id: u64,
        kind: BasicBlockKind,
        node: CfgNodeId,
        stable_key: &str,
    ) -> BasicBlockFact {
        BasicBlockFact {
            id: BasicBlockId(id),
            cfg_function: CfgFunctionId(0),
            kind,
            first_node: Some(node),
            last_node: Some(node),
            reachable: true,
            reverse_postorder: id as u32,
            stable_key: stable_key.to_string(),
            status: CfgStatus::Resolved,
            precision: CfgPrecision::ExactLowered,
        }
    }

    fn span(file: FileId) -> Span {
        Span {
            file,
            start_byte: 0,
            end_byte: 10,
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 11,
        }
    }
}

mod calls_debug_json {
    use super::super::AnalysisKernel;
    use crate::analysis::calls::facts::{
        CallAlgorithm, CallCallee, CallEdgeKind, CallPrecision, CallProvenance, CallSiteFact,
        CallSyntaxKind, CallTargetFact, CallTargetStatus, UnresolvedCallFact, UnresolvedCallReason,
    };
    use crate::analysis::calls::store::CallOutput;
    use crate::analysis::ids::{CallSiteId, CallTargetId, MirBodyId, MirOpId};
    use crate::core::{
        AnalysisDb, FileId, FunctionFact, FunctionId, Language, Span, SymbolFact, SymbolId,
        SymbolKind, SymbolNamespace, SymbolPrecision,
    };
    use std::path::PathBuf;

    #[test]
    fn metadata_debug_json_contains_call_rows_counts_and_indexes() {
        let mut db = call_db();
        db.replace_call_facts(CallOutput {
            sites: vec![site(0, "call-site:app-target")],
            targets: vec![target(0, "call-target:app-target")],
            unresolved: vec![unresolved("call-unresolved:dynamic")],
        })
        .expect("call rows should store");

        let report = AnalysisKernel::metadata_debug_json_for_test(&db);
        let calls = report["calls"]
            .as_object()
            .unwrap_or_else(|| panic!("missing calls debug object: {report:#?}"));

        for key in ["sites", "targets", "unresolved", "index_counts", "counts"] {
            assert!(calls.get(key).is_some(), "missing calls.{key}: {calls:#?}");
        }
        assert_eq!(calls["sites"][0]["stable_key"].as_str(), Some("call-site:app-target"));
        assert_eq!(
            calls["targets"][0]["stable_key"].as_str(),
            Some("call-target:app-target")
        );
        assert_eq!(
            calls["unresolved"][0]["stable_key"].as_str(),
            Some("call-unresolved:dynamic")
        );
        assert_eq!(calls["counts"]["by_language"]["TypeScript"].as_u64(), Some(1));
        assert_eq!(calls["counts"]["by_call_kind"]["Function"].as_u64(), Some(1));
        assert_eq!(
            calls["counts"]["by_algorithm"]["DirectReference"].as_u64(),
            Some(1)
        );
        assert_eq!(calls["counts"]["by_status"]["Resolved"].as_u64(), Some(2));
        assert_eq!(
            calls["counts"]["by_unresolved_reason"]["DynamicProperty"].as_u64(),
            Some(1)
        );
        assert_eq!(calls["counts"]["by_provider"]["polint.calls"].as_u64(), Some(3));
    }

    #[test]
    fn calls_debug_json_exposes_deterministic_d10_index_counts() {
        let mut db = call_db();
        db.replace_call_facts(CallOutput {
            sites: vec![site(0, "call-site:app-target")],
            targets: vec![target(0, "call-target:app-target")],
            unresolved: vec![unresolved("call-unresolved:dynamic")],
        })
        .expect("call rows should store");

        let report = AnalysisKernel::metadata_debug_json_for_test(&db);
        let index_counts = report["calls"]["index_counts"]
            .as_object()
            .unwrap_or_else(|| panic!("missing call index counts: {report:#?}"));

        for key in [
            "sites_by_caller",
            "targets_by_site",
            "outgoing_by_function",
            "outgoing_by_symbol",
            "incoming_by_symbol",
            "incoming_by_function",
            "unresolved_by_reason",
            "unresolved_by_status",
        ] {
            assert_eq!(index_counts[key].as_u64(), Some(1), "bad {key}: {index_counts:#?}");
        }
    }

    #[test]
    fn calls_debug_json_counts_call_site_only_and_unresolved_rows() {
        let mut db = call_db();
        let mut callback_site = site(0, "call-site:callback");
        callback_site.kind = CallSyntaxKind::FunctionValue;
        callback_site.callee = CallCallee::FunctionValue { place: crate::analysis::ids::PlaceId(1) };
        callback_site.status = CallTargetStatus::Unresolved;
        db.replace_call_facts(CallOutput {
            sites: vec![callback_site],
            targets: Vec::new(),
            unresolved: vec![UnresolvedCallFact {
                site: CallSiteId(0),
                caller: FunctionId(0),
                status: CallTargetStatus::Unresolved,
                reason: UnresolvedCallReason::FunctionValue,
                algorithm: CallAlgorithm::SyntaxOnly,
                provenance: CallProvenance::MirShape,
                precision: CallPrecision::Unknown,
                stable_key: "call-unresolved:function-value".to_string(),
            }],
        })
        .expect("call rows should store");

        let report = AnalysisKernel::metadata_debug_json_for_test(&db);
        let counts = &report["calls"]["counts"];

        assert_eq!(counts["by_language"]["TypeScript"].as_u64(), Some(1));
        assert_eq!(counts["by_call_kind"]["FunctionValue"].as_u64(), Some(1));
        assert_eq!(counts["by_status"]["Unresolved"].as_u64(), Some(2));
        assert_eq!(
            counts["by_unresolved_reason"]["FunctionValue"].as_u64(),
            Some(1)
        );
    }

    #[test]
    fn calls_debug_json_omits_source_ast_absolute_paths_and_dense_identity() {
        let mut db = call_db();
        db.replace_call_facts(CallOutput {
            sites: vec![site(0, "call-site:app-target")],
            targets: vec![target(0, "call-target:app-target")],
            unresolved: vec![unresolved("call-unresolved:dynamic")],
        })
        .expect("call rows should store");

        let report = AnalysisKernel::metadata_debug_json_for_test(&db);
        let encoded = report["calls"].to_string();

        assert!(!encoded.contains(env!("CARGO_MANIFEST_DIR")));
        assert!(!encoded.contains("export function app"));
        assert!(!encoded.contains("parser"));
        assert!(!encoded.contains("\"id\""));
    }

    fn call_db() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "export function app() { target(); dynamic[name](); }\n".to_string(),
        );
        db.push_function(FunctionFact {
            id: FunctionId(0),
            file,
            name: "app".to_string(),
            span: span(file),
            language: Language::TypeScript,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        db.push_function(FunctionFact {
            id: FunctionId(1),
            file,
            name: "target".to_string(),
            span: span(file),
            language: Language::TypeScript,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        db.replace_symbol_graph_facts(
            vec![symbol(SymbolId(0), file, "app"), symbol(SymbolId(1), file, "target")],
            Vec::new(),
            Vec::new(),
        );
        db
    }

    fn site(id: u64, stable_key: &str) -> CallSiteFact {
        CallSiteFact {
            id: CallSiteId(id),
            language: Language::TypeScript,
            file: FileId(0),
            caller: FunctionId(0),
            owner_symbol: Some(SymbolId(0)),
            body: MirBodyId(0),
            operation: MirOpId(0),
            span: span(FileId(0)),
            kind: CallSyntaxKind::Function,
            callee: CallCallee::Identifier {
                reference: None,
                name: "target".to_string(),
            },
            receiver: None,
            arguments: Vec::new(),
            result: None,
            status: CallTargetStatus::Resolved,
            precision: CallPrecision::SetupAware,
            stable_key: stable_key.to_string(),
        }
    }

    fn target(id: u64, stable_key: &str) -> CallTargetFact {
        CallTargetFact {
            id: CallTargetId(id),
            site: CallSiteId(0),
            caller: FunctionId(0),
            target_function: Some(FunctionId(1)),
            target_symbol: Some(SymbolId(1)),
            edge_kind: CallEdgeKind::Direct,
            algorithm: CallAlgorithm::DirectReference,
            status: CallTargetStatus::Resolved,
            reason: None,
            provenance: CallProvenance::Native,
            precision: CallPrecision::SetupAware,
            stable_key: stable_key.to_string(),
        }
    }

    fn unresolved(stable_key: &str) -> UnresolvedCallFact {
        UnresolvedCallFact {
            site: CallSiteId(0),
            caller: FunctionId(0),
            status: CallTargetStatus::Unresolved,
            reason: UnresolvedCallReason::DynamicProperty,
            algorithm: CallAlgorithm::SyntaxOnly,
            provenance: CallProvenance::MirShape,
            precision: CallPrecision::Unknown,
            stable_key: stable_key.to_string(),
        }
    }

    fn symbol(id: SymbolId, file: FileId, name: &str) -> SymbolFact {
        SymbolFact {
            id,
            language: Language::TypeScript,
            name: name.to_string(),
            qualified_name: name.to_string(),
            kind: SymbolKind::Function,
            namespace: SymbolNamespace::Value,
            file: Some(file),
            package: None,
            module: None,
            owner: None,
            primary_span: Some(span(file)),
            is_exported: true,
            stable_key: format!("symbol:{name}"),
            precision: SymbolPrecision::ExactLocal,
        }
    }

    fn span(file: FileId) -> Span {
        Span {
            file,
            start_byte: 0,
            end_byte: 10,
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 11,
        }
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

#[cfg(test)]
mod abstract_domains_debug_json {
    use super::metadata_debug_json_for_test;
    use crate::analysis::cfg::ids::BasicBlockId;
    use crate::analysis::domains::facts::{
        DomainEventFact, DomainLocation, DomainObservationFact, DomainPrecision, DomainSlot,
        DomainStatus, DomainValue,
    };
    use crate::analysis::domains::store::DomainOutput;
    use crate::analysis::ids::{DomainEventId, DomainObservationId, MirBodyId, MirOpId, PlaceId};
    use crate::analysis::mir::body::{MirBody, MirOutput, MirStatus};
    use crate::analysis::mir::op::{AssignMode, MirOperation, MirOperationKind, MirValue};
    use crate::analysis::places::{PlaceFact, PlaceRoot, PlaceStatus};
    use crate::core::{AnalysisDb, FileId, FunctionFact, FunctionId, Language, Span};
    use serde_json::Value;
    use std::path::PathBuf;

    #[test]
    fn abstract_domains_debug_json_exposes_compact_deterministic_rows() {
        let mut db = base_db();
        db.replace_abstract_domain_facts(DomainOutput {
            observations: vec![DomainObservationFact {
                id: DomainObservationId(0),
                body: MirBodyId(0),
                block: Some(BasicBlockId(0)),
                operation: Some(MirOpId(0)),
                place: Some(PlaceId(0)),
                slot: DomainSlot::Nilness,
                location: DomainLocation::AfterOperation,
                value: DomainValue::TopReason("unknown_value".to_string()),
                status: DomainStatus::Unknown,
                precision: DomainPrecision::Unknown,
                stable_key: "domain:observation:value".to_string(),
            }],
            events: vec![DomainEventFact {
                id: DomainEventId(0),
                body: MirBodyId(0),
                block: Some(BasicBlockId(0)),
                operation: Some(MirOpId(0)),
                slot: Some(DomainSlot::Nilness),
                status: DomainStatus::BudgetExceeded,
                precision: DomainPrecision::Unknown,
                reason: "budget_exceeded".to_string(),
                stable_key: "domain:event:budget".to_string(),
            }],
        });

        let first = metadata_debug_json_for_test(&db);
        let second = metadata_debug_json_for_test(&db);

        assert_eq!(first, second);
        let domains = first
            .get("abstract_domains")
            .expect("abstract domain debug object");
        assert_eq!(domains["observations"].as_array().map(Vec::len), Some(1));
        assert_eq!(domains["events"].as_array().map(Vec::len), Some(1));
        assert_eq!(
            domains["observations"][0]["path"],
            Value::String("src/app.ts".to_string())
        );
        assert_eq!(
            domains["observations"][0]["stable_key"],
            Value::String("domain:observation:value".to_string())
        );
        assert_eq!(
            domains["observations"][0]["slot"],
            Value::String("nilness".to_string())
        );
        assert_eq!(
            domains["observations"][0]["reason"],
            Value::String("unknown_value".to_string())
        );
        assert!(domains.get("counts").is_some());
        assert!(domains.get("index_counts").is_some());

        let encoded = serde_json::to_string(domains).expect("domain debug JSON serializes");
        assert!(!encoded.contains("/Users/"));
        assert!(!encoded.contains("export function app"));
        assert!(!encoded.contains("MirOperationKind"));
        assert!(!encoded.contains("\"run_id\""));
    }

    fn base_db() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "export function app() { let value = 1; return value; }\n".to_string(),
        );
        db.push_function(FunctionFact {
            id: FunctionId(0),
            file,
            name: "app".to_string(),
            span: span(file),
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
                span: span(file),
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
                projections: Vec::new(),
                stable_key: "place:value".to_string(),
                status: PlaceStatus::Partial,
            }],
            operations: vec![MirOperation {
                id: MirOpId(0),
                body: MirBodyId(0),
                ordinal: 0,
                span: span(file),
                kind: MirOperationKind::Assign {
                    place: PlaceId(0),
                    value: MirValue::Literal {
                        value: "1".to_string(),
                    },
                    mode: AssignMode::DeclarationBinding,
                },
                stable_key: "op:assign".to_string(),
                status: MirStatus::Partial,
            }],
            unsupported: Vec::new(),
        })
        .expect("semantic MIR rows should store");
        db
    }

    fn span(file: FileId) -> Span {
        Span {
            file,
            start_byte: 0,
            end_byte: 10,
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 11,
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

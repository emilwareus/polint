use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;

use crate::analysis::ids::{MirBodyId, MirOpId, PlaceId};
use crate::analysis::mir::body::MirStatus;
use crate::analysis::mir::op::{MirOperationKind, MirValue, UnsupportedPrecision};
use crate::analysis::places::{PlaceProjection, PlaceRoot};
use crate::analysis_kernel::validation::IdSets;
use crate::analysis_kernel::{FactFamily, FactPrecision};
use crate::core::{
    AnalysisDb, FileId, FunctionId, ModuleNodeId, PackageId, SEMANTIC_MIR_PROVIDER_ID, Span,
    SymbolId,
};
use crate::diagnostics::Diagnostic;

pub(crate) fn validate_semantic_mir(
    db: &AnalysisDb,
    _ids: &IdSets,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ids = SemanticMirIdSets::from_db(db);

    validate_body_rows(db, &ids, diagnostics);
    validate_place_rows(db, &ids, diagnostics);
    validate_operation_rows(db, &ids, diagnostics);
    validate_unsupported_rows(db, &ids, diagnostics);
    validate_semantic_mir_precision(db, diagnostics);
}

fn validate_body_rows(db: &AnalysisDb, ids: &SemanticMirIdSets, diagnostics: &mut Vec<Diagnostic>) {
    let mut keys = BTreeMap::new();
    for body in db.mir_bodies() {
        check_duplicate_stable_key(
            diagnostics,
            &mut keys,
            FactFamily::MirBody,
            body.stable_key.as_str(),
        );
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::MirBody,
            body.stable_key.as_str(),
            "MirBody.file",
            body.file,
        );
        check_ref(
            diagnostics,
            &ids.functions,
            FactFamily::MirBody,
            body.stable_key.as_str(),
            "MirBody.function",
            body.function,
        );
        check_optional_ref(
            diagnostics,
            &ids.packages,
            FactFamily::MirBody,
            body.stable_key.as_str(),
            "MirBody.package",
            body.package,
        );
        check_optional_ref(
            diagnostics,
            &ids.modules,
            FactFamily::MirBody,
            body.stable_key.as_str(),
            "MirBody.module",
            body.module,
        );
        check_nonempty(
            diagnostics,
            FactFamily::MirBody,
            body.stable_key.as_str(),
            "MirBody.owner_stable_key",
            body.owner_stable_key.as_str(),
        );
        check_span(
            db,
            &ids.files,
            diagnostics,
            FactFamily::MirBody,
            body.stable_key.as_str(),
            "MirBody.span",
            Some(body.file),
            &body.span,
        );
    }
}

fn validate_place_rows(
    db: &AnalysisDb,
    ids: &SemanticMirIdSets,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut keys = BTreeMap::new();
    for place in db.mir_places() {
        check_duplicate_stable_key(
            diagnostics,
            &mut keys,
            FactFamily::Place,
            place.stable_key.as_str(),
        );
        check_optional_ref(
            diagnostics,
            &ids.files,
            FactFamily::Place,
            place.stable_key.as_str(),
            "Place.file",
            place.file,
        );
        check_optional_ref(
            diagnostics,
            &ids.functions,
            FactFamily::Place,
            place.stable_key.as_str(),
            "Place.function",
            place.function,
        );
        validate_place_root(&place.root, ids, diagnostics, place.stable_key.as_str());
        validate_place_projections(&place.projections, diagnostics, place.stable_key.as_str());

        if let Some(file) = place.file
            && db.file(file).is_none()
        {
            diagnostics.push(semantic_mir_diagnostic(
                FactFamily::Place,
                place.stable_key.as_str(),
                "Place.file",
                format!("Place.file does not exist: {file:?}"),
            ));
        }
    }
}

fn validate_operation_rows(
    db: &AnalysisDb,
    ids: &SemanticMirIdSets,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut keys = BTreeMap::new();
    for operation in db.mir_operations() {
        check_duplicate_stable_key(
            diagnostics,
            &mut keys,
            FactFamily::MirOperation,
            operation.stable_key.as_str(),
        );
        check_ref(
            diagnostics,
            &ids.bodies,
            FactFamily::MirOperation,
            operation.stable_key.as_str(),
            "MirOperation.body",
            operation.body,
        );
        check_span(
            db,
            &ids.files,
            diagnostics,
            FactFamily::MirOperation,
            operation.stable_key.as_str(),
            "MirOperation.span",
            db.mir_bodies()
                .iter()
                .find(|body| body.id == operation.body)
                .map(|body| body.file),
            &operation.span,
        );
        validate_operation_kind(
            &operation.kind,
            ids,
            diagnostics,
            operation.stable_key.as_str(),
        );
    }
}

fn validate_unsupported_rows(
    db: &AnalysisDb,
    ids: &SemanticMirIdSets,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut keys = BTreeMap::new();
    for row in db.unsupported_semantics() {
        check_duplicate_stable_key(
            diagnostics,
            &mut keys,
            FactFamily::UnsupportedSemantic,
            row.stable_key.as_str(),
        );
        check_optional_ref(
            diagnostics,
            &ids.bodies,
            FactFamily::UnsupportedSemantic,
            row.stable_key.as_str(),
            "UnsupportedSemantic.body",
            row.body,
        );
        check_optional_ref(
            diagnostics,
            &ids.operations,
            FactFamily::UnsupportedSemantic,
            row.stable_key.as_str(),
            "UnsupportedSemantic.operation",
            row.operation,
        );
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::UnsupportedSemantic,
            row.stable_key.as_str(),
            "UnsupportedSemantic.file",
            row.file,
        );
        check_span(
            db,
            &ids.files,
            diagnostics,
            FactFamily::UnsupportedSemantic,
            row.stable_key.as_str(),
            "UnsupportedSemantic.span",
            Some(row.file),
            &row.span,
        );
        for place in &row.affected_places {
            check_ref(
                diagnostics,
                &ids.places,
                FactFamily::UnsupportedSemantic,
                row.stable_key.as_str(),
                "UnsupportedSemantic.affected_places",
                *place,
            );
        }
        check_nonempty(
            diagnostics,
            FactFamily::UnsupportedSemantic,
            row.stable_key.as_str(),
            "UnsupportedSemantic.construct",
            row.construct.as_str(),
        );
        check_nonempty(
            diagnostics,
            FactFamily::UnsupportedSemantic,
            row.stable_key.as_str(),
            "UnsupportedSemantic.source_evidence",
            row.source_evidence.as_str(),
        );
        if row.affected_domains.is_empty() {
            diagnostics.push(semantic_mir_diagnostic(
                FactFamily::UnsupportedSemantic,
                row.stable_key.as_str(),
                "UnsupportedSemantic.affected_domains",
                "unsupported semantics must name at least one affected analysis domain",
            ));
        }
        if matches!(row.status, MirStatus::Resolved) {
            diagnostics.push(semantic_mir_diagnostic(
                FactFamily::UnsupportedSemantic,
                row.stable_key.as_str(),
                "UnsupportedSemantic.status",
                "unsupported semantics cannot be resolved",
            ));
        }
        if !matches!(row.precision, UnsupportedPrecision::Unsupported) {
            diagnostics.push(semantic_mir_diagnostic(
                FactFamily::UnsupportedSemantic,
                row.stable_key.as_str(),
                "UnsupportedSemantic.precision",
                "unsupported semantics must preserve unsupported precision",
            ));
        }
    }
}

fn validate_place_root(
    root: &PlaceRoot,
    ids: &SemanticMirIdSets,
    diagnostics: &mut Vec<Diagnostic>,
    stable_key: &str,
) {
    match root {
        PlaceRoot::Local { function, name } => {
            check_ref(
                diagnostics,
                &ids.functions,
                FactFamily::Place,
                stable_key,
                "PlaceRoot::Local.function",
                *function,
            );
            check_nonempty(
                diagnostics,
                FactFamily::Place,
                stable_key,
                "PlaceRoot::Local.name",
                name.as_str(),
            );
        }
        PlaceRoot::Parameter { function, name, .. } => {
            check_ref(
                diagnostics,
                &ids.functions,
                FactFamily::Place,
                stable_key,
                "PlaceRoot::Parameter.function",
                *function,
            );
            if let Some(name) = name {
                check_nonempty(
                    diagnostics,
                    FactFamily::Place,
                    stable_key,
                    "PlaceRoot::Parameter.name",
                    name.as_str(),
                );
            }
        }
        PlaceRoot::Global { symbol, name } => {
            check_optional_ref(
                diagnostics,
                &ids.symbols,
                FactFamily::Place,
                stable_key,
                "PlaceRoot::Global.symbol",
                *symbol,
            );
            check_nonempty(
                diagnostics,
                FactFamily::Place,
                stable_key,
                "PlaceRoot::Global.name",
                name.as_str(),
            );
        }
        PlaceRoot::Temporary { body, .. } => {
            check_ref(
                diagnostics,
                &ids.bodies,
                FactFamily::Place,
                stable_key,
                "PlaceRoot::Temporary.body",
                *body,
            );
        }
        PlaceRoot::CallReturn { .. } => {}
        PlaceRoot::Unknown { evidence } => {
            check_nonempty(
                diagnostics,
                FactFamily::Place,
                stable_key,
                "PlaceRoot::Unknown.evidence",
                evidence.as_str(),
            );
        }
    }
}

fn validate_place_projections(
    projections: &[PlaceProjection],
    diagnostics: &mut Vec<Diagnostic>,
    stable_key: &str,
) {
    for (index, projection) in projections.iter().enumerate() {
        let field = "Place.projections";
        match projection {
            PlaceProjection::Field(name)
            | PlaceProjection::Property(name)
            | PlaceProjection::IndexKnown(name) => {
                if name.trim().is_empty() {
                    diagnostics.push(semantic_mir_diagnostic(
                        FactFamily::Place,
                        stable_key,
                        field,
                        format!("malformed projection at index {index}: empty selector"),
                    ));
                }
            }
            PlaceProjection::IndexUnknown { evidence } | PlaceProjection::Unknown { evidence } => {
                if evidence.trim().is_empty() {
                    diagnostics.push(semantic_mir_diagnostic(
                        FactFamily::Place,
                        stable_key,
                        field,
                        format!("malformed projection at index {index}: missing evidence"),
                    ));
                }
            }
            PlaceProjection::CallReturn(_) if index + 1 != projections.len() => {
                diagnostics.push(semantic_mir_diagnostic(
                    FactFamily::Place,
                    stable_key,
                    field,
                    format!("malformed projection at index {index}: call return must be terminal"),
                ));
            }
            PlaceProjection::Deref
            | PlaceProjection::AwaitResult
            | PlaceProjection::CallReturn(_) => {}
        }
    }
}

fn validate_operation_kind(
    kind: &MirOperationKind,
    ids: &SemanticMirIdSets,
    diagnostics: &mut Vec<Diagnostic>,
    stable_key: &str,
) {
    match kind {
        MirOperationKind::StorageLive { place } | MirOperationKind::Read { place } => {
            check_place_ref(
                diagnostics,
                ids,
                stable_key,
                "MirOperationKind.place",
                *place,
            );
        }
        MirOperationKind::Bind { place, value }
        | MirOperationKind::Assign { place, value, .. }
        | MirOperationKind::Write { place, value } => {
            check_place_ref(
                diagnostics,
                ids,
                stable_key,
                "MirOperationKind.place",
                *place,
            );
            validate_mir_value(value, ids, diagnostics, stable_key);
        }
        MirOperationKind::Branch { .. } => {}
        MirOperationKind::Call {
            callee,
            arguments,
            return_place,
            ..
        } => {
            validate_mir_value(callee, ids, diagnostics, stable_key);
            for argument in arguments {
                check_place_ref(
                    diagnostics,
                    ids,
                    stable_key,
                    "MirOperationKind::Call.arguments",
                    *argument,
                );
            }
            check_place_ref(
                diagnostics,
                ids,
                stable_key,
                "MirOperationKind::Call.return_place",
                *return_place,
            );
        }
        MirOperationKind::Return { value } => {
            if let Some(value) = value {
                validate_mir_value(value, ids, diagnostics, stable_key);
            }
        }
        MirOperationKind::Unsupported { unsupported } => {
            check_ref(
                diagnostics,
                &ids.unsupported,
                FactFamily::MirOperation,
                stable_key,
                "MirOperationKind::Unsupported.unsupported",
                *unsupported,
            );
        }
    }
}

fn validate_mir_value(
    value: &MirValue,
    ids: &SemanticMirIdSets,
    diagnostics: &mut Vec<Diagnostic>,
    stable_key: &str,
) {
    match value {
        MirValue::Literal { value } => check_nonempty(
            diagnostics,
            FactFamily::MirOperation,
            stable_key,
            "MirValue::Literal.value",
            value.as_str(),
        ),
        MirValue::Place(place) => {
            check_place_ref(diagnostics, ids, stable_key, "MirValue::Place", *place);
        }
        MirValue::Temporary(_) | MirValue::CallReturn(_) => {}
        MirValue::Unknown { evidence } => check_nonempty(
            diagnostics,
            FactFamily::MirOperation,
            stable_key,
            "MirValue::Unknown.evidence",
            evidence.as_str(),
        ),
    }
}

fn validate_semantic_mir_precision(db: &AnalysisDb, diagnostics: &mut Vec<Diagnostic>) {
    for (reference, metadata) in db.fact_meta().rows() {
        if !is_semantic_mir_family(reference.family)
            || metadata.producer_id != SEMANTIC_MIR_PROVIDER_ID
            || metadata.precision != FactPrecision::Exact
        {
            continue;
        }
        diagnostics.push(semantic_mir_diagnostic(
            reference.family,
            metadata.stable_key.as_str(),
            "FactMeta.precision",
            "provider precision ceiling exceeded: semantic MIR rows are setup-aware or lower, not exact",
        ));
    }
}

fn check_place_ref(
    diagnostics: &mut Vec<Diagnostic>,
    ids: &SemanticMirIdSets,
    stable_key: &str,
    field: &'static str,
    place: PlaceId,
) {
    check_ref(
        diagnostics,
        &ids.places,
        FactFamily::MirOperation,
        stable_key,
        field,
        place,
    );
}

fn check_duplicate_stable_key(
    diagnostics: &mut Vec<Diagnostic>,
    seen: &mut BTreeMap<String, usize>,
    family: FactFamily,
    stable_key: &str,
) {
    let count = seen.entry(stable_key.to_string()).or_default();
    *count += 1;
    if *count > 1 {
        diagnostics.push(semantic_mir_diagnostic(
            family,
            stable_key,
            "stable_key",
            "stable_key_conflict",
        ));
    }
}

fn check_ref<T>(
    diagnostics: &mut Vec<Diagnostic>,
    valid_ids: &BTreeSet<T>,
    family: FactFamily,
    stable_key: &str,
    field: &'static str,
    value: T,
) where
    T: Copy + Debug + Ord,
{
    if valid_ids.contains(&value) {
        return;
    }
    diagnostics.push(semantic_mir_diagnostic(
        family,
        stable_key,
        field,
        format!("{field} does not exist: {value:?}"),
    ));
}

fn check_optional_ref<T>(
    diagnostics: &mut Vec<Diagnostic>,
    valid_ids: &BTreeSet<T>,
    family: FactFamily,
    stable_key: &str,
    field: &'static str,
    value: Option<T>,
) where
    T: Copy + Debug + Ord,
{
    if let Some(value) = value {
        check_ref(diagnostics, valid_ids, family, stable_key, field, value);
    }
}

fn check_nonempty(
    diagnostics: &mut Vec<Diagnostic>,
    family: FactFamily,
    stable_key: &str,
    field: &'static str,
    value: &str,
) {
    if !value.trim().is_empty() {
        return;
    }
    diagnostics.push(semantic_mir_diagnostic(
        family,
        stable_key,
        field,
        format!("{field} must not be empty"),
    ));
}

fn check_span(
    db: &AnalysisDb,
    file_ids: &BTreeSet<FileId>,
    diagnostics: &mut Vec<Diagnostic>,
    family: FactFamily,
    stable_key: &str,
    field: &'static str,
    owner_file: Option<FileId>,
    span: &Span,
) {
    let Some(reason) = span_failure_reason(db, file_ids, owner_file, span) else {
        return;
    };
    diagnostics.push(semantic_mir_diagnostic(family, stable_key, field, reason));
}

fn span_failure_reason(
    db: &AnalysisDb,
    file_ids: &BTreeSet<FileId>,
    owner_file: Option<FileId>,
    span: &Span,
) -> Option<String> {
    if !file_ids.contains(&span.file) {
        return Some("span file does not exist".to_string());
    }
    if let Some(owner_file) = owner_file
        && owner_file != span.file
    {
        return Some("span file does not match owning file".to_string());
    }
    if span.start_byte > span.end_byte {
        return Some("start_byte exceeds end_byte".to_string());
    }
    let source_len = db.file(span.file).map(|file| file.source.len() as u32)?;
    if span.end_byte > source_len {
        return Some(format!("end_byte exceeds source length {source_len}"));
    }
    None
}

fn semantic_mir_diagnostic(
    family: FactFamily,
    stable_key: &str,
    field: &'static str,
    reason: impl Into<String>,
) -> Diagnostic {
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        crate::diagnostics::TextRange::point(1, 1),
        format!(
            "Semantic MIR validation failed for {} stable key.",
            family.label()
        ),
    )
    .with_evidence("family", family.label())
    .with_evidence("stable_key", stable_key.to_string())
    .with_evidence("field", field)
    .with_evidence("reason", reason.into())
}

fn is_semantic_mir_family(family: FactFamily) -> bool {
    matches!(
        family,
        FactFamily::MirBody
            | FactFamily::MirOperation
            | FactFamily::Place
            | FactFamily::UnsupportedSemantic
    )
}

#[derive(Default)]
struct SemanticMirIdSets {
    files: BTreeSet<FileId>,
    functions: BTreeSet<FunctionId>,
    packages: BTreeSet<PackageId>,
    modules: BTreeSet<ModuleNodeId>,
    symbols: BTreeSet<SymbolId>,
    bodies: BTreeSet<MirBodyId>,
    operations: BTreeSet<MirOpId>,
    places: BTreeSet<PlaceId>,
    unsupported: BTreeSet<crate::analysis::ids::UnsupportedId>,
}

impl SemanticMirIdSets {
    fn from_db(db: &AnalysisDb) -> Self {
        Self {
            files: db.files().iter().map(|fact| fact.id).collect(),
            functions: db.functions().iter().map(|fact| fact.id).collect(),
            packages: db.packages().iter().map(|fact| fact.id).collect(),
            modules: db.module_nodes().iter().map(|fact| fact.id).collect(),
            symbols: db.symbols().iter().map(|fact| fact.id).collect(),
            bodies: db.mir_bodies().iter().map(|fact| fact.id).collect(),
            operations: db.mir_operations().iter().map(|fact| fact.id).collect(),
            places: db.mir_places().iter().map(|fact| fact.id).collect(),
            unsupported: db
                .unsupported_semantics()
                .iter()
                .map(|fact| fact.id)
                .collect(),
        }
    }
}

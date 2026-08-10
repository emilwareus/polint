use std::collections::BTreeMap;

use crate::analysis::access_paths::facts::{
    AccessPathFact, AccessPathProjection, AccessPathStatus,
};
use crate::analysis::access_paths::store::AccessPathOutput;
use crate::analysis::ids::{
    AbstractValueId, AccessPathId, AllocationTokenId, PlaceId, TypeFactId, TypeSetId, ValueFactId,
};
use crate::analysis::mir::op::{
    MirAggregateKind, MirOperation, MirOperationKind, MirValue, UnsupportedPrecision,
};
use crate::analysis::places::{PlaceFact, PlaceProjection, PlaceRoot, PlaceStatus};
use crate::analysis::stable_key::semantic_stable_key;
use crate::analysis::types::facts::{
    NarrowedTypeFact, TypeConfidence, TypeFact, TypePhase, TypePrecision, TypeProvenance,
    TypeShape, TypeStatus, TypeSubject,
};
use crate::analysis::types::store::{TypeOutput, TypeValueAliasOutput};
use crate::analysis::values::facts::{
    AllocationKind, AllocationTokenFact, ValueFact, ValueKind, ValuePrecision, ValueProvenance,
    ValueStatus, ValueSubject,
};
use crate::analysis::values::store::ValueOutput;
use crate::analysis_kernel::FactFamily;
use crate::core::{AnalysisDb, FileId, FunctionId, Language};

type RootPlaceKey = (PlaceRoot, Option<FileId>, Option<FunctionId>);

pub(crate) fn derive_go_type_value_alias(db: &AnalysisDb) -> TypeValueAliasOutput {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    let body_by_id = db
        .mir_bodies()
        .iter()
        .map(|body| (body.id, body))
        .collect::<BTreeMap<_, _>>();
    let place_by_id = db
        .mir_places()
        .iter()
        .map(|place| (place.id, place))
        .collect::<BTreeMap<_, _>>();
    let root_place_by_key = root_place_by_key(db, Language::Go);
    let place_types = db
        .mir_place_types()
        .iter()
        .map(|fact| (fact.place, &fact.ty))
        .collect::<BTreeMap<_, _>>();

    let mut types = Vec::new();
    let mut access_paths = Vec::new();
    let mut values = Vec::new();
    let mut allocations = Vec::new();

    for place in db
        .mir_places()
        .iter()
        .filter(|place| place.language == Language::Go)
    {
        let body = place
            .root
            .body()
            .and_then(|body| body_by_id.get(&body).copied());
        types.push(type_fact_for_place(
            interner,
            types.len() as u64,
            place,
            body,
            place_types.get(&place.id).copied(),
        ));
        access_paths.push(access_path_for_place(
            interner,
            access_paths.len() as u64,
            place,
            &root_place_by_key,
        ));
    }

    for operation in db.mir_operations().iter().filter(|operation| {
        body_by_id
            .get(&operation.body)
            .is_some_and(|body| body.language == Language::Go)
    }) {
        collect_values_for_operation(
            interner,
            operation,
            &place_by_id,
            &mut values,
            &mut allocations,
        );
    }

    for unsupported in db
        .unsupported_semantics()
        .iter()
        .filter(|row| row.language == Language::Go)
    {
        let body = unsupported
            .body
            .and_then(|body| body_by_id.get(&body).copied());
        if unsupported.affected_places.is_empty() {
            types.push(unsupported_type_fact(
                interner,
                types.len() as u64,
                TypeSubject::Unknown(unsupported.construct.clone()),
                unsupported,
                body,
                None,
            ));
        } else {
            for place in &unsupported.affected_places {
                types.push(unsupported_type_fact(
                    interner,
                    types.len() as u64,
                    TypeSubject::Place(*place),
                    unsupported,
                    body,
                    Some(*place),
                ));
            }
        }
    }

    TypeValueAliasOutput {
        types: TypeOutput {
            types,
            narrowed: Vec::<NarrowedTypeFact>::new(),
        },
        values: ValueOutput {
            values,
            allocations,
        },
        access_paths: AccessPathOutput { access_paths },
        ..TypeValueAliasOutput::default()
    }
    .normalized()
}

fn type_fact_for_place(
    interner: &crate::core::StableKeyInterner,
    id: u64,
    place: &PlaceFact,
    body: Option<&crate::analysis::mir::body::MirBody>,
    ty: Option<&TypeShape>,
) -> TypeFact {
    let (status, phase, precision, confidence, shape) = match place.status {
        PlaceStatus::Resolved => (
            TypeStatus::Unknown,
            TypePhase::Unknown,
            TypePrecision::SetupAware,
            TypeConfidence::Medium,
            type_shape_for_place(place, ty),
        ),
        PlaceStatus::Partial => (
            TypeStatus::Unknown,
            TypePhase::Unknown,
            TypePrecision::Conservative,
            TypeConfidence::Low,
            type_shape_for_place(place, ty),
        ),
        PlaceStatus::Unknown => (
            TypeStatus::Unknown,
            TypePhase::Unknown,
            TypePrecision::Unknown,
            TypeConfidence::Low,
            TypeShape::Unknown {
                reason: interner.resolve(place.stable_key).to_string(),
            },
        ),
        PlaceStatus::Unsupported => (
            TypeStatus::Unsupported,
            TypePhase::Unsupported,
            TypePrecision::Unsupported,
            TypeConfidence::Low,
            TypeShape::Unsupported {
                reason: interner.resolve(place.stable_key).to_string(),
            },
        ),
    };

    TypeFact {
        id: TypeFactId(id),
        subject: TypeSubject::Place(place.id),
        type_set: TypeSetId(id),
        shape,
        phase,
        language: Language::Go,
        file: place.file.or_else(|| body.map(|body| body.file)),
        function: place.function.or_else(|| body.map(|body| body.function)),
        body: body.map(|body| body.id),
        place: Some(place.id),
        cfg_block: None,
        operation: None,
        precision,
        confidence,
        status,
        provenance: TypeProvenance::Native,
        stable_key: stable_key(
            interner,
            FactFamily::Type,
            [
                ("language", "go".to_string()),
                ("place", interner.resolve(place.stable_key).to_string()),
                ("phase", format!("{phase:?}")),
            ],
        ),
    }
}

fn type_shape_for_place(place: &PlaceFact, ty: Option<&TypeShape>) -> TypeShape {
    if let Some(ty) = ty {
        return ty.clone();
    }
    match &place.root {
        PlaceRoot::Parameter { index, name, .. } => TypeShape::Unknown {
            reason: format!(
                "go parameter {}{}",
                index,
                name.as_ref()
                    .map(|name| format!(":{name}"))
                    .unwrap_or_default()
            ),
        },
        PlaceRoot::Local { name, .. } => TypeShape::Unknown {
            reason: format!("go local:{name}"),
        },
        PlaceRoot::Global { name, .. } => TypeShape::Nominal {
            type_id: format!("go global:{name}"),
        },
        PlaceRoot::Temporary { .. } => TypeShape::Unknown {
            reason: "go temporary".to_string(),
        },
        PlaceRoot::CallReturn { .. } => TypeShape::Unknown {
            reason: "go call return".to_string(),
        },
        PlaceRoot::Unknown { evidence } => TypeShape::Unknown {
            reason: evidence.clone(),
        },
    }
}

fn access_path_for_place(
    interner: &crate::core::StableKeyInterner,
    id: u64,
    place: &PlaceFact,
    root_place_by_key: &BTreeMap<RootPlaceKey, PlaceId>,
) -> AccessPathFact {
    let projections = place
        .projections
        .iter()
        .map(access_path_projection)
        .collect::<Vec<_>>();
    let status = match place.status {
        PlaceStatus::Resolved => AccessPathStatus::Resolved,
        PlaceStatus::Partial => AccessPathStatus::Partial,
        PlaceStatus::Unknown => AccessPathStatus::Unknown,
        PlaceStatus::Unsupported => AccessPathStatus::Unsupported,
    };
    AccessPathFact {
        id: AccessPathId(id),
        base: root_place_id(place, root_place_by_key),
        depth: projections.len() as u32,
        projections,
        language: Language::Go,
        file: place.file,
        function: place.function,
        body: place.root.body(),
        status,
        stable_key: stable_key(
            interner,
            FactFamily::AccessPath,
            [
                ("language", "go".to_string()),
                ("place", interner.resolve(place.stable_key).to_string()),
                ("projection_count", place.projections.len().to_string()),
            ],
        ),
    }
}

fn root_place_by_key(db: &AnalysisDb, language: Language) -> BTreeMap<RootPlaceKey, PlaceId> {
    db.mir_places()
        .iter()
        .filter(|place| place.language == language && place.projections.is_empty())
        .map(|place| (root_place_key(place), place.id))
        .collect()
}

fn root_place_id(
    place: &PlaceFact,
    root_place_by_key: &BTreeMap<RootPlaceKey, PlaceId>,
) -> PlaceId {
    root_place_by_key
        .get(&root_place_key(place))
        .copied()
        .unwrap_or(place.id)
}

fn root_place_key(place: &PlaceFact) -> RootPlaceKey {
    (place.root.clone(), place.file, place.function)
}

fn access_path_projection(projection: &PlaceProjection) -> AccessPathProjection {
    match projection {
        PlaceProjection::Field(name) => AccessPathProjection::Field(name.clone()),
        PlaceProjection::Property(name) => AccessPathProjection::Property(name.clone()),
        PlaceProjection::IndexKnown(index) => AccessPathProjection::IndexKnown(index.clone()),
        PlaceProjection::IndexUnknown { evidence } => AccessPathProjection::IndexUnknown {
            evidence: evidence.clone(),
        },
        PlaceProjection::Deref => AccessPathProjection::Deref,
        PlaceProjection::AwaitResult => AccessPathProjection::AwaitResult,
        PlaceProjection::CallReturn(call) => AccessPathProjection::CallReturn(*call),
        PlaceProjection::Unknown { evidence } => AccessPathProjection::Unknown {
            evidence: evidence.clone(),
        },
    }
}

fn collect_values_for_operation(
    interner: &crate::core::StableKeyInterner,
    operation: &MirOperation,
    place_by_id: &BTreeMap<crate::analysis::ids::PlaceId, &PlaceFact>,
    values: &mut Vec<ValueFact>,
    allocations: &mut Vec<AllocationTokenFact>,
) {
    match &operation.kind {
        MirOperationKind::Bind { place, value }
        | MirOperationKind::Assign { place, value, .. }
        | MirOperationKind::Write { place, value } => {
            push_value_for_mir_value(
                interner,
                operation,
                Some(*place),
                value,
                values,
                allocations,
            );
        }
        MirOperationKind::Call {
            callee,
            return_place,
            ..
        } => {
            push_function_value(interner, operation, callee, values);
            values.push(ValueFact {
                id: ValueFactId(values.len() as u64),
                subject: ValueSubject::Place(*return_place),
                value: AbstractValueId(values.len() as u64),
                kind: ValueKind::CallReturn(*return_place),
                language: Language::Go,
                file: Some(operation.span.file),
                function: place_by_id
                    .get(return_place)
                    .and_then(|place| place.function),
                body: Some(operation.body),
                precision: ValuePrecision::Conservative,
                status: ValueStatus::Unknown,
                provenance: ValueProvenance::Native,
                stable_key: stable_key(
                    interner,
                    FactFamily::Value,
                    [
                        ("language", "go".to_string()),
                        (
                            "operation",
                            interner.resolve(operation.stable_key).to_string(),
                        ),
                        ("kind", "call_return".to_string()),
                    ],
                ),
            });
        }
        _ => {}
    }
}

fn push_value_for_mir_value(
    interner: &crate::core::StableKeyInterner,
    operation: &MirOperation,
    place: Option<crate::analysis::ids::PlaceId>,
    value: &MirValue,
    values: &mut Vec<ValueFact>,
    allocations: &mut Vec<AllocationTokenFact>,
) {
    let (kind, status, precision) = match value {
        MirValue::Literal { value } => literal_value_kind(interner, value, operation, allocations),
        MirValue::Place(place) => (
            ValueKind::PlaceRef(*place),
            ValueStatus::Present,
            ValuePrecision::SetupAware,
        ),
        MirValue::Temporary(_) => (
            ValueKind::Unknown {
                evidence: "go temporary".to_string(),
            },
            ValueStatus::Unknown,
            ValuePrecision::Unknown,
        ),
        MirValue::CallReturn(call) => (
            ValueKind::Unknown {
                evidence: format!("call:{}", call.0),
            },
            ValueStatus::Unknown,
            ValuePrecision::Conservative,
        ),
        MirValue::BinOp { op, .. } => (
            ValueKind::Unknown {
                evidence: format!("binary:{op}"),
            },
            ValueStatus::Present,
            ValuePrecision::Conservative,
        ),
        MirValue::Aggregate { kind, .. } => {
            let (allocation_kind, label) = match kind {
                MirAggregateKind::Array => (AllocationKind::ArrayLiteral, "array"),
                MirAggregateKind::Object => (AllocationKind::ObjectLiteral, "object"),
                MirAggregateKind::Composite => {
                    (AllocationKind::CompositeLiteral, "composite_literal")
                }
            };
            let token = push_allocation(interner, operation, allocation_kind, label, allocations);
            (
                ValueKind::CompositeLiteral(token),
                ValueStatus::Present,
                ValuePrecision::SetupAware,
            )
        }
        MirValue::Closure { .. } => {
            let _ = push_allocation(
                interner,
                operation,
                AllocationKind::Closure,
                "closure",
                allocations,
            );
            (
                ValueKind::FunctionObject,
                ValueStatus::Present,
                ValuePrecision::SetupAware,
            )
        }
        MirValue::Unknown { evidence } => (
            ValueKind::Unknown {
                evidence: evidence.clone(),
            },
            ValueStatus::Unknown,
            ValuePrecision::Unknown,
        ),
    };
    values.push(ValueFact {
        id: ValueFactId(values.len() as u64),
        subject: place
            .map(ValueSubject::Place)
            .unwrap_or(ValueSubject::Operation(operation.id)),
        value: AbstractValueId(values.len() as u64),
        kind,
        language: Language::Go,
        file: Some(operation.span.file),
        function: None,
        body: Some(operation.body),
        precision,
        status,
        provenance: ValueProvenance::Native,
        stable_key: stable_key(
            interner,
            FactFamily::Value,
            [
                ("language", "go".to_string()),
                (
                    "operation",
                    interner.resolve(operation.stable_key).to_string(),
                ),
                ("ordinal", values.len().to_string()),
            ],
        ),
    });
}

fn literal_value_kind(
    interner: &crate::core::StableKeyInterner,
    value: &str,
    operation: &MirOperation,
    allocations: &mut Vec<AllocationTokenFact>,
) -> (ValueKind, ValueStatus, ValuePrecision) {
    let trimmed = value.trim();
    if trimmed == "nil" {
        return (
            ValueKind::Nil,
            ValueStatus::Present,
            ValuePrecision::ExactLocal,
        );
    }
    if matches!(trimmed, "true" | "false") {
        return (
            ValueKind::Bool(trimmed.to_string()),
            ValueStatus::Present,
            ValuePrecision::ExactLocal,
        );
    }
    if trimmed.starts_with('"') || trimmed.starts_with('`') {
        return (
            ValueKind::String(trimmed.to_string()),
            ValueStatus::Present,
            ValuePrecision::ExactLocal,
        );
    }
    if trimmed.starts_with("func") {
        let _ = push_allocation(
            interner,
            operation,
            AllocationKind::FunctionObject,
            "function_object",
            allocations,
        );
        return (
            ValueKind::FunctionObject,
            ValueStatus::Present,
            ValuePrecision::SetupAware,
        );
    }
    if trimmed.contains('{') && trimmed.ends_with('}') {
        let token = push_allocation(
            interner,
            operation,
            AllocationKind::CompositeLiteral,
            "composite_literal",
            allocations,
        );
        return (
            ValueKind::CompositeLiteral(token),
            ValueStatus::Present,
            ValuePrecision::SetupAware,
        );
    }
    if trimmed.parse::<f64>().is_ok() {
        return (
            ValueKind::Number(trimmed.to_string()),
            ValueStatus::Present,
            ValuePrecision::ExactLocal,
        );
    }
    (
        ValueKind::Literal(trimmed.to_string()),
        ValueStatus::Present,
        ValuePrecision::Heuristic,
    )
}

fn push_function_value(
    interner: &crate::core::StableKeyInterner,
    operation: &MirOperation,
    callee: &MirValue,
    values: &mut Vec<ValueFact>,
) {
    if !matches!(callee, MirValue::Unknown { .. }) {
        return;
    }
    values.push(ValueFact {
        id: ValueFactId(values.len() as u64),
        subject: ValueSubject::Operation(operation.id),
        value: AbstractValueId(values.len() as u64),
        kind: ValueKind::FunctionObject,
        language: Language::Go,
        file: Some(operation.span.file),
        function: None,
        body: Some(operation.body),
        precision: ValuePrecision::Heuristic,
        status: ValueStatus::Present,
        provenance: ValueProvenance::Native,
        stable_key: stable_key(
            interner,
            FactFamily::Value,
            [
                ("language", "go".to_string()),
                (
                    "operation",
                    interner.resolve(operation.stable_key).to_string(),
                ),
                ("kind", "function_object".to_string()),
            ],
        ),
    });
}

fn push_allocation(
    interner: &crate::core::StableKeyInterner,
    operation: &MirOperation,
    kind: AllocationKind,
    label: &str,
    allocations: &mut Vec<AllocationTokenFact>,
) -> AllocationTokenId {
    let id = AllocationTokenId(allocations.len() as u64);
    allocations.push(AllocationTokenFact {
        id,
        kind,
        language: Language::Go,
        file: Some(operation.span.file),
        function: None,
        body: Some(operation.body),
        source_place: None,
        source_operation: Some(operation.id),
        span: Some(operation.span.clone()),
        provenance: ValueProvenance::Native,
        stable_key: stable_key(
            interner,
            FactFamily::AllocationToken,
            [
                ("language", "go".to_string()),
                (
                    "operation",
                    interner.resolve(operation.stable_key).to_string(),
                ),
                ("kind", label.to_string()),
            ],
        ),
    });
    id
}

fn unsupported_type_fact(
    interner: &crate::core::StableKeyInterner,
    id: u64,
    subject: TypeSubject,
    unsupported: &crate::analysis::mir::op::UnsupportedSemanticFact,
    body: Option<&crate::analysis::mir::body::MirBody>,
    place: Option<crate::analysis::ids::PlaceId>,
) -> TypeFact {
    let (status, phase, precision, shape) = match unsupported.precision {
        UnsupportedPrecision::Partial | UnsupportedPrecision::Unknown => (
            TypeStatus::Unknown,
            TypePhase::Unknown,
            TypePrecision::Unknown,
            TypeShape::Unknown {
                reason: unsupported.source_evidence.clone(),
            },
        ),
        UnsupportedPrecision::Unsupported => (
            TypeStatus::Unsupported,
            TypePhase::Unsupported,
            TypePrecision::Unsupported,
            TypeShape::Unsupported {
                reason: unsupported.source_evidence.clone(),
            },
        ),
    };
    TypeFact {
        id: TypeFactId(id),
        subject,
        type_set: TypeSetId(id),
        shape,
        phase,
        language: Language::Go,
        file: Some(unsupported.file),
        function: body.map(|body| body.function),
        body: unsupported.body,
        place,
        cfg_block: None,
        operation: unsupported.operation,
        precision,
        confidence: TypeConfidence::Low,
        status,
        provenance: TypeProvenance::Native,
        stable_key: stable_key(
            interner,
            FactFamily::Type,
            [
                ("language", "go".to_string()),
                (
                    "unsupported",
                    interner.resolve(unsupported.stable_key).to_string(),
                ),
                ("ordinal", id.to_string()),
            ],
        ),
    }
}

fn stable_key<const N: usize>(
    interner: &crate::core::StableKeyInterner,
    family: FactFamily,
    parts: [(&'static str, String); N],
) -> String {
    semantic_stable_key(interner, family, &parts).into_string()
}

trait PlaceRootBody {
    fn body(&self) -> Option<crate::analysis::ids::MirBodyId>;
}

impl PlaceRootBody for PlaceRoot {
    fn body(&self) -> Option<crate::analysis::ids::MirBodyId> {
        match self {
            PlaceRoot::Temporary { body, .. } => Some(*body),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ids::{CallSiteId, MirBodyId, MirOpId, PlaceId};
    use crate::analysis::mir::body::{MirBody, MirOutput, MirStatus};
    use crate::analysis::mir::op::{AssignMode, ConservativeAction, UnsupportedDomain};
    use crate::analysis::places::PlaceProjection;
    use crate::core::{FileId, FunctionFact, FunctionId, Span};

    #[test]
    fn go_receiver_selector_index_and_nil_seed_rows_are_emitted() {
        let db = db_with_go_mir();
        let output = derive_go_type_value_alias(&db);

        assert!(
            output
                .types
                .types
                .iter()
                .any(|row| row.language == Language::Go)
        );
        assert!(output.access_paths.access_paths.iter().any(|row| {
            row.language == Language::Go
                && row
                    .projections
                    .iter()
                    .any(|projection| matches!(projection, AccessPathProjection::Field(name) if name == "Tokens"))
                && row
                    .projections
                    .iter()
                    .any(|projection| matches!(projection, AccessPathProjection::IndexUnknown { evidence } if evidence == "index"))
        }));
        assert!(
            output
                .values
                .values
                .iter()
                .any(|row| row.kind == ValueKind::Nil)
        );
    }

    #[test]
    fn go_call_return_access_path_projection_keeps_call_site_domain() {
        let db = db_with_go_mir();
        let output = derive_go_type_value_alias(&db);

        assert!(output.access_paths.access_paths.iter().any(|row| {
            row.projections
                .contains(&AccessPathProjection::CallReturn(CallSiteId(42)))
        }));
    }

    #[test]
    fn go_composite_and_function_values_create_value_and_allocation_rows() {
        let db = db_with_go_mir();
        let output = derive_go_type_value_alias(&db);

        assert!(
            output
                .values
                .allocations
                .iter()
                .any(|row| row.kind == AllocationKind::CompositeLiteral)
        );
        assert!(
            output
                .values
                .values
                .iter()
                .any(|row| row.kind == ValueKind::FunctionObject)
        );
    }

    #[test]
    fn go_place_values_are_not_classified_as_call_returns() {
        let db = db_with_go_mir();
        let output = derive_go_type_value_alias(&db);

        assert!(output.values.values.iter().any(|row| {
            row.kind == ValueKind::PlaceRef(PlaceId(1))
                && row.status == ValueStatus::Present
                && row.precision == ValuePrecision::SetupAware
        }));
        assert!(!output.values.values.iter().any(|row| {
            row.subject == ValueSubject::Place(PlaceId(0))
                && row.kind == ValueKind::CallReturn(PlaceId(1))
        }));
    }

    #[test]
    fn go_unsupported_rows_stay_unknown_or_unsupported() {
        let db = db_with_go_mir();
        let output = derive_go_type_value_alias(&db);

        assert!(output.types.types.iter().any(|row| {
            row.status == TypeStatus::Unsupported
                && matches!(row.shape, TypeShape::Unsupported { .. })
        }));
        assert!(!output.types.types.iter().any(|row| {
            row.stable_key.contains("unsupported")
                && row.status == TypeStatus::Present
                && row.precision == TypePrecision::ExactLocal
        }));
        assert!(output.types.types.iter().any(|row| {
            row.status == TypeStatus::Unsupported
                && row.file == Some(FileId(0))
                && row.function == Some(FunctionId(0))
                && row.body == Some(MirBodyId(0))
                && row.operation == Some(MirOpId(2))
                && row.place == Some(PlaceId(1))
        }));
    }

    fn db_with_go_mir() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let interner = db.stable_key_interner();
        let file = db.add_file(
            "service.go".into(),
            "service.go".to_string(),
            "package service".to_string(),
        );
        let function = db.push_function(FunctionFact {
            id: FunctionId(0),
            file,
            name: "Service.Authorize".to_string(),
            span: Span::point(file, 1, 1),
            language: Language::Go,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        let body = MirBody {
            id: MirBodyId(0),
            language: Language::Go,
            file,
            function,
            package: None,
            module: None,
            owner_stable_key: interner.intern("go:function:Service.Authorize".to_string()),
            span: Span::point(file, 1, 1),
            stable_key: interner.intern("go:body:Service.Authorize".to_string()),
            status: MirStatus::Partial,
        };
        let receiver = PlaceFact {
            id: PlaceId(0),
            language: Language::Go,
            file: Some(file),
            function: Some(function),
            root: PlaceRoot::Parameter {
                function,
                index: 0,
                name: Some("svc".to_string()),
            },
            projections: Vec::new(),
            stable_key: interner.intern("go:place:svc".to_string()),
            status: PlaceStatus::Resolved,
        };
        let selector = PlaceFact {
            id: PlaceId(1),
            projections: vec![
                PlaceProjection::Field("Tokens".to_string()),
                PlaceProjection::IndexUnknown {
                    evidence: "index".to_string(),
                },
            ],
            stable_key: interner.intern("go:place:svc.Tokens[index]".to_string()),
            ..receiver.clone()
        };
        let call_projection = PlaceFact {
            id: PlaceId(2),
            projections: vec![PlaceProjection::CallReturn(CallSiteId(42))],
            stable_key: interner.intern("go:place:svc.call_return".to_string()),
            ..receiver.clone()
        };
        let nil_op = operation(
            &interner,
            MirOpId(0),
            body.id,
            file,
            MirOperationKind::Assign {
                place: receiver.id,
                value: MirValue::Literal {
                    value: "nil".to_string(),
                },
                mode: AssignMode::Overwrite,
            },
        );
        let composite_op = operation(
            &interner,
            MirOpId(1),
            body.id,
            file,
            MirOperationKind::Bind {
                place: selector.id,
                value: MirValue::Literal {
                    value: "User{}".to_string(),
                },
            },
        );
        let function_op = operation(
            &interner,
            MirOpId(2),
            body.id,
            file,
            MirOperationKind::Bind {
                place: selector.id,
                value: MirValue::Literal {
                    value: "func() {}".to_string(),
                },
            },
        );
        let place_copy_op = operation(
            &interner,
            MirOpId(3),
            body.id,
            file,
            MirOperationKind::Assign {
                place: receiver.id,
                value: MirValue::Place(selector.id),
                mode: AssignMode::Overwrite,
            },
        );
        db.replace_semantic_mir(MirOutput {
            bodies: vec![body],
            places: vec![receiver, selector, call_projection],
            operations: vec![nil_op, composite_op, function_op, place_copy_op],
            unsupported: vec![crate::analysis::mir::op::UnsupportedSemanticFact {
                id: crate::analysis::ids::UnsupportedId(0),
                body: Some(MirBodyId(0)),
                operation: Some(MirOpId(2)),
                language: Language::Go,
                file,
                span: Span::point(file, 1, 1),
                construct: "unsafe".to_string(),
                source_evidence: "unsafe.Pointer".to_string(),
                affected_places: vec![PlaceId(1)],
                affected_domains: vec![UnsupportedDomain::Aliases],
                conservative_action: ConservativeAction::HavocAffectedPlaces,
                precision: UnsupportedPrecision::Unsupported,
                status: MirStatus::Unsupported,
                stable_key: interner.intern("go:unsupported:unsafe".to_string()),
            }],
            ..MirOutput::default()
        })
        .expect("semantic MIR replacement");
        db
    }

    fn operation(
        interner: &crate::core::StableKeyInterner,
        id: MirOpId,
        body: MirBodyId,
        file: FileId,
        kind: MirOperationKind,
    ) -> MirOperation {
        MirOperation {
            id,
            body,
            ordinal: id.0 as u32,
            span: Span::point(file, id.0 as u32 + 1, 1),
            kind,
            stable_key: interner.intern(format!("go:op:{}", id.0)),
            status: MirStatus::Partial,
        }
    }
}

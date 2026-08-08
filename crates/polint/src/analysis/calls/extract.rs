use std::collections::BTreeMap;

use crate::analysis::calls::facts::{
    CallCallee, CallPrecision, CallSiteFact, CallSyntaxKind, CallTargetStatus, UnresolvedCallReason,
};
use crate::analysis::ids::{MirBodyId, MirValueId, PlaceId};
use crate::analysis::mir::body::MirBody;
use crate::analysis::mir::op::{MirOperation, MirOperationKind, MirValue};
use crate::analysis::places::{PlaceFact, PlaceProjection, PlaceRoot};
use crate::analysis::stable_key::semantic_stable_key;
use crate::analysis_kernel::{FactFamily, FactRef};
use crate::core::{AnalysisDb, FileId, FunctionFact, FunctionId, Language, Span, SymbolId};

pub(crate) fn extract_call_sites(db: &AnalysisDb) -> Vec<CallSiteFact> {
    let bodies = db
        .mir_bodies()
        .iter()
        .map(|body| (body.id, body))
        .collect::<BTreeMap<_, _>>();
    let places = db
        .mir_places()
        .iter()
        .map(|place| (place.id, place))
        .collect::<BTreeMap<_, _>>();
    let functions = db
        .functions()
        .iter()
        .map(|function| (function.id, function))
        .collect::<BTreeMap<_, _>>();

    let mut call_operations = db
        .mir_operations()
        .iter()
        .filter_map(|operation| bodies.get(&operation.body).map(|body| (*body, operation)))
        .filter(|(_, operation)| matches!(operation.kind, MirOperationKind::Call { .. }))
        .collect::<Vec<_>>();
    call_operations.sort_by(
        |(left_body, left_operation), (right_body, right_operation)| {
            (
                left_body.stable_key.as_str(),
                span_key(&left_operation.span),
                left_operation.ordinal,
                left_operation.stable_key.as_str(),
                left_operation.id,
            )
                .cmp(&(
                    right_body.stable_key.as_str(),
                    span_key(&right_operation.span),
                    right_operation.ordinal,
                    right_operation.stable_key.as_str(),
                    right_operation.id,
                ))
        },
    );

    // Spans of `throw` statements, per file — a call site whose span is contained
    // in one is lexically inside a `throw` argument (error path). The TS/JS MIR
    // lowering records a `throw` unsupported-semantic fact spanning the whole throw
    // statement, so containment marks `f()` in `throw new E(... f() ...)`.
    let mut throw_spans: BTreeMap<FileId, Vec<(u32, u32)>> = BTreeMap::new();
    for fact in db.unsupported_semantics() {
        if fact.construct == "throw" {
            throw_spans
                .entry(fact.file)
                .or_default()
                .push((fact.span.start_byte, fact.span.end_byte));
        }
    }
    let is_in_throw = |file: FileId, span: &Span| -> bool {
        throw_spans.get(&file).is_some_and(|spans| {
            spans
                .iter()
                .any(|(start, end)| *start <= span.start_byte && span.end_byte <= *end)
        })
    };

    let mut same_span_ordinals = BTreeMap::new();
    let mut sites = Vec::with_capacity(call_operations.len());
    for (body, operation) in call_operations {
        let MirOperationKind::Call {
            site,
            callee,
            arguments,
            return_place,
        } = &operation.kind
        else {
            continue;
        };
        let same_span = same_span_ordinal(&mut same_span_ordinals, body.id, &operation.span);
        let (call_callee, receiver, kind, callee_shape) =
            call_callee(callee, body.language, &places);
        let operation_stable_key = if operation.stable_key.trim().is_empty() {
            format!("same_span:{same_span:06}")
        } else {
            operation.stable_key.clone()
        };

        sites.push(CallSiteFact {
            id: *site,
            language: body.language,
            file: body.file,
            caller: body.function,
            owner_symbol: owner_symbol(db, &functions, body.function),
            body: body.id,
            operation: operation.id,
            span: operation.span.clone(),
            kind,
            callee: call_callee,
            receiver,
            arguments: arguments.clone(),
            result: Some(*return_place),
            status: CallTargetStatus::Unresolved,
            precision: CallPrecision::Conservative,
            in_throw: is_in_throw(body.file, &operation.span),
            stable_key: call_site_stable_key(
                db,
                body,
                operation,
                kind,
                &callee_shape,
                &operation_stable_key,
            ),
        });
    }

    sites.sort_by(|left, right| {
        (left.stable_key.as_str(), left.id).cmp(&(right.stable_key.as_str(), right.id))
    });
    sites
}

fn same_span_ordinal(
    ordinals: &mut BTreeMap<(MirBodyId, String), u32>,
    body: MirBodyId,
    span: &Span,
) -> u32 {
    let next = ordinals.entry((body, span_key(span))).or_insert(0);
    let ordinal = *next;
    *next += 1;
    ordinal
}

fn call_callee(
    value: &MirValue,
    language: Language,
    places: &BTreeMap<PlaceId, &PlaceFact>,
) -> (CallCallee, Option<PlaceId>, CallSyntaxKind, String) {
    match value {
        MirValue::Place(place) => place_callee(*place, language, places.get(place).copied()),
        MirValue::Temporary(value) => temporary_callee(*value),
        MirValue::CallReturn(site) => (
            CallCallee::Unknown {
                reason: UnresolvedCallReason::FunctionValue,
            },
            None,
            CallSyntaxKind::FunctionValue,
            format!("call_return:{}", site.0),
        ),
        MirValue::Unknown { evidence } => evidence_callee(evidence, language),
        MirValue::Literal { value } => (
            CallCallee::Unknown {
                reason: UnresolvedCallReason::UnsupportedSyntax,
            },
            None,
            CallSyntaxKind::Unknown,
            format!("literal:{}", value.trim()),
        ),
    }
}

fn evidence_callee(
    evidence: &str,
    language: Language,
) -> (CallCallee, Option<PlaceId>, CallSyntaxKind, String) {
    let evidence = evidence.trim();
    if evidence.is_empty() {
        return unknown_evidence_callee(evidence, UnresolvedCallReason::Unknown);
    }

    if let Some(name) = constructor_evidence_name(evidence) {
        return (
            CallCallee::Constructor {
                reference: None,
                name: Some(name.to_string()),
            },
            None,
            CallSyntaxKind::Constructor,
            format!("constructor:{name}"),
        );
    }

    if evidence.starts_with("import(") {
        return (
            CallCallee::Import,
            None,
            CallSyntaxKind::DynamicImport,
            "dynamic_import".to_string(),
        );
    }

    if evidence.to_ascii_lowercase().contains("dynamicimport") {
        return unknown_evidence_callee(evidence, UnresolvedCallReason::DynamicImport);
    }

    if evidence.to_ascii_lowercase().contains("setupmissing")
        || evidence.to_ascii_lowercase().contains("setup missing")
    {
        return unknown_evidence_callee(evidence, UnresolvedCallReason::SetupMissing);
    }

    if evidence == "eval" {
        return unknown_evidence_callee(evidence, UnresolvedCallReason::Eval);
    }

    if let Some((_, property)) = evidence.rsplit_once('.')
        && is_identifier_like(property)
    {
        if matches!(property, "call" | "apply" | "bind") {
            return unknown_evidence_callee(evidence, UnresolvedCallReason::CallApplyBind);
        }
        let kind = if is_static_member_evidence(language, evidence) {
            CallSyntaxKind::StaticMember
        } else {
            CallSyntaxKind::Member
        };
        return (
            CallCallee::Member {
                base: PlaceId(u64::MAX),
                property: property.to_string(),
            },
            None,
            kind,
            format!("member:{property}"),
        );
    }

    if matches!(evidence, "fn" | "callable" | "callback") {
        return (
            CallCallee::FunctionValue {
                place: PlaceId(u64::MAX),
            },
            None,
            CallSyntaxKind::FunctionValue,
            "function_value".to_string(),
        );
    }

    if crate::ts::is_anonymous_callable_name(evidence) {
        return (
            CallCallee::Identifier {
                reference: None,
                name: evidence.to_string(),
            },
            None,
            CallSyntaxKind::Function,
            format!("identifier:{evidence}"),
        );
    }

    if is_identifier_like(evidence) {
        if is_constructor_name(language, evidence) {
            return (
                CallCallee::Constructor {
                    reference: None,
                    name: Some(evidence.to_string()),
                },
                None,
                CallSyntaxKind::Constructor,
                format!("constructor:{evidence}"),
            );
        }
        return (
            CallCallee::Identifier {
                reference: None,
                name: evidence.to_string(),
            },
            None,
            CallSyntaxKind::Function,
            format!("identifier:{evidence}"),
        );
    }

    unknown_evidence_callee(evidence, UnresolvedCallReason::Unknown)
}

fn unknown_evidence_callee(
    evidence: &str,
    reason: UnresolvedCallReason,
) -> (CallCallee, Option<PlaceId>, CallSyntaxKind, String) {
    (
        CallCallee::Unknown { reason },
        None,
        CallSyntaxKind::Unknown,
        format!("unknown:{}", evidence.trim()),
    )
}

fn constructor_evidence_name(evidence: &str) -> Option<&str> {
    evidence
        .strip_prefix("new ")
        .and_then(|rest| rest.split(['(', '<', ' ']).next())
        .filter(|name| is_identifier_like(name))
}

fn is_static_member_evidence(language: Language, evidence: &str) -> bool {
    matches!(language, Language::TypeScript | Language::JavaScript)
        && evidence
            .split('.')
            .next()
            .is_some_and(|base| base.chars().next().is_some_and(char::is_uppercase))
}

fn is_identifier_like(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
}

fn temporary_callee(value: MirValueId) -> (CallCallee, Option<PlaceId>, CallSyntaxKind, String) {
    (
        CallCallee::Unknown {
            reason: UnresolvedCallReason::MissingSemanticReference,
        },
        None,
        CallSyntaxKind::Unknown,
        format!("temporary:{}", value.0),
    )
}

fn place_callee(
    place: PlaceId,
    language: Language,
    fact: Option<&PlaceFact>,
) -> (CallCallee, Option<PlaceId>, CallSyntaxKind, String) {
    let Some(fact) = fact else {
        return (
            CallCallee::FunctionValue { place },
            Some(place),
            CallSyntaxKind::FunctionValue,
            "function_value".to_string(),
        );
    };

    if let Some((callee, kind, shape)) = projection_callee(place, &fact.projections) {
        return (callee, Some(place), kind, shape);
    }

    match &fact.root {
        PlaceRoot::Global { name, .. } if is_constructor_name(language, name) => (
            CallCallee::Constructor {
                reference: None,
                name: Some(name.clone()),
            },
            None,
            CallSyntaxKind::Constructor,
            format!("constructor:{name}"),
        ),
        PlaceRoot::Global { name, .. } => (
            CallCallee::Identifier {
                reference: None,
                name: name.clone(),
            },
            None,
            CallSyntaxKind::Function,
            format!("identifier:{name}"),
        ),
        PlaceRoot::Unknown { evidence } => (
            CallCallee::Unknown {
                reason: UnresolvedCallReason::Unknown,
            },
            None,
            CallSyntaxKind::Unknown,
            format!("unknown:{}", evidence.trim()),
        ),
        PlaceRoot::Local { .. }
        | PlaceRoot::Parameter { .. }
        | PlaceRoot::Temporary { .. }
        | PlaceRoot::CallReturn { .. } => (
            CallCallee::FunctionValue { place },
            Some(place),
            CallSyntaxKind::FunctionValue,
            "function_value".to_string(),
        ),
    }
}

fn projection_callee(
    place: PlaceId,
    projections: &[PlaceProjection],
) -> Option<(CallCallee, CallSyntaxKind, String)> {
    let projection = projections.last()?;
    match projection {
        PlaceProjection::Field(property) | PlaceProjection::Property(property) => Some((
            CallCallee::Member {
                base: place,
                property: property.clone(),
            },
            CallSyntaxKind::Member,
            format!("member:{property}"),
        )),
        PlaceProjection::IndexKnown(index) => Some((
            CallCallee::Index {
                base: place,
                index: None,
            },
            CallSyntaxKind::Index,
            format!("index_known:{index}"),
        )),
        PlaceProjection::IndexUnknown { evidence } => Some((
            CallCallee::Index {
                base: place,
                index: None,
            },
            CallSyntaxKind::Index,
            format!("index_unknown:{}", evidence.trim()),
        )),
        PlaceProjection::CallReturn(call) => Some((
            CallCallee::Unknown {
                reason: UnresolvedCallReason::FunctionValue,
            },
            CallSyntaxKind::FunctionValue,
            format!("call_return:{}", call.0),
        )),
        PlaceProjection::Unknown { evidence } => Some((
            CallCallee::Unknown {
                reason: UnresolvedCallReason::Unknown,
            },
            CallSyntaxKind::Unknown,
            format!("unknown_projection:{}", evidence.trim()),
        )),
        PlaceProjection::Deref | PlaceProjection::AwaitResult => None,
    }
}

fn call_site_stable_key(
    db: &AnalysisDb,
    body: &MirBody,
    operation: &MirOperation,
    kind: CallSyntaxKind,
    callee_shape: &str,
    operation_stable_key: &str,
) -> String {
    semantic_stable_key(
        FactFamily::CallSite,
        &[
            ("language", format!("{:?}", body.language)),
            ("file_key", file_key(db, body.file)),
            ("caller_key", caller_key(db, body.function)),
            ("span", span_key(&operation.span)),
            ("callee_shape", callee_shape.to_string()),
            ("operation_key", operation_stable_key.to_string()),
            ("call_kind", format!("{kind:?}")),
        ],
    )
    .into_string()
}

fn owner_symbol(
    db: &AnalysisDb,
    functions: &BTreeMap<FunctionId, &FunctionFact>,
    function: FunctionId,
) -> Option<SymbolId> {
    let function = functions.get(&function)?;
    db.symbols()
        .iter()
        .find(|symbol| {
            symbol.file == Some(function.file)
                && symbol.name == function.name
                && symbol.primary_span.as_ref() == Some(&function.span)
        })
        .map(|symbol| symbol.id)
}

fn file_key(db: &AnalysisDb, file: FileId) -> String {
    db.metadata_for(FactRef::new(FactFamily::SourceFile, u64::from(file.0)))
        .map(|metadata| metadata.stable_key.clone())
        .or_else(|| {
            db.files()
                .iter()
                .find(|source_file| source_file.id == file)
                .map(|source_file| source_file.relative_path.replace('\\', "/"))
        })
        .unwrap_or_else(|| format!("<missing-file:{}>", file.0))
}

fn caller_key(db: &AnalysisDb, function: FunctionId) -> String {
    db.metadata_for(FactRef::new(FactFamily::Function, function.0))
        .map(|metadata| metadata.stable_key.clone())
        .unwrap_or_else(|| format!("<missing-function:{}>", function.0))
}

fn span_key(span: &Span) -> String {
    format!(
        "{}:{}..{}:{}@{}..{}",
        span.start_line,
        span.start_col,
        span.end_line,
        span.end_col,
        span.start_byte,
        span.end_byte
    )
}

fn is_constructor_name(language: Language, name: &str) -> bool {
    matches!(language, Language::TypeScript | Language::JavaScript)
        && name.chars().next().is_some_and(char::is_uppercase)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::analysis::calls::facts::{
        CallCallee, CallPrecision, CallSyntaxKind, CallTargetStatus,
    };
    use crate::analysis::ids::{CallSiteId, MirBodyId, MirOpId, PlaceId};
    use crate::analysis::mir::body::{MirBody, MirOutput, MirStatus};
    use crate::analysis::mir::op::{MirOperation, MirOperationKind, MirValue};
    use crate::analysis::places::{PlaceFact, PlaceProjection, PlaceRoot, PlaceStatus};
    use crate::core::{AnalysisDb, FileId, FunctionFact, FunctionId, Language, Span};
    use crate::ts::anonymous_callable_name;

    fn span(file: FileId, line: u32, start_byte: u32) -> Span {
        Span {
            file,
            start_byte,
            end_byte: start_byte + 4,
            start_line: line,
            start_col: 1,
            end_line: line,
            end_col: 5,
        }
    }

    fn add_file_and_function(db: &mut AnalysisDb, relative_path: &str) -> (FileId, FunctionId) {
        let file = db.add_file(
            PathBuf::from(relative_path),
            relative_path.to_string(),
            "function caller() {}".to_string(),
        );
        let function = db.push_function(FunctionFact {
            id: FunctionId(999),
            file,
            name: "caller".to_string(),
            span: span(file, 1, 0),
            language: Language::TypeScript,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        (file, function)
    }

    fn body(file: FileId, function: FunctionId, language: Language) -> MirBody {
        MirBody {
            id: MirBodyId(1),
            language,
            file,
            function,
            package: None,
            module: None,
            owner_stable_key: "function:caller:stable".to_string(),
            span: span(file, 1, 0),
            stable_key: "mir-body:caller".to_string(),
            status: MirStatus::Resolved,
        }
    }

    fn place(
        id: u64,
        file: FileId,
        function: FunctionId,
        root: PlaceRoot,
        projections: Vec<PlaceProjection>,
    ) -> PlaceFact {
        PlaceFact {
            id: PlaceId(id),
            language: Language::TypeScript,
            file: Some(file),
            function: Some(function),
            root,
            projections,
            stable_key: format!("place:{id}"),
            status: PlaceStatus::Resolved,
        }
    }

    fn call_op(
        id: u64,
        ordinal: u32,
        file: FileId,
        site: u64,
        callee: MirValue,
        arguments: Vec<PlaceId>,
    ) -> MirOperation {
        MirOperation {
            id: MirOpId(id),
            body: MirBodyId(1),
            ordinal,
            span: span(file, 2, 10),
            kind: MirOperationKind::Call {
                site: CallSiteId(site),
                callee,
                arguments,
                return_place: PlaceId(9),
            },
            stable_key: format!("mir-op:call:{id}"),
            status: MirStatus::Resolved,
        }
    }

    #[test]
    fn extract_call_sites_maps_mir_calls_to_complete_call_site_facts() {
        let mut db = AnalysisDb::new();
        let (file, function) = add_file_and_function(&mut db, "src/app.ts");
        db.replace_semantic_mir(MirOutput {
            bodies: vec![body(file, function, Language::TypeScript)],
            places: vec![
                place(
                    1,
                    file,
                    function,
                    PlaceRoot::Global {
                        symbol: None,
                        name: "run".to_string(),
                    },
                    Vec::new(),
                ),
                place(
                    2,
                    file,
                    function,
                    PlaceRoot::Local {
                        function,
                        name: "arg".to_string(),
                    },
                    Vec::new(),
                ),
                place(
                    9,
                    file,
                    function,
                    PlaceRoot::CallReturn {
                        call: CallSiteId(10),
                    },
                    Vec::new(),
                ),
            ],
            operations: vec![call_op(
                1,
                0,
                file,
                10,
                MirValue::Place(PlaceId(1)),
                vec![PlaceId(2)],
            )],
            unsupported: Vec::new(),
            ..MirOutput::default()
        })
        .expect("semantic MIR should store");

        let sites = super::extract_call_sites(&db);

        assert_eq!(sites.len(), 1);
        let site = &sites[0];
        assert_eq!(site.id, CallSiteId(10));
        assert_eq!(site.language, Language::TypeScript);
        assert_eq!(site.file, file);
        assert_eq!(site.caller, function);
        assert_eq!(site.body, MirBodyId(0));
        assert_eq!(site.operation, MirOpId(0));
        assert_eq!(site.kind, CallSyntaxKind::Function);
        assert_eq!(
            site.callee,
            CallCallee::Identifier {
                reference: None,
                name: "run".to_string()
            }
        );
        assert_eq!(site.arguments, vec![PlaceId(1)]);
        assert_eq!(site.result, Some(PlaceId(2)));
        assert_eq!(site.status, CallTargetStatus::Unresolved);
        assert_eq!(site.precision, CallPrecision::Conservative);
    }

    #[test]
    fn extract_call_sites_treats_anonymous_callable_evidence_as_lexical_callee() {
        let mut db = AnalysisDb::new();
        let (file, function) = add_file_and_function(&mut db, "src/iife.ts");
        let anonymous = anonymous_callable_name(1, 14);
        db.replace_semantic_mir(MirOutput {
            bodies: vec![body(file, function, Language::TypeScript)],
            places: vec![place(
                9,
                file,
                function,
                PlaceRoot::CallReturn {
                    call: CallSiteId(10),
                },
                Vec::new(),
            )],
            operations: vec![call_op(
                1,
                0,
                file,
                10,
                MirValue::Unknown {
                    evidence: anonymous.clone(),
                },
                Vec::new(),
            )],
            unsupported: Vec::new(),
            ..MirOutput::default()
        })
        .expect("semantic MIR should store");

        let sites = super::extract_call_sites(&db);

        assert_eq!(
            sites[0].callee,
            CallCallee::Identifier {
                reference: None,
                name: anonymous
            }
        );
        assert_eq!(sites[0].kind, CallSyntaxKind::Function);
    }

    #[test]
    fn extract_call_sites_stable_key_uses_required_stable_inputs() {
        let mut db = AnalysisDb::new();
        let (file, function) = add_file_and_function(&mut db, "src/app.ts");
        db.replace_semantic_mir(MirOutput {
            bodies: vec![body(file, function, Language::TypeScript)],
            places: vec![
                place(
                    1,
                    file,
                    function,
                    PlaceRoot::Local {
                        function,
                        name: "callback".to_string(),
                    },
                    Vec::new(),
                ),
                place(
                    9,
                    file,
                    function,
                    PlaceRoot::CallReturn {
                        call: CallSiteId(10),
                    },
                    Vec::new(),
                ),
            ],
            operations: vec![call_op(
                1,
                0,
                file,
                10,
                MirValue::Place(PlaceId(1)),
                Vec::new(),
            )],
            unsupported: Vec::new(),
            ..MirOutput::default()
        })
        .expect("semantic MIR should store");

        let sites = super::extract_call_sites(&db);
        let stable_key = &sites[0].stable_key;

        assert!(stable_key.contains("8:CallSite"));
        assert!(stable_key.contains("8:language=10:TypeScript"));
        assert!(stable_key.contains("8:file_key="));
        assert!(stable_key.contains("10:caller_key="));
        assert!(stable_key.contains("4:span="));
        assert!(stable_key.contains("12:callee_shape=14:function_value"));
        assert!(stable_key.contains("13:operation_key=13:mir-op:call:1"));
        assert!(stable_key.contains("9:call_kind=13:FunctionValue"));
    }

    #[test]
    fn extract_call_sites_is_deterministic_for_different_operation_orders() {
        let mut first = AnalysisDb::new();
        let (file, function) = add_file_and_function(&mut first, "src/app.ts");
        let output = MirOutput {
            bodies: vec![body(file, function, Language::TypeScript)],
            places: vec![
                place(
                    1,
                    file,
                    function,
                    PlaceRoot::Global {
                        symbol: None,
                        name: "alpha".to_string(),
                    },
                    Vec::new(),
                ),
                place(
                    2,
                    file,
                    function,
                    PlaceRoot::Global {
                        symbol: None,
                        name: "beta".to_string(),
                    },
                    Vec::new(),
                ),
                place(
                    9,
                    file,
                    function,
                    PlaceRoot::CallReturn {
                        call: CallSiteId(10),
                    },
                    Vec::new(),
                ),
            ],
            operations: vec![
                call_op(2, 1, file, 20, MirValue::Place(PlaceId(2)), Vec::new()),
                call_op(1, 0, file, 10, MirValue::Place(PlaceId(1)), Vec::new()),
            ],
            unsupported: Vec::new(),
            ..MirOutput::default()
        };
        first
            .replace_semantic_mir(output.clone())
            .expect("semantic MIR should store");

        let mut second = AnalysisDb::new();
        let (second_file, second_function) = add_file_and_function(&mut second, "src/app.ts");
        let mut reordered = output;
        reordered.bodies = vec![body(second_file, second_function, Language::TypeScript)];
        reordered.operations.reverse();
        second
            .replace_semantic_mir(reordered)
            .expect("semantic MIR should store");

        let first_keys = super::extract_call_sites(&first)
            .into_iter()
            .map(|site| site.stable_key)
            .collect::<Vec<_>>();
        let second_keys = super::extract_call_sites(&second)
            .into_iter()
            .map(|site| site.stable_key)
            .collect::<Vec<_>>();

        assert_eq!(first_keys, second_keys);
    }
}

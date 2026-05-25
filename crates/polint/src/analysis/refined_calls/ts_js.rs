use super::facts::{
    RefinedCallConfidence, RefinedCallEdgeFact, RefinedCallTier, RefinedCallValidation,
};
use super::store::RefinedCallOutput;
use crate::analysis::calls::facts::{
    CallAlgorithm, CallCallee, CallEdgeKind, CallPrecision, CallProvenance, CallTargetFact,
    CallTargetStatus, UnresolvedCallFact, UnresolvedCallReason,
};
use crate::analysis::ids::{CallSiteId, PlaceId, RefinedCallEdgeId};
use crate::analysis::points_to::facts::{PointsToBudgetStatus, PointsToSetFact, PointsToStatus};
use crate::analysis::points_to::vars::place_var;
use crate::analysis::types::facts::{TypeFact, TypePrecision, TypeShape, TypeStatus, TypeSubject};
use crate::analysis::values::facts::{ValueFact, ValueKind, ValueStatus, ValueSubject};
use crate::analysis_kernel::{FactFamily, FactRef, stable_key_from_parts};
use crate::core::{AnalysisDb, Language};

pub(crate) fn derive_ts_js_refinements(db: &AnalysisDb) -> RefinedCallOutput {
    let mut edges = Vec::new();

    for site in db
        .call_sites()
        .iter()
        .filter(|site| site.language.is_ts_family())
    {
        let Some(place) = callable_place(site) else {
            continue;
        };
        let callable_types = callable_type_facts_for_place(db, place);
        let callable_values = value_facts_for_place(db, place);
        let points_to_sets = points_to_sets_for_place(db, place);

        for target in db
            .call_targets()
            .iter()
            .filter(|target| target.site == site.id)
        {
            if let Some(type_fact) = callable_types
                .iter()
                .find(|type_fact| is_callable_type(type_fact))
            {
                edges.push(callable_type_edge(db, target, type_fact, edges.len()));
            }
            if let Some(value) = callable_values
                .iter()
                .find(|value| is_callable_value(value))
            {
                edges.push(function_token_edge(db, target, value, edges.len()));
            }
            if let Some(points_to) = points_to_sets.iter().find(|set| {
                set.status == PointsToStatus::Present
                    && set.budget == PointsToBudgetStatus::WithinBudget
            }) {
                edges.push(points_to_edge(db, target, points_to, edges.len()));
            }
        }

        if points_to_sets
            .iter()
            .any(|set| set.budget == PointsToBudgetStatus::BudgetExceeded)
        {
            edges.push(points_to_budget_edge(site, place, edges.len()));
        }
    }

    for unresolved in db.unresolved_calls().iter().filter(|unresolved| {
        matches!(
            unresolved.reason,
            UnresolvedCallReason::FunctionValue
                | UnresolvedCallReason::DynamicProperty
                | UnresolvedCallReason::Eval
                | UnresolvedCallReason::CallApplyBind
                | UnresolvedCallReason::ProxyOrAccessor
                | UnresolvedCallReason::MissingImportResolution
                | UnresolvedCallReason::SetupMissing
                | UnresolvedCallReason::BudgetExceeded
        )
    }) {
        if let Some(site) = db
            .call_sites()
            .iter()
            .find(|site| site.id == unresolved.site && site.language.is_ts_family())
        {
            let status = match unresolved.reason {
                UnresolvedCallReason::SetupMissing => CallTargetStatus::SetupMissing,
                UnresolvedCallReason::BudgetExceeded => CallTargetStatus::BudgetExceeded,
                UnresolvedCallReason::UnsupportedSyntax => CallTargetStatus::Unsupported,
                _ => CallTargetStatus::Unresolved,
            };
            edges.push(unresolved_ts_js_edge(
                db,
                unresolved,
                status,
                unresolved.reason,
                callable_place(site),
                edges.len(),
            ));
        }
    }

    RefinedCallOutput { edges }.normalized()
}

fn callable_place(site: &crate::analysis::calls::facts::CallSiteFact) -> Option<PlaceId> {
    match site.callee {
        CallCallee::FunctionValue { place } => Some(place),
        _ => site.receiver,
    }
}

fn function_token_edge(
    db: &AnalysisDb,
    target: &CallTargetFact,
    value: &ValueFact,
    index: usize,
) -> RefinedCallEdgeFact {
    let value_key = metadata_key(db, FactFamily::Value, value.id.0, &value.stable_key);
    let target_key = metadata_key(db, FactFamily::CallTarget, target.id.0, &target.stable_key);
    edge_from_target(
        db,
        target,
        RefinedCallEdgeId(index as u64),
        RefinedCallTier::TypeValueFunctionToken,
        CallAlgorithm::FunctionTokenFlow,
        value_precision(value),
        vec![
            "ts_js_function_token".to_string(),
            format!("value={value_key}"),
        ],
        vec![target_key.clone(), value_key.clone()],
        stable_key_from_parts(
            FactFamily::RefinedCallEdge,
            &[
                ("tier", "ts_js_function_token".to_string()),
                ("base_target", target_key),
                ("value", value_key),
            ],
        ),
    )
}

fn callable_type_edge(
    db: &AnalysisDb,
    target: &CallTargetFact,
    type_fact: &TypeFact,
    index: usize,
) -> RefinedCallEdgeFact {
    let type_key = metadata_key(db, FactFamily::Type, type_fact.id.0, &type_fact.stable_key);
    let target_key = metadata_key(db, FactFamily::CallTarget, target.id.0, &target.stable_key);
    edge_from_target(
        db,
        target,
        RefinedCallEdgeId(index as u64),
        RefinedCallTier::TypeValueFunctionToken,
        CallAlgorithm::FunctionTokenFlow,
        type_precision(type_fact.precision),
        vec![
            "ts_js_callable_type".to_string(),
            format!("type={type_key}"),
        ],
        vec![target_key.clone(), type_key.clone()],
        stable_key_from_parts(
            FactFamily::RefinedCallEdge,
            &[
                ("tier", "ts_js_callable_type".to_string()),
                ("base_target", target_key),
                ("type", type_key),
            ],
        ),
    )
}

fn points_to_edge(
    db: &AnalysisDb,
    target: &CallTargetFact,
    points_to: &PointsToSetFact,
    index: usize,
) -> RefinedCallEdgeFact {
    let points_to_key = metadata_key(
        db,
        FactFamily::PointsToSet,
        points_to.id.0,
        &points_to.stable_key,
    );
    let target_key = metadata_key(db, FactFamily::CallTarget, target.id.0, &target.stable_key);
    edge_from_target(
        db,
        target,
        RefinedCallEdgeId(index as u64),
        RefinedCallTier::PointsToAssisted,
        CallAlgorithm::PointsTo,
        CallPrecision::Conservative,
        vec![
            "ts_js_points_to".to_string(),
            format!("points_to={points_to_key}"),
        ],
        vec![target_key.clone(), points_to_key.clone()],
        stable_key_from_parts(
            FactFamily::RefinedCallEdge,
            &[
                ("tier", "ts_js_points_to".to_string()),
                ("base_target", target_key),
                ("points_to", points_to_key),
            ],
        ),
    )
}

fn points_to_budget_edge(
    site: &crate::analysis::calls::facts::CallSiteFact,
    place: PlaceId,
    index: usize,
) -> RefinedCallEdgeFact {
    RefinedCallEdgeFact {
        id: RefinedCallEdgeId(index as u64),
        site: site.id,
        base_target: None,
        caller: site.caller,
        target_function: None,
        target_symbol: None,
        synthetic_target: Some(format!("ts-js:points-to-budget:{}", place.0)),
        language: site.language,
        edge_kind: CallEdgeKind::Unknown,
        algorithm: CallAlgorithm::PointsTo,
        tier: RefinedCallTier::PointsToAssisted,
        status: CallTargetStatus::BudgetExceeded,
        reason: Some(UnresolvedCallReason::BudgetExceeded),
        provenance: CallProvenance::Native,
        precision: CallPrecision::Unknown,
        validation: RefinedCallValidation::ReferentiallyValidated,
        confidence: RefinedCallConfidence::Low,
        evidence: vec!["ts_js_points_to_budget_exceeded".to_string()],
        input_stable_keys: vec![site.stable_key.clone()],
        stable_key: stable_key_from_parts(
            FactFamily::RefinedCallEdge,
            &[
                ("tier", "ts_js_points_to_budget".to_string()),
                ("site", site.stable_key.clone()),
                ("place", place.0.to_string()),
            ],
        ),
    }
}

fn edge_from_target(
    db: &AnalysisDb,
    target: &CallTargetFact,
    id: RefinedCallEdgeId,
    tier: RefinedCallTier,
    algorithm: CallAlgorithm,
    precision: CallPrecision,
    evidence: Vec<String>,
    input_stable_keys: Vec<String>,
    stable_key: String,
) -> RefinedCallEdgeFact {
    RefinedCallEdgeFact {
        id,
        site: target.site,
        base_target: Some(target.id),
        caller: target.caller,
        target_function: target.target_function,
        target_symbol: target.target_symbol,
        synthetic_target: None,
        language: db
            .call_sites()
            .iter()
            .find(|site| site.id == target.site)
            .map(|site| site.language)
            .unwrap_or(Language::Unknown),
        edge_kind: target.edge_kind,
        algorithm,
        tier,
        status: target.status,
        reason: target.reason,
        provenance: CallProvenance::Native,
        precision,
        validation: RefinedCallValidation::ReferentiallyValidated,
        confidence: confidence_for_status(target.status),
        evidence,
        input_stable_keys,
        stable_key,
    }
}

fn unresolved_ts_js_edge(
    db: &AnalysisDb,
    unresolved: &UnresolvedCallFact,
    status: CallTargetStatus,
    reason: UnresolvedCallReason,
    place: Option<PlaceId>,
    index: usize,
) -> RefinedCallEdgeFact {
    let unresolved_key = unresolved.stable_key.clone();
    RefinedCallEdgeFact {
        id: RefinedCallEdgeId(index as u64),
        site: unresolved.site,
        base_target: None,
        caller: unresolved.caller,
        target_function: None,
        target_symbol: None,
        synthetic_target: place.map(|place| format!("ts-js:callable-place:{}", place.0)),
        language: db
            .call_sites()
            .iter()
            .find(|site| site.id == unresolved.site)
            .map(|site| site.language)
            .unwrap_or(Language::Unknown),
        edge_kind: CallEdgeKind::Unknown,
        algorithm: CallAlgorithm::FunctionTokenFlow,
        tier: RefinedCallTier::TypeValueFunctionToken,
        status,
        reason: Some(reason),
        provenance: CallProvenance::Native,
        precision: unresolved.precision,
        validation: RefinedCallValidation::ReferentiallyValidated,
        confidence: RefinedCallConfidence::Low,
        evidence: vec!["ts_js_unresolved_callable".to_string()],
        input_stable_keys: vec![unresolved_key.clone()],
        stable_key: stable_key_from_parts(
            FactFamily::RefinedCallEdge,
            &[
                ("tier", "ts_js_unresolved".to_string()),
                ("unresolved", unresolved_key),
                ("status", format!("{status:?}")),
            ],
        ),
    }
}

fn value_facts_for_place(db: &AnalysisDb, place: PlaceId) -> Vec<&ValueFact> {
    db.value_facts()
        .iter()
        .filter(|fact| matches!(fact.subject, ValueSubject::Place(subject) if subject == place))
        .collect()
}

fn callable_type_facts_for_place(db: &AnalysisDb, place: PlaceId) -> Vec<&TypeFact> {
    db.type_facts()
        .iter()
        .filter(|fact| matches!(fact.subject, TypeSubject::Place(subject) if subject == place))
        .collect()
}

fn points_to_sets_for_place(db: &AnalysisDb, place: PlaceId) -> Vec<&PointsToSetFact> {
    let var = place_var(place);
    db.points_to_sets()
        .iter()
        .filter(|set| set.variable == var)
        .collect()
}

fn is_callable_type(type_fact: &&TypeFact) -> bool {
    type_fact.status == TypeStatus::Present
        && matches!(
            &type_fact.shape,
            TypeShape::Callable { .. } | TypeShape::Class { .. }
        )
}

fn is_callable_value(value: &&ValueFact) -> bool {
    value.status == ValueStatus::Present
        && matches!(
            &value.kind,
            ValueKind::FunctionObject | ValueKind::PlaceRef(_) | ValueKind::CallReturn(_)
        )
}

fn type_precision(precision: TypePrecision) -> CallPrecision {
    match precision {
        TypePrecision::ExactLocal | TypePrecision::SetupAware => CallPrecision::SetupAware,
        TypePrecision::Conservative => CallPrecision::Conservative,
        TypePrecision::Heuristic => CallPrecision::Heuristic,
        TypePrecision::Unknown => CallPrecision::Unknown,
        TypePrecision::Unsupported => CallPrecision::Unsupported,
    }
}

fn value_precision(value: &ValueFact) -> CallPrecision {
    match value.precision {
        crate::analysis::values::facts::ValuePrecision::ExactLocal
        | crate::analysis::values::facts::ValuePrecision::SetupAware => CallPrecision::SetupAware,
        crate::analysis::values::facts::ValuePrecision::Conservative => CallPrecision::Conservative,
        crate::analysis::values::facts::ValuePrecision::Heuristic => CallPrecision::Heuristic,
        crate::analysis::values::facts::ValuePrecision::Unknown => CallPrecision::Unknown,
        crate::analysis::values::facts::ValuePrecision::Unsupported => CallPrecision::Unsupported,
    }
}

fn confidence_for_status(status: CallTargetStatus) -> RefinedCallConfidence {
    match status {
        CallTargetStatus::Resolved => RefinedCallConfidence::High,
        CallTargetStatus::Ambiguous => RefinedCallConfidence::Medium,
        CallTargetStatus::Unresolved
        | CallTargetStatus::Unsupported
        | CallTargetStatus::SetupMissing
        | CallTargetStatus::BudgetExceeded
        | CallTargetStatus::Rejected => RefinedCallConfidence::Low,
    }
}

fn metadata_key(db: &AnalysisDb, family: FactFamily, run_id: u64, fallback: &str) -> String {
    db.metadata_for(FactRef::new(family, run_id))
        .map(|metadata| metadata.stable_key.clone())
        .unwrap_or_else(|| fallback.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::calls::facts::{
        CallCallee, CallPrecision, CallSiteFact, CallSyntaxKind, CallTargetFact,
    };
    use crate::analysis::calls::store::CallOutput;
    use crate::analysis::ids::{
        CallTargetId, MirBodyId, MirOpId, PointsToSetId, TypeFactId, TypeSetId, ValueFactId,
    };
    use crate::analysis::points_to::facts::{PointsToPrecision, PointsToSetFact};
    use crate::analysis::points_to::store::PointsToOutput;
    use crate::analysis::types::facts::{
        TypeConfidence, TypeFact, TypePhase, TypePrecision, TypeProvenance,
    };
    use crate::analysis::types::store::{TypeOutput, TypeValueAliasOutput};
    use crate::analysis::values::facts::{ValueFact, ValuePrecision, ValueProvenance};
    use crate::analysis::values::store::ValueOutput;
    use crate::core::{FileId, FunctionFact, FunctionId, Span, SymbolId};

    #[test]
    fn function_value_call_can_create_function_token_edge() {
        let mut db = ts_db_with_target();
        db.replace_type_value_alias_facts(TypeValueAliasOutput {
            values: ValueOutput {
                values: vec![callable_value(ValueKind::FunctionObject)],
                allocations: Vec::new(),
            },
            ..TypeValueAliasOutput::default()
        });

        let output = derive_ts_js_refinements(&db);

        assert_eq!(output.edges.len(), 1);
        assert_eq!(
            output.edges[0].tier,
            RefinedCallTier::TypeValueFunctionToken
        );
        assert_eq!(output.edges[0].algorithm, CallAlgorithm::FunctionTokenFlow);
    }

    #[test]
    fn callable_type_can_create_function_token_edge() {
        let mut db = ts_db_with_target();
        db.replace_type_value_alias_facts(TypeValueAliasOutput {
            types: TypeOutput {
                types: vec![callable_type()],
                narrowed: Vec::new(),
            },
            ..TypeValueAliasOutput::default()
        });

        let output = derive_ts_js_refinements(&db);

        assert_eq!(output.edges.len(), 1);
        assert_eq!(
            output.edges[0].tier,
            RefinedCallTier::TypeValueFunctionToken
        );
        assert!(
            output.edges[0]
                .evidence
                .iter()
                .any(|item| item == "ts_js_callable_type")
        );
    }

    #[test]
    fn dynamic_property_unresolved_call_remains_unknown() {
        let mut db = ts_db_base();
        db.replace_call_facts(CallOutput {
            sites: vec![ts_call_site()],
            targets: Vec::new(),
            unresolved: vec![UnresolvedCallFact {
                site: CallSiteId(0),
                caller: FunctionId(0),
                status: CallTargetStatus::Unresolved,
                reason: UnresolvedCallReason::DynamicProperty,
                algorithm: CallAlgorithm::Unsupported,
                provenance: CallProvenance::Native,
                precision: CallPrecision::Unknown,
                stable_key: "unresolved:dynamic".to_string(),
            }],
        })
        .expect("valid call facts");

        let output = derive_ts_js_refinements(&db);

        assert_eq!(output.edges.len(), 1);
        assert_eq!(output.edges[0].status, CallTargetStatus::Unresolved);
        assert_eq!(
            output.edges[0].reason,
            Some(UnresolvedCallReason::DynamicProperty)
        );
    }

    #[test]
    fn within_budget_points_to_can_create_ts_js_points_to_edge() {
        let mut db = ts_db_with_target();
        db.replace_type_value_alias_facts(TypeValueAliasOutput {
            points_to: PointsToOutput {
                constraints: Vec::new(),
                sets: vec![points_to_set(
                    PointsToStatus::Present,
                    PointsToBudgetStatus::WithinBudget,
                )],
            },
            ..TypeValueAliasOutput::default()
        });

        let output = derive_ts_js_refinements(&db);

        assert_eq!(output.edges.len(), 1);
        assert_eq!(output.edges[0].tier, RefinedCallTier::PointsToAssisted);
    }

    #[test]
    fn budget_exceeded_points_to_creates_explicit_budget_row() {
        let mut db = ts_db_with_target();
        db.replace_type_value_alias_facts(TypeValueAliasOutput {
            points_to: PointsToOutput {
                constraints: Vec::new(),
                sets: vec![points_to_set(
                    PointsToStatus::Present,
                    PointsToBudgetStatus::BudgetExceeded,
                )],
            },
            ..TypeValueAliasOutput::default()
        });

        let output = derive_ts_js_refinements(&db);

        assert_eq!(output.edges.len(), 1);
        assert_eq!(output.edges[0].status, CallTargetStatus::BudgetExceeded);
        assert_eq!(output.edges[0].algorithm, CallAlgorithm::PointsTo);
    }

    fn ts_db_with_target() -> AnalysisDb {
        let mut db = ts_db_base();
        db.replace_call_facts(CallOutput {
            sites: vec![ts_call_site()],
            targets: vec![ts_target()],
            unresolved: Vec::new(),
        })
        .expect("valid call facts");
        db
    }

    fn ts_db_base() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            "src/app.ts".into(),
            "src/app.ts".to_string(),
            "function caller(fn) { fn(); } function callee() {}\n".to_string(),
        );
        db.push_function(FunctionFact {
            id: FunctionId(0),
            file,
            name: "caller".to_string(),
            span: span(),
            language: Language::TypeScript,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: vec!["fn".to_string()],
        });
        db.push_function(FunctionFact {
            id: FunctionId(1),
            file,
            name: "callee".to_string(),
            span: span(),
            language: Language::TypeScript,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        db
    }

    fn ts_call_site() -> CallSiteFact {
        CallSiteFact {
            id: CallSiteId(0),
            language: Language::TypeScript,
            file: FileId(0),
            caller: FunctionId(0),
            owner_symbol: None,
            body: MirBodyId(0),
            operation: MirOpId(0),
            span: span(),
            kind: CallSyntaxKind::FunctionValue,
            callee: CallCallee::FunctionValue { place: PlaceId(0) },
            receiver: None,
            arguments: Vec::new(),
            result: None,
            status: CallTargetStatus::Ambiguous,
            precision: CallPrecision::Unknown,
            stable_key: "call-site:fn".to_string(),
        }
    }

    fn ts_target() -> CallTargetFact {
        CallTargetFact {
            id: CallTargetId(0),
            site: CallSiteId(0),
            caller: FunctionId(0),
            target_function: Some(FunctionId(1)),
            target_symbol: Some(SymbolId(0)),
            edge_kind: CallEdgeKind::FunctionValue,
            algorithm: CallAlgorithm::FunctionTokenFlow,
            status: CallTargetStatus::Resolved,
            reason: None,
            provenance: CallProvenance::Native,
            precision: CallPrecision::SetupAware,
            stable_key: "call-target:fn".to_string(),
        }
    }

    fn callable_value(kind: ValueKind) -> ValueFact {
        ValueFact {
            id: ValueFactId(0),
            subject: ValueSubject::Place(PlaceId(0)),
            value: crate::analysis::ids::AbstractValueId(0),
            kind,
            language: Language::TypeScript,
            file: Some(FileId(0)),
            function: Some(FunctionId(0)),
            body: None,
            precision: ValuePrecision::SetupAware,
            status: ValueStatus::Present,
            provenance: ValueProvenance::Native,
            stable_key: "value:function-token".to_string(),
        }
    }

    fn callable_type() -> TypeFact {
        TypeFact {
            id: TypeFactId(0),
            subject: TypeSubject::Place(PlaceId(0)),
            type_set: TypeSetId(0),
            shape: TypeShape::Callable {
                signature: "(...args) => unknown".to_string(),
            },
            phase: TypePhase::Inferred,
            language: Language::TypeScript,
            file: Some(FileId(0)),
            function: Some(FunctionId(0)),
            body: None,
            place: Some(PlaceId(0)),
            cfg_block: None,
            operation: None,
            precision: TypePrecision::SetupAware,
            confidence: TypeConfidence::Medium,
            status: TypeStatus::Present,
            provenance: TypeProvenance::Native,
            stable_key: "type:callable".to_string(),
        }
    }

    fn points_to_set(status: PointsToStatus, budget: PointsToBudgetStatus) -> PointsToSetFact {
        PointsToSetFact {
            id: PointsToSetId(0),
            variable: place_var(PlaceId(0)),
            objects: Vec::new(),
            status,
            precision: PointsToPrecision::FlowInsensitive,
            budget,
            stable_key: "points-to:fn".to_string(),
        }
    }

    fn span() -> Span {
        Span::point(FileId(0), 1, 1)
    }
}

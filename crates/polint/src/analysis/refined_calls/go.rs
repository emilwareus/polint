use super::facts::{
    RefinedCallConfidence, RefinedCallEdgeFact, RefinedCallTier, RefinedCallValidation,
};
use super::store::RefinedCallOutput;
use crate::analysis::calls::facts::{
    CallAlgorithm, CallEdgeKind, CallPrecision, CallProvenance, CallTargetFact, CallTargetStatus,
    UnresolvedCallFact, UnresolvedCallReason,
};
#[cfg(test)]
use crate::analysis::ids::CallSiteId;
use crate::analysis::ids::{PlaceId, RefinedCallEdgeId};
use crate::analysis::points_to::facts::{PointsToBudgetStatus, PointsToSetFact, PointsToStatus};
use crate::analysis::points_to::vars::place_var;
use crate::analysis::types::facts::{TypeFact, TypePrecision, TypeStatus, TypeSubject};
use crate::analysis_kernel::{FactFamily, FactRef, stable_key_text_from_parts};
#[cfg(test)]
use crate::core::FunctionId;
use crate::core::{AnalysisDb, Language};

pub(crate) fn derive_go_refinements(db: &AnalysisDb) -> RefinedCallOutput {
    let mut edges = Vec::new();

    for site in db
        .call_sites()
        .iter()
        .filter(|site| site.language == Language::Go)
    {
        if let Some(receiver) = site.receiver {
            let receiver_types = type_facts_for_place(db, receiver);
            let receiver_points_to = points_to_sets_for_place(db, receiver);

            for target in db
                .call_targets()
                .iter()
                .filter(|target| target.site == site.id)
            {
                if receiver_types
                    .iter()
                    .any(|fact| fact.status == TypeStatus::Present)
                {
                    edges.push(type_edge_from_target(
                        db,
                        target,
                        receiver_types
                            .iter()
                            .find(|fact| fact.status == TypeStatus::Present)
                            .copied(),
                        edges.len(),
                    ));
                }

                if receiver_points_to.iter().any(|set| {
                    set.status == PointsToStatus::Present
                        && set.budget == PointsToBudgetStatus::WithinBudget
                }) {
                    edges.push(points_to_edge_from_target(
                        db,
                        target,
                        receiver_points_to
                            .iter()
                            .find(|set| {
                                set.status == PointsToStatus::Present
                                    && set.budget == PointsToBudgetStatus::WithinBudget
                            })
                            .copied(),
                        edges.len(),
                    ));
                }
            }
        }
    }

    for unresolved in db.unresolved_calls().iter().filter(|unresolved| {
        matches!(
            unresolved.reason,
            UnresolvedCallReason::InterfaceDispatch
                | UnresolvedCallReason::FunctionValue
                | UnresolvedCallReason::DynamicProperty
        )
    }) {
        if let Some(site) = db
            .call_sites()
            .iter()
            .find(|site| site.id == unresolved.site && site.language == Language::Go)
        {
            let mut emitted_status_edge = false;
            if site
                .receiver
                .is_some_and(|receiver| has_setup_missing_type(db, receiver))
            {
                edges.push(unresolved_go_edge(
                    db,
                    unresolved,
                    CallTargetStatus::SetupMissing,
                    UnresolvedCallReason::SetupMissing,
                    RefinedCallTier::TypeValueFunctionToken,
                    edges.len(),
                ));
                emitted_status_edge = true;
            }

            if site.receiver.is_some_and(|receiver| {
                points_to_sets_for_place(db, receiver).iter().any(|set| {
                    set.status == PointsToStatus::BudgetExceeded
                        || set.budget == PointsToBudgetStatus::BudgetExceeded
                })
            }) {
                edges.push(unresolved_go_edge(
                    db,
                    unresolved,
                    CallTargetStatus::BudgetExceeded,
                    UnresolvedCallReason::BudgetExceeded,
                    RefinedCallTier::PointsToAssisted,
                    edges.len(),
                ));
                emitted_status_edge = true;
            }

            if !emitted_status_edge {
                edges.push(unresolved_go_edge(
                    db,
                    unresolved,
                    CallTargetStatus::Unresolved,
                    unresolved.reason,
                    RefinedCallTier::TypeValueFunctionToken,
                    edges.len(),
                ));
            }
        }
    }

    RefinedCallOutput { edges }.normalized()
}

fn type_edge_from_target(
    db: &AnalysisDb,
    target: &CallTargetFact,
    type_fact: Option<&TypeFact>,
    index: usize,
) -> RefinedCallEdgeFact {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    let type_key = type_fact
        .map(|fact| metadata_key(db, FactFamily::Type, fact.id.0, &fact.stable_key))
        .unwrap_or_else(|| "type:none".to_string());
    let target_key = metadata_key(db, FactFamily::CallTarget, target.id.0, &target.stable_key);
    edge_from_target(
        target,
        RefinedCallEdgeId(index as u64),
        TargetRefinement {
            tier: RefinedCallTier::TypeValueFunctionToken,
            algorithm: CallAlgorithm::TypeHierarchy,
            precision: type_fact.map_or(CallPrecision::Heuristic, |fact| {
                type_precision(fact.precision)
            }),
            evidence: vec!["go_receiver_type".to_string(), format!("type={type_key}")],
            input_stable_keys: vec![target_key.clone(), type_key.clone()],
            stable_key: stable_key_text_from_parts(
                interner,
                FactFamily::RefinedCallEdge,
                &[
                    ("tier", "go_receiver_type".to_string()),
                    ("base_target", target_key),
                    ("type", type_key),
                ],
            ),
        },
    )
}

fn points_to_edge_from_target(
    db: &AnalysisDb,
    target: &CallTargetFact,
    points_to: Option<&PointsToSetFact>,
    index: usize,
) -> RefinedCallEdgeFact {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    let points_to_key = points_to
        .map(|fact| metadata_key(db, FactFamily::PointsToSet, fact.id.0, &fact.stable_key))
        .unwrap_or_else(|| "points-to:none".to_string());
    let target_key = metadata_key(db, FactFamily::CallTarget, target.id.0, &target.stable_key);
    edge_from_target(
        target,
        RefinedCallEdgeId(index as u64),
        TargetRefinement {
            tier: RefinedCallTier::PointsToAssisted,
            algorithm: CallAlgorithm::PointsTo,
            precision: CallPrecision::Conservative,
            evidence: vec![
                "go_receiver_points_to".to_string(),
                format!("points_to={points_to_key}"),
            ],
            input_stable_keys: vec![target_key.clone(), points_to_key.clone()],
            stable_key: stable_key_text_from_parts(
                interner,
                FactFamily::RefinedCallEdge,
                &[
                    ("tier", "go_points_to".to_string()),
                    ("base_target", target_key),
                    ("points_to", points_to_key),
                ],
            ),
        },
    )
}

struct TargetRefinement {
    tier: RefinedCallTier,
    algorithm: CallAlgorithm,
    precision: CallPrecision,
    evidence: Vec<String>,
    input_stable_keys: Vec<String>,
    stable_key: String,
}

fn edge_from_target(
    target: &CallTargetFact,
    id: RefinedCallEdgeId,
    refinement: TargetRefinement,
) -> RefinedCallEdgeFact {
    RefinedCallEdgeFact {
        id,
        site: target.site,
        base_target: Some(target.id),
        caller: target.caller,
        target_function: target.target_function,
        target_symbol: target.target_symbol,
        synthetic_target: None,
        language: Language::Go,
        edge_kind: target.edge_kind,
        algorithm: refinement.algorithm,
        tier: refinement.tier,
        status: target.status,
        reason: target.reason,
        provenance: CallProvenance::Native,
        precision: refinement.precision,
        validation: RefinedCallValidation::ReferentiallyValidated,
        confidence: confidence_for_status(target.status),
        evidence: refinement.evidence,
        input_stable_keys: refinement.input_stable_keys,
        stable_key: refinement.stable_key,
    }
}

fn unresolved_go_edge(
    db: &AnalysisDb,
    unresolved: &UnresolvedCallFact,
    status: CallTargetStatus,
    reason: UnresolvedCallReason,
    tier: RefinedCallTier,
    index: usize,
) -> RefinedCallEdgeFact {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    let unresolved_key = metadata_key(
        db,
        FactFamily::UnresolvedCall,
        unresolved
            .stable_key
            .rsplit(':')
            .next()
            .and_then(|part| part.parse::<u64>().ok())
            .unwrap_or(index as u64),
        &unresolved.stable_key,
    );
    RefinedCallEdgeFact {
        id: RefinedCallEdgeId(index as u64),
        site: unresolved.site,
        base_target: None,
        caller: unresolved.caller,
        target_function: None,
        target_symbol: None,
        synthetic_target: Some("go:interface-dispatch".to_string()),
        language: Language::Go,
        edge_kind: CallEdgeKind::Unknown,
        algorithm: match tier {
            RefinedCallTier::PointsToAssisted => CallAlgorithm::PointsTo,
            _ => CallAlgorithm::TypeHierarchy,
        },
        tier,
        status,
        reason: Some(reason),
        provenance: CallProvenance::Native,
        precision: match status {
            CallTargetStatus::SetupMissing => CallPrecision::Unknown,
            CallTargetStatus::BudgetExceeded => CallPrecision::Unknown,
            _ => unresolved.precision,
        },
        validation: RefinedCallValidation::ReferentiallyValidated,
        confidence: RefinedCallConfidence::Low,
        evidence: vec!["go_unresolved_dispatch".to_string()],
        input_stable_keys: vec![unresolved_key.clone()],
        stable_key: stable_key_text_from_parts(
            interner,
            FactFamily::RefinedCallEdge,
            &[
                ("tier", format!("{tier:?}")),
                ("unresolved", unresolved_key),
                ("status", format!("{status:?}")),
            ],
        ),
    }
}

fn type_facts_for_place(db: &AnalysisDb, place: PlaceId) -> Vec<&TypeFact> {
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

fn has_setup_missing_type(db: &AnalysisDb, place: PlaceId) -> bool {
    type_facts_for_place(db, place)
        .iter()
        .any(|fact| fact.status == TypeStatus::SetupMissing)
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
    use crate::analysis::calls::facts::{CallCallee, CallSiteFact, CallSyntaxKind, CallTargetFact};
    use crate::analysis::calls::store::CallOutput;
    use crate::analysis::ids::{
        CallTargetId, MirBodyId, MirOpId, PointsToSetId, TypeFactId, TypeSetId,
    };
    use crate::analysis::places::{PlaceFact, PlaceRoot, PlaceStatus};
    use crate::analysis::points_to::facts::{PointsToPrecision, PointsToSetFact};
    use crate::analysis::points_to::store::PointsToOutput;
    use crate::analysis::types::facts::{
        TypeConfidence, TypeFact, TypePhase, TypeProvenance, TypeShape,
    };
    use crate::analysis::types::store::{TypeOutput, TypeValueAliasOutput};
    use crate::core::{FileId, FunctionFact, Span, SymbolId};

    #[test]
    fn go_receiver_with_concrete_type_creates_type_refined_edge() {
        let mut db = go_db_with_target();
        db.replace_type_value_alias_facts(TypeValueAliasOutput {
            types: TypeOutput {
                types: vec![receiver_type(TypeStatus::Present)],
                narrowed: Vec::new(),
            },
            ..TypeValueAliasOutput::default()
        });

        let output = derive_go_refinements(&db);

        assert_eq!(output.edges.len(), 1);
        assert_eq!(
            output.edges[0].tier,
            RefinedCallTier::TypeValueFunctionToken
        );
        assert_eq!(output.edges[0].algorithm, CallAlgorithm::TypeHierarchy);
    }

    #[test]
    fn setup_missing_go_interface_call_stays_setup_missing() {
        let mut db = go_db_with_unresolved_receiver();
        db.replace_type_value_alias_facts(TypeValueAliasOutput {
            types: TypeOutput {
                types: vec![receiver_type(TypeStatus::SetupMissing)],
                narrowed: Vec::new(),
            },
            ..TypeValueAliasOutput::default()
        });

        let output = derive_go_refinements(&db);

        assert_eq!(output.edges.len(), 1);
        assert_eq!(output.edges[0].status, CallTargetStatus::SetupMissing);
        assert_eq!(
            output.edges[0].reason,
            Some(UnresolvedCallReason::SetupMissing)
        );
    }

    #[test]
    fn unresolved_go_interface_call_remains_explicit_unknown() {
        let db = go_db_with_unresolved_receiver();

        let output = derive_go_refinements(&db);

        assert_eq!(output.edges.len(), 1);
        assert_eq!(output.edges[0].status, CallTargetStatus::Unresolved);
        assert_eq!(
            output.edges[0].reason,
            Some(UnresolvedCallReason::InterfaceDispatch)
        );
    }

    #[test]
    fn unrelated_go_missing_semantic_call_is_not_recast_as_dispatch() {
        let db = go_db_with_unresolved_reason(UnresolvedCallReason::MissingSemanticReference);

        let output = derive_go_refinements(&db);

        assert!(output.edges.is_empty());
    }

    #[test]
    fn within_budget_points_to_set_creates_points_to_assisted_edge() {
        let mut db = go_db_with_target();
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

        let output = derive_go_refinements(&db);

        assert_eq!(output.edges.len(), 1);
        assert_eq!(output.edges[0].tier, RefinedCallTier::PointsToAssisted);
        assert_eq!(output.edges[0].algorithm, CallAlgorithm::PointsTo);
    }

    #[test]
    fn budget_exceeded_points_to_set_creates_budget_row() {
        let mut db = go_db_with_unresolved_receiver();
        db.replace_type_value_alias_facts(TypeValueAliasOutput {
            points_to: PointsToOutput {
                constraints: Vec::new(),
                sets: vec![points_to_set(
                    PointsToStatus::BudgetExceeded,
                    PointsToBudgetStatus::BudgetExceeded,
                )],
            },
            ..TypeValueAliasOutput::default()
        });

        let output = derive_go_refinements(&db);

        assert_eq!(output.edges.len(), 1);
        assert_eq!(output.edges[0].status, CallTargetStatus::BudgetExceeded);
    }

    fn go_db_with_target() -> AnalysisDb {
        let mut db = go_db_base();
        db.replace_call_facts(CallOutput {
            sites: vec![go_call_site()],
            targets: vec![go_target()],
            unresolved: Vec::new(),
        })
        .expect("valid call facts");
        db
    }

    fn go_db_with_unresolved_receiver() -> AnalysisDb {
        go_db_with_unresolved_reason(UnresolvedCallReason::InterfaceDispatch)
    }

    fn go_db_with_unresolved_reason(reason: UnresolvedCallReason) -> AnalysisDb {
        let mut db = go_db_base();
        db.replace_call_facts(CallOutput {
            sites: vec![go_call_site()],
            targets: Vec::new(),
            unresolved: vec![UnresolvedCallFact {
                site: CallSiteId(0),
                caller: FunctionId(0),
                status: CallTargetStatus::Unresolved,
                reason,
                algorithm: CallAlgorithm::Unsupported,
                provenance: CallProvenance::Native,
                precision: CallPrecision::Unknown,
                stable_key: "unresolved:interface:0".to_string(),
            }],
        })
        .expect("valid call facts");
        db
    }

    fn go_db_base() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            "handler.go".into(),
            "handler.go".to_string(),
            "package p\nfunc caller(r Receiver) { r.Handle() }\nfunc Handle() {}\n".to_string(),
        );
        let caller = db.push_function(FunctionFact {
            id: FunctionId(99),
            file,
            name: "caller".to_string(),
            span: span(),
            language: Language::Go,
            is_test: false,
            is_exported: false,
            cyclomatic_complexity: 1,
            calls: vec!["Handle".to_string()],
        });
        db.push_function(FunctionFact {
            id: FunctionId(99),
            file,
            name: "Handle".to_string(),
            span: span(),
            language: Language::Go,
            is_test: false,
            is_exported: false,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        db.replace_semantic_mir(crate::analysis::mir::body::MirOutput {
            bodies: Vec::new(),
            operations: Vec::new(),
            places: vec![PlaceFact {
                id: PlaceId(0),
                language: Language::Go,
                file: Some(file),
                function: Some(caller),
                root: PlaceRoot::Parameter {
                    function: caller,
                    index: 0,
                    name: Some("r".to_string()),
                },
                projections: Vec::new(),
                stable_key: "place:receiver".to_string(),
                status: PlaceStatus::Resolved,
            }],
            unsupported: Vec::new(),
            ..crate::analysis::mir::body::MirOutput::default()
        })
        .expect("valid MIR");
        db
    }

    fn go_call_site() -> CallSiteFact {
        CallSiteFact {
            in_throw: false,
            id: CallSiteId(0),
            language: Language::Go,
            file: FileId(0),
            caller: FunctionId(0),
            owner_symbol: None,
            body: MirBodyId(0),
            operation: MirOpId(0),
            span: span(),
            kind: CallSyntaxKind::Method,
            callee: CallCallee::Unknown {
                reason: UnresolvedCallReason::InterfaceDispatch,
            },
            receiver: Some(PlaceId(0)),
            arguments: Vec::new(),
            result: None,
            status: CallTargetStatus::Ambiguous,
            precision: CallPrecision::Unknown,
            stable_key: "call-site:receiver".to_string(),
        }
    }

    fn go_target() -> CallTargetFact {
        CallTargetFact {
            id: CallTargetId(0),
            site: CallSiteId(0),
            caller: FunctionId(0),
            target_function: Some(FunctionId(1)),
            target_symbol: Some(SymbolId(0)),
            edge_kind: CallEdgeKind::Method,
            algorithm: CallAlgorithm::GoStatic,
            status: CallTargetStatus::Resolved,
            reason: None,
            provenance: CallProvenance::Native,
            precision: CallPrecision::SetupAware,
            stable_key: "call-target:go-method".to_string(),
        }
    }

    fn receiver_type(status: TypeStatus) -> TypeFact {
        TypeFact {
            id: TypeFactId(0),
            subject: TypeSubject::Place(PlaceId(0)),
            type_set: TypeSetId(0),
            shape: TypeShape::Nominal {
                type_id: "Receiver".to_string(),
            },
            phase: TypePhase::Resolved,
            language: Language::Go,
            file: Some(FileId(0)),
            function: Some(FunctionId(0)),
            body: None,
            place: Some(PlaceId(0)),
            cfg_block: None,
            operation: None,
            precision: TypePrecision::SetupAware,
            confidence: TypeConfidence::High,
            status,
            provenance: TypeProvenance::Native,
            stable_key: "type:receiver".to_string(),
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
            stable_key: "points-to:receiver".to_string(),
        }
    }

    fn span() -> Span {
        Span::point(FileId(0), 1, 1)
    }
}

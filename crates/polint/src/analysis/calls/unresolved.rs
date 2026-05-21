use std::collections::BTreeMap;

use crate::analysis::calls::facts::{
    CallAlgorithm, CallCallee, CallPrecision, CallProvenance, CallSiteFact, CallSyntaxKind,
    CallTargetStatus, UnresolvedCallFact, UnresolvedCallReason,
};
use crate::analysis::ids::MirOpId;
use crate::analysis::mir::op::{UnsupportedDomain, UnsupportedSemanticFact};
use crate::analysis::stable_key::semantic_stable_key;
use crate::analysis_kernel::FactFamily;
use crate::core::AnalysisDb;

pub(crate) fn derive_unresolved_calls(
    db: &AnalysisDb,
    sites: &[CallSiteFact],
) -> Vec<UnresolvedCallFact> {
    let mut rows = BTreeMap::new();

    for site in sites {
        if let Some(reason) = reason_for_site(site) {
            insert_unresolved(&mut rows, site, reason, "call-site-shape");
        }
    }

    let sites_by_operation = sites
        .iter()
        .map(|site| (site.operation, site))
        .collect::<BTreeMap<_, _>>();
    for unsupported in db
        .unsupported_semantics()
        .iter()
        .filter(|row| row.affected_domains.contains(&UnsupportedDomain::Calls))
    {
        let Some(site) = site_for_unsupported(unsupported, sites, &sites_by_operation) else {
            continue;
        };
        let reason = reason_for_unsupported(unsupported);
        insert_unresolved(&mut rows, site, reason, &unsupported.stable_key);
    }

    rows.into_values().collect()
}

fn insert_unresolved(
    rows: &mut BTreeMap<String, UnresolvedCallFact>,
    site: &CallSiteFact,
    reason: UnresolvedCallReason,
    evidence: &str,
) {
    let status = status_for_reason(reason);
    let stable_key = unresolved_stable_key(site, reason, status, evidence);
    rows.entry(stable_key.clone())
        .or_insert(UnresolvedCallFact {
            site: site.id,
            caller: site.caller,
            status,
            reason,
            algorithm: algorithm_for_reason(reason),
            provenance: CallProvenance::MirShape,
            precision: precision_for_reason(reason),
            stable_key,
        });
}

fn reason_for_site(site: &CallSiteFact) -> Option<UnresolvedCallReason> {
    match &site.callee {
        CallCallee::FunctionValue { .. } => Some(UnresolvedCallReason::FunctionValue),
        CallCallee::Unknown { reason } => Some(normalize_unknown_reason(*reason)),
        CallCallee::Index { .. } => Some(UnresolvedCallReason::DynamicProperty),
        CallCallee::Member { .. } if matches!(site.kind, CallSyntaxKind::Member) => {
            Some(UnresolvedCallReason::DynamicProperty)
        }
        CallCallee::Import | CallCallee::Constructor { .. }
            if matches!(site.kind, CallSyntaxKind::DynamicImport) =>
        {
            Some(UnresolvedCallReason::DynamicImport)
        }
        _ => None,
    }
}

fn normalize_unknown_reason(reason: UnresolvedCallReason) -> UnresolvedCallReason {
    match reason {
        UnresolvedCallReason::Unknown => UnresolvedCallReason::UnknownCallee,
        other => other,
    }
}

fn site_for_unsupported<'site>(
    unsupported: &UnsupportedSemanticFact,
    sites: &'site [CallSiteFact],
    sites_by_operation: &BTreeMap<MirOpId, &'site CallSiteFact>,
) -> Option<&'site CallSiteFact> {
    unsupported
        .operation
        .and_then(|operation| sites_by_operation.get(&operation).copied())
        .or_else(|| {
            sites
                .iter()
                .find(|site| site.file == unsupported.file && site.language == unsupported.language)
        })
}

fn reason_for_unsupported(row: &UnsupportedSemanticFact) -> UnresolvedCallReason {
    let construct = row.construct.to_ascii_lowercase();
    let evidence = row.source_evidence.to_ascii_lowercase();
    let labels = [construct.as_str(), evidence.as_str()];
    if any_contains(
        &labels,
        &["setup missing", "setup-missing", "package missing"],
    ) {
        UnresolvedCallReason::SetupMissing
    } else if any_contains(&labels, &["interface"]) {
        UnresolvedCallReason::InterfaceDispatch
    } else if any_contains(&labels, &["reflect"]) {
        UnresolvedCallReason::Reflection
    } else if any_contains(&labels, &["go_statement", "goroutine", "go statement"]) {
        UnresolvedCallReason::GoroutineBoundary
    } else if any_contains(&labels, &["eval"]) {
        UnresolvedCallReason::Eval
    } else if any_contains(&labels, &["dynamic import", "import("]) {
        UnresolvedCallReason::DynamicImport
    } else if any_contains(&labels, &["call/apply/bind", ".call", ".apply", ".bind"]) {
        UnresolvedCallReason::CallApplyBind
    } else if any_contains(&labels, &["proxy", "getter", "setter", "accessor"]) {
        UnresolvedCallReason::ProxyOrAccessor
    } else if any_contains(&labels, &["framework", "decorator"]) {
        UnresolvedCallReason::FrameworkDispatch
    } else if any_contains(&labels, &["dynamic property", "index", "computed"]) {
        UnresolvedCallReason::DynamicProperty
    } else if any_contains(&labels, &["unsupported", "parser", "syntax"]) {
        UnresolvedCallReason::UnsupportedSyntax
    } else {
        UnresolvedCallReason::UnknownCallee
    }
}

fn any_contains(values: &[&str], needles: &[&str]) -> bool {
    values
        .iter()
        .any(|value| needles.iter().any(|needle| value.contains(needle)))
}

fn unresolved_stable_key(
    site: &CallSiteFact,
    reason: UnresolvedCallReason,
    status: CallTargetStatus,
    evidence: &str,
) -> String {
    semantic_stable_key(
        FactFamily::UnresolvedCall,
        &[
            ("site", site.stable_key.clone()),
            ("reason", format!("{reason:?}")),
            ("status", format!("{status:?}")),
            ("algorithm", format!("{:?}", algorithm_for_reason(reason))),
            ("evidence", evidence.to_string()),
        ],
    )
    .into_string()
}

fn status_for_reason(reason: UnresolvedCallReason) -> CallTargetStatus {
    match reason {
        UnresolvedCallReason::SetupMissing => CallTargetStatus::SetupMissing,
        UnresolvedCallReason::UnsupportedSyntax
        | UnresolvedCallReason::Reflection
        | UnresolvedCallReason::GoroutineBoundary
        | UnresolvedCallReason::FrameworkDispatch
        | UnresolvedCallReason::ProxyOrAccessor => CallTargetStatus::Unsupported,
        _ => CallTargetStatus::Unresolved,
    }
}

fn precision_for_reason(reason: UnresolvedCallReason) -> CallPrecision {
    match reason {
        UnresolvedCallReason::SetupMissing => CallPrecision::Unsupported,
        UnresolvedCallReason::UnsupportedSyntax
        | UnresolvedCallReason::Reflection
        | UnresolvedCallReason::GoroutineBoundary
        | UnresolvedCallReason::FrameworkDispatch
        | UnresolvedCallReason::ProxyOrAccessor => CallPrecision::Unsupported,
        _ => CallPrecision::Unknown,
    }
}

fn algorithm_for_reason(reason: UnresolvedCallReason) -> CallAlgorithm {
    match reason {
        UnresolvedCallReason::FrameworkDispatch => CallAlgorithm::FrameworkModel,
        UnresolvedCallReason::UnsupportedSyntax
        | UnresolvedCallReason::Reflection
        | UnresolvedCallReason::GoroutineBoundary
        | UnresolvedCallReason::ProxyOrAccessor => CallAlgorithm::Unsupported,
        _ => CallAlgorithm::SyntaxOnly,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use crate::analysis::calls::facts::{
        CallCallee, CallPrecision, CallSiteFact, CallSyntaxKind, CallTargetStatus,
        UnresolvedCallReason,
    };
    use crate::analysis::ids::{CallSiteId, MirBodyId, MirOpId, PlaceId, UnsupportedId};
    use crate::analysis::mir::body::{MirBody, MirOutput, MirStatus};
    use crate::analysis::mir::op::{
        ConservativeAction, MirOperation, MirOperationKind, MirValue, UnsupportedDomain,
        UnsupportedPrecision, UnsupportedSemanticFact,
    };
    use crate::analysis::places::{PlaceFact, PlaceRoot, PlaceStatus};
    use crate::core::{AnalysisDb, FileId, FunctionFact, FunctionId, Language, Span};

    fn span(file: FileId, line: u32) -> Span {
        Span {
            file,
            start_byte: line * 10,
            end_byte: line * 10 + 4,
            start_line: line,
            start_col: 1,
            end_line: line,
            end_col: 5,
        }
    }

    fn db_with_function(language: Language, path: &str) -> (AnalysisDb, FileId, FunctionId) {
        let mut db = AnalysisDb::new();
        let file = db.add_file(PathBuf::from(path), path.to_string(), String::new());
        let function = db.push_function(FunctionFact {
            id: FunctionId(999),
            file,
            name: "caller".to_string(),
            span: span(file, 1),
            language,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        (db, file, function)
    }

    fn site(
        language: Language,
        file: FileId,
        caller: FunctionId,
        site: u64,
        callee: CallCallee,
        kind: CallSyntaxKind,
    ) -> CallSiteFact {
        CallSiteFact {
            id: CallSiteId(site),
            language,
            file,
            caller,
            owner_symbol: None,
            body: MirBodyId(0),
            operation: MirOpId(site),
            span: span(file, 2),
            kind,
            callee,
            receiver: None,
            arguments: Vec::new(),
            result: Some(PlaceId(2)),
            status: CallTargetStatus::Unresolved,
            precision: CallPrecision::Conservative,
            stable_key: format!("call-site:{site}"),
        }
    }

    fn replace_unsupported(
        db: &mut AnalysisDb,
        language: Language,
        file: FileId,
        function: FunctionId,
        rows: Vec<UnsupportedSemanticFact>,
    ) {
        db.replace_semantic_mir(MirOutput {
            bodies: vec![MirBody {
                id: MirBodyId(0),
                language,
                file,
                function,
                package: None,
                module: None,
                owner_stable_key: "function:caller".to_string(),
                span: span(file, 1),
                stable_key: "mir-body:caller".to_string(),
                status: MirStatus::Partial,
            }],
            places: vec![PlaceFact {
                id: PlaceId(1),
                language,
                file: Some(file),
                function: Some(function),
                root: PlaceRoot::Local {
                    function,
                    name: "callee".to_string(),
                },
                projections: Vec::new(),
                stable_key: "place:callee".to_string(),
                status: PlaceStatus::Partial,
            }],
            operations: vec![MirOperation {
                id: MirOpId(0),
                body: MirBodyId(0),
                ordinal: 0,
                span: span(file, 2),
                kind: MirOperationKind::Call {
                    site: CallSiteId(10),
                    callee: MirValue::Place(PlaceId(1)),
                    arguments: Vec::new(),
                    return_place: PlaceId(1),
                },
                stable_key: "mir-op:call".to_string(),
                status: MirStatus::Partial,
            }],
            unsupported: rows,
        })
        .expect("semantic MIR should store");
    }

    fn unsupported(
        id: u64,
        language: Language,
        file: FileId,
        construct: &str,
    ) -> UnsupportedSemanticFact {
        UnsupportedSemanticFact {
            id: UnsupportedId(id),
            body: Some(MirBodyId(0)),
            operation: Some(MirOpId(0)),
            language,
            file,
            span: span(file, 2 + id as u32),
            construct: construct.to_string(),
            source_evidence: construct.to_string(),
            affected_places: Vec::new(),
            affected_domains: vec![UnsupportedDomain::Calls],
            conservative_action: ConservativeAction::HavocAffectedPlaces,
            precision: UnsupportedPrecision::Unsupported,
            status: MirStatus::Unsupported,
            stable_key: format!("unsupported:{construct}"),
        }
    }

    #[test]
    fn derive_unresolved_calls_emits_function_value_and_unknown_callee_rows() {
        let (db, file, caller) = db_with_function(Language::TypeScript, "src/app.ts");
        let sites = vec![
            site(
                Language::TypeScript,
                file,
                caller,
                10,
                CallCallee::FunctionValue { place: PlaceId(1) },
                CallSyntaxKind::FunctionValue,
            ),
            site(
                Language::TypeScript,
                file,
                caller,
                20,
                CallCallee::Unknown {
                    reason: UnresolvedCallReason::UnknownCallee,
                },
                CallSyntaxKind::Unknown,
            ),
        ];

        let unresolved = super::derive_unresolved_calls(&db, &sites);
        let reasons = unresolved
            .iter()
            .map(|row| (row.site, row.reason, row.status, row.precision))
            .collect::<Vec<_>>();

        assert_eq!(
            reasons,
            vec![
                (
                    CallSiteId(10),
                    UnresolvedCallReason::FunctionValue,
                    CallTargetStatus::Unresolved,
                    CallPrecision::Unknown,
                ),
                (
                    CallSiteId(20),
                    UnresolvedCallReason::UnknownCallee,
                    CallTargetStatus::Unresolved,
                    CallPrecision::Unknown,
                ),
            ]
        );
    }

    #[test]
    fn derive_unresolved_calls_preserves_specific_ts_unsupported_reasons() {
        let (mut db, file, caller) = db_with_function(Language::TypeScript, "src/app.ts");
        replace_unsupported(
            &mut db,
            Language::TypeScript,
            file,
            caller,
            vec![
                unsupported(1, Language::TypeScript, file, "eval"),
                unsupported(2, Language::TypeScript, file, "dynamic property key"),
                unsupported(3, Language::TypeScript, file, "dynamic import"),
                unsupported(4, Language::TypeScript, file, "call/apply/bind"),
                unsupported(5, Language::TypeScript, file, "Proxy"),
                unsupported(6, Language::TypeScript, file, "decorator dispatch"),
                unsupported(7, Language::TypeScript, file, "framework dispatch"),
            ],
        );
        let sites = vec![site(
            Language::TypeScript,
            file,
            caller,
            10,
            CallCallee::Unknown {
                reason: UnresolvedCallReason::UnknownCallee,
            },
            CallSyntaxKind::Unknown,
        )];

        let reasons = super::derive_unresolved_calls(&db, &sites)
            .into_iter()
            .map(|row| row.reason)
            .collect::<BTreeSet<_>>();

        assert!(reasons.contains(&UnresolvedCallReason::Eval));
        assert!(reasons.contains(&UnresolvedCallReason::DynamicProperty));
        assert!(reasons.contains(&UnresolvedCallReason::DynamicImport));
        assert!(reasons.contains(&UnresolvedCallReason::CallApplyBind));
        assert!(reasons.contains(&UnresolvedCallReason::ProxyOrAccessor));
        assert!(reasons.contains(&UnresolvedCallReason::FrameworkDispatch));
    }

    #[test]
    fn derive_unresolved_calls_preserves_specific_go_unsupported_reasons() {
        let (mut db, file, caller) = db_with_function(Language::Go, "flow.go");
        replace_unsupported(
            &mut db,
            Language::Go,
            file,
            caller,
            vec![
                unsupported(1, Language::Go, file, "interface dispatch"),
                unsupported(2, Language::Go, file, "reflect"),
                unsupported(3, Language::Go, file, "go_statement"),
                unsupported(4, Language::Go, file, "setup missing package"),
                unsupported(5, Language::Go, file, "unsupported syntax"),
            ],
        );
        let sites = vec![site(
            Language::Go,
            file,
            caller,
            10,
            CallCallee::FunctionValue { place: PlaceId(1) },
            CallSyntaxKind::FunctionValue,
        )];

        let reasons = super::derive_unresolved_calls(&db, &sites)
            .into_iter()
            .map(|row| row.reason)
            .collect::<BTreeSet<_>>();

        assert!(reasons.contains(&UnresolvedCallReason::InterfaceDispatch));
        assert!(reasons.contains(&UnresolvedCallReason::Reflection));
        assert!(reasons.contains(&UnresolvedCallReason::GoroutineBoundary));
        assert!(reasons.contains(&UnresolvedCallReason::SetupMissing));
        assert!(reasons.contains(&UnresolvedCallReason::UnsupportedSyntax));
    }
}

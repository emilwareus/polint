use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::calls::facts::CallTargetStatus;
use crate::analysis_kernel::{FactFamily, FactPrecision};
use crate::core::AnalysisDb;
use crate::diagnostics::{Diagnostic, TextRange};

pub(crate) fn validate_calls(db: &AnalysisDb, diagnostics: &mut Vec<Diagnostic>) {
    let files = db.files().iter().map(|row| row.id).collect::<BTreeSet<_>>();
    let functions = db
        .functions()
        .iter()
        .map(|row| row.id)
        .collect::<BTreeSet<_>>();
    let symbols = db
        .symbols()
        .iter()
        .map(|row| row.id)
        .collect::<BTreeSet<_>>();
    let bodies = db
        .mir_bodies()
        .iter()
        .map(|row| row.id)
        .collect::<BTreeSet<_>>();
    let operations = db
        .mir_operations()
        .iter()
        .map(|row| row.id)
        .collect::<BTreeSet<_>>();
    let places = db
        .mir_places()
        .iter()
        .map(|row| row.id)
        .collect::<BTreeSet<_>>();
    let sites = db
        .call_sites()
        .iter()
        .map(|row| (row.id, row))
        .collect::<BTreeMap<_, _>>();

    check_duplicate_stable_keys(
        diagnostics,
        "CallSite",
        db.call_sites().iter().map(|row| row.stable_key.as_str()),
    );
    check_duplicate_stable_keys(
        diagnostics,
        "CallTarget",
        db.call_targets().iter().map(|row| row.stable_key.as_str()),
    );
    check_duplicate_stable_keys(
        diagnostics,
        "UnresolvedCall",
        db.unresolved_calls()
            .iter()
            .map(|row| row.stable_key.as_str()),
    );

    for site in db.call_sites() {
        check_ref(
            diagnostics,
            &files,
            site.file,
            "CallSite",
            &site.stable_key,
            "file",
            "dangling call file reference",
        );
        check_ref(
            diagnostics,
            &functions,
            site.caller,
            "CallSite",
            &site.stable_key,
            "caller",
            "dangling call caller function reference",
        );
        check_ref(
            diagnostics,
            &bodies,
            site.body,
            "CallSite",
            &site.stable_key,
            "body",
            "dangling call MIR body reference",
        );
        check_ref(
            diagnostics,
            &operations,
            site.operation,
            "CallSite",
            &site.stable_key,
            "operation",
            "dangling call MIR operation reference",
        );
        if let Some(owner_symbol) = site.owner_symbol {
            check_ref(
                diagnostics,
                &symbols,
                owner_symbol,
                "CallSite",
                &site.stable_key,
                "owner_symbol",
                "dangling call owner symbol reference",
            );
        }
        if let Some(receiver) = site.receiver {
            check_ref(
                diagnostics,
                &places,
                receiver,
                "CallSite",
                &site.stable_key,
                "receiver",
                "dangling call receiver place reference",
            );
        }
        for argument in &site.arguments {
            check_ref(
                diagnostics,
                &places,
                *argument,
                "CallSite",
                &site.stable_key,
                "arguments",
                "dangling call argument place reference",
            );
        }
        if let Some(result) = site.result {
            check_ref(
                diagnostics,
                &places,
                result,
                "CallSite",
                &site.stable_key,
                "result",
                "dangling call result place reference",
            );
        }
        if site.span.start_byte > site.span.end_byte {
            push_call_diagnostic(
                diagnostics,
                "CallSite",
                &site.stable_key,
                "span",
                "invalid span byte range",
            );
        }
    }

    for target in db.call_targets() {
        if !sites.contains_key(&target.site) {
            push_call_diagnostic(
                diagnostics,
                "CallTarget",
                &target.stable_key,
                "site",
                "target without matching call site",
            );
        }
        check_ref(
            diagnostics,
            &functions,
            target.caller,
            "CallTarget",
            &target.stable_key,
            "caller",
            "dangling call target caller function reference",
        );
        if let Some(target_function) = target.target_function {
            check_ref(
                diagnostics,
                &functions,
                target_function,
                "CallTarget",
                &target.stable_key,
                "target_function",
                "dangling call target function reference",
            );
        }
        if let Some(target_symbol) = target.target_symbol {
            check_ref(
                diagnostics,
                &symbols,
                target_symbol,
                "CallTarget",
                &target.stable_key,
                "target_symbol",
                "dangling call target symbol reference",
            );
        }
        if target.status == CallTargetStatus::Resolved && target.reason.is_some() {
            push_call_diagnostic(
                diagnostics,
                "CallTarget",
                &target.stable_key,
                "status",
                "contradictory resolved target status with unresolved reason",
            );
        }
        if target.status == CallTargetStatus::Resolved
            && target.target_function.is_none()
            && target.target_symbol.is_none()
        {
            push_call_diagnostic(
                diagnostics,
                "CallTarget",
                &target.stable_key,
                "target",
                "resolved call target requires a function or symbol",
            );
        }
        if unresolved_status(target.status) && target.reason.is_none() {
            push_call_diagnostic(
                diagnostics,
                "CallTarget",
                &target.stable_key,
                "reason",
                "missing unresolved reason",
            );
        }
        if unresolved_status(target.status)
            && target.reason.is_some()
            && (target.target_function.is_some() || target.target_symbol.is_some())
        {
            push_call_diagnostic(
                diagnostics,
                "CallTarget",
                &target.stable_key,
                "target",
                "unresolved call target cannot carry target identity",
            );
        }
    }

    for unresolved in db.unresolved_calls() {
        if !sites.contains_key(&unresolved.site) {
            push_call_diagnostic(
                diagnostics,
                "UnresolvedCall",
                &unresolved.stable_key,
                "site",
                "unresolved row without matching call site",
            );
        }
        check_ref(
            diagnostics,
            &functions,
            unresolved.caller,
            "UnresolvedCall",
            &unresolved.stable_key,
            "caller",
            "dangling unresolved caller function reference",
        );
        if !unresolved_status(unresolved.status) {
            push_call_diagnostic(
                diagnostics,
                "UnresolvedCall",
                &unresolved.stable_key,
                "status",
                "contradictory unresolved row with resolved status",
            );
        }
    }

    for family in [
        FactFamily::CallSite,
        FactFamily::CallTarget,
        FactFamily::UnresolvedCall,
    ] {
        for (reference, metadata) in db.fact_meta().rows() {
            if reference.family != family || metadata.producer_id != "polint.calls" {
                continue;
            }
            if metadata.precision == FactPrecision::Exact {
                push_call_diagnostic(
                    diagnostics,
                    family.label(),
                    &metadata.stable_key,
                    "precision",
                    "precision ceiling exceeded: polint.calls rows are SetupAware, not Exact",
                );
            }
        }
    }
}

fn check_duplicate_stable_keys<'a>(
    diagnostics: &mut Vec<Diagnostic>,
    family: &'static str,
    keys: impl Iterator<Item = &'a str>,
) {
    let mut seen = BTreeSet::new();
    for key in keys {
        if !seen.insert(key) {
            push_call_diagnostic(
                diagnostics,
                family,
                key,
                "stable_key",
                "duplicate stable key",
            );
        }
    }
}

fn check_ref<T: Ord + Copy>(
    diagnostics: &mut Vec<Diagnostic>,
    valid: &BTreeSet<T>,
    value: T,
    family: &'static str,
    stable_key: &str,
    field: &'static str,
    reason: &'static str,
) {
    if !valid.contains(&value) {
        push_call_diagnostic(diagnostics, family, stable_key, field, reason);
    }
}

fn unresolved_status(status: CallTargetStatus) -> bool {
    matches!(
        status,
        CallTargetStatus::Unresolved
            | CallTargetStatus::Unsupported
            | CallTargetStatus::SetupMissing
            | CallTargetStatus::BudgetExceeded
            | CallTargetStatus::Rejected
    )
}

fn push_call_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    family: &'static str,
    stable_key: &str,
    field: &'static str,
    reason: &'static str,
) {
    diagnostics.push(
        Diagnostic::error(
            "polint/internal",
            "<workspace>",
            TextRange::point(1, 1),
            format!("Calls validation failed for {family} stable key."),
        )
        .with_evidence("family", family)
        .with_evidence("stable_key", stable_key.to_string())
        .with_evidence("field", field)
        .with_evidence("reason", reason),
    );
}

#[cfg(test)]
mod tests {
    use super::validate_calls;
    use crate::analysis::calls::facts::{
        CallAlgorithm, CallCallee, CallEdgeKind, CallPrecision, CallProvenance, CallSiteFact,
        CallSyntaxKind, CallTargetFact, CallTargetStatus, UnresolvedCallFact, UnresolvedCallReason,
    };
    use crate::analysis::calls::store::{CallOutput, CallStore};
    use crate::analysis::ids::{CallSiteId, CallTargetId, MirBodyId, MirOpId, PlaceId};
    use crate::analysis_kernel::validation::validate_fact_metadata;
    use crate::analysis_kernel::{
        AnalysisKernel, FactConfidence, FactFamily, FactMeta, FactPrecision, FactRef,
        ValidationStatus,
    };
    use crate::core::{
        AnalysisDb, FileId, FunctionFact, FunctionId, Language, Span, SymbolFact, SymbolId,
        SymbolKind, SymbolNamespace, SymbolPrecision,
    };
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    #[test]
    fn calls_validation_reports_malformed_rows_with_required_evidence() {
        let mut db = base_db();
        db.replace_call_facts(CallOutput {
            sites: vec![
                site(0, "call-site:dup"),
                CallSiteFact {
                    id: CallSiteId(1),
                    file: FileId(99),
                    caller: FunctionId(99),
                    owner_symbol: Some(SymbolId(99)),
                    body: MirBodyId(99),
                    operation: MirOpId(99),
                    span: Span {
                        file: FileId(0),
                        start_byte: 10,
                        end_byte: 1,
                        start_line: 1,
                        start_col: 11,
                        end_line: 1,
                        end_col: 2,
                    },
                    arguments: vec![PlaceId(99)],
                    receiver: Some(PlaceId(98)),
                    result: Some(PlaceId(97)),
                    stable_key: "call-site:dup".to_string(),
                    ..site(1, "call-site:bad")
                },
            ],
            targets: vec![
                target(0, CallSiteId(0), "call-target:bad"),
                CallTargetFact {
                    id: CallTargetId(1),
                    status: CallTargetStatus::Resolved,
                    reason: Some(UnresolvedCallReason::DynamicProperty),
                    target_function: None,
                    target_symbol: None,
                    stable_key: "call-target:contradictory".to_string(),
                    ..target(1, CallSiteId(0), "call-target:ok")
                },
                CallTargetFact {
                    id: CallTargetId(2),
                    status: CallTargetStatus::Unresolved,
                    target_function: None,
                    target_symbol: None,
                    stable_key: "call-target:missing-reason".to_string(),
                    ..target(2, CallSiteId(0), "call-target:ok")
                },
            ],
            unresolved: vec![UnresolvedCallFact {
                site: CallSiteId(0),
                caller: FunctionId(99),
                status: CallTargetStatus::Resolved,
                reason: UnresolvedCallReason::Unknown,
                algorithm: CallAlgorithm::DirectReference,
                provenance: CallProvenance::Native,
                precision: CallPrecision::Exact,
                stable_key: "call-unresolved:bad".to_string(),
            }],
        })
        .expect("call rows should store for validation");

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());
        let calls = call_diagnostics(&diagnostics);

        assert!(
            calls.len() >= 8,
            "expected call validation diagnostics: {diagnostics:#?}"
        );
        assert!(calls.iter().all(|diagnostic| {
            let labels = evidence_labels(diagnostic);
            labels.contains("family")
                && labels.contains("stable_key")
                && labels.contains("field")
                && labels.contains("reason")
        }));
        assert!(
            calls
                .iter()
                .any(|diagnostic| diagnostic.evidence.iter().any(|evidence| {
                    evidence.label == "reason" && evidence.value.contains("contradictory")
                })),
            "expected contradictory status diagnostic: {diagnostics:#?}"
        );
        assert!(
            calls
                .iter()
                .any(|diagnostic| diagnostic.evidence.iter().any(|evidence| {
                    evidence.label == "reason"
                        && evidence.value.contains("missing unresolved reason")
                })),
            "expected missing unresolved reason diagnostic: {diagnostics:#?}"
        );
    }

    #[test]
    fn calls_validation_rejects_unresolved_target_identity_with_reason() {
        let mut db = base_db();
        db.replace_call_facts(CallOutput {
            sites: vec![site(0, "call-site:ok")],
            targets: vec![CallTargetFact {
                status: CallTargetStatus::Unsupported,
                reason: Some(UnresolvedCallReason::FrameworkDispatch),
                target_function: Some(FunctionId(1)),
                target_symbol: Some(SymbolId(1)),
                stable_key: "call-target:unsupported-with-target".to_string(),
                ..target(0, CallSiteId(0), "call-target:ok")
            }],
            unresolved: Vec::new(),
        })
        .expect("call rows should store for validation");

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());
        let calls = call_diagnostics(&diagnostics);

        assert!(
            calls.iter().any(|diagnostic| {
                diagnostic.evidence.iter().any(|evidence| {
                    evidence.label == "reason"
                        && evidence
                            .value
                            .contains("unresolved call target cannot carry target identity")
                })
            }),
            "expected contradictory target identity diagnostic: {diagnostics:#?}"
        );
    }

    #[test]
    fn calls_validation_rejects_exact_provider_precision() {
        let mut db = base_db();
        db.replace_call_facts(CallOutput {
            sites: vec![site(0, "call-site:ok")],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("call rows should store");
        db.fact_meta_mut_for_test()
            .remove_for_test(FactRef::new(FactFamily::CallSite, 0));
        db.fact_meta_mut_for_test().insert(
            FactRef::new(FactFamily::CallSite, 0),
            FactMeta {
                stable_key: "call-site:ok".to_string(),
                producer_id: "polint.calls",
                layer_id: "polint.calls",
                precision: FactPrecision::Exact,
                confidence: FactConfidence::High,
                validation: ValidationStatus::NativeTrusted,
                payload_digest: "payload:exact-calls".to_string(),
            },
        );

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());

        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .starts_with("Fact metadata precision ceiling violated")
                    && diagnostic
                        .evidence
                        .iter()
                        .any(|evidence| evidence.label == "family" && evidence.value == "CallSite")
            }),
            "expected calls precision ceiling diagnostic: {diagnostics:#?}"
        );
    }

    #[test]
    fn calls_validation_exercises_all_d10_indexes() {
        let output = CallOutput {
            sites: vec![site(0, "call-site:ok")],
            targets: vec![target(0, CallSiteId(0), "call-target:ok")],
            unresolved: vec![unresolved(0, "call-unresolved:ok")],
        };
        let store = CallStore::from_output(output).expect("call store should index rows");

        assert_eq!(store.sites_by_caller(FunctionId(0)).len(), 1);
        assert_eq!(store.targets_by_site(CallSiteId(0)).len(), 1);
        assert_eq!(store.outgoing_by_function(FunctionId(0)).len(), 1);
        assert_eq!(store.outgoing_by_symbol(SymbolId(0)).len(), 1);
        assert_eq!(store.incoming_by_symbol(SymbolId(1)).len(), 1);
        assert_eq!(store.incoming_by_function(FunctionId(1)).len(), 1);
        assert_eq!(
            store
                .unresolved_by_reason(UnresolvedCallReason::DynamicProperty)
                .len(),
            1
        );
        assert_eq!(
            store
                .unresolved_by_status(CallTargetStatus::Unresolved)
                .len(),
            1
        );

        let missing = CallStore::from_output(CallOutput {
            sites: Vec::new(),
            targets: vec![target(0, CallSiteId(99), "call-target:without-site")],
            unresolved: Vec::new(),
        })
        .expect_err("targets without sites should be rejected before indexing");
        assert!(missing.to_string().contains("dangling call site"));
    }

    fn base_db() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "export function app() { target(); }\n".to_string(),
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
            vec![
                symbol(SymbolId(0), file, "app"),
                symbol(SymbolId(1), file, "target"),
            ],
            Vec::new(),
            Vec::new(),
        );
        db
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

    fn target(id: u64, site: CallSiteId, stable_key: &str) -> CallTargetFact {
        CallTargetFact {
            id: CallTargetId(id),
            site,
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

    fn unresolved(site: u64, stable_key: &str) -> UnresolvedCallFact {
        UnresolvedCallFact {
            site: CallSiteId(site),
            caller: FunctionId(0),
            status: CallTargetStatus::Unresolved,
            reason: UnresolvedCallReason::DynamicProperty,
            algorithm: CallAlgorithm::SyntaxOnly,
            provenance: CallProvenance::MirShape,
            precision: CallPrecision::Unknown,
            stable_key: stable_key.to_string(),
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

    fn call_diagnostics(
        diagnostics: &[crate::diagnostics::Diagnostic],
    ) -> Vec<&crate::diagnostics::Diagnostic> {
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.message.starts_with("Calls validation failed"))
            .collect()
    }

    fn evidence_labels(diagnostic: &crate::diagnostics::Diagnostic) -> BTreeSet<&str> {
        diagnostic
            .evidence
            .iter()
            .map(|evidence| evidence.label.as_str())
            .collect()
    }

    #[test]
    fn direct_validate_calls_entrypoint_is_empty_until_green_step() {
        let db = base_db();
        let mut diagnostics = Vec::new();
        validate_calls(&db, &mut diagnostics);
        assert!(diagnostics.is_empty());
    }
}

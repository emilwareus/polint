use crate::analysis::adaptation::facts::RejectionReason;
use crate::analysis::calls::facts::{CallTargetStatus, UnresolvedCallReason};
use crate::analysis::points_to::facts::{PointsToPrecision, PointsToStatus};
use crate::analysis::refined_calls::facts::RefinedCallEdgeFact;
use crate::analysis::unknown_taxonomy::facts::{
    UnknownCategory, UnknownRow, UnknownRowInput, UnknownSpan, normalize_rows,
};
use crate::analysis_kernel::{FactFamily, FactRef};
use crate::core::{
    AnalysisDb, ResolutionPrecision, ResolutionStatus, SymbolPrecision, SymbolResolutionStatus,
    UnresolvedReason,
};
use crate::diagnostics::Diagnostic;
use crate::go::semantic::diagnostics::{
    GO_PACKAGES_LOAD_FAILED, GO_SIDECAR_TIMEOUT, GO_VERSION_UNSUPPORTED,
};

pub(crate) fn unsupported_capability_row(capability: &str, docs_path: Option<&str>) -> UnknownRow {
    UnknownRow::new(UnknownRowInput {
        category: UnknownCategory::UnsupportedSemantic,
        capability: Some(capability.to_string()),
        family: None,
        provider: "polint.capabilities".to_string(),
        file: "<workspace>".to_string(),
        span: None,
        status: "unsupported".to_string(),
        reason: Some("Capability does not support public unknown inspection.".to_string()),
        precision: None,
        docs_path: docs_path
            .or(Some("docs/facts/capability-plans.md"))
            .map(str::to_string),
        suggested_artifact: Some("provider".to_string()),
        source_stable_key: Some(format!("capability:{capability}")),
    })
}

pub(crate) fn public_capability_unknowns(db: &AnalysisDb, capability: &str) -> Vec<UnknownRow> {
    let rows = match capability {
        "resolved_imports" => resolved_import_unknowns(db),
        "symbols" => symbol_unknowns(db),
        "references" => reference_unknowns(db),
        _ => Vec::new(),
    };
    normalize_rows(rows)
}

pub(crate) fn graph_engine_unknowns(db: &AnalysisDb) -> Vec<UnknownRow> {
    graph_engine_unknowns_with_diagnostics(db, &[])
}

pub(crate) fn graph_engine_unknowns_with_diagnostics(
    db: &AnalysisDb,
    diagnostics: &[Diagnostic],
) -> Vec<UnknownRow> {
    let mut rows = Vec::new();
    rows.extend(go_semantic_unknowns(db));
    rows.extend(go_semantic_diagnostic_unknowns(diagnostics));
    rows.extend(solver_unknowns(db));
    rows.extend(refined_call_unknowns(db));
    rows.extend(adaptation_unknowns(db));
    normalize_rows(rows)
}

#[allow(
    dead_code,
    reason = "Plan 52-04 wires the consolidated inspect unknowns command to this collector."
)]
pub(crate) fn all_unknowns(db: &AnalysisDb) -> Vec<UnknownRow> {
    all_unknowns_with_diagnostics(db, &[])
}

pub(crate) fn all_unknowns_with_diagnostics(
    db: &AnalysisDb,
    diagnostics: &[Diagnostic],
) -> Vec<UnknownRow> {
    let mut rows = Vec::new();
    for capability in ["resolved_imports", "symbols", "references"] {
        rows.extend(public_capability_unknowns(db, capability));
    }
    rows.extend(graph_engine_unknowns_with_diagnostics(db, diagnostics));
    normalize_rows(rows)
}

fn resolved_import_unknowns(db: &AnalysisDb) -> Vec<UnknownRow> {
    db.resolved_imports()
        .iter()
        .filter(|fact| {
            !matches!(
                fact.status,
                ResolutionStatus::Resolved | ResolutionStatus::External
            )
        })
        .map(|fact| {
            let reason = fact.reason.map(unresolved_reason_label).map(str::to_string);
            UnknownRow::new(UnknownRowInput {
                category: resolution_category(fact.status, fact.reason),
                capability: Some("resolved_imports".to_string()),
                family: Some("ResolvedImport".to_string()),
                provider: "polint.module_graph".to_string(),
                file: db.path_for(fact.from_file),
                span: None,
                status: resolution_status_label(fact.status).to_string(),
                reason,
                precision: Some(resolution_precision_label(fact.precision).to_string()),
                docs_path: Some("docs/facts/resolved-imports.md".to_string()),
                suggested_artifact: Some(artifact_for_resolution_status(fact.status).to_string()),
                source_stable_key: stable_key_for(db, FactFamily::ResolvedImport, fact.id.0),
            })
        })
        .collect()
}

fn symbol_unknowns(db: &AnalysisDb) -> Vec<UnknownRow> {
    db.symbols()
        .iter()
        .filter(|symbol| {
            matches!(
                symbol.precision,
                SymbolPrecision::Unresolved
                    | SymbolPrecision::Ambiguous
                    | SymbolPrecision::SetupMissing
                    | SymbolPrecision::Unsupported
            )
        })
        .map(|symbol| {
            let status = symbol_precision_label(symbol.precision);
            UnknownRow::new(UnknownRowInput {
                category: symbol_precision_category(symbol.precision),
                capability: Some("symbols".to_string()),
                family: Some("Symbol".to_string()),
                provider: "polint.symbol_graph".to_string(),
                file: symbol
                    .file
                    .map(|file| db.path_for(file))
                    .unwrap_or_else(|| "<workspace>".to_string()),
                span: symbol.primary_span.as_ref().map(UnknownSpan::from_span),
                status: status.to_string(),
                reason: Some("symbol precision is not exact".to_string()),
                precision: Some(status.to_string()),
                docs_path: Some("docs/facts/symbols-and-references.md".to_string()),
                suggested_artifact: Some("model".to_string()),
                source_stable_key: Some(symbol.stable_key.clone()),
            })
        })
        .collect()
}

fn reference_unknowns(db: &AnalysisDb) -> Vec<UnknownRow> {
    db.references()
        .iter()
        .filter(|reference| reference.status != SymbolResolutionStatus::Resolved)
        .map(|reference| {
            UnknownRow::new(UnknownRowInput {
                category: symbol_status_category(reference.status),
                capability: Some("references".to_string()),
                family: Some("Reference".to_string()),
                provider: "polint.symbol_graph".to_string(),
                file: reference
                    .file
                    .map(|file| db.path_for(file))
                    .unwrap_or_else(|| "<workspace>".to_string()),
                span: reference.primary_span.as_ref().map(UnknownSpan::from_span),
                status: symbol_status_label(reference.status).to_string(),
                reason: Some("reference did not resolve to exactly one public symbol".to_string()),
                precision: Some(symbol_precision_label(reference.precision).to_string()),
                docs_path: Some("docs/facts/symbols-and-references.md".to_string()),
                suggested_artifact: Some("model".to_string()),
                source_stable_key: Some(reference.stable_key.clone()),
            })
        })
        .collect()
}

fn go_semantic_unknowns(db: &AnalysisDb) -> Vec<UnknownRow> {
    db.go_semantic_package_errors()
        .iter()
        .map(|error| {
            let category = go_package_error_category(&error.message);
            UnknownRow::new(UnknownRowInput {
                category,
                capability: Some("go_semantic".to_string()),
                family: Some("GoSemanticPackageError".to_string()),
                provider: "polint.go.semantic".to_string(),
                file: "<workspace>".to_string(),
                span: None,
                status: category.as_str().to_string(),
                reason: Some(error.message.clone()),
                precision: Some("unsupported".to_string()),
                docs_path: Some("docs/facts/capability-plans.md".to_string()),
                suggested_artifact: Some("go_setup".to_string()),
                source_stable_key: Some(error.stable_key.clone()),
            })
        })
        .collect()
}

fn go_semantic_diagnostic_unknowns(diagnostics: &[Diagnostic]) -> Vec<UnknownRow> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.rule_id == "polint/go-semantic")
        .filter_map(|diagnostic| {
            let (category, reason) = go_diagnostic_category_and_reason(&diagnostic.message)?;
            let span = if diagnostic.file == "<workspace>" {
                None
            } else {
                Some(UnknownSpan {
                    line: diagnostic.range.start_line,
                    column: diagnostic.range.start_col,
                })
            };
            Some(UnknownRow::new(UnknownRowInput {
                category,
                capability: Some("go_semantic".to_string()),
                family: Some("GoSemanticDiagnostic".to_string()),
                provider: "polint.go.semantic".to_string(),
                file: diagnostic.file.clone(),
                span,
                status: category.as_str().to_string(),
                reason: Some(reason),
                precision: Some("unsupported".to_string()),
                docs_path: Some("docs/facts/capability-plans.md".to_string()),
                suggested_artifact: Some("go_setup".to_string()),
                source_stable_key: Some(diagnostic.stable_fingerprint.clone()),
            }))
        })
        .collect()
}

fn solver_unknowns(db: &AnalysisDb) -> Vec<UnknownRow> {
    let mut rows = Vec::new();
    if db.solver_budget_status() == crate::analysis::solver::budget::BudgetStatus::BudgetExceeded {
        let reason = if db.solver_budget_reasons().is_empty() {
            "budget_exceeded".to_string()
        } else {
            db.solver_budget_reasons()
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(",")
        };
        rows.push(UnknownRow::new(UnknownRowInput {
            category: UnknownCategory::BudgetExceeded,
            capability: Some("solver".to_string()),
            family: Some("SolverRun".to_string()),
            provider: "polint.solver".to_string(),
            file: "<workspace>".to_string(),
            span: None,
            status: "budget_exceeded".to_string(),
            reason: Some(reason),
            precision: Some("unknown".to_string()),
            docs_path: Some("docs/facts/capability-plans.md".to_string()),
            suggested_artifact: Some("budget_or_model".to_string()),
            source_stable_key: Some("polint.solver:run-level-budget".to_string()),
        }));
    }

    rows.extend(
        db.solver_derived_edges()
            .iter()
            .filter(|edge| edge.status != PointsToStatus::Present)
            .map(|edge| {
                let category = match edge.status {
                    PointsToStatus::BudgetExceeded => UnknownCategory::BudgetExceeded,
                    PointsToStatus::SetupMissing => UnknownCategory::SetupMissing,
                    PointsToStatus::Unsupported => UnknownCategory::UnsupportedSemantic,
                    PointsToStatus::Unknown => UnknownCategory::MissingFact,
                    PointsToStatus::Present => UnknownCategory::MissingFact,
                };
                UnknownRow::new(UnknownRowInput {
                    category,
                    capability: Some("solver".to_string()),
                    family: Some("SolverDerivedEdge".to_string()),
                    provider: "polint.solver".to_string(),
                    file: "<workspace>".to_string(),
                    span: None,
                    status: points_to_status_label(edge.status).to_string(),
                    reason: Some(edge.provenance.constraint_kind.clone()),
                    precision: Some(points_to_precision_label(edge.precision).to_string()),
                    docs_path: Some("docs/facts/capability-plans.md".to_string()),
                    suggested_artifact: Some("budget_or_model".to_string()),
                    source_stable_key: Some(edge.stable_key.clone()),
                })
            }),
    );
    rows
}

fn refined_call_unknowns(db: &AnalysisDb) -> Vec<UnknownRow> {
    db.refined_call_edges()
        .iter()
        .filter(|edge| edge.status != CallTargetStatus::Resolved)
        .map(|edge| refined_call_unknown(db, edge))
        .collect()
}

fn refined_call_unknown(db: &AnalysisDb, edge: &RefinedCallEdgeFact) -> UnknownRow {
    let category = match edge.status {
        CallTargetStatus::SetupMissing => UnknownCategory::SetupMissing,
        CallTargetStatus::Unsupported => UnknownCategory::UnsupportedSemantic,
        CallTargetStatus::BudgetExceeded => UnknownCategory::BudgetExceeded,
        CallTargetStatus::Rejected => UnknownCategory::Rejected,
        CallTargetStatus::Unresolved | CallTargetStatus::Ambiguous => UnknownCategory::MissingFact,
        CallTargetStatus::Resolved => UnknownCategory::MissingFact,
    };
    let site = db.call_sites().iter().find(|site| site.id == edge.site);
    UnknownRow::new(UnknownRowInput {
        category,
        capability: Some("refined_calls".to_string()),
        family: Some("RefinedCallEdge".to_string()),
        provider: "polint.refined_calls".to_string(),
        file: site
            .map(|site| db.path_for(site.file))
            .unwrap_or_else(|| "<workspace>".to_string()),
        span: site.map(|site| UnknownSpan::from_span(&site.span)),
        status: call_status_label(edge.status).to_string(),
        reason: edge
            .reason
            .map(unresolved_call_reason_label)
            .map(str::to_string),
        precision: Some(call_precision_label(edge.precision).to_string()),
        docs_path: Some("docs/facts/capability-plans.md".to_string()),
        suggested_artifact: Some("model".to_string()),
        source_stable_key: Some(edge.stable_key.clone()),
    })
}

fn adaptation_unknowns(db: &AnalysisDb) -> Vec<UnknownRow> {
    db.rejected_adaptation_model_facts()
        .iter()
        .map(|fact| {
            let category = match fact.reason {
                RejectionReason::BudgetExceeded => UnknownCategory::BudgetExceeded,
                RejectionReason::NonResolvingSource | RejectionReason::NonResolvingTarget => {
                    UnknownCategory::ModelMissing
                }
                _ => UnknownCategory::Rejected,
            };
            UnknownRow::new(UnknownRowInput {
                category,
                capability: Some("adaptation_models".to_string()),
                family: Some("RejectedModelFact".to_string()),
                provider: "polint.adaptation.model".to_string(),
                file: fact.fact.model_path.clone(),
                span: None,
                status: "rejected".to_string(),
                reason: Some(fact.reason.as_str().to_string()),
                precision: Some(fact.fact.confidence.as_str().to_string()),
                docs_path: Some("docs/facts/capability-plans.md".to_string()),
                suggested_artifact: Some("model".to_string()),
                source_stable_key: Some(fact.fact.stable_key.clone()),
            })
        })
        .collect()
}

fn stable_key_for(db: &AnalysisDb, family: FactFamily, run_id: u64) -> Option<String> {
    db.metadata_for(FactRef::new(family, run_id))
        .map(|metadata| metadata.stable_key.clone())
}

fn resolution_category(
    status: ResolutionStatus,
    reason: Option<UnresolvedReason>,
) -> UnknownCategory {
    match (status, reason) {
        (ResolutionStatus::SetupMissing, _) | (_, Some(UnresolvedReason::SetupMissing)) => {
            UnknownCategory::SetupMissing
        }
        (ResolutionStatus::Unsupported, _)
        | (_, Some(UnresolvedReason::UnsupportedLanguage))
        | (_, Some(UnresolvedReason::UnsupportedImport)) => UnknownCategory::UnsupportedSemantic,
        (ResolutionStatus::Dynamic, _) => UnknownCategory::OutOfScope,
        (_, Some(UnresolvedReason::OutsideWorkspace)) => UnknownCategory::OutOfScope,
        _ => UnknownCategory::MissingFact,
    }
}

fn symbol_precision_category(precision: SymbolPrecision) -> UnknownCategory {
    match precision {
        SymbolPrecision::SetupMissing => UnknownCategory::SetupMissing,
        SymbolPrecision::Unsupported => UnknownCategory::UnsupportedSemantic,
        SymbolPrecision::Unresolved | SymbolPrecision::Ambiguous => UnknownCategory::MissingFact,
        _ => UnknownCategory::MissingFact,
    }
}

fn symbol_status_category(status: SymbolResolutionStatus) -> UnknownCategory {
    match status {
        SymbolResolutionStatus::SetupMissing => UnknownCategory::SetupMissing,
        SymbolResolutionStatus::Unsupported => UnknownCategory::UnsupportedSemantic,
        SymbolResolutionStatus::Unresolved | SymbolResolutionStatus::Ambiguous => {
            UnknownCategory::MissingFact
        }
        SymbolResolutionStatus::Resolved => UnknownCategory::MissingFact,
    }
}

fn go_package_error_category(message: &str) -> UnknownCategory {
    if message.contains("GoSidecarTimeout") {
        UnknownCategory::GoSidecarTimeout
    } else if message.contains("Go 1.25") || message.contains("go version") {
        UnknownCategory::GoVersionUnsupported
    } else {
        UnknownCategory::GoPackagesLoadFailed
    }
}

fn go_diagnostic_category_and_reason(message: &str) -> Option<(UnknownCategory, String)> {
    [
        (
            GO_PACKAGES_LOAD_FAILED,
            UnknownCategory::GoPackagesLoadFailed,
        ),
        (
            GO_VERSION_UNSUPPORTED,
            UnknownCategory::GoVersionUnsupported,
        ),
        (GO_SIDECAR_TIMEOUT, UnknownCategory::GoSidecarTimeout),
    ]
    .into_iter()
    .find_map(|(prefix, category)| {
        message.strip_prefix(prefix).map(|reason| {
            let reason = reason
                .strip_prefix(':')
                .unwrap_or(reason)
                .trim_start()
                .to_string();
            (category, reason)
        })
    })
    .filter(|(category, reason)| go_diagnostic_represents_setup_unknown(*category, reason))
}

fn go_diagnostic_represents_setup_unknown(category: UnknownCategory, reason: &str) -> bool {
    match category {
        UnknownCategory::GoSidecarTimeout | UnknownCategory::GoVersionUnsupported => true,
        UnknownCategory::GoPackagesLoadFailed => {
            !reason.starts_with("package ")
                && !reason.contains(" failed to load:")
                && !reason.contains("Go RTA-signal rows dropped")
                && !reason.contains("duplicate Go ")
        }
        _ => false,
    }
}

fn resolution_status_label(status: ResolutionStatus) -> &'static str {
    match status {
        ResolutionStatus::Resolved => "resolved",
        ResolutionStatus::External => "external",
        ResolutionStatus::Unresolved => "unresolved",
        ResolutionStatus::SetupMissing => "setup_missing",
        ResolutionStatus::Dynamic => "dynamic",
        ResolutionStatus::Unsupported => "unsupported",
    }
}

fn resolution_precision_label(precision: ResolutionPrecision) -> &'static str {
    match precision {
        ResolutionPrecision::ExactFile => "exact_file",
        ResolutionPrecision::Package => "package",
        ResolutionPrecision::ExternalPackage => "external_package",
        ResolutionPrecision::Heuristic => "heuristic",
        ResolutionPrecision::None => "none",
    }
}

fn unresolved_reason_label(reason: UnresolvedReason) -> &'static str {
    match reason {
        UnresolvedReason::NotFound => "not_found",
        UnresolvedReason::SetupMissing => "setup_missing",
        UnresolvedReason::DynamicExpression => "dynamic_expression",
        UnresolvedReason::UnsupportedLanguage => "unsupported_language",
        UnresolvedReason::UnsupportedImport => "unsupported_import",
        UnresolvedReason::ResolverError => "resolver_error",
        UnresolvedReason::OutsideWorkspace => "outside_workspace",
    }
}

fn symbol_status_label(status: SymbolResolutionStatus) -> &'static str {
    match status {
        SymbolResolutionStatus::Resolved => "resolved",
        SymbolResolutionStatus::Unresolved => "unresolved",
        SymbolResolutionStatus::Ambiguous => "ambiguous",
        SymbolResolutionStatus::SetupMissing => "setup_missing",
        SymbolResolutionStatus::Unsupported => "unsupported",
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

fn artifact_for_resolution_status(status: ResolutionStatus) -> &'static str {
    match status {
        ResolutionStatus::SetupMissing => "fixture",
        ResolutionStatus::Dynamic | ResolutionStatus::Unsupported => "provider",
        ResolutionStatus::Unresolved => "model",
        ResolutionStatus::External | ResolutionStatus::Resolved => "rule",
    }
}

fn points_to_status_label(status: PointsToStatus) -> &'static str {
    match status {
        PointsToStatus::Present => "present",
        PointsToStatus::Unknown => "unknown",
        PointsToStatus::Unsupported => "unsupported",
        PointsToStatus::SetupMissing => "setup_missing",
        PointsToStatus::BudgetExceeded => "budget_exceeded",
    }
}

fn points_to_precision_label(precision: PointsToPrecision) -> &'static str {
    match precision {
        PointsToPrecision::FlowInsensitive => "flow_insensitive",
        PointsToPrecision::LocalFlowSensitive => "local_flow_sensitive",
        PointsToPrecision::SummaryProjected => "summary_projected",
        PointsToPrecision::Heuristic => "heuristic",
        PointsToPrecision::Unknown => "unknown",
        PointsToPrecision::Unsupported => "unsupported",
    }
}

fn call_status_label(status: CallTargetStatus) -> &'static str {
    match status {
        CallTargetStatus::Resolved => "resolved",
        CallTargetStatus::Ambiguous => "ambiguous",
        CallTargetStatus::Unresolved => "unresolved",
        CallTargetStatus::Unsupported => "unsupported",
        CallTargetStatus::SetupMissing => "setup_missing",
        CallTargetStatus::BudgetExceeded => "budget_exceeded",
        CallTargetStatus::Rejected => "rejected",
    }
}

fn call_precision_label(precision: crate::analysis::calls::facts::CallPrecision) -> &'static str {
    match precision {
        crate::analysis::calls::facts::CallPrecision::Exact => "exact",
        crate::analysis::calls::facts::CallPrecision::SetupAware => "setup_aware",
        crate::analysis::calls::facts::CallPrecision::Conservative => "conservative",
        crate::analysis::calls::facts::CallPrecision::Heuristic => "heuristic",
        crate::analysis::calls::facts::CallPrecision::Ambiguous => "ambiguous",
        crate::analysis::calls::facts::CallPrecision::Unknown => "unknown",
        crate::analysis::calls::facts::CallPrecision::Unsupported => "unsupported",
    }
}

fn unresolved_call_reason_label(reason: UnresolvedCallReason) -> &'static str {
    match reason {
        UnresolvedCallReason::FunctionValue => "function_value",
        UnresolvedCallReason::DynamicProperty => "dynamic_property",
        UnresolvedCallReason::InterfaceDispatch => "interface_dispatch",
        UnresolvedCallReason::Eval => "eval",
        UnresolvedCallReason::CallApplyBind => "call_apply_bind",
        UnresolvedCallReason::FrameworkDispatch => "framework_dispatch",
        UnresolvedCallReason::Reflection => "reflection",
        UnresolvedCallReason::GoroutineBoundary => "goroutine_boundary",
        UnresolvedCallReason::DynamicImport => "dynamic_import",
        UnresolvedCallReason::ProxyOrAccessor => "proxy_or_accessor",
        UnresolvedCallReason::MissingSemanticReference => "missing_semantic_reference",
        UnresolvedCallReason::MissingImportResolution => "missing_import_resolution",
        UnresolvedCallReason::SetupMissing => "setup_missing",
        UnresolvedCallReason::UnsupportedSyntax => "unsupported_syntax",
        UnresolvedCallReason::BudgetExceeded => "budget_exceeded",
        UnresolvedCallReason::UnknownCallee => "unknown_callee",
        UnresolvedCallReason::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::adaptation::facts::{
        LoadedModelFact, ModelConfidence, ModelLanguage, RejectedModelFact,
    };
    use crate::analysis::calls::facts::{
        CallAlgorithm, CallCallee, CallEdgeKind, CallPrecision, CallProvenance, CallSiteFact,
        CallSyntaxKind,
    };
    use crate::analysis::calls::store::CallOutput;
    use crate::analysis::ids::{
        CallSiteId, DerivedEdgeId, MirBodyId, MirOpId, RefinedCallEdgeId, SemanticNodeId,
    };
    use crate::analysis::refined_calls::facts::{
        RefinedCallConfidence, RefinedCallTier, RefinedCallValidation,
    };
    use crate::analysis::refined_calls::store::RefinedCallOutput;
    use crate::analysis::semantic_graph::constraints::ConstraintKind;
    use crate::analysis::solver::budget::BudgetStatus;
    use crate::analysis::solver::facts::DerivedEdgeFact;
    use crate::analysis::solver::provenance::{ContributingFact, DerivedEdgeProvenance};
    use crate::analysis::solver::store::SolverOutput;
    use crate::core::{
        FunctionId, ImportFact, ImportId, Language, ModuleEdge, ModuleNode, ResolutionStatus,
        ResolvedImportFact, ResolvedImportId, Span, UnresolvedReason,
    };
    use crate::diagnostics::{Diagnostic, TextRange};
    use crate::go::semantic::facts::{GoSemanticPackageErrorFact, GoSemanticPackageErrorId};
    use crate::go::semantic::store::GoSemanticFactsOutput;
    use std::collections::BTreeSet;

    #[test]
    fn public_import_unknowns_preserve_current_status_docs_and_artifact() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            "a.ts".into(),
            "a.ts".to_string(),
            "import x from 'x';".to_string(),
        );
        let import = db.push_import(ImportFact {
            id: ImportId(0),
            file,
            package: None,
            path: "x".to_string(),
            span: Span::point(file, 1, 1),
            language: Language::TypeScript,
        });
        db.replace_module_graph_facts(
            vec![ResolvedImportFact {
                id: ResolvedImportId(0),
                import,
                from_file: file,
                target_node: None,
                status: ResolutionStatus::SetupMissing,
                precision: ResolutionPrecision::None,
                reason: Some(UnresolvedReason::SetupMissing),
            }],
            Vec::<ModuleNode>::new(),
            Vec::<ModuleEdge>::new(),
        );

        let rows = public_capability_unknowns(&db, "resolved_imports");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].category, UnknownCategory::SetupMissing);
        assert_eq!(rows[0].status, "setup_missing");
        assert_eq!(
            rows[0].docs_path.as_deref(),
            Some("docs/facts/resolved-imports.md")
        );
        assert_eq!(rows[0].suggested_artifact.as_deref(), Some("fixture"));
    }

    #[test]
    fn graph_engine_unknowns_include_go_solver_model_and_refined_rows() {
        let mut db = AnalysisDb::new();
        db.replace_go_semantic_facts(GoSemanticFactsOutput {
            package_errors: vec![
                go_error("go:load", "package load failed"),
                go_error(
                    "go:version",
                    "polint-go-frontend source mode requires Go 1.25",
                ),
                go_error("go:timeout", "GoSidecarTimeout: request timeout"),
            ],
            ..GoSemanticFactsOutput::default()
        })
        .expect("go semantic facts");
        db.replace_solver_facts(SolverOutput {
            derived_edges: vec![DerivedEdgeFact {
                id: DerivedEdgeId(0),
                source: SemanticNodeId(0),
                target: SemanticNodeId(1),
                status: PointsToStatus::BudgetExceeded,
                precision: PointsToPrecision::Unknown,
                stable_key: "solver:budget".to_string(),
                provenance: DerivedEdgeProvenance::new(
                    vec![ContributingFact {
                        stable_key: "constraint:call".to_string(),
                    }],
                    &ConstraintKind::CallConstraint {
                        callsite: SemanticNodeId(2),
                    },
                    1,
                ),
            }],
            budget_status: BudgetStatus::BudgetExceeded,
            budget_reasons: BTreeSet::from(["solver.max_steps".to_string()]),
        })
        .expect("solver facts");
        let file = db.add_file("a.ts".into(), "a.ts".to_string(), "call();".to_string());
        db.replace_call_facts(CallOutput {
            sites: vec![CallSiteFact {
                in_throw: false,
                id: CallSiteId(0),
                language: Language::TypeScript,
                file,
                caller: FunctionId(0),
                owner_symbol: None,
                body: MirBodyId(0),
                operation: MirOpId(0),
                span: Span::point(file, 1, 1),
                kind: CallSyntaxKind::Function,
                callee: CallCallee::Unknown {
                    reason: UnresolvedCallReason::BudgetExceeded,
                },
                receiver: None,
                arguments: Vec::new(),
                result: None,
                status: CallTargetStatus::BudgetExceeded,
                precision: CallPrecision::Unknown,
                stable_key: "callsite:budget".to_string(),
            }],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("call facts");
        db.replace_refined_call_facts(RefinedCallOutput {
            edges: vec![refined_budget_edge()],
        })
        .expect("refined calls");
        db.replace_adaptation_model_facts(
            Vec::new(),
            vec![RejectedModelFact {
                fact: model_fact(),
                reason: RejectionReason::NonResolvingTarget,
            }],
        );

        let categories = graph_engine_unknowns(&db)
            .into_iter()
            .map(|row| row.category)
            .collect::<Vec<_>>();

        assert!(categories.contains(&UnknownCategory::GoPackagesLoadFailed));
        assert!(categories.contains(&UnknownCategory::GoVersionUnsupported));
        assert!(categories.contains(&UnknownCategory::GoSidecarTimeout));
        assert!(categories.contains(&UnknownCategory::BudgetExceeded));
        assert!(categories.contains(&UnknownCategory::ModelMissing));
        assert!(
            graph_engine_unknowns(&db).iter().any(|row| {
                row.family.as_deref() == Some("SolverRun")
                    && row.reason.as_deref() == Some("solver.max_steps")
            }),
            "run-level solver budget reason must survive storage and taxonomy projection"
        );
    }

    #[test]
    fn graph_engine_unknowns_include_run_level_solver_budget_without_edge_rows() {
        let mut db = AnalysisDb::new();
        db.replace_solver_facts(SolverOutput {
            budget_status: BudgetStatus::BudgetExceeded,
            budget_reasons: BTreeSet::from(["go.max_rta_rounds".to_string()]),
            ..SolverOutput::default()
        })
        .expect("solver facts");

        let rows = graph_engine_unknowns(&db);

        assert!(rows.iter().any(|row| {
            row.family.as_deref() == Some("SolverRun")
                && row.category == UnknownCategory::BudgetExceeded
                && row.reason.as_deref() == Some("go.max_rta_rounds")
        }));
    }

    #[test]
    fn graph_engine_unknowns_include_go_provider_diagnostics_without_facts() {
        let db = AnalysisDb::new();
        let diagnostics = vec![
            go_diagnostic(
                "GoPackagesLoadFailed: POLINT_GO_FRONTEND must point to a polint-go-frontend binary or source directory.",
            ),
            go_diagnostic("GoSidecarTimeout: request timeout"),
            go_diagnostic("GoVersionUnsupported: detected go version go1.24.5"),
        ];

        let rows = graph_engine_unknowns_with_diagnostics(&db, &diagnostics);
        let categories = rows.iter().map(|row| row.category).collect::<Vec<_>>();

        assert!(categories.contains(&UnknownCategory::GoPackagesLoadFailed));
        assert!(categories.contains(&UnknownCategory::GoSidecarTimeout));
        assert!(categories.contains(&UnknownCategory::GoVersionUnsupported));
        assert!(
            rows.iter()
                .all(|row| row.family.as_deref() == Some("GoSemanticDiagnostic"))
        );
    }

    #[test]
    fn graph_engine_unknowns_do_not_duplicate_package_error_or_quality_diagnostics() {
        let mut db = AnalysisDb::new();
        db.replace_go_semantic_facts(GoSemanticFactsOutput {
            package_errors: vec![go_error("go:load", "load failed")],
            ..GoSemanticFactsOutput::default()
        })
        .expect("go semantic facts");
        let diagnostics = vec![
            go_diagnostic(
                "GoPackagesLoadFailed: package example.com/p failed to load: load failed",
            ),
            go_diagnostic(
                "GoPackagesLoadFailed: 2 Go RTA-signal rows dropped (invalid identity/stable_key); interface/func-value dispatch may under-resolve.",
            ),
            go_diagnostic(
                "GoPackagesLoadFailed: 1 duplicate Go function stable key row(s) collapsed keep-first (byte-identical double-emit); facts preserved.",
            ),
        ];

        let rows = graph_engine_unknowns_with_diagnostics(&db, &diagnostics);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].family.as_deref(), Some("GoSemanticPackageError"));
        assert_eq!(rows[0].reason.as_deref(), Some("load failed"));
    }

    fn go_error(stable_key: &str, message: &str) -> GoSemanticPackageErrorFact {
        GoSemanticPackageErrorFact {
            id: GoSemanticPackageErrorId(0),
            stable_key: stable_key.to_string(),
            package_id: "pkg".to_string(),
            package_path: "pkg".to_string(),
            message: message.to_string(),
        }
    }

    fn go_diagnostic(message: &str) -> Diagnostic {
        Diagnostic::warning(
            "polint/go-semantic",
            "<workspace>",
            TextRange::point(1, 1),
            message,
        )
    }

    fn refined_budget_edge() -> RefinedCallEdgeFact {
        RefinedCallEdgeFact {
            id: RefinedCallEdgeId(0),
            site: CallSiteId(0),
            base_target: None,
            caller: FunctionId(0),
            target_function: None,
            target_symbol: None,
            synthetic_target: None,
            language: Language::TypeScript,
            edge_kind: CallEdgeKind::FunctionValue,
            algorithm: CallAlgorithm::PointsTo,
            tier: RefinedCallTier::PointsToAssisted,
            status: CallTargetStatus::BudgetExceeded,
            reason: Some(UnresolvedCallReason::BudgetExceeded),
            provenance: CallProvenance::Model,
            precision: CallPrecision::Unknown,
            validation: RefinedCallValidation::ReferentiallyValidated,
            confidence: RefinedCallConfidence::Low,
            evidence: vec!["solver_derived_edge".to_string()],
            input_stable_keys: vec!["solver:budget".to_string()],
            stable_key: "refined:budget".to_string(),
        }
    }

    fn model_fact() -> LoadedModelFact {
        LoadedModelFact {
            model_path: ".polint/models.toml".to_string(),
            source_pattern: "a".to_string(),
            target_pattern: "b".to_string(),
            confidence: ModelConfidence::Heuristic,
            language: ModelLanguage::TypeScript,
            scope: "repo".to_string(),
            evidence: vec!["fixture".to_string()],
            stable_key: "model:missing".to_string(),
        }
    }
}

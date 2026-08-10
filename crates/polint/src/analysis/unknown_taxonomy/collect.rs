use crate::analysis::adaptation::facts::RejectionReason;
use crate::analysis::calls::facts::{CallTargetStatus, UnresolvedCallReason};
use crate::analysis::data_flow::facts::{
    DataFlowBudgetFact, DataFlowBudgetReason, DataFlowEdgeFact, DataFlowEdgeKind,
    DataFlowPrecision, DataFlowStatus,
};
use crate::analysis::evidence::facts::{EvidenceUnknownFact, EvidenceUnknownReason};
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

pub(crate) const PUBLIC_UNKNOWN_CAPABILITIES: &[&str] = &[
    "resolved_imports",
    "symbols",
    "references",
    "events",
    "calls",
    "control_flow",
    "dataflow",
];

pub(crate) fn unsupported_capability_row(
    interner: &crate::core::StableKeyInterner,
    capability: &str,
    docs_path: Option<&str>,
) -> UnknownRow {
    UnknownRow::new(
        interner,
        UnknownRowInput {
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
        },
    )
}

pub(crate) fn public_capability_unknowns(db: &AnalysisDb, capability: &str) -> Vec<UnknownRow> {
    let rows = match capability {
        "resolved_imports" => resolved_import_unknowns(db),
        "symbols" => symbol_unknowns(db),
        "references" => reference_unknowns(db),
        "events" | "calls" | "control_flow" => policy_call_unknowns(db, capability),
        "dataflow" => {
            let mut rows = data_flow_unknowns(db);
            rows.extend(policy_call_unknowns(db, capability));
            rows
        }
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
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    let mut rows = Vec::new();
    rows.extend(go_semantic_unknowns(db));
    rows.extend(go_semantic_diagnostic_unknowns(interner, diagnostics));
    rows.extend(unsupported_semantic_unknowns(db));
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
    for capability in PUBLIC_UNKNOWN_CAPABILITIES {
        rows.extend(public_capability_unknowns(db, capability));
    }
    rows.extend(graph_engine_unknowns_with_diagnostics(db, diagnostics));
    normalize_rows(rows)
}

fn resolved_import_unknowns(db: &AnalysisDb) -> Vec<UnknownRow> {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
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
            UnknownRow::new(
                interner,
                UnknownRowInput {
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
                    suggested_artifact: Some(
                        artifact_for_resolution_status(fact.status).to_string(),
                    ),
                    source_stable_key: stable_key_for(db, FactFamily::ResolvedImport, fact.id.0),
                },
            )
        })
        .collect()
}

fn symbol_unknowns(db: &AnalysisDb) -> Vec<UnknownRow> {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
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
            UnknownRow::new(
                interner,
                UnknownRowInput {
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
                    source_stable_key: Some(interner.resolve(symbol.stable_key).to_string()),
                },
            )
        })
        .collect()
}

fn reference_unknowns(db: &AnalysisDb) -> Vec<UnknownRow> {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    db.references()
        .iter()
        .filter(|reference| reference.status != SymbolResolutionStatus::Resolved)
        .map(|reference| {
            UnknownRow::new(
                interner,
                UnknownRowInput {
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
                    reason: Some(
                        "reference did not resolve to exactly one public symbol".to_string(),
                    ),
                    precision: Some(symbol_precision_label(reference.precision).to_string()),
                    docs_path: Some("docs/facts/symbols-and-references.md".to_string()),
                    suggested_artifact: Some("model".to_string()),
                    source_stable_key: Some(interner.resolve(reference.stable_key).to_string()),
                },
            )
        })
        .collect()
}

fn go_semantic_unknowns(db: &AnalysisDb) -> Vec<UnknownRow> {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    db.go_semantic_package_errors()
        .iter()
        .map(|error| {
            let category = go_package_error_category(&error.message);
            UnknownRow::new(
                interner,
                UnknownRowInput {
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
                    source_stable_key: Some(interner.resolve(error.stable_key).to_string()),
                },
            )
        })
        .collect()
}

fn go_semantic_diagnostic_unknowns(
    interner: &crate::core::StableKeyInterner,
    diagnostics: &[Diagnostic],
) -> Vec<UnknownRow> {
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
            Some(UnknownRow::new(
                interner,
                UnknownRowInput {
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
                },
            ))
        })
        .collect()
}

fn unsupported_semantic_unknowns(db: &AnalysisDb) -> Vec<UnknownRow> {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    db.unsupported_semantics()
        .iter()
        .map(|row| {
            UnknownRow::new(
                interner,
                UnknownRowInput {
                    category: UnknownCategory::UnsupportedSemantic,
                    capability: Some("semantic_mir".to_string()),
                    family: Some("UnsupportedSemantic".to_string()),
                    provider: "polint.semantic_mir".to_string(),
                    file: db.path_for(row.file),
                    span: Some(UnknownSpan::from_span(&row.span)),
                    status: "unsupported".to_string(),
                    reason: Some(row.construct.clone()),
                    precision: Some("unsupported".to_string()),
                    docs_path: Some("docs/facts/capability-plans.md".to_string()),
                    suggested_artifact: Some("provider".to_string()),
                    source_stable_key: Some(interner.resolve(row.stable_key).to_string()),
                },
            )
        })
        .collect()
}

fn solver_unknowns(db: &AnalysisDb) -> Vec<UnknownRow> {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
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
        rows.push(UnknownRow::new(
            interner,
            UnknownRowInput {
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
            },
        ));
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
                UnknownRow::new(
                    interner,
                    UnknownRowInput {
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
                    },
                )
            }),
    );
    rows
}

fn refined_call_unknowns(db: &AnalysisDb) -> Vec<UnknownRow> {
    db.refined_call_edges()
        .iter()
        .filter(|edge| edge.status != CallTargetStatus::Resolved)
        .map(|edge| {
            refined_call_unknown(
                db,
                edge,
                "refined_calls",
                "docs/facts/capability-plans.md",
                "model",
            )
        })
        .collect()
}

fn refined_call_unknown(
    db: &AnalysisDb,
    edge: &RefinedCallEdgeFact,
    capability: &str,
    docs_path: &str,
    suggested_artifact: &str,
) -> UnknownRow {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    let category = match edge.status {
        CallTargetStatus::SetupMissing => UnknownCategory::SetupMissing,
        CallTargetStatus::Unsupported => UnknownCategory::UnsupportedSemantic,
        CallTargetStatus::BudgetExceeded => UnknownCategory::BudgetExceeded,
        CallTargetStatus::Rejected => UnknownCategory::Rejected,
        CallTargetStatus::Unresolved | CallTargetStatus::Ambiguous => UnknownCategory::MissingFact,
        CallTargetStatus::Resolved => UnknownCategory::MissingFact,
    };
    let site = db.call_sites().iter().find(|site| site.id == edge.site);
    UnknownRow::new(
        interner,
        UnknownRowInput {
            category,
            capability: Some(capability.to_string()),
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
            docs_path: Some(docs_path.to_string()),
            suggested_artifact: Some(suggested_artifact.to_string()),
            source_stable_key: Some(db.resolve_stable_key(edge.stable_key).to_string()),
        },
    )
}

fn adaptation_unknowns(db: &AnalysisDb) -> Vec<UnknownRow> {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
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
            UnknownRow::new(
                interner,
                UnknownRowInput {
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
                    source_stable_key: Some(interner.resolve(fact.fact.stable_key).to_string()),
                },
            )
        })
        .collect()
}

fn policy_call_unknowns(db: &AnalysisDb, capability: &str) -> Vec<UnknownRow> {
    db.refined_call_edges()
        .iter()
        .filter(|edge| edge.status != CallTargetStatus::Resolved)
        .map(|edge| {
            refined_call_unknown(
                db,
                edge,
                capability,
                policy_capability_docs_path(capability),
                "model_or_setup",
            )
        })
        .collect()
}

fn data_flow_unknowns(db: &AnalysisDb) -> Vec<UnknownRow> {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    let mut rows = Vec::new();
    rows.extend(
        db.data_flow_edges()
            .iter()
            .filter(|edge| edge.status != DataFlowStatus::Present)
            .map(|edge| data_flow_edge_unknown(db, edge)),
    );
    rows.extend(
        db.data_flow_budgets()
            .iter()
            .filter(|budget| budget.status != DataFlowStatus::Present)
            .map(|budget| data_flow_budget_unknown(interner, budget)),
    );
    rows.extend(
        db.evidence_unknowns()
            .iter()
            .map(|unknown| evidence_unknown_for_data_flow(interner, unknown)),
    );
    rows
}

fn data_flow_edge_unknown(db: &AnalysisDb, edge: &DataFlowEdgeFact) -> UnknownRow {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    let (file, span) = data_flow_edge_location(db, edge);
    UnknownRow::new(
        interner,
        UnknownRowInput {
            category: data_flow_status_category(edge.status),
            capability: Some("dataflow".to_string()),
            family: Some("DataFlowEdge".to_string()),
            provider: "polint.data_flow".to_string(),
            file,
            span,
            status: data_flow_status_label(edge.status).to_string(),
            reason: Some(data_flow_edge_reason(edge)),
            precision: Some(data_flow_precision_label(edge.precision).to_string()),
            docs_path: Some("docs/facts/data-flow.md".to_string()),
            suggested_artifact: Some(data_flow_status_artifact(edge.status).to_string()),
            source_stable_key: Some(interner.resolve(edge.stable_key).to_string()),
        },
    )
}

fn data_flow_budget_unknown(
    interner: &crate::core::StableKeyInterner,
    budget: &DataFlowBudgetFact,
) -> UnknownRow {
    UnknownRow::new(
        interner,
        UnknownRowInput {
            category: UnknownCategory::BudgetExceeded,
            capability: Some("dataflow".to_string()),
            family: Some("DataFlowBudget".to_string()),
            provider: "polint.data_flow".to_string(),
            file: "<workspace>".to_string(),
            span: None,
            status: data_flow_status_label(budget.status).to_string(),
            reason: Some(format!(
                "{} limit={} observed={}",
                data_flow_budget_reason_label(budget.reason),
                budget.limit,
                budget.observed
            )),
            precision: Some("unknown".to_string()),
            docs_path: Some("docs/facts/data-flow.md".to_string()),
            suggested_artifact: Some("budget_or_model".to_string()),
            source_stable_key: Some(interner.resolve(budget.stable_key).to_string()),
        },
    )
}

fn evidence_unknown_for_data_flow(
    interner: &crate::core::StableKeyInterner,
    unknown: &EvidenceUnknownFact,
) -> UnknownRow {
    UnknownRow::new(
        interner,
        UnknownRowInput {
            category: evidence_unknown_category(unknown.reason),
            capability: Some("dataflow".to_string()),
            family: Some("EvidenceUnknown".to_string()),
            provider: "polint.evidence".to_string(),
            file: "<workspace>".to_string(),
            span: None,
            status: "unknown".to_string(),
            reason: Some(format!(
                "{}:{}",
                evidence_unknown_reason_label(unknown.reason),
                unknown.message
            )),
            precision: Some("unknown".to_string()),
            docs_path: Some("docs/facts/evidence.md".to_string()),
            suggested_artifact: Some("model_or_budget".to_string()),
            source_stable_key: Some(interner.resolve(unknown.stable_key).to_string()),
        },
    )
}

fn data_flow_edge_location(
    db: &AnalysisDb,
    edge: &DataFlowEdgeFact,
) -> (String, Option<UnknownSpan>) {
    let node = db
        .data_flow_nodes()
        .iter()
        .find(|node| node.id == edge.to)
        .or_else(|| {
            db.data_flow_nodes()
                .iter()
                .find(|node| node.id == edge.from)
        });
    let file = node
        .and_then(|node| node.file)
        .map(|file| db.path_for(file))
        .unwrap_or_else(|| "<workspace>".to_string());
    let span = node
        .and_then(|node| node.span.as_ref())
        .map(UnknownSpan::from_span);
    (file, span)
}

fn data_flow_status_category(status: DataFlowStatus) -> UnknownCategory {
    match status {
        DataFlowStatus::SetupMissing => UnknownCategory::SetupMissing,
        DataFlowStatus::Unsupported => UnknownCategory::UnsupportedSemantic,
        DataFlowStatus::BudgetExceeded => UnknownCategory::BudgetExceeded,
        DataFlowStatus::Rejected => UnknownCategory::Rejected,
        DataFlowStatus::Unknown | DataFlowStatus::Present => UnknownCategory::MissingFact,
    }
}

fn data_flow_status_artifact(status: DataFlowStatus) -> &'static str {
    match status {
        DataFlowStatus::SetupMissing => "fixture",
        DataFlowStatus::Unsupported => "provider",
        DataFlowStatus::BudgetExceeded => "budget_or_model",
        DataFlowStatus::Rejected => "model",
        DataFlowStatus::Unknown | DataFlowStatus::Present => "model",
    }
}

fn evidence_unknown_category(reason: EvidenceUnknownReason) -> UnknownCategory {
    match reason {
        EvidenceUnknownReason::SetupMissing => UnknownCategory::SetupMissing,
        EvidenceUnknownReason::UnsupportedEdge => UnknownCategory::UnsupportedSemantic,
        EvidenceUnknownReason::BudgetExceeded => UnknownCategory::BudgetExceeded,
        EvidenceUnknownReason::DynamicCall | EvidenceUnknownReason::OpaqueSummary => {
            UnknownCategory::MissingFact
        }
    }
}

fn policy_capability_docs_path(capability: &str) -> &'static str {
    match capability {
        "events" => "docs/facts/events.md",
        "calls" => "docs/facts/calls.md",
        "control_flow" => "docs/facts/control-flow.md",
        "dataflow" => "docs/facts/data-flow.md",
        _ => "docs/facts/capability-plans.md",
    }
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

fn data_flow_status_label(status: DataFlowStatus) -> &'static str {
    match status {
        DataFlowStatus::Present => "present",
        DataFlowStatus::Unknown => "unknown",
        DataFlowStatus::Unsupported => "unsupported",
        DataFlowStatus::SetupMissing => "setup_missing",
        DataFlowStatus::BudgetExceeded => "budget_exceeded",
        DataFlowStatus::Rejected => "rejected",
    }
}

fn data_flow_precision_label(precision: DataFlowPrecision) -> &'static str {
    match precision {
        DataFlowPrecision::Exact => "exact",
        DataFlowPrecision::SetupAware => "setup_aware",
        DataFlowPrecision::Syntax => "syntax",
        DataFlowPrecision::Conservative => "conservative",
        DataFlowPrecision::Heuristic => "heuristic",
        DataFlowPrecision::Unknown => "unknown",
    }
}

fn data_flow_edge_kind_label(kind: DataFlowEdgeKind) -> &'static str {
    match kind {
        DataFlowEdgeKind::LocalBinding => "local_binding",
        DataFlowEdgeKind::LocalAssignment => "local_assignment",
        DataFlowEdgeKind::LocalUse => "local_use",
        DataFlowEdgeKind::LocalRead => "local_read",
        DataFlowEdgeKind::LocalWrite => "local_write",
        DataFlowEdgeKind::ReturnValue => "return_value",
        DataFlowEdgeKind::FieldProjection => "field_projection",
        DataFlowEdgeKind::IndexProjection => "index_projection",
        DataFlowEdgeKind::Dereference => "dereference",
        DataFlowEdgeKind::AddressOf => "address_of",
        DataFlowEdgeKind::CallArgumentToParameter => "call_argument_to_parameter",
        DataFlowEdgeKind::CallArgumentToReturn => "call_argument_to_return",
        DataFlowEdgeKind::CallReturnToUse => "call_return_to_use",
        DataFlowEdgeKind::ReceiverToMethod => "receiver_to_method",
        DataFlowEdgeKind::SummaryTito => "summary_tito",
        DataFlowEdgeKind::SummaryProjected => "summary_projected",
        DataFlowEdgeKind::UnknownFlow => "unknown_flow",
        DataFlowEdgeKind::HavocFlow => "havoc_flow",
        DataFlowEdgeKind::BudgetTruncated => "budget_truncated",
        DataFlowEdgeKind::SourceIntroduction => "source_introduction",
        DataFlowEdgeKind::Sanitizer => "sanitizer",
        DataFlowEdgeKind::Barrier => "barrier",
        DataFlowEdgeKind::Model => "model",
    }
}

fn data_flow_edge_reason(edge: &DataFlowEdgeFact) -> String {
    if edge.evidence.is_empty() {
        data_flow_edge_kind_label(edge.kind).to_string()
    } else {
        edge.evidence.join(",")
    }
}

fn data_flow_budget_reason_label(reason: DataFlowBudgetReason) -> &'static str {
    match reason {
        DataFlowBudgetReason::NodeLimit => "node_limit",
        DataFlowBudgetReason::EdgeLimit => "edge_limit",
        DataFlowBudgetReason::PathDepth => "path_depth",
        DataFlowBudgetReason::PathCount => "path_count",
    }
}

fn evidence_unknown_reason_label(reason: EvidenceUnknownReason) -> &'static str {
    match reason {
        EvidenceUnknownReason::DynamicCall => "dynamic_call",
        EvidenceUnknownReason::UnsupportedEdge => "unsupported_edge",
        EvidenceUnknownReason::SetupMissing => "setup_missing",
        EvidenceUnknownReason::BudgetExceeded => "budget_exceeded",
        EvidenceUnknownReason::OpaqueSummary => "opaque_summary",
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
    use crate::analysis::data_flow::facts::{
        DataFlowAlgorithm, DataFlowBudgetFact, DataFlowConfidence, DataFlowEdgeFact,
        DataFlowEdgeKind, DataFlowNodeFact, DataFlowNodeKind, DataFlowPrecision,
        DataFlowProvenance, DataFlowStatus, DataFlowValidation,
    };
    use crate::analysis::data_flow::store::DataFlowOutput;
    use crate::analysis::evidence::facts::{EvidenceUnknownFact, EvidenceUnknownReason};
    use crate::analysis::evidence::store::EvidenceOutput;
    use crate::analysis::ids::{
        CallSiteId, DataFlowBudgetId, DataFlowEdgeId, DataFlowNodeId, DerivedEdgeId, MirBodyId,
        MirOpId, RefinedCallEdgeId, SemanticNodeId,
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
    fn public_policy_call_unknowns_are_cap_filtered() {
        let mut db = AnalysisDb::new();
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
                stable_key: db
                    .stable_key_interner()
                    .intern("callsite:budget".to_string()),
            }],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("call facts");
        db.replace_refined_call_facts(RefinedCallOutput {
            edges: vec![refined_budget_edge()],
        })
        .expect("refined calls");

        let rows = public_capability_unknowns(&db, "control_flow");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].capability.as_deref(), Some("control_flow"));
        assert_eq!(rows[0].category, UnknownCategory::BudgetExceeded);
        assert_eq!(
            rows[0].docs_path.as_deref(),
            Some("docs/facts/control-flow.md")
        );
        assert_eq!(
            rows[0].suggested_artifact.as_deref(),
            Some("model_or_setup")
        );
    }

    #[test]
    fn aggregate_unknowns_include_policy_capabilities() {
        let mut db = AnalysisDb::new();
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
                stable_key: db
                    .stable_key_interner()
                    .intern("callsite:budget".to_string()),
            }],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("call facts");
        db.replace_refined_call_facts(RefinedCallOutput {
            edges: vec![refined_budget_edge()],
        })
        .expect("refined calls");

        let capabilities = all_unknowns(&db)
            .into_iter()
            .filter_map(|row| row.capability)
            .collect::<BTreeSet<_>>();

        assert!(capabilities.contains("events"));
        assert!(capabilities.contains("calls"));
        assert!(capabilities.contains("control_flow"));
    }

    #[test]
    fn public_dataflow_unknowns_include_edges_budgets_and_evidence_unknowns() {
        let mut db = AnalysisDb::new();
        let file = db.add_file("a.ts".into(), "a.ts".to_string(), "source();".to_string());
        db.replace_data_flow_facts(DataFlowOutput {
            nodes: vec![data_flow_node(0, file, 1), data_flow_node(1, file, 2)],
            edges: vec![DataFlowEdgeFact {
                id: DataFlowEdgeId(0),
                from: DataFlowNodeId(0),
                to: DataFlowNodeId(1),
                kind: DataFlowEdgeKind::UnknownFlow,
                algorithm: DataFlowAlgorithm::QuerySearch,
                status: DataFlowStatus::Unknown,
                precision: DataFlowPrecision::Unknown,
                validation: DataFlowValidation::Native,
                confidence: DataFlowConfidence::Low,
                provenance: DataFlowProvenance::Query,
                call_site: None,
                call_target: None,
                refined_call: None,
                model: None,
                budget: None,
                evidence: vec!["dynamic_property".to_string()],
                input_stable_keys: Vec::new(),
                stable_key: crate::core::stable_key_for_test("df:unknown"),
            }],
            models: Vec::new(),
            budgets: vec![DataFlowBudgetFact {
                id: DataFlowBudgetId(0),
                reason: DataFlowBudgetReason::PathDepth,
                limit: 4,
                observed: 5,
                status: DataFlowStatus::BudgetExceeded,
                stable_key: crate::core::stable_key_for_test("df:budget"),
            }],
        })
        .expect("data-flow facts");
        db.replace_evidence_facts(EvidenceOutput {
            unknowns: vec![EvidenceUnknownFact {
                bundle: None,
                path: None,
                slice: None,
                edge: None,
                reason: EvidenceUnknownReason::OpaqueSummary,
                message: "summary expansion omitted".to_string(),
                source_fact_stable_keys: Vec::new(),
                stable_key: crate::core::stable_key_for_test("ev:unknown"),
            }],
            ..EvidenceOutput::empty()
        })
        .expect("evidence facts");

        let rows = public_capability_unknowns(&db, "dataflow");
        let families = rows
            .iter()
            .filter_map(|row| row.family.as_deref())
            .collect::<BTreeSet<_>>();

        assert_eq!(rows.len(), 3);
        assert!(families.contains("DataFlowEdge"));
        assert!(families.contains("DataFlowBudget"));
        assert!(families.contains("EvidenceUnknown"));
        assert!(rows.iter().all(|row| {
            row.capability.as_deref() == Some("dataflow") && row.docs_path.as_deref().is_some()
        }));
    }

    #[test]
    fn public_dataflow_unknowns_include_refined_call_unknowns() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            "a.ts".into(),
            "a.ts".to_string(),
            "sink(value);".to_string(),
        );
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
                    reason: UnresolvedCallReason::UnknownCallee,
                },
                receiver: None,
                arguments: Vec::new(),
                result: None,
                status: CallTargetStatus::Unresolved,
                precision: CallPrecision::Unknown,
                stable_key: db
                    .stable_key_interner()
                    .intern("callsite:unknown".to_string()),
            }],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("call facts");
        db.replace_refined_call_facts(RefinedCallOutput {
            edges: vec![refined_budget_edge()],
        })
        .expect("refined calls");

        let rows = public_capability_unknowns(&db, "dataflow");

        assert!(rows.iter().any(|row| {
            row.capability.as_deref() == Some("dataflow")
                && row.family.as_deref() == Some("RefinedCallEdge")
                && row.docs_path.as_deref() == Some("docs/facts/data-flow.md")
        }));
    }

    #[test]
    fn graph_engine_unknowns_include_go_solver_model_and_refined_rows() {
        let mut db = AnalysisDb::new();
        let interner = db.stable_key_interner();
        db.replace_go_semantic_facts(GoSemanticFactsOutput {
            package_errors: vec![
                go_error(&interner, "go:load", "package load failed"),
                go_error(
                    &interner,
                    "go:version",
                    "polint-go-frontend source mode requires Go 1.25",
                ),
                go_error(&interner, "go:timeout", "GoSidecarTimeout: request timeout"),
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
                stable_key: db
                    .stable_key_interner()
                    .intern("callsite:budget".to_string()),
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
        let interner = db.stable_key_interner();
        db.replace_go_semantic_facts(GoSemanticFactsOutput {
            package_errors: vec![go_error(&interner, "go:load", "load failed")],
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

    fn go_error(
        interner: &crate::core::StableKeyInterner,
        stable_key: &str,
        message: &str,
    ) -> GoSemanticPackageErrorFact {
        GoSemanticPackageErrorFact {
            id: GoSemanticPackageErrorId(0),
            stable_key: interner.intern(stable_key),
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
            stable_key: crate::core::stable_key_for_test("refined:budget"),
        }
    }

    fn data_flow_node(id: u64, file: crate::core::FileId, line: u32) -> DataFlowNodeFact {
        DataFlowNodeFact {
            id: DataFlowNodeId(id),
            kind: DataFlowNodeKind::Place,
            language: Language::TypeScript,
            file: Some(file),
            function: None,
            body: None,
            operation: None,
            cfg_node: None,
            place: None,
            symbol: None,
            reference: None,
            call_site: None,
            model: None,
            span: Some(Span::point(file, line, 1)),
            stable_key: crate::core::stable_key_for_test(&format!("df:node:{id}")),
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
            stable_key: crate::core::stable_key_for_test("model:missing"),
        }
    }
}

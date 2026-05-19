use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;

use crate::analysis_kernel::{
    FactFamily, FactPrecision, FactRef, PrecisionCeiling, ProviderManifest,
};
use crate::core::{
    AnalysisDb, BranchId, FileId, FunctionId, ImportId, ModuleNodeId, PackageId, ResolvedImportId,
    Span, SymbolId,
};
use crate::diagnostics::{Diagnostic, TextRange};

pub(crate) fn validate_fact_metadata(
    db: &AnalysisDb,
    manifests: &[ProviderManifest],
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let ids = IdSets::from_db(db);
    let manifests_by_id = manifests
        .iter()
        .map(|manifest| (manifest.id, *manifest))
        .collect::<BTreeMap<_, _>>();

    validate_missing_metadata(db, &mut diagnostics);
    validate_stable_key_conflicts(db, &mut diagnostics);
    validate_references(db, &ids, &mut diagnostics);
    validate_spans(db, &ids.files, &mut diagnostics);
    validate_metadata_providers(db, &manifests_by_id, &mut diagnostics);
    validate_precision_ceilings(db, &manifests_by_id, &mut diagnostics);

    diagnostics.sort_by(diagnostic_order);
    diagnostics
}

#[derive(Debug, Default)]
struct IdSets {
    files: BTreeSet<FileId>,
    packages: BTreeSet<PackageId>,
    functions: BTreeSet<FunctionId>,
    branches: BTreeSet<BranchId>,
    imports: BTreeSet<ImportId>,
    resolved_imports: BTreeSet<ResolvedImportId>,
    module_nodes: BTreeSet<ModuleNodeId>,
    symbols: BTreeSet<SymbolId>,
}

impl IdSets {
    fn from_db(db: &AnalysisDb) -> Self {
        Self {
            files: db.files().iter().map(|fact| fact.id).collect(),
            packages: db.packages().iter().map(|fact| fact.id).collect(),
            functions: db.functions().iter().map(|fact| fact.id).collect(),
            branches: db.branches().iter().map(|fact| fact.id).collect(),
            imports: db.imports().iter().map(|fact| fact.id).collect(),
            resolved_imports: db.resolved_imports().iter().map(|fact| fact.id).collect(),
            module_nodes: db.module_nodes().iter().map(|fact| fact.id).collect(),
            symbols: db.symbols().iter().map(|fact| fact.id).collect(),
        }
    }
}

fn validate_missing_metadata(db: &AnalysisDb, diagnostics: &mut Vec<Diagnostic>) {
    for missing in db.missing_fact_metadata() {
        diagnostics.push(
            internal_diagnostic(format!(
                "Fact metadata missing for {}#{}.",
                missing.family.label(),
                missing.run_id
            ))
            .with_evidence("family", missing.family.label())
            .with_evidence(
                "fact_ref",
                fact_ref_value(FactRef::new(missing.family, missing.run_id)),
            ),
        );
    }
}

fn validate_stable_key_conflicts(db: &AnalysisDb, diagnostics: &mut Vec<Diagnostic>) {
    for conflict in db.fact_meta().stable_key_conflicts() {
        diagnostics.push(
            internal_diagnostic(format!(
                "Fact metadata stable key conflict detected for {} stable key.",
                conflict.family.label()
            ))
            .with_evidence("family", conflict.family.label())
            .with_evidence("stable_key", conflict.stable_key.clone())
            .with_evidence("existing_ref", fact_ref_value(conflict.existing))
            .with_evidence("incoming_ref", fact_ref_value(conflict.incoming)),
        );
    }
}

fn validate_metadata_providers(
    db: &AnalysisDb,
    manifests_by_id: &BTreeMap<&'static str, ProviderManifest>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (reference, metadata) in db.fact_meta().rows() {
        if !manifests_by_id.contains_key(metadata.producer_id) {
            diagnostics.push(provider_manifest_diagnostic(
                reference,
                "producer_id",
                metadata.producer_id,
            ));
        }
        if !manifests_by_id.contains_key(metadata.layer_id) {
            diagnostics.push(provider_manifest_diagnostic(
                reference,
                "layer_id",
                metadata.layer_id,
            ));
        }
    }
}

fn validate_references(db: &AnalysisDb, ids: &IdSets, diagnostics: &mut Vec<Diagnostic>) {
    for fact in db.functions() {
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::Function,
            fact.id.0,
            "FunctionFact.file",
            fact.file,
        );
    }
    for fact in db.packages() {
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::Package,
            fact.id.0,
            "PackageFact.file",
            fact.file,
        );
    }
    for fact in db.imports() {
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::Import,
            fact.id.0,
            "ImportFact.file",
            fact.file,
        );
    }
    for fact in db.branches() {
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::BranchObligation,
            fact.id.0,
            "BranchObligation.file",
            fact.file,
        );
        check_optional_ref(
            diagnostics,
            &ids.functions,
            FactFamily::BranchObligation,
            fact.id.0,
            "BranchObligation.function",
            fact.function,
        );
    }
    for (run_id, fact) in db.tests().iter().enumerate() {
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::Test,
            run_id as u64,
            "TestFact.file",
            fact.file,
        );
        check_optional_ref(
            diagnostics,
            &ids.functions,
            FactFamily::Test,
            run_id as u64,
            "TestFact.function",
            fact.function,
        );
    }
    for (run_id, fact) in db.coverage().iter().enumerate() {
        check_ref(
            diagnostics,
            &ids.branches,
            FactFamily::Coverage,
            run_id as u64,
            "CoverageFact.branch",
            fact.branch,
        );
    }
    for (run_id, fact) in db.ts_components().iter().enumerate() {
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::TsComponent,
            run_id as u64,
            "TsComponentFact.file",
            fact.file,
        );
        check_optional_ref(
            diagnostics,
            &ids.functions,
            FactFamily::TsComponent,
            run_id as u64,
            "TsComponentFact.function",
            fact.function,
        );
    }
    for (run_id, fact) in db.ts_classes().iter().enumerate() {
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::TsClass,
            run_id as u64,
            "TsClassFact.file",
            fact.file,
        );
    }
    for (run_id, fact) in db.string_literals().iter().enumerate() {
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::StringLiteral,
            run_id as u64,
            "StringLiteralFact.file",
            fact.file,
        );
    }
    for (run_id, fact) in db.jsx_attributes().iter().enumerate() {
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::JsxAttribute,
            run_id as u64,
            "JsxAttributeFact.file",
            fact.file,
        );
    }
    for (run_id, fact) in db.file_metrics().iter().enumerate() {
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::FileMetric,
            run_id as u64,
            "FileMetricFact.file",
            fact.file,
        );
    }
    for (run_id, fact) in db.function_metrics().iter().enumerate() {
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::FunctionMetric,
            run_id as u64,
            "FunctionMetricFact.file",
            fact.file,
        );
        check_ref(
            diagnostics,
            &ids.functions,
            FactFamily::FunctionMetric,
            run_id as u64,
            "FunctionMetricFact.function",
            fact.function,
        );
    }
    for (run_id, fact) in db.complexity_metrics().iter().enumerate() {
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::ComplexityMetric,
            run_id as u64,
            "ComplexityMetricFact.file",
            fact.file,
        );
        check_ref(
            diagnostics,
            &ids.functions,
            FactFamily::ComplexityMetric,
            run_id as u64,
            "ComplexityMetricFact.function",
            fact.function,
        );
    }
    for fact in db.resolved_imports() {
        check_ref(
            diagnostics,
            &ids.imports,
            FactFamily::ResolvedImport,
            fact.id.0,
            "ResolvedImportFact.import",
            fact.import,
        );
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::ResolvedImport,
            fact.id.0,
            "ResolvedImportFact.from_file",
            fact.from_file,
        );
        check_optional_ref(
            diagnostics,
            &ids.module_nodes,
            FactFamily::ResolvedImport,
            fact.id.0,
            "ResolvedImportFact.target_node",
            fact.target_node,
        );
    }
    for node in db.module_nodes() {
        check_optional_ref(
            diagnostics,
            &ids.files,
            FactFamily::ModuleNode,
            node.id.0,
            "ModuleNode.file",
            node.file,
        );
        check_optional_ref(
            diagnostics,
            &ids.packages,
            FactFamily::ModuleNode,
            node.id.0,
            "ModuleNode.package",
            node.package,
        );
    }
    for edge in db.module_edges() {
        check_ref(
            diagnostics,
            &ids.module_nodes,
            FactFamily::ModuleEdge,
            edge.id.0,
            "ModuleEdge.from",
            edge.from,
        );
        check_ref(
            diagnostics,
            &ids.module_nodes,
            FactFamily::ModuleEdge,
            edge.id.0,
            "ModuleEdge.to",
            edge.to,
        );
        check_optional_ref(
            diagnostics,
            &ids.imports,
            FactFamily::ModuleEdge,
            edge.id.0,
            "ModuleEdge.import",
            edge.import,
        );
        check_optional_ref(
            diagnostics,
            &ids.resolved_imports,
            FactFamily::ModuleEdge,
            edge.id.0,
            "ModuleEdge.resolved_import",
            edge.resolved_import,
        );
    }
    for symbol in db.symbols() {
        check_optional_ref(
            diagnostics,
            &ids.files,
            FactFamily::Symbol,
            symbol.id.0,
            "SymbolFact.file",
            symbol.file,
        );
        check_optional_ref(
            diagnostics,
            &ids.packages,
            FactFamily::Symbol,
            symbol.id.0,
            "SymbolFact.package",
            symbol.package,
        );
        check_optional_ref(
            diagnostics,
            &ids.module_nodes,
            FactFamily::Symbol,
            symbol.id.0,
            "SymbolFact.module",
            symbol.module,
        );
        check_optional_ref(
            diagnostics,
            &ids.symbols,
            FactFamily::Symbol,
            symbol.id.0,
            "SymbolFact.owner",
            symbol.owner,
        );
    }
    for definition in db.definitions() {
        check_ref(
            diagnostics,
            &ids.symbols,
            FactFamily::Definition,
            definition.id.0,
            "DefinitionFact.symbol",
            definition.symbol,
        );
        check_optional_ref(
            diagnostics,
            &ids.files,
            FactFamily::Definition,
            definition.id.0,
            "DefinitionFact.file",
            definition.file,
        );
        check_optional_ref(
            diagnostics,
            &ids.packages,
            FactFamily::Definition,
            definition.id.0,
            "DefinitionFact.package",
            definition.package,
        );
        check_optional_ref(
            diagnostics,
            &ids.module_nodes,
            FactFamily::Definition,
            definition.id.0,
            "DefinitionFact.module",
            definition.module,
        );
        check_optional_ref(
            diagnostics,
            &ids.symbols,
            FactFamily::Definition,
            definition.id.0,
            "DefinitionFact.owner",
            definition.owner,
        );
    }
    for reference in db.references() {
        check_optional_ref(
            diagnostics,
            &ids.files,
            FactFamily::Reference,
            reference.id.0,
            "ReferenceFact.file",
            reference.file,
        );
        check_optional_ref(
            diagnostics,
            &ids.packages,
            FactFamily::Reference,
            reference.id.0,
            "ReferenceFact.package",
            reference.package,
        );
        check_optional_ref(
            diagnostics,
            &ids.module_nodes,
            FactFamily::Reference,
            reference.id.0,
            "ReferenceFact.module",
            reference.module,
        );
        check_optional_ref(
            diagnostics,
            &ids.symbols,
            FactFamily::Reference,
            reference.id.0,
            "ReferenceFact.owner",
            reference.owner,
        );
        check_optional_ref(
            diagnostics,
            &ids.symbols,
            FactFamily::Reference,
            reference.id.0,
            "ReferenceFact.target",
            reference.target,
        );
        for candidate in &reference.candidates {
            check_ref(
                diagnostics,
                &ids.symbols,
                FactFamily::Reference,
                reference.id.0,
                "ReferenceFact.candidates",
                *candidate,
            );
        }
    }
}

fn validate_spans(db: &AnalysisDb, file_ids: &BTreeSet<FileId>, diagnostics: &mut Vec<Diagnostic>) {
    for fact in db.functions() {
        check_span(
            db,
            file_ids,
            diagnostics,
            SpanCheck {
                family: FactFamily::Function,
                run_id: fact.id.0,
                field: "FunctionFact.span",
                owner_file: Some(fact.file),
                span: &fact.span,
            },
        );
    }
    for fact in db.packages() {
        check_span(
            db,
            file_ids,
            diagnostics,
            SpanCheck {
                family: FactFamily::Package,
                run_id: fact.id.0,
                field: "PackageFact.span",
                owner_file: Some(fact.file),
                span: &fact.span,
            },
        );
    }
    for fact in db.imports() {
        check_span(
            db,
            file_ids,
            diagnostics,
            SpanCheck {
                family: FactFamily::Import,
                run_id: fact.id.0,
                field: "ImportFact.span",
                owner_file: Some(fact.file),
                span: &fact.span,
            },
        );
    }
    for fact in db.branches() {
        check_span(
            db,
            file_ids,
            diagnostics,
            SpanCheck {
                family: FactFamily::BranchObligation,
                run_id: fact.id.0,
                field: "BranchObligation.decision_span",
                owner_file: Some(fact.file),
                span: &fact.decision_span,
            },
        );
    }
    for (run_id, fact) in db.tests().iter().enumerate() {
        check_span(
            db,
            file_ids,
            diagnostics,
            SpanCheck {
                family: FactFamily::Test,
                run_id: run_id as u64,
                field: "TestFact.span",
                owner_file: Some(fact.file),
                span: &fact.span,
            },
        );
    }
    for (run_id, fact) in db.ts_components().iter().enumerate() {
        check_span(
            db,
            file_ids,
            diagnostics,
            SpanCheck {
                family: FactFamily::TsComponent,
                run_id: run_id as u64,
                field: "TsComponentFact.span",
                owner_file: Some(fact.file),
                span: &fact.span,
            },
        );
    }
    for (run_id, fact) in db.ts_classes().iter().enumerate() {
        check_span(
            db,
            file_ids,
            diagnostics,
            SpanCheck {
                family: FactFamily::TsClass,
                run_id: run_id as u64,
                field: "TsClassFact.span",
                owner_file: Some(fact.file),
                span: &fact.span,
            },
        );
    }
    for (run_id, fact) in db.string_literals().iter().enumerate() {
        check_span(
            db,
            file_ids,
            diagnostics,
            SpanCheck {
                family: FactFamily::StringLiteral,
                run_id: run_id as u64,
                field: "StringLiteralFact.span",
                owner_file: Some(fact.file),
                span: &fact.span,
            },
        );
    }
    for (run_id, fact) in db.jsx_attributes().iter().enumerate() {
        check_span(
            db,
            file_ids,
            diagnostics,
            SpanCheck {
                family: FactFamily::JsxAttribute,
                run_id: run_id as u64,
                field: "JsxAttributeFact.span",
                owner_file: Some(fact.file),
                span: &fact.span,
            },
        );
    }
    for (run_id, fact) in db.function_metrics().iter().enumerate() {
        check_span(
            db,
            file_ids,
            diagnostics,
            SpanCheck {
                family: FactFamily::FunctionMetric,
                run_id: run_id as u64,
                field: "FunctionMetricFact.span",
                owner_file: Some(fact.file),
                span: &fact.span,
            },
        );
    }
    for (run_id, fact) in db.complexity_metrics().iter().enumerate() {
        check_span(
            db,
            file_ids,
            diagnostics,
            SpanCheck {
                family: FactFamily::ComplexityMetric,
                run_id: run_id as u64,
                field: "ComplexityMetricFact.span",
                owner_file: Some(fact.file),
                span: &fact.span,
            },
        );
    }
    for symbol in db.symbols() {
        if let Some(span) = &symbol.primary_span {
            check_span(
                db,
                file_ids,
                diagnostics,
                SpanCheck {
                    family: FactFamily::Symbol,
                    run_id: symbol.id.0,
                    field: "SymbolFact.primary_span",
                    owner_file: symbol.file,
                    span,
                },
            );
        }
    }
    for definition in db.definitions() {
        if let Some(span) = &definition.primary_span {
            check_span(
                db,
                file_ids,
                diagnostics,
                SpanCheck {
                    family: FactFamily::Definition,
                    run_id: definition.id.0,
                    field: "DefinitionFact.primary_span",
                    owner_file: definition.file,
                    span,
                },
            );
        }
    }
    for reference in db.references() {
        if let Some(span) = &reference.primary_span {
            check_span(
                db,
                file_ids,
                diagnostics,
                SpanCheck {
                    family: FactFamily::Reference,
                    run_id: reference.id.0,
                    field: "ReferenceFact.primary_span",
                    owner_file: reference.file,
                    span,
                },
            );
        }
    }
}

fn validate_precision_ceilings(
    db: &AnalysisDb,
    manifests_by_id: &BTreeMap<&'static str, ProviderManifest>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (reference, metadata) in db.fact_meta().rows() {
        let Some(manifest) = manifests_by_id.get(metadata.producer_id) else {
            continue;
        };
        if precision_within_ceiling(metadata.precision, manifest.precision_ceiling) {
            continue;
        }
        diagnostics.push(
            internal_diagnostic(format!(
                "Fact metadata precision ceiling violated for {}#{}.",
                reference.family.label(),
                reference.run_id
            ))
            .with_evidence("producer_id", metadata.producer_id)
            .with_evidence("family", reference.family.label())
            .with_evidence("precision", precision_label(metadata.precision))
            .with_evidence("ceiling", ceiling_label(manifest.precision_ceiling)),
        );
    }
}

fn check_ref<T>(
    diagnostics: &mut Vec<Diagnostic>,
    valid_ids: &BTreeSet<T>,
    family: FactFamily,
    run_id: u64,
    field: &'static str,
    value: T,
) where
    T: Copy + Debug + Ord,
{
    if valid_ids.contains(&value) {
        return;
    }
    diagnostics.push(reference_diagnostic(family, run_id, field, value));
}

fn check_optional_ref<T>(
    diagnostics: &mut Vec<Diagnostic>,
    valid_ids: &BTreeSet<T>,
    family: FactFamily,
    run_id: u64,
    field: &'static str,
    value: Option<T>,
) where
    T: Copy + Debug + Ord,
{
    let Some(value) = value else {
        return;
    };
    check_ref(diagnostics, valid_ids, family, run_id, field, value);
}

fn reference_diagnostic<T: Debug>(
    family: FactFamily,
    run_id: u64,
    field: &'static str,
    value: T,
) -> Diagnostic {
    internal_diagnostic(format!(
        "Fact metadata reference validation failed for {}#{}.",
        family.label(),
        run_id
    ))
    .with_evidence("family", family.label())
    .with_evidence("fact_ref", fact_ref_value(FactRef::new(family, run_id)))
    .with_evidence("field", field)
    .with_evidence("value", format!("{value:?}"))
}

fn provider_manifest_diagnostic(
    reference: FactRef,
    field: &'static str,
    value: &'static str,
) -> Diagnostic {
    internal_diagnostic(format!(
        "Fact metadata provider manifest missing for {}#{}.",
        reference.family.label(),
        reference.run_id
    ))
    .with_evidence("family", reference.family.label())
    .with_evidence("fact_ref", fact_ref_value(reference))
    .with_evidence("field", field)
    .with_evidence("value", value)
}

struct SpanCheck<'a> {
    family: FactFamily,
    run_id: u64,
    field: &'static str,
    owner_file: Option<FileId>,
    span: &'a Span,
}

fn check_span(
    db: &AnalysisDb,
    file_ids: &BTreeSet<FileId>,
    diagnostics: &mut Vec<Diagnostic>,
    check: SpanCheck<'_>,
) {
    let Some(reason) = span_failure_reason(db, file_ids, check.owner_file, check.span) else {
        return;
    };
    diagnostics.push(
        internal_diagnostic(format!(
            "Fact metadata span validation failed for {}#{}.",
            check.family.label(),
            check.run_id
        ))
        .with_evidence("family", check.family.label())
        .with_evidence(
            "fact_ref",
            fact_ref_value(FactRef::new(check.family, check.run_id)),
        )
        .with_evidence("field", check.field)
        .with_evidence("reason", reason)
        .with_evidence("span_file", format!("{:?}", check.span.file))
        .with_evidence("owner_file", owner_file_value(check.owner_file))
        .with_evidence("start_byte", check.span.start_byte.to_string())
        .with_evidence("end_byte", check.span.end_byte.to_string()),
    );
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

fn precision_within_ceiling(precision: FactPrecision, ceiling: PrecisionCeiling) -> bool {
    match ceiling {
        PrecisionCeiling::Exact => true,
        PrecisionCeiling::Syntax => matches!(
            precision,
            FactPrecision::Syntax
                | FactPrecision::Heuristic
                | FactPrecision::Unresolved
                | FactPrecision::Ambiguous
                | FactPrecision::SetupMissing
                | FactPrecision::Unsupported
        ),
        PrecisionCeiling::SetupAware => true,
    }
}

fn internal_diagnostic(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        TextRange::point(1, 1),
        message,
    )
}

fn fact_ref_value(reference: FactRef) -> String {
    format!("{}#{}", reference.family.label(), reference.run_id)
}

fn owner_file_value(owner_file: Option<FileId>) -> String {
    owner_file.map_or_else(|| "none".to_string(), |file| format!("{file:?}"))
}

fn precision_label(precision: FactPrecision) -> &'static str {
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

fn ceiling_label(ceiling: PrecisionCeiling) -> &'static str {
    match ceiling {
        PrecisionCeiling::Exact => "exact",
        PrecisionCeiling::Syntax => "syntax",
        PrecisionCeiling::SetupAware => "setup_aware",
    }
}

fn diagnostic_order(left: &Diagnostic, right: &Diagnostic) -> std::cmp::Ordering {
    (
        left.rule_id.as_str(),
        left.file.as_str(),
        left.range.start_line,
        left.range.start_col,
        left.message.as_str(),
        evidence_order_key(left),
        left.stable_fingerprint.as_str(),
    )
        .cmp(&(
            right.rule_id.as_str(),
            right.file.as_str(),
            right.range.start_line,
            right.range.start_col,
            right.message.as_str(),
            evidence_order_key(right),
            right.stable_fingerprint.as_str(),
        ))
}

fn evidence_order_key(diagnostic: &Diagnostic) -> String {
    diagnostic
        .evidence
        .iter()
        .map(|evidence| format!("{}={}", evidence.label, evidence.value))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::validate_fact_metadata;
    use crate::analysis_kernel::{
        AnalysisKernel, FactConfidence, FactFamily, FactMeta, FactPrecision, FactRef,
        ValidationStatus,
    };
    use crate::core::{
        AnalysisDb, BranchId, BranchObligation, ComplexityMetricFact, FileId, FileMetricFact,
        FunctionFact, FunctionId, FunctionMetricFact, Language, ModuleNode, ModuleNodeId,
        ModuleNodeKind, PackageId, Span, SymbolFact, SymbolId, SymbolKind, SymbolNamespace,
        SymbolPrecision, TestFact, TsComponentFact,
    };
    use crate::symbol_graph::semantic::{
        GeneratedSymbolFact, GeneratedSymbolId, GeneratedSymbolKind, SemanticStatus,
    };
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    #[test]
    fn metadata_validation_conflict_records_render_internal_diagnostics_with_evidence() {
        let mut db = AnalysisDb::new();
        let existing = FactRef::new(FactFamily::Import, 1);
        let incoming = FactRef::new(FactFamily::Import, 2);

        db.fact_meta_mut_for_test().insert(
            existing,
            test_meta(FactFamily::Import, "import:key", "payload:a"),
        );
        db.fact_meta_mut_for_test().insert(
            incoming,
            test_meta(FactFamily::Import, "import:key", "payload:b"),
        );

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule_id, "polint/internal");
        assert!(
            diagnostics[0]
                .message
                .starts_with("Fact metadata stable key conflict detected")
        );
        assert_eq!(
            evidence_labels(&diagnostics[0]),
            BTreeSet::from(["existing_ref", "family", "incoming_ref", "stable_key",])
        );
    }

    #[test]
    fn metadata_validation_span_failures_are_reported_deterministically() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "abc".to_string(),
        );
        let other_file = db.add_file(
            PathBuf::from("src/other.ts"),
            "src/other.ts".to_string(),
            "abcdef".to_string(),
        );
        db.push_function(FunctionFact {
            id: FunctionId(99),
            file,
            name: "too_long".to_string(),
            span: span(file, 0, 4),
            language: Language::TypeScript,
            is_test: false,
            is_exported: false,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        db.push_function(FunctionFact {
            id: FunctionId(99),
            file,
            name: "reversed".to_string(),
            span: span(file, 2, 1),
            language: Language::TypeScript,
            is_test: false,
            is_exported: false,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        db.push_function(FunctionFact {
            id: FunctionId(99),
            file,
            name: "wrong_file".to_string(),
            span: span(other_file, 0, 1),
            language: Language::TypeScript,
            is_test: false,
            is_exported: false,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());
        let messages = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>();

        assert_eq!(messages.len(), 3);
        assert!(
            messages
                .iter()
                .all(|message| message.starts_with("Fact metadata span validation failed"))
        );
    }

    #[test]
    fn metadata_validation_reference_failures_cover_current_focused_fields() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.tsx"),
            "src/app.tsx".to_string(),
            "export function Button() { return null; }\n".to_string(),
        );
        db.push_branch(BranchObligation {
            id: BranchId(99),
            function: Some(FunctionId(404)),
            file,
            decision_span: span(file, 0, 1),
            condition_text: "enabled".to_string(),
            edge_label: "true".to_string(),
            is_error_path: false,
            stable_fingerprint: "branch:key".to_string(),
        });
        db.push_test(TestFact {
            file,
            function: Some(FunctionId(405)),
            name: "TestButton".to_string(),
            span: span(file, 0, 1),
            evidence_terms: Vec::new(),
            assertion_count: 0,
            subtest_count: 0,
            subtest_names: Vec::new(),
            table_rows: 0,
        });
        db.push_ts_component(TsComponentFact {
            file,
            function: Some(FunctionId(406)),
            name: "Button".to_string(),
            span: span(file, 0, 1),
        });
        db.replace_module_graph_facts(
            Vec::new(),
            vec![ModuleNode {
                id: ModuleNodeId(99),
                kind: ModuleNodeKind::File,
                label: "missing".to_string(),
                file: Some(FileId(404)),
                package: Some(PackageId(405)),
                language: Some(Language::Tsx),
            }],
            Vec::new(),
        );
        db.replace_metric_facts(
            vec![FileMetricFact {
                file: FileId(406),
                language: Language::Tsx,
                line_count: 1,
                non_empty_line_count: 1,
                byte_count: 1,
                function_count: 0,
            }],
            vec![FunctionMetricFact {
                function: FunctionId(407),
                file: FileId(407),
                name: "Button".to_string(),
                span: span(file, 0, 1),
                language: Language::Tsx,
                line_count: 1,
                byte_count: 1,
            }],
            vec![ComplexityMetricFact {
                function: FunctionId(408),
                file: FileId(408),
                name: "Button".to_string(),
                span: span(file, 0, 1),
                language: Language::Tsx,
                cyclomatic_complexity: 1,
            }],
        );

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());
        let evidence_values = diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic
                    .message
                    .starts_with("Fact metadata reference validation failed")
            })
            .flat_map(|diagnostic| {
                diagnostic
                    .evidence
                    .iter()
                    .filter(|evidence| evidence.label == "field")
                    .map(|evidence| evidence.value.as_str())
            })
            .collect::<BTreeSet<_>>();

        assert_eq!(
            evidence_values,
            BTreeSet::from([
                "BranchObligation.function",
                "ComplexityMetricFact.file",
                "ComplexityMetricFact.function",
                "FileMetricFact.file",
                "FunctionMetricFact.file",
                "FunctionMetricFact.function",
                "ModuleNode.file",
                "ModuleNode.package",
                "TestFact.function",
                "TsComponentFact.function",
            ])
        );
    }

    #[test]
    fn metadata_validation_precision_ceiling_violations_name_provider_family_and_precision() {
        let mut db = AnalysisDb::new();
        db.fact_meta_mut_for_test().insert(
            FactRef::new(FactFamily::FileMetric, 0),
            FactMeta {
                stable_key: "metric:key".to_string(),
                producer_id: "polint.metrics",
                layer_id: "polint.metrics",
                precision: FactPrecision::Exact,
                confidence: FactConfidence::High,
                validation: ValidationStatus::NativeTrusted,
                payload_digest: "payload:a".to_string(),
            },
        );

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());

        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .starts_with("Fact metadata precision ceiling violated")
        );
        assert_eq!(
            evidence_labels(&diagnostics[0]),
            BTreeSet::from(["ceiling", "family", "precision", "producer_id"])
        );
    }

    #[test]
    fn metadata_validation_reports_unknown_producer_and_layer_ids() {
        let mut db = AnalysisDb::new();
        db.fact_meta_mut_for_test().insert(
            FactRef::new(FactFamily::FileMetric, 0),
            FactMeta {
                stable_key: "metric:key".to_string(),
                producer_id: "polint.unknown_producer",
                layer_id: "polint.unknown_layer",
                precision: FactPrecision::Syntax,
                confidence: FactConfidence::High,
                validation: ValidationStatus::NativeTrusted,
                payload_digest: "payload:a".to_string(),
            },
        );

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());
        let provider_fields = diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic
                    .message
                    .starts_with("Fact metadata provider manifest missing")
            })
            .flat_map(|diagnostic| {
                diagnostic
                    .evidence
                    .iter()
                    .filter(|evidence| evidence.label == "field")
                    .map(|evidence| evidence.value.as_str())
            })
            .collect::<BTreeSet<_>>();

        assert_eq!(provider_fields, BTreeSet::from(["layer_id", "producer_id"]));
    }

    mod semantic_index {
        use super::*;

        #[test]
        fn semantic_validation_reports_malformed_generated_rows_with_evidence() {
            let mut db = semantic_db();
            db.replace_semantic_index_facts(
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                vec![GeneratedSymbolFact {
                    id: GeneratedSymbolId(99),
                    language: Language::TypeScript,
                    file: Some(FileId(404)),
                    package: None,
                    module: None,
                    symbol_stable_key: "symbol:answer".to_string(),
                    source_stable_key: String::new(),
                    producer_id: String::new(),
                    generator: "test".to_string(),
                    generated_discriminator: String::new(),
                    kind: GeneratedSymbolKind::BuildGenerated,
                    span: Some(span(FileId(404), 0, 999)),
                    stable_key: "generated:bad".to_string(),
                    status: SemanticStatus::Resolved,
                }],
                Vec::new(),
            );

            let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());
            let semantic_diagnostics = diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic
                        .message
                        .starts_with("Semantic index validation failed")
                })
                .collect::<Vec<_>>();

            assert!(
                semantic_diagnostics.len() >= 4,
                "expected generated-row validation diagnostics: {diagnostics:#?}"
            );
            assert!(
                semantic_diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.rule_id == "polint/internal")
            );
            assert!(semantic_diagnostics.iter().all(|diagnostic| {
                let labels = evidence_labels(diagnostic);
                labels.contains("family")
                    && labels.contains("stable_key")
                    && labels.contains("reason")
            }));
        }

        #[test]
        fn semantic_validation_rejects_symbol_graph_exact_semantic_metadata() {
            let mut db = semantic_db();
            db.replace_semantic_index_facts(
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                vec![GeneratedSymbolFact {
                    id: GeneratedSymbolId(0),
                    language: Language::TypeScript,
                    file: Some(FileId(0)),
                    package: None,
                    module: None,
                    symbol_stable_key: "symbol:answer".to_string(),
                    source_stable_key: "symbol:answer".to_string(),
                    producer_id: "polint.symbol_graph".to_string(),
                    generator: "test".to_string(),
                    generated_discriminator: "entrypoint".to_string(),
                    kind: GeneratedSymbolKind::BuildGenerated,
                    span: Some(span(FileId(0), 0, 1)),
                    stable_key: "generated:answer".to_string(),
                    status: SemanticStatus::Generated,
                }],
                Vec::new(),
            );
            db.fact_meta_mut_for_test()
                .remove_for_test(FactRef::new(FactFamily::GeneratedSymbol, 0));
            db.fact_meta_mut_for_test().insert(
                FactRef::new(FactFamily::GeneratedSymbol, 0),
                FactMeta {
                    stable_key: "generated:answer".to_string(),
                    producer_id: "polint.symbol_graph",
                    layer_id: "polint.symbol_graph",
                    precision: FactPrecision::Exact,
                    confidence: FactConfidence::High,
                    validation: ValidationStatus::NativeTrusted,
                    payload_digest: "payload:exact-generated".to_string(),
                },
            );

            let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());

            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic
                        .message
                        .starts_with("Semantic index validation failed")
                        && diagnostic.evidence.iter().any(|evidence| {
                            evidence.label == "reason"
                                && evidence.value.contains("provider precision ceiling")
                        })
                        && diagnostic.evidence.iter().any(|evidence| {
                            evidence.label == "family" && evidence.value == "GeneratedSymbol"
                        })
                        && diagnostic.evidence.iter().any(|evidence| {
                            evidence.label == "stable_key" && evidence.value == "generated:answer"
                        })
                }),
                "expected semantic precision ceiling diagnostic: {diagnostics:#?}"
            );
        }

        fn semantic_db() -> AnalysisDb {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "export const answer = 1;\n".to_string(),
            );
            db.replace_symbol_graph_facts(
                vec![SymbolFact {
                    id: SymbolId(0),
                    language: Language::TypeScript,
                    name: "answer".to_string(),
                    qualified_name: "answer".to_string(),
                    kind: SymbolKind::Constant,
                    namespace: SymbolNamespace::Value,
                    file: Some(file),
                    package: None,
                    module: None,
                    owner: None,
                    primary_span: Some(span(file, 13, 19)),
                    is_exported: true,
                    stable_key: "symbol:answer".to_string(),
                    precision: SymbolPrecision::ExactLocal,
                }],
                Vec::new(),
                Vec::new(),
            );
            db
        }
    }

    fn test_meta(family: FactFamily, stable_key: &str, payload_digest: &str) -> FactMeta {
        FactMeta {
            stable_key: stable_key.to_string(),
            producer_id: match family {
                FactFamily::SourceFile => "polint.source",
                FactFamily::FileMetric => "polint.metrics",
                _ => "polint.go.syntax",
            },
            layer_id: "polint.go.syntax",
            precision: FactPrecision::Syntax,
            confidence: FactConfidence::High,
            validation: ValidationStatus::NativeTrusted,
            payload_digest: payload_digest.to_string(),
        }
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

    fn evidence_labels(diagnostic: &crate::diagnostics::Diagnostic) -> BTreeSet<&str> {
        diagnostic
            .evidence
            .iter()
            .map(|evidence| evidence.label.as_str())
            .collect()
    }
}

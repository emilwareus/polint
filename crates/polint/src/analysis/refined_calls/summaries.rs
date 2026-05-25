use super::facts::{
    RefinedCallConfidence, RefinedCallEdgeFact, RefinedCallTier, RefinedCallValidation,
};
use super::store::RefinedCallOutput;
use crate::analysis::calls::facts::{
    CallAlgorithm, CallPrecision, CallProvenance, CallTargetFact, CallTargetStatus,
};
use crate::analysis::ids::RefinedCallEdgeId;
use crate::analysis::summaries::facts::{
    SummaryDomainKind, SummaryFact, SummaryPrecision, SummaryStatus,
};
use crate::analysis_kernel::{FactFamily, FactRef, stable_key_from_parts};
use crate::core::AnalysisDb;

pub(crate) fn derive_summary_assisted_refinements(db: &AnalysisDb) -> RefinedCallOutput {
    let mut edges = Vec::new();
    for summary in db.summary_facts() {
        if summary.domain != SummaryDomainKind::CallEffects
            || summary.status != SummaryStatus::Present
        {
            continue;
        }
        for target in db
            .call_targets()
            .iter()
            .filter(|target| target.caller == summary.function)
        {
            edges.push(edge_from_summary(db, summary, target, edges.len()));
        }
    }
    RefinedCallOutput { edges }.normalized()
}

fn edge_from_summary(
    db: &AnalysisDb,
    summary: &SummaryFact,
    target: &CallTargetFact,
    index: usize,
) -> RefinedCallEdgeFact {
    let summary_key = metadata_key(
        db,
        FactFamily::SummaryCall,
        summary.id.0,
        &summary.stable_key,
    );
    let target_key = metadata_key(db, FactFamily::CallTarget, target.id.0, &target.stable_key);
    RefinedCallEdgeFact {
        id: RefinedCallEdgeId(index as u64),
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
            .unwrap_or(crate::core::Language::Unknown),
        edge_kind: target.edge_kind,
        algorithm: CallAlgorithm::SummaryAssisted,
        tier: RefinedCallTier::SummaryAssisted,
        status: target.status,
        reason: target.reason,
        provenance: CallProvenance::Native,
        precision: summary_precision(summary.precision),
        validation: RefinedCallValidation::ReferentiallyValidated,
        confidence: summary_confidence(summary.precision, target.status),
        evidence: vec![
            "summary_call_effect".to_string(),
            format!("summary={summary_key}"),
        ],
        input_stable_keys: vec![summary_key.clone(), target_key.clone()],
        stable_key: stable_key_from_parts(
            FactFamily::RefinedCallEdge,
            &[
                ("tier", "summary_assisted".to_string()),
                ("summary", summary_key),
                ("base_target", target_key),
            ],
        ),
    }
}

fn summary_precision(precision: SummaryPrecision) -> CallPrecision {
    match precision {
        SummaryPrecision::Local | SummaryPrecision::SetupAware => CallPrecision::SetupAware,
        SummaryPrecision::Heuristic => CallPrecision::Heuristic,
        SummaryPrecision::UnknownTop => CallPrecision::Unknown,
    }
}

fn summary_confidence(
    precision: SummaryPrecision,
    target_status: CallTargetStatus,
) -> RefinedCallConfidence {
    match target_status {
        CallTargetStatus::Resolved => match precision {
            SummaryPrecision::Local | SummaryPrecision::SetupAware => RefinedCallConfidence::High,
            SummaryPrecision::Heuristic => RefinedCallConfidence::Medium,
            SummaryPrecision::UnknownTop => RefinedCallConfidence::Low,
        },
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
        CallCallee, CallEdgeKind, CallSiteFact, CallSyntaxKind, CallTargetFact,
    };
    use crate::analysis::calls::store::CallOutput;
    use crate::analysis::ids::{CallSiteId, CallTargetId, MirBodyId, MirOpId, SummaryId};
    use crate::analysis::summaries::facts::SummaryProvenance;
    use crate::analysis::summaries::store::SummaryOutput;
    use crate::core::{FileId, FunctionFact, FunctionId, Language, Span, SymbolId};

    #[test]
    fn bindable_call_summary_creates_summary_assisted_edge() {
        let mut db = db_with_call_target();
        db.replace_summary_facts(SummaryOutput {
            summaries: vec![summary(
                SummaryStatus::Present,
                SummaryDomainKind::CallEffects,
            )],
            events: Vec::new(),
        });

        let output = derive_summary_assisted_refinements(&db);

        assert_eq!(output.edges.len(), 1);
        assert_eq!(output.edges[0].tier, RefinedCallTier::SummaryAssisted);
        assert_eq!(output.edges[0].algorithm, CallAlgorithm::SummaryAssisted);
    }

    #[test]
    fn missing_or_non_call_summary_does_not_create_guessed_edges() {
        let mut db = db_with_call_target();
        db.replace_summary_facts(SummaryOutput {
            summaries: vec![summary(
                SummaryStatus::Present,
                SummaryDomainKind::ControlEffects,
            )],
            events: Vec::new(),
        });

        let output = derive_summary_assisted_refinements(&db);

        assert!(output.edges.is_empty());
    }

    fn db_with_call_target() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            "src/app.ts".into(),
            "src/app.ts".to_string(),
            "function caller() { callee(); } function callee() {}\n".to_string(),
        );
        let caller = db.push_function(FunctionFact {
            id: FunctionId(99),
            file,
            name: "caller".to_string(),
            span: span(),
            language: Language::TypeScript,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: vec!["callee".to_string()],
        });
        let callee = db.push_function(FunctionFact {
            id: FunctionId(99),
            file,
            name: "callee".to_string(),
            span: span(),
            language: Language::TypeScript,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        db.replace_call_facts(CallOutput {
            sites: vec![CallSiteFact {
                id: CallSiteId(0),
                language: Language::TypeScript,
                file,
                caller,
                owner_symbol: None,
                body: MirBodyId(0),
                operation: MirOpId(0),
                span: span(),
                kind: CallSyntaxKind::Function,
                callee: CallCallee::Identifier {
                    reference: None,
                    name: "callee".to_string(),
                },
                receiver: None,
                arguments: Vec::new(),
                result: None,
                status: CallTargetStatus::Resolved,
                precision: CallPrecision::SetupAware,
                stable_key: "call-site:callee".to_string(),
            }],
            targets: vec![CallTargetFact {
                id: CallTargetId(0),
                site: CallSiteId(0),
                caller,
                target_function: Some(callee),
                target_symbol: Some(SymbolId(0)),
                edge_kind: CallEdgeKind::Direct,
                algorithm: CallAlgorithm::DirectReference,
                status: CallTargetStatus::Resolved,
                reason: None,
                provenance: CallProvenance::NativeDirect,
                precision: CallPrecision::SetupAware,
                stable_key: "call-target:callee".to_string(),
            }],
            unresolved: Vec::new(),
        })
        .expect("valid call facts");
        db
    }

    fn summary(status: SummaryStatus, domain: SummaryDomainKind) -> SummaryFact {
        SummaryFact {
            id: SummaryId(0),
            callable_stable_key: "function:caller".to_string(),
            function: FunctionId(0),
            domain,
            status,
            precision: SummaryPrecision::SetupAware,
            provenance: SummaryProvenance::NativeLocal,
            payload_digest: "digest".to_string(),
            stable_key: "summary:caller:call".to_string(),
        }
    }

    fn span() -> Span {
        Span::point(FileId(0), 1, 1)
    }
}

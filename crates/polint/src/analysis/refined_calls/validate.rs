use std::collections::BTreeSet;

use crate::analysis::calls::facts::{
    CallAlgorithm, CallEdgeKind, CallPrecision, CallProvenance, CallTargetStatus,
};
#[cfg(test)]
use crate::analysis::refined_calls::facts::RefinedCallTier;
use crate::analysis::refined_calls::facts::{RefinedCallEdgeFact, RefinedCallValidation};
use crate::analysis_kernel::FactFamily;
use crate::core::AnalysisDb;
use crate::diagnostics::{Diagnostic, Severity, TextRange, fingerprint};

pub(crate) fn validate_refined_calls(db: &AnalysisDb, diagnostics: &mut Vec<Diagnostic>) {
    validate_refined_call_edges(db, db.refined_call_edges(), diagnostics);
}

fn validate_refined_call_edges(
    db: &AnalysisDb,
    edges: &[RefinedCallEdgeFact],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let call_sites = db
        .call_sites()
        .iter()
        .map(|site| site.id)
        .collect::<BTreeSet<_>>();
    let call_targets = db
        .call_targets()
        .iter()
        .map(|target| target.id)
        .collect::<BTreeSet<_>>();
    let functions = db
        .functions()
        .iter()
        .map(|function| function.id)
        .collect::<BTreeSet<_>>();
    let symbols = db
        .symbols()
        .iter()
        .map(|symbol| symbol.id)
        .collect::<BTreeSet<_>>();
    let mut seen_stable_keys = BTreeSet::new();

    for edge in edges {
        if !seen_stable_keys.insert(edge.stable_key.as_str()) {
            diagnostics.push(invalid_refined_call_diagnostic(
                &edge.stable_key,
                "duplicate stable key".to_string(),
            ));
        }
        if !call_sites.contains(&edge.site) && !uses_synthetic_framework_site(edge) {
            diagnostics.push(invalid_refined_call_diagnostic(
                &edge.stable_key,
                format!("dangling call site {:?}", edge.site),
            ));
        }
        if let Some(base_target) = edge.base_target
            && !call_targets.contains(&base_target)
        {
            diagnostics.push(invalid_refined_call_diagnostic(
                &edge.stable_key,
                format!("dangling base call target {base_target:?}"),
            ));
        }
        if !functions.contains(&edge.caller) {
            diagnostics.push(invalid_refined_call_diagnostic(
                &edge.stable_key,
                format!("dangling caller {:?}", edge.caller),
            ));
        }
        if let Some(target_function) = edge.target_function
            && !functions.contains(&target_function)
        {
            diagnostics.push(invalid_refined_call_diagnostic(
                &edge.stable_key,
                format!("dangling target function {target_function:?}"),
            ));
        }
        if let Some(target_symbol) = edge.target_symbol
            && !symbols.contains(&target_symbol)
        {
            diagnostics.push(invalid_refined_call_diagnostic(
                &edge.stable_key,
                format!("dangling target symbol {target_symbol:?}"),
            ));
        }
        if !edge
            .stable_key
            .contains(FactFamily::RefinedCallEdge.label())
        {
            diagnostics.push(invalid_refined_call_diagnostic(
                &edge.stable_key,
                "stable key does not include refined call fact family".to_string(),
            ));
        }
        if edge.evidence.is_empty() {
            diagnostics.push(invalid_refined_call_diagnostic(
                &edge.stable_key,
                "missing refined call evidence".to_string(),
            ));
        }
        if edge.input_stable_keys.is_empty() {
            diagnostics.push(invalid_refined_call_diagnostic(
                &edge.stable_key,
                "missing refined call input stable keys".to_string(),
            ));
        }
        if edge.provenance == CallProvenance::Model && edge.precision == CallPrecision::Exact {
            diagnostics.push(invalid_refined_call_diagnostic(
                &edge.stable_key,
                "model refined call edge cannot claim exact precision".to_string(),
            ));
        }
        if edge.validation == RefinedCallValidation::ExtensionValidated
            && matches!(edge.precision, CallPrecision::Exact)
        {
            diagnostics.push(invalid_refined_call_diagnostic(
                &edge.stable_key,
                "extension refined call edge cannot claim exact precision".to_string(),
            ));
        }
        if matches!(
            edge.algorithm,
            CallAlgorithm::FunctionTokenFlow
                | CallAlgorithm::PointsTo
                | CallAlgorithm::FrameworkModel
                | CallAlgorithm::RepoModel
        ) && edge.precision == CallPrecision::Exact
        {
            diagnostics.push(invalid_refined_call_diagnostic(
                &edge.stable_key,
                "dynamic refined call algorithm cannot claim exact precision".to_string(),
            ));
        }
        if let Some(synthetic_target) = &edge.synthetic_target
            && !valid_synthetic_target(synthetic_target)
        {
            diagnostics.push(invalid_refined_call_diagnostic(
                &edge.stable_key,
                format!("malformed synthetic target `{synthetic_target}`"),
            ));
        }
        if matches!(
            edge.status,
            CallTargetStatus::Resolved | CallTargetStatus::Ambiguous
        ) && edge.target_function.is_none()
            && edge.target_symbol.is_none()
            && edge.synthetic_target.is_none()
        {
            diagnostics.push(invalid_refined_call_diagnostic(
                &edge.stable_key,
                "resolved or ambiguous refined edge has no target".to_string(),
            ));
        }
    }
}

fn uses_synthetic_framework_site(
    edge: &crate::analysis::refined_calls::facts::RefinedCallEdgeFact,
) -> bool {
    edge.base_target.is_none()
        && edge.edge_kind == CallEdgeKind::Synthetic
        && edge
            .evidence
            .iter()
            .any(|evidence| evidence == "framework_dispatch" || evidence == "unresolved_framework")
}

fn valid_synthetic_target(target: &str) -> bool {
    !target.trim().is_empty()
        && !target.starts_with('/')
        && !target.contains("..")
        && target.contains(':')
}

fn invalid_refined_call_diagnostic(stable_key: &str, reason: String) -> Diagnostic {
    Diagnostic {
        rule_id: "polint/internal".to_string(),
        severity: Severity::Error,
        message: format!("invalid refined call fact `{stable_key}`: {reason}"),
        file: "<workspace>".to_string(),
        range: TextRange::point(1, 1),
        labels: Vec::new(),
        help: None,
        evidence: Vec::new(),
        evidence_v1: None,
        evidence_bundle: None,
        suggestions: Vec::new(),
        fix: None,
        stable_fingerprint: fingerprint(&["polint.refined_calls.validate", stable_key, &reason]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::calls::facts::{
        CallAlgorithm, CallCallee, CallPrecision, CallProvenance, CallSiteFact, CallSyntaxKind,
        CallTargetStatus,
    };
    use crate::analysis::calls::store::CallOutput;
    use crate::analysis::ids::{CallSiteId, MirBodyId, MirOpId, RefinedCallEdgeId};
    use crate::analysis::refined_calls::facts::RefinedCallConfidence;
    use crate::analysis::refined_calls::store::RefinedCallOutput;
    use crate::core::{FileId, FunctionFact, FunctionId, Language, Span};

    #[test]
    fn validation_catches_dangling_call_site() {
        let mut db = db_with_call_site();
        db.replace_refined_call_facts(RefinedCallOutput {
            edges: vec![edge("family=RefinedCallEdge/test", CallSiteId(99))],
        })
        .expect("store accepts edge for validation");
        let mut diagnostics = Vec::new();

        validate_refined_calls(&db, &mut diagnostics);

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("dangling call site"))
        );
    }

    #[test]
    fn validation_catches_exact_model_refinement() {
        let db = db_with_call_site();
        let mut refined = edge("family=RefinedCallEdge/model", CallSiteId(0));
        refined.precision = CallPrecision::Exact;
        refined.provenance = CallProvenance::Model;
        refined.algorithm = CallAlgorithm::FrameworkModel;
        refined.tier = RefinedCallTier::DirectPlusFramework;
        let mut diagnostics = Vec::new();

        validate_refined_call_edges(&db, &[refined], &mut diagnostics);

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot claim exact precision"))
        );
    }

    #[test]
    fn validation_catches_duplicate_stable_keys_deterministically() {
        let db = db_with_call_site();
        let edges = vec![
            edge("family=RefinedCallEdge/duplicate", CallSiteId(0)),
            edge("family=RefinedCallEdge/duplicate", CallSiteId(0)),
        ];
        let mut diagnostics = Vec::new();

        validate_refined_call_edges(&db, &edges, &mut diagnostics);

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("duplicate stable key"))
        );
    }

    #[test]
    fn validation_catches_missing_evidence_and_inputs() {
        let db = db_with_call_site();
        let mut refined = edge("family=RefinedCallEdge/no-evidence", CallSiteId(0));
        refined.evidence.clear();
        refined.input_stable_keys.clear();
        let mut diagnostics = Vec::new();

        validate_refined_call_edges(&db, &[refined], &mut diagnostics);

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("missing refined call evidence"))
        );
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("missing refined call input stable keys")
        }));
    }

    fn db_with_call_site() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            "app.ts".into(),
            "app.ts".to_string(),
            "callee();\n".to_string(),
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
            calls: vec!["callee".to_string()],
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
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("valid calls");
        db
    }

    fn edge(stable_key: &str, site: CallSiteId) -> RefinedCallEdgeFact {
        RefinedCallEdgeFact {
            id: RefinedCallEdgeId(0),
            site,
            base_target: None,
            caller: FunctionId(0),
            target_function: Some(FunctionId(1)),
            target_symbol: None,
            synthetic_target: None,
            language: Language::TypeScript,
            edge_kind: CallEdgeKind::Synthetic,
            algorithm: CallAlgorithm::RepoModel,
            tier: RefinedCallTier::ExtensionModel,
            status: CallTargetStatus::Resolved,
            reason: None,
            provenance: CallProvenance::Extension,
            precision: CallPrecision::Heuristic,
            validation: RefinedCallValidation::ExtensionValidated,
            confidence: RefinedCallConfidence::Medium,
            evidence: vec!["test".to_string()],
            input_stable_keys: vec!["input".to_string()],
            stable_key: stable_key.to_string(),
        }
    }

    fn span() -> Span {
        Span::point(FileId(0), 1, 1)
    }
}

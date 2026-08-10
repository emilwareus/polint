use std::collections::BTreeSet;

use super::facts::{
    RefinedCallConfidence, RefinedCallEdgeFact, RefinedCallTier, RefinedCallValidation,
};
use super::store::RefinedCallOutput;
use crate::analysis::calls::facts::{
    CallAlgorithm, CallCallee, CallEdgeKind, CallProvenance, CallTargetStatus, UnresolvedCallFact,
    UnresolvedCallReason,
};
use crate::analysis::ids::{PlaceId, RefinedCallEdgeId};
use crate::analysis::points_to::facts::PointsToStatus;
use crate::analysis::semantic_graph::constraints::ConstraintKind;
use crate::analysis::semantic_graph::facts::NodeKind;
use crate::analysis_kernel::{FactFamily, stable_key_text_from_parts};
use crate::core::AnalysisDb;

pub(crate) fn derive_ts_js_refinements(db: &AnalysisDb) -> RefinedCallOutput {
    let edges = db
        .unresolved_calls()
        .iter()
        .filter_map(|unresolved| unresolved_edge(db, unresolved))
        .collect();
    RefinedCallOutput { edges }.normalized()
}

fn unresolved_edge(
    db: &AnalysisDb,
    unresolved: &UnresolvedCallFact,
) -> Option<RefinedCallEdgeFact> {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    let site = db
        .call_sites()
        .iter()
        .find(|site| site.id == unresolved.site && site.language.is_ts_family())?;
    if solver_resolved_site(db, site.id) {
        return None;
    }
    let status = match unresolved.reason {
        UnresolvedCallReason::SetupMissing => CallTargetStatus::SetupMissing,
        UnresolvedCallReason::BudgetExceeded => CallTargetStatus::BudgetExceeded,
        UnresolvedCallReason::UnsupportedSyntax => CallTargetStatus::Unsupported,
        _ => CallTargetStatus::Unresolved,
    };
    let unresolved_key = db.resolve_stable_key(unresolved.stable_key).to_string();
    let place = callable_place(site);

    Some(RefinedCallEdgeFact {
        id: RefinedCallEdgeId(0),
        site: unresolved.site,
        base_target: None,
        caller: unresolved.caller,
        target_function: None,
        target_symbol: None,
        synthetic_target: place.map(|place| format!("ts-js:callable-place:{}", place.0)),
        language: site.language,
        edge_kind: CallEdgeKind::Unknown,
        algorithm: CallAlgorithm::PointsTo,
        tier: RefinedCallTier::PointsToAssisted,
        status,
        reason: Some(unresolved.reason),
        provenance: CallProvenance::Native,
        precision: unresolved.precision,
        validation: RefinedCallValidation::ReferentiallyValidated,
        confidence: RefinedCallConfidence::Low,
        evidence: vec!["ts_js_points_to_unresolved".to_string()],
        input_stable_keys: vec![unresolved_key.clone()],
        stable_key: stable_key_text_from_parts(
            interner,
            FactFamily::RefinedCallEdge,
            &[
                ("tier", "ts_js_points_to_unresolved".to_string()),
                ("unresolved", unresolved_key),
                ("status", format!("{status:?}")),
            ],
        ),
    })
}

fn solver_resolved_site(db: &AnalysisDb, site: crate::analysis::ids::CallSiteId) -> bool {
    let Some(callsite_node) = db.semantic_nodes().iter().find_map(|node| match node.kind {
        NodeKind::Callsite(candidate) if candidate == site => Some(node.id),
        _ => None,
    }) else {
        return false;
    };
    let constraint_keys = db
        .semantic_constraints()
        .iter()
        .filter_map(|constraint| match constraint.kind {
            ConstraintKind::CallConstraint { callsite } if callsite == callsite_node => {
                Some(db.resolve_stable_key(constraint.stable_key).to_string())
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();

    db.solver_derived_edges().iter().any(|edge| {
        edge.status == PointsToStatus::Present
            && edge.provenance.constraint_kind == "call_constraint"
            && edge
                .provenance
                .contributing_facts
                .iter()
                .any(|fact| constraint_keys.contains(&fact.stable_key))
    })
}

fn callable_place(site: &crate::analysis::calls::facts::CallSiteFact) -> Option<PlaceId> {
    match site.callee {
        CallCallee::FunctionValue { place } => Some(place),
        _ => site.receiver,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::calls::facts::{
        CallPrecision, CallSiteFact, CallSyntaxKind, UnresolvedCallFact,
    };
    use crate::analysis::calls::store::CallOutput;
    use crate::analysis::ids::{CallSiteId, MirBodyId, MirOpId};
    use crate::core::{FileId, FunctionFact, FunctionId, Language, Span};

    #[test]
    fn unresolved_ts_call_is_reported_as_points_to_unresolved() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            "src/app.ts".into(),
            "src/app.ts".to_string(),
            "function caller(fn) { fn(); }\n".to_string(),
        );
        db.push_function(FunctionFact {
            id: FunctionId(0),
            file,
            name: "caller".to_string(),
            span: Span::point(file, 1, 1),
            language: Language::TypeScript,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: vec!["fn".to_string()],
        });
        let site = CallSiteFact {
            in_throw: false,
            id: CallSiteId(0),
            language: Language::TypeScript,
            file: FileId(0),
            caller: FunctionId(0),
            owner_symbol: None,
            body: MirBodyId(0),
            operation: MirOpId(0),
            span: Span::point(file, 1, 1),
            kind: CallSyntaxKind::FunctionValue,
            callee: CallCallee::FunctionValue { place: PlaceId(0) },
            receiver: None,
            arguments: Vec::new(),
            result: None,
            status: CallTargetStatus::Unresolved,
            precision: CallPrecision::Unknown,
            stable_key: crate::core::StableKeyId(0),
        };
        db.replace_call_facts(CallOutput {
            sites: vec![site],
            targets: Vec::new(),
            unresolved: vec![UnresolvedCallFact {
                site: CallSiteId(0),
                caller: FunctionId(0),
                status: CallTargetStatus::Unresolved,
                reason: UnresolvedCallReason::FunctionValue,
                algorithm: CallAlgorithm::Unsupported,
                provenance: CallProvenance::MirShape,
                precision: CallPrecision::Unknown,
                stable_key: crate::core::StableKeyId(1),
            }],
        })
        .expect("valid call facts");

        let output = derive_ts_js_refinements(&db);

        assert_eq!(output.edges.len(), 1);
        assert_eq!(output.edges[0].algorithm, CallAlgorithm::PointsTo);
        assert_eq!(output.edges[0].tier, RefinedCallTier::PointsToAssisted);
    }
}

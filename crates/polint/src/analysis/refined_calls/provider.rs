use std::collections::BTreeMap;

use super::cache_key::{
    refined_calls_provider_parameter_digest, refined_calls_provider_parameter_digest_for_snapshot,
};
use super::facts::{
    RefinedCallConfidence, RefinedCallEdgeFact, RefinedCallTier, RefinedCallValidation,
};
use super::store::RefinedCallOutput;
use crate::analysis::calls::facts::{
    CallAlgorithm, CallEdgeKind, CallPrecision, CallProvenance, CallSiteFact, CallSyntaxKind,
    CallTargetFact, CallTargetStatus, UnresolvedCallReason,
};
use crate::analysis::points_to::facts::{PointsToPrecision, PointsToStatus};
use crate::analysis::semantic_graph::constraints::ConstraintKind;
use crate::analysis::semantic_graph::facts::NodeKind;
use crate::analysis::solver::facts::DerivedEdgeFact;
use crate::analysis_kernel::incremental::{
    CacheStats, Digest, DigestKind, InputComponent, InputSnapshot,
};
use crate::analysis_kernel::{FactFamily, FactRef, ProviderManifest, stable_key_from_parts};
use crate::core::{AnalysisDb, StableKeyInterner};
use crate::diagnostics::Diagnostic;
use serde::Serialize;

pub(crate) const REFINED_CALLS_PROVIDER_ID: &str = "polint.refined_calls";

#[derive(Debug, Clone, Default)]
pub(crate) struct RefinedCallsProviderOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_refined_calls_with_cache_stats(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    calls_output_digest: Digest,
    entrypoints_output_digest: Digest,
    direct_summaries_output_digest: Digest,
    type_value_alias_output_digest: Digest,
    extensions_output_digest: Digest,
    solver_output_digest: Digest,
) -> RefinedCallsProviderOutput {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    debug_assert_eq!(manifest.id, REFINED_CALLS_PROVIDER_ID);
    let mut output = RefinedCallOutput::empty();
    let call_site_languages = db
        .call_sites()
        .iter()
        .map(|site| (site.id, site.language))
        .collect::<BTreeMap<_, _>>();
    for target in db.call_targets() {
        let base_target_key = db
            .metadata_for(FactRef::new(FactFamily::CallTarget, target.id.0))
            .map(|metadata| db.resolve_stable_key(metadata.stable_key).to_string())
            .unwrap_or_else(|| interner.resolve(target.stable_key).to_string());
        output.edges.push(refined_edge_from_base_target(
            interner,
            target,
            crate::analysis::ids::RefinedCallEdgeId(0),
            RefinedCallTier::DirectOnly,
            call_site_languages
                .get(&target.site)
                .copied()
                .unwrap_or(crate::core::Language::Unknown),
            base_target_key,
        ));
    }
    output
        .edges
        .extend(super::framework::derive_framework_refinements(db).edges);
    output
        .edges
        .extend(super::go::derive_go_refinements(db).edges);
    output
        .edges
        .extend(super::ts_js::derive_ts_js_refinements(db).edges);
    output
        .edges
        .extend(super::summaries::derive_summary_assisted_refinements(db).edges);
    output
        .edges
        .extend(super::extensions::derive_extension_refinements(db).edges);
    output.edges.extend(derive_solver_refinements(db).edges);
    output = finalized_output(interner, output);

    let output_digest = refined_calls_output_digest(
        interner,
        manifest,
        input_snapshot,
        &calls_output_digest,
        &entrypoints_output_digest,
        &direct_summaries_output_digest,
        &type_value_alias_output_digest,
        &extensions_output_digest,
        &solver_output_digest,
        &output,
    );
    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();

    match db.replace_normalized_refined_call_facts(output) {
        Ok(()) => RefinedCallsProviderOutput {
            diagnostics: Vec::new(),
            cache_stats,
            output_digest: Some(output_digest),
        },
        Err(error) => RefinedCallsProviderOutput {
            diagnostics: vec![provider_error_diagnostic(error.to_string())],
            cache_stats,
            output_digest: Some(output_digest),
        },
    }
}

fn finalized_output(
    interner: &crate::core::StableKeyInterner,
    mut output: RefinedCallOutput,
) -> RefinedCallOutput {
    output = output.normalized(interner);
    for (index, edge) in output.edges.iter_mut().enumerate() {
        edge.id = crate::analysis::ids::RefinedCallEdgeId(index as u64);
    }
    output
}

fn refined_edge_from_base_target(
    interner: &crate::core::StableKeyInterner,
    target: &CallTargetFact,
    id: crate::analysis::ids::RefinedCallEdgeId,
    tier: RefinedCallTier,
    language: crate::core::Language,
    base_target_key: String,
) -> RefinedCallEdgeFact {
    RefinedCallEdgeFact {
        id,
        site: target.site,
        base_target: Some(target.id),
        caller: target.caller,
        target_function: target.target_function,
        target_symbol: target.target_symbol,
        synthetic_target: None,
        language,
        edge_kind: target.edge_kind,
        algorithm: target.algorithm,
        tier,
        status: target.status,
        reason: target.reason,
        provenance: target.provenance,
        precision: target.precision,
        validation: validation_for_target(target),
        confidence: confidence_for_target(target),
        evidence: vec!["base_call_target".to_string()],
        input_stable_keys: vec![base_target_key.clone()],
        stable_key: stable_refined_call_key(interner, target, tier, &base_target_key),
    }
}

fn derive_solver_refinements(db: &AnalysisDb) -> RefinedCallOutput {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    let index = SolverProjectionIndex::new(db);
    let mut output = RefinedCallOutput::empty();
    output.edges.extend(
        db.solver_derived_edges()
            .iter()
            .filter_map(|edge| refined_edge_from_solver_edge(interner, &index, edge)),
    );
    output
}

fn refined_edge_from_solver_edge(
    interner: &crate::core::StableKeyInterner,
    index: &SolverProjectionIndex<'_>,
    edge: &DerivedEdgeFact,
) -> Option<RefinedCallEdgeFact> {
    if edge.provenance.constraint_kind != "call_constraint" {
        return None;
    }
    let caller = index.function_by_node.get(&edge.source).copied()?;
    let target_function = index.function_by_node.get(&edge.target).copied()?;
    let site = index.callsite_for_solver_edge(interner, edge)?;
    if site.caller != caller {
        return None;
    }

    Some(RefinedCallEdgeFact {
        id: crate::analysis::ids::RefinedCallEdgeId(0),
        site: site.id,
        base_target: None,
        caller,
        target_function: Some(target_function),
        target_symbol: None,
        synthetic_target: None,
        language: site.language,
        edge_kind: solver_edge_kind_for_site(site),
        algorithm: solver_algorithm_for_site(site),
        tier: RefinedCallTier::PointsToAssisted,
        status: call_status_for_solver_edge(edge.status),
        reason: unresolved_reason_for_solver_edge(edge.status),
        provenance: CallProvenance::Model,
        precision: call_precision_for_solver_edge(edge.precision),
        validation: RefinedCallValidation::ReferentiallyValidated,
        confidence: confidence_for_solver_edge(edge.status),
        evidence: solver_evidence(edge),
        input_stable_keys: solver_input_stable_keys(interner, edge),
        stable_key: stable_refined_call_key_from_solver_edge(interner, site, edge),
    })
}

struct SolverProjectionIndex<'a> {
    function_by_node: BTreeMap<crate::analysis::ids::SemanticNodeId, crate::core::FunctionId>,
    callsite_by_stable_key: BTreeMap<String, &'a CallSiteFact>,
}

impl<'a> SolverProjectionIndex<'a> {
    fn new(db: &'a AnalysisDb) -> Self {
        let function_by_node = db
            .semantic_nodes()
            .iter()
            .filter_map(|node| {
                let NodeKind::Function(function) = node.kind else {
                    return None;
                };
                Some((node.id, function))
            })
            .collect();
        let callsite_by_id = db
            .call_sites()
            .iter()
            .map(|site| (site.id, site))
            .collect::<BTreeMap<_, _>>();
        let callsite_id_by_node = db
            .semantic_nodes()
            .iter()
            .filter_map(|node| {
                let NodeKind::Callsite(callsite) = node.kind else {
                    return None;
                };
                Some((node.id, callsite))
            })
            .collect::<BTreeMap<_, _>>();
        let mut callsite_by_stable_key = db
            .call_sites()
            .iter()
            .map(|site| (db.resolve_stable_key(site.stable_key).to_string(), site))
            .collect::<BTreeMap<_, _>>();
        for constraint in db.semantic_constraints() {
            let ConstraintKind::CallConstraint { callsite } = &constraint.kind else {
                continue;
            };
            let Some(site) = callsite_id_by_node
                .get(callsite)
                .and_then(|callsite| callsite_by_id.get(callsite))
                .copied()
            else {
                continue;
            };
            callsite_by_stable_key.insert(
                db.resolve_stable_key(constraint.stable_key).to_string(),
                site,
            );
        }
        for callsite in db.go_semantic_callsites() {
            let Some(site) = core_callsite_for_go_semantic_callsite(db, callsite) else {
                continue;
            };
            callsite_by_stable_key
                .insert(db.resolve_stable_key(callsite.stable_key).to_string(), site);
        }
        Self {
            function_by_node,
            callsite_by_stable_key,
        }
    }

    fn callsite_for_solver_edge(
        &self,
        interner: &crate::core::StableKeyInterner,
        edge: &DerivedEdgeFact,
    ) -> Option<&'a CallSiteFact> {
        edge.provenance.contributing_facts.iter().find_map(|fact| {
            self.callsite_by_stable_key
                .get(interner.resolve(fact.stable_key).as_ref())
                .copied()
        })
    }
}

fn core_callsite_for_go_semantic_callsite<'a>(
    db: &'a AnalysisDb,
    callsite: &crate::go::semantic::facts::GoSemanticCallsiteFact,
) -> Option<&'a CallSiteFact> {
    let file = callsite.file?;
    let span = callsite.span.as_ref()?;
    let mut candidates = db
        .call_sites()
        .iter()
        .filter(|site| {
            site.language == crate::core::Language::Go
                && site.file == file
                && same_byte_span(&site.span, span)
        })
        .collect::<Vec<_>>();
    let caller = core_function_for_go_semantic_function(db, &callsite.caller);
    if let Some(caller) = caller {
        let caller_matches = candidates
            .iter()
            .copied()
            .filter(|site| site.caller == caller)
            .collect::<Vec<_>>();
        if !caller_matches.is_empty() {
            candidates = caller_matches;
        }
    }
    let dynamic_matches = candidates
        .iter()
        .copied()
        .filter(|site| {
            matches!(
                site.status,
                CallTargetStatus::Unresolved | CallTargetStatus::Ambiguous
            )
        })
        .collect::<Vec<_>>();
    if !dynamic_matches.is_empty() {
        candidates = dynamic_matches;
    }
    candidates
        .into_iter()
        .min_by_key(|site| db.resolve_stable_key(site.stable_key))
}

fn core_function_for_go_semantic_function(
    db: &AnalysisDb,
    qualified: &str,
) -> Option<crate::core::FunctionId> {
    db.go_semantic_functions()
        .iter()
        .filter(|function| function.qualified == qualified)
        .filter_map(|function| core_function_for_go_semantic_fact(db, function))
        .next()
}

fn core_function_for_go_semantic_fact(
    db: &AnalysisDb,
    function: &crate::go::semantic::facts::GoSemanticFunctionFact,
) -> Option<crate::core::FunctionId> {
    let file = function.file?;
    let span = function.span.as_ref()?;
    matching_core_function_for_go_semantic_span(db, file, &function.name, span).map(|core| core.id)
}

fn matching_core_function_for_go_semantic_span<'a>(
    db: &'a AnalysisDb,
    file: crate::core::FileId,
    name: &str,
    span: &crate::core::Span,
) -> Option<&'a crate::core::FunctionFact> {
    let bucket = db
        .functions()
        .iter()
        .filter(|core| {
            core.language == crate::core::Language::Go && core.file == file && core.name == name
        })
        .collect::<Vec<_>>();
    if let Some(exact) = bucket
        .iter()
        .copied()
        .find(|core| same_byte_span(&core.span, span))
    {
        return Some(exact);
    }
    if span.start_byte != span.end_byte {
        return None;
    }
    bucket
        .iter()
        .copied()
        .filter(|core| {
            core.span.start_byte <= span.start_byte && span.start_byte <= core.span.end_byte
        })
        .min_by_key(|core| core.span.end_byte.saturating_sub(core.span.start_byte))
}

fn same_byte_span(left: &crate::core::Span, right: &crate::core::Span) -> bool {
    left.file == right.file
        && left.start_byte == right.start_byte
        && left.end_byte == right.end_byte
}

fn solver_edge_kind_for_site(site: &CallSiteFact) -> CallEdgeKind {
    match site.kind {
        CallSyntaxKind::Method | CallSyntaxKind::Member => CallEdgeKind::Method,
        CallSyntaxKind::Constructor | CallSyntaxKind::New => CallEdgeKind::Constructor,
        CallSyntaxKind::GoRoutine => CallEdgeKind::Spawn,
        CallSyntaxKind::Deferred => CallEdgeKind::Deferred,
        CallSyntaxKind::FunctionValue => CallEdgeKind::FunctionValue,
        CallSyntaxKind::Function => CallEdgeKind::FunctionValue,
        _ => CallEdgeKind::Unknown,
    }
}

fn solver_algorithm_for_site(site: &CallSiteFact) -> CallAlgorithm {
    match site.language {
        crate::core::Language::Go => CallAlgorithm::GoRta,
        language if language.is_ts_family() => CallAlgorithm::PointsTo,
        _ => CallAlgorithm::PointsTo,
    }
}

fn call_status_for_solver_edge(status: PointsToStatus) -> CallTargetStatus {
    match status {
        PointsToStatus::Present => CallTargetStatus::Resolved,
        PointsToStatus::Unknown => CallTargetStatus::Unresolved,
        PointsToStatus::Unsupported => CallTargetStatus::Unsupported,
        PointsToStatus::SetupMissing => CallTargetStatus::SetupMissing,
        PointsToStatus::BudgetExceeded => CallTargetStatus::BudgetExceeded,
    }
}

fn unresolved_reason_for_solver_edge(status: PointsToStatus) -> Option<UnresolvedCallReason> {
    match status {
        PointsToStatus::BudgetExceeded => Some(UnresolvedCallReason::BudgetExceeded),
        PointsToStatus::SetupMissing => Some(UnresolvedCallReason::SetupMissing),
        PointsToStatus::Unsupported => Some(UnresolvedCallReason::UnsupportedSyntax),
        PointsToStatus::Unknown => Some(UnresolvedCallReason::Unknown),
        PointsToStatus::Present => None,
    }
}

fn call_precision_for_solver_edge(precision: PointsToPrecision) -> CallPrecision {
    match precision {
        PointsToPrecision::FlowInsensitive | PointsToPrecision::LocalFlowSensitive => {
            CallPrecision::SetupAware
        }
        PointsToPrecision::SummaryProjected | PointsToPrecision::Heuristic => {
            CallPrecision::Heuristic
        }
        PointsToPrecision::Unknown => CallPrecision::Unknown,
        PointsToPrecision::Unsupported => CallPrecision::Unsupported,
    }
}

fn confidence_for_solver_edge(status: PointsToStatus) -> RefinedCallConfidence {
    match status {
        PointsToStatus::Present => RefinedCallConfidence::Medium,
        PointsToStatus::BudgetExceeded => RefinedCallConfidence::Low,
        PointsToStatus::Unknown | PointsToStatus::Unsupported | PointsToStatus::SetupMissing => {
            RefinedCallConfidence::Low
        }
    }
}

fn solver_evidence(edge: &DerivedEdgeFact) -> Vec<String> {
    vec![
        "solver_derived_edge".to_string(),
        format!("constraint:{}", edge.provenance.constraint_kind),
    ]
}

fn solver_input_stable_keys(
    interner: &crate::core::StableKeyInterner,
    edge: &DerivedEdgeFact,
) -> Vec<String> {
    let mut keys = vec![interner.resolve(edge.stable_key).to_string()];
    keys.extend(
        edge.provenance
            .contributing_facts
            .iter()
            .map(|fact| interner.resolve(fact.stable_key).to_string()),
    );
    keys
}

fn stable_refined_call_key_from_solver_edge(
    interner: &crate::core::StableKeyInterner,
    site: &CallSiteFact,
    edge: &DerivedEdgeFact,
) -> crate::core::StableKeyId {
    stable_key_from_parts(
        interner,
        FactFamily::RefinedCallEdge,
        &[
            ("tier", format!("{:?}", RefinedCallTier::PointsToAssisted)),
            ("solver_edge", interner.resolve(edge.stable_key).to_string()),
            ("site", interner.resolve(site.stable_key).to_string()),
        ],
    )
}

fn validation_for_target(target: &CallTargetFact) -> RefinedCallValidation {
    match target.status {
        CallTargetStatus::Rejected => RefinedCallValidation::Rejected,
        _ => RefinedCallValidation::Native,
    }
}

fn confidence_for_target(target: &CallTargetFact) -> RefinedCallConfidence {
    match target.status {
        CallTargetStatus::Resolved => RefinedCallConfidence::High,
        CallTargetStatus::Ambiguous => RefinedCallConfidence::Medium,
        CallTargetStatus::Unresolved
        | CallTargetStatus::Unsupported
        | CallTargetStatus::SetupMissing
        | CallTargetStatus::BudgetExceeded
        | CallTargetStatus::Rejected => RefinedCallConfidence::Low,
    }
}

fn stable_refined_call_key(
    interner: &crate::core::StableKeyInterner,
    target: &CallTargetFact,
    tier: RefinedCallTier,
    base_target_key: &str,
) -> crate::core::StableKeyId {
    stable_key_from_parts(
        interner,
        FactFamily::RefinedCallEdge,
        &[
            ("tier", format!("{tier:?}")),
            ("base_target", base_target_key.to_string()),
            ("site", target.site.0.to_string()),
        ],
    )
}

#[allow(clippy::too_many_arguments)]
fn refined_calls_output_digest(
    interner: &crate::core::StableKeyInterner,
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    calls_output_digest: &Digest,
    entrypoints_output_digest: &Digest,
    direct_summaries_output_digest: &Digest,
    type_value_alias_output_digest: &Digest,
    extensions_output_digest: &Digest,
    solver_output_digest: &Digest,
    output: &RefinedCallOutput,
) -> Digest {
    let upstream = vec![
        calls_output_digest.clone(),
        entrypoints_output_digest.clone(),
        direct_summaries_output_digest.clone(),
        type_value_alias_output_digest.clone(),
        extensions_output_digest.clone(),
        solver_output_digest.clone(),
    ];
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!("parameters={}", refined_calls_provider_parameter_digest()),
        format!(
            "input_parameters={}",
            refined_calls_provider_parameter_digest_for_snapshot(input_snapshot, &upstream)
        ),
        format!("config={}", input_snapshot.config.digest),
        format!("calls={calls_output_digest}"),
        format!("entrypoints={entrypoints_output_digest}"),
        format!("direct_summaries={direct_summaries_output_digest}"),
        format!("type_value_alias={type_value_alias_output_digest}"),
        format!("extensions={extensions_output_digest}"),
        format!("solver={solver_output_digest}"),
    ];
    extend_component_parts(
        &mut parts,
        "go_lifecycle",
        &input_snapshot.go_lifecycle.components,
    );
    extend_component_parts(
        &mut parts,
        "ts_js_lifecycle",
        &input_snapshot.ts_js_lifecycle.components,
    );
    extend_component_parts(&mut parts, "model", &input_snapshot.models);
    extend_component_parts(&mut parts, "extension", &input_snapshot.extensions);
    extend_component_parts(&mut parts, "tool", &input_snapshot.tool_invocations);
    parts.extend(output.edges.iter().map(|edge| {
        format!(
            "refined_call_edge={}",
            refined_call_edge_payload(interner, edge)
        )
    }));
    if output.edges.is_empty() {
        parts.push("refined_calls_output=empty".to_string());
    }

    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "refined_calls_output", &refs)
}

fn extend_component_parts(parts: &mut Vec<String>, prefix: &str, components: &[InputComponent]) {
    if components.is_empty() {
        parts.push(format!("{prefix}=absent"));
        return;
    }
    parts.extend(components.iter().map(|component| {
        format!(
            "{prefix}:{}:{:?}:{}",
            component.name, component.status, component.digest
        )
    }));
}

fn refined_call_edge_payload(interner: &StableKeyInterner, edge: &RefinedCallEdgeFact) -> String {
    serde_json::to_string(&RefinedCallEdgeDigest {
        id: edge.id,
        site: edge.site,
        base_target: edge.base_target,
        caller: edge.caller,
        target_function: edge.target_function,
        target_symbol: edge.target_symbol,
        synthetic_target: edge.synthetic_target.as_deref(),
        language: edge.language,
        edge_kind: edge.edge_kind,
        algorithm: edge.algorithm,
        tier: edge.tier,
        status: edge.status,
        reason: edge.reason,
        provenance: edge.provenance,
        precision: edge.precision,
        validation: edge.validation,
        confidence: edge.confidence,
        evidence: &edge.evidence,
        input_stable_keys: &edge.input_stable_keys,
        stable_key: interner.resolve(edge.stable_key).as_ref(),
    })
    .unwrap_or_else(|_| "{}".to_string())
}

#[derive(Serialize)]
struct RefinedCallEdgeDigest<'a> {
    id: crate::analysis::ids::RefinedCallEdgeId,
    site: crate::analysis::ids::CallSiteId,
    base_target: Option<crate::analysis::ids::CallTargetId>,
    caller: crate::core::FunctionId,
    target_function: Option<crate::core::FunctionId>,
    target_symbol: Option<crate::core::SymbolId>,
    synthetic_target: Option<&'a str>,
    language: crate::core::Language,
    edge_kind: CallEdgeKind,
    algorithm: CallAlgorithm,
    tier: RefinedCallTier,
    status: CallTargetStatus,
    reason: Option<UnresolvedCallReason>,
    provenance: CallProvenance,
    precision: CallPrecision,
    validation: RefinedCallValidation,
    confidence: RefinedCallConfidence,
    evidence: &'a [String],
    input_stable_keys: &'a [String],
    stable_key: &'a str,
}

#[cfg(test)]
mod solver_projection_tests {
    use super::*;
    use crate::analysis::calls::facts::CallCallee;
    use crate::analysis::calls::store::CallOutput;
    use crate::analysis::ids::{
        CallSiteId, DerivedEdgeId, MirBodyId, MirOpId, SemanticConstraintId, SemanticNodeId,
    };
    use crate::analysis::semantic_graph::constraints::{ConstraintFact, ConstraintKind};
    use crate::analysis::semantic_graph::facts::{SemanticNodeFact, SemanticPrecision};
    use crate::analysis::semantic_graph::store::SemanticGraphOutput;
    use crate::analysis::solver::budget::BudgetStatus;
    use crate::analysis::solver::provenance::{ContributingFact, DerivedEdgeProvenance};
    use crate::analysis::solver::store::SolverOutput;
    use crate::analysis_kernel::AnalysisKernel;
    use crate::analysis_kernel::incremental::InputSnapshot;
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::load_config;
    use crate::core::{FileId, FunctionFact, FunctionId, Language, Span};
    use crate::go::semantic::facts::{
        GoSemanticCallStatus, GoSemanticCallsiteFact, GoSemanticFunctionFact, GoSemanticFunctionId,
        GoSemanticFunctionKind,
    };
    use crate::go::semantic::store::GoSemanticFactsOutput;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn solver_derived_call_edges_project_to_refined_calls() {
        let db = db_with_solver_edge();

        let output = derive_solver_refinements(&db);

        assert_eq!(output.edges.len(), 1);
        let edge = &output.edges[0];
        assert_eq!(edge.site, CallSiteId(0));
        assert_eq!(edge.base_target, None);
        assert_eq!(edge.caller, FunctionId(0));
        assert_eq!(edge.target_function, Some(FunctionId(1)));
        assert_eq!(edge.tier, RefinedCallTier::PointsToAssisted);
        assert_eq!(edge.status, CallTargetStatus::Resolved);
        assert_eq!(edge.precision, CallPrecision::SetupAware);
        assert_eq!(
            edge.validation,
            RefinedCallValidation::ReferentiallyValidated
        );
        assert!(edge.evidence.contains(&"solver_derived_edge".to_string()));
        assert!(
            edge.input_stable_keys
                .iter()
                .any(|key| key == "call-site:callee")
        );
    }

    #[test]
    fn solver_projection_resolves_semantic_call_constraint_keys_to_core_callsites() {
        let db = db_with_solver_edge_referenced_by_semantic_constraint_key();

        let output = derive_solver_refinements(&db);

        assert_eq!(output.edges.len(), 1);
        let edge = &output.edges[0];
        assert_eq!(edge.site, CallSiteId(0));
        assert_eq!(edge.caller, FunctionId(0));
        assert_eq!(edge.target_function, Some(FunctionId(1)));
        assert_eq!(edge.tier, RefinedCallTier::PointsToAssisted);
        assert!(
            edge.input_stable_keys
                .iter()
                .any(|key| key == "constraint:go-semantic-callsite")
        );
    }

    #[test]
    fn solver_projection_resolves_go_semantic_callsite_keys_to_core_callsites() {
        let db = db_with_solver_edge_referenced_by_go_semantic_callsite_key();

        let output = derive_solver_refinements(&db);

        assert_eq!(output.edges.len(), 1);
        let edge = &output.edges[0];
        assert_eq!(edge.site, CallSiteId(0));
        assert_eq!(edge.caller, FunctionId(0));
        assert_eq!(edge.target_function, Some(FunctionId(1)));
        assert_eq!(edge.tier, RefinedCallTier::PointsToAssisted);
        assert!(
            edge.input_stable_keys
                .iter()
                .any(|key| key == "go-semantic-callsite:caller-callee")
        );
    }

    #[test]
    fn solver_projection_resolves_zero_width_go_semantic_method_callers() {
        let db = db_with_zero_width_go_method_caller_solver_edge();

        let output = derive_solver_refinements(&db);

        assert_eq!(output.edges.len(), 1);
        let edge = &output.edges[0];
        assert_eq!(edge.site, CallSiteId(0));
        assert_eq!(edge.caller, FunctionId(0));
        assert_eq!(edge.target_function, Some(FunctionId(1)));
        assert_eq!(edge.tier, RefinedCallTier::PointsToAssisted);
        assert!(
            edge.input_stable_keys
                .iter()
                .any(|key| key == "go-semantic-callsite:method-dispatch")
        );
    }

    #[test]
    fn refined_calls_digest_changes_with_solver_output_digest() {
        let mut db_a = db_with_solver_edge();
        let snapshot_a = snapshot(&db_a);
        let base = derive_refined_calls_with_cache_stats(
            &mut db_a,
            &snapshot_a,
            manifest(),
            absent("polint.calls"),
            absent("polint.entrypoints"),
            absent("polint.direct_summaries"),
            absent("polint.type_value_alias"),
            absent("polint.extensions"),
            Digest::from_parts(DigestKind::ProviderOutput, "polint.solver", &["a"]),
        )
        .output_digest;

        let mut db_b = db_with_solver_edge();
        let snapshot_b = snapshot(&db_b);
        let changed = derive_refined_calls_with_cache_stats(
            &mut db_b,
            &snapshot_b,
            manifest(),
            absent("polint.calls"),
            absent("polint.entrypoints"),
            absent("polint.direct_summaries"),
            absent("polint.type_value_alias"),
            absent("polint.extensions"),
            Digest::from_parts(DigestKind::ProviderOutput, "polint.solver", &["b"]),
        )
        .output_digest;

        assert_ne!(base, changed);
    }

    fn db_with_solver_edge() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let interner = db.stable_key_interner();
        let file = db.add_file(
            "app.ts".into(),
            "app.ts".to_string(),
            "callee();\n".to_string(),
        );
        db.push_function(ts_function(
            FunctionId(0),
            file,
            "caller",
            vec!["callee".to_string()],
        ));
        db.push_function(ts_function(FunctionId(1), file, "callee", Vec::new()));
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
                stable_key: interner.intern("call-site:callee"),
            }],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("valid calls");
        db.replace_semantic_graph_facts(SemanticGraphOutput {
            nodes: vec![
                semantic_node(
                    &db,
                    SemanticNodeId(0),
                    NodeKind::Function(FunctionId(0)),
                    "node:function:caller",
                ),
                semantic_node(
                    &db,
                    SemanticNodeId(1),
                    NodeKind::Function(FunctionId(1)),
                    "node:function:callee",
                ),
            ],
            edges: Vec::new(),
            constraints: Vec::new(),
        })
        .expect("valid semantic graph");

        let caller_node = function_node(&db, FunctionId(0));
        let callee_node = function_node(&db, FunctionId(1));
        let provenance = DerivedEdgeProvenance::new(
            &interner,
            vec![
                ContributingFact {
                    stable_key: interner.intern("call-site:callee"),
                },
                ContributingFact {
                    stable_key: interner.intern("constraint:call"),
                },
            ],
            &ConstraintKind::CallConstraint {
                callsite: SemanticNodeId(99),
            },
            1,
        );
        db.replace_solver_facts(SolverOutput {
            derived_edges: vec![DerivedEdgeFact {
                id: DerivedEdgeId(0),
                source: caller_node,
                target: callee_node,
                status: PointsToStatus::Present,
                precision: PointsToPrecision::FlowInsensitive,
                stable_key: interner.intern("solver-edge:caller-callee"),
                provenance,
            }],
            budget_status: BudgetStatus::WithinBudget,
            ..SolverOutput::default()
        })
        .expect("valid solver facts");
        db
    }

    fn db_with_solver_edge_referenced_by_semantic_constraint_key() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            "main.go".into(),
            "main.go".to_string(),
            "package main\nfunc caller(){ callee() }\nfunc callee() {}\n".to_string(),
        );
        db.push_function(go_function(
            FunctionId(0),
            file,
            "caller",
            vec!["callee".to_string()],
        ));
        db.push_function(go_function(FunctionId(1), file, "callee", Vec::new()));
        db.replace_call_facts(CallOutput {
            sites: vec![CallSiteFact {
                in_throw: false,
                id: CallSiteId(0),
                language: Language::Go,
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
                stable_key: db.stable_key_interner().intern("call-site:go-callee"),
            }],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("valid calls");
        db.replace_semantic_graph_facts(SemanticGraphOutput {
            nodes: vec![
                semantic_node(
                    &db,
                    SemanticNodeId(0),
                    NodeKind::Function(FunctionId(0)),
                    "node:function:caller",
                ),
                semantic_node(
                    &db,
                    SemanticNodeId(1),
                    NodeKind::Function(FunctionId(1)),
                    "node:function:callee",
                ),
                semantic_node(
                    &db,
                    SemanticNodeId(2),
                    NodeKind::Callsite(CallSiteId(0)),
                    "node:callsite:callee",
                ),
            ],
            edges: Vec::new(),
            constraints: vec![ConstraintFact {
                id: SemanticConstraintId(0),
                kind: ConstraintKind::CallConstraint {
                    callsite: SemanticNodeId(2),
                },
                status: PointsToStatus::Present,
                precision: PointsToPrecision::FlowInsensitive,
                stable_key: db
                    .stable_key_interner()
                    .intern("constraint:go-semantic-callsite"),
            }],
        })
        .expect("valid semantic graph");

        let caller_node = function_node(&db, FunctionId(0));
        let callee_node = function_node(&db, FunctionId(1));
        let callsite_node = callsite_node(&db, CallSiteId(0));
        let interner = db.stable_key_interner();
        let provenance = DerivedEdgeProvenance::new(
            &interner,
            vec![ContributingFact {
                stable_key: interner.intern("constraint:go-semantic-callsite"),
            }],
            &ConstraintKind::CallConstraint {
                callsite: callsite_node,
            },
            1,
        );
        db.replace_solver_facts(SolverOutput {
            derived_edges: vec![DerivedEdgeFact {
                id: DerivedEdgeId(0),
                source: caller_node,
                target: callee_node,
                status: PointsToStatus::Present,
                precision: PointsToPrecision::FlowInsensitive,
                stable_key: interner.intern("solver-edge:go-caller-callee"),
                provenance,
            }],
            budget_status: BudgetStatus::WithinBudget,
            ..SolverOutput::default()
        })
        .expect("valid solver facts");
        db
    }

    fn db_with_solver_edge_referenced_by_go_semantic_callsite_key() -> AnalysisDb {
        let mut db = db_with_solver_edge_referenced_by_semantic_constraint_key();
        let interner = db.stable_key_interner();
        let file = db
            .files()
            .iter()
            .find(|file| file.relative_path == "main.go")
            .map(|file| file.id)
            .expect("main.go file");
        db.replace_go_semantic_facts(GoSemanticFactsOutput {
            functions: vec![
                go_semantic_function(
                    &interner,
                    "go-function:caller",
                    "pkg.caller",
                    file,
                    FunctionId(0),
                ),
                go_semantic_function(
                    &interner,
                    "go-function:callee",
                    "pkg.callee",
                    file,
                    FunctionId(1),
                ),
            ],
            callsites: vec![GoSemanticCallsiteFact {
                id: crate::go::semantic::facts::GoSemanticCallsiteId(0),
                stable_key: interner.intern("go-semantic-callsite:caller-callee"),
                package_id: "pkg".to_string(),
                package_path: "pkg".to_string(),
                caller: "pkg.caller".to_string(),
                static_callee: None,
                status: GoSemanticCallStatus::UnresolvedDynamic,
                reason: None,
                relative_file: Some("main.go".to_string()),
                file: Some(file),
                span: Some(span()),
            }],
            ..GoSemanticFactsOutput::default()
        })
        .expect("valid go semantic facts");

        let caller_node = function_node(&db, FunctionId(0));
        let callee_node = function_node(&db, FunctionId(1));
        let callsite_node = callsite_node(&db, CallSiteId(0));
        let provenance = DerivedEdgeProvenance::new(
            &interner,
            vec![ContributingFact {
                stable_key: interner.intern("go-semantic-callsite:caller-callee"),
            }],
            &ConstraintKind::CallConstraint {
                callsite: callsite_node,
            },
            1,
        );
        db.replace_solver_facts(SolverOutput {
            derived_edges: vec![DerivedEdgeFact {
                id: DerivedEdgeId(0),
                source: caller_node,
                target: callee_node,
                status: PointsToStatus::Present,
                precision: PointsToPrecision::FlowInsensitive,
                stable_key: interner.intern("solver-edge:go-semantic-caller-callee"),
                provenance,
            }],
            budget_status: BudgetStatus::WithinBudget,
            ..SolverOutput::default()
        })
        .expect("valid solver facts");
        db
    }

    fn db_with_zero_width_go_method_caller_solver_edge() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            "main.go".into(),
            "main.go".to_string(),
            "package main\ntype Handler struct{}\nfunc (Handler) Handle(){ speaker.Speak() }\nfunc other(){ speaker.Speak() }\nfunc Speak() {}\n".to_string(),
        );
        let caller_span = span_for_file(file, 20, 80);
        let decoy_span = span_for_file(file, 90, 120);
        let callee_span = span_for_file(file, 130, 145);
        let call_span = span_for_file(file, 60, 75);
        let interner = db.stable_key_interner();
        db.push_function(go_function_with_span(
            FunctionId(0),
            file,
            "Handler.Handle",
            caller_span,
            vec!["Speak".to_string()],
        ));
        db.push_function(go_function_with_span(
            FunctionId(1),
            file,
            "Speak",
            callee_span.clone(),
            Vec::new(),
        ));
        db.push_function(go_function_with_span(
            FunctionId(2),
            file,
            "other",
            decoy_span,
            vec!["Speak".to_string()],
        ));
        db.replace_call_facts(CallOutput {
            sites: vec![
                CallSiteFact {
                    in_throw: false,
                    id: CallSiteId(0),
                    language: Language::Go,
                    file,
                    caller: FunctionId(0),
                    owner_symbol: None,
                    body: MirBodyId(0),
                    operation: MirOpId(0),
                    span: call_span.clone(),
                    kind: CallSyntaxKind::Method,
                    callee: CallCallee::Identifier {
                        reference: None,
                        name: "Speak".to_string(),
                    },
                    receiver: None,
                    arguments: Vec::new(),
                    result: None,
                    status: CallTargetStatus::Unresolved,
                    precision: CallPrecision::Conservative,
                    stable_key: interner.intern("call-site:handler-speak"),
                },
                CallSiteFact {
                    in_throw: false,
                    id: CallSiteId(1),
                    language: Language::Go,
                    file,
                    caller: FunctionId(2),
                    owner_symbol: None,
                    body: MirBodyId(1),
                    operation: MirOpId(0),
                    span: call_span.clone(),
                    kind: CallSyntaxKind::Method,
                    callee: CallCallee::Identifier {
                        reference: None,
                        name: "Speak".to_string(),
                    },
                    receiver: None,
                    arguments: Vec::new(),
                    result: None,
                    status: CallTargetStatus::Unresolved,
                    precision: CallPrecision::Conservative,
                    stable_key: interner.intern("call-site:other-speak"),
                },
            ],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("valid calls");
        db.replace_semantic_graph_facts(SemanticGraphOutput {
            nodes: vec![
                semantic_node(
                    &db,
                    SemanticNodeId(0),
                    NodeKind::Function(FunctionId(0)),
                    "node:function:handler-handle",
                ),
                semantic_node(
                    &db,
                    SemanticNodeId(1),
                    NodeKind::Function(FunctionId(1)),
                    "node:function:speak",
                ),
            ],
            edges: Vec::new(),
            constraints: Vec::new(),
        })
        .expect("valid semantic graph");
        db.replace_go_semantic_facts(GoSemanticFactsOutput {
            functions: vec![
                go_semantic_method_function_with_span(
                    &interner,
                    "go-function:handler-handle",
                    "Handler.Handle",
                    "pkg.Handler.Handle",
                    file,
                    FunctionId(0),
                    span_for_file(file, 25, 25),
                ),
                go_semantic_function_with_span(
                    &interner,
                    "go-function:speak",
                    "Speak",
                    "pkg.Speak",
                    file,
                    FunctionId(1),
                    callee_span,
                ),
            ],
            callsites: vec![GoSemanticCallsiteFact {
                id: crate::go::semantic::facts::GoSemanticCallsiteId(0),
                stable_key: interner.intern("go-semantic-callsite:method-dispatch"),
                package_id: "pkg".to_string(),
                package_path: "pkg".to_string(),
                caller: "pkg.Handler.Handle".to_string(),
                static_callee: None,
                status: GoSemanticCallStatus::UnresolvedDynamic,
                reason: None,
                relative_file: Some("main.go".to_string()),
                file: Some(file),
                span: Some(call_span),
            }],
            ..GoSemanticFactsOutput::default()
        })
        .expect("valid go semantic facts");

        let caller_node = function_node(&db, FunctionId(0));
        let callee_node = function_node(&db, FunctionId(1));
        let provenance = DerivedEdgeProvenance::new(
            &interner,
            vec![ContributingFact {
                stable_key: interner.intern("go-semantic-callsite:method-dispatch"),
            }],
            &ConstraintKind::CallConstraint {
                callsite: SemanticNodeId(99),
            },
            1,
        );
        db.replace_solver_facts(SolverOutput {
            derived_edges: vec![DerivedEdgeFact {
                id: DerivedEdgeId(0),
                source: caller_node,
                target: callee_node,
                status: PointsToStatus::Present,
                precision: PointsToPrecision::FlowInsensitive,
                stable_key: interner.intern("solver-edge:method-dispatch"),
                provenance,
            }],
            budget_status: BudgetStatus::WithinBudget,
            ..SolverOutput::default()
        })
        .expect("valid solver facts");
        db
    }

    fn ts_function(id: FunctionId, file: FileId, name: &str, calls: Vec<String>) -> FunctionFact {
        function(id, file, name, Language::TypeScript, calls)
    }

    fn go_function(id: FunctionId, file: FileId, name: &str, calls: Vec<String>) -> FunctionFact {
        function(id, file, name, Language::Go, calls)
    }

    fn go_function_with_span(
        id: FunctionId,
        file: FileId,
        name: &str,
        span: Span,
        calls: Vec<String>,
    ) -> FunctionFact {
        function_with_span(id, file, name, Language::Go, span, calls)
    }

    fn function(
        id: FunctionId,
        file: FileId,
        name: &str,
        language: Language,
        calls: Vec<String>,
    ) -> FunctionFact {
        function_with_span(id, file, name, language, span(), calls)
    }

    fn function_with_span(
        id: FunctionId,
        file: FileId,
        name: &str,
        language: Language,
        span: Span,
        calls: Vec<String>,
    ) -> FunctionFact {
        FunctionFact {
            id,
            file,
            name: name.to_string(),
            span,
            language,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls,
        }
    }

    fn go_semantic_function(
        interner: &crate::core::StableKeyInterner,
        stable_key: &str,
        qualified: &str,
        file: FileId,
        function: FunctionId,
    ) -> GoSemanticFunctionFact {
        go_semantic_function_with_span(
            interner,
            stable_key,
            qualified.rsplit('.').next().unwrap_or(qualified),
            qualified,
            file,
            function,
            span(),
        )
    }

    fn go_semantic_function_with_span(
        interner: &crate::core::StableKeyInterner,
        stable_key: &str,
        name: &str,
        qualified: &str,
        file: FileId,
        function: FunctionId,
        span: Span,
    ) -> GoSemanticFunctionFact {
        GoSemanticFunctionFact {
            id: GoSemanticFunctionId(function.0),
            stable_key: interner.intern(stable_key),
            package_id: "pkg".to_string(),
            package_path: "pkg".to_string(),
            name: name.to_string(),
            qualified: qualified.to_string(),
            signature: "func()".to_string(),
            kind: GoSemanticFunctionKind::Function,
            receiver: None,
            relative_file: Some("main.go".to_string()),
            file: Some(file),
            span: Some(span),
        }
    }

    fn go_semantic_method_function_with_span(
        interner: &crate::core::StableKeyInterner,
        stable_key: &str,
        name: &str,
        qualified: &str,
        file: FileId,
        function: FunctionId,
        span: Span,
    ) -> GoSemanticFunctionFact {
        GoSemanticFunctionFact {
            kind: GoSemanticFunctionKind::Method,
            receiver: Some("pkg.Handler".to_string()),
            ..go_semantic_function_with_span(
                interner, stable_key, name, qualified, file, function, span,
            )
        }
    }

    fn semantic_node(
        db: &AnalysisDb,
        id: SemanticNodeId,
        kind: NodeKind,
        stable_key: &str,
    ) -> SemanticNodeFact {
        SemanticNodeFact {
            id,
            kind,
            precision: SemanticPrecision::SetupAware,
            stable_key: db.stable_key_interner().intern(stable_key),
        }
    }

    fn function_node(db: &AnalysisDb, function: FunctionId) -> SemanticNodeId {
        db.semantic_nodes()
            .iter()
            .find_map(|node| {
                let NodeKind::Function(candidate) = node.kind else {
                    return None;
                };
                (candidate == function).then_some(node.id)
            })
            .expect("function node")
    }

    fn callsite_node(db: &AnalysisDb, callsite: CallSiteId) -> SemanticNodeId {
        db.semantic_nodes()
            .iter()
            .find_map(|node| {
                let NodeKind::Callsite(candidate) = node.kind else {
                    return None;
                };
                (candidate == callsite).then_some(node.id)
            })
            .expect("callsite node")
    }

    fn manifest() -> &'static ProviderManifest {
        AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == REFINED_CALLS_PROVIDER_ID)
            .expect("refined calls manifest")
    }

    fn snapshot(db: &AnalysisDb) -> InputSnapshot {
        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        InputSnapshot::from_run_inputs(
            &loaded,
            db,
            "config-a",
            "rules-a",
            AnalysisPlan::empty().digest(),
            AnalysisKernel::provider_manifests(),
        )
    }

    fn absent(provider: &str) -> Digest {
        Digest::absent(DigestKind::ProviderOutput, provider)
    }

    fn span() -> Span {
        Span::point(FileId(0), 1, 1)
    }

    fn span_for_file(file: FileId, start_byte: u32, end_byte: u32) -> Span {
        Span {
            file,
            start_byte,
            end_byte,
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 1,
        }
    }
}

fn provider_error_diagnostic(message: String) -> Diagnostic {
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        crate::diagnostics::TextRange::point(1, 1),
        format!("Refined call provider failed: {message}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::calls::facts::{
        CallAlgorithm, CallEdgeKind, CallPrecision, CallProvenance, CallTargetFact,
        CallTargetStatus,
    };
    use crate::analysis::ids::{CallSiteId, CallTargetId};
    use crate::core::{FunctionId, SymbolId};

    #[test]
    fn refined_key_is_stable_for_base_target_and_tier() {
        let db = crate::core::AnalysisDb::new();
        let interner = db.stable_key_interner();
        let target = CallTargetFact {
            id: CallTargetId(7),
            site: CallSiteId(3),
            caller: FunctionId(1),
            target_function: Some(FunctionId(2)),
            target_symbol: Some(SymbolId(4)),
            edge_kind: CallEdgeKind::Direct,
            algorithm: CallAlgorithm::DirectReference,
            status: CallTargetStatus::Resolved,
            reason: None,
            provenance: CallProvenance::Native,
            precision: CallPrecision::SetupAware,
            stable_key: interner.intern("call-target:stable".to_string()),
        };

        assert_eq!(
            stable_refined_call_key(
                &interner,
                &target,
                RefinedCallTier::DirectOnly,
                "call-target:stable"
            ),
            stable_refined_call_key(
                &interner,
                &target,
                RefinedCallTier::DirectOnly,
                "call-target:stable"
            )
        );
    }

    #[test]
    fn finalized_output_reassigns_dense_ids_after_sorting() {
        let interner = crate::core::test_stable_key_interner();
        let output = finalized_output(
            &interner,
            RefinedCallOutput {
                edges: vec![refined_edge("z", 10), refined_edge("a", 10)],
            },
        );

        assert_eq!(
            output
                .edges
                .iter()
                .map(|edge| (edge.id.0, interner.resolve(edge.stable_key).to_string()))
                .collect::<Vec<_>>(),
            vec![(0, "a".to_string()), (1, "z".to_string())]
        );
    }

    fn refined_edge(stable_key: &str, id: u64) -> RefinedCallEdgeFact {
        RefinedCallEdgeFact {
            id: crate::analysis::ids::RefinedCallEdgeId(id),
            site: CallSiteId(0),
            base_target: None,
            caller: FunctionId(0),
            target_function: None,
            target_symbol: None,
            synthetic_target: None,
            language: crate::core::Language::TypeScript,
            edge_kind: CallEdgeKind::Synthetic,
            algorithm: CallAlgorithm::FrameworkModel,
            tier: RefinedCallTier::DirectPlusFramework,
            status: CallTargetStatus::Resolved,
            reason: None,
            provenance: CallProvenance::Model,
            precision: CallPrecision::Heuristic,
            validation: RefinedCallValidation::Native,
            confidence: RefinedCallConfidence::Medium,
            evidence: Vec::new(),
            input_stable_keys: Vec::new(),
            stable_key: crate::core::stable_key_for_test(stable_key),
        }
    }
}

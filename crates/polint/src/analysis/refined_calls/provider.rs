use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt::Debug;

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
use crate::analysis::semantic_graph::facts::NodeKind;
use crate::analysis::solver::facts::DerivedEdgeFact;
use crate::analysis_kernel::incremental::{
    CacheStats, Digest, DigestKind, InputComponent, InputSnapshot,
};
use crate::analysis_kernel::{FactFamily, FactRef, ProviderManifest};
use crate::core::AnalysisDb;
use crate::diagnostics::Diagnostic;

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
    debug_assert_eq!(manifest.id, REFINED_CALLS_PROVIDER_ID);
    let mut output = RefinedCallOutput::empty();
    for target in db.call_targets() {
        output.edges.push(refined_edge_from_base_target(
            db,
            target,
            crate::analysis::ids::RefinedCallEdgeId(0),
            RefinedCallTier::DirectOnly,
        ));
    }
    output.edges.extend(derive_solver_refinements(db).edges);
    output = finalized_output(output);

    let output_digest = refined_calls_output_digest(
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

    match db.replace_refined_call_facts(output) {
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

fn finalized_output(mut output: RefinedCallOutput) -> RefinedCallOutput {
    output = output.normalized();
    for (index, edge) in output.edges.iter_mut().enumerate() {
        edge.id = crate::analysis::ids::RefinedCallEdgeId(index as u64);
    }
    output
}

fn refined_edge_from_base_target(
    db: &AnalysisDb,
    target: &CallTargetFact,
    id: crate::analysis::ids::RefinedCallEdgeId,
    tier: RefinedCallTier,
) -> RefinedCallEdgeFact {
    RefinedCallEdgeFact {
        id,
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
        algorithm: target.algorithm,
        tier,
        status: target.status,
        reason: target.reason,
        provenance: target.provenance,
        precision: target.precision,
        validation: validation_for_target(target),
        confidence: confidence_for_target(target),
        evidence: vec!["base_call_target".to_string()],
        input_stable_keys: vec![
            db.metadata_for(FactRef::new(FactFamily::CallTarget, target.id.0))
                .map(|metadata| metadata.stable_key.clone())
                .unwrap_or_else(|| target.stable_key.clone()),
        ],
        stable_key: stable_refined_call_key(db, target, tier),
    }
}

fn derive_solver_refinements(db: &AnalysisDb) -> RefinedCallOutput {
    let index = SolverProjectionIndex::new(db);
    let mut output = RefinedCallOutput::empty();
    output.edges.extend(
        db.solver_derived_edges()
            .iter()
            .filter_map(|edge| refined_edge_from_solver_edge(&index, edge)),
    );
    output
}

fn refined_edge_from_solver_edge(
    index: &SolverProjectionIndex<'_>,
    edge: &DerivedEdgeFact,
) -> Option<RefinedCallEdgeFact> {
    if edge.provenance.constraint_kind != "call_constraint" {
        return None;
    }
    let caller = index.function_by_node.get(&edge.source).copied()?;
    let target_function = index.function_by_node.get(&edge.target).copied()?;
    let site = index.callsite_for_solver_edge(edge)?;
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
        input_stable_keys: solver_input_stable_keys(edge),
        stable_key: stable_refined_call_key_from_solver_edge(site, edge),
    })
}

struct SolverProjectionIndex<'a> {
    function_by_node: BTreeMap<crate::analysis::ids::SemanticNodeId, crate::core::FunctionId>,
    callsite_by_stable_key: BTreeMap<&'a str, &'a CallSiteFact>,
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
        let callsite_by_stable_key = db
            .call_sites()
            .iter()
            .map(|site| (site.stable_key.as_str(), site))
            .collect();
        Self {
            function_by_node,
            callsite_by_stable_key,
        }
    }

    fn callsite_for_solver_edge(&self, edge: &DerivedEdgeFact) -> Option<&'a CallSiteFact> {
        edge.provenance.contributing_facts.iter().find_map(|fact| {
            self.callsite_by_stable_key
                .get(fact.stable_key.as_str())
                .copied()
        })
    }
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

fn solver_input_stable_keys(edge: &DerivedEdgeFact) -> Vec<String> {
    let mut keys = vec![edge.stable_key.clone()];
    keys.extend(
        edge.provenance
            .contributing_facts
            .iter()
            .map(|fact| fact.stable_key.clone()),
    );
    keys
}

fn stable_refined_call_key_from_solver_edge(site: &CallSiteFact, edge: &DerivedEdgeFact) -> String {
    crate::analysis_kernel::stable_key_from_parts(
        FactFamily::RefinedCallEdge,
        &[
            ("tier", format!("{:?}", RefinedCallTier::PointsToAssisted)),
            ("solver_edge", edge.stable_key.clone()),
            ("site", site.stable_key.clone()),
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
    db: &AnalysisDb,
    target: &CallTargetFact,
    tier: RefinedCallTier,
) -> String {
    let base_key = db
        .metadata_for(FactRef::new(FactFamily::CallTarget, target.id.0))
        .map(|metadata| metadata.stable_key.clone())
        .unwrap_or_else(|| target.stable_key.clone());
    crate::analysis_kernel::stable_key_from_parts(
        FactFamily::RefinedCallEdge,
        &[
            ("tier", format!("{tier:?}")),
            ("base_target", base_key),
            ("site", target.site.0.to_string()),
        ],
    )
}

#[allow(clippy::too_many_arguments)]
fn refined_calls_output_digest(
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
    parts.extend(
        output
            .edges
            .iter()
            .map(|edge| format!("refined_call_edge={}", stable_fact_payload(edge))),
    );
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

fn stable_fact_payload<T>(fact: &T) -> String
where
    T: Serialize + Debug,
{
    serde_json::to_string(fact).unwrap_or_else(|_| format!("{fact:?}"))
}

#[cfg(test)]
mod solver_projection_tests {
    use super::*;
    use crate::analysis::calls::facts::CallCallee;
    use crate::analysis::calls::store::CallOutput;
    use crate::analysis::ids::{CallSiteId, DerivedEdgeId, MirBodyId, MirOpId, SemanticNodeId};
    use crate::analysis::semantic_graph::constraints::ConstraintKind;
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
        let file = db.add_file(
            "app.ts".into(),
            "app.ts".to_string(),
            "callee();\n".to_string(),
        );
        db.push_function(function(
            FunctionId(0),
            file,
            "caller",
            vec!["callee".to_string()],
        ));
        db.push_function(function(FunctionId(1), file, "callee", Vec::new()));
        db.replace_call_facts(CallOutput {
            sites: vec![CallSiteFact {
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
        db.replace_semantic_graph_facts(SemanticGraphOutput {
            nodes: vec![
                semantic_node(
                    SemanticNodeId(0),
                    NodeKind::Function(FunctionId(0)),
                    "node:function:caller",
                ),
                semantic_node(
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
            vec![
                ContributingFact {
                    stable_key: "call-site:callee".to_string(),
                },
                ContributingFact {
                    stable_key: "constraint:call".to_string(),
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
                stable_key: "solver-edge:caller-callee".to_string(),
                provenance,
            }],
            budget_status: BudgetStatus::WithinBudget,
        })
        .expect("valid solver facts");
        db
    }

    fn function(id: FunctionId, file: FileId, name: &str, calls: Vec<String>) -> FunctionFact {
        FunctionFact {
            id,
            file,
            name: name.to_string(),
            span: span(),
            language: Language::TypeScript,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls,
        }
    }

    fn semantic_node(id: SemanticNodeId, kind: NodeKind, stable_key: &str) -> SemanticNodeFact {
        SemanticNodeFact {
            id,
            kind,
            precision: SemanticPrecision::SetupAware,
            stable_key: stable_key.to_string(),
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
        let db = AnalysisDb::new();
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
            stable_key: "call-target:stable".to_string(),
        };

        assert_eq!(
            stable_refined_call_key(&db, &target, RefinedCallTier::DirectOnly),
            stable_refined_call_key(&db, &target, RefinedCallTier::DirectOnly)
        );
    }

    #[test]
    fn finalized_output_reassigns_dense_ids_after_sorting() {
        let output = finalized_output(RefinedCallOutput {
            edges: vec![refined_edge("z", 10), refined_edge("a", 10)],
        });

        assert_eq!(
            output
                .edges
                .iter()
                .map(|edge| (edge.id.0, edge.stable_key.as_str()))
                .collect::<Vec<_>>(),
            vec![(0, "a"), (1, "z")]
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
            stable_key: stable_key.to_string(),
        }
    }
}

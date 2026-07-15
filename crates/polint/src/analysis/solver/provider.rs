//! `polint.solver` provider entry point (D-13, D-15).
//!
//! Mirrors `analysis::semantic_graph::provider`: a `derive_*_with_cache_stats` entry
//! that drives the unified solver engine over the closed input snapshot (the stored
//! `polint.semantic_graph` constraints), emits derived edges + provenance, validates
//! via [`crate::analysis::solver::validate`], persists through
//! [`crate::core::AnalysisDb::replace_solver_facts`], and returns a
//! [`SolverProviderRunOutput`] carrying the output digest (`None` on store error so a
//! cache layer never records a hit for un-persisted state).
//!
//! The output digest (D-15) folds (a) provider/version/schema/parameter digests
//! (the parameter digest already embeds active [`SolverBudget`] knobs), (b) every consumed
//! upstream provider output digest — `polint.semantic_graph`, the points-to source
//! families produced by `polint.type_value_alias` (`points_to_constraints` /
//! `points_to_sets`), and `polint.go.semantic` (whose RTA-signal harvest families the
//! Go RTA policy reads to resolve dynamic dispatch — FIX 4), (c) the [`SolverBudget`]
//! explicitly, and (d) per-row stable keys, then `parts.sort()` + `Digest::from_parts`.
//! Any upstream change, algorithm bump, or active budget change deterministically
//! invalidates the solver cache.

use std::collections::BTreeSet;

use crate::analysis::ids::SemanticNodeId;
use crate::analysis::solver::budget::{BudgetStatus, SolverBudget};
use crate::analysis::solver::cache_key::{
    solver_budget_digest_parts, solver_provider_parameter_digest,
};
use crate::analysis::solver::engine::SolverEngine;
use crate::analysis::solver::go_rta::GoRtaInputs;
use crate::analysis::solver::policy::{
    GoRtaPolicy, SolverPolicy, TsObjectModelPolicy, TsTokensPolicy,
};
use crate::analysis::solver::store::{SOLVER_PROVIDER_ID, SolverOutput};
use crate::analysis::solver::ts_object_model::TsObjectModelInputs;
use crate::analysis::solver::ts_tokens::TsTokenInputs;
use crate::analysis::solver::validate::{detect_solver_summary_cycle, validate_derived_edges};
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{CacheStats, Digest, DigestKind, InputSnapshot};
use crate::core::AnalysisDb;
use crate::diagnostics::{Diagnostic, TextRange};

#[derive(Debug, Clone, Default)]
pub(crate) struct SolverProviderRunOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) execution: crate::analysis_kernel::incremental::ProviderExecutionOutcome,
    pub(crate) output_digest: Option<Digest>,
}

/// `polint.solver` provider entry point (D-13).
///
/// Pipeline mirroring `polint.semantic_graph`:
/// 1. read the closed input snapshot — the stored semantic-graph constraints — and
///    drive [`derive_edges`] over the unified `CopyEdge` vocabulary, bounded by the
///    [`SolverBudget`] (D-11),
/// 2. `normalized()` is applied inside `derive_edges` (stable-key sort),
/// 3. compute the output digest over the stored stable KEYS + upstream digests +
///    the budget (D-15), never run-local dense IDs,
/// 4. validate the derived edges (precision ceiling, dup keys, dangling endpoints)
///    AND run the D-12 cycle-detection check over the input constraints — both
///    surface as evidence-bearing diagnostics, never silent drops,
/// 5. `db.replace_solver_facts(...)` stores + referentially validates,
/// 6. on store error return `output_digest: None`.
pub(crate) fn derive_solver_with_cache_stats(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    budget: SolverBudget,
    semantic_graph_output_digest: Digest,
    type_value_alias_output_digest: Digest,
    go_semantic_output_digest: Digest,
) -> SolverProviderRunOutput {
    debug_assert_eq!(manifest.id, SOLVER_PROVIDER_ID);

    // Step: drive the unified solver ENGINE over the closed input snapshot (D-02,
    // the reserved seam). The points-to CopyEdge closure (`derive_edges`, byte-
    // identical) AND the Go RTA policy's resolved call edges converge into one
    // SolverOutput under one SolverBudget. The build is read-only; the engine
    // normalizes the merged output.
    //
    // FINDING 3: the production engine does NOT register `PointsToPolicy`. The points-to
    // edges that enter this output are the step-1 `derive_edges` CopyEdge closure inside
    // `run_to_solver_output` (over the semantic-graph `CopyEdge` constraints); the
    // Andersen points-to solve runs in the `type_value_alias` provider and is consumed
    // here only via its output digest. Registering `PointsToPolicy` here re-ran a full
    // Andersen solve whose edges were discarded, yet its object/dynamic-var budget
    // exhaustion polluted the run-level budget_status (a misleading diagnostic + a
    // spurious solver_output_digest bust, WR-06) — a redundant double-solve with a
    // harmful side effect. Register only the edge-contributing policies.
    let constraints = db.semantic_constraints().to_vec();
    let engine = SolverEngine::new(solver_policies_for_db(db, &budget), budget);
    let output = engine.run_to_solver_output(&constraints);

    // Step: digest over the stored stable KEYS + upstream digests + the budget.
    let output_digest = solver_output_digest(
        manifest,
        input_snapshot,
        &budget,
        &semantic_graph_output_digest,
        &type_value_alias_output_digest,
        &go_semantic_output_digest,
        &output,
    );

    // Step: validate the derived edges + the D-12 cycle-detection check. These are
    // surfaced as diagnostics; the store performs the hard referential rejection.
    let mut diagnostics = Vec::new();
    let node_ids: BTreeSet<SemanticNodeId> =
        db.semantic_nodes().iter().map(|node| node.id).collect();
    validate_derived_edges(&output.derived_edges, &node_ids, &mut diagnostics);
    detect_solver_summary_cycle(&constraints, &mut diagnostics);
    // Surface budget exhaustion as an honest diagnostic (D-06): when the solver
    // truncated any source's closure under the per-source step budget, the run is
    // flagged rather than presenting a silently-truncated edge set as complete.
    if output.budget_status == BudgetStatus::BudgetExceeded {
        diagnostics.push(budget_exceeded_diagnostic(&output.budget_reasons));
    }

    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();

    // Step: store (assigns dense IDs + referentially validates inside
    // from_output). On store error the db keeps its prior state and the facts the
    // digest certifies were not persisted, so return output_digest: None.
    match db.replace_solver_facts(output) {
        Ok(()) => SolverProviderRunOutput {
            diagnostics,
            cache_stats,
            execution: crate::analysis_kernel::incremental::ProviderExecutionOutcome::Succeeded,
            output_digest: Some(output_digest),
        },
        Err(error) => {
            diagnostics.push(provider_error_diagnostic(error.to_string()));
            SolverProviderRunOutput {
                diagnostics,
                cache_stats,
                execution: crate::analysis_kernel::incremental::ProviderExecutionOutcome::Failed,
                output_digest: None,
            }
        }
    }
}

fn solver_policies_for_db(db: &AnalysisDb, budget: &SolverBudget) -> Vec<Box<dyn SolverPolicy>> {
    let mut policies: Vec<Box<dyn SolverPolicy>> = vec![
        // The real Go RTA policy (GO-05): contributes resolved call edges.
        Box::new(GoRtaPolicy::new(GoRtaInputs::from_db(db))),
        // The TS token policy owns a closed JS/TS snapshot (JS-04).
        Box::new(TsTokensPolicy::new(TsTokenInputs::from_db(db))),
    ];
    register_ts_object_model_policy_if_enabled(&mut policies, db, budget);
    policies
}

fn register_ts_object_model_policy_if_enabled(
    policies: &mut Vec<Box<dyn SolverPolicy>>,
    db: &AnalysisDb,
    budget: &SolverBudget,
) -> bool {
    if !budget.object_model_enabled {
        return false;
    }

    policies.push(Box::new(TsObjectModelPolicy::new(
        TsObjectModelInputs::from_db(db),
    )));
    true
}

/// Output digest over stable KEYS + upstream digests + budget (D-15), never dense
/// IDs.
///
/// Folds (a) provider/version/schema/parameter digests (the parameter digest already
/// embeds the budget via `solver_provider_parameter_digest`), (b) the consumed
/// upstream provider output digests — `polint.semantic_graph`, the points-to source
/// families carried by `polint.type_value_alias`'s output digest, and
/// `polint.go.semantic`'s output digest (the RTA-signal families the Go RTA policy
/// reads — FIX 4), (c) the `SolverBudget` explicitly (belt-and-suspenders with the
/// parameter digest so a budget change is unmissable, D-15), and (d) per-row stable
/// keys + status/precision + provenance fragment, then `parts.sort()` +
/// `Digest::from_parts`.
fn solver_output_digest(
    manifest: &ProviderManifest,
    _input_snapshot: &InputSnapshot,
    budget: &SolverBudget,
    semantic_graph_output_digest: &Digest,
    type_value_alias_output_digest: &Digest,
    go_semantic_output_digest: &Digest,
    output: &SolverOutput,
) -> Digest {
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!("parameters={}", solver_provider_parameter_digest(budget)),
        format!("semantic_graph_output={semantic_graph_output_digest}"),
        format!("type_value_alias_output={type_value_alias_output_digest}"),
        // The Go RTA policy reads the stored `polint.go.semantic` RTA-signal families
        // (instantiated_types / address_taken / dynamic_dispatch / method_sets) to resolve
        // dynamic dispatch, so a Go edit touching ONLY those families changes the resolved
        // edges and MUST invalidate the solver cache (FIX 4). Fold the go.semantic output
        // digest like the other consumed upstream digests.
        format!("go_semantic_output={go_semantic_output_digest}"),
        // Run-level budget status (WR-06): two runs over the same inputs that produce
        // the same SURVIVING edge set but differ in whether the budget was exhausted
        // (WithinBudget vs BudgetExceeded) must NOT share an output digest. A fixpoint
        // that aborted BEFORE reaching an edge leaves no per-edge trace, so the
        // run-level status is the only carrier of the truncation signal — fold it in so
        // a cache hit can never serve a truncated result under a complete run's digest.
        format!("budget_status={}", output.budget_status.as_str()),
    ];
    parts.extend(solver_budget_digest_parts(budget));

    parts.extend(output.derived_edges.iter().map(|edge| {
        format!(
            "edge={}|status={:?}|prec={:?}|prov={}",
            edge.stable_key,
            edge.status,
            edge.precision,
            edge.provenance.stable_key_fragment(),
        )
    }));
    if output.derived_edges.is_empty() {
        parts.push("solver_output=empty".to_string());
    }
    parts.extend(
        output
            .budget_reasons
            .iter()
            .map(|reason| format!("budget_exceeded_reason={reason}")),
    );

    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "solver_output", &refs)
}

/// Honest budget-exhaustion signal (D-06): emitted when the solver truncated a
/// source's transitive closure under the per-source step budget. The downstream
/// unknown taxonomy categorizes it; surviving derived edges keep their own honest
/// status and precision.
fn budget_exceeded_diagnostic(budget_reasons: &BTreeSet<String>) -> Diagnostic {
    let mut diagnostic = Diagnostic::warning(
        "polint/internal",
        "<workspace>",
        TextRange::point(1, 1),
        "Solver budget exceeded; some potential derived edges may be missing, while \
         surviving derived edges keep their own status and precision.",
    )
    .with_evidence("provider", SOLVER_PROVIDER_ID)
    .with_evidence("budget_status", BudgetStatus::BudgetExceeded.as_str());
    for reason in budget_reasons {
        diagnostic = diagnostic.with_evidence("budget_reason", reason.clone());
    }
    diagnostic
}

fn provider_error_diagnostic(message: String) -> Diagnostic {
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        TextRange::point(1, 1),
        "Solver analysis failed; solver-derived edges were not stored.",
    )
    .with_evidence("provider", SOLVER_PROVIDER_ID)
    .with_evidence("reason", message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::adaptation::budget::AdaptationModelBudget;
    use crate::analysis::ids::SemanticConstraintId;
    use crate::analysis::points_to::facts::{PointsToPrecision, PointsToStatus};
    use crate::analysis::semantic_graph::constraints::{ConstraintFact, ConstraintKind};
    use crate::analysis::semantic_graph::facts::{NodeKind, SemanticNodeFact, SemanticPrecision};
    use crate::analysis::semantic_graph::provider::derive_semantic_graph_with_cache_stats;
    use crate::analysis::semantic_graph::store::SEMANTIC_GRAPH_PROVIDER_ID;
    use crate::analysis::semantic_graph::store::SemanticGraphOutput;
    use crate::analysis::solver::budget::{BudgetReason, JsObjectModelSubBudget, SolverBudget};
    use crate::analysis_kernel::AnalysisKernel;
    use crate::analysis_kernel::incremental::{Digest, DigestKind};
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::{LoadedConfig, load_config};
    use crate::core::{AnalysisDb, FunctionId};
    use std::fs;
    use tempfile::tempdir;

    fn manifest() -> &'static ProviderManifest {
        AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == SOLVER_PROVIDER_ID)
            .expect("solver manifest")
    }

    fn semantic_graph_manifest() -> &'static ProviderManifest {
        AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == SEMANTIC_GRAPH_PROVIDER_ID)
            .expect("semantic graph manifest")
    }

    fn snapshot(db: &AnalysisDb) -> InputSnapshot {
        snapshot_with_config(db, "")
    }

    fn snapshot_with_config(db: &AnalysisDb, config: &str) -> InputSnapshot {
        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join(".polint.toml"), config).expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        snapshot_from_loaded(db, &loaded)
    }

    fn snapshot_from_loaded(db: &AnalysisDb, loaded: &LoadedConfig) -> InputSnapshot {
        let empty_plan = AnalysisPlan::empty();
        assert!(empty_plan.requested_capability_snapshots().is_empty());
        let identity_sources = InputSnapshot::identity_sources_from_plan(loaded, &empty_plan);
        assert!(identity_sources.requested_capabilities.is_empty());
        assert_eq!(
            identity_sources.analysis_requirements_identity,
            Digest::absent(DigestKind::AnalysisRequirements, "requested_capabilities")
        );

        InputSnapshot::from_run_inputs_with_plan(
            loaded,
            db,
            "config-a",
            "rules-a",
            &empty_plan,
            AnalysisKernel::provider_manifests(),
        )
    }

    fn solver_budget_from_loaded(loaded: &LoadedConfig) -> SolverBudget {
        SolverBudget {
            go: loaded.config.solver.to_go_sub_budget(),
            js: loaded.config.solver.to_js_sub_budget(),
            object_model_enabled: loaded.config.solver.js_object_model_enabled(),
            object: loaded.config.solver.to_js_object_sub_budget(),
            ..SolverBudget::default()
        }
    }

    fn semantic_graph_digest_for_loaded(
        db: &mut AnalysisDb,
        loaded: &LoadedConfig,
    ) -> Option<Digest> {
        let snapshot = snapshot_from_loaded(db, loaded);
        derive_semantic_graph_with_cache_stats(
            db,
            loaded,
            AdaptationModelBudget::default(),
            &snapshot,
            semantic_graph_manifest(),
            absent("polint.calls"),
            absent("polint.identity"),
            absent("polint.abstract_domains"),
            absent("polint.entrypoints"),
            absent("polint.type_value_alias"),
            absent("polint.symbol_graph"),
            absent("polint.module_topology"),
            absent("polint.go.syntax"),
            absent("polint.ts.syntax"),
            absent("polint.semantic_mir"),
            absent("polint.go.semantic"),
        )
        .output_digest
    }

    fn absent(kind: &str) -> Digest {
        Digest::absent(DigestKind::ProviderOutput, kind)
    }

    fn node(id: u64, stable_key: &str) -> SemanticNodeFact {
        SemanticNodeFact {
            id: SemanticNodeId(id),
            kind: NodeKind::Function(FunctionId(id)),
            precision: SemanticPrecision::Conservative,
            stable_key: stable_key.to_string(),
        }
    }

    fn copy(src: u64, dst: u64, stable_key: &str) -> ConstraintFact {
        ConstraintFact {
            id: SemanticConstraintId(0),
            kind: ConstraintKind::CopyEdge {
                dst: SemanticNodeId(dst),
                src: SemanticNodeId(src),
            },
            status: PointsToStatus::Present,
            precision: PointsToPrecision::FlowInsensitive,
            stable_key: stable_key.to_string(),
        }
    }

    /// Builds a db with a small semantic graph: a chain a -> b -> c so the solver
    /// derives the transitive edge a -> c.
    fn db_with_copy_chain() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let graph = SemanticGraphOutput {
            nodes: vec![
                node(0, "node|function|a"),
                node(1, "node|function|b"),
                node(2, "node|function|c"),
            ],
            edges: Vec::new(),
            constraints: vec![
                copy(0, 1, "constraint|copy|a-b"),
                copy(1, 2, "constraint|copy|b-c"),
            ],
        };
        db.replace_semantic_graph_facts(graph)
            .expect("semantic graph stores");
        db
    }

    fn run(db: &mut AnalysisDb) -> SolverProviderRunOutput {
        let snapshot = snapshot(db);
        derive_solver_with_cache_stats(
            db,
            &snapshot,
            manifest(),
            SolverBudget::default(),
            absent("polint.semantic_graph"),
            absent("polint.type_value_alias"),
            absent("polint.go.semantic"),
        )
    }

    #[test]
    fn empty_db_produces_empty_output_sentinel_digest() {
        let mut db = AnalysisDb::new();
        let output = run(&mut db);
        assert!(output.output_digest.is_some());
        assert!(output.diagnostics.is_empty());
        assert!(db.solver_derived_edges().is_empty());
    }

    #[test]
    fn populated_db_derives_transitive_edge_and_stores() {
        let mut db = db_with_copy_chain();
        let output = run(&mut db);
        assert!(output.output_digest.is_some());
        assert!(
            db.solver_derived_edges()
                .iter()
                .any(|e| e.source == SemanticNodeId(0) && e.target == SemanticNodeId(2)),
            "expected the transitive a -> c edge stored"
        );
    }

    #[test]
    fn semantic_graph_upstream_digest_change_invalidates_output_digest() {
        let mut db_a = db_with_copy_chain();
        let snapshot = snapshot(&db_a);
        let with_absent = derive_solver_with_cache_stats(
            &mut db_a,
            &snapshot,
            manifest(),
            SolverBudget::default(),
            absent("polint.semantic_graph"),
            absent("polint.type_value_alias"),
            absent("polint.go.semantic"),
        )
        .output_digest;

        let mut db_b = db_with_copy_chain();
        let with_present = derive_solver_with_cache_stats(
            &mut db_b,
            &snapshot,
            manifest(),
            SolverBudget::default(),
            Digest::from_parts(
                DigestKind::ProviderOutput,
                "polint.semantic_graph",
                &["changed"],
            ),
            absent("polint.type_value_alias"),
            absent("polint.go.semantic"),
        )
        .output_digest;

        assert_ne!(with_absent, with_present);
    }

    #[test]
    fn points_to_source_family_digest_change_invalidates_output_digest() {
        // The points-to source families are carried by polint.type_value_alias's
        // output digest; changing it invalidates the solver output (D-15).
        let mut db_a = db_with_copy_chain();
        let snapshot = snapshot(&db_a);
        let base = derive_solver_with_cache_stats(
            &mut db_a,
            &snapshot,
            manifest(),
            SolverBudget::default(),
            absent("polint.semantic_graph"),
            absent("polint.type_value_alias"),
            absent("polint.go.semantic"),
        )
        .output_digest;

        let mut db_b = db_with_copy_chain();
        let changed = derive_solver_with_cache_stats(
            &mut db_b,
            &snapshot,
            manifest(),
            SolverBudget::default(),
            absent("polint.semantic_graph"),
            Digest::from_parts(
                DigestKind::ProviderOutput,
                "polint.type_value_alias",
                &["changed"],
            ),
            absent("polint.go.semantic"),
        )
        .output_digest;

        assert_ne!(base, changed);

        let mut bumped_js = SolverBudget::default();
        bumped_js.js.max_tokens_per_var += 1;
        let mut db_c = db_with_copy_chain();
        let js_changed = derive_solver_with_cache_stats(
            &mut db_c,
            &snapshot,
            manifest(),
            bumped_js,
            absent("polint.semantic_graph"),
            absent("polint.type_value_alias"),
            absent("polint.go.semantic"),
        )
        .output_digest;

        assert_ne!(
            base, js_changed,
            "changing a JS token budget knob must invalidate the solver output digest"
        );
    }

    #[test]
    fn go_semantic_upstream_digest_change_invalidates_output_digest() {
        // FIX 4: the Go RTA policy reads the stored polint.go.semantic RTA-signal families
        // (instantiated_types / address_taken / dynamic_dispatch / method_sets) to resolve
        // dynamic dispatch, so a Go edit touching ONLY those families changes the resolved
        // edges and MUST invalidate the solver cache. A change to the consumed
        // polint.go.semantic output digest therefore changes the solver output digest.
        let mut db_a = db_with_copy_chain();
        let snapshot = snapshot(&db_a);
        let base = derive_solver_with_cache_stats(
            &mut db_a,
            &snapshot,
            manifest(),
            SolverBudget::default(),
            absent("polint.semantic_graph"),
            absent("polint.type_value_alias"),
            absent("polint.go.semantic"),
        )
        .output_digest;

        let mut db_b = db_with_copy_chain();
        let changed = derive_solver_with_cache_stats(
            &mut db_b,
            &snapshot,
            manifest(),
            SolverBudget::default(),
            absent("polint.semantic_graph"),
            absent("polint.type_value_alias"),
            Digest::from_parts(
                DigestKind::ProviderOutput,
                "polint.go.semantic",
                &["changed"],
            ),
        )
        .output_digest;

        assert_ne!(
            base, changed,
            "a polint.go.semantic output-digest change must invalidate the solver output digest"
        );
    }

    #[test]
    fn budget_change_invalidates_output_digest() {
        // D-15: active SolverBudget knobs participate in the output digest.
        let mut db_a = db_with_copy_chain();
        let snapshot = snapshot(&db_a);
        let base = derive_solver_with_cache_stats(
            &mut db_a,
            &snapshot,
            manifest(),
            SolverBudget::default(),
            absent("polint.semantic_graph"),
            absent("polint.type_value_alias"),
            absent("polint.go.semantic"),
        )
        .output_digest;

        let mut bumped = SolverBudget::default();
        bumped.max_steps += 1;
        let mut db_b = db_with_copy_chain();
        let changed = derive_solver_with_cache_stats(
            &mut db_b,
            &snapshot,
            manifest(),
            bumped,
            absent("polint.semantic_graph"),
            absent("polint.type_value_alias"),
            absent("polint.go.semantic"),
        )
        .output_digest;

        assert_ne!(base, changed);
    }

    #[test]
    fn inactive_budget_knobs_do_not_invalidate_output_digest() {
        let mut db_a = db_with_copy_chain();
        let snapshot = snapshot(&db_a);
        let base = derive_solver_with_cache_stats(
            &mut db_a,
            &snapshot,
            manifest(),
            SolverBudget::default(),
            absent("polint.semantic_graph"),
            absent("polint.type_value_alias"),
            absent("polint.go.semantic"),
        )
        .output_digest;

        let mut bumped_points_to = SolverBudget::default();
        bumped_points_to.points_to.max_objects_per_var += 1;
        let mut db_b = db_with_copy_chain();
        let changed_points_to = derive_solver_with_cache_stats(
            &mut db_b,
            &snapshot,
            manifest(),
            bumped_points_to,
            absent("polint.semantic_graph"),
            absent("polint.type_value_alias"),
            absent("polint.go.semantic"),
        )
        .output_digest;

        assert_eq!(
            base, changed_points_to,
            "changing inactive points-to knobs must not invalidate solver output"
        );

        let mut bumped_object = SolverBudget::default();
        bumped_object.object.max_receiver_candidates_per_callsite += 1;
        let mut db_c = db_with_copy_chain();
        let changed_object = derive_solver_with_cache_stats(
            &mut db_c,
            &snapshot,
            manifest(),
            bumped_object,
            absent("polint.semantic_graph"),
            absent("polint.type_value_alias"),
            absent("polint.go.semantic"),
        )
        .output_digest;

        assert_eq!(
            base, changed_object,
            "changing disabled object-model knobs must not invalidate solver output"
        );
    }

    #[test]
    fn inactive_solver_config_changes_do_not_invalidate_output_digest() {
        let mut db_a = db_with_copy_chain();
        let base_snapshot = snapshot_with_config(&db_a, "");
        let base = derive_solver_with_cache_stats(
            &mut db_a,
            &base_snapshot,
            manifest(),
            SolverBudget::default(),
            absent("polint.semantic_graph"),
            absent("polint.type_value_alias"),
            absent("polint.go.semantic"),
        )
        .output_digest;

        let mut db_b = db_with_copy_chain();
        let inactive_object_snapshot = snapshot_with_config(
            &db_b,
            r#"
[solver.js]
max_object_receiver_candidates_per_callsite = 127
"#,
        );
        let changed = derive_solver_with_cache_stats(
            &mut db_b,
            &inactive_object_snapshot,
            manifest(),
            SolverBudget::default(),
            absent("polint.semantic_graph"),
            absent("polint.type_value_alias"),
            absent("polint.go.semantic"),
        )
        .output_digest;

        assert_eq!(
            base, changed,
            "inactive solver config changes must not invalidate solver output"
        );
    }

    #[test]
    fn inactive_solver_config_changes_do_not_flow_through_semantic_graph_digest() {
        let base_temp = tempdir().expect("tempdir");
        fs::write(base_temp.path().join(".polint.toml"), "").expect("config");
        let base_loaded = load_config(base_temp.path()).expect("config loads");
        let mut base_semantic_db = AnalysisDb::new();
        let base_semantic_digest =
            semantic_graph_digest_for_loaded(&mut base_semantic_db, &base_loaded);

        let changed_temp = tempdir().expect("tempdir");
        fs::write(
            changed_temp.path().join(".polint.toml"),
            r#"
[solver.js]
max_object_receiver_candidates_per_callsite = 127
"#,
        )
        .expect("config");
        let changed_loaded = load_config(changed_temp.path()).expect("config loads");
        let mut changed_semantic_db = AnalysisDb::new();
        let changed_semantic_digest =
            semantic_graph_digest_for_loaded(&mut changed_semantic_db, &changed_loaded);

        assert_eq!(
            base_semantic_digest, changed_semantic_digest,
            "disabled object-model caps must not invalidate semantic graph output"
        );

        let mut base_solver_db = AnalysisDb::new();
        let base_snapshot = snapshot_from_loaded(&base_solver_db, &base_loaded);
        let base = derive_solver_with_cache_stats(
            &mut base_solver_db,
            &base_snapshot,
            manifest(),
            solver_budget_from_loaded(&base_loaded),
            base_semantic_digest.expect("semantic digest"),
            absent("polint.type_value_alias"),
            absent("polint.go.semantic"),
        )
        .output_digest;

        let mut changed_solver_db = AnalysisDb::new();
        let changed_snapshot = snapshot_from_loaded(&changed_solver_db, &changed_loaded);
        let changed = derive_solver_with_cache_stats(
            &mut changed_solver_db,
            &changed_snapshot,
            manifest(),
            solver_budget_from_loaded(&changed_loaded),
            changed_semantic_digest.expect("semantic digest"),
            absent("polint.type_value_alias"),
            absent("polint.go.semantic"),
        )
        .output_digest;

        assert_eq!(
            base, changed,
            "inactive solver config changes must not invalidate solver through semantic graph"
        );
    }

    #[test]
    fn default_budget_does_not_register_ts_object_model_policy() {
        let db = AnalysisDb::new();
        let mut policies = solver_policies_for_db(&db, &SolverBudget::default());

        assert_eq!(policies.len(), 2);
        assert!(!register_ts_object_model_policy_if_enabled(
            &mut policies,
            &db,
            &SolverBudget::default()
        ));
        assert_eq!(
            policies
                .iter()
                .map(|policy| policy.id())
                .collect::<Vec<_>>(),
            vec!["go_rta", "ts_tokens"]
        );
    }

    #[test]
    fn enabled_budget_registers_ts_object_model_policy() {
        let db = AnalysisDb::new();
        let policies = solver_policies_for_db(
            &db,
            &SolverBudget {
                object_model_enabled: true,
                ..SolverBudget::default()
            },
        );

        assert_eq!(
            policies
                .iter()
                .map(|policy| policy.id())
                .collect::<Vec<_>>(),
            vec!["go_rta", "ts_tokens", "ts_object_model"]
        );
    }

    #[test]
    fn enabled_registration_helper_pushes_ts_object_model_policy() {
        let db = AnalysisDb::new();
        let mut policies = solver_policies_for_db(&db, &SolverBudget::default());
        assert!(register_ts_object_model_policy_if_enabled(
            &mut policies,
            &db,
            &SolverBudget {
                object_model_enabled: true,
                ..SolverBudget::default()
            }
        ));
        assert_eq!(
            policies
                .iter()
                .map(|policy| policy.id())
                .collect::<Vec<_>>(),
            vec!["go_rta", "ts_tokens", "ts_object_model"]
        );
    }

    #[test]
    fn object_model_flag_invalidates_output_digest_without_placeholder_edges() {
        let mut db_a = db_with_copy_chain();
        let snapshot = snapshot(&db_a);
        let base = derive_solver_with_cache_stats(
            &mut db_a,
            &snapshot,
            manifest(),
            SolverBudget::default(),
            absent("polint.semantic_graph"),
            absent("polint.type_value_alias"),
            absent("polint.go.semantic"),
        )
        .output_digest;

        let mut db_b = db_with_copy_chain();
        let changed = derive_solver_with_cache_stats(
            &mut db_b,
            &snapshot,
            manifest(),
            SolverBudget {
                object_model_enabled: true,
                ..SolverBudget::default()
            },
            absent("polint.semantic_graph"),
            absent("polint.type_value_alias"),
            absent("polint.go.semantic"),
        )
        .output_digest;

        assert_ne!(
            base, changed,
            "toggling object-model enablement must invalidate solver output"
        );
        let base_edges = db_a
            .solver_derived_edges()
            .iter()
            .map(|edge| edge.stable_key.as_str())
            .collect::<Vec<_>>();
        let changed_edges = db_b
            .solver_derived_edges()
            .iter()
            .map(|edge| edge.stable_key.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            changed_edges, base_edges,
            "enabling object model over an input without object facts must not add edges"
        );
    }

    #[test]
    fn object_model_budget_change_invalidates_output_digest() {
        let mut db_a = db_with_copy_chain();
        let snapshot = snapshot(&db_a);
        let base = derive_solver_with_cache_stats(
            &mut db_a,
            &snapshot,
            manifest(),
            SolverBudget {
                object_model_enabled: true,
                ..SolverBudget::default()
            },
            absent("polint.semantic_graph"),
            absent("polint.type_value_alias"),
            absent("polint.go.semantic"),
        )
        .output_digest;

        let bumped = SolverBudget {
            object_model_enabled: true,
            object: JsObjectModelSubBudget {
                max_receiver_candidates_per_callsite: SolverBudget::default()
                    .object
                    .max_receiver_candidates_per_callsite
                    + 1,
                ..JsObjectModelSubBudget::default()
            },
            ..SolverBudget::default()
        };
        let mut db_b = db_with_copy_chain();
        let changed = derive_solver_with_cache_stats(
            &mut db_b,
            &snapshot,
            manifest(),
            bumped,
            absent("polint.semantic_graph"),
            absent("polint.type_value_alias"),
            absent("polint.go.semantic"),
        )
        .output_digest;

        assert_ne!(
            base, changed,
            "changing an enabled object-model budget knob must invalidate solver output"
        );
    }

    #[test]
    fn adaptation_model_budget_change_invalidates_output_digest() {
        let mut db_a = db_with_copy_chain();
        let snapshot = snapshot(&db_a);
        let base = derive_solver_with_cache_stats(
            &mut db_a,
            &snapshot,
            manifest(),
            SolverBudget::default(),
            absent("polint.semantic_graph"),
            absent("polint.type_value_alias"),
            absent("polint.go.semantic"),
        )
        .output_digest;

        let bumped = SolverBudget {
            adaptation: AdaptationModelBudget {
                max_model_derived_edges: SolverBudget::default().adaptation.max_model_derived_edges
                    + 1,
                ..AdaptationModelBudget::default()
            },
            ..SolverBudget::default()
        };
        let mut db_b = db_with_copy_chain();
        let changed = derive_solver_with_cache_stats(
            &mut db_b,
            &snapshot,
            manifest(),
            bumped,
            absent("polint.semantic_graph"),
            absent("polint.type_value_alias"),
            absent("polint.go.semantic"),
        )
        .output_digest;

        assert_ne!(
            base, changed,
            "changing an adaptation-model budget knob must invalidate solver output"
        );
    }

    #[test]
    fn run_level_budget_status_invalidates_output_digest() {
        // WR-06: two outputs with the SAME surviving edge set but different run-level
        // budget_status (WithinBudget vs BudgetExceeded) must produce DIFFERENT output
        // digests, so a cache layer keyed on the digest cannot serve a truncated
        // (BudgetExceeded) result under a complete (WithinBudget) run's digest.
        let db = db_with_copy_chain();
        let snapshot = snapshot(&db);
        let budget = SolverBudget::default();

        // Identical (empty) edge sets; only the run-level status differs.
        let within = SolverOutput {
            derived_edges: Vec::new(),
            budget_status: BudgetStatus::WithinBudget,
            budget_reasons: BTreeSet::new(),
        };
        let exceeded = SolverOutput {
            derived_edges: Vec::new(),
            budget_status: BudgetStatus::BudgetExceeded,
            budget_reasons: BTreeSet::from([BudgetReason::SolverMaxSteps.as_str().to_string()]),
        };

        let within_digest = solver_output_digest(
            manifest(),
            &snapshot,
            &budget,
            &absent("polint.semantic_graph"),
            &absent("polint.type_value_alias"),
            &absent("polint.go.semantic"),
            &within,
        );
        let exceeded_digest = solver_output_digest(
            manifest(),
            &snapshot,
            &budget,
            &absent("polint.semantic_graph"),
            &absent("polint.type_value_alias"),
            &absent("polint.go.semantic"),
            &exceeded,
        );

        assert_ne!(
            within_digest, exceeded_digest,
            "a budget-truncated run must not share an output digest with a complete run"
        );
    }

    #[test]
    fn provider_manifests_list_solver_between_semantic_graph_and_refined_calls() {
        let manifests = AnalysisKernel::provider_manifests();
        let semantic_graph = manifests
            .iter()
            .position(|manifest| manifest.id == "polint.semantic_graph")
            .expect("semantic_graph present");
        assert_eq!(manifests[semantic_graph + 1].id, "polint.solver");
        assert_eq!(manifests[semantic_graph + 2].id, "polint.refined_calls");
    }
}

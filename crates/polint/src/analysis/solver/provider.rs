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
//! (the parameter digest already embeds the [`SolverBudget`]), (b) every consumed
//! upstream provider output digest — `polint.semantic_graph` plus the points-to
//! source families produced by `polint.type_value_alias` (`points_to_constraints` /
//! `points_to_sets`), (c) the [`SolverBudget`] explicitly, and (d) per-row stable
//! keys, then `parts.sort()` + `Digest::from_parts`. Any upstream change, algorithm
//! bump, or budget change deterministically invalidates the solver cache.

use std::collections::BTreeSet;

use crate::analysis::ids::SemanticNodeId;
use crate::analysis::solver::budget::{BudgetStatus, SolverBudget};
use crate::analysis::solver::cache_key::solver_provider_parameter_digest;
use crate::analysis::solver::engine::derive_edges;
use crate::analysis::solver::store::{SOLVER_PROVIDER_ID, SolverOutput};
use crate::analysis::solver::validate::{detect_solver_summary_cycle, validate_derived_edges};
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{CacheStats, Digest, DigestKind, InputSnapshot};
use crate::core::AnalysisDb;
use crate::diagnostics::{Diagnostic, TextRange};

#[derive(Debug, Clone, Default)]
pub(crate) struct SolverProviderRunOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
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
) -> SolverProviderRunOutput {
    debug_assert_eq!(manifest.id, SOLVER_PROVIDER_ID);

    // Phase 1-2: drive the solver over the closed input snapshot (the stored
    // semantic-graph constraints). The build is read-only; derive_edges normalizes.
    let constraints = db.semantic_constraints().to_vec();
    let output = derive_edges(&constraints, &budget);

    // Phase 3: digest over the stored stable KEYS + upstream digests + the budget.
    let output_digest = solver_output_digest(
        manifest,
        input_snapshot,
        &budget,
        &semantic_graph_output_digest,
        &type_value_alias_output_digest,
        &output,
    );

    // Phase 4: validate the derived edges + the D-12 cycle-detection check. These are
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
        diagnostics.push(budget_exceeded_diagnostic());
    }

    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();

    // Phase 5-6: store (assigns dense IDs + referentially validates inside
    // from_output). On store error the db keeps its prior state and the facts the
    // digest certifies were not persisted, so return output_digest: None.
    match db.replace_solver_facts(output) {
        Ok(()) => SolverProviderRunOutput {
            diagnostics,
            cache_stats,
            output_digest: Some(output_digest),
        },
        Err(error) => {
            diagnostics.push(provider_error_diagnostic(error.to_string()));
            SolverProviderRunOutput {
                diagnostics,
                cache_stats,
                output_digest: None,
            }
        }
    }
}

/// Output digest over stable KEYS + upstream digests + budget (D-15), never dense
/// IDs.
///
/// Folds (a) provider/version/schema/parameter digests (the parameter digest already
/// embeds the budget via `solver_provider_parameter_digest`), (b) the consumed
/// upstream provider output digests — `polint.semantic_graph` plus the points-to
/// source families carried by `polint.type_value_alias`'s output digest, (c) the
/// `SolverBudget` explicitly (belt-and-suspenders with the parameter digest so a
/// budget change is unmissable, D-15), and (d) per-row stable keys + status/precision
/// + provenance fragment, then `parts.sort()` + `Digest::from_parts`.
fn solver_output_digest(
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    budget: &SolverBudget,
    semantic_graph_output_digest: &Digest,
    type_value_alias_output_digest: &Digest,
    output: &SolverOutput,
) -> Digest {
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!("parameters={}", solver_provider_parameter_digest(budget)),
        format!("config={}", input_snapshot.config.digest),
        format!("semantic_graph_output={semantic_graph_output_digest}"),
        format!("type_value_alias_output={type_value_alias_output_digest}"),
        // The budget participates explicitly (D-15), in addition to being folded
        // through the parameter digest, so a budget change unmissably invalidates.
        format!("budget.max_steps={}", budget.max_steps),
        format!(
            "budget.max_outer_iterations={}",
            budget.max_outer_iterations
        ),
        format!(
            "budget.points_to.max_objects_per_var={}",
            budget.points_to.max_objects_per_var
        ),
        format!(
            "budget.points_to.max_dynamic_vars={}",
            budget.points_to.max_dynamic_vars
        ),
    ];

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

    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "solver_output", &refs)
}

/// Honest budget-exhaustion signal (D-06): emitted when the solver truncated a
/// source's transitive closure under the per-source step budget. Downstream (Phase 52
/// unknown taxonomy) categorizes it; it is never a silent precision drop.
fn budget_exceeded_diagnostic() -> Diagnostic {
    Diagnostic::warning(
        "polint/internal",
        "<workspace>",
        TextRange::point(1, 1),
        "Solver budget exceeded; some transitive edges were truncated (derived edges \
         are flagged BudgetExceeded).",
    )
    .with_evidence("provider", SOLVER_PROVIDER_ID)
    .with_evidence("budget_status", BudgetStatus::BudgetExceeded.as_str())
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
    use crate::analysis::ids::SemanticConstraintId;
    use crate::analysis::points_to::facts::{PointsToPrecision, PointsToStatus};
    use crate::analysis::semantic_graph::constraints::{ConstraintFact, ConstraintKind};
    use crate::analysis::semantic_graph::facts::{NodeKind, SemanticNodeFact, SemanticPrecision};
    use crate::analysis::semantic_graph::store::SemanticGraphOutput;
    use crate::analysis_kernel::AnalysisKernel;
    use crate::analysis_kernel::incremental::{Digest, DigestKind};
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::load_config;
    use crate::core::{AnalysisDb, FunctionId};
    use std::fs;
    use tempfile::tempdir;

    fn manifest() -> &'static ProviderManifest {
        AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == SOLVER_PROVIDER_ID)
            .expect("solver manifest")
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
        )
        .output_digest;

        assert_ne!(base, changed);
    }

    #[test]
    fn budget_change_invalidates_output_digest() {
        // D-15: the SolverBudget participates in the output digest.
        let mut db_a = db_with_copy_chain();
        let snapshot = snapshot(&db_a);
        let base = derive_solver_with_cache_stats(
            &mut db_a,
            &snapshot,
            manifest(),
            SolverBudget::default(),
            absent("polint.semantic_graph"),
            absent("polint.type_value_alias"),
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
        )
        .output_digest;

        assert_ne!(base, changed);
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

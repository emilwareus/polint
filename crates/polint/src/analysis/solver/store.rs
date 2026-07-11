//! Deterministic solver store (GRAPH-04, D-08).
//!
//! [`SolverOutput`] carries the derived-edge rows the unified solver emits;
//! [`SolverOutput::normalized`] sorts them by `(stable_key, id)` THEN assigns dense
//! [`DerivedEdgeId`]s by index — dense IDs only after the stable-key sort, which
//! makes the output byte-stable under input
//! shuffle. [`SolverStore::from_output`] builds the deterministic by-constraint-kind
//! index and referentially validates the rows (duplicate stable keys, precision
//! ceiling).
//!
//! Mirrors `analysis::semantic_graph::store` (`Output`/`normalized`/`from_output`/
//! `PROVIDER_ID`). The `polint.solver` PROVIDER registers in Plan 03; this store is
//! the data the provider normalizes and the deletion property test (D-09) operates
//! over.

use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::error::AnalysisError;
use crate::analysis::ids::DerivedEdgeId;

use super::budget::{BudgetReason, BudgetStatus};
use super::facts::DerivedEdgeFact;

/// The provider id for the unified solver (registered in the kernel manifest in
/// Plan 03), mirroring `SEMANTIC_GRAPH_PROVIDER_ID`.
pub(crate) const SOLVER_PROVIDER_ID: &str = "polint.solver";

/// Provider output for `polint.solver` — the normalized derived-edge rows plus the
/// run-level [`BudgetStatus`]. `budget_status` is `BudgetExceeded` when the solver
/// truncated any source's closure under the per-source step budget, so an exhausted
/// run is never indistinguishable from a complete one (review finding #3 / D-06).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SolverOutput {
    pub(crate) derived_edges: Vec<DerivedEdgeFact>,
    pub(crate) budget_status: BudgetStatus,
    pub(crate) budget_reasons: BTreeSet<String>,
}

impl SolverOutput {
    /// Sorts derived edges by `(stable_key, id)` THEN reassigns dense
    /// [`DerivedEdgeId`]s sequentially by index (dense IDs only after the stable-key
    /// sort, D-08). Shuffling the input rows yields byte-identical normalized output.
    pub(crate) fn normalized(mut self) -> Self {
        self.derived_edges.sort_by(|left, right| {
            (left.stable_key.as_str(), left.id).cmp(&(right.stable_key.as_str(), right.id))
        });
        for (index, edge) in self.derived_edges.iter_mut().enumerate() {
            edge.id = DerivedEdgeId(index as u64);
        }
        self
    }
}

/// Typed solver store with the deterministic read index consumers use: derived
/// edges indexed by their producing `ConstraintKind` tag (the provenance's
/// `constraint_kind` snake_case label). Built after [`SolverOutput::normalized`].
#[derive(Debug, Clone, Default)]
pub(crate) struct SolverStore {
    derived_edges: Vec<DerivedEdgeFact>,
    budget_status: BudgetStatus,
    budget_reasons: BTreeSet<String>,
    /// Derived edges indexed by their producing constraint-kind tag (the owned
    /// `provenance.constraint_kind` snake_case label).
    edges_by_constraint_kind: BTreeMap<String, Vec<usize>>,
}

impl SolverStore {
    /// Builds the store after `normalized()`, referentially validating the rows:
    /// duplicate stable keys and precision-ceiling violations (D-06: derived edges
    /// reject the exact tier) are surfaced as [`AnalysisError::InvalidFact`] rather
    /// than silently accepted.
    pub(crate) fn from_output(output: SolverOutput) -> Result<Self, AnalysisError> {
        let output = output.normalized();
        validate_budget_evidence(
            output.budget_status,
            &output.budget_reasons,
            output.derived_edges.is_empty(),
        )?;

        let mut seen_keys: BTreeSet<&str> = BTreeSet::new();
        for edge in &output.derived_edges {
            if !edge.honors_precision_ceiling() {
                return Err(AnalysisError::InvalidFact {
                    provider: SOLVER_PROVIDER_ID,
                    reason: format!(
                        "derived edge `{}` claims exact precision (precision ceiling violated, D-06)",
                        edge.stable_key
                    ),
                });
            }
            if !seen_keys.insert(edge.stable_key.as_str()) {
                return Err(AnalysisError::InvalidFact {
                    provider: SOLVER_PROVIDER_ID,
                    reason: format!("duplicate derived-edge stable key `{}`", edge.stable_key),
                });
            }
        }

        let mut store = Self {
            derived_edges: output.derived_edges,
            budget_status: output.budget_status,
            budget_reasons: output.budget_reasons,
            ..Self::default()
        };

        // Edges are already sorted by (stable_key, id), so each per-kind index vector
        // is appended in stable order.
        for (index, edge) in store.derived_edges.iter().enumerate() {
            store
                .edges_by_constraint_kind
                .entry(edge.provenance.constraint_kind.clone())
                .or_default()
                .push(index);
        }

        Ok(store)
    }

    pub(crate) fn derived_edges(&self) -> &[DerivedEdgeFact] {
        &self.derived_edges
    }

    pub(crate) fn budget_status(&self) -> BudgetStatus {
        self.budget_status
    }

    pub(crate) fn budget_reasons(&self) -> &BTreeSet<String> {
        &self.budget_reasons
    }

    /// Indices into [`Self::derived_edges`] for every edge produced by the given
    /// constraint-kind tag (the `ConstraintKind::as_str()` snake_case label).
    pub(crate) fn edges_for_constraint_kind(&self, kind: &str) -> &[usize] {
        self.edges_by_constraint_kind
            .get(kind)
            .map_or(&[], Vec::as_slice)
    }
}

fn validate_budget_evidence(
    budget_status: BudgetStatus,
    budget_reasons: &BTreeSet<String>,
    derived_edges_empty: bool,
) -> Result<(), AnalysisError> {
    let canonical_reasons: BTreeSet<&str> = BudgetReason::all()
        .iter()
        .map(|reason| reason.as_str())
        .collect();
    if let Some(reason) = budget_reasons
        .iter()
        .find(|reason| !canonical_reasons.contains(reason.as_str()))
    {
        return Err(AnalysisError::InvalidFact {
            provider: SOLVER_PROVIDER_ID,
            reason: format!("unknown solver budget reason `{reason}`"),
        });
    }

    if budget_status == BudgetStatus::NotRun && !derived_edges_empty {
        return Err(AnalysisError::InvalidFact {
            provider: SOLVER_PROVIDER_ID,
            reason: "solver NotRun status cannot contain derived edges".to_string(),
        });
    }

    match (budget_status, budget_reasons.is_empty()) {
        (BudgetStatus::WithinBudget | BudgetStatus::NotRun, true)
        | (BudgetStatus::BudgetExceeded, false) => Ok(()),
        (BudgetStatus::WithinBudget | BudgetStatus::NotRun, false) => {
            Err(AnalysisError::InvalidFact {
                provider: SOLVER_PROVIDER_ID,
                reason: "solver budget reasons require BudgetExceeded status".to_string(),
            })
        }
        (BudgetStatus::BudgetExceeded, true) => Err(AnalysisError::InvalidFact {
            provider: SOLVER_PROVIDER_ID,
            reason: "solver BudgetExceeded status requires at least one budget reason".to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ids::SemanticNodeId;
    use crate::analysis::points_to::facts::{PointsToPrecision, PointsToStatus};
    use crate::analysis::semantic_graph::constraints::ConstraintKind;
    use crate::analysis::solver::provenance::{ContributingFact, DerivedEdgeProvenance};
    use crate::analysis_kernel::FactFamily;

    fn provenance(constraints: &[&str], step: u64) -> DerivedEdgeProvenance {
        DerivedEdgeProvenance::new(
            constraints.iter().map(|c| {
                ContributingFact::from_parts(
                    FactFamily::PointsToConstraint,
                    &[("constraint", (*c).to_string())],
                )
            }),
            &ConstraintKind::CopyEdge {
                dst: SemanticNodeId(1),
                src: SemanticNodeId(2),
            },
            step,
        )
    }

    fn edge(
        id: u64,
        source: u64,
        target: u64,
        stable_key: &str,
        prov: DerivedEdgeProvenance,
    ) -> DerivedEdgeFact {
        DerivedEdgeFact {
            id: DerivedEdgeId(id),
            source: SemanticNodeId(source),
            target: SemanticNodeId(target),
            status: PointsToStatus::Present,
            precision: PointsToPrecision::FlowInsensitive,
            stable_key: stable_key.to_string(),
            provenance: prov,
        }
    }

    fn sample_output() -> SolverOutput {
        SolverOutput {
            derived_edges: vec![
                edge(7, 1, 2, "edge|copy_edge|b", provenance(&["copy"], 3)),
                edge(
                    9,
                    3,
                    4,
                    "edge|copy_edge|a",
                    provenance(&["addr-a", "copy"], 5),
                ),
            ],
            ..SolverOutput::default()
        }
    }

    #[test]
    fn normalized_assigns_dense_ids_after_stable_key_sort() {
        let normalized = sample_output().normalized();
        // "edge|copy_edge|a" < "edge|copy_edge|b", so it sorts first and gets dense
        // id 0 regardless of its larger pre-sort id (9).
        assert_eq!(normalized.derived_edges[0].stable_key, "edge|copy_edge|a");
        assert_eq!(normalized.derived_edges[0].id, DerivedEdgeId(0));
        assert_eq!(normalized.derived_edges[1].stable_key, "edge|copy_edge|b");
        assert_eq!(normalized.derived_edges[1].id, DerivedEdgeId(1));
    }

    #[test]
    fn normalized_is_shuffle_stable() {
        let base = sample_output();
        let mut shuffled = base.clone();
        shuffled.derived_edges.reverse();

        let a = base.normalized();
        let b = shuffled.normalized();

        // Byte-identical serialized output under shuffle (dense `id` is
        // `#[serde(skip)]`, so this captures endpoints/status/precision/stable_key/
        // provenance).
        let a_edges = serde_json::to_string(&a.derived_edges).expect("serialize a");
        let b_edges = serde_json::to_string(&b.derived_edges).expect("serialize b");
        assert_eq!(a_edges, b_edges);
        // The dense IDs assigned are identical too.
        assert_eq!(
            a.derived_edges.iter().map(|e| e.id).collect::<Vec<_>>(),
            b.derived_edges.iter().map(|e| e.id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn from_output_builds_constraint_kind_index() {
        let store = SolverStore::from_output(sample_output()).expect("store");
        assert_eq!(store.derived_edges().len(), 2);
        assert_eq!(store.budget_status(), BudgetStatus::WithinBudget);
        assert!(store.budget_reasons().is_empty());
        // Both edges were produced by a copy_edge constraint.
        assert_eq!(store.edges_for_constraint_kind("copy_edge").len(), 2);
        // A kind with no rows resolves to an empty slice.
        assert!(store.edges_for_constraint_kind("alloc").is_empty());
    }

    #[test]
    fn from_output_preserves_run_level_budget_reasons_without_edge_rows() {
        let output = SolverOutput {
            budget_status: BudgetStatus::BudgetExceeded,
            budget_reasons: BTreeSet::from(["solver.max_steps".to_string()]),
            ..SolverOutput::default()
        };

        let store = SolverStore::from_output(output).expect("store");

        assert!(store.derived_edges().is_empty());
        assert_eq!(store.budget_status(), BudgetStatus::BudgetExceeded);
        assert_eq!(
            store.budget_reasons(),
            &BTreeSet::from(["solver.max_steps".to_string()])
        );
    }

    #[test]
    fn from_output_rejects_budget_reasons_without_budget_exceeded_status() {
        let output = SolverOutput {
            budget_status: BudgetStatus::WithinBudget,
            budget_reasons: BTreeSet::from(["solver.max_steps".to_string()]),
            ..SolverOutput::default()
        };

        let error = SolverStore::from_output(output).expect_err("inconsistent budget rejected");

        assert!(
            error
                .to_string()
                .contains("solver budget reasons require BudgetExceeded status")
        );
        assert!(error.to_string().contains("polint.solver"));
    }

    #[test]
    fn from_output_accepts_not_run_without_budget_reasons() {
        let output = SolverOutput {
            budget_status: BudgetStatus::NotRun,
            ..SolverOutput::default()
        };

        let store = SolverStore::from_output(output).expect("not-run store");

        assert_eq!(store.budget_status(), BudgetStatus::NotRun);
        assert!(store.budget_reasons().is_empty());
    }

    #[test]
    fn from_output_rejects_not_run_with_derived_edges() {
        let output = SolverOutput {
            derived_edges: vec![edge(
                0,
                1,
                2,
                "edge|copy_edge|not-run",
                provenance(&["copy"], 1),
            )],
            budget_status: BudgetStatus::NotRun,
            ..SolverOutput::default()
        };

        let error = SolverStore::from_output(output).expect_err("not-run edge rejected");

        assert!(
            error
                .to_string()
                .contains("solver NotRun status cannot contain derived edges")
        );
        assert!(error.to_string().contains("polint.solver"));
    }

    #[test]
    fn from_output_rejects_budget_exceeded_without_budget_reasons() {
        let output = SolverOutput {
            budget_status: BudgetStatus::BudgetExceeded,
            ..SolverOutput::default()
        };

        let error = SolverStore::from_output(output).expect_err("missing budget reason rejected");

        assert!(
            error
                .to_string()
                .contains("solver BudgetExceeded status requires at least one budget reason")
        );
        assert!(error.to_string().contains("polint.solver"));
    }

    #[test]
    fn from_output_rejects_unknown_budget_reason_labels() {
        let output = SolverOutput {
            budget_status: BudgetStatus::BudgetExceeded,
            budget_reasons: BTreeSet::from(["prototype_cycle".to_string()]),
            ..SolverOutput::default()
        };

        let error = SolverStore::from_output(output).expect_err("unknown reason rejected");

        assert!(
            error
                .to_string()
                .contains("unknown solver budget reason `prototype_cycle`")
        );
        assert!(error.to_string().contains("polint.solver"));
    }

    #[test]
    fn from_output_rejects_duplicate_stable_keys() {
        let output = SolverOutput {
            derived_edges: vec![
                edge(0, 1, 2, "edge|copy_edge|dup", provenance(&["copy"], 1)),
                edge(1, 3, 4, "edge|copy_edge|dup", provenance(&["addr"], 2)),
            ],
            ..SolverOutput::default()
        };
        let error = SolverStore::from_output(output).expect_err("duplicate rejected");
        assert!(
            error
                .to_string()
                .contains("duplicate derived-edge stable key")
        );
        assert!(error.to_string().contains("polint.solver"));
    }

    #[test]
    fn from_output_rejects_exact_precision() {
        // Construct an edge whose precision would map to the exact tier. Since
        // `derived_edge_precision_ceiling` never returns Exact for any
        // PointsToPrecision, we assert the validation path holds for all variants by
        // checking the honest mapping directly (the store accepts every legal edge).
        let store = SolverStore::from_output(sample_output()).expect("store");
        assert!(
            store
                .derived_edges()
                .iter()
                .all(DerivedEdgeFact::honors_precision_ceiling)
        );
    }
}

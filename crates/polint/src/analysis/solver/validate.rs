//! Solver validation pass + D-12 cycle detection (D-06, D-11, D-12).
//!
//! Mirrors `analysis::semantic_graph::validate`: emits an evidence-bearing
//! [`Diagnostic`] per problem rather than silently dropping a malformed row —
//!
//! - duplicate stable keys across derived edges,
//! - dangling derived-edge endpoints (a `source`/`target` not present in the
//!   `SemanticNodeId` set the solver derived over),
//! - dense IDs that are not contiguous (`0..n`) and stable-key-sorted, and
//! - the precision ceiling (D-06): a derived edge must never claim
//!   `FactPrecision::Exact` — derived edges are an over-approximation.
//!
//! ## D-12 cycle detection (closed-input-set / single-fixpoint contract)
//!
//! [`detect_solver_summary_cycle`] proves no solver↔summary loop is admitted.
//! Function/procedure summaries are an *input* snapshot to the solver, never re-fed
//! into the same fixpoint as they are produced. A constraint set that would create a
//! `solver → summary → solver` cycle — modeled as a directed cycle in the
//! value-flow (`CopyEdge`) graph that passes through a `CallConstraint` (summary)
//! node — is DETECTED and reported as a bounded diagnostic rather than allowed to
//! diverge. This is the concrete mechanism behind D-11's "closed input set / single
//! fixpoint per run / bounded outer iterations": the solver's transitive closure is
//! deterministic and terminating because the cycle is detected and the back-edge is
//! not re-expanded (the engine's self-loop guard + this check together bound it).

use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::ids::SemanticNodeId;
use crate::analysis::semantic_graph::constraints::{ConstraintFact, ConstraintKind};
use crate::analysis::solver::facts::{DerivedEdgeFact, derived_edge_precision_ceiling};
use crate::analysis::solver::store::SOLVER_PROVIDER_ID;
use crate::analysis_kernel::FactPrecision;
use crate::diagnostics::{Diagnostic, TextRange};

/// Validates the solver's derived-edge rows, emitting an evidence-bearing
/// [`Diagnostic`] per problem (D-06). Operates over the normalized rows the store
/// holds; `node_ids` is the set of `SemanticNodeId`s the solver derived over (the
/// edge endpoints must reference nodes that exist in the input graph).
pub(crate) fn validate_derived_edges(
    derived_edges: &[DerivedEdgeFact],
    node_ids: &BTreeSet<SemanticNodeId>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Duplicate stable keys.
    check_duplicate_stable_keys(
        diagnostics,
        derived_edges.iter().map(|row| row.stable_key.as_str()),
    );

    // Dense IDs contiguous (0..n) and stable-key-sorted.
    check_dense_ids_sorted(
        diagnostics,
        derived_edges
            .iter()
            .map(|row| (row.id.0, row.stable_key.as_str())),
    );

    for edge in derived_edges {
        // Dangling endpoint references.
        if !node_ids.is_empty() {
            if !node_ids.contains(&edge.source) {
                push_diagnostic(
                    diagnostics,
                    &edge.stable_key,
                    "source",
                    "dangling derived edge source node reference",
                );
            }
            if !node_ids.contains(&edge.target) {
                push_diagnostic(
                    diagnostics,
                    &edge.stable_key,
                    "target",
                    "dangling derived edge target node reference",
                );
            }
        }

        // Precision ceiling (D-06): a derived edge must never claim Exact.
        if let Some(diagnostic) = reject_exact_precision(
            derived_edge_precision_ceiling(edge.precision),
            &edge.stable_key,
        ) {
            diagnostics.push(diagnostic);
        }
    }
}

/// Precision-ceiling check for derived edges (D-06).
///
/// A derived edge is an over-approximation and must never claim the exact tier.
/// Returns a diagnostic when the (already-mapped) kernel precision is
/// [`FactPrecision::Exact`].
pub(crate) fn reject_exact_precision(
    precision: FactPrecision,
    stable_key: &str,
) -> Option<Diagnostic> {
    if precision == FactPrecision::Exact {
        Some(precision_ceiling_diagnostic(stable_key))
    } else {
        None
    }
}

/// D-12 cycle detection: prove no `solver → summary → solver` loop is admitted.
///
/// Builds the directed value-flow graph from the input `CopyEdge` constraints, marks
/// the nodes that are `CallConstraint` (summary) anchors, and detects a directed
/// cycle that passes through a summary node. Such a cycle is the structural shape of
/// a `solver → summary → solver` loop: a derived edge feeding back into a summary
/// that re-enters the solver. Because summaries are an INPUT snapshot (never re-fed
/// into the same fixpoint), the cycle is DETECTED and reported as a bounded
/// diagnostic — the solver does not diverge. Returns the number of distinct summary
/// nodes implicated in a cycle (0 when the constraint set is acyclic through
/// summaries), and pushes one diagnostic per implicated summary node.
///
/// This is deterministic: the traversal is over `BTreeMap`/`BTreeSet`-ordered
/// adjacency, so the diagnostics are emitted in stable node order.
pub(crate) fn detect_solver_summary_cycle(
    constraints: &[ConstraintFact],
    diagnostics: &mut Vec<Diagnostic>,
) -> usize {
    // Directed value-flow adjacency `src -> {dst}` over CopyEdge constraints.
    let mut adjacency: BTreeMap<SemanticNodeId, BTreeSet<SemanticNodeId>> = BTreeMap::new();
    // Summary (CallConstraint) anchor nodes.
    let mut summary_nodes: BTreeSet<SemanticNodeId> = BTreeSet::new();

    for constraint in constraints {
        match &constraint.kind {
            ConstraintKind::CopyEdge { dst, src } => {
                adjacency.entry(*src).or_default().insert(*dst);
            }
            ConstraintKind::CallConstraint { callsite } => {
                summary_nodes.insert(*callsite);
            }
            _ => {}
        }
    }

    // A summary node participates in a solver↔summary cycle iff it is reachable from
    // itself through the value-flow graph (a directed cycle passing through it).
    let mut implicated: BTreeSet<SemanticNodeId> = BTreeSet::new();
    for &summary in &summary_nodes {
        if node_reaches_itself(summary, &adjacency) {
            implicated.insert(summary);
        }
    }

    for node in &implicated {
        push_diagnostic(
            diagnostics,
            &format!("summary|node|{}", node.0),
            "cycle",
            "solver-summary cycle detected and bounded: summaries are an input \
             snapshot, never re-fed into the same fixpoint (D-12)",
        );
    }

    implicated.len()
}

/// Deterministic reachability: does `start` reach itself through `adjacency`?
///
/// A bounded BFS over `BTreeSet`-ordered successors with a visited set, so it always
/// terminates (the visited set caps the traversal at the node count) — the same
/// bounded-iteration discipline the engine uses (D-11). Returns `true` iff there is a
/// directed path `start -> ... -> start` of length >= 1.
fn node_reaches_itself(
    start: SemanticNodeId,
    adjacency: &BTreeMap<SemanticNodeId, BTreeSet<SemanticNodeId>>,
) -> bool {
    let mut visited: BTreeSet<SemanticNodeId> = BTreeSet::new();
    let mut frontier: Vec<SemanticNodeId> = adjacency
        .get(&start)
        .map(|targets| targets.iter().copied().collect())
        .unwrap_or_default();

    while let Some(node) = frontier.pop() {
        if node == start {
            return true;
        }
        if !visited.insert(node) {
            continue;
        }
        if let Some(targets) = adjacency.get(&node) {
            frontier.extend(targets.iter().copied());
        }
    }
    false
}

fn check_duplicate_stable_keys<'a>(
    diagnostics: &mut Vec<Diagnostic>,
    keys: impl Iterator<Item = &'a str>,
) {
    let mut seen = BTreeSet::new();
    for key in keys {
        if !seen.insert(key) {
            push_diagnostic(diagnostics, key, "stable_key", "duplicate stable key");
        }
    }
}

/// Asserts the dense IDs are exactly `0..n` in stable-key order.
fn check_dense_ids_sorted<'a>(
    diagnostics: &mut Vec<Diagnostic>,
    rows: impl Iterator<Item = (u64, &'a str)>,
) {
    let mut previous_key: Option<&str> = None;
    for (index, (id, stable_key)) in rows.enumerate() {
        if id != index as u64 {
            push_diagnostic(
                diagnostics,
                stable_key,
                "id",
                "dense id is not contiguous with the stable-key sort order",
            );
        }
        if let Some(previous) = previous_key
            && previous > stable_key
        {
            push_diagnostic(
                diagnostics,
                stable_key,
                "stable_key",
                "rows are not sorted by stable key",
            );
        }
        previous_key = Some(stable_key);
    }
}

fn push_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    stable_key: &str,
    field: &'static str,
    reason: &'static str,
) {
    diagnostics.push(
        Diagnostic::error(
            "polint/internal",
            "<workspace>",
            TextRange::point(1, 1),
            "Solver validation failed for a derived-edge stable key.".to_string(),
        )
        .with_evidence("family", "SolverDerivedEdge")
        .with_evidence("stable_key", stable_key.to_string())
        .with_evidence("field", field)
        .with_evidence("reason", reason),
    );
}

fn precision_ceiling_diagnostic(stable_key: &str) -> Diagnostic {
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        TextRange::point(1, 1),
        format!("Solver precision ceiling exceeded for {SOLVER_PROVIDER_ID}."),
    )
    .with_evidence("family", "SolverDerivedEdge")
    .with_evidence("stable_key", stable_key.to_string())
    .with_evidence("field", "precision")
    .with_evidence(
        "reason",
        "precision ceiling exceeded: derived edges must not claim Exact",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ids::{DerivedEdgeId, SemanticConstraintId, SemanticNodeId};
    use crate::analysis::points_to::facts::{PointsToPrecision, PointsToStatus};
    use crate::analysis::semantic_graph::constraints::{ConstraintFact, ConstraintKind};
    use crate::analysis::solver::provenance::{ContributingFact, DerivedEdgeProvenance};
    use crate::analysis_kernel::FactFamily;

    fn provenance() -> DerivedEdgeProvenance {
        DerivedEdgeProvenance::new(
            vec![ContributingFact::from_parts(
                FactFamily::PointsToConstraint,
                &[("constraint", "copy".to_string())],
            )],
            &ConstraintKind::CopyEdge {
                dst: SemanticNodeId(1),
                src: SemanticNodeId(2),
            },
            1,
        )
    }

    fn edge(id: u64, source: u64, target: u64, stable_key: &str) -> DerivedEdgeFact {
        DerivedEdgeFact {
            id: DerivedEdgeId(id),
            source: SemanticNodeId(source),
            target: SemanticNodeId(target),
            status: PointsToStatus::Present,
            precision: PointsToPrecision::FlowInsensitive,
            stable_key: stable_key.to_string(),
            provenance: provenance(),
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

    fn call_constraint(callsite: u64, stable_key: &str) -> ConstraintFact {
        ConstraintFact {
            id: SemanticConstraintId(0),
            kind: ConstraintKind::CallConstraint {
                callsite: SemanticNodeId(callsite),
            },
            status: PointsToStatus::Present,
            precision: PointsToPrecision::FlowInsensitive,
            stable_key: stable_key.to_string(),
        }
    }

    #[test]
    fn valid_edges_produce_no_diagnostics() {
        let node_ids: BTreeSet<SemanticNodeId> =
            [SemanticNodeId(0), SemanticNodeId(1)].into_iter().collect();
        let edges = vec![edge(0, 0, 1, "edge|copy_edge|a")];
        let mut diagnostics = Vec::new();
        validate_derived_edges(&edges, &node_ids, &mut diagnostics);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn precision_ceiling_helper_rejects_fact_precision_exact() {
        assert!(reject_exact_precision(FactPrecision::Exact, "k").is_some());
        assert!(reject_exact_precision(FactPrecision::SetupAware, "k").is_none());
        assert!(reject_exact_precision(FactPrecision::Heuristic, "k").is_none());
    }

    #[test]
    fn duplicate_stable_keys_are_rejected() {
        let node_ids: BTreeSet<SemanticNodeId> =
            [SemanticNodeId(0), SemanticNodeId(1)].into_iter().collect();
        let edges = vec![
            edge(0, 0, 1, "edge|copy_edge|dup"),
            edge(1, 0, 1, "edge|copy_edge|dup"),
        ];
        let mut diagnostics = Vec::new();
        validate_derived_edges(&edges, &node_ids, &mut diagnostics);
        assert!(
            diagnostics
                .iter()
                .any(|d| format!("{d:?}").contains("duplicate stable key")),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn dangling_endpoint_is_rejected() {
        let node_ids: BTreeSet<SemanticNodeId> = [SemanticNodeId(0)].into_iter().collect();
        // target node 9 is not in the node set.
        let edges = vec![edge(0, 0, 9, "edge|copy_edge|a")];
        let mut diagnostics = Vec::new();
        validate_derived_edges(&edges, &node_ids, &mut diagnostics);
        assert!(
            diagnostics
                .iter()
                .any(|d| format!("{d:?}").contains("dangling derived edge target")),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn non_contiguous_dense_id_is_rejected() {
        let node_ids: BTreeSet<SemanticNodeId> =
            [SemanticNodeId(0), SemanticNodeId(1)].into_iter().collect();
        // id 5 at index 0 — not contiguous.
        let edges = vec![edge(5, 0, 1, "edge|copy_edge|a")];
        let mut diagnostics = Vec::new();
        validate_derived_edges(&edges, &node_ids, &mut diagnostics);
        assert!(
            diagnostics
                .iter()
                .any(|d| format!("{d:?}").contains("dense id is not contiguous")),
            "{diagnostics:?}"
        );
    }

    // -----------------------------------------------------------------------
    // D-12 cycle detection.
    // -----------------------------------------------------------------------

    #[test]
    fn acyclic_constraint_set_admits_no_solver_summary_cycle() {
        // A linear chain a -> b -> c through a summary node `b` is acyclic: the
        // summary does not reach itself, so no cycle is reported.
        let constraints = vec![
            copy(1, 2, "copy|a-b"),
            copy(2, 3, "copy|b-c"),
            call_constraint(2, "call|b"),
        ];
        let mut diagnostics = Vec::new();
        let implicated = detect_solver_summary_cycle(&constraints, &mut diagnostics);
        assert_eq!(implicated, 0, "{diagnostics:?}");
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn solver_summary_solver_cycle_is_detected_and_bounded() {
        // A value-flow cycle a -> b -> a passing through a summary node `b`
        // (CallConstraint at node 2) is the structural shape of a
        // solver → summary → solver loop. It MUST be detected (and bounded, not
        // divergent): the check terminates and reports the implicated summary node.
        let constraints = vec![
            copy(1, 2, "copy|a-b"),
            copy(2, 1, "copy|b-a"),
            call_constraint(2, "call|b"),
        ];
        let mut diagnostics = Vec::new();
        let implicated = detect_solver_summary_cycle(&constraints, &mut diagnostics);
        assert_eq!(implicated, 1, "the summary node should be implicated");
        assert!(
            diagnostics
                .iter()
                .any(|d| format!("{d:?}").contains("solver-summary cycle detected and bounded")),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn cycle_detection_terminates_on_a_self_loop_summary() {
        // A degenerate self-loop b -> b on a summary node terminates (bounded by the
        // visited set) rather than diverging, and is reported.
        let constraints = vec![copy(2, 2, "copy|b-b"), call_constraint(2, "call|b")];
        let mut diagnostics = Vec::new();
        let implicated = detect_solver_summary_cycle(&constraints, &mut diagnostics);
        assert_eq!(implicated, 1);
    }

    #[test]
    fn non_summary_cycle_is_not_flagged_as_solver_summary_loop() {
        // A pure value-flow cycle a -> b -> a with NO summary (CallConstraint) node is
        // not a solver↔summary loop — the closed-input contract is about summaries
        // specifically — so it is not flagged here.
        let constraints = vec![copy(1, 2, "copy|a-b"), copy(2, 1, "copy|b-a")];
        let mut diagnostics = Vec::new();
        let implicated = detect_solver_summary_cycle(&constraints, &mut diagnostics);
        assert_eq!(implicated, 0);
        assert!(diagnostics.is_empty());
    }
}

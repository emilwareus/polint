//! D-09 deletion property for the neutral solver engine.

use crate::ids::{SemanticConstraintId, SemanticNodeId};
use crate::points_to::facts::{PointsToPrecision, PointsToStatus};
use crate::semantic_graph::constraints::{ConstraintFact, ConstraintKind};
use crate::solver::budget::SolverBudget;
use crate::solver::engine::derive_edges;
use polint_core::{StableKeyId, StableKeyInterner};

fn copy_constraint(
    interner: &StableKeyInterner,
    stable_key: &str,
    src: u64,
    dst: u64,
) -> ConstraintFact {
    ConstraintFact {
        id: SemanticConstraintId(0),
        kind: ConstraintKind::CopyEdge {
            dst: SemanticNodeId(dst),
            src: SemanticNodeId(src),
        },
        status: PointsToStatus::Present,
        precision: PointsToPrecision::FlowInsensitive,
        stable_key: interner.intern(stable_key),
    }
}

/// D-09: deleting ANY single contributing fact INVALIDATES the derived edge.
#[test]
fn deleting_any_contributing_fact_invalidates_the_derived_edge() {
    let interner = StableKeyInterner::default();
    let constraints = vec![
        copy_constraint(&interner, "copy|a-b", 1, 2),
        copy_constraint(&interner, "copy|b-c", 2, 3),
    ];
    let budget = SolverBudget::default();

    let baseline = derive_edges(&interner, &constraints, &budget);
    let derived = baseline
        .derived_edges
        .iter()
        .find(|e| e.source == SemanticNodeId(1) && e.target == SemanticNodeId(3))
        .expect("baseline derives transitive a -> c");
    assert_eq!(derived.provenance.contributing_len(), 2);

    let contributing_keys: Vec<StableKeyId> = derived
        .provenance
        .contributing_facts
        .iter()
        .map(|fact| fact.stable_key)
        .collect();
    assert_eq!(contributing_keys.len(), 2);

    for deleted in &contributing_keys {
        let remaining: Vec<ConstraintFact> = constraints
            .iter()
            .filter(|c| c.stable_key != *deleted)
            .cloned()
            .collect();
        assert_eq!(
            remaining.len(),
            1,
            "exactly one contributing constraint removed"
        );

        let rerun = derive_edges(&interner, &remaining, &budget);
        let reproduced = rerun
            .derived_edges
            .iter()
            .any(|e| e.source == SemanticNodeId(1) && e.target == SemanticNodeId(3));
        assert!(
            !reproduced,
            "deleting contributing fact `{}` must invalidate the derived a -> c edge",
            interner.resolve(*deleted),
        );
    }
}

//! Derived-edge provenance (GRAPH-04, D-08/D-09/D-10).
//!
//! Every solver-derived edge carries a [`DerivedEdgeProvenance`] recording the
//! three roadmap-named fields:
//!
//! 1. the **contributing fact IDs**, TOTALLY ORDERED BY STABLE ID (the Phase 42
//!    dedup total-order rule). Provenance references EXISTING stable identities by
//!    their stable key (composition over duplication — it does not mint a parallel
//!    identity space); the total order is the `stable_key_from_parts`
//!    length-prefixed, label-sorted recipe, so provenance is itself byte-stable and
//!    insensitive to the order contributing facts are discovered in.
//! 2. the producing **constraint kind**, sourced from
//!    [`crate::analysis::semantic_graph::constraints::ConstraintKind::as_str`] — a
//!    stable snake_case label reused wholesale (no parallel enum is minted).
//! 3. the **solver step** — the monotonic `u64` worklist step counter the Wave-1
//!    [`super::engine::SolverEngine`] maintains.
//!
//! Provenance must be SOUND and LOAD-BEARING, not decorative: the deletion property
//! test (D-09, in [`super::store`]/`tests`) proves that removing any single
//! contributing fact prevents the derived edge from being reproduced.
//!
//! All types are `pub(crate)` (D-16); nothing here reaches the public SDK surface.

use serde::{Deserialize, Serialize};

use crate::analysis::semantic_graph::constraints::ConstraintKind;
use crate::analysis_kernel::{FactFamily, stable_key_from_parts};

/// A reference to one EXISTING upstream fact that contributed to deriving an edge.
///
/// Provenance references existing stable identities — it carries the contributing
/// fact's `stable_key` rather than a run-local dense ID, so the reference survives
/// re-densification and is byte-stable across runs (D-08, composition over
/// duplication). The `stable_key` is built via `stable_key_from_parts`, which
/// length-prefixes and embeds the originating [`FactFamily`] label, so the family is
/// captured INSIDE the stable key (no separate non-serializable `FactFamily` field
/// is carried). The total order over a set of these is the lexicographic order of
/// `stable_key` (the Phase 42 dedup total-order rule).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct ContributingFact {
    /// The contributing fact's stable key — its EXISTING stable identity, built via
    /// the `stable_key_from_parts` recipe (which embeds the `FactFamily` label).
    /// Never a run-local dense ID.
    pub(crate) stable_key: String,
}

impl ContributingFact {
    /// Build a contributing-fact reference from a family + labeled key parts, using
    /// the canonical length-prefixed `stable_key_from_parts` recipe so the stable
    /// key is byte-identical to the upstream producer's key for the same identity.
    /// The `family` is folded into the stable key, not stored separately.
    pub(crate) fn from_parts(family: FactFamily, parts: &[(&str, String)]) -> Self {
        Self {
            stable_key: stable_key_from_parts(family, parts),
        }
    }
}

/// Full provenance for one solver-derived edge (GRAPH-04, D-08).
///
/// The three roadmap-named fields: contributing fact IDs (total-ordered by stable
/// ID), the producing constraint kind, and the monotonic solver step. Two
/// provenances built from the SAME contributing facts in different discovery order
/// are equal and serialize byte-identically, because [`DerivedEdgeProvenance::new`]
/// sorts and de-duplicates the contributing set by `stable_key` (which embeds the
/// originating family).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct DerivedEdgeProvenance {
    /// Contributing fact identities, TOTALLY ORDERED BY STABLE ID (sorted by
    /// `stable_key`, de-duplicated). This is the load-bearing set the deletion
    /// property test (D-09) operates on.
    pub(crate) contributing_facts: Vec<ContributingFact>,
    /// The producing `ConstraintKind`, as its stable snake_case label
    /// (`ConstraintKind::as_str()`, owned). Reuses the existing vocabulary; no
    /// parallel enum is minted. Owned (`String`) rather than `&'static str` so the
    /// provenance — and the derived-edge fact that carries it — is `Deserialize`.
    pub(crate) constraint_kind: String,
    /// The monotonic `u64` solver step (the engine's worklist step counter) at
    /// which this edge was derived.
    pub(crate) solver_step: u64,
}

impl DerivedEdgeProvenance {
    /// Build provenance from an UNORDERED set of contributing facts, the producing
    /// constraint kind, and the solver step.
    ///
    /// The contributing facts are sorted by `(stable_key, family)` and de-duplicated
    /// so the result is byte-stable regardless of the order facts were discovered in
    /// (the Phase 42 total-order rule). The constraint kind is captured as its stable
    /// label via [`ConstraintKind::as_str`] — referencing the existing vocabulary
    /// rather than duplicating it.
    pub(crate) fn new(
        contributing_facts: impl IntoIterator<Item = ContributingFact>,
        constraint_kind: &ConstraintKind,
        solver_step: u64,
    ) -> Self {
        let mut contributing_facts: Vec<ContributingFact> =
            contributing_facts.into_iter().collect();
        // Total order by stable ID (the stable_key embeds the family), then
        // de-duplicate so a fact referenced twice does not perturb the byte layout.
        contributing_facts.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        contributing_facts.dedup();
        Self {
            contributing_facts,
            constraint_kind: constraint_kind.as_str().to_string(),
            solver_step,
        }
    }

    /// The number of contributing facts this derived edge depends on.
    pub(crate) fn contributing_len(&self) -> usize {
        self.contributing_facts.len()
    }

    /// A stable-key fragment summarizing the provenance, suitable for embedding in a
    /// derived-edge fact's `stable_key`. Built from the totally-ordered contributing
    /// stable keys + the constraint kind (the solver step is a run-local progress
    /// counter and is intentionally excluded from the stable key so two byte-equal
    /// derivations that converge at different steps still dedup).
    pub(crate) fn stable_key_fragment(&self) -> String {
        let mut fragment = self.constraint_kind.clone();
        for fact in &self.contributing_facts {
            fragment.push('|');
            fragment.push_str(&fact.stable_key);
        }
        fragment
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ids::SemanticNodeId;

    fn copy_edge() -> ConstraintKind {
        ConstraintKind::CopyEdge {
            dst: SemanticNodeId(1),
            src: SemanticNodeId(2),
        }
    }

    fn fact(name: &str) -> ContributingFact {
        ContributingFact::from_parts(
            FactFamily::PointsToConstraint,
            &[("constraint", name.to_string())],
        )
    }

    #[test]
    fn provenance_is_shuffle_stable_in_contributing_order() {
        // Provenance built from the same contributing facts in DIFFERENT input order
        // is equal and serializes byte-identically (total-order by stable ID).
        let forward = DerivedEdgeProvenance::new(
            vec![fact("addr-a"), fact("copy"), fact("field-store")],
            &copy_edge(),
            7,
        );
        let reversed = DerivedEdgeProvenance::new(
            vec![fact("field-store"), fact("copy"), fact("addr-a")],
            &copy_edge(),
            7,
        );

        assert_eq!(forward, reversed);
        assert_eq!(
            serde_json::to_string(&forward).expect("serialize forward"),
            serde_json::to_string(&reversed).expect("serialize reversed"),
        );
    }

    #[test]
    fn provenance_dedups_repeated_contributing_facts() {
        let with_dup = DerivedEdgeProvenance::new(
            vec![fact("copy"), fact("copy"), fact("addr-a")],
            &copy_edge(),
            3,
        );
        assert_eq!(with_dup.contributing_len(), 2);
    }

    #[test]
    fn provenance_captures_constraint_kind_label_and_step() {
        let provenance = DerivedEdgeProvenance::new(vec![fact("copy")], &copy_edge(), 42);
        assert_eq!(provenance.constraint_kind, "copy_edge");
        assert_eq!(provenance.solver_step, 42);
    }

    #[test]
    fn stable_key_fragment_is_independent_of_input_order() {
        let a = DerivedEdgeProvenance::new(vec![fact("a"), fact("b")], &copy_edge(), 1);
        // A different solver step must NOT change the stable-key fragment (the step
        // is run-local progress, not identity).
        let b = DerivedEdgeProvenance::new(vec![fact("b"), fact("a")], &copy_edge(), 99);
        assert_eq!(a.stable_key_fragment(), b.stable_key_fragment());
    }
}

//! Derived-edge fact family (GRAPH-04, D-06/D-08).
//!
//! A [`DerivedEdgeFact`] is one edge the unified solver derived from the frontend
//! constraint vocabulary. It mirrors `semantic_graph::constraints::ConstraintFact`
//! shape (`{ id, ..., status, precision, stable_key }`) and carries, in addition,
//! the [`DerivedEdgeProvenance`] (D-08) recording the contributing facts, producing
//! `ConstraintKind`, and solver step.
//!
//! It reuses the shared `points_to::facts::{PointsToStatus, PointsToPrecision}`
//! status/precision vocabulary rather than minting redundant enums (composition over
//! duplication). The dense `id` is a post-normalization read concern only and is
//! excluded from resolved-text stable payloads (D-06).
//!
//! **Precision ceiling (D-06).** Derived edges are an OVER-APPROXIMATION; they reject
//! the exact precision tier. [`derived_edge_precision_ceiling`] maps the points-to
//! precision onto the kernel [`FactPrecision`] vocabulary and is asserted to NEVER
//! return [`FactPrecision::Exact`] — budget exhaustion and conservative derivation
//! surface honestly, never as a falsely-exact edge.
//!
//! Identity fields are interned [`StableKeyId`]s. Wire/cache/test byte stability must
//! go through [`DerivedEdgeFact::stable_payload`] (resolved text), never raw numeric
//! interner ids.

use serde::Serialize;

use crate::analysis::ids::{DerivedEdgeId, SemanticNodeId};
use crate::analysis::points_to::facts::{PointsToPrecision, PointsToStatus};
use crate::analysis_kernel::FactPrecision;
use crate::core::{StableKeyId, StableKeyInterner};

use super::provenance::{DerivedEdgeProvenance, DerivedEdgeProvenanceStablePayload};

/// One solver-derived edge `source -> target` with full provenance (GRAPH-04).
///
/// The dense `id` is a post-normalization read concern only and is omitted from
/// resolved-text stable payloads. The `stable_key` is built from the referenced
/// endpoints + the provenance fragment (never run-local dense IDs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DerivedEdgeFact {
    /// Run-local dense handle, assigned only after the stable-key sort (D-08).
    pub(crate) id: DerivedEdgeId,
    /// The edge's source endpoint (a unified `SemanticNodeId`).
    pub(crate) source: SemanticNodeId,
    /// The edge's target endpoint (a unified `SemanticNodeId`).
    pub(crate) target: SemanticNodeId,
    /// Honest derivation status, reusing the shared points-to vocabulary (D-06):
    /// `Present` for a converged edge, `BudgetExceeded` when a budget ceiling was
    /// hit during derivation.
    pub(crate) status: PointsToStatus,
    /// Honest precision, reusing the shared points-to vocabulary. A derived edge is
    /// at most flow-insensitive/heuristic — never an exact claim (the precision
    /// ceiling, D-06; see [`derived_edge_precision_ceiling`]).
    pub(crate) precision: PointsToPrecision,
    /// Built from the referenced endpoints + the provenance fragment, never run-local
    /// dense IDs. Populated by the solver store.
    pub(crate) stable_key: StableKeyId,
    /// The edge's provenance (D-08): contributing facts (total-ordered by resolved
    /// stable text), producing constraint kind, and solver step.
    pub(crate) provenance: DerivedEdgeProvenance,
}

/// Resolved-text wire/cache payload for one derived edge. Never carries numeric
/// interner ids; requires an interner at construction time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct DerivedEdgeStablePayload {
    pub(crate) source: SemanticNodeId,
    pub(crate) target: SemanticNodeId,
    pub(crate) status: PointsToStatus,
    pub(crate) precision: PointsToPrecision,
    #[serde(rename = "stable_key")]
    pub(crate) stable_key_text: String,
    pub(crate) provenance: DerivedEdgeProvenanceStablePayload,
}

impl DerivedEdgeFact {
    /// Returns `true` iff this derived edge's precision honors the D-06 ceiling
    /// (never exact). Used by the store's referential validation.
    pub(crate) fn honors_precision_ceiling(&self) -> bool {
        derived_edge_precision_ceiling(self.precision) != FactPrecision::Exact
    }

    /// Project this edge into a resolved-text stable payload (dense `id` omitted).
    pub(crate) fn stable_payload(&self, interner: &StableKeyInterner) -> DerivedEdgeStablePayload {
        DerivedEdgeStablePayload {
            source: self.source,
            target: self.target,
            status: self.status,
            precision: self.precision,
            stable_key_text: interner.resolve(self.stable_key).to_string(),
            provenance: self.provenance.stable_payload(interner),
        }
    }
}

/// Serialize derived edges as resolved-text stable payloads (never numeric ids).
pub(crate) fn serialize_derived_edges_stable(
    interner: &StableKeyInterner,
    edges: &[DerivedEdgeFact],
) -> Result<String, serde_json::Error> {
    let payload: Vec<DerivedEdgeStablePayload> = edges
        .iter()
        .map(|edge| edge.stable_payload(interner))
        .collect();
    serde_json::to_string(&payload)
}

/// Maps a derived edge's points-to precision onto the kernel [`FactPrecision`]
/// vocabulary, ENFORCING the D-06 precision ceiling: a derived edge is an
/// over-approximation, so it can never be `FactPrecision::Exact`. The most-precise
/// tier a derived edge may claim is `SetupAware`; everything weaker maps to
/// `Heuristic`/`Unresolved`/`Unsupported`. The exact tier is unreachable by
/// construction (no arm returns it), which the unit tests lock.
pub(crate) fn derived_edge_precision_ceiling(precision: PointsToPrecision) -> FactPrecision {
    match precision {
        // The most precise a SOLVER-DERIVED edge may claim: setup-aware, never exact.
        PointsToPrecision::FlowInsensitive | PointsToPrecision::LocalFlowSensitive => {
            FactPrecision::SetupAware
        }
        PointsToPrecision::SummaryProjected | PointsToPrecision::Heuristic => {
            FactPrecision::Heuristic
        }
        PointsToPrecision::Unknown => FactPrecision::Unresolved,
        PointsToPrecision::Unsupported => FactPrecision::Unsupported,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::semantic_graph::constraints::ConstraintKind;
    use crate::analysis::solver::provenance::ContributingFact;
    use crate::analysis_kernel::FactFamily;

    fn provenance() -> DerivedEdgeProvenance {
        let interner = crate::core::AnalysisDb::new().stable_key_interner();
        DerivedEdgeProvenance::new(
            &interner,
            vec![ContributingFact::from_parts(
                &interner,
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

    #[test]
    fn derived_edge_precision_ceiling_never_returns_exact() {
        // Exhaustive over every PointsToPrecision: NONE maps to FactPrecision::Exact.
        for precision in [
            PointsToPrecision::FlowInsensitive,
            PointsToPrecision::LocalFlowSensitive,
            PointsToPrecision::SummaryProjected,
            PointsToPrecision::Heuristic,
            PointsToPrecision::Unknown,
            PointsToPrecision::Unsupported,
        ] {
            assert_ne!(
                derived_edge_precision_ceiling(precision),
                FactPrecision::Exact,
                "derived edge precision must never be exact (D-06): {precision:?}"
            );
        }
    }

    #[test]
    fn derived_edge_honors_precision_ceiling() {
        let edge = DerivedEdgeFact {
            id: DerivedEdgeId(0),
            source: SemanticNodeId(1),
            target: SemanticNodeId(2),
            status: PointsToStatus::Present,
            precision: PointsToPrecision::FlowInsensitive,
            stable_key: crate::core::stable_key_for_test("edge|copy_edge|a"),
            provenance: provenance(),
        };
        assert!(edge.honors_precision_ceiling());
    }

    #[test]
    fn dense_id_is_omitted_from_stable_payload() {
        let interner = crate::core::test_stable_key_interner();
        let edge = DerivedEdgeFact {
            id: DerivedEdgeId(99),
            source: SemanticNodeId(1),
            target: SemanticNodeId(2),
            status: PointsToStatus::Present,
            precision: PointsToPrecision::FlowInsensitive,
            stable_key: crate::core::stable_key_for_test("edge|copy_edge|a"),
            provenance: provenance(),
        };
        let json = serde_json::to_string(&edge.stable_payload(&interner)).expect("serialize");
        assert!(!json.contains("\"id\""));
        assert!(json.contains("edge|copy_edge|a"));
        assert!(!json.contains(&format!("{}", edge.stable_key.0)));
    }
}

//! JS/TS function-token callsite dispatch.
//!
//! A concrete token set at a callsite becomes conservative solver
//! `DerivedEdgeFact` rows. The source is the caller function semantic node, the
//! target is the function-token semantic node, and provenance records the callsite
//! constraint plus every token-flow fact that carried the token to that callsite.

use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::ids::{DerivedEdgeId, SemanticNodeId};
use crate::analysis::points_to::facts::{PointsToPrecision, PointsToStatus};
use crate::analysis::semantic_graph::constraints::ConstraintKind;
use crate::analysis::solver::engine::{weakest_precision, weakest_status};
use crate::analysis::solver::facts::DerivedEdgeFact;
use crate::analysis::solver::provenance::{ContributingFact, DerivedEdgeProvenance};
use crate::analysis_kernel::{FactFamily, stable_key_from_parts};

use super::inputs::TsTokenCallsite;

/// Outcome of resolving one token-backed JS/TS callsite.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TokenCallsiteResolution {
    pub(crate) edges: Vec<DerivedEdgeFact>,
    pub(crate) candidate_cap_exceeded: bool,
}

/// Turn concrete function tokens at `callsite` into derived call edges.
///
/// `token_candidates` is keyed by callee semantic function node and carries the
/// stable facts that contributed that token. Sentinel states are handled by the
/// caller; this function accepts concrete tokens only.
pub(crate) fn resolve_token_callsite(
    callsite: &TsTokenCallsite,
    token_candidates: &BTreeMap<SemanticNodeId, BTreeSet<String>>,
    handoff_keys: &[String],
    max_candidates_per_callsite: usize,
    solver_step: u64,
) -> TokenCallsiteResolution {
    let mut candidate_cap_exceeded = false;
    let mut edges = Vec::new();

    for (index, (&target, contributing_keys)) in token_candidates.iter().enumerate() {
        if index >= max_candidates_per_callsite {
            candidate_cap_exceeded = true;
            break;
        }

        let status = weakest_status(PointsToStatus::Present, PointsToStatus::Present);
        let precision = weakest_precision(
            PointsToPrecision::Heuristic,
            PointsToPrecision::FlowInsensitive,
        );

        let mut contributing = vec![
            ContributingFact {
                stable_key: callsite.constraint_stable_key.clone(),
            },
            ContributingFact {
                stable_key: callsite.callsite_stable_key.clone(),
            },
        ];
        contributing.extend(
            handoff_keys
                .iter()
                .cloned()
                .map(|stable_key| ContributingFact { stable_key }),
        );
        contributing.extend(
            contributing_keys
                .iter()
                .cloned()
                .map(|stable_key| ContributingFact { stable_key }),
        );

        let provenance = DerivedEdgeProvenance::new(
            contributing,
            &ConstraintKind::CallConstraint {
                callsite: callsite.callsite_node,
            },
            solver_step,
        );
        let stable_key = stable_key_from_parts(
            FactFamily::SolverDerivedEdge,
            &[
                ("source", callsite.caller_node.0.to_string()),
                ("target", target.0.to_string()),
                ("provenance", provenance.stable_key_fragment()),
            ],
        );
        edges.push(DerivedEdgeFact {
            id: DerivedEdgeId(0),
            source: callsite.caller_node,
            target,
            status,
            precision,
            stable_key,
            provenance,
        });
    }

    TokenCallsiteResolution {
        edges,
        candidate_cap_exceeded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::solver::facts::derived_edge_precision_ceiling;
    use crate::analysis_kernel::FactPrecision;

    #[test]
    fn resolved_token_callsite_emits_solver_edge_with_provenance() {
        let callsite = callsite();
        let tokens = BTreeMap::from([(
            SemanticNodeId(2),
            BTreeSet::from(["function:target".to_string(), "copy:alias".to_string()]),
        )]);

        let resolution =
            resolve_token_callsite(&callsite, &tokens, &["binding:token".to_string()], 8, 11);

        assert!(!resolution.candidate_cap_exceeded);
        assert_eq!(resolution.edges.len(), 1);
        let edge = &resolution.edges[0];
        assert_eq!(edge.source, SemanticNodeId(1));
        assert_eq!(edge.target, SemanticNodeId(2));
        assert_eq!(edge.provenance.constraint_kind, "call_constraint");
        assert_eq!(edge.provenance.solver_step, 11);
        let contributing = contributing_keys(edge);
        assert!(contributing.contains("constraint:call"));
        assert!(contributing.contains("callsite:alias"));
        assert!(contributing.contains("binding:token"));
        assert!(contributing.contains("function:target"));
        assert!(contributing.contains("copy:alias"));
    }

    #[test]
    fn token_edges_never_claim_exact_precision() {
        let callsite = callsite();
        let tokens = BTreeMap::from([(SemanticNodeId(2), BTreeSet::from(["token".to_string()]))]);

        let resolution = resolve_token_callsite(&callsite, &tokens, &[], 8, 1);

        assert_ne!(
            derived_edge_precision_ceiling(resolution.edges[0].precision),
            FactPrecision::Exact
        );
    }

    #[test]
    fn removing_token_flow_evidence_changes_edge_stable_key() {
        let callsite = callsite();
        let with_copy = BTreeMap::from([(
            SemanticNodeId(2),
            BTreeSet::from(["function:target".to_string(), "copy:alias".to_string()]),
        )]);
        let without_copy = BTreeMap::from([(
            SemanticNodeId(2),
            BTreeSet::from(["function:target".to_string()]),
        )]);

        let first = resolve_token_callsite(&callsite, &with_copy, &[], 8, 1);
        let second = resolve_token_callsite(&callsite, &without_copy, &[], 8, 1);

        assert_ne!(first.edges[0].stable_key, second.edges[0].stable_key);
    }

    #[test]
    fn candidate_cap_latches_and_keeps_precap_edges() {
        let callsite = callsite();
        let tokens = BTreeMap::from([
            (SemanticNodeId(2), BTreeSet::from(["token:a".to_string()])),
            (SemanticNodeId(3), BTreeSet::from(["token:b".to_string()])),
        ]);

        let resolution = resolve_token_callsite(&callsite, &tokens, &[], 1, 1);

        assert!(resolution.candidate_cap_exceeded);
        assert_eq!(resolution.edges.len(), 1);
        assert_eq!(resolution.edges[0].target, SemanticNodeId(2));
    }

    fn callsite() -> TsTokenCallsite {
        TsTokenCallsite {
            caller_node: SemanticNodeId(1),
            callsite_node: SemanticNodeId(9),
            callsite_stable_key: "callsite:alias".to_string(),
            constraint_stable_key: "constraint:call".to_string(),
        }
    }

    fn contributing_keys(edge: &DerivedEdgeFact) -> BTreeSet<String> {
        edge.provenance
            .contributing_facts
            .iter()
            .map(|fact| fact.stable_key.clone())
            .collect()
    }
}

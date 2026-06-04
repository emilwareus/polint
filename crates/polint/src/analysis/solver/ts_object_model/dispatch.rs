//! JS/TS object-model property-backed call dispatch.

use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::ids::DerivedEdgeId;
use crate::analysis::points_to::facts::{PointsToPrecision, PointsToStatus};
use crate::analysis::semantic_graph::constraints::ConstraintKind;
use crate::analysis::solver::engine::{weakest_precision, weakest_status};
use crate::analysis::solver::facts::DerivedEdgeFact;
use crate::analysis::solver::provenance::{ContributingFact, DerivedEdgeProvenance};
use crate::analysis_kernel::{FactFamily, stable_key_from_parts};

use super::fixpoint::TsObjectValueToken;
use super::inputs::TsObjectPropertyRead;

/// Outcome of resolving one object/property-backed callsite.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ObjectCallsiteResolution {
    pub(crate) edges: Vec<DerivedEdgeFact>,
    pub(crate) candidate_cap_exceeded: bool,
}

/// Turn callable tokens loaded from a property bucket into solver-derived call edges.
pub(crate) fn resolve_object_callsite(
    read: &TsObjectPropertyRead,
    bucket_tokens: &BTreeMap<TsObjectValueToken, BTreeSet<String>>,
    handoff_keys: &[String],
    max_candidates_per_callsite: usize,
    solver_step: u64,
) -> ObjectCallsiteResolution {
    let Some(callsite_node) = read.callsite_node else {
        return ObjectCallsiteResolution::default();
    };
    let Some(caller_node) = read.caller_node else {
        return ObjectCallsiteResolution::default();
    };

    let mut candidate_cap_exceeded = false;
    let mut edges = Vec::new();
    let mut emitted_candidates = 0_usize;

    for (token, contributing_keys) in bucket_tokens {
        let TsObjectValueToken::Function(target) = *token else {
            continue;
        };
        if emitted_candidates >= max_candidates_per_callsite {
            candidate_cap_exceeded = true;
            break;
        }
        emitted_candidates += 1;

        let status = weakest_status(PointsToStatus::Present, PointsToStatus::Present);
        let precision = weakest_precision(
            PointsToPrecision::Heuristic,
            PointsToPrecision::FlowInsensitive,
        );

        let mut contributing = vec![
            ContributingFact {
                stable_key: read.constraint_stable_key.clone(),
            },
            ContributingFact {
                stable_key: read.stable_key.clone(),
            },
        ];
        if let Some(callsite_stable_key) = &read.callsite_stable_key {
            contributing.push(ContributingFact {
                stable_key: callsite_stable_key.clone(),
            });
        }
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
                callsite: callsite_node,
            },
            solver_step,
        );
        let stable_key = stable_key_from_parts(
            FactFamily::SolverDerivedEdge,
            &[
                ("source", caller_node.0.to_string()),
                ("target", target.0.to_string()),
                ("provenance", provenance.stable_key_fragment()),
            ],
        );
        edges.push(DerivedEdgeFact {
            id: DerivedEdgeId(0),
            source: caller_node,
            target,
            status,
            precision,
            stable_key,
            provenance,
        });
    }

    ObjectCallsiteResolution {
        edges,
        candidate_cap_exceeded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ids::SemanticNodeId;
    use crate::analysis::solver::facts::derived_edge_precision_ceiling;
    use crate::analysis_kernel::FactPrecision;

    #[test]
    fn resolved_object_callsite_emits_solver_edge_with_property_provenance() {
        let read = read();
        let tokens = BTreeMap::from([(
            TsObjectValueToken::Function(SemanticNodeId(2)),
            BTreeSet::from([
                "allocation:holder".to_string(),
                "write:target".to_string(),
                "function:target".to_string(),
            ]),
        )]);

        let resolution =
            resolve_object_callsite(&read, &tokens, &["binding:property".to_string()], 8, 11);

        assert!(!resolution.candidate_cap_exceeded);
        assert_eq!(resolution.edges.len(), 1);
        let edge = &resolution.edges[0];
        assert_eq!(edge.source, SemanticNodeId(1));
        assert_eq!(edge.target, SemanticNodeId(2));
        assert_eq!(edge.provenance.constraint_kind, "call_constraint");
        let contributing = contributing_keys(edge);
        assert!(contributing.contains("read:target"));
        assert!(contributing.contains("callsite:target"));
        assert!(contributing.contains("binding:property"));
        assert!(contributing.contains("allocation:holder"));
        assert!(contributing.contains("write:target"));
        assert!(contributing.contains("function:target"));
    }

    #[test]
    fn object_edges_never_claim_exact_precision() {
        let tokens = BTreeMap::from([(
            TsObjectValueToken::Function(SemanticNodeId(2)),
            BTreeSet::from(["write:target".to_string()]),
        )]);

        let resolution = resolve_object_callsite(&read(), &tokens, &[], 8, 1);

        assert_ne!(
            derived_edge_precision_ceiling(resolution.edges[0].precision),
            FactPrecision::Exact
        );
    }

    #[test]
    fn removing_property_write_evidence_changes_edge_stable_key() {
        let with_write = BTreeMap::from([(
            TsObjectValueToken::Function(SemanticNodeId(2)),
            BTreeSet::from(["write:target".to_string(), "function:target".to_string()]),
        )]);
        let without_write = BTreeMap::from([(
            TsObjectValueToken::Function(SemanticNodeId(2)),
            BTreeSet::from(["function:target".to_string()]),
        )]);

        let first = resolve_object_callsite(&read(), &with_write, &[], 8, 1);
        let second = resolve_object_callsite(&read(), &without_write, &[], 8, 1);

        assert_ne!(first.edges[0].stable_key, second.edges[0].stable_key);
    }

    #[test]
    fn candidate_cap_latches_and_keeps_precap_edges() {
        let tokens = BTreeMap::from([
            (
                TsObjectValueToken::Function(SemanticNodeId(2)),
                BTreeSet::from(["token:a".to_string()]),
            ),
            (
                TsObjectValueToken::Function(SemanticNodeId(3)),
                BTreeSet::from(["token:b".to_string()]),
            ),
        ]);

        let resolution = resolve_object_callsite(&read(), &tokens, &[], 1, 1);

        assert!(resolution.candidate_cap_exceeded);
        assert_eq!(resolution.edges.len(), 1);
        assert_eq!(resolution.edges[0].target, SemanticNodeId(2));
    }

    fn read() -> TsObjectPropertyRead {
        TsObjectPropertyRead {
            base_object: SemanticNodeId(10),
            base_is_this: false,
            field: "static:target".to_string(),
            destination_node: SemanticNodeId(20),
            callsite_node: Some(SemanticNodeId(9)),
            caller_node: Some(SemanticNodeId(1)),
            callsite_stable_key: Some("callsite:target".to_string()),
            constraint_stable_key: "read:target".to_string(),
            stable_key: "read:target".to_string(),
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

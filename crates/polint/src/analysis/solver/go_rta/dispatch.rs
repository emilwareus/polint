//! RTA dispatch resolver: interface-invoke-by-method-set + func-value-by-signature
//! (D-06, D-08, D-09).
//!
//! [`resolve_callsite`] resolves one `UnresolvedDynamic` Go callsite to its candidate
//! callees, FILTERED by the current instantiated runtime-type set (the filter that
//! makes this RTA, not coarse CHA), and emits each resolved edge as a
//! [`DerivedEdgeFact`] `caller-function-node -> callee-function-node` in the unified
//! vocabulary (D-04). Edges reuse the shared `points_to` status/precision vocabulary,
//! carry [`DerivedEdgeProvenance`] listing the contributing callsite + dispatch +
//! method-set + instantiated-type facts (so the deletion-invalidation property holds,
//! D-09), and never claim exact precision (D-08 — the store's `derived_edge_precision_
//! ceiling` is the hard gate; we floor RTA edges at `Heuristic` because dynamic
//! dispatch is an over-approximation).

use std::collections::BTreeSet;

use crate::analysis::ids::DerivedEdgeId;
use crate::analysis::points_to::facts::{PointsToPrecision, PointsToStatus};
use crate::analysis::semantic_graph::constraints::ConstraintKind;
use crate::analysis::solver::engine::{weakest_precision, weakest_status};
use crate::analysis::solver::facts::DerivedEdgeFact;
use crate::analysis::solver::provenance::{ContributingFact, DerivedEdgeProvenance};
use crate::analysis_kernel::{FactFamily, stable_key_from_parts};

use super::inputs::{GoRtaCallsite, GoRtaInputs};

/// Outcome of resolving one callsite: the (possibly empty) resolved edges and whether
/// the per-callsite candidate cap was exceeded (a run-level exhaustion signal, D-13).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CallsiteResolution {
    pub(crate) edges: Vec<DerivedEdgeFact>,
    pub(crate) candidate_cap_exceeded: bool,
}

/// Resolve one dynamic callsite to its candidate-callee edges under the current
/// instantiated-type / address-taken sets (D-06).
///
/// - Interface invoke (`interface_method` set): candidate callees are the concrete
///   methods whose receiver type is in `instantiated` AND whose method-set contains
///   the invoked method. The instantiated-type filter is the RTA discriminant.
/// - Func-value call (`signature` set): candidate callees are address-taken functions
///   whose signature matches.
///
/// Honest-unresolved (D-08): a callsite whose interface type / method has no
/// method-set match — or whose caller/callee has no semantic node — contributes NO
/// edge; it stays an unresolved obligation rather than a fabricated edge. When the
/// candidate set would exceed `max_candidates_per_callsite`, resolution stops and
/// `candidate_cap_exceeded` latches (edges resolved before the cap keep their honest
/// status — review finding #R1).
pub(crate) fn resolve_callsite(
    callsite: &GoRtaCallsite,
    inputs: &GoRtaInputs,
    instantiated: &BTreeSet<String>,
    address_taken: &BTreeSet<String>,
    max_candidates_per_callsite: usize,
    solver_step: u64,
) -> CallsiteResolution {
    // The edge SOURCE is the caller FUNCTION node (D-04), not the callsite node; the
    // callsite node is recorded via the producing CallConstraint in provenance. No
    // caller node => no honest edge anchor.
    let Some(&caller_node) = inputs.function_node.get(&callsite.caller) else {
        return CallsiteResolution::default();
    };

    // Gather candidate callees as (callee_node, callee_qualified, extra contributing
    // fact keys) — deterministic, deduplicated by callee qualified identity.
    let mut candidates: Vec<DispatchCandidate> = Vec::new();
    let mut candidate_cap_exceeded = false;

    if let Some(method) = callsite.interface_method.as_deref() {
        collect_interface_candidates(
            method,
            inputs,
            instantiated,
            max_candidates_per_callsite,
            &mut candidates,
            &mut candidate_cap_exceeded,
        );
    } else if let Some(signature) = callsite.signature.as_deref() {
        collect_func_value_candidates(
            signature,
            inputs,
            address_taken,
            max_candidates_per_callsite,
            &mut candidates,
            &mut candidate_cap_exceeded,
        );
    }

    let mut edges = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        // Worst-trust status/precision across the adopted derivation (D-09). The
        // contributing frontend facts are all "present", so status is Present; an
        // RTA-resolved dynamic edge is an over-approximation, so precision is floored
        // at Heuristic via the worst-of combine (never exact — D-08).
        let status = weakest_status(PointsToStatus::Present, PointsToStatus::Present);
        let precision = weakest_precision(
            PointsToPrecision::Heuristic,
            PointsToPrecision::FlowInsensitive,
        );

        // Contributing facts: the callsite + dispatch detail (always) + whatever
        // method-set / instantiated-type / address-taken facts justified THIS callee
        // (so deleting any of them does not reproduce the edge — D-09).
        let mut contributing: Vec<ContributingFact> = vec![
            ContributingFact {
                stable_key: callsite.callsite_stable_key.clone(),
            },
            ContributingFact {
                stable_key: callsite.dispatch_stable_key.clone(),
            },
        ];
        contributing.extend(
            candidate
                .contributing_keys
                .into_iter()
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
                ("source", caller_node.0.to_string()),
                ("target", candidate.node.0.to_string()),
                ("provenance", provenance.stable_key_fragment()),
            ],
        );
        edges.push(DerivedEdgeFact {
            id: DerivedEdgeId(0),
            source: caller_node,
            target: candidate.node,
            status,
            precision,
            stable_key,
            provenance,
        });
    }

    CallsiteResolution {
        edges,
        candidate_cap_exceeded,
    }
}

/// One resolved candidate callee with the contributing-fact keys that justified it.
struct DispatchCandidate {
    node: crate::analysis::ids::SemanticNodeId,
    contributing_keys: Vec<String>,
}

/// Interface-invoke candidates: concrete methods named `method` whose receiver type
/// is instantiated AND whose method-set contains `method` (D-06). The instantiated
/// filter is what makes it RTA. Deterministic by `(type_name, callee qualified)`.
fn collect_interface_candidates(
    method: &str,
    inputs: &GoRtaInputs,
    instantiated: &BTreeSet<String>,
    max_candidates_per_callsite: usize,
    candidates: &mut Vec<DispatchCandidate>,
    candidate_cap_exceeded: &mut bool,
) {
    // BTreeSet iteration is sorted, so candidate discovery is deterministic.
    for type_name in instantiated {
        // The instantiated type must declare `method` in its method-set; otherwise it
        // is not a dispatch target for this invoke (honest — no fabricated edge).
        let Some(methods) = inputs.method_sets.get(type_name) else {
            continue;
        };
        if !methods.contains(method) {
            continue;
        }
        let Some(concrete_methods) = inputs.methods_by_receiver.get(type_name) else {
            continue;
        };
        for concrete in concrete_methods {
            if concrete.method_name != method {
                continue;
            }
            if candidates.len() >= max_candidates_per_callsite {
                *candidate_cap_exceeded = true;
                return;
            }
            let mut contributing_keys = Vec::new();
            if let Some(key) = inputs.method_set_keys.get(type_name) {
                contributing_keys.push(key.clone());
            }
            if let Some(key) = inputs.instantiated_keys.get(type_name) {
                contributing_keys.push(key.clone());
            }
            candidates.push(DispatchCandidate {
                node: concrete.node,
                contributing_keys,
            });
        }
    }
}

/// Func-value candidates: address-taken functions whose signature matches the call
/// signature (D-06). Deterministic by address-taken qualified identity.
fn collect_func_value_candidates(
    signature: &str,
    inputs: &GoRtaInputs,
    address_taken: &BTreeSet<String>,
    max_candidates_per_callsite: usize,
    candidates: &mut Vec<DispatchCandidate>,
    candidate_cap_exceeded: &mut bool,
) {
    for function in address_taken {
        let Some(&node) = inputs.function_node.get(function) else {
            continue;
        };
        // Signature match: the address-taken function's signature must equal the call
        // signature. Functions with no recorded signature cannot honestly match.
        let Some(function_signature) = inputs.function_signature.get(function) else {
            continue;
        };
        if function_signature != signature {
            continue;
        }
        if candidates.len() >= max_candidates_per_callsite {
            *candidate_cap_exceeded = true;
            return;
        }
        let mut contributing_keys = Vec::new();
        if let Some(key) = inputs.address_taken_keys.get(function) {
            contributing_keys.push(key.clone());
        }
        candidates.push(DispatchCandidate {
            node,
            contributing_keys,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    use super::super::inputs::{GoRtaCallsite, GoRtaInputs, GoRtaMethod};
    use crate::analysis::ids::SemanticNodeId;

    /// Two instantiated types both declare `Read` → two candidate callees. A
    /// per-callsite cap of 1 latches `candidate_cap_exceeded` and emits only the
    /// pre-cap edge (review finding #R1: edges resolved before the cap are honest).
    #[test]
    fn per_callsite_candidate_cap_latches_and_keeps_precap_edges() {
        let mut function_node = BTreeMap::new();
        function_node.insert("main.main".to_string(), SemanticNodeId(1));
        function_node.insert("(pkg.A).Read".to_string(), SemanticNodeId(3));
        function_node.insert("(pkg.B).Read".to_string(), SemanticNodeId(4));

        let mut method_sets = BTreeMap::new();
        method_sets.insert("pkg.A".to_string(), BTreeSet::from(["Read".to_string()]));
        method_sets.insert("pkg.B".to_string(), BTreeSet::from(["Read".to_string()]));
        let mut method_set_keys = BTreeMap::new();
        method_set_keys.insert("pkg.A".to_string(), "ms|A".to_string());
        method_set_keys.insert("pkg.B".to_string(), "ms|B".to_string());

        let mut methods_by_receiver = BTreeMap::new();
        methods_by_receiver.insert(
            "pkg.A".to_string(),
            vec![GoRtaMethod {
                method_name: "Read".to_string(),
                qualified: "(pkg.A).Read".to_string(),
                node: SemanticNodeId(3),
            }],
        );
        methods_by_receiver.insert(
            "pkg.B".to_string(),
            vec![GoRtaMethod {
                method_name: "Read".to_string(),
                qualified: "(pkg.B).Read".to_string(),
                node: SemanticNodeId(4),
            }],
        );

        let instantiated = BTreeSet::from(["pkg.A".to_string(), "pkg.B".to_string()]);
        let mut instantiated_keys = BTreeMap::new();
        instantiated_keys.insert("pkg.A".to_string(), "inst|A".to_string());
        instantiated_keys.insert("pkg.B".to_string(), "inst|B".to_string());

        let inputs = GoRtaInputs {
            method_sets,
            method_set_keys,
            instantiated: instantiated.clone(),
            instantiated_keys,
            methods_by_receiver,
            function_node,
            ..GoRtaInputs::default()
        };
        let callsite = GoRtaCallsite {
            caller: "main.main".to_string(),
            callsite_node: SemanticNodeId(2),
            callsite_stable_key: "cs|main".to_string(),
            interface_method: Some("Read".to_string()),
            signature: None,
            dispatch_stable_key: "dd|main".to_string(),
        };

        let resolution =
            resolve_callsite(&callsite, &inputs, &instantiated, &BTreeSet::new(), 1, 7);
        assert!(resolution.candidate_cap_exceeded, "cap of 1 must latch");
        assert_eq!(
            resolution.edges.len(),
            1,
            "only the pre-cap edge is emitted; it keeps its honest status"
        );
        assert!(resolution.edges[0].honors_precision_ceiling());
    }

    /// No caller node => no honest edge anchor (the source endpoint is missing).
    #[test]
    fn missing_caller_node_resolves_nothing() {
        let inputs = GoRtaInputs::default();
        let callsite = GoRtaCallsite {
            caller: "main.main".to_string(),
            callsite_node: SemanticNodeId(2),
            callsite_stable_key: "cs|main".to_string(),
            interface_method: Some("Read".to_string()),
            signature: None,
            dispatch_stable_key: "dd|main".to_string(),
        };
        let resolution = resolve_callsite(
            &callsite,
            &inputs,
            &BTreeSet::new(),
            &BTreeSet::new(),
            128,
            1,
        );
        assert!(resolution.edges.is_empty());
        assert!(!resolution.candidate_cap_exceeded);
    }
}

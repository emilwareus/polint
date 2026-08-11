//! Composition-root solver policies.
//!
//! The points-to policy contract and neutral Andersen composition live in
//! `polint-analysis`. This module retains only policies that adapt the facade's
//! concrete frontend snapshots: Go RTA and the TS/JS callsite projection.

pub(crate) use polint_analysis::solver::policy::{PolicyOutcome, SolverPolicy};

use super::budget::SolverBudget;
use super::go_rta::{GoRtaInputs, solve_go_rta};

use ts_points_to::budget_status;
pub(crate) use ts_points_to::{TsPointsToInputs, solve_ts_points_to};

/// The real Go RTA policy (GO-05). Owns a CLOSED snapshot of the Go RTA
/// inputs (reachability roots + the Go-frontend address-taken / instantiated-type /
/// dispatch facts + method-sets + callsites), mirroring the neutral points-to
/// policy's closed snapshot. [`SolverPolicy::solve`] runs the RTA fixpoint
/// ([`solve_go_rta`]) and returns the resolved call edges + budget status.
pub(crate) struct GoRtaPolicy {
    inputs: GoRtaInputs,
}

impl GoRtaPolicy {
    pub(crate) fn new(inputs: GoRtaInputs) -> Self {
        Self { inputs }
    }
}

impl SolverPolicy for GoRtaPolicy {
    fn id(&self) -> &'static str {
        "go_rta"
    }

    fn solve(
        &self,
        interner: &crate::core::StableKeyInterner,
        budget: &SolverBudget,
    ) -> PolicyOutcome {
        // Run the RTA fixpoint over the closed snapshot (composition over the engine
        // worklist, mirroring the neutral points-to policy's fold). The output is
        // already normalized.
        let output = solve_go_rta(interner, &self.inputs, budget);
        PolicyOutcome {
            points_to: None,
            derived_edges: output.derived_edges,
            budget_status: output.budget_status,
            budget_reasons: output.budget_reasons,
            steps: 0,
        }
    }
}

/// JS/TS points-to policy. It projects the semantic graph into the shared
/// field-sensitive Andersen solver and emits indirect call edges from solved sets.
pub(crate) struct TsPointsToPolicy {
    inputs: TsPointsToInputs,
}

impl TsPointsToPolicy {
    pub(crate) fn new(inputs: TsPointsToInputs) -> Self {
        Self { inputs }
    }
}

impl SolverPolicy for TsPointsToPolicy {
    fn id(&self) -> &'static str {
        "ts_points_to"
    }

    fn solve(
        &self,
        interner: &crate::core::StableKeyInterner,
        budget: &SolverBudget,
    ) -> PolicyOutcome {
        let output = solve_ts_points_to(interner, &self.inputs, budget);
        PolicyOutcome {
            points_to: None,
            derived_edges: output.derived_edges,
            budget_status: budget_status(&output.points_to),
            budget_reasons: output.points_to.budget_reasons,
            steps: 0,
        }
    }
}

mod ts_points_to {
    use std::collections::{BTreeMap, BTreeSet};

    use crate::analysis::ids::{
        DerivedEdgeId, ObjectTokenId, PointsToConstraintId, PtVarId, SemanticNodeId,
    };
    use crate::analysis::points_to::facts::{
        PointsToConstraintFact, PointsToConstraintKind, PointsToPrecision, PointsToStatus,
    };
    use crate::analysis::points_to::solver::{PointsToSolveResult, solve_points_to};
    use crate::analysis::semantic_graph::constraints::{ConstraintFact, ConstraintKind};
    use crate::analysis::semantic_graph::facts::NodeKind;
    use crate::analysis::solver::budget::{BudgetStatus, SolverBudget};
    use crate::analysis::solver::facts::DerivedEdgeFact;
    use crate::analysis::solver::provenance::{ContributingFact, DerivedEdgeProvenance};
    use crate::analysis_kernel::{FactFamily, stable_key_from_parts};
    use crate::core::AnalysisDb;
    use crate::core::StableKeyId;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct TsPointsToCallsite {
        caller_node: SemanticNodeId,
        callsite_node: SemanticNodeId,
        callsite_stable_key: StableKeyId,
        constraint_stable_key: StableKeyId,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub(crate) struct TsPointsToInputs {
        constraints: Vec<ConstraintFact>,
        callable_objects: BTreeMap<ObjectTokenId, (SemanticNodeId, StableKeyId)>,
        callsites: Vec<TsPointsToCallsite>,
    }

    impl TsPointsToInputs {
        pub(crate) fn from_db(db: &AnalysisDb) -> Self {
            let function_node_by_id = db
                .semantic_nodes()
                .iter()
                .filter_map(|node| match node.kind {
                    NodeKind::Function(function) => Some((function, node.id)),
                    _ => None,
                })
                .collect::<BTreeMap<_, _>>();
            let callsite_node_by_id = db
                .semantic_nodes()
                .iter()
                .filter_map(|node| match node.kind {
                    NodeKind::Callsite(callsite) => Some((callsite, node.id)),
                    _ => None,
                })
                .collect::<BTreeMap<_, _>>();
            let node_key_by_id = db
                .semantic_nodes()
                .iter()
                .map(|node| (node.id, node.stable_key))
                .collect::<BTreeMap<_, _>>();
            let constraint_key_by_callsite = db
                .semantic_constraints()
                .iter()
                .filter_map(|constraint| match constraint.kind {
                    ConstraintKind::CallConstraint { callsite } => {
                        Some((callsite, constraint.stable_key))
                    }
                    _ => None,
                })
                .collect::<BTreeMap<_, _>>();

            let callable_objects = db
                .semantic_nodes()
                .iter()
                .filter_map(|node| match node.kind {
                    NodeKind::Function(_) => {
                        Some((object_for_node(node.id), (node.id, node.stable_key)))
                    }
                    _ => None,
                })
                .collect();
            let callsites = db
                .call_sites()
                .iter()
                .filter(|site| site.language.is_ts_family())
                .filter_map(|site| {
                    let callsite_node = *callsite_node_by_id.get(&site.id)?;
                    let caller_node = *function_node_by_id.get(&site.caller)?;
                    Some(TsPointsToCallsite {
                        caller_node,
                        callsite_node,
                        callsite_stable_key: *node_key_by_id.get(&callsite_node)?,
                        constraint_stable_key: constraint_key_by_callsite
                            .get(&callsite_node)
                            .copied()
                            .unwrap_or(site.stable_key),
                    })
                })
                .collect();

            Self {
                constraints: db.semantic_constraints().to_vec(),
                callable_objects,
                callsites,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct TsPointsToSolveResult {
        pub(crate) points_to: PointsToSolveResult,
        pub(crate) derived_edges: Vec<DerivedEdgeFact>,
    }

    pub(crate) fn solve_ts_points_to(
        interner: &crate::core::StableKeyInterner,
        inputs: &TsPointsToInputs,
        budget: &SolverBudget,
    ) -> TsPointsToSolveResult {
        let constraints = project_constraints(interner, inputs);
        let points_to = solve_points_to(interner, &constraints, budget.points_to_budget());
        let sets = points_to
            .sets
            .iter()
            .map(|set| (set.variable, set))
            .collect::<BTreeMap<_, _>>();
        // Membership-only dedup by StableKeyId; emission order is assigned below via
        // `SolverOutput::normalized` (resolved-text sort), never allocation order.
        let mut edges = std::collections::HashMap::new();

        for callsite in &inputs.callsites {
            let Some(set) = sets.get(&var_for_node(callsite.callsite_node)) else {
                continue;
            };
            if set.status != PointsToStatus::Present {
                continue;
            }
            for object in &set.objects {
                let Some((target, target_stable_key)) = inputs.callable_objects.get(object) else {
                    continue;
                };
                let provenance = DerivedEdgeProvenance::new(
                    interner,
                    vec![
                        ContributingFact {
                            stable_key: callsite.constraint_stable_key,
                        },
                        ContributingFact {
                            stable_key: callsite.callsite_stable_key,
                        },
                        ContributingFact {
                            stable_key: *target_stable_key,
                        },
                        ContributingFact {
                            stable_key: set.stable_key,
                        },
                    ],
                    &ConstraintKind::CallConstraint {
                        callsite: callsite.callsite_node,
                    },
                    0,
                );
                let stable_key = stable_key_from_parts(
                    interner,
                    FactFamily::SolverDerivedEdge,
                    &[
                        ("source", callsite.caller_node.0.to_string()),
                        ("target", target.0.to_string()),
                        ("provenance", provenance.stable_key_fragment(interner)),
                    ],
                );
                edges.entry(stable_key).or_insert(DerivedEdgeFact {
                    id: DerivedEdgeId(0),
                    source: callsite.caller_node,
                    target: *target,
                    status: set.status,
                    precision: set.precision,
                    stable_key,
                    provenance,
                });
            }
        }

        // Direct/policy consumers must never observe allocation-order emission.
        let derived_edges = crate::analysis::solver::store::SolverOutput {
            derived_edges: edges.into_values().collect(),
            ..crate::analysis::solver::store::SolverOutput::default()
        }
        .normalized(interner)
        .derived_edges;

        TsPointsToSolveResult {
            points_to,
            derived_edges,
        }
    }

    fn project_constraints(
        interner: &crate::core::StableKeyInterner,
        inputs: &TsPointsToInputs,
    ) -> Vec<PointsToConstraintFact> {
        let mut kinds = BTreeSet::new();
        let callsite_by_source_key = inputs
            .constraints
            .iter()
            .filter_map(|constraint| match constraint.kind {
                ConstraintKind::CallConstraint { callsite } => {
                    Some((constraint.stable_key, callsite))
                }
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();

        for (&object, &(node, _)) in &inputs.callable_objects {
            kinds.insert(PointsToConstraintKind::AddressOf {
                dst: var_for_node(node),
                object,
            });
        }
        for constraint in &inputs.constraints {
            if constraint.status != PointsToStatus::Present {
                continue;
            }
            match &constraint.kind {
                ConstraintKind::CopyEdge { dst, src } => {
                    kinds.insert(PointsToConstraintKind::Copy {
                        dst: var_for_node(*dst),
                        src: var_for_node(*src),
                    });
                }
                ConstraintKind::Alloc { dst, object } => {
                    let object_token = object_for_node(*object);
                    kinds.insert(PointsToConstraintKind::AddressOf {
                        dst: var_for_node(*object),
                        object: object_token,
                    });
                    kinds.insert(PointsToConstraintKind::AddressOf {
                        dst: var_for_node(*dst),
                        object: object_token,
                    });
                }
                ConstraintKind::FieldLoad { dst, base, field } => {
                    kinds.insert(PointsToConstraintKind::FieldLoad {
                        dst: var_for_node(*dst),
                        base: var_for_node(*base),
                        field: field.clone(),
                    });
                    if let Some(callsite) = callsite_by_source_key.get(&constraint.stable_key) {
                        kinds.insert(PointsToConstraintKind::Copy {
                            dst: var_for_node(*callsite),
                            src: var_for_node(*dst),
                        });
                    }
                }
                ConstraintKind::FieldStore { base, field, src } => {
                    kinds.insert(PointsToConstraintKind::FieldStore {
                        base: var_for_node(*base),
                        field: field.clone(),
                        src: var_for_node(*src),
                    });
                }
                ConstraintKind::CallConstraint { .. }
                | ConstraintKind::ModelEdge { .. }
                | ConstraintKind::TypeConstraint { .. } => {}
            }
        }

        kinds
            .into_iter()
            .enumerate()
            .map(|(index, kind)| PointsToConstraintFact {
                id: PointsToConstraintId(index as u64),
                stable_key: stable_key_from_parts(
                    interner,
                    FactFamily::PointsToConstraint,
                    &[("kind", format!("{kind:?}"))],
                ),
                kind,
                status: PointsToStatus::Present,
                precision: PointsToPrecision::FlowInsensitive,
            })
            .collect()
    }

    fn var_for_node(node: SemanticNodeId) -> PtVarId {
        PtVarId(node.0)
    }

    fn object_for_node(node: SemanticNodeId) -> ObjectTokenId {
        ObjectTokenId(node.0)
    }

    pub(crate) fn budget_status(result: &PointsToSolveResult) -> BudgetStatus {
        BudgetStatus::from_points_to(result.budget_status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    use crate::analysis::ids::SemanticNodeId;
    use crate::analysis::solver::budget::BudgetStatus;
    use crate::analysis::solver::engine::SolverEngine;
    use crate::analysis::solver::go_rta::inputs::{GoRtaCallsite, GoRtaMethod};
    use polint_analysis::solver::policy::PointsToPolicy;

    #[test]
    fn ts_policy_id_is_stable() {
        let budget = SolverBudget::default();
        let ts = TsPointsToPolicy::new(TsPointsToInputs::default());
        let ts_outcome = ts.solve(&crate::core::test_stable_key_interner(), &budget);
        assert_eq!(ts.id(), "ts_points_to");
        assert!(ts_outcome.points_to.is_none());
        assert!(ts_outcome.derived_edges.is_empty());
        assert_eq!(ts_outcome.budget_status, BudgetStatus::WithinBudget);
    }
    #[test]
    fn go_rta_policy_derives_edges_from_a_resolvable_dispatch() {
        let interner = crate::core::test_stable_key_interner();
        // The real Go RTA policy now derives ≥1 edge from a resolvable interface
        // dispatch (an instantiated receiver whose method-set declares the method).
        let mut function_node = BTreeMap::new();
        function_node.insert("main.main".to_string(), SemanticNodeId(1));
        function_node.insert("(pkg.File).Read".to_string(), SemanticNodeId(3));

        let mut method_sets = BTreeMap::new();
        method_sets.insert("pkg.File".to_string(), BTreeSet::from(["Read".to_string()]));
        let mut method_set_keys = BTreeMap::new();
        method_set_keys.insert("pkg.File".to_string(), interner.intern("ms|pkg.File"));

        let mut methods_by_receiver = BTreeMap::new();
        methods_by_receiver.insert(
            "pkg.File".to_string(),
            vec![GoRtaMethod {
                method_name: "Read".to_string(),
                qualified: "(pkg.File).Read".to_string(),
                node: SemanticNodeId(3),
            }],
        );

        let mut instantiated_keys = BTreeMap::new();
        instantiated_keys.insert("pkg.File".to_string(), interner.intern("inst|pkg.File"));

        let inputs = GoRtaInputs {
            roots: BTreeSet::from(["main.main".to_string()]),
            callsites: vec![GoRtaCallsite {
                caller: "main.main".to_string(),
                callsite_node: SemanticNodeId(2),
                callsite_stable_key: interner.intern("cs|main:read"),
                interface_method: Some("Read".to_string()),
                signature: None,
                dispatch_stable_key: interner.intern("dd|main:read"),
            }],
            method_sets,
            method_set_keys,
            instantiated: BTreeSet::from(["pkg.File".to_string()]),
            instantiated_keys,
            methods_by_receiver,
            function_node,
            ..GoRtaInputs::default()
        }
        .finalize_indexes();

        let policy = GoRtaPolicy::new(inputs);
        assert_eq!(policy.id(), "go_rta");
        let outcome = policy.solve(
            &crate::core::test_stable_key_interner(),
            &SolverBudget::default(),
        );
        assert!(outcome.points_to.is_none());
        assert!(
            !outcome.derived_edges.is_empty(),
            "the Go RTA policy must derive ≥1 edge from a resolvable dispatch"
        );
        assert_eq!(outcome.budget_status, BudgetStatus::WithinBudget);
    }

    #[test]
    fn engine_drives_empty_go_rta_and_ts_inputs_to_zero_results() {
        // Empty Go RTA and TS token snapshots derive nothing, proving the composition
        // root drives both frontend policies without fabricating edges.
        let budget = SolverBudget::default();
        let engine = SolverEngine::new(
            vec![
                Box::new(GoRtaPolicy::new(GoRtaInputs::default())),
                Box::new(TsPointsToPolicy::new(TsPointsToInputs::default())),
            ],
            budget,
        );
        let run = engine.run(&crate::core::test_stable_key_interner());

        assert_eq!(run.policy_outcomes.len(), 2);
        assert_eq!(run.policy_outcomes[0].policy_id, "go_rta");
        assert_eq!(run.policy_outcomes[1].policy_id, "ts_points_to");
        assert!(
            run.policy_outcomes
                .iter()
                .all(|record| record.outcome.points_to.is_none()
                    && record.outcome.derived_edges.is_empty())
        );
        assert_eq!(run.budget_status, BudgetStatus::WithinBudget);
    }

    #[test]
    fn run_to_solver_output_still_surfaces_go_rta_budget_exhaustion() {
        let interner = crate::core::test_stable_key_interner();
        // Excluding the discarded points-to fold must not mask a policy whose edges
        // enter the output. A Go RTA policy that exhausts its round cap must still
        // surface BudgetExceeded at the merged output.
        let mut function_node = BTreeMap::new();
        function_node.insert("main.main".to_string(), SemanticNodeId(1));
        function_node.insert("(pkg.A).Step".to_string(), SemanticNodeId(3));
        function_node.insert("(pkg.B).Step".to_string(), SemanticNodeId(5));
        let mut method_sets = BTreeMap::new();
        method_sets.insert("pkg.A".to_string(), BTreeSet::from(["Step".to_string()]));
        method_sets.insert("pkg.B".to_string(), BTreeSet::from(["Step".to_string()]));
        let mut method_set_keys = BTreeMap::new();
        method_set_keys.insert("pkg.A".to_string(), interner.intern("ms|A"));
        method_set_keys.insert("pkg.B".to_string(), interner.intern("ms|B"));
        let mut methods_by_receiver = BTreeMap::new();
        methods_by_receiver.insert(
            "pkg.A".to_string(),
            vec![GoRtaMethod {
                method_name: "Step".to_string(),
                qualified: "(pkg.A).Step".to_string(),
                node: SemanticNodeId(3),
            }],
        );
        methods_by_receiver.insert(
            "pkg.B".to_string(),
            vec![GoRtaMethod {
                method_name: "Step".to_string(),
                qualified: "(pkg.B).Step".to_string(),
                node: SemanticNodeId(5),
            }],
        );
        let mut instantiated_keys = BTreeMap::new();
        instantiated_keys.insert("pkg.A".to_string(), interner.intern("inst|A"));
        instantiated_keys.insert("pkg.B".to_string(), interner.intern("inst|B"));
        let go_inputs = GoRtaInputs {
            roots: BTreeSet::from(["main.main".to_string()]),
            callsites: vec![
                GoRtaCallsite {
                    caller: "main.main".to_string(),
                    callsite_node: SemanticNodeId(2),
                    callsite_stable_key: interner.intern("cs|main"),
                    interface_method: Some("Step".to_string()),
                    signature: None,
                    dispatch_stable_key: interner.intern("dd|main"),
                },
                GoRtaCallsite {
                    caller: "(pkg.A).Step".to_string(),
                    callsite_node: SemanticNodeId(4),
                    callsite_stable_key: interner.intern("cs|a"),
                    interface_method: Some("Step".to_string()),
                    signature: None,
                    dispatch_stable_key: interner.intern("dd|a"),
                },
            ],
            method_sets,
            method_set_keys,
            instantiated: BTreeSet::from(["pkg.A".to_string(), "pkg.B".to_string()]),
            instantiated_keys,
            methods_by_receiver,
            function_node,
            ..GoRtaInputs::default()
        }
        .finalize_indexes();

        let mut budget = SolverBudget::default();
        budget.go.max_rta_rounds = 1;
        let engine = SolverEngine::new(
            vec![
                Box::new(PointsToPolicy::new(Vec::new())),
                Box::new(GoRtaPolicy::new(go_inputs)),
            ],
            budget,
        );
        let output = engine.run_to_solver_output(&crate::core::test_stable_key_interner(), &[]);
        assert_eq!(
            output.budget_status,
            BudgetStatus::BudgetExceeded,
            "a Go RTA policy whose edges enter the output must still surface exhaustion"
        );
    }
}

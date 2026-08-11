//! [`SolverPolicy`] scaffolding (D-03, D-07).
//!
//! A [`SolverPolicy`] is the abstraction the unified [`super::engine`] core drives:
//! a policy contributes propagation/derivation over a **closed constraint
//! snapshot** (D-11) and reports its budget outcome.
//! [`PointsToPolicy`], which folds v1.2's
//! `points_to::solver` fixpoint in **by composition** (D-03): it invokes the
//! existing `solve_points_to` engine in place, so the points-to snapshot and
//! determinism fixtures stay byte-identical. It does NOT rewrite the points-to
//! fixpoint.
//!
//! The Go ([`GoRtaPolicy`]) and TS ([`TsPointsToPolicy`]) policies are the private
//! language-specific edge-producing policies driven by the unified engine.
//!
//! See [`super`] for the D-04 naming-collision guard (unified core vs. the
//! points-to sub-domain's internal `PointsToConstraintKind`/`PtVarId` language).

use std::collections::BTreeSet;

#[cfg(test)]
use crate::analysis::points_to::facts::PointsToConstraintFact;
use crate::analysis::points_to::solver::PointsToSolveResult;
#[cfg(test)]
use crate::analysis::points_to::solver::solve_points_to;

use super::budget::{BudgetStatus, SolverBudget};
use super::facts::DerivedEdgeFact;
use super::go_rta::{GoRtaInputs, solve_go_rta};
use ts_points_to::budget_status;
pub(crate) use ts_points_to::{TsPointsToInputs, solve_ts_points_to};

/// Outcome produced by a [`SolverPolicy`] when driven by the engine.
///
/// `points_to` carries the folded points-to sub-domain result (D-03) when the
/// policy is the points-to domain; `derived_edges` carries a policy's derived edges
/// (D-03) — the Go RTA and TS token policies produce
/// `CallConstraint`-derived call edges here. Empty input snapshots leave both empty
/// and report [`BudgetStatus::WithinBudget`] over zero derivation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PolicyOutcome {
    /// Folded points-to sub-domain result, if this policy is the points-to domain.
    pub(crate) points_to: Option<PointsToSolveResult>,
    /// The policy's derived edges (D-03). The Go RTA policy fills this with its
    /// resolved `DerivedEdgeFact`s; the points-to policy leaves it empty because its
    /// CopyEdge closure flows through the free `engine::derive_edges` (the composition
    /// that keeps points-to output byte-identical), not this channel.
    pub(crate) derived_edges: Vec<DerivedEdgeFact>,
    /// The policy's budget outcome, projected to the unified [`BudgetStatus`].
    pub(crate) budget_status: BudgetStatus,
    /// Stable reason labels for every budget ceiling this policy exhausted.
    pub(crate) budget_reasons: BTreeSet<String>,
    /// Number of worklist steps the policy reported consuming (sourced into the
    /// engine's monotonic step counter for provenance in Plan 02).
    pub(crate) steps: u64,
}

impl PolicyOutcome {
    /// An honest-empty outcome: zero derivation, within budget, zero steps. Stays
    /// semantically empty: no points-to result, no derived edges.
    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        Self {
            points_to: None,
            derived_edges: Vec::new(),
            budget_status: BudgetStatus::WithinBudget,
            budget_reasons: BTreeSet::new(),
            steps: 0,
        }
    }
}

/// The abstraction the unified solver core drives (D-07).
///
/// An implementation contributes derivation over a closed snapshot and reports a
/// budget outcome. The points-to policy folds the existing sub-domain; Go RTA and
/// TS tokens are language-specific edge-producing policies.
///
/// Production routes edge derivation through [`super::engine::SolverEngine`], which
/// merges the free [`super::engine::derive_edges`] CopyEdge closure with policy
/// outputs.
pub(crate) trait SolverPolicy {
    /// Stable lowercase policy identifier (used in stable keys / diagnostics).
    fn id(&self) -> &'static str;

    /// Drive the policy to its single fixpoint over the closed snapshot, bounded
    /// by `budget`. Returns the derived outcome + budget status.
    fn solve(
        &self,
        interner: &crate::core::StableKeyInterner,
        budget: &SolverBudget,
    ) -> PolicyOutcome;
}

/// The points-to policy folds in the points-to sub-domain by
/// composition (D-03). It owns a closed snapshot of points-to constraints and
/// delegates to the existing `solve_points_to` fixpoint unchanged.
#[cfg(test)]
pub(crate) struct PointsToPolicy {
    constraints: Vec<PointsToConstraintFact>,
}

#[cfg(test)]
impl PointsToPolicy {
    pub(crate) fn new(constraints: Vec<PointsToConstraintFact>) -> Self {
        Self { constraints }
    }
}

#[cfg(test)]
impl SolverPolicy for PointsToPolicy {
    fn id(&self) -> &'static str {
        "points_to"
    }

    fn solve(
        &self,
        interner: &crate::core::StableKeyInterner,
        budget: &SolverBudget,
    ) -> PolicyOutcome {
        // Composition, not rewrite (D-03): invoke the existing engine in place via
        // the projected points-to budget. The result is byte-identical to calling
        // `solve_points_to` directly.
        let result = solve_points_to(interner, &self.constraints, budget.points_to_budget());
        let budget_status = BudgetStatus::from_points_to(result.budget_status);
        let budget_reasons = result.budget_reasons.clone();
        PolicyOutcome {
            points_to: Some(result),
            // Points-to derived edges flow through `engine::derive_edges` (the
            // byte-identical CopyEdge closure), not this channel.
            derived_edges: Vec::new(),
            budget_status,
            budget_reasons,
            steps: 0,
        }
    }
}

/// The real Go RTA policy (GO-05). Owns a CLOSED snapshot of the Go RTA
/// inputs (reachability roots + the Go-frontend address-taken / instantiated-type /
/// dispatch facts + method-sets + callsites), mirroring how [`PointsToPolicy`] owns
/// its constraints. [`SolverPolicy::solve`] runs the RTA fixpoint
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
        // worklist, mirroring PointsToPolicy's fold). The output is already normalized.
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
    use crate::analysis::solver::go_rta::inputs::{GoRtaCallsite, GoRtaMethod};

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
    fn empty_outcome_stays_semantically_empty() {
        // empty() must carry no points-to result AND no derived edges.
        let outcome = PolicyOutcome::empty();
        assert!(outcome.points_to.is_none());
        assert!(outcome.derived_edges.is_empty());
        assert_eq!(outcome.budget_status, BudgetStatus::WithinBudget);
        assert_eq!(outcome.steps, 0);
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
    fn points_to_policy_id_is_stable() {
        let policy = PointsToPolicy::new(Vec::new());
        assert_eq!(policy.id(), "points_to");
    }
}

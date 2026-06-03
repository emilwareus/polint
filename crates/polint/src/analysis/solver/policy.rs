//! [`SolverPolicy`] scaffolding (D-03, D-07).
//!
//! A [`SolverPolicy`] is the abstraction the unified [`super::engine`] core drives:
//! a policy contributes propagation/derivation over a **closed constraint
//! snapshot** (D-11) and reports its budget outcome. Phase 47 ships EXACTLY ONE
//! real implementation — [`PointsToPolicy`], which folds v1.2's
//! `points_to::solver` fixpoint in **by composition** (D-03): it invokes the
//! existing `solve_points_to` engine in place, so the points-to snapshot and
//! determinism fixtures stay byte-identical. It does NOT rewrite the points-to
//! fixpoint.
//!
//! The Go ([`GoRtaPolicy`]) and TS ([`TsTokensPolicy`]) policies are HONEST STUBS
//! reserved for Phases 48/49: they derive NOTHING (honest emptiness, D-07),
//! mirroring the `ConstraintKind::ModelEdge` reserved-but-stubbed precedent. They
//! are not fake drivers — they intentionally produce zero output until their
//! reserving phase lands.
//!
//! See [`super`] for the D-04 naming-collision guard (unified core vs. the
//! points-to sub-domain's internal `PointsToConstraintKind`/`PtVarId` language).

use crate::analysis::points_to::facts::PointsToConstraintFact;
use crate::analysis::points_to::solver::{PointsToSolveResult, solve_points_to};

use super::budget::{BudgetStatus, SolverBudget};
use super::facts::DerivedEdgeFact;
use super::go_rta::{GoRtaInputs, solve_go_rta};
use super::ts_tokens::TsTokenInputs;

/// Outcome produced by a [`SolverPolicy`] when driven by the engine.
///
/// `points_to` carries the folded points-to sub-domain result (D-03) when the
/// policy is the points-to domain; `derived_edges` carries a policy's derived edges
/// (D-03, Phase 48) — the Go RTA policy produces `CallConstraint`-derived call edges
/// here. Honest stubs leave both empty and report [`BudgetStatus::WithinBudget`] over
/// zero derivation.
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
    /// Number of worklist steps the policy reported consuming (sourced into the
    /// engine's monotonic step counter for provenance in Plan 02).
    pub(crate) steps: u64,
}

impl PolicyOutcome {
    /// An honest-empty outcome: zero derivation, within budget, zero steps. Used
    /// by the reserved TS stub (D-07). Stays semantically empty: no points-to result,
    /// no derived edges.
    pub(crate) fn empty() -> Self {
        Self {
            points_to: None,
            derived_edges: Vec::new(),
            budget_status: BudgetStatus::WithinBudget,
            steps: 0,
        }
    }
}

/// The abstraction the unified solver core drives (D-07).
///
/// An implementation contributes derivation over a closed snapshot and reports a
/// budget outcome. Phase 47 ships one real impl ([`PointsToPolicy`], the D-03
/// points-to fold) and two honest stubs ([`GoRtaPolicy`], [`TsTokensPolicy`]).
///
/// Reserved seam: production edge derivation runs through the free
/// [`super::engine::derive_edges`] function today; this trait + [`super::engine::SolverEngine`]
/// are the reserved multi-policy orchestration Phases 48/49 extend (see the engine
/// module docs). The impls here are exercised by tests until their producing phase
/// routes production through the engine — deliberate scaffolding, not dead code.
pub(crate) trait SolverPolicy {
    /// Stable lowercase policy identifier (used in stable keys / diagnostics).
    fn id(&self) -> &'static str;

    /// Drive the policy to its single fixpoint over the closed snapshot, bounded
    /// by `budget`. Returns the derived outcome + budget status.
    fn solve(&self, budget: &SolverBudget) -> PolicyOutcome;
}

/// The one real Phase 47 policy: the points-to sub-domain folded in by
/// composition (D-03). It owns a closed snapshot of points-to constraints and
/// delegates to the existing `solve_points_to` fixpoint unchanged.
pub(crate) struct PointsToPolicy {
    constraints: Vec<PointsToConstraintFact>,
}

impl PointsToPolicy {
    pub(crate) fn new(constraints: Vec<PointsToConstraintFact>) -> Self {
        Self { constraints }
    }
}

impl SolverPolicy for PointsToPolicy {
    fn id(&self) -> &'static str {
        "points_to"
    }

    fn solve(&self, budget: &SolverBudget) -> PolicyOutcome {
        // Composition, not rewrite (D-03): invoke the existing engine in place via
        // the projected points-to budget. The result is byte-identical to calling
        // `solve_points_to` directly.
        let result = solve_points_to(&self.constraints, budget.points_to_budget());
        let budget_status = BudgetStatus::from_points_to(result.budget_status);
        PolicyOutcome {
            points_to: Some(result),
            // Points-to derived edges flow through `engine::derive_edges` (the
            // byte-identical CopyEdge closure), not this channel.
            derived_edges: Vec::new(),
            budget_status,
            steps: 0,
        }
    }
}

/// The real Go RTA policy (Phase 48, GO-05). Owns a CLOSED snapshot of the Go RTA
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

    fn solve(&self, budget: &SolverBudget) -> PolicyOutcome {
        // Run the RTA fixpoint over the closed snapshot (composition over the engine
        // worklist, mirroring PointsToPolicy's fold). The output is already normalized.
        let output = solve_go_rta(&self.inputs, budget);
        PolicyOutcome {
            points_to: None,
            derived_edges: output.derived_edges,
            budget_status: output.budget_status,
            steps: 0,
        }
    }
}

/// JS/TS function-token policy (Phase 49, JS-04). It owns a closed token snapshot;
/// the fixpoint/dispatch implementation lands in the following Plan 02 tasks.
pub(crate) struct TsTokensPolicy {
    inputs: TsTokenInputs,
}

impl TsTokensPolicy {
    pub(crate) fn new(inputs: TsTokenInputs) -> Self {
        Self { inputs }
    }
}

impl SolverPolicy for TsTokensPolicy {
    fn id(&self) -> &'static str {
        "ts_tokens"
    }

    fn solve(&self, _budget: &SolverBudget) -> PolicyOutcome {
        let _ = &self.inputs;
        PolicyOutcome::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    use crate::analysis::ids::SemanticNodeId;
    use crate::analysis::solver::go_rta::inputs::{GoRtaCallsite, GoRtaMethod};

    #[test]
    fn ts_policy_id_is_stable_while_driver_lands() {
        // Task 1 wires a closed snapshot into the policy; later Plan 02 tasks replace
        // the temporary empty outcome with the real token fixpoint.
        let budget = SolverBudget::default();
        let ts = TsTokensPolicy::new(TsTokenInputs::default());
        let ts_outcome = ts.solve(&budget);
        assert_eq!(ts.id(), "ts_tokens");
        assert_eq!(ts_outcome, PolicyOutcome::empty());
        assert!(ts_outcome.points_to.is_none());
        assert!(ts_outcome.derived_edges.is_empty());
    }

    #[test]
    fn empty_outcome_stays_semantically_empty() {
        // empty() must carry no points-to result AND no derived edges, so the TS stub
        // (and any future reserved stub) returning it derives nothing.
        let outcome = PolicyOutcome::empty();
        assert!(outcome.points_to.is_none());
        assert!(outcome.derived_edges.is_empty());
        assert_eq!(outcome.budget_status, BudgetStatus::WithinBudget);
        assert_eq!(outcome.steps, 0);
    }

    #[test]
    fn go_rta_policy_derives_edges_from_a_resolvable_dispatch() {
        // The real Go RTA policy now derives ≥1 edge from a resolvable interface
        // dispatch (an instantiated receiver whose method-set declares the method).
        let mut function_node = BTreeMap::new();
        function_node.insert("main.main".to_string(), SemanticNodeId(1));
        function_node.insert("(pkg.File).Read".to_string(), SemanticNodeId(3));

        let mut method_sets = BTreeMap::new();
        method_sets.insert("pkg.File".to_string(), BTreeSet::from(["Read".to_string()]));
        let mut method_set_keys = BTreeMap::new();
        method_set_keys.insert("pkg.File".to_string(), "ms|pkg.File".to_string());

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
        instantiated_keys.insert("pkg.File".to_string(), "inst|pkg.File".to_string());

        let inputs = GoRtaInputs {
            roots: BTreeSet::from(["main.main".to_string()]),
            callsites: vec![GoRtaCallsite {
                caller: "main.main".to_string(),
                callsite_node: SemanticNodeId(2),
                callsite_stable_key: "cs|main:read".to_string(),
                interface_method: Some("Read".to_string()),
                signature: None,
                dispatch_stable_key: "dd|main:read".to_string(),
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
        let outcome = policy.solve(&SolverBudget::default());
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

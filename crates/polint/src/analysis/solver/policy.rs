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

/// Outcome produced by a [`SolverPolicy`] when driven by the engine.
///
/// `points_to` carries the folded points-to sub-domain result (D-03) when the
/// policy is the points-to domain; honest stubs leave it `None` and report
/// [`BudgetStatus::WithinBudget`] over zero derivation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PolicyOutcome {
    /// Folded points-to sub-domain result, if this policy is the points-to domain.
    pub(crate) points_to: Option<PointsToSolveResult>,
    /// The policy's budget outcome, projected to the unified [`BudgetStatus`].
    pub(crate) budget_status: BudgetStatus,
    /// Number of worklist steps the policy reported consuming (sourced into the
    /// engine's monotonic step counter for provenance in Plan 02).
    pub(crate) steps: u64,
}

impl PolicyOutcome {
    /// An honest-empty outcome: zero derivation, within budget, zero steps. Used
    /// by the reserved Go/TS stubs (D-07).
    pub(crate) fn empty() -> Self {
        Self {
            points_to: None,
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
            budget_status,
            steps: 0,
        }
    }
}

/// Reserved Go RTA policy. No driver exists until **Phase 48 (GO-05)**;
/// [`SolverPolicy::solve`] derives ZERO results (honest emptiness, D-07),
/// mirroring the `ConstraintKind::ModelEdge` reserved-but-stubbed precedent. This
/// is NOT a fake driver — the emptiness is intentional until Phase 48 lands the
/// reachability fixpoint, address-taken tracking, and dynamic dispatch.
pub(crate) struct GoRtaPolicy;

impl SolverPolicy for GoRtaPolicy {
    fn id(&self) -> &'static str {
        "go_rta"
    }

    fn solve(&self, _budget: &SolverBudget) -> PolicyOutcome {
        PolicyOutcome::empty()
    }
}

/// Reserved JS/TS token-propagation policy. No driver exists until **Phase 49
/// (JS-04)**; [`SolverPolicy::solve`] derives ZERO results (honest emptiness,
/// D-07), mirroring the `ConstraintKind::ModelEdge` reserved-but-stubbed
/// precedent. This is NOT a fake driver — the emptiness is intentional until
/// Phase 49 lands token propagation through copy/call/return constraints.
pub(crate) struct TsTokensPolicy;

impl SolverPolicy for TsTokensPolicy {
    fn id(&self) -> &'static str {
        "ts_tokens"
    }

    fn solve(&self, _budget: &SolverBudget) -> PolicyOutcome {
        PolicyOutcome::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn go_and_ts_stubs_derive_nothing() {
        let budget = SolverBudget::default();

        let go = GoRtaPolicy;
        let go_outcome = go.solve(&budget);
        assert_eq!(go.id(), "go_rta");
        assert_eq!(go_outcome, PolicyOutcome::empty());
        assert!(go_outcome.points_to.is_none());

        let ts = TsTokensPolicy;
        let ts_outcome = ts.solve(&budget);
        assert_eq!(ts.id(), "ts_tokens");
        assert_eq!(ts_outcome, PolicyOutcome::empty());
        assert!(ts_outcome.points_to.is_none());
    }

    #[test]
    fn points_to_policy_id_is_stable() {
        let policy = PointsToPolicy::new(Vec::new());
        assert_eq!(policy.id(), "points_to");
    }
}

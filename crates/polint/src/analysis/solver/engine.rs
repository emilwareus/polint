//! Unified solver engine: deterministic worklist core (D-02, D-11).
//!
//! The engine owns the single deterministic `VecDeque` worklist that drives the
//! registered [`super::policy::SolverPolicy`] impls to a **single fixpoint per
//! run** (D-11), enforces the [`SolverBudget`] ceilings, projects the outcome to
//! [`BudgetStatus`], and maintains a monotonic `u64` step counter (Plan 02 reads
//! this for the provenance solver-step field, D-08). The worklist drain mirrors
//! the proven `points_to::solver` shape (`while let Some(..) = queue.pop_front()`
//! with `if !budget_ok { break; }`) and `BTreeMap`/`BTreeSet`-ordered
//! accumulation, which is what makes the output byte-stable (D-02).
//!
//! The points-to fixpoint is folded in by composition (D-03): the engine drives
//! the [`super::policy::PointsToPolicy`], which invokes the existing
//! `solve_points_to` engine in place. Driving the points-to policy through this
//! engine produces a result byte-identical to calling `solve_points_to`
//! directly.

use std::collections::VecDeque;

use super::budget::{BudgetStatus, SolverBudget};
use super::policy::{PolicyOutcome, SolverPolicy};

/// Result of a single unified solver run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SolverRunResult {
    /// Per-policy outcomes, in the deterministic order the engine drained them.
    pub(crate) policy_outcomes: Vec<PolicyRunRecord>,
    /// The combined budget outcome across all driven policies (worst-case wins:
    /// any policy exceeding its budget surfaces [`BudgetStatus::BudgetExceeded`]).
    pub(crate) budget_status: BudgetStatus,
    /// Monotonic worklist step counter for this run (provenance solver-step).
    pub(crate) steps: u64,
}

/// One policy's contribution recorded by the engine, tagged by policy id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PolicyRunRecord {
    pub(crate) policy_id: &'static str,
    pub(crate) outcome: PolicyOutcome,
}

/// The unified solver engine. Holds a closed, ordered set of registered policies
/// (the closed input set, D-11) and drives them to convergence with a single
/// deterministic worklist drain.
pub(crate) struct SolverEngine {
    policies: Vec<Box<dyn SolverPolicy>>,
    budget: SolverBudget,
}

impl SolverEngine {
    /// Build an engine over a closed, ordered policy set. Order is the
    /// registration order; the worklist drains it deterministically.
    pub(crate) fn new(policies: Vec<Box<dyn SolverPolicy>>, budget: SolverBudget) -> Self {
        Self { policies, budget }
    }

    /// Drive every registered policy to its single fixpoint, bounded by the
    /// budget, accumulating outcomes in deterministic order (D-02). The monotonic
    /// step counter increments once per worklist step and latches budget
    /// exhaustion honestly (D-06/D-11) rather than looping unbounded.
    pub(crate) fn run(&self) -> SolverRunResult {
        // Deterministic worklist of policy indices in stable registration order.
        // The bounded outer-iteration cap (D-11) guards against ever draining
        // more steps than the budget allows.
        let mut queue: VecDeque<usize> = (0..self.policies.len()).collect();
        let mut steps: u64 = 0;
        let mut budget_exceeded = false;
        let mut records: Vec<PolicyRunRecord> = Vec::with_capacity(self.policies.len());

        while let Some(index) = queue.pop_front() {
            // Bounded outer-iteration cap: each policy drained is one worklist
            // step; exceeding the cap latches exhaustion instead of looping.
            steps += 1;
            if steps > self.budget.max_outer_iterations as u64 {
                budget_exceeded = true;
                break;
            }

            let policy = &self.policies[index];
            let outcome = policy.solve(&self.budget);
            if outcome.budget_status == BudgetStatus::BudgetExceeded {
                budget_exceeded = true;
            }
            // Fold the policy's own internal step count into the run counter so
            // provenance (Plan 02) sees the full worklist progress.
            steps = steps.saturating_add(outcome.steps);
            records.push(PolicyRunRecord {
                policy_id: policy.id(),
                outcome,
            });
        }

        let budget_status = if budget_exceeded {
            BudgetStatus::BudgetExceeded
        } else if records.is_empty() {
            BudgetStatus::NotRun
        } else {
            BudgetStatus::WithinBudget
        };

        SolverRunResult {
            policy_outcomes: records,
            budget_status,
            steps,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ids::{ObjectTokenId, PointsToConstraintId, PtVarId};
    use crate::analysis::points_to::facts::{
        PointsToConstraintFact, PointsToConstraintKind, PointsToPrecision, PointsToStatus,
    };
    use crate::analysis::points_to::solver::{PointsToBudget, solve_points_to};
    use crate::analysis::solver::policy::{GoRtaPolicy, PointsToPolicy, TsTokensPolicy};

    fn constraint(stable_key: &str, kind: PointsToConstraintKind) -> PointsToConstraintFact {
        PointsToConstraintFact {
            id: PointsToConstraintId(0),
            kind,
            status: PointsToStatus::Present,
            precision: PointsToPrecision::FlowInsensitive,
            stable_key: stable_key.to_string(),
        }
    }

    fn sample_constraints() -> Vec<PointsToConstraintFact> {
        vec![
            constraint(
                "addr-a",
                PointsToConstraintKind::AddressOf {
                    dst: PtVarId(1),
                    object: ObjectTokenId(10),
                },
            ),
            constraint(
                "copy",
                PointsToConstraintKind::Copy {
                    dst: PtVarId(2),
                    src: PtVarId(1),
                },
            ),
            constraint(
                "field-store",
                PointsToConstraintKind::FieldStore {
                    base: PtVarId(1),
                    field: "name".to_string(),
                    src: PtVarId(2),
                },
            ),
            constraint(
                "field-load",
                PointsToConstraintKind::FieldLoad {
                    dst: PtVarId(3),
                    base: PtVarId(1),
                    field: "name".to_string(),
                },
            ),
        ]
    }

    #[test]
    fn points_to_via_engine_equals_solve_points_to() {
        // The fold (D-03) preserves behavior: driving the points-to policy through
        // the unified engine yields exactly the standalone `solve_points_to`
        // result over the same constraints.
        let constraints = sample_constraints();
        let budget = SolverBudget::default();

        let direct = solve_points_to(&constraints, PointsToBudget::default());

        let policy = PointsToPolicy::new(constraints);
        let engine = SolverEngine::new(vec![Box::new(policy)], budget);
        let run = engine.run();

        let via_engine = run.policy_outcomes[0]
            .outcome
            .points_to
            .clone()
            .expect("points-to policy yields a folded result");

        assert_eq!(via_engine, direct);
    }

    #[test]
    fn engine_is_deterministic_across_two_runs() {
        let constraints = sample_constraints();
        let budget = SolverBudget::default();

        let first = SolverEngine::new(
            vec![Box::new(PointsToPolicy::new(constraints.clone()))],
            budget,
        )
        .run();
        let second =
            SolverEngine::new(vec![Box::new(PointsToPolicy::new(constraints))], budget).run();

        assert_eq!(first, second);
        assert_eq!(first.budget_status, BudgetStatus::WithinBudget);
    }

    #[test]
    fn engine_surfaces_budget_exhaustion_honestly() {
        // A tight per-sub-domain object cap forces the points-to fixpoint to latch
        // exhaustion; the engine projects it to BudgetStatus::BudgetExceeded
        // rather than dropping silently (D-06).
        let constraints = vec![
            constraint(
                "addr-a",
                PointsToConstraintKind::AddressOf {
                    dst: PtVarId(1),
                    object: ObjectTokenId(10),
                },
            ),
            constraint(
                "addr-b",
                PointsToConstraintKind::AddressOf {
                    dst: PtVarId(1),
                    object: ObjectTokenId(11),
                },
            ),
        ];
        let mut budget = SolverBudget::default();
        budget.points_to.max_objects_per_var = 1;

        let engine = SolverEngine::new(vec![Box::new(PointsToPolicy::new(constraints))], budget);
        let run = engine.run();

        assert_eq!(run.budget_status, BudgetStatus::BudgetExceeded);
    }

    #[test]
    fn engine_drives_stubs_to_zero_results() {
        let budget = SolverBudget::default();
        let engine = SolverEngine::new(
            vec![Box::new(GoRtaPolicy), Box::new(TsTokensPolicy)],
            budget,
        );
        let run = engine.run();

        assert_eq!(run.policy_outcomes.len(), 2);
        assert_eq!(run.policy_outcomes[0].policy_id, "go_rta");
        assert_eq!(run.policy_outcomes[1].policy_id, "ts_tokens");
        assert!(
            run.policy_outcomes
                .iter()
                .all(|record| record.outcome.points_to.is_none())
        );
        assert_eq!(run.budget_status, BudgetStatus::WithinBudget);
    }

    #[test]
    fn empty_engine_reports_not_run() {
        let engine = SolverEngine::new(Vec::new(), SolverBudget::default());
        let run = engine.run();
        assert_eq!(run.budget_status, BudgetStatus::NotRun);
        assert!(run.policy_outcomes.is_empty());
    }
}

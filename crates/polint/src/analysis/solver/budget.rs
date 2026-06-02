//! Unified solver budget model (D-05, D-06).
//!
//! [`SolverBudget`] generalizes the points-to sub-domain's `PointsToBudget`: it
//! carries the cross-domain knobs (`max_steps` and an explicit bounded
//! outer-iteration cap per D-11) plus a per-sub-domain channel for the
//! points-to-specific knobs. [`BudgetStatus`] generalizes `PointsToBudgetStatus`
//! and models budget exhaustion as an explicit, honest signal (D-06) — never a
//! silent drop.
//!
//! D-05 (Claude's discretion: alias or wrap): the existing `PointsToBudget` and
//! `PointsToBudgetStatus` are treated as a **sub-domain projection** of the
//! unified types via explicit mapping functions
//! ([`SolverBudget::points_to_budget`] / [`BudgetStatus::from_points_to`]) rather
//! than editing `PointsToBudget`. Its `Default` (10_000 / 64 / 512) stays
//! byte-identical, so points-to fixtures do not change.

use serde::{Deserialize, Serialize};

use crate::analysis::points_to::facts::PointsToBudgetStatus;
use crate::analysis::points_to::solver::PointsToBudget;

/// Per-sub-domain budget knobs carried alongside the unified cross-domain knobs.
///
/// This is the "channel for per-sub-domain knobs" D-05 prescribes. Today it holds
/// the points-to-specific caps; Phase 48 (Go RTA) and Phase 49 (TS tokens) extend
/// it as their drivers land.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PointsToSubBudget {
    pub(crate) max_objects_per_var: usize,
    pub(crate) max_dynamic_vars: usize,
}

impl Default for PointsToSubBudget {
    fn default() -> Self {
        // Mirror `PointsToBudget::default()` (64 / 512) so the projection stays
        // byte-identical to the standalone points-to defaults.
        Self {
            max_objects_per_var: 64,
            max_dynamic_vars: 512,
        }
    }
}

/// Unified solver budget generalizing `PointsToBudget` (D-05).
///
/// Cross-domain knobs: `max_steps` (per-step worklist cap, mirrors the points-to
/// default 10_000) and `max_outer_iterations` (the explicit bounded outer-iteration
/// cap mandated by D-11 — the unified core never loops unbounded). Per-sub-domain
/// knobs hang off [`PointsToSubBudget`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SolverBudget {
    pub(crate) max_steps: usize,
    pub(crate) max_outer_iterations: usize,
    pub(crate) points_to: PointsToSubBudget,
}

impl Default for SolverBudget {
    fn default() -> Self {
        Self {
            // Matches the points-to default 10_000 so the folded sub-domain runs
            // identically when driven through the unified budget.
            max_steps: 10_000,
            // Bounded outer-iteration cap (D-11). A single fixpoint drain is one
            // outer iteration today; the cap keeps future multi-policy rounds
            // bounded rather than unbounded.
            max_outer_iterations: 64,
            points_to: PointsToSubBudget::default(),
        }
    }
}

impl SolverBudget {
    /// Project the unified budget onto the existing `PointsToBudget` (D-05).
    ///
    /// This is the sub-domain projection: it never mutates `PointsToBudget`'s own
    /// `Default`, so the points-to fixtures stay byte-identical.
    pub(crate) fn points_to_budget(&self) -> PointsToBudget {
        PointsToBudget {
            max_steps: self.max_steps,
            max_objects_per_var: self.points_to.max_objects_per_var,
            max_dynamic_vars: self.points_to.max_dynamic_vars,
        }
    }
}

/// Unified budget outcome, generalizing `PointsToBudgetStatus` (D-05/D-06).
///
/// Pinned declaration order drives the derived `Ord` + serde, making this enum
/// byte-stable without `#[repr(u8)]`. Budget exhaustion surfaces as an explicit
/// [`BudgetStatus::BudgetExceeded`] signal rather than a silent precision drop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum BudgetStatus {
    /// The run converged within every budget ceiling.
    WithinBudget,
    /// A budget ceiling (steps, outer iterations, or a per-sub-domain cap) was
    /// hit; downstream (Phase 52 unknown taxonomy) categorizes this honestly.
    BudgetExceeded,
    /// The solver did not run for this input.
    NotRun,
}

impl BudgetStatus {
    /// Stable lowercase tag used in stable keys / digest payloads.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::WithinBudget => "within_budget",
            Self::BudgetExceeded => "budget_exceeded",
            Self::NotRun => "not_run",
        }
    }

    /// Lift a points-to sub-domain status into the unified status (D-05).
    pub(crate) fn from_points_to(status: PointsToBudgetStatus) -> Self {
        match status {
            PointsToBudgetStatus::WithinBudget => Self::WithinBudget,
            PointsToBudgetStatus::BudgetExceeded => Self::BudgetExceeded,
            PointsToBudgetStatus::NotRun => Self::NotRun,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solver_budget_default_matches_points_to_defaults() {
        let budget = SolverBudget::default();
        assert_eq!(budget.max_steps, 10_000);
        assert!(budget.max_outer_iterations > 0);

        // The projection reproduces the standalone points-to defaults exactly,
        // without touching PointsToBudget::default itself.
        let projected = budget.points_to_budget();
        assert_eq!(projected, PointsToBudget::default());
        assert_eq!(projected.max_steps, 10_000);
        assert_eq!(projected.max_objects_per_var, 64);
        assert_eq!(projected.max_dynamic_vars, 512);
    }

    #[test]
    fn points_to_budget_default_is_unchanged_by_projection() {
        // Guard the D-03/D-05 invariant: the standalone points-to defaults remain
        // 10_000 / 64 / 512 (byte-identical fixtures).
        let pt = PointsToBudget::default();
        assert_eq!(pt.max_steps, 10_000);
        assert_eq!(pt.max_objects_per_var, 64);
        assert_eq!(pt.max_dynamic_vars, 512);
    }

    #[test]
    fn budget_status_has_exactly_3_variants() {
        // Compile-time exhaustive match; adding a variant without updating this
        // test fails to compile, forcing a deliberate edit.
        fn assert_all(status: BudgetStatus) -> &'static str {
            match status {
                BudgetStatus::WithinBudget => "within_budget",
                BudgetStatus::BudgetExceeded => "budget_exceeded",
                BudgetStatus::NotRun => "not_run",
            }
        }
        let variants = [
            assert_all(BudgetStatus::WithinBudget),
            assert_all(BudgetStatus::BudgetExceeded),
            assert_all(BudgetStatus::NotRun),
        ];
        assert_eq!(variants.len(), 3);
    }

    #[test]
    fn budget_status_sorts_in_pinned_declaration_order() {
        // A permuted list sorts back to declaration order: the discriminant
        // ordering is declaration-driven, so the enum is byte-stable.
        let mut statuses = [
            BudgetStatus::NotRun,
            BudgetStatus::BudgetExceeded,
            BudgetStatus::WithinBudget,
        ];
        statuses.sort();
        let tags: Vec<&'static str> = statuses.iter().map(|s| s.as_str()).collect();
        assert_eq!(tags, vec!["within_budget", "budget_exceeded", "not_run"]);
    }

    #[test]
    fn budget_status_projects_from_points_to_status() {
        assert_eq!(
            BudgetStatus::from_points_to(PointsToBudgetStatus::WithinBudget),
            BudgetStatus::WithinBudget
        );
        assert_eq!(
            BudgetStatus::from_points_to(PointsToBudgetStatus::BudgetExceeded),
            BudgetStatus::BudgetExceeded
        );
        assert_eq!(
            BudgetStatus::from_points_to(PointsToBudgetStatus::NotRun),
            BudgetStatus::NotRun
        );
    }
}

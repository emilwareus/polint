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

/// Per-sub-domain budget knobs for the Go RTA driver (D-10/D-11).
///
/// Mirrors [`PointsToSubBudget`] structurally: a `Copy` bag of caps hung on
/// [`SolverBudget`] as `go`. These bound the RTA fixpoint so runaway interface
/// dispatch latches [`BudgetStatus::BudgetExceeded`] honestly (D-13) rather than
/// looping unbounded:
///
/// - `address_taken_threshold` — the roadmap-named knob (D-10): if the accumulated
///   address-taken function set exceeds this, the run latches exhaustion.
/// - `max_candidates_per_callsite` — caps the candidate-callee fan-out resolved for
///   one dynamic callsite; exceeding it is run-level exhaustion (edges resolved
///   before the cap keep their honest status).
/// - `max_rta_rounds` — caps GENUINE dynamic-dispatch re-iteration (the number of
///   rounds that actually resolved a dynamic callsite, never static-call-graph depth);
///   exceeding it latches exhaustion. Static-reachability growth is bounded by
///   `max_worklist_steps`, not this cap, so a deep first-party static call chain whose
///   depth exceeds this value still converges (FIX 1).
/// - `max_worklist_steps` — the Go-scaled per-callsite-resolution worklist-step cap.
///   One step is one callsite resolution. This is sized like the points-to
///   `max_steps` default (10_000), NOT the cross-domain `max_outer_iterations` (64,
///   a policy-drain count): the RTA fixpoint can resolve thousands of dynamic
///   callsite-visits across rounds on a real repo, so reusing the policy-count cap
///   here would spuriously latch [`BudgetStatus::BudgetExceeded`] and drop real edges
///   (Phase 48 review CR-01). Exceeding it is honest run-level exhaustion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GoRtaSubBudget {
    pub(crate) address_taken_threshold: usize,
    pub(crate) max_candidates_per_callsite: usize,
    pub(crate) max_rta_rounds: usize,
    pub(crate) max_worklist_steps: usize,
}

impl Default for GoRtaSubBudget {
    fn default() -> Self {
        // Honest defaults sized to comfortably accommodate real Go interface-dispatch
        // graphs while still bounding pathological fan-out / cyclic method-set graphs.
        Self {
            address_taken_threshold: 256,
            max_candidates_per_callsite: 128,
            max_rta_rounds: 32,
            // Go-scaled worklist-step cap (CR-01): mirrors the points-to `max_steps`
            // default (10_000), not the policy-count `max_outer_iterations` (64).
            max_worklist_steps: 10_000,
        }
    }
}

/// Per-sub-domain budget knobs for the JS/TS function-token driver (JS-04).
///
/// These caps bound the private `analysis::solver::ts_tokens` fixpoint planned for
/// Phase 49. They are intentionally crate-private: rule authors consume the final
/// derived facts through SDK views, not these internal propagation controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct JsTokensSubBudget {
    pub(crate) max_tokens_per_var: usize,
    pub(crate) max_candidates_per_callsite: usize,
    pub(crate) max_token_worklist_steps: usize,
}

impl Default for JsTokensSubBudget {
    fn default() -> Self {
        // Finite, strictly-positive defaults: large enough for ordinary first-party
        // higher-order flows, bounded enough that pathological token fan-out reports
        // BudgetExceeded instead of running unbounded.
        Self {
            max_tokens_per_var: 128,
            max_candidates_per_callsite: 256,
            max_token_worklist_steps: 10_000,
        }
    }
}

/// Per-sub-domain budget knobs for the JS/TS object/property/prototype/`this`
/// driver (JS-05).
///
/// This is intentionally separate from [`JsTokensSubBudget`]: object modeling has
/// property buckets, prototype traversal, receiver fan-out, and its own worklist
/// ceiling. The model remains disabled by default through
/// [`SolverBudget::object_model_enabled`] until benchmark promotion gates approve it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct JsObjectModelSubBudget {
    pub(crate) max_objects_per_place: usize,
    pub(crate) max_properties_per_object: usize,
    pub(crate) max_tokens_per_property: usize,
    pub(crate) max_computed_buckets_per_object: usize,
    pub(crate) max_prototype_depth: usize,
    pub(crate) max_receiver_candidates_per_callsite: usize,
    pub(crate) max_object_worklist_steps: usize,
}

impl Default for JsObjectModelSubBudget {
    fn default() -> Self {
        // Finite, strictly-positive defaults. They are sized to admit ordinary local
        // object/property flows while bounding fan-out until Phase 54 benchmark gates
        // decide whether this model should be on by default.
        Self {
            max_objects_per_place: 128,
            max_properties_per_object: 128,
            max_tokens_per_property: 128,
            max_computed_buckets_per_object: 8,
            max_prototype_depth: 8,
            max_receiver_candidates_per_callsite: 64,
            max_object_worklist_steps: 10_000,
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
    pub(crate) go: GoRtaSubBudget,
    pub(crate) js: JsTokensSubBudget,
    pub(crate) object_model_enabled: bool,
    pub(crate) object: JsObjectModelSubBudget,
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
            // Go RTA sub-budget (D-10/D-11). Adding this field MUST NOT perturb the
            // existing fields' values — `solver_budget_default_matches_points_to_defaults`
            // pins 10_000 / 64 / points-to defaults byte-identically.
            go: GoRtaSubBudget::default(),
            // JS token sub-budget (Phase 49/JS-04). Adding this field MUST NOT perturb
            // any existing cross-domain, points-to, or Go default.
            js: JsTokensSubBudget::default(),
            // JS object/property/prototype/receiver model (Phase 50/JS-05). It is
            // explicitly opt-in until benchmark gates approve default enablement.
            object_model_enabled: false,
            object: JsObjectModelSubBudget::default(),
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
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub(crate) enum BudgetStatus {
    /// The run converged within every budget ceiling.
    #[default]
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
    fn solver_budget_default_go_sub_budget_matches_go_defaults() {
        // The `go` sub-budget defaults are the roadmap-named honest caps. Adding the
        // field must not perturb the existing default fields (covered above).
        let budget = SolverBudget::default();
        assert_eq!(budget.go, GoRtaSubBudget::default());
        assert_eq!(budget.go.address_taken_threshold, 256);
        assert_eq!(budget.go.max_candidates_per_callsite, 128);
        assert_eq!(budget.go.max_rta_rounds, 32);
        // The Go-scaled worklist-step cap is sized like points-to `max_steps`
        // (10_000), never the policy-count 64 (CR-01).
        assert_eq!(budget.go.max_worklist_steps, 10_000);
        // The existing cross-domain/points-to defaults are still byte-identical.
        assert_eq!(budget.max_steps, 10_000);
        assert_eq!(budget.max_outer_iterations, 64);
        assert_eq!(budget.points_to, PointsToSubBudget::default());
    }

    #[test]
    fn solver_budget_default_js_sub_budget_matches_js_defaults() {
        // The `js` sub-budget defaults are strictly-positive bounded caps for the
        // private TS token driver. Adding the field must not perturb existing defaults.
        let budget = SolverBudget::default();
        assert_eq!(budget.js, JsTokensSubBudget::default());
        assert_eq!(budget.js.max_tokens_per_var, 128);
        assert_eq!(budget.js.max_candidates_per_callsite, 256);
        assert_eq!(budget.js.max_token_worklist_steps, 10_000);
        assert_eq!(budget.max_steps, 10_000);
        assert_eq!(budget.max_outer_iterations, 64);
        assert_eq!(budget.points_to, PointsToSubBudget::default());
        assert_eq!(budget.go, GoRtaSubBudget::default());
    }

    #[test]
    fn solver_budget_default_object_model_is_disabled() {
        let budget = SolverBudget::default();
        assert!(!budget.object_model_enabled);
        assert_eq!(budget.object, JsObjectModelSubBudget::default());
        assert_eq!(budget.max_steps, 10_000);
        assert_eq!(budget.max_outer_iterations, 64);
        assert_eq!(budget.points_to, PointsToSubBudget::default());
        assert_eq!(budget.go, GoRtaSubBudget::default());
        assert_eq!(budget.js, JsTokensSubBudget::default());
    }

    #[test]
    fn solver_budget_default_object_sub_budget_matches_object_defaults() {
        let budget = SolverBudget::default();
        assert_eq!(budget.object.max_objects_per_place, 128);
        assert_eq!(budget.object.max_properties_per_object, 128);
        assert_eq!(budget.object.max_tokens_per_property, 128);
        assert_eq!(budget.object.max_computed_buckets_per_object, 8);
        assert_eq!(budget.object.max_prototype_depth, 8);
        assert_eq!(budget.object.max_receiver_candidates_per_callsite, 64);
        assert_eq!(budget.object.max_object_worklist_steps, 10_000);
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

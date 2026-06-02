//! Solver provider cache key + parameter digest (D-15).
//!
//! Mirrors `analysis::semantic_graph::cache_key`. Defines [`SOLVER_SCHEMA_LABEL`]
//! (the `polint.solver` provider schema label) and
//! [`solver_provider_parameter_digest`], which lists the frozen solver
//! algorithm-version strings AND — per D-15 — the [`SolverBudget`] knobs, so a
//! budget-default change deterministically invalidates the solver cache (budgets
//! participate in the digest, forward-compatible with CACHE-01/02, Phase 53).
//!
//! Two locked tests pin the recipe (the established Phase 43/44 trip-wire pattern):
//! a "parts list" assertion (adding/bumping an algorithm version requires extending
//! the list) and an "algorithm-version bump invalidates" assertion. A third proves
//! that a SolverBudget change changes the parameter digest (D-15).

use crate::analysis::solver::budget::SolverBudget;
use crate::analysis_kernel::incremental::{Digest, DigestKind};

/// Schema label for the `polint.solver` provider manifest.
pub(crate) const SOLVER_SCHEMA_LABEL: &str = "solver-derived-edges-1";

/// Provider parameter digest for `polint.solver` (D-15).
///
/// The algorithm-version strings are part of the parameter digest so any bump to the
/// derived-edge / provenance / closure emission algorithm deterministically
/// invalidates the solver cache. The locked test below is the intended trip-wire:
/// adding or bumping an algorithm version requires extending this list.
///
/// Per D-15 the digest additionally folds in the [`SolverBudget`] knobs (via
/// [`budget_parts`]), so a budget-default change — or a config-driven budget
/// override — changes the parameter digest and invalidates downstream. This is the
/// "solver budgets participate in the cache key" contract (forward-compatible with
/// CACHE-01/02, Phase 53).
pub(crate) fn solver_provider_parameter_digest(budget: &SolverBudget) -> Digest {
    let budget_parts = budget_parts(budget);
    let mut parts: Vec<&str> = vec![
        SOLVER_SCHEMA_LABEL,
        "derived_edges",
        "derived_edge_provenance",
        "transitive_copy_closure_v1",
        "provenance_projection_v1",
        "precision_ceiling_v1",
        // Go RTA fixpoint algorithm version (Phase 48, D-12): a change to the RTA
        // reachability ⊗ instantiated-types ⊗ dispatch derivation bumps this and
        // deterministically invalidates the solver cache.
        "go_rta_fixpoint_v1",
    ];
    parts.extend(budget_parts.iter().map(String::as_str));
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "solver_provider_parameters",
        &parts,
    )
}

/// The [`SolverBudget`] knobs, rendered as ordered digest parts (D-15).
///
/// Every cross-domain and per-sub-domain budget knob participates, so changing any
/// one changes the parameter digest. Kept in a single helper so the budget recipe is
/// the single source of truth shared by the digest and the locked trip-wire test.
fn budget_parts(budget: &SolverBudget) -> Vec<String> {
    vec![
        format!("budget.max_steps={}", budget.max_steps),
        format!(
            "budget.max_outer_iterations={}",
            budget.max_outer_iterations
        ),
        format!(
            "budget.points_to.max_objects_per_var={}",
            budget.points_to.max_objects_per_var
        ),
        format!(
            "budget.points_to.max_dynamic_vars={}",
            budget.points_to.max_dynamic_vars
        ),
        // Go RTA sub-budget knobs (D-12): a Go-knob change invalidates downstream.
        // Appended AFTER the points-to parts so the existing parts keep their order.
        format!(
            "budget.go.address_taken_threshold={}",
            budget.go.address_taken_threshold
        ),
        format!(
            "budget.go.max_candidates_per_callsite={}",
            budget.go.max_candidates_per_callsite
        ),
        format!("budget.go.max_rta_rounds={}", budget.go.max_rta_rounds),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::solver::budget::SolverBudget;
    use crate::analysis_kernel::incremental::{Digest, DigestKind};

    #[test]
    fn solver_schema_label_is_solver_derived_edges_1() {
        assert_eq!(super::SOLVER_SCHEMA_LABEL, "solver-derived-edges-1");
    }

    #[test]
    fn solver_provider_parameter_digest_locks_parts_list() {
        // The locked recipe: the live digest equals an explicit reconstruction of the
        // frozen algorithm-version parts PLUS the default-budget knobs. Adding or
        // bumping an algorithm version (or a budget knob) requires updating this test.
        let budget = SolverBudget::default();
        assert_eq!(
            solver_provider_parameter_digest(&budget),
            Digest::from_parts(
                DigestKind::ProviderParameters,
                "solver_provider_parameters",
                &[
                    "solver-derived-edges-1",
                    "derived_edges",
                    "derived_edge_provenance",
                    "transitive_copy_closure_v1",
                    "provenance_projection_v1",
                    "precision_ceiling_v1",
                    "go_rta_fixpoint_v1",
                    "budget.max_steps=10000",
                    "budget.max_outer_iterations=64",
                    "budget.points_to.max_objects_per_var=64",
                    "budget.points_to.max_dynamic_vars=512",
                    "budget.go.address_taken_threshold=256",
                    "budget.go.max_candidates_per_callsite=128",
                    "budget.go.max_rta_rounds=32",
                ],
            )
        );
    }

    #[test]
    fn algorithm_version_bump_invalidates_the_pre_bump_digest() {
        // Bumping any frozen algorithm version must deterministically invalidate the
        // solver cache: the live digest differs from a pre-bump parts list. The
        // pre-bump list mirrors the CURRENT recipe (incl. the Go RTA parts) but with
        // one frozen algorithm version rolled back, so this still asserts a DIFFERENCE
        // attributable to the version bump alone.
        let budget = SolverBudget::default();
        let pre_bump = Digest::from_parts(
            DigestKind::ProviderParameters,
            "solver_provider_parameters",
            &[
                "solver-derived-edges-1",
                "derived_edges",
                "derived_edge_provenance",
                "transitive_copy_closure_v0",
                "provenance_projection_v1",
                "precision_ceiling_v1",
                "go_rta_fixpoint_v1",
                "budget.max_steps=10000",
                "budget.max_outer_iterations=64",
                "budget.points_to.max_objects_per_var=64",
                "budget.points_to.max_dynamic_vars=512",
                "budget.go.address_taken_threshold=256",
                "budget.go.max_candidates_per_callsite=128",
                "budget.go.max_rta_rounds=32",
            ],
        );
        assert_ne!(solver_provider_parameter_digest(&budget), pre_bump);
    }

    #[test]
    fn budget_change_invalidates_the_parameter_digest() {
        // D-15: a SolverBudget change participates in the parameter digest, so a
        // budget-default (or override) change invalidates downstream.
        let default_budget = SolverBudget::default();
        let base = solver_provider_parameter_digest(&default_budget);

        let mut bumped_steps = SolverBudget::default();
        bumped_steps.max_steps += 1;
        assert_ne!(
            solver_provider_parameter_digest(&bumped_steps),
            base,
            "changing max_steps must change the parameter digest"
        );

        let mut bumped_outer = SolverBudget::default();
        bumped_outer.max_outer_iterations += 1;
        assert_ne!(
            solver_provider_parameter_digest(&bumped_outer),
            base,
            "changing max_outer_iterations must change the parameter digest"
        );

        let mut bumped_objects = SolverBudget::default();
        bumped_objects.points_to.max_objects_per_var += 1;
        assert_ne!(
            solver_provider_parameter_digest(&bumped_objects),
            base,
            "changing a per-sub-domain budget knob must change the parameter digest"
        );

        // D-12: a Go RTA sub-budget knob (the roadmap-named address-taken threshold)
        // participates in the parameter digest, so a Go-knob change invalidates
        // downstream.
        let mut bumped_go = SolverBudget::default();
        bumped_go.go.address_taken_threshold += 1;
        assert_ne!(
            solver_provider_parameter_digest(&bumped_go),
            base,
            "changing a Go RTA sub-budget knob must change the parameter digest"
        );
    }
}

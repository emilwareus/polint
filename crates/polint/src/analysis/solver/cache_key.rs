//! Solver provider cache key + parameter digest (D-15).
//!
//! Mirrors `analysis::semantic_graph::cache_key`. Defines [`SOLVER_SCHEMA_LABEL`]
//! (the `polint.solver` provider schema label) and
//! [`solver_provider_parameter_digest`], which lists the frozen solver
//! algorithm-version strings AND — per D-15 — the [`SolverBudget`] knobs, so a
//! budget-default change deterministically invalidates the solver cache (budgets
//! participate in the digest and remain forward-compatible with CACHE-01/02).
//!
//! Two locked tests pin the recipe (the established trip-wire pattern):
//! a "parts list" assertion (adding/bumping an algorithm version requires extending
//! the list) and an "algorithm-version bump invalidates" assertion. A third proves
//! that a SolverBudget change changes the parameter digest (D-15).

use crate::analysis::solver::budget::SolverBudget;
use crate::analysis_kernel::incremental::{Digest, DigestKind};

/// Schema label for the `polint.solver` provider manifest.
pub(crate) const SOLVER_SCHEMA_LABEL: &str = "solver-run-output-2";

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
/// CACHE-01/02).
pub(crate) fn solver_provider_parameter_digest(budget: &SolverBudget) -> Digest {
    let budget_parts = solver_budget_digest_parts(budget);
    let mut parts: Vec<&str> = vec![
        SOLVER_SCHEMA_LABEL,
        "derived_edges",
        "derived_edge_provenance",
        "transitive_copy_closure_v1",
        "provenance_projection_v1",
        "precision_ceiling_v1",
        // Go RTA fixpoint algorithm version (D-12): a change to the RTA
        // reachability ⊗ instantiated-types ⊗ dispatch derivation bumps this and
        // deterministically invalidates the solver cache.
        "go_rta_fixpoint_v1",
        // TS function-token fixpoint algorithm version (JS-04): a change
        // to token propagation, callable-token admission, or token-to-call derivation
        // bumps this and deterministically invalidates the solver cache.
        "ts_tokens_fixpoint_v1",
        // TS object/property/prototype/receiver model algorithm version (
        // JS-05): the control plane is present before the real policy lands, so the
        // future driver cannot reuse a pre-object-model solver cache.
        "ts_object_model_fixpoint_v3",
        // Repo-local adaptation model facts (ADAPT-01): accepted facts and
        // budget knobs affect future model-derived solver edges.
        "adaptation_model_v1",
    ];
    parts.extend(budget_parts.iter().map(String::as_str));
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "solver_provider_parameters",
        &parts,
    )
}

/// The active [`SolverBudget`] knobs, rendered as ordered digest parts (D-15).
///
/// Only knobs observable by the production solver participate. Inactive sub-domain
/// knobs are intentionally excluded: changing points-to caps cannot affect
/// `polint.solver` because production no longer registers `PointsToPolicy`, and
/// object-model caps cannot affect output while the object model is disabled.
pub(crate) fn solver_budget_digest_parts(budget: &SolverBudget) -> Vec<String> {
    let mut parts = vec![
        format!("budget.max_steps={}", budget.max_steps),
        format!(
            "budget.max_outer_iterations={}",
            budget.max_outer_iterations
        ),
        // Go RTA sub-budget knobs (D-12): a Go-knob change invalidates downstream.
        format!(
            "budget.go.address_taken_threshold={}",
            budget.go.address_taken_threshold
        ),
        format!(
            "budget.go.max_candidates_per_callsite={}",
            budget.go.max_candidates_per_callsite
        ),
        format!("budget.go.max_rta_rounds={}", budget.go.max_rta_rounds),
        format!(
            "budget.go.max_worklist_steps={}",
            budget.go.max_worklist_steps
        ),
        // JS/TS function-token sub-budget knobs (JS-04): a token-knob change
        // invalidates downstream. Appended after Go so existing parts keep order.
        format!(
            "budget.js.max_tokens_per_var={}",
            budget.js.max_tokens_per_var
        ),
        format!(
            "budget.js.max_candidates_per_callsite={}",
            budget.js.max_candidates_per_callsite
        ),
        format!(
            "budget.js.max_token_worklist_steps={}",
            budget.js.max_token_worklist_steps
        ),
        // JS/TS object-model flag and sub-budget knobs (JS-05): appended after the
        // token fields so the existing digest recipe keeps its established order.
        format!(
            "budget.object_model_enabled={}",
            budget.object_model_enabled
        ),
        format!(
            "budget.adaptation.max_expansions_per_model={}",
            budget.adaptation.max_expansions_per_model
        ),
        format!(
            "budget.adaptation.max_targets_per_source={}",
            budget.adaptation.max_targets_per_source
        ),
        format!(
            "budget.adaptation.max_model_derived_edges={}",
            budget.adaptation.max_model_derived_edges
        ),
    ];

    if budget.object_model_enabled {
        parts.extend([
            format!(
                "budget.object.max_objects_per_place={}",
                budget.object.max_objects_per_place
            ),
            format!(
                "budget.object.max_properties_per_object={}",
                budget.object.max_properties_per_object
            ),
            format!(
                "budget.object.max_tokens_per_property={}",
                budget.object.max_tokens_per_property
            ),
            format!(
                "budget.object.max_computed_buckets_per_object={}",
                budget.object.max_computed_buckets_per_object
            ),
            format!(
                "budget.object.max_prototype_depth={}",
                budget.object.max_prototype_depth
            ),
            format!(
                "budget.object.max_receiver_candidates_per_callsite={}",
                budget.object.max_receiver_candidates_per_callsite
            ),
            format!(
                "budget.object.max_object_worklist_steps={}",
                budget.object.max_object_worklist_steps
            ),
        ]);
    }

    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::solver::budget::SolverBudget;
    use crate::analysis_kernel::incremental::{Digest, DigestKind};

    #[test]
    fn solver_schema_label_is_solver_run_output_2() {
        assert_eq!(super::SOLVER_SCHEMA_LABEL, "solver-run-output-2");
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
                    "solver-run-output-2",
                    "derived_edges",
                    "derived_edge_provenance",
                    "transitive_copy_closure_v1",
                    "provenance_projection_v1",
                    "precision_ceiling_v1",
                    "go_rta_fixpoint_v1",
                    "ts_tokens_fixpoint_v1",
                    "ts_object_model_fixpoint_v3",
                    "adaptation_model_v1",
                    "budget.max_steps=10000",
                    "budget.max_outer_iterations=64",
                    "budget.go.address_taken_threshold=256",
                    "budget.go.max_candidates_per_callsite=128",
                    "budget.go.max_rta_rounds=32",
                    "budget.go.max_worklist_steps=10000",
                    "budget.js.max_tokens_per_var=128",
                    "budget.js.max_candidates_per_callsite=256",
                    "budget.js.max_token_worklist_steps=10000",
                    "budget.object_model_enabled=false",
                    "budget.adaptation.max_expansions_per_model=64",
                    "budget.adaptation.max_targets_per_source=16",
                    "budget.adaptation.max_model_derived_edges=2048",
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
                "solver-run-output-2",
                "derived_edges",
                "derived_edge_provenance",
                "transitive_copy_closure_v0",
                "provenance_projection_v1",
                "precision_ceiling_v1",
                "go_rta_fixpoint_v1",
                "ts_tokens_fixpoint_v1",
                "ts_object_model_fixpoint_v3",
                "adaptation_model_v1",
                "budget.max_steps=10000",
                "budget.max_outer_iterations=64",
                "budget.go.address_taken_threshold=256",
                "budget.go.max_candidates_per_callsite=128",
                "budget.go.max_rta_rounds=32",
                "budget.go.max_worklist_steps=10000",
                "budget.js.max_tokens_per_var=128",
                "budget.js.max_candidates_per_callsite=256",
                "budget.js.max_token_worklist_steps=10000",
                "budget.object_model_enabled=false",
                "budget.adaptation.max_expansions_per_model=64",
                "budget.adaptation.max_targets_per_source=16",
                "budget.adaptation.max_model_derived_edges=2048",
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
        assert_eq!(
            solver_provider_parameter_digest(&bumped_objects),
            base,
            "changing an inactive points-to budget knob must not change the parameter digest"
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

        // CR-01: the Go-scaled worklist-step cap participates in the digest, so a
        // change to it invalidates downstream.
        let mut bumped_steps = SolverBudget::default();
        bumped_steps.go.max_worklist_steps += 1;
        assert_ne!(
            solver_provider_parameter_digest(&bumped_steps),
            base,
            "changing the Go RTA worklist-step cap must change the parameter digest"
        );

        // JS-04: a JS token sub-budget knob participates in the digest, so a token-knob
        // change invalidates downstream.
        let mut bumped_js = SolverBudget::default();
        bumped_js.js.max_tokens_per_var += 1;
        assert_ne!(
            solver_provider_parameter_digest(&bumped_js),
            base,
            "changing a JS token sub-budget knob must change the parameter digest"
        );

        let mut bumped_js_steps = SolverBudget::default();
        bumped_js_steps.js.max_token_worklist_steps += 1;
        assert_ne!(
            solver_provider_parameter_digest(&bumped_js_steps),
            base,
            "changing the JS token worklist-step cap must change the parameter digest"
        );

        let mut toggled_object_model = SolverBudget {
            object_model_enabled: true,
            ..SolverBudget::default()
        };
        assert_ne!(
            solver_provider_parameter_digest(&toggled_object_model),
            base,
            "toggling object-model enablement must change the parameter digest"
        );

        toggled_object_model = SolverBudget::default();
        toggled_object_model.object.max_objects_per_place += 1;
        assert_eq!(
            solver_provider_parameter_digest(&toggled_object_model),
            base,
            "changing disabled object-model caps must not change the parameter digest"
        );

        let mut bumped_object_properties = SolverBudget {
            object_model_enabled: true,
            ..SolverBudget::default()
        };
        let enabled_object_base = solver_provider_parameter_digest(&bumped_object_properties);
        bumped_object_properties.object.max_properties_per_object += 1;
        assert_ne!(
            solver_provider_parameter_digest(&bumped_object_properties),
            enabled_object_base,
            "changing enabled max_properties_per_object must change the parameter digest"
        );

        let mut bumped_object_tokens = SolverBudget {
            object_model_enabled: true,
            ..SolverBudget::default()
        };
        bumped_object_tokens.object.max_tokens_per_property += 1;
        assert_ne!(
            solver_provider_parameter_digest(&bumped_object_tokens),
            enabled_object_base,
            "changing enabled max_tokens_per_property must change the parameter digest"
        );

        let mut bumped_object_computed = SolverBudget {
            object_model_enabled: true,
            ..SolverBudget::default()
        };
        bumped_object_computed
            .object
            .max_computed_buckets_per_object += 1;
        assert_ne!(
            solver_provider_parameter_digest(&bumped_object_computed),
            enabled_object_base,
            "changing enabled max_computed_buckets_per_object must change the parameter digest"
        );

        let mut bumped_object_depth = SolverBudget {
            object_model_enabled: true,
            ..SolverBudget::default()
        };
        bumped_object_depth.object.max_prototype_depth += 1;
        assert_ne!(
            solver_provider_parameter_digest(&bumped_object_depth),
            enabled_object_base,
            "changing enabled max_prototype_depth must change the parameter digest"
        );

        let mut bumped_object_receivers = SolverBudget {
            object_model_enabled: true,
            ..SolverBudget::default()
        };
        bumped_object_receivers
            .object
            .max_receiver_candidates_per_callsite += 1;
        assert_ne!(
            solver_provider_parameter_digest(&bumped_object_receivers),
            enabled_object_base,
            "changing enabled max_receiver_candidates_per_callsite must change the parameter digest"
        );

        let mut bumped_object_steps = SolverBudget {
            object_model_enabled: true,
            ..SolverBudget::default()
        };
        bumped_object_steps.object.max_object_worklist_steps += 1;
        assert_ne!(
            solver_provider_parameter_digest(&bumped_object_steps),
            enabled_object_base,
            "changing enabled max_object_worklist_steps must change the parameter digest"
        );

        let mut bumped_adaptation = SolverBudget::default();
        bumped_adaptation.adaptation.max_model_derived_edges += 1;
        assert_ne!(
            solver_provider_parameter_digest(&bumped_adaptation),
            base,
            "changing an active adaptation model budget knob must change the parameter digest"
        );
    }
}

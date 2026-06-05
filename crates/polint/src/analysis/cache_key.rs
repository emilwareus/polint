use crate::analysis_kernel::incremental::{Digest, DigestKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct V13CacheDependency {
    pub(crate) provider_id: &'static str,
    pub(crate) manifest_inputs: &'static [&'static str],
}

pub(crate) fn v13_cache_dependency_ledger() -> &'static [V13CacheDependency] {
    &[
        V13CacheDependency {
            provider_id: "polint.semantic_graph",
            manifest_inputs: &[
                "go_semantic_functions",
                "go_semantic_callsites",
                "ts_object_allocations",
                "ts_property_writes",
                "ts_property_reads",
                "ts_receiver_bindings",
                "ts_prototype_links",
                "adaptation_model_files",
                "adaptation_model_budget",
            ],
        },
        V13CacheDependency {
            provider_id: "polint.go.semantic",
            manifest_inputs: &[
                "source_files",
                "packages",
                "functions",
                "go.module_roots",
                "go.package_patterns",
                "go.build_tags",
                "go.include_tests",
                "go.offline",
            ],
        },
        V13CacheDependency {
            provider_id: "polint.solver",
            manifest_inputs: &[
                "semantic_constraints",
                "semantic_nodes",
                "points_to_constraints",
                "points_to_sets",
            ],
        },
        V13CacheDependency {
            provider_id: "polint.refined_calls",
            manifest_inputs: &[
                "call_sites",
                "call_targets",
                "unresolved_calls",
                "solver_derived_edges",
            ],
        },
    ]
}

pub(crate) fn semantic_mir_provider_parameter_digest() -> Digest {
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "semantic_mir_provider_parameters",
        &[
            "mir_bodies",
            "mir_operations",
            "places",
            "unsupported_semantics",
            "go_lowering",
            "ts_js_lowering",
            "semantic-mir-facts-1",
        ],
    )
}

#[cfg(test)]
mod semantic_mir_layer_key {
    use super::*;

    #[test]
    fn semantic_mir_provider_parameter_digest_uses_exact_output_and_lowering_terms() {
        assert_eq!(
            super::semantic_mir_provider_parameter_digest(),
            Digest::from_parts(
                DigestKind::ProviderParameters,
                "semantic_mir_provider_parameters",
                &[
                    "mir_bodies",
                    "mir_operations",
                    "places",
                    "unsupported_semantics",
                    "go_lowering",
                    "ts_js_lowering",
                    "semantic-mir-facts-1",
                ],
            )
        );
    }

    #[test]
    fn v13_cache_dependency_ledger_names_cache_sensitive_inputs() {
        let ledger = v13_cache_dependency_ledger();

        assert_eq!(
            ledger
                .iter()
                .map(|entry| entry.provider_id)
                .collect::<Vec<_>>(),
            vec![
                "polint.semantic_graph",
                "polint.go.semantic",
                "polint.solver",
                "polint.refined_calls",
            ]
        );
        assert!(
            ledger
                .iter()
                .find(|entry| entry.provider_id == "polint.solver")
                .unwrap()
                .manifest_inputs
                .contains(&"semantic_constraints")
        );
        assert!(
            ledger
                .iter()
                .find(|entry| entry.provider_id == "polint.refined_calls")
                .unwrap()
                .manifest_inputs
                .contains(&"solver_derived_edges")
        );
    }
}

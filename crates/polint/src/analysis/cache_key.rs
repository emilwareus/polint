use crate::analysis_kernel::incremental::{Digest, DigestKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct V13CacheDependency {
    pub(crate) family: &'static str,
    pub(crate) inputs: &'static [&'static str],
}

pub(crate) fn v13_cache_dependency_ledger() -> &'static [V13CacheDependency] {
    &[
        V13CacheDependency {
            family: "polint.semantic_graph",
            inputs: &[
                "schema_label",
                "algorithm_label",
                "upstream_provider_output_digest",
                "go_semantic_output_digest",
                "adaptation_model_digest",
                "solver_budget",
            ],
        },
        V13CacheDependency {
            family: "polint.go.semantic",
            inputs: &[
                "sidecar_digest",
                "go_version",
                "x_tools_version",
                "go_lifecycle_digest",
                "upstream_provider_output_digest",
            ],
        },
        V13CacheDependency {
            family: "polint.solver",
            inputs: &[
                "schema_label",
                "algorithm_label",
                "upstream_provider_output_digest",
                "solver_budget",
                "budget_status",
                "stable_output_keys",
            ],
        },
        V13CacheDependency {
            family: "polint.refined_calls",
            inputs: &[
                "schema_label",
                "solver_output_digest",
                "upstream_provider_output_digest",
                "stable_output_keys",
            ],
        },
        V13CacheDependency {
            family: "polint.adaptation.model",
            inputs: &[
                "schema_label",
                "validator_version",
                "adaptation_model_digest",
                "adaptation_model_files",
                "accepted_rejected_status",
                "solver_budget",
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
            ledger.iter().map(|entry| entry.family).collect::<Vec<_>>(),
            vec![
                "polint.semantic_graph",
                "polint.go.semantic",
                "polint.solver",
                "polint.refined_calls",
                "polint.adaptation.model",
            ]
        );
        assert!(
            ledger
                .iter()
                .find(|entry| entry.family == "polint.solver")
                .unwrap()
                .inputs
                .contains(&"budget_status")
        );
        assert!(
            ledger
                .iter()
                .find(|entry| entry.family == "polint.adaptation.model")
                .unwrap()
                .inputs
                .contains(&"accepted_rejected_status")
        );
    }
}

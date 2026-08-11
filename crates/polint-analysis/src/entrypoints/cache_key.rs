use polint_analysis_api::{Digest, DigestKind};

pub fn entrypoints_provider_parameter_digest() -> Digest {
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "entrypoints_provider_parameters",
        &[
            "entrypoints-facts-1",
            "entrypoints",
            "trust_boundaries",
            "dispatch_edges",
            "unresolved_framework",
            "go_net_http",
            "go_chi",
            "go_cobra",
            "go_testing",
            "ts_express",
            "ts_mcp_sdk",
            "ts_commander_yargs",
            "ts_jest_vitest_mocha",
        ],
    )
}

#[cfg(test)]
mod entrypoints_provider_parameters {
    use polint_analysis_api::{Digest, DigestKind};

    #[test]
    fn entrypoints_provider_parameters_include_schema_outputs_and_recognizer_labels() {
        assert_eq!(
            super::entrypoints_provider_parameter_digest(),
            Digest::from_parts(
                DigestKind::ProviderParameters,
                "entrypoints_provider_parameters",
                &[
                    "entrypoints-facts-1",
                    "entrypoints",
                    "trust_boundaries",
                    "dispatch_edges",
                    "unresolved_framework",
                    "go_net_http",
                    "go_chi",
                    "go_cobra",
                    "go_testing",
                    "ts_express",
                    "ts_mcp_sdk",
                    "ts_commander_yargs",
                    "ts_jest_vitest_mocha",
                ],
            )
        );
    }
}

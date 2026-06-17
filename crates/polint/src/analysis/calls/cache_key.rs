use crate::analysis_kernel::incremental::{Digest, DigestKind};

pub(crate) fn calls_provider_parameter_digest() -> Digest {
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "calls_provider_parameters",
        &[
            "calls-facts-2",
            "ts-super-ctor-args-3",
            "call_sites",
            "call_targets",
            "unresolved_calls",
            "direct_binding",
            "direct_reference",
            "import_binding",
            "constructor_binding",
            "static_member",
            "direct_member",
            "go_direct",
            "ts_js_direct",
        ],
    )
}

#[cfg(test)]
mod calls_provider_parameters {
    use crate::analysis_kernel::incremental::{Digest, DigestKind};

    #[test]
    fn calls_provider_parameters_include_direct_call_outputs_schema_and_algorithms() {
        assert_eq!(
            super::calls_provider_parameter_digest(),
            Digest::from_parts(
                DigestKind::ProviderParameters,
                "calls_provider_parameters",
                &[
                    "calls-facts-2",
                    "ts-super-ctor-args-3",
                    "call_sites",
                    "call_targets",
                    "unresolved_calls",
                    "direct_binding",
                    "direct_reference",
                    "import_binding",
                    "constructor_binding",
                    "static_member",
                    "direct_member",
                    "go_direct",
                    "ts_js_direct",
                ],
            )
        );
    }
}

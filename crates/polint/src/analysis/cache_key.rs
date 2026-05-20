#[cfg(test)]
mod semantic_mir_layer_key {
    use crate::analysis_kernel::incremental::{Digest, DigestKind};

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
}

use crate::analysis_kernel::incremental::{Digest, DigestKind};

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
}

use crate::analysis_kernel::incremental::{Digest, DigestKind};

/// Schema label for the `polint.identity` provider manifest (Pattern N).
pub(crate) const IDENTITY_SCHEMA_LABEL: &str = "identity-facts-1";

/// Provider parameter digest for `polint.identity` (Pattern D).
///
/// The renderer version strings (`go_relstring_v1`, `jelly_span_v1`) are part of
/// the parameter digest so that any renderer code-version bump deterministically
/// invalidates the identity cache (D-24). The locked test below is the intended
/// trip-wire: adding a downstream renderer/version requires extending this list.
pub(crate) fn identity_provider_parameter_digest() -> Digest {
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "identity_provider_parameters",
        &[
            "identity-facts-1",
            "identity_records",
            "go_relstring_v1",
            "jelly_span_v1",
            "dedup_v1",
            "categorize_v1",
        ],
    )
}

#[cfg(test)]
mod tests {
    use crate::analysis_kernel::incremental::{Digest, DigestKind};

    #[test]
    fn identity_provider_parameter_digest_locks_parts_list() {
        assert_eq!(
            super::identity_provider_parameter_digest(),
            Digest::from_parts(
                DigestKind::ProviderParameters,
                "identity_provider_parameters",
                &[
                    "identity-facts-1",
                    "identity_records",
                    "go_relstring_v1",
                    "jelly_span_v1",
                    "dedup_v1",
                    "categorize_v1",
                ],
            )
        );
    }

    #[test]
    fn identity_schema_label_is_identity_facts_1() {
        assert_eq!(super::IDENTITY_SCHEMA_LABEL, "identity-facts-1");
    }
}

use crate::analysis_kernel::incremental::{Digest, DigestKind};

/// Schema label for the `polint.identity` provider manifest (Pattern N).
pub(crate) const IDENTITY_SCHEMA_LABEL: &str = "identity-facts-1";

/// Provider parameter digest for `polint.identity` (Pattern D).
///
/// The renderer version strings are part of the parameter digest so that any
/// renderer code-version bump deterministically invalidates the identity cache
/// (D-24). The locked test below is the intended trip-wire: adding a downstream
/// renderer/version requires extending this list.
///
/// The Go renderer version was bumped (v1 -> v2) in Plan 05 because the provider
/// now resolves the Go package-clause NAME (`foo.Bar`) for `Language::Go` records
/// instead of the file path (`src/main.go.Bar`), so the Go `package_or_module`
/// content — and thus the IdentityRecord bytes, signature digest, and provider
/// output digest — change.
pub(crate) fn identity_provider_parameter_digest() -> Digest {
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "identity_provider_parameters",
        &[
            "identity-facts-1",
            "identity_records",
            "go_relstring_v2",
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
                    "go_relstring_v2",
                    "jelly_span_v1",
                    "dedup_v1",
                    "categorize_v1",
                ],
            )
        );
    }

    #[test]
    fn go_renderer_version_bump_invalidates_the_pre_bump_digest() {
        // The v1 -> v2 trip-wire bump (Plan 05) must deterministically invalidate
        // the identity cache: the live digest differs from the pre-bump (`v1`)
        // parts list, so cached Go identity records that rendered `src/main.go.Bar`
        // are not silently reused after the provider switched to `foo.Bar`.
        let pre_bump = Digest::from_parts(
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
        );
        assert_ne!(super::identity_provider_parameter_digest(), pre_bump);
    }

    #[test]
    fn identity_schema_label_is_identity_facts_1() {
        assert_eq!(super::IDENTITY_SCHEMA_LABEL, "identity-facts-1");
    }
}

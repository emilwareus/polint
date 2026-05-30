use crate::analysis_kernel::incremental::{Digest, DigestKind};

/// Schema label for the `polint.reachability` provider manifest.
pub(crate) const REACHABILITY_SCHEMA_LABEL: &str = "reachability-facts-1";

/// Provider parameter digest for `polint.reachability`.
///
/// The algorithm-version strings are part of the parameter digest so any bump to
/// a discovery/marking algorithm deterministically invalidates the reachability
/// cache. The locked test below is the intended trip-wire: adding or bumping an
/// algorithm version requires extending this list.
pub(crate) fn reachability_provider_parameter_digest() -> Digest {
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "reachability_provider_parameters",
        &[
            "reachability-facts-1",
            "reachability_roots",
            "call_marks",
            "go_main_init_v1",
            "exported_v1",
            "entrypoint_bridge_v1",
            "configured_roots_v1",
            "bfs_v1",
        ],
    )
}

#[cfg(test)]
mod tests {
    use crate::analysis_kernel::incremental::{Digest, DigestKind};

    #[test]
    fn reachability_provider_parameter_digest_locks_parts_list() {
        assert_eq!(
            super::reachability_provider_parameter_digest(),
            Digest::from_parts(
                DigestKind::ProviderParameters,
                "reachability_provider_parameters",
                &[
                    "reachability-facts-1",
                    "reachability_roots",
                    "call_marks",
                    "go_main_init_v1",
                    "exported_v1",
                    "entrypoint_bridge_v1",
                    "configured_roots_v1",
                    "bfs_v1",
                ],
            )
        );
    }

    #[test]
    fn algorithm_version_bump_invalidates_the_pre_bump_digest() {
        // Bumping any frozen algorithm version must deterministically invalidate
        // the reachability cache: the live digest differs from a pre-bump parts
        // list, so stale roots/marks are not silently reused.
        let pre_bump = Digest::from_parts(
            DigestKind::ProviderParameters,
            "reachability_provider_parameters",
            &[
                "reachability-facts-1",
                "reachability_roots",
                "call_marks",
                "go_main_init_v0",
                "exported_v1",
                "entrypoint_bridge_v1",
                "configured_roots_v1",
                "bfs_v1",
            ],
        );
        assert_ne!(super::reachability_provider_parameter_digest(), pre_bump);
    }

    #[test]
    fn reachability_schema_label_is_reachability_facts_1() {
        assert_eq!(super::REACHABILITY_SCHEMA_LABEL, "reachability-facts-1");
    }
}

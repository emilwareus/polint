#[cfg(test)]
mod tests {
    use crate::analysis_kernel::{
        CachePolicy, LanguageScope, PrecisionCeiling, ProviderKind, ProviderManifest,
        SchemaVersion,
    };
    use crate::analysis_kernel::incremental::{CacheStats, DigestKind, PrecisionTier};

    #[test]
    fn provider_outputs_are_constructed_in_manifest_order() {
        let provider_outputs = crate::analysis_kernel::AnalysisKernel::provider_manifests()
            .iter()
            .map(|manifest| {
                let output_digest = provider_output_digest_from_manifest(
                    manifest,
                    &[format!("provider={}", manifest.id)],
                );
                provider_output_from_manifest(manifest, output_digest, CacheStats::default())
            })
            .collect::<Vec<_>>();

        assert_eq!(
            provider_outputs
                .iter()
                .map(|output| output.provider_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "polint.source",
                "polint.go.syntax",
                "polint.ts.syntax",
                "polint.module_graph",
                "polint.symbol_graph",
                "polint.metrics",
            ]
        );
    }

    #[test]
    fn provider_output_rows_include_manifest_identity_digest_dependencies_and_stats() {
        let mut stats = CacheStats::default();
        stats.record_miss();
        stats.record_recompute();
        stats.record_write();

        for manifest in crate::analysis_kernel::AnalysisKernel::provider_manifests() {
            let output_digest =
                provider_output_digest_from_manifest(manifest, &[format!("provider={}", manifest.id)]);
            let row = provider_output_from_manifest(manifest, output_digest, stats.clone());

            assert_eq!(row.provider_id, manifest.id);
            assert_eq!(row.provider_version, env!("CARGO_PKG_VERSION"));
            assert_eq!(row.schema_version, manifest.primary_schema_label());
            assert_eq!(row.output_digest.kind, DigestKind::ProviderOutput);
            assert!(matches!(
                row.precision,
                PrecisionTier::Exact | PrecisionTier::Syntax | PrecisionTier::SetupAware
            ));
            assert_eq!(row.validation, "native_trusted");
            assert_eq!(row.dependency_inputs.len(), manifest.inputs.len());
            assert_eq!(row.cache_stats, stats);
        }
    }

    #[test]
    fn provider_output_digest_is_deterministic_for_identical_inputs() {
        let manifest = &crate::analysis_kernel::AnalysisKernel::provider_manifests()[1];
        let first = provider_output_digest_from_manifest(
            manifest,
            &["fact=b".to_string(), "fact=a".to_string()],
        );
        let second = provider_output_digest_from_manifest(
            manifest,
            &["fact=a".to_string(), "fact=b".to_string()],
        );

        assert_eq!(first, second);
    }

    #[test]
    fn provider_output_digest_consumes_language_scope_and_cache_policy() {
        const SCHEMAS: &[SchemaVersion] = &[SchemaVersion {
            name: "example-facts",
            version: 1,
        }];

        let base = ProviderManifest {
            id: "polint.example",
            kind: ProviderKind::LanguageSyntax,
            inputs: &["source_files"],
            outputs: &["example_facts"],
            language_scope: LanguageScope::Go,
            cache_policy: CachePolicy::NoCache,
            schema_versions: SCHEMAS,
            precision_ceiling: PrecisionCeiling::Syntax,
        };
        let scope_changed = ProviderManifest {
            language_scope: LanguageScope::TypeScriptJavaScript,
            ..base
        };
        let policy_changed = ProviderManifest {
            cache_policy: CachePolicy::ExistingFileFactCache {
                schema: "example-facts",
            },
            ..base
        };

        let base_digest = provider_output_digest_from_manifest(&base, &["facts=1".to_string()]);

        assert_ne!(
            base_digest,
            provider_output_digest_from_manifest(&scope_changed, &["facts=1".to_string()])
        );
        assert_ne!(
            base_digest,
            provider_output_digest_from_manifest(&policy_changed, &["facts=1".to_string()])
        );
    }
}

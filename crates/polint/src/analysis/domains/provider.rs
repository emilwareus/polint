#[cfg(test)]
mod abstract_domains_provider {
    use super::*;
    use crate::analysis_kernel::AnalysisKernel;
    use crate::analysis_kernel::incremental::{Digest, DigestKind, InputSnapshot};
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::LoadedConfig;
    use crate::core::AnalysisDb;

    #[test]
    fn abstract_domains_provider_accepts_empty_output_with_deterministic_digest() {
        let mut db = AnalysisDb::new();
        let loaded = LoadedConfig::for_test_empty();
        let input_snapshot = InputSnapshot::from_run_inputs(
            &loaded,
            &db,
            "config",
            "rules",
            "plan",
            AnalysisKernel::provider_manifests(),
        );
        let output = derive_abstract_domains_with_cache_stats(
            &mut db,
            &input_snapshot,
            AnalysisKernel::provider_manifests()
                .iter()
                .find(|manifest| manifest.id == "polint.abstract_domains")
                .expect("abstract domains manifest should exist"),
            Digest::absent(DigestKind::ProviderOutput, "semantic_mir"),
            Digest::absent(DigestKind::ProviderOutput, "cfg"),
            Digest::absent(DigestKind::ProviderOutput, "calls"),
            Digest::absent(DigestKind::ProviderOutput, "symbol_graph"),
            Digest::absent(DigestKind::ProviderOutput, "module_topology"),
            Vec::new(),
        );

        assert!(output.diagnostics.is_empty());
        assert!(output.output_digest.is_some());
        assert_eq!(output.cache_stats.recomputed, 1);
    }

    #[test]
    fn abstract_domains_provider_manifest_declares_private_outputs() {
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.abstract_domains")
            .expect("abstract domains manifest should exist");

        assert_eq!(manifest.primary_schema_label(), "abstract-domain-facts-1:1");
        assert!(manifest.outputs.contains(&"domain_observations"));
        assert!(manifest.outputs.contains(&"domain_events"));
    }
}

#[cfg(test)]
mod kernel_run_report_abstract_domains_row_carries_output_digest {
    use crate::analysis_kernel::provider::provider_order_for_test;

    #[test]
    fn abstract_domains_runs_after_calls_and_before_metrics() {
        let order = provider_order_for_test();
        let calls = order
            .iter()
            .position(|provider| *provider == "polint.calls")
            .expect("calls provider");
        let domains = order
            .iter()
            .position(|provider| *provider == "polint.abstract_domains")
            .expect("abstract domains provider");
        let metrics = order
            .iter()
            .position(|provider| *provider == "polint.metrics")
            .expect("metrics provider");

        assert!(calls < domains);
        assert!(domains < metrics);
    }
}

#[cfg(test)]
mod abstract_domains_layer_key {
    use crate::analysis::domains::cache_key::abstract_domains_provider_parameter_digest;
    use crate::analysis_kernel::incremental::{Digest, DigestKind, LayerKey, LayerKind};
    use crate::analysis_kernel::AnalysisKernel;

    #[test]
    fn abstract_domains_layer_key_records_upstream_and_absent_future_inputs() {
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.abstract_domains")
            .expect("abstract domains manifest should exist");
        let key = LayerKey::abstract_domains_layer_key(
            manifest,
            Vec::new(),
            Digest::from_parts(DigestKind::Config, "config", &["a"]),
            Digest::absent(DigestKind::ProviderParameters, "go_lifecycle"),
            Digest::absent(DigestKind::ProviderParameters, "ts_lifecycle"),
            Vec::new(),
            Digest::absent(DigestKind::ProviderOutput, "semantic_mir"),
            Digest::absent(DigestKind::ProviderOutput, "cfg"),
            Digest::absent(DigestKind::ProviderOutput, "calls"),
            Digest::absent(DigestKind::ProviderOutput, "symbol_graph"),
            Digest::absent(DigestKind::ProviderOutput, "module_topology"),
            abstract_domains_provider_parameter_digest(),
        );

        assert_eq!(key.layer_kind, LayerKind::AbstractDomains);
        assert!(key.extension_digests.len() >= 2);
    }
}

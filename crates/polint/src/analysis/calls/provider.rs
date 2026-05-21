#[cfg(test)]
mod calls_provider {
    use crate::analysis_kernel::AnalysisKernel;

    #[test]
    fn calls_provider_accepts_empty_output_with_deterministic_digest() {
        let first = super::calls_output_digest_for_test(&[]);
        let second = super::calls_output_digest_for_test(&[]);

        assert_eq!(first, second);
        assert!(!first.value.is_empty());
    }

    #[test]
    fn calls_provider_manifest_declares_private_outputs() {
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.calls")
            .expect("calls manifest should exist");

        assert_eq!(manifest.primary_schema_label(), "calls-facts-1:1");
        assert!(manifest.outputs.contains(&"call_sites"));
        assert!(manifest.outputs.contains(&"call_targets"));
        assert!(manifest.outputs.contains(&"unresolved_calls"));
    }
}

#[cfg(test)]
mod abstract_domains_provider_parameters {
    use super::*;

    #[test]
    fn abstract_domains_provider_parameters_change_when_policy_inputs_change() {
        let baseline = abstract_domains_provider_parameter_digest();
        let changed = abstract_domains_provider_parameter_digest_for_test(99, 8, 10_000);

        assert_ne!(baseline, changed);
        assert!(baseline.to_string().contains("provider_parameters"));
    }
}

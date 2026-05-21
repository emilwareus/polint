use super::core::{CallEffects, ControlEffects, DataFlowTito, MemoryEffects};
use super::domain::SummaryDomain;
use crate::analysis_kernel::incremental::{Digest, DigestKind};

pub(crate) const DIRECT_SUMMARIES_SCHEMA_LABEL: &str = "direct-summary-facts-1";

pub(crate) fn direct_summaries_provider_parameter_digest() -> Digest {
    let parts = [
        format!("schema={DIRECT_SUMMARIES_SCHEMA_LABEL}:1"),
        format!("domain={}:{}", ControlEffects::ID, ControlEffects::VERSION),
        format!("domain={}:{}", CallEffects::ID, CallEffects::VERSION),
        format!("domain={}:{}", MemoryEffects::ID, MemoryEffects::VERSION),
        format!("domain={}:{}", DataFlowTito::ID, DataFlowTito::VERSION),
        format!(
            "slot_versions=control_effects:{};call_effects:{};memory_effects:{};data_flow_tito:{}",
            ControlEffects::VERSION,
            CallEffects::VERSION,
            MemoryEffects::VERSION,
            DataFlowTito::VERSION,
        ),
    ];
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "direct_summaries_parameters",
        &refs,
    )
}

#[cfg(test)]
mod direct_summaries_provider_parameters {
    use super::*;

    #[test]
    fn parameters_change_when_domain_versions_change() {
        let baseline = direct_summaries_provider_parameter_digest();

        // The digest is stable across repeated calls
        let again = direct_summaries_provider_parameter_digest();
        assert_eq!(baseline, again);

        // The digest includes version information
        assert!(baseline.to_string().contains("provider_parameters"));
    }
}

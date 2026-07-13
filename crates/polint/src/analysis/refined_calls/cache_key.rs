use crate::analysis_kernel::incremental::{Digest, DigestKind, InputSnapshot};
use crate::cache::keys::AnalysisSettingsScope;

const REQUESTED_CAPABILITIES: &[&str] = &["calls", "control_flow", "dataflow"];

pub(crate) const REFINED_CALLS_SCHEMA_LABEL: &str = "refined-call-facts-1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefinedCallsProviderParameters {
    pub(crate) precision_tier: &'static str,
    pub(crate) direct_mirror: bool,
    pub(crate) solver_projection: &'static str,
    pub(crate) retired_heuristic_producers: bool,
}

impl RefinedCallsProviderParameters {
    pub(crate) fn deterministic_default() -> Self {
        Self {
            precision_tier: "setup-aware",
            direct_mirror: true,
            solver_projection: "solver_derived_edges",
            retired_heuristic_producers: true,
        }
    }
}

pub(crate) fn refined_calls_provider_parameter_digest() -> Digest {
    refined_calls_provider_parameter_digest_for_settings(
        &RefinedCallsProviderParameters::deterministic_default(),
    )
}

pub(crate) fn refined_calls_provider_parameter_digest_for_snapshot(
    input_snapshot: &InputSnapshot,
    upstream_output_digests: &[Digest],
) -> Digest {
    let mut parts = parameter_parts(&RefinedCallsProviderParameters::deterministic_default());
    parts.push(format!(
        "analysis_settings={}",
        input_snapshot.analysis_settings_digest(AnalysisSettingsScope::RefinedCalls)
    ));
    parts.push(format!(
        "requested_capabilities={}",
        input_snapshot.analysis_requirements_digest_for(REQUESTED_CAPABILITIES)
    ));
    parts.extend(
        upstream_output_digests
            .iter()
            .map(|digest| format!("upstream={digest}")),
    );
    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "refined_calls_inputs",
        &refs,
    )
}

fn refined_calls_provider_parameter_digest_for_settings(
    settings: &RefinedCallsProviderParameters,
) -> Digest {
    let parts = parameter_parts(settings);
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "refined_calls_parameters",
        &refs,
    )
}

fn parameter_parts(settings: &RefinedCallsProviderParameters) -> Vec<String> {
    vec![
        REFINED_CALLS_SCHEMA_LABEL.to_string(),
        format!("precision_tier={}", settings.precision_tier),
        format!("direct_mirror={}", settings.direct_mirror),
        format!("solver_projection={}", settings.solver_projection),
        format!(
            "retired_heuristic_producers={}",
            settings.retired_heuristic_producers
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refined_calls_parameters_include_schema_and_planned_refinement_knobs() {
        let digest = refined_calls_provider_parameter_digest();

        assert_eq!(
            digest,
            refined_calls_provider_parameter_digest_for_settings(
                &RefinedCallsProviderParameters::deterministic_default()
            )
        );
    }
}

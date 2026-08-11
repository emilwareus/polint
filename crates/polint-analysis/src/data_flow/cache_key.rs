use polint_analysis_api::{Digest, DigestKind, InputComponent, InputSnapshot};

pub const DATA_FLOW_SCHEMA_LABEL: &str = "data-flow-facts-1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataFlowProviderParameters {
    pub local_mir: bool,
    pub direct_calls: bool,
    pub summary_projection: bool,
    pub extension_models: bool,
    pub max_query_depth: u32,
    pub max_path_count: u32,
}

impl DataFlowProviderParameters {
    pub fn deterministic_default() -> Self {
        Self {
            local_mir: true,
            direct_calls: true,
            summary_projection: true,
            extension_models: true,
            max_query_depth: 32,
            max_path_count: 256,
        }
    }
}

pub fn data_flow_provider_parameter_digest() -> Digest {
    data_flow_provider_parameter_digest_for_settings(
        &DataFlowProviderParameters::deterministic_default(),
    )
}

pub fn data_flow_provider_parameter_digest_for_snapshot(
    input_snapshot: &InputSnapshot,
    upstream_output_digests: &[Digest],
) -> Digest {
    let mut parts = parameter_parts(&DataFlowProviderParameters::deterministic_default());
    parts.push(format!("config={}", input_snapshot.config.digest));
    extend_component_parts(
        &mut parts,
        "go_lifecycle",
        &input_snapshot.go_lifecycle.components,
    );
    extend_component_parts(
        &mut parts,
        "ts_js_lifecycle",
        &input_snapshot.ts_js_lifecycle.components,
    );
    extend_component_parts(&mut parts, "extension", &input_snapshot.extensions);
    extend_component_parts(&mut parts, "model", &input_snapshot.models);
    extend_component_parts(&mut parts, "tool", &input_snapshot.tool_invocations);
    parts.extend(
        upstream_output_digests
            .iter()
            .map(|digest| format!("upstream={digest}")),
    );
    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderParameters, "data_flow_inputs", &refs)
}

fn data_flow_provider_parameter_digest_for_settings(
    settings: &DataFlowProviderParameters,
) -> Digest {
    let parts = parameter_parts(settings);
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "data_flow_parameters",
        &refs,
    )
}

fn parameter_parts(settings: &DataFlowProviderParameters) -> Vec<String> {
    vec![
        DATA_FLOW_SCHEMA_LABEL.to_string(),
        format!("local_mir={}", settings.local_mir),
        format!("direct_calls={}", settings.direct_calls),
        format!("summary_projection={}", settings.summary_projection),
        format!("extension_models={}", settings.extension_models),
        format!("max_query_depth={}", settings.max_query_depth),
        format!("max_path_count={}", settings.max_path_count),
    ]
}

fn extend_component_parts(parts: &mut Vec<String>, prefix: &str, components: &[InputComponent]) {
    if components.is_empty() {
        parts.push(format!("{prefix}=absent"));
        return;
    }
    parts.extend(components.iter().map(|component| {
        format!(
            "{prefix}:{}:{:?}:{}",
            component.name, component.status, component.digest
        )
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameters_include_schema_and_budget_knobs() {
        let digest = data_flow_provider_parameter_digest();

        assert_eq!(
            digest,
            data_flow_provider_parameter_digest_for_settings(
                &DataFlowProviderParameters::deterministic_default()
            )
        );
    }
}

use crate::analysis_kernel::incremental::{Digest, DigestKind, InputComponent, InputSnapshot};

pub(crate) const TYPE_VALUE_ALIAS_SCHEMA_LABEL: &str = "type-value-alias-facts-1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypeValueAliasProviderParameters {
    pub(crate) precision_tier: &'static str,
    pub(crate) alias_budget: u32,
    pub(crate) points_to_budget: u32,
    pub(crate) extension_slot: &'static str,
    pub(crate) model_slot: &'static str,
    pub(crate) tool_slot: &'static str,
}

impl TypeValueAliasProviderParameters {
    pub(crate) fn deterministic_default() -> Self {
        Self {
            precision_tier: "setup-aware",
            alias_budget: 0,
            points_to_budget: 0,
            extension_slot: "absent",
            model_slot: "absent",
            tool_slot: "absent",
        }
    }
}

pub(crate) fn type_value_alias_provider_parameter_digest() -> Digest {
    type_value_alias_provider_parameter_digest_for_settings(
        &TypeValueAliasProviderParameters::deterministic_default(),
    )
}

pub(crate) fn type_value_alias_provider_parameter_digest_for_snapshot(
    input_snapshot: &InputSnapshot,
    upstream_output_digests: &[Digest],
) -> Digest {
    let mut parts = parameter_parts(&TypeValueAliasProviderParameters::deterministic_default());
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
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "type_value_alias_inputs",
        &refs,
    )
}

fn type_value_alias_provider_parameter_digest_for_settings(
    settings: &TypeValueAliasProviderParameters,
) -> Digest {
    let parts = parameter_parts(settings);
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "type_value_alias_parameters",
        &refs,
    )
}

fn parameter_parts(settings: &TypeValueAliasProviderParameters) -> Vec<String> {
    vec![
        format!("schema={TYPE_VALUE_ALIAS_SCHEMA_LABEL}:1"),
        "output=type_facts".to_string(),
        "output=narrowed_type_facts".to_string(),
        "output=value_facts".to_string(),
        "output=allocation_tokens".to_string(),
        "output=access_paths".to_string(),
        "output=points_to_constraints".to_string(),
        "output=points_to_sets".to_string(),
        "output=alias_answers".to_string(),
        format!("precision_tier={}", settings.precision_tier),
        format!("alias_budget={}", settings.alias_budget),
        format!("points_to_budget={}", settings.points_to_budget),
        format!("extension_slot={}", settings.extension_slot),
        format!("model_slot={}", settings.model_slot),
        format!("tool_slot={}", settings.tool_slot),
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
pub(crate) fn type_value_alias_provider_parameter_digest_for_test(
    precision_tier: &'static str,
    alias_budget: u32,
    points_to_budget: u32,
    extension_slot: &'static str,
    model_slot: &'static str,
    tool_slot: &'static str,
) -> Digest {
    type_value_alias_provider_parameter_digest_for_settings(&TypeValueAliasProviderParameters {
        precision_tier,
        alias_budget,
        points_to_budget,
        extension_slot,
        model_slot,
        tool_slot,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_parameter_digest_changes_for_behavior_inputs() {
        let baseline = type_value_alias_provider_parameter_digest();
        for changed in [
            type_value_alias_provider_parameter_digest_for_test(
                "exact", 0, 0, "absent", "absent", "absent",
            ),
            type_value_alias_provider_parameter_digest_for_test(
                "setup-aware",
                1,
                0,
                "absent",
                "absent",
                "absent",
            ),
            type_value_alias_provider_parameter_digest_for_test(
                "setup-aware",
                0,
                1,
                "absent",
                "absent",
                "absent",
            ),
            type_value_alias_provider_parameter_digest_for_test(
                "setup-aware",
                0,
                0,
                "extension-present",
                "absent",
                "absent",
            ),
            type_value_alias_provider_parameter_digest_for_test(
                "setup-aware",
                0,
                0,
                "absent",
                "model-present",
                "absent",
            ),
            type_value_alias_provider_parameter_digest_for_test(
                "setup-aware",
                0,
                0,
                "absent",
                "absent",
                "tool-present",
            ),
        ] {
            assert_ne!(baseline, changed);
        }
    }
}
